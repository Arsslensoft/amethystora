//! The Phones page: every phone the program knows that is paired, asking to be, or there to pair with, each with
//! what can be done with it now. A request to pair shows the verification key to compare with the phone's own
//! screen, and is answered only by a click: nothing here accepts a pairing by itself, and the buttons that
//! answer one never take the focus when the page opens.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use amethystora_ui::{gio, glib, gtk, i18n, process, t, ui};
use gtk::prelude::*;

use crate::icons::{self, icon};
use crate::model::{self, bdi, Device, Situation};
use crate::widgets::{self, Look};
use crate::{failure_text, Page, Phone};

/// The widgets of the page made once
pub struct Parts {
    pub scroller: gtk::ScrolledWindow,
    body: gtk::Box,
    /// The cards drawn, by phone, so that a command can bring one into view
    cards: RefCell<Vec<(String, gtk::Box)>>,
}

impl Parts {
    pub fn new() -> Parts {
        let body = ui::vbox(0, &[]);
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .overlay_scrolling(false)
            .css_classes(["page-scroll"])
            .vexpand(true)
            .child(&widgets::page_column(&body))
            .build();
        scroller.set_visible(false);
        Parts { scroller, body, cards: RefCell::new(Vec::new()) }
    }
}

/// What a click on a phone's button does
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Do {
    Pair,
    Accept,
    Reject,
    Unpair,
    Ping,
    Ring,
}

/// "Expires in 20 seconds"
fn expires(seconds: i64) -> String {
    t!("{count, plural, one {Expires in # second} other {Expires in # seconds}}", count = seconds)
}

impl Phone {
    pub(crate) fn render_phones(&self) {
        let devices = model::listed(&self.link.devices());
        let body = &self.parts.phones.body;
        ui::clear(body);
        self.countdowns.borrow_mut().clear();
        self.parts.phones.cards.borrow_mut().clear();

        let view = ui::vbox(0, &["page"]);
        let head = ui::vbox(6, &["page-head"]);
        // With the switch for other computers on, the page lists them too, and says how they pair
        let computers = self.link.computers();
        head.append(&widgets::heading(&if computers { t!("Phones and computers") } else { t!("Phones") }, "h1"));
        head.append(&ui::label(&t!("Pair a phone that has the KDE Connect app to see its notifications, read its texts and send files and links back and forth."), &["lead"]));
        if computers {
            head.append(&ui::label(&t!("Another computer pairs the same way once Pair with other computers is on on both."), &["lead"]));
        }
        view.append(&head);

        if devices.is_empty() {
            view.append(&self.no_phone());
        } else {
            let attention = self.state.borrow().attention.clone();
            let asking = devices.iter().any(|d| group_of(d.situation()) == 0);
            let mut current: Option<u8> = None;
            let mut section = ui::vbox(14, &["cards"]);
            for device in &devices {
                let group = group_of(device.situation());
                if current != Some(group) {
                    if current.is_some() {
                        view.append(&section);
                        section = ui::vbox(14, &["cards"]);
                    }
                    // The paired phones are titled when requests come before them, the others always
                    let title = match group {
                        1 if asking && computers => Some(t!("Your phones and computers")),
                        1 if asking => Some(t!("Your phones")),
                        2 => Some(t!("Nearby, not paired")),
                        _ => None,
                    };
                    if let Some(title) = title {
                        section.append(&ui::label(&title.to_uppercase(), &["nav-label", "section"]));
                    }
                    current = Some(group);
                }
                let card = self.device_card(device, attention.as_deref() == Some(device.id.as_str()));
                self.parts.phones.cards.borrow_mut().push((device.id.clone(), card.clone()));
                section.append(&card);
            }
            view.append(&section);
        }
        view.append(&self.page_foot());
        body.append(&view);
        self.ensure_countdown();
    }

    /// Nothing to list: the link is on, and no phone has been found
    fn no_phone(&self) -> gtk::Widget {
        let gem = amethystora_ui::gem::Gem::plain(46.0);
        gem.add_css_class("gem");
        let again = self.button(&t!("Look again"), Look { icon: Some("refresh"), primary: true, ..Default::default() }, |this| this.look_again());
        // What to do, in the order to do it, at the start of the line
        let steps = ui::vbox(10, &["steps"]);
        for (at, step) in [
            t!("Install the KDE Connect app on the phone."),
            t!("Join the same network as this computer, which has to be one you named home."),
            t!("Open KDE Connect on the phone: this computer is in its list. Ask to pair from there, or from here."),
        ]
        .into_iter()
        .enumerate()
        {
            let line = ui::hbox(12, &[]);
            let number = ui::line(&i18n::number((at + 1) as f64), &["step-number"]);
            number.set_xalign(0.5);
            number.set_valign(gtk::Align::Start);
            line.append(&number);
            line.append(&ui::label(&step, &["grow"]));
            steps.append(&line);
        }
        let home = ui::vbox(6, &[]);
        home.append(&ui::label(&t!("A network is a home network once you have named it, in a terminal:"), &["faint", "small"]));
        home.append(&widgets::command("ame security home-networks"));
        let view = ui::vbox(22, &[]);
        view.append(&widgets::empty(vec![
            gem.upcast(),
            widgets::centered(widgets::heading(&t!("No phone found yet"), "h2")).upcast(),
            widgets::centered(ui::label(&t!("This computer is announcing itself on your home networks."), &[])).upcast(),
        ]));
        view.append(&ui::limit_center(&steps, 480));
        let actions = widgets::actions(vec![again.upcast()], &["actions"]);
        actions.set_halign(gtk::Align::Center);
        view.append(&actions);
        view.append(&ui::limit_center(&home, 480));
        view.upcast()
    }


    /// Under the cards: how phones see this computer, and where their files go
    fn page_foot(&self) -> gtk::Widget {
        let foot = ui::vbox(8, &["page-foot"]);
        let name = self.link.name();
        if !name.is_empty() {
            let seen = if self.link.computers() {
                t!("Phones and other computers see this computer as {name}.", name = bdi(&name))
            } else {
                t!("Phones see this computer as {name}.", name = bdi(&name))
            };
            foot.append(&ui::label(&seen, &["faint", "small"]));
        }
        let folder = self.link.receive_folder();
        if !folder.is_empty() {
            let row = ui::hbox(10, &[]);
            row.append(&ui::label(&t!("Files from a phone are put in {folder}.", folder = bdi(&folder)), &["faint", "small", "grow"]));
            // Only a folder in the account's own home is offered to be opened, whatever the program says
            if inside_home(&folder) {
                let window = self.shell.gtk_window();
                let path = folder.clone();
                row.append(&widgets::link_button(&t!("Open the folder"), Some("external"), move || {
                    process::open_uri(Some(&window), &gio::File::for_path(&path).uri());
                }));
            }
            foot.append(&row);
        }
        foot.upcast()
    }

    // --- One phone -------------------------------------------------------------------------------------

    fn device_card(&self, device: &Device, attention: bool) -> gtk::Box {
        let situation = device.situation();
        let card = ui::vbox(14, &["card", "phone-card"]);
        card.set_focusable(true);
        if attention {
            card.add_css_class("attention");
        }
        if situation == Situation::Away {
            card.add_css_class("away");
        }
        let top = ui::hbox(14, &[]);
        let mark = ui::hbox(0, &["device-icon"]);
        mark.set_valign(gtk::Align::Start);
        let glyph = icons::device(&device.kind, 24);
        glyph.set_hexpand(true);
        mark.set_hexpand(false);
        mark.set_halign(gtk::Align::Start);
        mark.append(&glyph);
        top.append(&mark);
        let words = ui::vbox(2, &["grow"]);
        words.set_hexpand(true);
        let name = widgets::heading(&device.title(), "h3");
        words.append(&name);
        let subline = ui::hbox(8, &[]);
        // The kind and the state are two words of their own, joined by a mark and not by a sentence
        subline.append(&ui::line(&model::kind_word(&device.kind), &["faint"]));
        subline.append(&ui::line("\u{b7}", &["faint"]));
        subline.append(&ui::line(&state_word(device), &["faint"]));
        if situation == Situation::Ready && !device.address.is_empty() {
            let address = ui::line(&device.address, &["code", "faint", "small"]);
            address.set_direction(gtk::TextDirection::Ltr);
            subline.append(&address);
        }
        words.append(&subline);
        top.append(&words);
        if let Some(battery) = widgets::battery(device) {
            if situation == Situation::Ready {
                top.append(&battery);
            }
        }
        card.append(&top);

        if matches!(situation, Situation::Asked | Situation::Asking) {
            card.append(&self.key_box(device));
        }
        let moving: Vec<_> = self.link.transfers(&device.id).into_iter().filter(|t| t.running()).collect();
        if !moving.is_empty() {
            card.append(&self.transfers_strip(&moving));
        }
        card.append(&self.device_actions(device));
        card
    }

    /// The key to compare with the phone's screen, and what to do with it
    fn key_box(&self, device: &Device) -> gtk::Box {
        let asked = device.situation() == Situation::Asked;
        let boxed = ui::vbox(8, &["keybox"]);
        boxed.update_property(&[gtk::accessible::Property::Label(&t!("Verification key"))]);
        let head = ui::hbox(8, &["faint", "small"]);
        head.append(&icon("key", 15));
        head.append(&ui::line(&t!("Verification key"), &[]));
        boxed.append(&head);
        let key = ui::line(&device.pairing_key, &["key", "code"]);
        key.set_direction(gtk::TextDirection::Ltr);
        key.set_selectable(true);
        boxed.append(&key);
        boxed.append(&ui::label(
            &match (asked, device.is_computer()) {
                (true, false) => t!("{phone} asks to pair. Pair only if the phone in your hand shows this same key.", phone = bdi(&device.title())),
                (false, false) => t!("Waiting for {phone}. Its screen should show this same key, and ask you to accept there.", phone = bdi(&device.title())),
                (true, true) => t!("{phone} asks to pair. Pair only if the other computer's screen shows this same key.", phone = bdi(&device.title())),
                (false, true) => t!("Waiting for {phone}. The other computer's screen should show this same key, and ask you to accept there.", phone = bdi(&device.title())),
            },
            &["soft"],
        ));
        let left = ui::line(&expires(device.seconds_left(i18n::now())), &["faint", "small"]);
        boxed.append(&left);
        self.countdowns.borrow_mut().push((device.id.clone(), left));
        boxed
    }

    fn transfers_strip(&self, moving: &[model::Transfer]) -> gtk::Box {
        let strip = ui::vbox(8, &["transfers"]);
        for transfer in moving.iter().take(3) {
            let row = ui::vbox(4, &[]);
            let line = ui::hbox(8, &[]);
            line.append(&icon(if transfer.incoming { "download" } else { "upload" }, 15));
            let name = ui::line(&transfer.name, &["grow"]);
            name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            name.set_hexpand(true);
            line.append(&name);
            line.append(&ui::line(&transfer.status(), &["faint", "small"]));
            row.append(&line);
            let bar = ui::Bar::new();
            bar.set_fraction(transfer.fraction());
            row.append(&bar);
            strip.append(&row);
        }
        if moving.len() > 3 {
            strip.append(&ui::line(&t!("and {count, plural, one {# more file} other {# more files}}", count = moving.len() - 3), &["faint", "small"]));
        }
        strip
    }

    fn device_actions(&self, device: &Device) -> gtk::Widget {
        let id = device.id.clone();
        let busy = self.state.borrow().busy.contains(&id);
        let mut buttons: Vec<gtk::Widget> = Vec::new();
        let act = |label: String, look: Look, what: Do| {
            let id = id.clone();
            self.button(&label, Look { disabled: look.disabled || busy, ..look }, move |this| this.act(&id, what)).upcast::<gtk::Widget>()
        };
        match device.situation() {
            Situation::Asked => {
                // Neither takes the focus: Enter on a freshly opened page must not answer a pairing
                buttons.push(act(t!("Accept"), Look { icon: Some("check"), primary: true, ..Default::default() }, Do::Accept));
                buttons.push(act(t!("Reject"), Look { icon: Some("close"), ..Default::default() }, Do::Reject));
            }
            Situation::Asking => buttons.push(act(t!("Cancel"), Look { icon: Some("close"), ..Default::default() }, Do::Unpair)),
            Situation::Ready => {
                if device.has("share") {
                    let id = id.clone();
                    buttons.push(self.button(&t!("Send…"), Look { icon: Some("upload"), primary: true, ..Default::default() }, move |this| this.open_send(Some(id.clone()), crate::cli::Sending::Nothing, false)).upcast());
                }
                if device.has("sms") {
                    let id = id.clone();
                    buttons.push(
                        self.button(&t!("Messages"), Look { icon: Some("message"), ..Default::default() }, move |this| {
                            this.texts.remember(&id);
                            this.show(Page::Messages);
                        })
                        .upcast(),
                    );
                }
                if device.has("ping") {
                    buttons.push(act(t!("Ping"), Look { icon: Some("bell"), ..Default::default() }, Do::Ping));
                }
                // Rings even when the phone is silent, until Found it is pressed on it
                if device.has("ring") {
                    buttons.push(act(t!("Ring"), Look { icon: Some("phone"), ..Default::default() }, Do::Ring));
                }
                buttons.push(act(t!("Unpair…"), Look { icon: Some("unlink"), ..Default::default() }, Do::Unpair));
            }
            Situation::Away => {
                buttons.push(act(t!("Unpair…"), Look { icon: Some("unlink"), ..Default::default() }, Do::Unpair));
            }
            Situation::Nearby => {
                buttons.push(act(t!("Pair"), Look { icon: Some("link"), primary: true, disabled: !device.connected, ..Default::default() }, Do::Pair));
            }
        }
        let actions = widgets::actions(buttons, &["actions"]);
        if device.situation() == Situation::Away {
            let row = ui::vbox(10, &[]);
            row.append(&ui::label(&t!("Out of reach now: it is off, asleep, or on another network. It connects again by itself."), &["faint", "small"]));
            row.append(&actions);
            return row.upcast();
        }
        actions.upcast()
    }

    // --- What the buttons do ---------------------------------------------------------------------------

    fn act(self: &Rc<Self>, id: &str, what: Do) {
        let Some(device) = self.link.device(id) else { return };
        // Unpairing a paired phone is asked about first
        if what == Do::Unpair && device.paired {
            self.open_unpair(id);
            return;
        }
        self.run_act(id, what);
    }

    /// The call a button asked for, with the page marked busy meanwhile and the answer said in a line
    fn run_act(self: &Rc<Self>, id: &str, what: Do) {
        let Some(device) = self.link.device(id) else { return };
        if !self.state.borrow_mut().busy.insert(id.to_string()) {
            return;
        }
        self.schedule_render();
        let (this, id, name) = (self.clone(), id.to_string(), bdi(&device.title()));
        glib::spawn_future_local(async move {
            let result = match what {
                Do::Pair => this.link.pair(&id).await,
                Do::Accept => this.link.accept_pairing(&id).await,
                Do::Reject => this.link.reject_pairing(&id).await,
                Do::Unpair => this.link.unpair(&id).await,
                Do::Ping => this.link.ping(&id).await,
                Do::Ring => this.link.ring(&id).await,
            };
            this.state.borrow_mut().busy.remove(&id);
            this.render();
            match (result, what) {
                (Err(failure), _) => this.say(true, failure_text(&failure)),
                (Ok(()), Do::Accept) => this.say(false, t!("Paired with {phone}.", phone = name)),
                (Ok(()), Do::Unpair) => this.say(false, t!("{phone} is unpaired.", phone = name)),
                (Ok(()), Do::Ping) => this.say(false, t!("Pinged {phone}.", phone = name)),
                (Ok(()), Do::Ring) => this.say(false, t!("Ringing {phone}. It rings until Found it is pressed on it.", phone = name)),
                (Ok(()), _) => {}
            }
        });
    }

    pub(crate) fn unpair_confirmed(self: &Rc<Self>, id: &str) {
        self.run_act(id, Do::Unpair);
    }

    /// `amethystora-phone pair DEVICE`: the page, the phone marked and in view, and a request sent if the phone is
    /// there and nothing was asked yet. A request the phone made is never answered by this: it is shown.
    pub(crate) fn pair_command(self: &Rc<Self>, id: &str) {
        self.state.borrow_mut().attention = Some(id.to_string());
        self.render();
        let card = self.parts.phones.cards.borrow().iter().find(|(card_id, _)| card_id == id).map(|(_, card)| card.clone());
        match self.link.device(id) {
            Some(device) => {
                if let Some(card) = card {
                    card.grab_focus();
                }
                if device.situation() == Situation::Nearby && device.connected {
                    self.run_act(id, Do::Pair);
                }
            }
            None => self.say(true, t!("That phone is not there now. Open KDE Connect on it and try again.")),
        }
        let weak = self.weak();
        glib::timeout_add_seconds_local_once(4, move || {
            if let Some(this) = weak.upgrade() {
                this.state.borrow_mut().attention = None;
                this.schedule_render();
            }
        });
    }

    // --- The seconds left ------------------------------------------------------------------------------

    /// One timer, while some request waits, that counts its seconds down
    fn ensure_countdown(&self) {
        if self.countdowns.borrow().is_empty() || self.countdown_timer.borrow().is_some() {
            return;
        }
        let weak = self.weak();
        let id = glib::timeout_add_local(Duration::from_secs(1), move || {
            let Some(this) = weak.upgrade() else { return glib::ControlFlow::Break };
            if this.countdowns.borrow().is_empty() {
                // The source ends with this return: forgetting it is all that is left to do
                this.countdown_timer.take();
                return glib::ControlFlow::Break;
            }
            let now = i18n::now();
            for (id, label) in this.countdowns.borrow().iter() {
                if let Some(device) = this.link.device(id) {
                    label.set_label(&expires(device.seconds_left(now)));
                }
            }
            glib::ControlFlow::Continue
        });
        self.countdown_timer.replace(Some(id));
    }
}

/// Which part of the page a phone is listed in: requests, paired phones, those nearby
fn group_of(situation: Situation) -> u8 {
    match situation {
        Situation::Asked | Situation::Asking => 0,
        Situation::Ready | Situation::Away => 1,
        Situation::Nearby => 2,
    }
}

/// Where a phone is: "Connected", "Out of reach"
fn state_word(device: &Device) -> String {
    match device.situation() {
        Situation::Asked => t!("Asks to pair"),
        Situation::Asking if device.is_computer() => t!("Waiting for the other computer"),
        Situation::Asking => t!("Waiting for the phone"),
        Situation::Ready => t!("Connected"),
        Situation::Away => t!("Out of reach"),
        Situation::Nearby => t!("Not paired"),
    }
}

/// Whether a folder is, once links and `..` are followed, inside the account's home folder
fn inside_home(folder: &str) -> bool {
    let (Ok(real), Ok(home)) = (std::fs::canonicalize(folder), std::fs::canonicalize(amethystora_ui::palette::home())) else { return false };
    real.is_dir() && real.starts_with(home)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_folder_inside_the_home_is_offered() {
        let home = std::env::temp_dir().join(format!("phone-home-{}", std::process::id()));
        std::fs::create_dir_all(home.join("Downloads/Phone")).expect("a folder");
        // SAFETY: the test sets HOME once, before anything else here reads it, and no other test reads it
        unsafe { std::env::set_var("HOME", &home) };
        assert!(inside_home(home.join("Downloads/Phone").to_str().expect("text")));
        assert!(!inside_home(home.join("Downloads/Phone/../../..").to_str().expect("text")));
        assert!(!inside_home("/etc"));
        assert!(!inside_home(home.join("nothing").to_str().expect("text")));
        let _ = std::fs::remove_dir_all(&home);
    }
}
