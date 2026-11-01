//! This computer's media players, shown on the phones and played from them (plans/ui-ideas/spec-phone.md 3.1). The
//! players are the session's MPRIS ones, read and told what to do over the session bus. Watched only while music from
//! either side is on and a paired phone that takes their state is connected: otherwise nothing is subscribed and
//! nothing is kept, so that the switch off, or no phone, costs nothing.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::{Rc, Weak};

use glib::variant::{ObjectPath, ToVariant};
use gio::prelude::*;
use serde_json::{Map, Value};

use crate::device::Device;
use crate::media::{self, Local};
use crate::service::Service;

const PREFIX: &str = "org.mpris.MediaPlayer2";
const PATH: &str = "/org/mpris/MediaPlayer2";
const ROOT: &str = "org.mpris.MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
/// Milliseconds a player is given to answer
const WAIT: i32 = 2000;

struct Entry {
    /// The unique name behind the well-known one, which its signals come from
    owner: String,
    local: Local,
    /// What the phones call it
    name: String,
}

pub struct Watcher {
    bus: gio::DBusConnection,
    subscriptions: Vec<gio::SignalSubscriptionId>,
    /// By well-known bus name, so that the names the phones see come in the same order every time
    players: RefCell<BTreeMap<String, Entry>>,
    cancellable: gio::Cancellable,
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.cancellable.cancel();
        #[allow(deprecated)]
        for id in self.subscriptions.drain(..) {
            self.bus.signal_unsubscribe(id);
        }
    }
}

fn props_of(value: &glib::Variant) -> HashMap<String, glib::Variant> {
    value.get::<HashMap<String, glib::Variant>>().unwrap_or_default()
}

impl Service {
    /// The phones that are shown this computer's players now
    fn music_phones(&self) -> Vec<Rc<Device>> {
        if !self.music.get() {
            return Vec::new();
        }
        self.devices.borrow().values().filter(|d| d.paired() && d.connected() && d.takes(media::MPRIS)).cloned().collect()
    }

    /// Watches the players while some phone is to see them, and stops when none is
    pub fn sync_players(self: &Rc<Self>) {
        let wanted = !self.stopping.get() && !self.music_phones().is_empty();
        let bus = self.bus.borrow().clone();
        let watching = self.players.borrow().is_some();
        match (wanted, bus, watching) {
            (true, Some(bus), false) => self.watch_players(bus),
            (false, _, true) => {
                self.players.borrow_mut().take();
            }
            _ => {}
        }
    }

    /// Whether the players are watched: for the tests
    pub fn watching_players(&self) -> bool {
        self.players.borrow().is_some()
    }

    fn watch_players(self: &Rc<Self>, bus: gio::DBusConnection) {
        let weak = Rc::downgrade(self);
        let mut subscriptions = Vec::new();
        #[allow(deprecated)] // the stream form would need the program to poll it for callbacks this small
        {
            let w = weak.clone();
            subscriptions.push(bus.signal_subscribe(
                Some("org.freedesktop.DBus"),
                Some("org.freedesktop.DBus"),
                Some("NameOwnerChanged"),
                Some("/org/freedesktop/DBus"),
                Some(PREFIX),
                gio::DBusSignalFlags::MATCH_ARG0_NAMESPACE,
                move |_, _, _, _, _, params| {
                    let (Some(name), Some(owner)) = (params.child_value(0).get::<String>(), params.child_value(2).get::<String>()) else { return };
                    if let Some(service) = w.upgrade() {
                        if owner.is_empty() { service.player_gone(&name) } else { service.player_came(name, owner) }
                    }
                },
            ));
            let w = weak.clone();
            subscriptions.push(bus.signal_subscribe(None, Some(PROPERTIES), Some("PropertiesChanged"), Some(PATH), Some(PLAYER), gio::DBusSignalFlags::NONE, move |_, sender, _, _, _, params| {
                if let Some(service) = w.upgrade() {
                    let invalidated = params.child_value(2).get::<Vec<String>>().unwrap_or_default();
                    service.player_changed(sender, props_of(&params.child_value(1)), !invalidated.is_empty());
                }
            }));
            let w = weak.clone();
            subscriptions.push(bus.signal_subscribe(None, Some(PLAYER), Some("Seeked"), Some(PATH), None, gio::DBusSignalFlags::NONE, move |_, sender, _, _, _, params| {
                if let (Some(service), Some(position)) = (w.upgrade(), params.child_value(0).get::<i64>()) {
                    let mut props = HashMap::new();
                    props.insert("Position".to_string(), position.to_variant());
                    service.player_changed(sender, props, false);
                }
            }));
        }
        let watcher = Watcher { bus: bus.clone(), subscriptions, players: RefCell::default(), cancellable: gio::Cancellable::new() };
        let cancellable = watcher.cancellable.clone();
        *self.players.borrow_mut() = Some(watcher);
        self.spawn_task(async move {
            let reply = bus.call_future(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", "org.freedesktop.DBus", "ListNames", None, None, gio::DBusCallFlags::NONE, WAIT).await;
            let names: Vec<String> = reply.ok().and_then(|r| r.child_value(0).get::<Vec<String>>()).unwrap_or_default();
            for name in names.into_iter().filter(|n| n.starts_with(&format!("{PREFIX}."))) {
                if cancellable.is_cancelled() {
                    return;
                }
                let owner = bus
                    .call_future(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetNameOwner", Some(&(name.as_str(),).to_variant()), None, gio::DBusCallFlags::NONE, WAIT)
                    .await
                    .ok()
                    .and_then(|r| r.child_value(0).get::<String>());
                if let (Some(owner), Some(service)) = (owner, weak.upgrade()) {
                    service.player_came(name, owner);
                }
            }
        });
    }

    fn watched_bus(&self) -> Option<(gio::DBusConnection, gio::Cancellable)> {
        self.players.borrow().as_ref().map(|w| (w.bus.clone(), w.cancellable.clone()))
    }

    /// A player arrived (or was found): its properties are read, then it is on the phones' list
    fn player_came(self: &Rc<Self>, name: String, owner: String) {
        let Some((bus, cancellable)) = self.watched_bus() else { return };
        let weak: Weak<Service> = Rc::downgrade(self);
        self.spawn_task(async move {
            let mut local = Local::default();
            for interface in [ROOT, PLAYER] {
                let reply = bus.call_future(Some(&owner), PATH, PROPERTIES, "GetAll", Some(&(interface,).to_variant()), None, gio::DBusCallFlags::NONE, WAIT).await;
                if let Ok(reply) = reply {
                    local.apply(&props_of(&reply.child_value(0)));
                }
            }
            let Some(service) = weak.upgrade() else { return };
            if cancellable.is_cancelled() {
                return;
            }
            if let Some(watcher) = service.players.borrow().as_ref() {
                watcher.players.borrow_mut().insert(name, Entry { owner, local, name: String::new() });
            }
            service.players_listed();
        });
    }

    fn player_gone(self: &Rc<Self>, name: &str) {
        let gone = self.players.borrow().as_ref().is_some_and(|w| w.players.borrow_mut().remove(name).is_some());
        if gone {
            self.players_listed();
        }
    }

    fn player_changed(self: &Rc<Self>, owner: &str, props: HashMap<String, glib::Variant>, reread: bool) {
        let found = {
            let players = self.players.borrow();
            let Some(watcher) = players.as_ref() else { return };
            let mut list = watcher.players.borrow_mut();
            list.iter_mut().find(|(_, e)| e.owner == owner).map(|(bus_name, entry)| {
                entry.local.apply(&props);
                (bus_name.clone(), entry.owner.clone())
            })
        };
        let Some((bus_name, owner)) = found else { return };
        if reread {
            // A property said only to have changed is read again, with the rest
            self.player_came(bus_name, owner);
        } else {
            self.send_player(&bus_name, None);
        }
    }

    /// The names are given again, and the list is sent to every phone that sees it
    fn players_listed(self: &Rc<Self>) {
        let names = self.player_names();
        for dev in self.music_phones() {
            self.send_to(&dev, media::MPRIS, media::player_list(&names));
        }
    }

    fn player_names(&self) -> Vec<String> {
        let players = self.players.borrow();
        let Some(watcher) = players.as_ref() else { return Vec::new() };
        let mut list = watcher.players.borrow_mut();
        let pairs: Vec<(String, String)> = list.iter().map(|(bus, e)| (bus.clone(), e.local.identity.clone())).collect();
        let named = media::names(&pairs);
        for (bus, name) in &named {
            if let Some(entry) = list.get_mut(bus) {
                entry.name = name.clone();
            }
        }
        named.into_iter().map(|(_, name)| name).collect()
    }

    /// A player's state, to one phone or to all that see the players; its position is asked for first, since it
    /// changes by itself and is never signalled
    fn send_player(self: &Rc<Self>, bus_name: &str, only: Option<Rc<Device>>) {
        let Some((bus, cancellable)) = self.watched_bus() else { return };
        let owner = self.players.borrow().as_ref().and_then(|w| w.players.borrow().get(bus_name).map(|e| e.owner.clone()));
        let Some(owner) = owner else { return };
        let (weak, bus_name) = (Rc::downgrade(self), bus_name.to_string());
        self.spawn_task(async move {
            let position = bus
                .call_future(Some(&owner), PATH, PROPERTIES, "Get", Some(&(PLAYER, "Position").to_variant()), None, gio::DBusCallFlags::NONE, WAIT)
                .await
                .ok()
                .and_then(|r| r.child_value(0).as_variant())
                .and_then(|v| v.get::<i64>());
            let Some(service) = weak.upgrade() else { return };
            if cancellable.is_cancelled() {
                return;
            }
            let status = {
                let players = service.players.borrow();
                let Some(watcher) = players.as_ref() else { return };
                let mut list = watcher.players.borrow_mut();
                let Some(entry) = list.get_mut(&bus_name) else { return };
                if let Some(position) = position {
                    entry.local.position = position;
                }
                if entry.name.is_empty() {
                    return;
                }
                entry.local.status(&entry.name)
            };
            let targets = match only {
                Some(dev) => vec![dev],
                None => service.music_phones(),
            };
            for dev in targets {
                service.send_to(&dev, media::MPRIS, status.clone());
            }
        });
    }

    /// The phone's `kdeconnect.mpris.request` about this computer's players
    pub fn on_mpris_request(self: &Rc<Self>, dev: &Rc<Device>, body: &Map<String, Value>) {
        if !self.music.get() {
            return;
        }
        // A phone that asks is one more to show them to
        self.sync_players();
        let asked = media::asked(body);
        if asked.list {
            let names = self.player_names();
            self.send_to(dev, media::MPRIS, media::player_list(&names));
        }
        let Some(phone_name) = asked.player.as_deref() else { return };
        let found = self.players.borrow().as_ref().and_then(|w| {
            w.players.borrow().iter().find(|(_, e)| e.name == phone_name).map(|(bus, e)| (bus.clone(), e.owner.clone(), e.local.track.clone()))
        });
        let Some((bus_name, owner, track)) = found else { return };
        let Some((bus, _)) = self.watched_bus() else { return };
        let call = |method: &str, args: Option<glib::Variant>| {
            let (bus, owner, method) = (bus.clone(), owner.clone(), method.to_string());
            async move {
                let _ = bus.call_future(Some(&owner), PATH, PLAYER, &method, args.as_ref(), None, gio::DBusCallFlags::NONE, WAIT).await;
            }
        };
        let set = |property: &str, value: glib::Variant| {
            let (bus, owner, property) = (bus.clone(), owner.clone(), property.to_string());
            async move {
                let args = (PLAYER, property.as_str(), value).to_variant();
                let _ = bus.call_future(Some(&owner), PATH, PROPERTIES, "Set", Some(&args), None, gio::DBusCallFlags::NONE, WAIT).await;
            }
        };
        if let Some(action) = asked.action {
            self.spawn_task(call(action, None));
        }
        if let Some(seek) = asked.seek {
            self.spawn_task(call("Seek", Some((seek,).to_variant())));
        }
        if let (Some(position), Some(track)) = (asked.position, track.and_then(|t| ObjectPath::try_from(t).ok())) {
            self.spawn_task(call("SetPosition", Some((track, position.saturating_mul(1000)).to_variant())));
        }
        if let Some(volume) = asked.volume {
            self.spawn_task(set("Volume", volume.to_variant()));
        }
        if let Some(status) = asked.loop_status {
            self.spawn_task(set("LoopStatus", status.to_variant()));
        }
        if let Some(shuffle) = asked.shuffle {
            self.spawn_task(set("Shuffle", shuffle.to_variant()));
        }
        if asked.state {
            self.send_player(&bus_name, Some(dev.clone()));
        }
    }
}
