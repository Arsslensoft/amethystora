//! The closed screen: the gem first, then the ways this vault opens. A password can always be typed, whatever the vault says it
//! has, since that is how a hidden vault opens, and nothing here may tell whether there is one.

use std::rc::Rc;

use amethystora_ui::gem::{Gem, Motion};
use amethystora_ui::{gio, glib, gtk, t, ui};
use gtk::prelude::*;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use super::Vault;
use crate::fido;
use crate::rpc::{self, Trouble};
use crate::secret;
use crate::widgets::{self, btn, say};

/// What is on the closed screen
#[derive(Clone, Copy, PartialEq)]
enum Form {
    Choose,
    Password,
    Recovery,
}

impl Vault {
    pub fn show_closed(&self, problem: Option<&str>) {
        // The form is made anew, and the old one's fields go with it
        if let Some(old) = self.root.child_by_name("closed") {
            self.root.remove(&old);
        }
        self.closed_key.replace(self.methods().join(","));
        let page = self.closed_page(Form::Choose, problem);
        self.root.add_named(&page, Some("closed"));
        self.root.set_visible_child_name("closed");
    }

    pub(super) fn methods(&self) -> Vec<String> {
        let status = self.status.borrow();
        let first = status["vaults"].as_array().into_iter().flatten().find(|vault| vault["first"] == json!(true));
        first.map_or_else(Vec::new, |vault| vault["methods"].as_array().into_iter().flatten().filter_map(|method| method.as_str().map(str::to_string)).collect())
    }

    fn closed_page(&self, form: Form, problem: Option<&str>) -> gtk::Widget {
        let column = ui::vbox(18, &["closed"]);
        column.set_halign(gtk::Align::Center);
        column.set_valign(gtk::Align::Center);
        let gem = Gem::halo(112.0, 92.0, 28.0);
        self.gate_gem.replace(Some(gem.clone()));
        column.append(&gem);
        let methods = self.methods();
        let has = |name: &str| methods.iter().any(|method| method == name);
        let title = widgets::h1(&t!("Your vault is closed"));
        title.set_xalign(0.5);
        column.append(&title);
        let sentence = match (form, has("approval"), has("no-prompt")) {
            (Form::Password, ..) => t!("Type the password of the vault you want to open."),
            (Form::Recovery, ..) => t!("Type the recovery key of the vault you want to open."),
            (_, true, _) => t!("Touch the reader, or use your password."),
            (_, _, true) => t!("It opens without asking on this computer."),
            _ => t!("Use your password to open it."),
        };
        let line = widgets::p(&sentence, &["soft"]);
        line.set_xalign(0.5);
        line.set_justify(gtk::Justification::Center);
        column.append(&line);

        let error = widgets::problem();
        error.set_xalign(0.5);
        if let Some(problem) = problem {
            say(&error, problem);
        }
        let choices = ui::vbox(10, &["closed-choices"]);
        match form {
            Form::Choose => self.choices(&choices, &methods, &error),
            Form::Password => self.typed_form(&choices, "password", &error),
            Form::Recovery => self.typed_form(&choices, "recovery", &error),
        }
        column.append(&choices);
        column.append(&error);
        let (words, next) = match form {
            Form::Recovery | Form::Password => (t!("Back"), Form::Choose),
            Form::Choose => (t!("I lost these. Use the recovery key"), Form::Recovery),
        };
        let weak = self.me.clone();
        let other = widgets::link(&words, move || {
            if let Some(this) = weak.upgrade() {
                this.switch_form(next);
            }
        });
        other.set_halign(gtk::Align::Center);
        column.append(&other);
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).hexpand(true).vexpand(true).child(&column).build();
        scroller.upcast()
    }

    fn switch_form(&self, form: Form) {
        if let Some(old) = self.root.child_by_name("closed") {
            self.root.remove(&old);
        }
        let page = self.closed_page(form, None);
        self.root.add_named(&page, Some("closed"));
        self.root.set_visible_child_name("closed");
    }

    /// Every way this vault opens without typing, first, then the password, which is always there
    fn choices(&self, choices: &gtk::Box, methods: &[String], error: &gtk::Label) {
        let has = |name: &str| methods.iter().any(|method| method == name);
        let without_typing = has("approval") || has("no-prompt");
        if has("approval") {
            let method = "approval";
            choices.append(&self.open_button(&t!("Open with fingerprint"), "finger", method, error));
        } else if has("no-prompt") {
            choices.append(&self.open_button(&t!("Open"), "bolt", "no-prompt", error));
        }
        let weak = self.me.clone();
        let (words, classes): (String, &[&str]) = if without_typing { (t!("Use your password instead"), &[]) } else { (t!("Use your password"), &["primary"]) };
        let password = btn(&words, Some("pass"), classes, move || {
            if let Some(this) = weak.upgrade() {
                this.switch_form(Form::Password);
            }
        });
        choices.append(&password);
        if has("security-key") {
            let (weak, failing) = (self.me.clone(), error.clone());
            choices.append(&btn(&t!("Use a security key"), Some("usb"), &[], move || {
                if let Some(this) = weak.upgrade() {
                    this.open_with_key(failing.clone(), None);
                }
            }));
        }
    }

    fn open_button(&self, words: &str, glyph: &str, method: &'static str, error: &gtk::Label) -> gtk::Button {
        let (weak, failing) = (self.me.clone(), error.clone());
        let button = btn(words, Some(glyph), &["primary"], || {});
        button.connect_clicked(move |button| {
            let Some(this) = weak.upgrade() else { return };
            let (button, failing) = (button.clone(), failing.clone());
            glib::spawn_future_local(async move {
                this.opening(true);
                button.set_sensitive(false);
                say(&failing, "");
                let answer = rpc::ask(json!({"op": "unlock", "method": method})).await;
                this.opening(false);
                button.set_sensitive(true);
                match answer {
                    Ok(_) => this.opened(),
                    // The person said no: nothing to say about it
                    Err(Trouble::Refused { wait: 0, problem: None }) => {}
                    Err(trouble) => say(&failing, &trouble.words()),
                }
            });
        });
        button
    }

    /// The stem lifts while a vault is opening, as in Notes, and stays still otherwise
    fn opening(&self, opening: bool) {
        if let Some(gem) = &*self.gate_gem.borrow() {
            gem.set_motion(if opening { Motion::Latch } else { Motion::Still });
        }
    }

    fn typed_form(&self, choices: &gtk::Box, method: &'static str, error: &gtk::Label) {
        let input = widgets::password_entry();
        let label = if method == "password" { t!("Vault password") } else { t!("Recovery key") };
        input.update_property(&[gtk::accessible::Property::Label(&label)]);
        input.set_placeholder_text(Some(&label));
        choices.append(&input);
        let submit = btn(&t!("Open"), None, &["primary"], || {});
        choices.append(&submit);
        let weak = self.me.clone();
        let (reading, pressing, failing) = (input.clone(), submit.clone(), error.clone());
        let go = Rc::new(move || {
            let Some(this) = weak.upgrade() else { return };
            if reading.text().is_empty() || !pressing.is_sensitive() {
                return;
            }
            let typed = secret::read_text(&reading);
            pressing.set_sensitive(false);
            say(&failing, "");
            this.opening(true);
            let (reading, pressing, failing) = (reading.clone(), pressing.clone(), failing.clone());
            glib::spawn_future_local(async move {
                let mut request = json!({"op": "unlock", "method": method});
                request["secret"] = rpc::encode(typed.as_bytes());
                let answer = rpc::ask(request).await;
                drop(typed);
                this.opening(false);
                pressing.set_sensitive(true);
                match answer {
                    Ok(answer) => {
                        reading.set_text("");
                        // With a recovery key, the person has lost another way in, and is told where a new one is made
                        if method == "recovery" {
                            this.say(&t!("Opened with the recovery key. Make a new way in under Your vaults."));
                        }
                        drop(answer);
                        this.opened();
                    }
                    Err(trouble) => {
                        let words = match &trouble {
                            Trouble::Refused { wait: 0, .. } => t!("That did not open anything."),
                            other => other.words(),
                        };
                        say(&failing, &words);
                        reading.grab_focus();
                    }
                }
            });
        });
        let run = go.clone();
        submit.connect_clicked(move |_| run());
        input.connect_activate(move |_| go());
        glib::idle_add_local_once(move || {
            input.grab_focus();
        });
    }

    /// Every security key slot this account has, each tried on the key plugged in
    pub fn open_with_key(&self, error: gtk::Label, pin: Option<Zeroizing<String>>) {
        self.spawn(async move |this| {
            say(&error, &t!("Touch your security key when it blinks."));
            let Ok(methods) = rpc::ask(json!({"op": "methods"})).await else {
                say(&error, &t!("The vault's service could not be reached."));
                return;
            };
            let slots = key_slots(&methods);
            let asked = pin.clone();
            let answered = gio::spawn_blocking(move || fido::answer(&slots, asked.as_ref().map(|pin| pin.as_str()))).await;
            let found = match answered {
                Ok(found) => found,
                Err(_) => Err(t!("The security key could not do that.")),
            };
            let found = match found {
                Ok(found) => found,
                Err(words) if pin.is_none() && fido::wants_pin(&words) => {
                    say(&error, "");
                    this.ask_pin(error);
                    return;
                }
                Err(words) => {
                    say(&error, &words);
                    return;
                }
            };
            for (slot, secret) in found {
                let mut request = json!({"op": "unlock", "method": "security-key", "slot": slot});
                request["secret"] = rpc::encode(&secret);
                let answer = rpc::ask(request).await;
                if answer.is_ok() {
                    say(&error, "");
                    this.opened();
                    return;
                }
            }
            say(&error, &t!("This security key is not one that opens a vault here."));
        });
    }

    fn ask_pin(&self, error: gtk::Label) {
        let sheet = widgets::sheet(&t!("Use a security key"), 420);
        sheet.body.append(&widgets::p(&t!("Enter the PIN of your security key, then touch the key when it blinks."), &["soft"]));
        let input = widgets::password_entry();
        sheet.body.append(&widgets::field(&t!("Security key PIN"), &input, &[]));
        let dialog = sheet.dialog.clone();
        let cancel = btn(&t!("Cancel"), None, &[], move || {
            dialog.close();
        });
        let (weak, dialog, reading) = (self.me.clone(), sheet.dialog.clone(), input.clone());
        let go = Rc::new(move || {
            let Some(this) = weak.upgrade() else { return };
            let pin = secret::take_text(&reading);
            if pin.is_empty() {
                return;
            }
            dialog.close();
            this.open_with_key(error.clone(), Some(pin));
        });
        let run = go.clone();
        let open = btn(&t!("Open"), None, &["primary"], move || run());
        input.connect_activate(move |_| go());
        sheet.pin(&widgets::actions(&[cancel.upcast(), open.upcast()]));
        sheet.dialog.present(Some(&self.shell.window));
    }
}

/// The security key slots of everything listed
fn key_slots(methods: &Value) -> Vec<fido::Slot> {
    methods["vaults"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|vault| vault["slots"].as_array().into_iter().flatten())
        .filter(|slot| slot["kind"] == "security-key")
        .map(|slot| fido::Slot {
            id: slot["id"].as_str().unwrap_or("").to_string(),
            credential: slot["credential"].as_str().unwrap_or("").to_string(),
            salt: slot["salt"].as_str().unwrap_or("").to_string(),
        })
        .collect()
}

use amethystora_ui::adw;
use adw::prelude::*;
