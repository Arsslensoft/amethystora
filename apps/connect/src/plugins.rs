//! What a paired phone's packets do (spec-plugins.md), and what the bus can ask of it. Every handler parses,
//! updates the device's state, tells the desktop and returns: anything slow runs as a task of its own, so that
//! the connection is never left unread.

use std::cell::RefCell;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::rc::Rc;
use std::str::FromStr;
use std::time::Instant;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use glib::variant::ToVariant;
use serde_json::json;

use crate::battery::Alert;
use crate::codes;
use crate::connectivity;
use crate::device::Device;
use crate::limits;
use crate::link::Link;
use crate::media;
use crate::netinfo;
use crate::notifications::{self, Incoming, Kind, Outcome, Parsed};
use crate::notify::Notice;
use crate::packet::{self, Packet, PAIR};
use crate::payload;
use crate::service::{ApiError, ApiResult, Service};
use crate::share::{self, Shared};
use crate::sms;
use crate::telephony;
use crate::text::{self, tr, trf};
use crate::{dbus, icons};

fn nkey(device: &str, id: &str) -> String {
    format!("n:{device}:{id}")
}

impl Service {
    /// A packet from a connection that passed the handshake
    pub fn on_packet(self: &Rc<Self>, dev: &Rc<Device>, link: &Rc<Link>, packet: Packet) {
        if self.cfg.verbose {
            self.log(&format!("{} sent {}", dev.id, packet.kind));
        }
        let tap = self.tap.borrow().clone();
        if let Some(tap) = tap {
            tap(&dev.id, &packet);
        }
        if packet.kind == PAIR {
            return self.on_pair_packet(dev, link, &packet);
        }
        if !dev.paired() {
            return self.unpaired_packet(dev, link);
        }
        // The phone as a slide clicker: its keys are heard although we never list them (media.rs says why)
        if packet.kind == media::KEYS {
            return self.on_click(dev, &packet);
        }
        // What we did not say we handle is dropped, whoever sends it
        if !self.local.borrow().handles(&packet.kind) {
            return;
        }
        match packet.kind.as_str() {
            "kdeconnect.ping" => self.on_ping(dev, &packet),
            "kdeconnect.battery" => self.on_battery(dev, &packet),
            "kdeconnect.notification" => self.on_notification(dev, link, packet),
            "kdeconnect.share.request" => self.on_share(dev, link, packet),
            "kdeconnect.share.request.update" => {}
            "kdeconnect.clipboard" => self.on_clipboard(dev, &packet),
            "kdeconnect.clipboard.connect" => self.on_clipboard_connect(dev, &packet),
            "kdeconnect.contacts.response_uids_timestamps" => self.on_contact_times(dev, &packet),
            "kdeconnect.contacts.response_vcards" => self.on_contact_cards(dev, &packet),
            "kdeconnect.sms.messages" => self.on_sms(dev, &packet),
            "kdeconnect.telephony" => self.on_telephony(dev, &packet),
            "kdeconnect.connectivity_report" => self.on_connectivity(dev, &packet),
            media::MPRIS => self.on_mpris(dev, &packet),
            media::MPRIS_REQUEST => self.on_mpris_request(dev, &packet.body),
            // Listed so that the phone offers its presenter; its pointer is never acted on
            media::PRESENTER => {}
            // The ones we only list so that the phone offers them: battery.request, notification.request
            _ => {}
        }
    }

    /// A paired phone is connected (or has just been paired): ask for what it knows
    pub fn on_connected(self: &Rc<Self>, dev: &Rc<Device>) {
        self.send_to(dev, "kdeconnect.battery.request", json!({ "request": true }));
        self.send_to(dev, "kdeconnect.notification.request", json!({ "request": true }));
        if self.clip_send.get() {
            let (text, stamp) = {
                let clip = self.clip.borrow();
                (clip.text().to_string(), clip.stamp())
            };
            if stamp != 0 && !text.is_empty() && text.len() <= limits::CLIPBOARD_TEXT {
                self.send_to(dev, "kdeconnect.clipboard.connect", json!({ "content": text, "timestamp": stamp }));
            }
        }
        self.send_to(dev, "kdeconnect.contacts.request_all_uids_timestamps", json!({}));
        self.send_to(dev, "kdeconnect.sms.request_conversations", json!({}));
        // Music from either side: the phone's players are asked for, and this computer's watched while it sees them
        if self.music.get() && dev.sends(media::MPRIS) {
            self.send_to(dev, media::MPRIS_REQUEST, media::ask_list());
        }
        self.sync_players();
        dbus::props_changed(self, dev);
    }

    /// The last connection of a phone ended: what we showed of it goes
    pub fn on_disconnected(self: &Rc<Self>, dev: &Rc<Device>) {
        let (entries, had_call) = {
            let mut st = dev.st.borrow_mut();
            st.battery.disconnected();
            // Away, the bars simply go: a phone that comes back says its signal again when it changes
            st.signal = None;
            st.contacts_queue.clear();
            st.players.clear();
            st.last_click = None;
            (st.notifications.clear(), st.call.take().is_some())
        };
        self.sync_players();
        for entry in entries {
            self.notifier.close(&nkey(&dev.id, &entry.id));
        }
        for key in ["battery", "call", "ping"] {
            self.notifier.close(&format!("{key}:{}", dev.id));
        }
        if had_call {
            dbus::emit(self, Some(dev), "Call", Some(("ended", "", "").to_variant()));
        }
    }

    /// Everything kept for a phone is dropped: it is no longer paired
    pub fn clear_device_state(self: &Rc<Self>, dev: &Rc<Device>) {
        self.on_disconnected(dev);
        let mut st = dev.st.borrow_mut();
        st.sms = sms::Sms::default();
        st.contacts = crate::contacts::Book::default();
        st.battery = crate::battery::Battery::default();
        st.batch = None;
    }

    // --- Ping and battery ----------------------------------------------------------------------------------

    fn on_ping(self: &Rc<Self>, dev: &Rc<Device>, p: &Packet) {
        {
            let mut st = dev.st.borrow_mut();
            if st.last_ping.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(2)) {
                return;
            }
            st.last_ping = Some(Instant::now());
        }
        let message = p.str("message").map(|m| text::shorten(&text::clean_line(m), limits::PING_TEXT)).filter(|m| !m.trim().is_empty());
        self.raise(Notice {
            key: format!("ping:{}", dev.id),
            summary: dev.name(),
            body: message.unwrap_or_else(|| tr("Ping!")),
            icon: Some("smartphone-symbolic".into()),
            ..Notice::default()
        });
    }

    fn on_battery(self: &Rc<Self>, dev: &Rc<Device>, p: &Packet) {
        let (alert, level) = {
            let mut st = dev.st.borrow_mut();
            let alert = st.battery.apply(&p.body);
            (alert, st.battery.level)
        };
        match alert {
            Alert::Raise => self.raise(Notice {
                key: format!("battery:{}", dev.id),
                summary: trf("The battery of “{name}” is low", &[("name", &dev.name())]),
                body: trf("{percent}% left", &[("percent", &level.max(0).to_string())]),
                icon: Some("battery-caution-symbolic".into()),
                ..Notice::default()
            }),
            Alert::Withdraw => self.notifier.close(&format!("battery:{}", dev.id)),
            Alert::None => {}
        }
        dbus::props_changed(self, dev);
    }

    /// The phone's mobile signal. A body without the map says nothing; a map with nothing valid in it means the phone
    /// has no signal to tell of. Only a phone that listed the report as one it sends is heard: the program never asks
    /// for one (the request is deprecated, and phones ignore it).
    fn on_connectivity(self: &Rc<Self>, dev: &Rc<Device>, p: &Packet) {
        if !dev.sends("kdeconnect.connectivity_report") || !connectivity::is_report(&p.body) {
            return;
        }
        dev.st.borrow_mut().signal = connectivity::parse(&p.body);
        dbus::props_changed(self, dev);
    }

    // --- Notifications -------------------------------------------------------------------------------------

    fn refuse_payload(self: &Rc<Self>, link: &Rc<Link>, dev: &Rc<Device>, payload_: Option<packet::Payload>) {
        let (Some(pl), Some(pin)) = (payload_, self.pin_of(&dev.id)) else { return };
        let (ip, scope, cert) = (link.ip(), link.peer.2, self.me.tls.clone());
        self.spawn_task(async move { payload::refuse(ip, scope, pl.port, &cert, pin).await });
    }

    fn on_notification(self: &Rc<Self>, dev: &Rc<Device>, link: &Rc<Link>, p: Packet) {
        match notifications::parse(&p) {
            Parsed::Ignore => self.refuse_payload(link, dev, p.payload().ok().flatten()),
            Parsed::Cancel(id) => {
                let entry = dev.st.borrow_mut().notifications.cancelled(&id);
                if let Some(entry) = entry {
                    self.notifier.close(&nkey(&dev.id, &entry.id));
                }
            }
            Parsed::Show(n) => {
                let outcome = dev.st.borrow_mut().notifications.incoming(&n);
                match outcome {
                    Outcome::Show { id } => self.show_phone_notification(dev, link, *n, id),
                    Outcome::Same | Outcome::Skip => {
                        self.log(&format!("a notification of {} is not shown (silent: {}, can be cleared: {}, already shown: {})", dev.id, n.silent, n.clearable, matches!(outcome, Outcome::Same)));
                        self.refuse_payload(link, dev, n.payload)
                    }
                }
            }
        }
    }

    /// Raises (or replaces) the desktop notification of a phone's, with its icon if it comes in time
    fn show_phone_notification(self: &Rc<Self>, dev: &Rc<Device>, link: &Rc<Link>, n: Incoming, id: String) {
        let service = self.clone();
        let (dev, link) = (dev.clone(), link.clone());
        self.spawn_task(async move {
            let mut icon = None;
            let wanted = n.icon_hash.as_deref().map(|hash| service.icons_dir(&dev.id).join(format!("{hash}.png")));
            match (&wanted, n.payload) {
                (Some(path), pl) if path.exists() => {
                    icon = Some(path.clone());
                    service.refuse_payload(&link, &dev, pl);
                }
                (Some(path), Some(pl)) if pl.size <= limits::ICON_BYTES && service.icons_active.get() < limits::ICON_DOWNLOADS => {
                    if let Some(pin) = service.pin_of(&dev.id) {
                        service.icons_active.set(service.icons_active.get() + 1);
                        let bytes = Rc::new(RefCell::new(Vec::<u8>::new()));
                        let sink = bytes.clone();
                        let fetched = glib::future_with_timeout(limits::ICON_WAIT, async {
                            payload::fetch(link.ip(), link.peer.2, pl.port, &service.me.tls, pin, pl.size, &mut |chunk| {
                                sink.borrow_mut().extend_from_slice(chunk);
                                Ok(())
                            }, &|_| {})
                            .await
                        })
                        .await;
                        service.icons_active.set(service.icons_active.get().saturating_sub(1));
                        if matches!(fetched, Ok(Ok(_))) && icons::save_png(&bytes.borrow(), path) {
                            icons::trim(path.parent().unwrap_or(path));
                            icon = Some(path.clone());
                        }
                    }
                }
                (_, pl) => service.refuse_payload(&link, &dev, pl),
            }
            // It may have been closed, or replaced, while the icon came
            let current = dev.st.borrow().notifications.by_phone_id(&n.id).map(|e| e.id == id).unwrap_or(false);
            if !current || service.stopping.get() {
                return;
            }
            let notice = service.phone_notice(&dev, &n, &id, icon.map(|p| p.to_string_lossy().into_owned()));
            service.raise(notice);
        });
    }

    fn phone_notice(&self, dev: &Rc<Device>, n: &Incoming, id: &str, icon: Option<String>) -> Notice {
        let words = notifications::words(n, &dev.name());
        let mut actions: Vec<(String, String)> = Vec::new();
        let can_reply = n.reply_id.is_some() && dev.takes("kdeconnect.notification.reply");
        // A click does something only where there is something to open
        if n.kind == Kind::Sms || can_reply {
            actions.push(("default".into(), String::new()));
        }
        if can_reply {
            actions.push(("reply".into(), tr("Reply")));
        }
        for (i, label) in n.actions.iter().take(limits::NOTIFICATION_BUTTONS).enumerate() {
            if dev.takes("kdeconnect.notification.action") {
                actions.push((format!("action:{i}"), text::shorten(label, 40)));
            }
        }
        if n.clearable && dev.takes("kdeconnect.notification.request") {
            actions.push(("dismiss".into(), tr("Dismiss")));
        }
        let fallback = match n.kind {
            Kind::MissedCall => "call-missed-symbolic",
            Kind::Sms | Kind::Chat => "chat-message-new-symbolic",
            Kind::Plain => "smartphone-symbolic",
        };
        // A text message has a sound of its own in the desktop's sound theme (the Soft set's three quick notes)
        let sound = (n.kind == Kind::Sms).then_some("message-new-instant-phone");
        Notice { key: nkey(&dev.id, id), summary: words.heading, body: words.body, icon: Some(icon.unwrap_or_else(|| fallback.into())), actions, sound, ..Notice::default() }
    }

    /// The person did something with the desktop notification of a phone's (`rest` is its id, `None` is closing it)
    pub fn on_phone_notification_event(self: &Rc<Self>, dev: &Rc<Device>, id: &str, action: Option<&str>) {
        let Some(entry) = dev.st.borrow().notifications.by_id(id).cloned() else { return };
        match action {
            None => {
                // Closed by the person: the phone is asked to close it too
                dev.st.borrow_mut().notifications.dismissed(id);
                if entry.clearable {
                    self.send_to(dev, "kdeconnect.notification.request", notifications::cancel_request(&entry.phone_id));
                }
            }
            Some("reply") => self.launch_app(&["reply", "--", &dev.id, id]),
            Some("dismiss") => {
                let _ = self.dismiss_notification(&dev.id, id);
            }
            Some("default") => {
                if entry.kind == Kind::Sms {
                    self.launch_app(&["messages", "--", &dev.id]);
                } else if entry.reply_id.is_some() {
                    self.launch_app(&["reply", "--", &dev.id, id]);
                }
            }
            Some(action) => {
                let label = action.strip_prefix("action:").and_then(|i| i.parse::<usize>().ok()).and_then(|i| entry.actions.get(i));
                if let Some(label) = label {
                    self.send_to(dev, "kdeconnect.notification.action", json!({ "key": entry.phone_id, "action": label }));
                }
            }
        }
    }

    pub fn reply_to_notification(self: &Rc<Self>, device: &str, id: &str, reply: &str) -> ApiResult<()> {
        let dev = self.device(device).ok_or_else(|| ApiError::unavailable("no such device"))?;
        let reply_id = dev.st.borrow().notifications.by_id(id).and_then(|e| e.reply_id.clone()).ok_or_else(|| ApiError::unavailable("this notification cannot be answered"))?;
        if reply.trim().is_empty() {
            return Err(ApiError::InvalidArgs("an empty reply".into()));
        }
        let message = text::shorten(reply, limits::NOTIFICATION_BODY);
        self.require_send(device, "kdeconnect.notification.reply", json!({ "requestReplyId": reply_id, "message": message }))
    }

    pub fn dismiss_notification(self: &Rc<Self>, device: &str, id: &str) -> ApiResult<()> {
        let dev = self.device(device).ok_or_else(|| ApiError::unavailable("no such device"))?;
        let entry = dev.st.borrow_mut().notifications.dismissed(id).ok_or_else(|| ApiError::unavailable("no such notification"))?;
        self.notifier.close(&nkey(&dev.id, &entry.id));
        if entry.clearable {
            self.send_to(&dev, "kdeconnect.notification.request", notifications::cancel_request(&entry.phone_id));
        }
        Ok(())
    }

    pub fn repliable(&self, device: &str) -> ApiResult<Vec<HashMap<String, glib::Variant>>> {
        let dev = self.device(device).ok_or_else(|| ApiError::unavailable("no such device"))?;
        let st = dev.st.borrow();
        Ok(st
            .notifications
            .repliable()
            .into_iter()
            .map(|e| {
                let mut m = HashMap::new();
                m.insert("id".to_string(), e.id.to_variant());
                m.insert("app".to_string(), e.app.to_variant());
                m.insert("title".to_string(), e.title.to_variant());
                m.insert("text".to_string(), e.text.to_variant());
                // The app's icon as the program keeps it (a PNG it wrote itself from the phone's picture), when it came
                if let Some(path) = e.icon_hash.as_ref().map(|hash| self.icons_dir(&dev.id).join(format!("{hash}.png"))).filter(|p| p.is_file()) {
                    m.insert("icon".to_string(), path.to_string_lossy().into_owned().to_variant());
                }
                m
            })
            .collect())
    }

    // --- Clipboard ----------------------------------------------------------------------------------------

    fn apply_remote_clipboard(self: &Rc<Self>, dev: &Rc<Device>, text: Option<String>) {
        if let Some(text) = text {
            // A phone's clipboard may hold a password: only the module, which sets the clipboard with it, is sent it
            dbus::emit(self, None, "ClipboardReceived", Some((dev.id.as_str(), text.as_str()).to_variant()));
        }
    }

    fn on_clipboard(self: &Rc<Self>, dev: &Rc<Device>, p: &Packet) {
        let Some(content) = p.str("content") else { return };
        let applied = self.clip.borrow_mut().remote_changed(&dev.id, content, self.clip_receive.get(), packet::now_ms());
        self.apply_remote_clipboard(dev, applied);
    }

    fn on_clipboard_connect(self: &Rc<Self>, dev: &Rc<Device>, p: &Packet) {
        let (Some(content), Some(stamp)) = (p.str("content"), p.int("timestamp")) else { return };
        let applied = self.clip.borrow_mut().remote_connected(&dev.id, content, stamp, self.clip_receive.get(), packet::now_ms());
        self.apply_remote_clipboard(dev, applied);
    }

    // --- Contacts and texts -------------------------------------------------------------------------------

    fn on_contact_times(self: &Rc<Self>, dev: &Rc<Device>, p: &Packet) {
        let ask = dev.st.borrow_mut().contacts.on_timestamps(&p.body);
        if ask.is_empty() {
            self.save_contacts(dev);
            return;
        }
        dev.st.borrow_mut().contacts_queue = ask.into();
        self.ask_next_cards(dev);
    }

    fn ask_next_cards(self: &Rc<Self>, dev: &Rc<Device>) {
        let batch: Vec<String> = {
            let mut st = dev.st.borrow_mut();
            let n = st.contacts_queue.len().min(limits::CONTACT_CARDS_PER_REQUEST);
            st.contacts_queue.drain(..n).collect()
        };
        if batch.is_empty() {
            self.save_contacts(dev);
            dbus::emit(self, Some(dev), "ConversationsChanged", None);
        } else {
            self.send_to(dev, "kdeconnect.contacts.request_vcards_by_uid", json!({ "uids": batch }));
        }
    }

    fn on_contact_cards(self: &Rc<Self>, dev: &Rc<Device>, p: &Packet) {
        dev.st.borrow_mut().contacts.on_vcards(&p.body);
        self.ask_next_cards(dev);
    }

    fn save_contacts(&self, dev: &Rc<Device>) {
        if let Err(e) = dev.st.borrow().contacts.save(&self.contacts_path(&dev.id)) {
            eprintln!("amethystora-connect: cannot keep the contacts: {e}");
        }
    }

    fn on_sms(self: &Rc<Self>, dev: &Rc<Device>, p: &Packet) {
        // A code from your phone (the key codes): only while it is on is any text looked at here, and only the digits of a
        // code leave, as one signal for the module; the body, the sender and the thread stay in this program
        let looking = self.codes.get().then(packet::now_ms);
        let changes = dev.st.borrow_mut().sms.apply_looking(&p.body, looking);
        if let Some(code) = changes.fresh.iter().find_map(|body| codes::find_code(body)) {
            dbus::emit(self, None, "Code", Some((code,).to_variant()));
        }
        for thread in &changes.threads {
            dbus::emit(self, Some(dev), "MessagesChanged", Some((*thread,).to_variant()));
        }
        if changes.conversations {
            dbus::emit(self, Some(dev), "ConversationsChanged", None);
        }
    }

    pub fn request_conversations(&self, device: &str) -> ApiResult<()> {
        self.require_send(device, "kdeconnect.sms.request_conversations", json!({}))
    }

    pub fn list_conversations(&self, device: &str, limit: i32) -> ApiResult<Vec<HashMap<String, glib::Variant>>> {
        let dev = self.device(device).ok_or_else(|| ApiError::unavailable("no such device"))?;
        let st = dev.st.borrow();
        let names = |a: &str| st.contacts.name_of(a).map(str::to_string);
        let limit = if limit <= 0 { 100 } else { (limit as usize).min(limits::THREADS_KEPT) };
        Ok(st.sms.conversations(limit, &names).into_iter().map(to_dict).collect())
    }

    pub fn request_messages(&self, device: &str, thread: i64, before: i64, count: i32) -> ApiResult<()> {
        self.require_send(device, "kdeconnect.sms.request_conversation", sms::thread_request(thread, before.max(0), count.clamp(0, 200)))
    }

    pub fn list_messages(&self, device: &str, thread: i64, limit: i32) -> ApiResult<Vec<HashMap<String, glib::Variant>>> {
        let dev = self.device(device).ok_or_else(|| ApiError::unavailable("no such device"))?;
        let st = dev.st.borrow();
        let names = |a: &str| st.contacts.name_of(a).map(str::to_string);
        let limit = if limit <= 0 { 100 } else { (limit as usize).min(limits::MESSAGES_PER_THREAD) };
        Ok(st.sms.messages(thread, limit, &names).into_iter().map(to_dict).collect())
    }

    /// Sends a text. One recipient in the first version: a group is read on the computer and answered on the phone.
    pub fn send_message(&self, device: &str, addresses: &[String], body: &str, thread: i64) -> ApiResult<()> {
        let dev = self.device(device).ok_or_else(|| ApiError::unavailable("no such device"))?;
        if body.trim().is_empty() {
            return Err(ApiError::InvalidArgs("an empty message".into()));
        }
        let body = text::clean(body);
        if body.len() > limits::MESSAGE_BODY {
            return Err(ApiError::InvalidArgs("the message is too long".into()));
        }
        let to: Vec<String> = if thread != 0 {
            dev.st.borrow().sms.participants(thread).map(<[String]>::to_vec).unwrap_or_else(|| addresses.to_vec())
        } else {
            addresses.to_vec()
        };
        let to: Vec<String> = to.iter().map(|a| text::clean_line(a).trim().to_string()).filter(|a| !a.is_empty() && a.len() <= 64).collect();
        match to.len() {
            0 => return Err(ApiError::InvalidArgs("no recipient".into())),
            1 => {}
            _ => return Err(ApiError::unavailable("group conversations are read-only; answer on the phone")),
        }
        let documented = dev.st.borrow().protocol >= 8;
        self.require_send(device, "kdeconnect.sms.request", sms::send_request(&to, &body, documented))
    }

    // --- Calls --------------------------------------------------------------------------------------------

    fn on_telephony(self: &Rc<Self>, dev: &Rc<Device>, p: &Packet) {
        let Some(event) = telephony::parse(&p.body) else { return };
        let key = format!("call:{}", dev.id);
        let name = if event.name.trim().is_empty() { dev.st.borrow().contacts.name_of(&event.number).map(str::to_string).unwrap_or_default() } else { event.name.clone() };
        let shown = telephony::Event { name, ..event.clone() };
        let (state, number) = (|| match (shown.kind, shown.cancel) {
            (_, true) => ("ended", shown.number.clone()),
            (telephony::Kind::Ringing, false) => ("ringing", shown.number.clone()),
            (telephony::Kind::Talking, false) => ("talking", shown.number.clone()),
            (telephony::Kind::Missed, false) => ("missed", shown.number.clone()),
        })();
        if shown.cancel {
            self.notifier.close(&key);
            dev.st.borrow_mut().call = None;
        } else if shown.kind == telephony::Kind::Missed {
            // The phone's own notification says it, when notifications are mirrored; if not, say it here
            if !dev.sends("kdeconnect.notification") {
                self.raise(Notice {
                    key: format!("missed:{}", dev.id),
                    summary: trf("Missed call from {caller}", &[("caller", &shown.label())]),
                    body: trf("On “{name}”", &[("name", &dev.name())]),
                    icon: Some("call-missed-symbolic".into()),
                    ..Notice::default()
                });
            }
        } else {
            let ringing = shown.kind == telephony::Kind::Ringing;
            let photo = shown.photo.as_deref().and_then(|b64| STANDARD.decode(b64).ok()).and_then(|bytes| {
                let path = self.icons_dir(&dev.id).join("caller.png");
                icons::save_png(&bytes, &path).then_some(path)
            });
            self.raise(Notice {
                key,
                summary: shown.label(),
                body: if ringing { tr("Incoming call") } else { tr("Call in progress") },
                icon: Some(photo.map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "call-start-symbolic".into())),
                actions: if ringing { vec![("mute".into(), tr("Mute"))] } else { Vec::new() },
                urgent: ringing,
                ..Notice::default()
            });
            dev.st.borrow_mut().call = Some(shown.clone());
        }
        dbus::emit(self, Some(dev), "Call", Some((state, number.as_str(), shown.name.as_str()).to_variant()));
    }

    pub fn mute_ringer(self: &Rc<Self>, device: &str) -> ApiResult<()> {
        self.require_send(device, "kdeconnect.telephony.request_mute", json!({}))
    }

    // --- Ringing, music, the clicker (plans/ui-ideas/spec-phone.md) -------------------------------------------

    /// Makes the phone ring, even when it is silent, until Found it is pressed on it (or this is sent again)
    pub fn ring(&self, device: &str) -> ApiResult<()> {
        self.require_send(device, media::RING, json!({}))
    }

    /// The phone's players: its list, or one's state. Heard only while music from either side is on
    fn on_mpris(self: &Rc<Self>, dev: &Rc<Device>, p: &Packet) {
        if !self.music.get() {
            return;
        }
        let heard = dev.st.borrow_mut().players.apply(&p.body);
        for player in &heard.ask {
            self.send_to(dev, media::MPRIS_REQUEST, media::ask_state(player));
        }
        if heard.changed {
            dbus::props_changed(self, dev);
        }
    }

    /// One of the phone's players told to play, pause or skip
    pub fn media_action(&self, device: &str, player: &str, action: &str) -> ApiResult<()> {
        let dev = self.device(device).ok_or_else(|| ApiError::unavailable("no such device"))?;
        if !self.music.get() {
            return Err(ApiError::unavailable("music from either side is off"));
        }
        if !dev.st.borrow().players.knows(player) {
            return Err(ApiError::unavailable("no such player on the phone"));
        }
        let body = media::action_request(player, action).ok_or_else(|| ApiError::InvalidArgs("not an action of a player".into()))?;
        self.require_send(device, media::MPRIS_REQUEST, body)
    }

    /// A key from the phone: a click of the slide clicker, when the person chose it and is presenting now, and one of
    /// the three; anything else is dropped without a word
    fn on_click(self: &Rc<Self>, dev: &Rc<Device>, p: &Packet) {
        if !self.clicker.get() || !dbus::presenting(self) {
            return;
        }
        let Some(click) = media::clicker_of(&p.kind, &p.body) else { return };
        {
            let mut st = dev.st.borrow_mut();
            if st.last_click.is_some_and(|t| t.elapsed() < limits::CLICK_SPACING) {
                return;
            }
            st.last_click = Some(Instant::now());
        }
        dbus::emit(self, None, "Clicker", Some((dev.id.as_str(), click.word()).to_variant()));
    }

    // --- The rest of the bus ------------------------------------------------------------------------------

    pub fn ping(&self, device: &str) -> ApiResult<()> {
        self.require_send(device, "kdeconnect.ping", json!({}))
    }

    pub fn send_url(&self, device: &str, url: &str) -> ApiResult<()> {
        // A scheme, a colon, and no spaces or control characters: what the phone makes of it is its own business
        let scheme_ok = url.split_once(':').is_some_and(|(scheme, rest)| {
            !rest.is_empty() && scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
        });
        if url.is_empty() || url.len() > limits::SHARED_LINK || url.chars().any(|c| c.is_control() || c.is_whitespace()) || !scheme_ok {
            return Err(ApiError::InvalidArgs("not a link".into()));
        }
        self.require_send(device, "kdeconnect.share.request", json!({ "url": url }))
    }

    pub fn send_text(&self, device: &str, text: &str) -> ApiResult<()> {
        if text.trim().is_empty() || text.len() > limits::SHARED_TEXT {
            return Err(ApiError::InvalidArgs("no text, or too much".into()));
        }
        self.require_send(device, "kdeconnect.share.request", json!({ "text": text }))
    }

    /// Calls a phone at an address (a private or link-local one): for a phone on another network of the home
    pub fn connect_to(self: &Rc<Self>, address: &str) -> ApiResult<()> {
        let parsed = SocketAddr::from_str(address).map_err(|_| ApiError::InvalidArgs("an address is HOST:PORT with a numeric host".into()))?;
        let ip: IpAddr = netinfo::unmap(parsed.ip());
        if parsed.port() == 0 || !netinfo::local_enough(ip, self.cfg.allow_loopback) {
            return Err(ApiError::InvalidArgs("only addresses of a home network are called".into()));
        }
        self.connect_out(ip, parsed.port(), 0, None);
        Ok(())
    }

    // --- What was shared with us ---------------------------------------------------------------------------

    fn on_share(self: &Rc<Self>, dev: &Rc<Device>, link: &Rc<Link>, p: Packet) {
        match share::parse(&p) {
            Shared::File { name, modified, batch } => self.receive_file(dev, link, &p, name, modified, batch),
            Shared::Link(url) => {
                self.refuse_payload(link, dev, p.payload().ok().flatten());
                self.show_shared(dev, url, true);
            }
            Shared::Text(t) => {
                self.refuse_payload(link, dev, p.payload().ok().flatten());
                self.show_shared(dev, t, false);
            }
            Shared::Nothing => self.refuse_payload(link, dev, p.payload().ok().flatten()),
        }
    }

    /// A link or text from a phone: said in a notification, and opened only by a button
    fn show_shared(self: &Rc<Self>, dev: &Rc<Device>, content: String, is_link: bool) {
        let content = text::clean(&content);
        let n = self.next_id();
        let rest = n.to_string();
        let openable = is_link && share::openable(&content) && content.len() <= limits::SHARED_LINK;
        // While the shell's module has said it shows them (TakeLinks), a web link it can take goes to the live pill,
        // which shows it with a button to open it and keeps it in the notification list itself when it is put off: no
        // notification of the program's own. Anything else, and any time the module has not said so (the screen
        // locked, the extension off), is the program's notification as before
        if openable && share::for_the_shell(&content) && dbus::links_taken(self) {
            dbus::emit(self, None, "UrlReceived", Some((dev.id.as_str(), content.as_str()).to_variant()));
            return;
        }
        let mut actions = Vec::new();
        if openable {
            actions.push(("open".to_string(), tr("Open")));
        }
        actions.push(("show".to_string(), tr("Show")));
        {
            let mut shared = self.shared.borrow_mut();
            shared.push_back((rest.clone(), content.clone(), is_link));
            while shared.len() > 20 {
                shared.pop_front();
            }
        }
        let name = dev.name();
        self.raise(Notice {
            key: format!("text:{}:{rest}", dev.id),
            summary: if is_link { trf("“{name}” sent a link", &[("name", &name)]) } else { trf("“{name}” sent a text", &[("name", &name)]) },
            body: text::shorten(&text::clean_line(&content), 300),
            icon: Some(if is_link { "emblem-web-symbolic" } else { "edit-paste-symbolic" }.into()),
            actions,
            ..Notice::default()
        });
    }

    pub fn on_text_event(self: &Rc<Self>, dev: &Rc<Device>, rest: &str, what: &str) {
        let found = self.shared.borrow().iter().find(|(r, _, _)| r == rest).cloned();
        let Some((_, content, is_link)) = found else { return };
        match what {
            "open" if is_link && share::openable(&content) => {
                self.open_uri(&content);
            }
            // The window takes the text as one argument: a very long text is shortened to what fits
            "show" => self.launch_app(&["text", "--", &dev.id, "--", &text::shorten(&content, 16 * 1024)]),
            _ => {}
        }
    }
}

/// A map of the values of an `a{sv}`
fn to_dict(map: serde_json::Map<String, serde_json::Value>) -> HashMap<String, glib::Variant> {
    map.into_iter()
        .filter_map(|(k, v)| {
            let value = match v {
                serde_json::Value::String(s) => s.to_variant(),
                serde_json::Value::Bool(b) => b.to_variant(),
                serde_json::Value::Number(n) => n.as_i64()?.to_variant(),
                serde_json::Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<String>>().to_variant(),
                _ => return None,
            };
            Some((k, value))
        })
        .collect()
}
