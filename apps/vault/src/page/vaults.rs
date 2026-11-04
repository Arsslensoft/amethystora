//! Your vaults: those listed and those open, a new one, hidden or not, how each opens, and the logos' switch. A hidden vault is only here
//! while it is open, and nothing else says it exists: not a count, and not by leaving something out that a person could notice.

use crate::scan;
use std::rc::Rc;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{glib, gtk, t, ui};
use serde_json::{json, Value};

use super::{Page, Vault};
use crate::icons::icon;
use crate::logos;
use crate::rpc::{self, Trouble};
use crate::secret;
use crate::widgets::{self, btn, say};

pub struct Vaults {
    vault: Rc<Vault>,
    root: gtk::ScrolledWindow,
    list: gtk::Box,
    logos_words: gtk::Label,
    logos_button: gtk::Button,
    me: std::cell::RefCell<std::rc::Weak<Vaults>>,
}

impl Vaults {
    pub fn new(vault: &Rc<Vault>) -> Rc<dyn Page> {
        let page = ui::vbox(18, &["page"]);
        let head = ui::hbox(12, &["page-head"]);
        let title = widgets::h1(&t!("Your vaults"));
        title.set_hexpand(true);
        head.append(&title);
        let weak = Rc::downgrade(vault);
        head.append(&btn(&t!("New vault"), Some("plus"), &["primary"], move || {
            if let Some(vault) = weak.upgrade() {
                new_vault(&vault);
            }
        }));
        page.append(&head);
        let list = ui::vbox(8, &[]);
        page.append(&list);

        let hidden = widgets::card();
        hidden.append(&widgets::h2(&t!("Hidden vaults")));
        hidden.append(&widgets::p(&t!("A hidden vault is not listed anywhere. To open it, type its password on the closed screen. It opens by password only, never syncs, and never fetches a logo."), &["soft", "small"]));
        hidden.append(&widgets::callout(&t!("This hides it from someone looking at the app, and from programs asking. It does not hide it from someone with administrator access to this computer, who can count the vault folders.")));
        page.append(&hidden);

        let logos_card = widgets::card();
        let logos_row = ui::hbox(12, &[]);
        let logos_text = ui::vbox(4, &[]);
        logos_text.set_hexpand(true);
        logos_text.append(&widgets::h2(&t!("Service logos")));
        let logos_words = ui::label("", &["soft", "small"]);
        logos_text.append(&logos_words);
        logos_row.append(&logos_text);
        let logos_button = btn(&t!("Turn on"), None, &["small"], || {});
        logos_button.set_valign(gtk::Align::Center);
        logos_row.append(&logos_button);
        logos_card.append(&logos_row);
        page.append(&logos_card);

        let backup = widgets::card();
        backup.append(&widgets::h2(&t!("Backups and starting over")));
        backup.append(&widgets::p(&t!("A sealed backup needs only your recovery key to open, and holds every vault here, including any you cannot see. These run in a terminal."), &["soft", "small"]));
        let row = ui::wrap(8, 8, &[]);
        let (backup_vault, restore_vault, forget_vault) = (vault.clone(), vault.clone(), vault.clone());
        row.append(&btn(&t!("Back up now"), Some("down"), &["small"], move || backup_vault.in_terminal(&["ame", "vault", "backup"])));
        row.append(&btn(&t!("Restore from a backup"), Some("up"), &["small"], move || {
            let vault = restore_vault.clone();
            glib::spawn_future_local(async move {
                if let Some(path) = scan::choose_file(&t!("Restore from a backup"), None).await {
                    vault.in_terminal(&["ame", "vault", "restore", &path.to_string_lossy()]);
                }
            });
        }));
        row.append(&btn(&t!("Forget every vault here"), Some("trash"), &["small", "danger"], move || forget_vault.in_terminal(&["ame", "vault", "forget"])));
        backup.append(&row);
        page.append(&backup);

        let root = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).hexpand(true).vexpand(true).child(&page).build();
        let this = Rc::new(Vaults { vault: vault.clone(), root, list, logos_words, logos_button: logos_button.clone(), me: std::cell::RefCell::new(std::rc::Weak::new()) });
        this.me.replace(Rc::downgrade(&this));
        let weak = this.me.borrow().clone();
        logos_button.connect_clicked(move |_| {
            if let Some(page) = weak.upgrade() {
                page.flip_logos();
            }
        });
        this
    }

    fn draw(&self, status: &Value) {
        ui::clear(&self.list);
        for vault in status["vaults"].as_array().into_iter().flatten() {
            let open = vault["open"] == json!(true);
            let hidden = vault["hidden"] == json!(true);
            let first = vault["first"] == json!(true);
            let id = vault["id"].as_str().unwrap_or("").to_string();
            let name = vault["name"].as_str().map_or_else(|| t!("A closed vault"), widgets::clean);
            let methods: Vec<String> = vault["methods"].as_array().into_iter().flatten().filter_map(|method| method.as_str().map(str::to_string)).collect();
            let has = |method: &str| methods.iter().any(|name| name == method);
            let how = if has("approval") {
                t!("Opens with your fingerprint or account password")
            } else if has("no-prompt") {
                t!("Opens without asking on this computer")
            } else if has("security-key") {
                t!("Opens with a security key or its own password")
            } else {
                t!("Opens with its own password")
            };
            let sub = if first { t!("The first vault: your certificates live here. {how}", how = how.clone()) } else { how };
            let row = ui::hbox(12, &["vault-row"]);
            let mark = icon("lock", 18);
            mark.add_css_class("accent-text");
            row.append(&mark);
            let words = ui::vbox(0, &[]);
            words.set_hexpand(true);
            words.append(&widgets::data(&name, &["item-title"]));
            words.append(&ui::label(&sub, &["soft", "small"]));
            row.append(&words);
            if hidden {
                row.append(&widgets::chip(&t!("Hidden"), "accent"));
            }
            row.append(&widgets::chip(&if open { t!("Open") } else { t!("Closed") }, if open { "ok" } else { "" }));
            let page = self.me.borrow().clone();
            if open {
                let action = btn(&t!("Settings"), None, &["small"], move || {
                    if let Some(page) = page.upgrade() {
                        settings(&page.vault, id.clone(), name.clone(), hidden, first);
                    }
                });
                row.append(&action);
            } else {
                let action = btn(&t!("Open"), None, &["small"], move || {
                    if let Some(page) = page.upgrade() {
                        open_another(&page.vault);
                    }
                });
                row.append(&action);
            }
            self.list.append(&row);
        }
    }

    fn draw_logos(&self) {
        let on = logos::is_on();
        self.logos_words.set_label(&if on {
            t!("On. Asks each site for its own icon, and keeps them sealed here. A hidden vault never does.")
        } else {
            t!("Asks each site for its own icon. That tells the site someone here has an account. Off until you choose. A letter stands in meanwhile.")
        });
        if let Some(words) = ui::button_words(&self.logos_button) {
            words.set_label(&if on { t!("Turn off") } else { t!("Turn on") });
        }
    }

    fn flip_logos(&self) {
        let (vault, weak) = (self.vault.clone(), self.me.borrow().clone());
        glib::spawn_future_local(async move {
            let on = logos::is_on();
            if !on {
                let sure = vault
                    .confirm(
                        &t!("Fetch service logos?"),
                        &t!("Vault asks each site for its own icon, and nothing else goes out but the site's name. That tells each site that someone at this address has an account there."),
                        &t!("Turn on"),
                        false,
                    )
                    .await;
                if !sure {
                    return;
                }
            }
            logos::set_on(!on).await;
            if let Some(page) = weak.upgrade() {
                page.draw_logos();
            }
            if !on {
                vault.reload_current();
            }
        });
    }
}

/// A vault's name and whether it is hidden, then its password, then its own recovery key
fn new_vault(vault: &Rc<Vault>) {
    let sheet = widgets::sheet(&t!("Make a vault"), 540);
    let name = widgets::entry("", "");
    sheet.body.append(&widgets::field(&t!("Name"), &name, &[]));
    let hidden = widgets::check(&t!("Hide this vault"), false, |_| {});
    sheet.body.append(&hidden);
    sheet.body.append(&widgets::p(&t!("Hidden, it will not be listed anywhere. To open it, type its password on the closed screen."), &["soft", "small"]));
    let (first, again) = (widgets::password_entry(), widgets::password_entry());
    sheet.body.append(&widgets::field(&t!("Its own password"), &first, &[]));
    sheet.body.append(&widgets::field(&t!("Once more"), &again, &[]));
    let error = widgets::problem();
    sheet.body.append(&error);
    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    let (this, dialog) = (vault.clone(), sheet.dialog.clone());
    let make = btn(&t!("Make it"), None, &["primary"], move || {
        let title = name.text().trim().to_string();
        if title.is_empty() {
            say(&error, &t!("Give it a name."));
            return;
        }
        let typed = secret::read_text(&first);
        if typed.chars().count() < 8 {
            say(&error, &t!("A password has at least 8 characters."));
            return;
        }
        if *typed != *secret::read_text(&again) {
            say(&error, &t!("That was not typed the same both times."));
            return;
        }
        let mut request = json!({"op": "new", "name": title, "hidden": hidden.is_active()});
        request["secret"] = rpc::encode(typed.as_bytes());
        let (vault, dialog, error) = (this.clone(), dialog.clone(), error.clone());
        glib::spawn_future_local(async move {
            match rpc::ask(request).await {
                Ok(answer) => {
                    dialog.close();
                    let recovery = zeroize::Zeroizing::new(answer["recovery"].as_str().unwrap_or("").to_string());
                    vault.show_recovery(recovery, false);
                }
                // Too short, too long, or a password that opens another vault: the same word for each, so that this cannot be asked
                Err(Trouble::Refused { wait: 0, .. }) => say(&error, &t!("That password cannot be used. Choose another.")),
                Err(trouble) => say(&error, &trouble.words()),
            }
        });
    });
    sheet.pin(&widgets::actions(&[cancel.upcast(), make.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

/// Another vault, with its password, beside what is open
fn open_another(vault: &Rc<Vault>) {
    let sheet = widgets::sheet(&t!("Open a vault"), 440);
    let input = widgets::password_entry();
    sheet.body.append(&widgets::field(&t!("Its password"), &input, &[]));
    let error = widgets::problem();
    sheet.body.append(&error);
    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    let (this, dialog, reading) = (vault.clone(), sheet.dialog.clone(), input.clone());
    let go = Rc::new(move || {
        let typed = secret::read_text(&reading);
        if typed.is_empty() {
            return;
        }
        let (vault, dialog, error) = (this.clone(), dialog.clone(), error.clone());
        glib::spawn_future_local(async move {
            let mut request = json!({"op": "unlock", "method": "password"});
            request["secret"] = rpc::encode(typed.as_bytes());
            match rpc::ask(request).await {
                Ok(_) => {
                    dialog.close();
                    vault.check();
                    vault.count_items();
                    vault.reload_current();
                }
                Err(Trouble::Refused { wait: 0, .. }) => say(&error, &t!("That did not open anything.")),
                Err(trouble) => say(&error, &trouble.words()),
            }
        });
    });
    let run = go.clone();
    let open = btn(&t!("Open"), None, &["primary"], move || run());
    input.connect_activate(move |_| go());
    sheet.pin(&widgets::actions(&[cancel.upcast(), open.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

/// One vault's settings: its name, how it opens, how long it stays open, taking it away
fn settings(vault: &Rc<Vault>, id: String, name: String, hidden: bool, first: bool) {
    let sheet = widgets::sheet(&t!("Settings of {name}", name = name.clone()), 560);
    let title = widgets::entry(&name, "");
    let (this, renaming, rename_id) = (vault.clone(), title.clone(), id.clone());
    let save_name = btn(&t!("Rename"), None, &["small"], move || {
        let (vault, new, id) = (this.clone(), renaming.text().to_string(), rename_id.clone());
        glib::spawn_future_local(async move {
            match vault.call(json!({"op": "rename", "vault": id, "name": new})).await {
                Ok(_) => {
                    vault.say(&t!("Renamed."));
                    vault.check();
                    vault.reload_current();
                }
                Err(trouble) => vault.say(&trouble.words()),
            }
        });
    });
    sheet.body.append(&widgets::field(&t!("Name"), &title, &[save_name.upcast()]));

    let ways = widgets::card();
    ways.append(&widgets::h2(&t!("How it opens")));
    let (this, change_id) = (vault.clone(), id.clone());
    let change = btn(&t!("Change the password"), Some("pass"), &["small"], move || change_password(&this, change_id.clone()));
    change.set_halign(gtk::Align::Start);
    ways.append(&change);
    // A hidden vault opens by password only: each other way would need the service to know which vault to offer
    if !hidden {
        let (this, for_vault) = (vault.clone(), id.clone());
        let approval = btn(&t!("Add fingerprint or account password"), Some("finger"), &["small"], move || {
            let (vault, id) = (this.clone(), for_vault.clone());
            glib::spawn_future_local(async move {
                match vault.call(json!({"op": "add-slot", "kind": "approval", "vault": id})).await {
                    Ok(_) => {
                        vault.say(&t!("Added."));
                        vault.check();
                    }
                    Err(Trouble::Refused { wait: 0, problem: None }) => {}
                    Err(trouble) => vault.say(&trouble.words()),
                }
            });
        });
        approval.set_halign(gtk::Align::Start);
        ways.append(&approval);
        // Into this vault, and not the one opened last
        let (this, for_vault) = (vault.clone(), id.clone());
        let key = btn(&t!("Add a security key"), Some("usb"), &["small"], move || this.add_security_key(false, Some(for_vault.clone())));
        key.set_halign(gtk::Align::Start);
        ways.append(&key);
    }
    sheet.body.append(&ways);

    let closing = widgets::card();
    closing.append(&widgets::h2(&t!("When it closes")));
    let minutes = gtk::SpinButton::with_range(1.0, 1440.0, 1.0);
    minutes.set_value(vault.status.borrow()["idle_minutes"].as_f64().unwrap_or(15.0));
    let this = vault.clone();
    let save_minutes = btn(&t!("Set"), None, &["small"], || {});
    let reading = minutes.clone();
    save_minutes.connect_clicked(move |_| {
        let (vault, value) = (this.clone(), reading.value() as u64);
        glib::spawn_future_local(async move {
            match vault.call(json!({"op": "set-idle", "minutes": value})).await {
                Ok(_) => {
                    vault.say(&t!("Set."));
                    vault.check();
                }
                Err(trouble) => vault.say(&trouble.words()),
            }
        });
    });
    closing.append(&widgets::field(&t!("Minutes without use"), &minutes, &[save_minutes.upcast()]));
    closing.append(&widgets::p(&t!("It always closes when the screen locks, when the computer sleeps and when you log out."), &["soft", "small"]));
    sheet.body.append(&closing);

    let dialog = sheet.dialog.clone();
    let done = btn(&t!("Done"), None, &[], move || {
        dialog.close();
    });
    let (this, dialog) = (vault.clone(), sheet.dialog.clone());
    let remove = btn(&t!("Remove this vault"), Some("trash"), &["danger"], move || {
        let (vault, dialog, id, name) = (this.clone(), dialog.clone(), id.clone(), name.clone());
        glib::spawn_future_local(async move {
            let sure = vault.confirm(&t!("Remove “{name}”?", name = name), &t!("Everything in it goes for good. Only a sealed backup can bring it back."), &t!("Remove"), true).await;
            if !sure {
                return;
            }
            match vault.call(json!({"op": "remove-this", "vault": id})).await {
                Ok(_) => {
                    dialog.close();
                    vault.say(&t!("Removed."));
                    vault.check();
                    vault.count_items();
                }
                Err(Trouble::Refused { wait: 0, problem: None }) => {}
                Err(trouble) => vault.say(&trouble.words()),
            }
        });
    });
    remove.set_visible(!first);
    sheet.pin(&widgets::actions(&[remove.upcast(), done.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

fn change_password(vault: &Rc<Vault>, id: String) {
    let sheet = widgets::sheet(&t!("Change the password"), 480);
    let (current, first, again) = (widgets::password_entry(), widgets::password_entry(), widgets::password_entry());
    sheet.body.append(&widgets::field(&t!("Current password, if it has one"), &current, &[]));
    sheet.body.append(&widgets::field(&t!("New password"), &first, &[]));
    sheet.body.append(&widgets::field(&t!("Once more"), &again, &[]));
    let error = widgets::problem();
    sheet.body.append(&error);
    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    let (this, dialog) = (vault.clone(), sheet.dialog.clone());
    let save = btn(&t!("Change it"), None, &["primary"], move || {
        let new = secret::read_text(&first);
        if new.chars().count() < 8 {
            say(&error, &t!("A password has at least 8 characters."));
            return;
        }
        if *new != *secret::read_text(&again) {
            say(&error, &t!("That was not typed the same both times."));
            return;
        }
        let mut request = json!({"op": "change-password", "vault": id});
        request["secret"] = rpc::encode(new.as_bytes());
        let old = secret::read_text(&current);
        if !old.is_empty() {
            request["current"] = rpc::encode(old.as_bytes());
        }
        let (vault, dialog, error) = (this.clone(), dialog.clone(), error.clone());
        glib::spawn_future_local(async move {
            match vault.call(request).await {
                Ok(_) => {
                    dialog.close();
                    vault.say(&t!("Changed."));
                    vault.check();
                }
                Err(Trouble::Refused { wait: 0, .. }) => say(&error, &t!("That was refused: the current password is wrong, or the new one cannot be used.")),
                Err(trouble) => say(&error, &trouble.words()),
            }
        });
    });
    sheet.pin(&widgets::actions(&[cancel.upcast(), save.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

impl Page for Vaults {
    fn widget(&self) -> gtk::Widget {
        self.root.clone().upcast()
    }

    fn reload(&self) {
        let status = self.vault.status.borrow().clone();
        self.draw(&status);
        self.draw_logos();
    }

}
