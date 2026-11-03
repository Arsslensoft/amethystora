//! The first run: how the vault should open, then the recovery key, which has to be typed back. Nothing is kept until it has been: the
//! service holds the draft, and a window closed before then leaves no vault. The same two steps make every later vault.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{glib, gtk, t, ui};
use serde_json::json;
use zeroize::Zeroizing;

use super::Vault;
use crate::fido;
use crate::icons::icon;
use crate::rpc::{self, Trouble};
use crate::secret;
use crate::widgets::{self, btn, say};

#[derive(Clone, Copy, PartialEq)]
enum Way {
    Approval,
    Password,
    SecurityKey,
    NoPrompt,
}

impl Way {
    /// What `setup` is asked for. A security key opens a vault made for approval, and takes that way's place once it is added: `setup`
    /// has no way of its own to take a key that needs the vault open to be sealed under.
    fn method(self) -> &'static str {
        match self {
            Way::Approval | Way::SecurityKey => "approval",
            Way::Password => "password",
            Way::NoPrompt => "no-prompt",
        }
    }
}

impl Vault {
    pub fn show_first(&self) {
        if let Some(old) = self.root.child_by_name("first") {
            self.root.remove(&old);
        }
        let page = self.how_it_opens();
        self.root.add_named(&page, Some("first"));
        self.root.set_visible_child_name("first");
    }

    /// One way of opening, as a card of its own that is chosen by a click
    #[allow(clippy::too_many_arguments)]
    fn way_card(glyph: &str, title: &str, text: &str, tag: &str, tone: &str, group: &RefCell<Option<gtk::ToggleButton>>, on: bool, chosen: impl Fn() + 'static) -> gtk::ToggleButton {
        let row = ui::hbox(14, &[]);
        let mark = icon(glyph, 22);
        mark.set_valign(gtk::Align::Start);
        row.append(&mark);
        let words = ui::vbox(3, &[]);
        words.set_hexpand(true);
        words.append(&ui::line(title, &["way-title"]));
        words.append(&ui::label(text, &["soft", "small"]));
        row.append(&words);
        let chip = widgets::chip(tag, tone);
        chip.set_valign(gtk::Align::Start);
        row.append(&chip);
        let button = gtk::ToggleButton::builder().child(&row).css_classes(["plain", "way"]).active(on).build();
        button.update_property(&[gtk::accessible::Property::Label(&format!("{title}. {text}"))]);
        if let Some(first) = &*group.borrow() {
            button.set_group(Some(first));
        }
        if group.borrow().is_none() {
            group.replace(Some(button.clone()));
        }
        button.connect_toggled(move |button| {
            if button.is_active() {
                chosen();
            }
        });
        button
    }

    fn how_it_opens(&self) -> gtk::Widget {
        let column = ui::vbox(14, &["first"]);
        let tpm = self.status.borrow()["tpm"] == json!(true);
        let chosen = Rc::new(Cell::new(if tpm { Way::Approval } else { Way::Password }));
        let head = ui::vbox(6, &[]);
        head.append(&widgets::h1(&t!("How should your vault open?")));
        head.append(&widgets::p(&t!("You can add or change these later. Each one opens the same vault."), &["soft"]));
        column.append(&head);
        let group = RefCell::new(None);
        let pick = |way: Way| {
            let chosen = chosen.clone();
            move || chosen.set(way)
        };
        let recommended = t!("Recommended");
        let approval_text = if tpm { t!("GNOME asks you. No extra password to remember.") } else { t!("GNOME asks you. This computer has no TPM, so only the disk's encryption protects it while the computer is off.") };
        column.append(&Self::way_card("finger", &t!("Fingerprint or account password"), &approval_text, if tpm { &recommended } else { "" }, "accent", &group, tpm, pick(Way::Approval)));
        column.append(&Self::way_card(
            "pass",
            &t!("A vault password"),
            &t!("One more password, separate from your login. Nobody else can open the vault without it, not even an administrator."),
            &t!("Password"),
            "",
            &group,
            !tpm,
            pick(Way::Password),
        ));
        column.append(&Self::way_card("usb", &t!("A security key"), &t!("Touch it and enter its PIN. Resists a fake prompt."), &t!("No typing"), "", &group, false, pick(Way::SecurityKey)));
        if tpm {
            column.append(&Self::way_card(
                "bolt",
                &t!("No prompt"),
                &t!("Opens by itself on this computer. Stops programs that read files, but any program of yours can ask for what is inside."),
                &t!("Weakest"),
                "warn",
                &group,
                false,
                pick(Way::NoPrompt),
            ));
        }
        let error = widgets::problem();
        column.append(&error);
        let weak = self.me.clone();
        let go = btn(&t!("Continue"), None, &["primary"], move || {
            if let Some(this) = weak.upgrade() {
                this.way_chosen(chosen.get(), error.clone());
            }
        });
        let spring = ui::hbox(0, &[]);
        spring.set_vexpand(true);
        column.append(&spring);
        column.append(&widgets::actions(&[go.upcast()]));
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).hexpand(true).vexpand(true).child(&ui::limit_center(&column, 820)).build();
        scroller.upcast()
    }

    fn way_chosen(&self, way: Way, error: gtk::Label) {
        match way {
            Way::Password => self.new_password(&t!("Choose a vault password"), None, move |this, password| {
                let error = error.clone();
                this.start_setup(way, Some(password), error);
            }),
            Way::NoPrompt => {
                let sheet = widgets::sheet(&t!("No prompt"), 480);
                sheet.body.append(&widgets::p(&t!("No prompt stops a program that reads your files, a copied backup and a Flatpak with access to your home. It does not stop a program of this account that asks."), &[]));
                let typed = widgets::entry("", "");
                sheet.body.append(&widgets::field(&t!("To choose it, type: no prompt"), &typed, &[]));
                let dialog = sheet.dialog.clone();
                let cancel = btn(&t!("Cancel"), None, &[], move || {
                    dialog.close();
                });
                let (weak, dialog) = (self.me.clone(), sheet.dialog.clone());
                let choose = btn(&t!("Continue"), None, &["primary"], move || {
                    if typed.text() == t!("no prompt") {
                        dialog.close();
                        if let Some(this) = weak.upgrade() {
                            this.start_setup(way, None, error.clone());
                        }
                    }
                });
                sheet.pin(&widgets::actions(&[cancel.upcast(), choose.upcast()]));
                sheet.dialog.present(Some(&self.shell.window));
            }
            Way::Approval | Way::SecurityKey => self.start_setup(way, None, error),
        }
    }

    fn start_setup(&self, way: Way, password: Option<Zeroizing<String>>, error: gtk::Label) {
        self.spawn(async move |this| {
            say(&error, "");
            let mut request = json!({"op": "setup", "method": way.method()});
            if let Some(password) = &password {
                request["secret"] = rpc::encode(password.as_bytes());
            }
            drop(password);
            match rpc::ask(request).await {
                Ok(answer) => {
                    let recovery = Zeroizing::new(answer["recovery"].as_str().unwrap_or("").to_string());
                    this.show_recovery(recovery, way == Way::SecurityKey);
                }
                // A password too short is refused; so is a prompt the person answered no to, which is no error
                Err(Trouble::Refused { wait: 0, problem: None }) if way == Way::Password => say(&error, &t!("That was refused. A password has at least 8 characters.")),
                Err(Trouble::Refused { wait: 0, problem: None }) => {}
                Err(trouble) => say(&error, &trouble.words()),
            }
        });
    }

    /// A password typed twice, in a sheet of its own
    pub fn new_password(&self, title: &str, sentence: Option<&str>, done: impl Fn(&Vault, Zeroizing<String>) + 'static) {
        let sheet = widgets::sheet(title, 480);
        if let Some(sentence) = sentence {
            sheet.body.append(&widgets::p(sentence, &["soft"]));
        }
        let (first, again) = (widgets::password_entry(), widgets::password_entry());
        sheet.body.append(&widgets::field(&t!("New password"), &first, &[]));
        sheet.body.append(&widgets::field(&t!("Once more"), &again, &[]));
        sheet.body.append(&widgets::p(&t!("Nobody can recover a forgotten password but your recovery key."), &["soft", "small"]));
        let error = widgets::problem();
        sheet.body.append(&error);
        let dialog = sheet.dialog.clone();
        let cancel = btn(&t!("Cancel"), None, &[], move || {
            dialog.close();
        });
        let (weak, dialog, reading, repeating) = (self.me.clone(), sheet.dialog.clone(), first.clone(), again.clone());
        let go = Rc::new(move || {
            let Some(this) = weak.upgrade() else { return };
            let password = secret::read_text(&reading);
            if password.chars().count() < 8 {
                say(&error, &t!("A password has at least 8 characters."));
                return;
            }
            if *password != *secret::read_text(&repeating) {
                say(&error, &t!("That was not typed the same both times."));
                return;
            }
            dialog.close();
            done(&this, password);
        });
        let run = go.clone();
        let ok = btn(&t!("Continue"), None, &["primary"], move || run());
        let run = go.clone();
        first.connect_activate(move |_| run());
        again.connect_activate(move |_| go());
        sheet.pin(&widgets::actions(&[cancel.upcast(), ok.upcast()]));
        sheet.dialog.present(Some(&self.shell.window));
    }

    /// The recovery key, and the field it is typed back into
    pub fn show_recovery(&self, recovery: Zeroizing<String>, then_key: bool) {
        if let Some(old) = self.root.child_by_name("first") {
            self.root.remove(&old);
        }
        // Until it has been typed back, nothing the service says may take this page away: the key is on it, and the vault it belongs to is
        // not one yet
        self.recovery_up.set(true);
        let column = ui::vbox(16, &["first", "recovery"]);
        column.append(&widgets::h1(&t!("Keep your recovery key")));
        column.append(&widgets::p(&t!("It is the only way back if you lose every way to open your vault. Nobody can make you a new one. Write it down or print it, and keep it away from this computer."), &["soft"]));
        let groups = gtk::Grid::builder().column_homogeneous(true).column_spacing(10).row_spacing(10).css_classes(["recovery-key"]).build();
        groups.set_direction(gtk::TextDirection::Ltr);
        for (at, group) in recovery.split('-').enumerate() {
            let cell = ui::line(group, &["mono", "recovery-group"]);
            cell.set_xalign(0.5);
            // Not selectable: a selection is on the primary selection at once. Copy, here, empties the clipboard in 30 seconds.
            groups.attach(&cell, at as i32 % 4, at as i32 / 4, 1, 1);
        }
        column.append(&groups);
        let held = Rc::new(RefCell::new(Some(recovery)));
        let reading = held.clone();
        let weak = self.me.clone();
        let copy = btn(&t!("Copy"), Some("copy"), &["small"], move || {
            if let Some(text) = &*reading.borrow() {
                secret::copy(Zeroizing::new(text.as_bytes().to_vec()));
                if let Some(this) = weak.upgrade() {
                    this.say(&t!("Copied. It leaves the clipboard in 30 seconds."));
                }
            }
        });
        let reading = held.clone();
        let print = btn(&t!("Print"), Some("print"), &["small"], move || {
            if let Some(text) = &*reading.borrow() {
                crate::print::text(&t!("Amethystora Vault recovery key"), text);
            }
        });
        let row = ui::hbox(8, &[]);
        row.append(&copy);
        row.append(&print);
        column.append(&row);
        let typed = widgets::password_entry();
        typed.set_show_peek_icon(true);
        column.append(&widgets::field(&t!("Type the whole key back, to show you have it"), &typed, &[]));
        let error = widgets::problem();
        column.append(&error);
        let spring = ui::hbox(0, &[]);
        spring.set_vexpand(true);
        column.append(&spring);
        let weak = self.me.clone();
        let (reading, failing) = (typed.clone(), error.clone());
        let keep = held.clone();
        let go = Rc::new(move || {
            let Some(this) = weak.upgrade() else { return };
            let text = secret::read_text(&reading);
            let (failing, keep) = (failing.clone(), keep.clone());
            glib::spawn_future_local(async move {
                let mut request = json!({"op": "confirm"});
                request["recovery"] = rpc::encode(text.as_bytes());
                match rpc::ask(request).await {
                    Ok(_) => {
                        keep.borrow_mut().take();
                        this.recovery_up.set(false);
                        if then_key {
                            this.add_first_key();
                        }
                        // The page with the key on it goes once the screen changes (see apply_status)
                        this.opened();
                    }
                    Err(Trouble::Refused { .. }) => say(&failing, &t!("That is not your recovery key. Check it against what is written above.")),
                    Err(trouble) => say(&failing, &trouble.words()),
                }
            });
        });
        let run = go.clone();
        let finish = btn(&t!("Finish"), None, &["primary"], move || run());
        typed.connect_activate(move |_| go());
        column.append(&widgets::actions(&[finish.upcast()]));
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).hexpand(true).vexpand(true).child(&ui::limit_center(&column, 780)).build();
        self.root.add_named(&scroller, Some("first"));
        self.root.set_visible_child_name("first");
        typed.grab_focus();
    }

    /// A security key chosen at the first run: added now the vault is open, and the approval it was opened with is taken away once the
    /// key has been added
    fn add_first_key(&self) {
        self.add_security_key(true, None);
    }

    /// A credential on the key plugged in, which seals the vault key once more. Asks for the key's PIN first.
    pub fn add_security_key(&self, instead_of_approval: bool, vault: Option<String>) {
        let sheet = widgets::sheet(&t!("Add a security key"), 480);
        sheet.body.append(&widgets::p(&t!("Plug it in, and enter its PIN. Touch it each time it blinks: it blinks twice."), &["soft"]));
        let (name, pin) = (widgets::entry(&t!("Security key"), ""), widgets::password_entry());
        sheet.body.append(&widgets::field(&t!("What to call it"), &name, &[]));
        sheet.body.append(&widgets::field(&t!("Security key PIN"), &pin, &[]));
        let error = widgets::problem();
        sheet.body.append(&error);
        let dialog = sheet.dialog.clone();
        let cancel = btn(&t!("Cancel"), None, &[], move || {
            dialog.close();
        });
        let (weak, dialog) = (self.me.clone(), sheet.dialog.clone());
        let add = btn(&t!("Add it"), Some("usb"), &["primary"], || {});
        add.connect_clicked(move |add| {
            let Some(this) = weak.upgrade() else { return };
            let vault = vault.clone();
            let typed = secret::read_text(&pin);
            let (label, dialog, error, add) = (name.text().to_string(), dialog.clone(), error.clone(), add.clone());
            add.set_sensitive(false);
            say(&error, &t!("Touch your security key when it blinks."));
            glib::spawn_future_local(async move {
                let made = amethystora_ui::gio::spawn_blocking(move || fido::enrol(&typed)).await;
                add.set_sensitive(true);
                let (credential, salt, answer) = match made {
                    Ok(Ok(made)) => made,
                    Ok(Err(words)) => {
                        say(&error, &words);
                        return;
                    }
                    Err(_) => {
                        say(&error, &t!("The security key could not do that."));
                        return;
                    }
                };
                // It goes into the vault whose settings it was asked in, and no other
                let mut request = json!({"op": "add-security-key", "name": label, "credential": credential, "salt": salt});
                request["secret"] = rpc::encode(&answer);
                if let Some(vault) = &vault {
                    request["vault"] = json!(vault);
                }
                let added = rpc::ask(request).await;
                match added {
                    Ok(_) => {
                        dialog.close();
                        if instead_of_approval {
                            this.drop_approval_slot().await;
                        }
                        this.check();
                    }
                    Err(trouble) => say(&error, &trouble.words()),
                }
            });
        });
        sheet.pin(&widgets::actions(&[cancel.upcast(), add.upcast()]));
        sheet.dialog.present(Some(&self.shell.window));
    }

    async fn drop_approval_slot(&self) {
        let Ok(methods) = rpc::ask(json!({"op": "methods"})).await else { return };
        let slot = methods["vaults"].as_array().into_iter().flatten().flat_map(|vault| vault["slots"].as_array().into_iter().flatten()).find(|slot| slot["kind"] == "approval").and_then(|slot| slot["id"].as_str().map(str::to_string));
        if let Some(slot) = slot {
            let _ = rpc::ask(json!({"op": "remove-slot", "slot": slot})).await;
        }
    }
}
