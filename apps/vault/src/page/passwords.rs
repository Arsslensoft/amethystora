//! Passwords: the list, one in detail, and the form that makes or changes one. What the list holds is the name, the username and the
//! address. The password itself is only asked for when it is shown or copied, and only for as long.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use amethystora_ui::{gtk, i18n, t, ui};
use amethystora_vault_core::generate;
use amethystora_vault_core::items::{Password, Secret, KIND_PASSWORD};
use amethystora_ui::adw::prelude::*;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use super::{generator, Page, Vault, Width};
use crate::list::{self, ListPage, Row};
use crate::rpc::{self, Trouble};
use crate::secret;
use crate::widgets::{self, btn, say};

const DOTS: &str = "••••••••••••••••••••";

pub struct Passwords {
    vault: Rc<Vault>,
    list: Rc<ListPage>,
    reused: RefCell<HashSet<String>>,
    me: RefCell<std::rc::Weak<Passwords>>,
}

/// The list's line for a stored password. `hidden` are the vaults whose items never ask a site for a logo.
fn row_of(entry: &Value, logos: bool, hidden: &HashSet<String>) -> Row {
    let title = list::text(entry, "name");
    let meta = &entry["meta"];
    let user = meta["username"].as_str().unwrap_or("");
    let address = meta["address"].as_str().unwrap_or("");
    let mut row = Row::new(entry, title, user.to_string(), None);
    row.words.push('\n');
    row.words.push_str(&address.to_lowercase());
    // Only the item's own address, and never for what a hidden vault holds
    if logos && !hidden.contains(&row.vault) {
        row.host = crate::logos::host_of(address);
    }
    row
}

impl Passwords {
    pub fn new(vault: &Rc<Vault>) -> Rc<dyn Page> {
        let weak = Rc::downgrade(vault);
        let list = ListPage::new(
            &t!("Search passwords"),
            &t!("New password"),
            move || {
                if let Some(vault) = weak.upgrade() {
                    Passwords::form(&vault, None);
                }
            },
            &t!("No passwords yet. Add one with the plus above."),
        );
        let page = Rc::new(Passwords { vault: vault.clone(), list: list.clone(), reused: RefCell::new(HashSet::new()), me: RefCell::new(std::rc::Weak::new()) });
        page.me.replace(Rc::downgrade(&page));
        let weak = Rc::downgrade(&page);
        list.on_select(move |row| {
            if let Some(page) = weak.upgrade() {
                page.render(row);
            }
        });
        page
    }

    fn render(&self, row: Option<&Row>) {
        self.vault.secrets.hide_all();
        let Some(row) = row else {
            self.list.clear();
            return;
        };
        let body = Vault::detail_body();
        let weak = self.vault.me.clone();
        let edit_id = row.id.clone();
        let edit_meta = row.meta.clone();
        let edit = btn(&t!("Edit"), None, &["small"], move || {
            if let Some(vault) = weak.upgrade() {
                Passwords::form(&vault, Some((edit_id.clone(), edit_meta.clone())));
            }
        });
        body.append(&self.vault.detail_head(&self.list, row, "", &[edit.upcast()]));

        let meta = &row.meta;
        let card = widgets::card();
        let user = meta["username"].as_str().unwrap_or("").to_string();
        let shown_user = widgets::entry(&widgets::clean(&user), "");
        shown_user.set_editable(false);
        shown_user.set_direction(gtk::TextDirection::Ltr);
        let vault = self.vault.clone();
        let copy_user = widgets::icon_btn("copy", &t!("Copy the username"), move || {
            amethystora_ui::process::copy(&user);
            vault.say(&t!("Copied."));
        });
        card.append(&widgets::field(&t!("Username"), &shown_user, &[copy_user.upcast()]));
        card.append(&self.password_field(&row.id));

        let chips = ui::wrap(8, 8, &[]);
        if meta["generated"] == json!(true) {
            chips.append(&widgets::chip(&t!("Generated, {bits} bits", bits = meta["bits"].as_u64().unwrap_or(0)), "ok"));
        } else if let Some(bits) = meta["bits"].as_u64() {
            chips.append(&widgets::chip(&t!("About {bits} bits, an estimate", bits = bits), ""));
        }
        if row.changed > 0 {
            // The service keeps the time in seconds
            chips.append(&widgets::chip(&t!("Changed {when}", when = i18n::days(i18n::days_from_today(row.changed as f64))), ""));
        }
        chips.append(&widgets::chip(&approval_words(&row.approval), ""));
        if self.reused.borrow().contains(&row.id) {
            chips.append(&widgets::chip(&t!("Another item has the same password"), "warn"));
        }
        card.append(&chips);
        let address = meta["address"].as_str().unwrap_or("").to_string();
        if !address.is_empty() {
            let shown = widgets::entry(&widgets::clean(&address), "");
            shown.set_editable(false);
            shown.set_direction(gtk::TextDirection::Ltr);
            card.append(&widgets::field(&t!("Address"), &shown, &[]));
        }
        if meta["note"] == json!(true) {
            card.append(&self.note_field(&row.id));
        }
        body.append(&card);

        let history = meta["history"].as_array().cloned().unwrap_or_default();
        if !history.is_empty() {
            let card = widgets::card();
            card.append(&widgets::h2(&t!("Earlier passwords")));
            for changed in &history {
                let line = ui::hbox(8, &["earlier"]);
                let dots = widgets::data("••••••••••••••", &["mono", "soft"]);
                dots.set_hexpand(true);
                dots.set_xalign(0.0);
                line.append(&dots);
                line.append(&ui::line(&t!("Until {date}", date = list::long_date(changed.as_i64().unwrap_or(0))), &["soft", "small"]));
                card.append(&line);
            }
            body.append(&card);
        }

        body.append(&self.vault.approval_card(KIND_PASSWORD, row));
        body.append(&self.vault.sync_card(KIND_PASSWORD, row));
        let weak = self.me.borrow().clone();
        let (id, title) = (row.id.clone(), row.title.clone());
        let remove = btn(&t!("Remove"), Some("trash"), &["danger", "small"], move || {
            let Some(page) = weak.upgrade() else { return };
            let reload = page.me.borrow().clone();
            page.vault.remove_with_undo(&page.list, KIND_PASSWORD, &id, &title, move || {
                if let Some(page) = reload.upgrade() {
                    page.reload();
                }
            });
        });
        remove.set_halign(gtk::Align::Start);
        body.append(&remove);
        self.list.show(&body);
    }

    /// The password as dots until it is asked for, for 30 seconds or until the window loses the focus
    fn password_field(&self, id: &str) -> gtk::Box {
        let shown = widgets::data(DOTS, &["mono", "secret-text"]);
        shown.set_direction(gtk::TextDirection::Ltr);
        shown.set_xalign(0.0);
        // Not selectable: a selection is on the primary selection at once, and Copy there is the clipboard with no hint and no end.
        // The button beside it is the only way out.
        let frame = ui::hbox(0, &["input", "secret-box"]);
        frame.append(&shown);
        let token: Rc<Cell<u64>> = Rc::new(Cell::new(0));
        let (vault, reading, held, id_show) = (self.vault.clone(), shown.clone(), token.clone(), id.to_string());
        let eye = widgets::icon_btn("eye", &t!("Show the password"), || {});
        let pressing = eye.clone();
        eye.connect_clicked(move |_| {
            // Shown already: back to dots
            if held.get() != 0 {
                vault.secrets.release(held.replace(0));
                reading.set_label(DOTS);
                widgets::relabel(&pressing, &t!("Show the password"));
                return;
            }
            let (vault, reading, held, id, pressing) = (vault.clone(), reading.clone(), held.clone(), id_show.clone(), pressing.clone());
            amethystora_ui::glib::spawn_future_local(async move {
                match vault.call(json!({"op": "password-copy", "id": id})).await {
                    Ok(answer) => {
                        let Some(text) = rpc::secret_of(&answer, "secret") else { return };
                        reading.set_label(&text);
                        widgets::relabel(&pressing, &t!("Hide the password"));
                        let (hiding, cleared, button) = (reading.clone(), held.clone(), pressing.clone());
                        held.set(vault.secrets.hold(move || {
                            hiding.set_label(DOTS);
                            cleared.set(0);
                            widgets::relabel(&button, &t!("Show the password"));
                        }));
                    }
                    Err(Trouble::Refused { wait: 0, problem: None }) => {}
                    Err(trouble) => vault.say(&trouble.words()),
                }
            });
        });
        let (vault, id_copy) = (self.vault.clone(), id.to_string());
        let copy = widgets::icon_btn("copy", &t!("Copy the password"), move || {
            let (vault, id) = (vault.clone(), id_copy.clone());
            amethystora_ui::glib::spawn_future_local(async move {
                match vault.call(json!({"op": "password-copy", "id": id})).await {
                    Ok(answer) => {
                        if let Some(text) = rpc::secret_of(&answer, "secret") {
                            vault.copy_secret(text.as_bytes());
                        }
                    }
                    Err(Trouble::Refused { wait: 0, problem: None }) => {}
                    Err(trouble) => vault.say(&trouble.words()),
                }
            });
        });
        let field = widgets::field(&t!("Password"), &frame, &[eye.upcast(), copy.upcast()]);
        // What a screen reader reads for the text is the label above it, and the text itself
        if let Some(label) = field.first_child() {
            shown.update_relation(&[gtk::accessible::Relation::LabelledBy(&[label.upcast_ref()])]);
        }
        field
    }

    /// The note is sealed with the password: it comes the same way, for as long
    fn note_field(&self, id: &str) -> gtk::Box {
        let shown = ui::label(&t!("Hidden until you ask."), &["soft"]);
        let (vault, id, reading) = (self.vault.clone(), id.to_string(), shown.clone());
        let show = widgets::icon_btn("eye", &t!("Show the note"), move || {
            let (vault, id, reading) = (vault.clone(), id.clone(), reading.clone());
            amethystora_ui::glib::spawn_future_local(async move {
                match vault.call(json!({"op": "get", "kind": KIND_PASSWORD, "id": id})).await {
                    Ok(answer) => {
                        reading.set_label(answer["value"]["note"].as_str().unwrap_or(""));
                        let hiding = reading.clone();
                        vault.secrets.hold(move || hiding.set_label(&t!("Hidden until you ask.")));
                    }
                    Err(Trouble::Refused { wait: 0, problem: None }) => {}
                    Err(trouble) => vault.say(&trouble.words()),
                }
            });
        });
        widgets::field(&t!("Note"), &shown, &[show.upcast()])
    }

    /// New, or changing what is held under an id, with what the list said of it
    fn form(vault: &Rc<Vault>, editing: Option<(String, Value)>) {
        let vault = vault.clone();
        amethystora_ui::glib::spawn_future_local(async move {
            // What it holds comes first, approved as the item says. It says which vault it is in, and the change goes there.
            let mut held_in: Option<String> = None;
            let held: Option<Password> = match &editing {
                Some((id, _)) => match vault.call(json!({"op": "get", "kind": KIND_PASSWORD, "id": id})).await {
                    Ok(answer) => {
                        held_in = answer["vault"].as_str().map(str::to_string);
                        serde_json::from_value(answer["value"].clone()).ok()
                    }
                    Err(Trouble::Refused { wait: 0, problem: None }) => return,
                    Err(trouble) => {
                        vault.say(&trouble.words());
                        return;
                    }
                },
                None => None,
            };
            Passwords::show_form(&vault, editing, held, held_in);
        });
    }

    fn show_form(vault: &Rc<Vault>, editing: Option<(String, Value)>, held: Option<Password>, held_in: Option<String>) {
        let id = editing.as_ref().map(|(id, _)| id.clone());
        let before = editing.map(|(_, meta)| meta);
        let sheet = widgets::sheet(&if id.is_some() { t!("Edit the password") } else { t!("New password") }, 540);
        let title = widgets::entry(held.as_ref().map_or("", |held| held.title.as_str()), "");
        sheet.body.append(&widgets::field(&t!("Name"), &title, &[]));
        let user = widgets::entry(held.as_ref().map_or("", |held| held.username.as_str()), "");
        user.set_direction(gtk::TextDirection::Ltr);
        sheet.body.append(&widgets::field(&t!("Username"), &user, &[]));
        let password = widgets::password_entry();
        if let Some(held) = &held {
            password.set_text(held.password.as_str());
        }
        let generated: Rc<Cell<Option<f64>>> = Rc::new(Cell::new(None));
        // What was made, to tell it from what is typed over it afterwards
        let made_text: Rc<RefCell<Zeroizing<String>>> = Rc::new(RefCell::new(Zeroizing::new(String::new())));
        let bits = widgets::chip("", "");
        bits.set_visible(false);
        let (filling, chip, chosen, window, remembered) = (password.clone(), bits.clone(), generated.clone(), vault.clone(), made_text.clone());
        let make = btn(&t!("Make one"), Some("refresh"), &["small"], move || {
            let (filling, chip, chosen, remembered) = (filling.clone(), chip.clone(), chosen.clone(), remembered.clone());
            generator::show(&window, move |made, real_bits| {
                remembered.replace(Zeroizing::new(made.to_string()));
                filling.set_text(&made);
                chosen.set(Some(real_bits));
                chip.set_label(&t!("Generated, {bits} bits", bits = real_bits.floor() as u64));
                chip.add_css_class("ok");
                chip.set_visible(true);
            });
        });
        sheet.body.append(&widgets::field(&t!("Password"), &password, &[make.upcast()]));
        let (chosen, chip, remembered) = (generated.clone(), bits.clone(), made_text.clone());
        password.connect_changed(move |entry| {
            // A typed password gets an estimate, marked as one. Anything that is not what was made is typed.
            let text = secret::read_text(entry);
            if chosen.get().is_some() && *text != **remembered.borrow() {
                chosen.set(None);
                chip.remove_css_class("ok");
            }
            if chosen.get().is_none() {
                chip.set_visible(!text.is_empty());
                if !text.is_empty() {
                    chip.set_label(&t!("About {bits} bits, an estimate", bits = generate::estimate_bits(&text).floor() as u64));
                }
            }
        });
        sheet.body.append(&bits);
        let address = widgets::entry(held.as_ref().map_or("", |held| held.address.as_str()), "https://");
        address.set_direction(gtk::TextDirection::Ltr);
        sheet.body.append(&widgets::field(&t!("Address"), &address, &[]));
        let note = gtk::TextView::builder().wrap_mode(gtk::WrapMode::WordChar).accepts_tab(false).css_classes(["note-input"]).build();
        if let Some(text) = held.as_ref().and_then(|held| held.note.as_ref()) {
            note.buffer().set_text(text.as_str());
        }
        sheet.body.append(&widgets::field(&t!("Note"), &note, &[]));
        // A change goes to the vault the item is in, and something new to the one chosen, which is the first vault unless another is
        let which = vault.target();
        if id.is_none() {
            if let Some(field) = which.field() {
                sheet.body.append(&field);
            }
        }
        let error = widgets::problem();
        sheet.body.append(&error);
        let dialog = sheet.dialog.clone();
        let cancel = btn(&t!("Cancel"), None, &[], move || {
            dialog.close();
        });
        let window = vault.shell.window.clone();
        // The password it had, to tell whether it was changed
        let held_password: Option<Zeroizing<String>> = held.as_ref().map(|held| Zeroizing::new(held.password.as_str().to_string()));
        let (keeping, dialog, vault) = (Rc::new(RefCell::new(held)), sheet.dialog.clone(), vault.clone());
        let save = btn(&t!("Save"), None, &["primary"], move || {
            let name = title.text().trim().to_string();
            if name.is_empty() {
                say(&error, &t!("Give it a name."));
                return;
            }
            let typed = secret::read_text(&password);
            if typed.is_empty() {
                say(&error, &t!("It has no password yet."));
                return;
            }
            let now = amethystora_ui::i18n::now() as i64 * 1000;
            let mut item = keeping.borrow().clone().unwrap_or_default();
            if item.added_at == 0 {
                item.added_at = now;
            }
            item.title = name.clone();
            item.username = user.text().to_string();
            // A full address with its scheme, which the browser extension matches the page's origin against
            item.address = with_scheme(address.text().trim());
            let buffer = note.buffer();
            let words = Zeroizing::new(buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string());
            item.note = (!words.is_empty()).then(|| Secret::new(words.as_str()));
            item.set_password(Secret::new(typed.as_str()), now);
            let real_bits = generated.get();
            // The name changed and the password did not: what was known of the password stays known
            let same = held_password.as_ref().is_some_and(|before| **before == *typed);
            let (bits, was_generated) = match (&before, same) {
                (Some(before), true) if before["bits"].is_u64() => (before["bits"].as_u64().unwrap_or(0), before["generated"] == json!(true)),
                _ => (real_bits.unwrap_or_else(|| generate::estimate_bits(&typed)).floor() as u64, real_bits.is_some()),
            };
            let meta = json!({
                "username": item.username,
                "address": item.address,
                "bits": bits,
                "generated": was_generated,
                "note": item.note.is_some(),
                "history": item.history.iter().map(|old| old.changed_at).collect::<Vec<i64>>(),
                "tags": item.tags.clone().unwrap_or_default(),
            });
            let Ok(value) = serde_json::to_value(&item) else { return };
            let mut request = json!({"op": "put", "kind": KIND_PASSWORD, "name": name, "meta": meta});
            request["value"] = value;
            match (&id, &held_in) {
                (Some(id), Some(there)) => {
                    request["id"] = json!(id);
                    request["vault"] = json!(there);
                }
                (Some(id), None) => request["id"] = json!(id),
                (None, _) => which.name(&mut request),
            }
            let (vault, dialog, error, editing) = (vault.clone(), dialog.clone(), error.clone(), id.clone());
            amethystora_ui::glib::spawn_future_local(async move {
                match vault.call(request).await {
                    Ok(answer) => {
                        dialog.close();
                        vault.count_items();
                        let chosen = editing.or_else(|| answer["id"].as_str().map(str::to_string));
                        vault.reload_and_select(chosen);
                    }
                    Err(trouble) => say(&error, &trouble.words()),
                }
            });
        });
        sheet.pin(&widgets::actions(&[cancel.upcast(), save.upcast()]));
        sheet.dialog.present(Some(&window));
    }
}

/// `example.org/login` as `https://example.org/login`; nothing, or an address with a scheme of its own, as it is
fn with_scheme(address: &str) -> String {
    if address.is_empty() || address.contains("://") {
        address.to_string()
    } else {
        format!("https://{address}")
    }
}

/// How an item is set to ask, in a few words
pub fn approval_words(approval: &str) -> String {
    match approval {
        "ask" => t!("Asks every time"),
        "never" => t!("Never asks"),
        _ => t!("Asks while open"),
    }
}

impl Page for Passwords {
    fn widget(&self) -> gtk::Widget {
        self.list.root.clone().upcast()
    }

    fn reload(&self) {
        let (vault, weak) = (self.vault.clone(), self.me.borrow().clone());
        vault.clone().spawn(async move |_| {
            crate::logos::refresh().await;
            let Ok(answer) = vault.call(json!({"op": "list", "kind": KIND_PASSWORD})).await else { return };
            let Some(page) = weak.upgrade() else { return };
            if let Ok(reused) = vault.call(json!({"op": "password-reused"})).await {
                let ids = reused["groups"].as_array().into_iter().flatten().flat_map(|group| group.as_array().into_iter().flatten()).filter_map(|id| id.as_str().map(str::to_string)).collect();
                page.reused.replace(ids);
            }
            let (logos, hidden) = (crate::logos::available(), vault.hidden_vaults());
            let mut rows: Vec<Row> = answer["items"].as_array().into_iter().flatten().map(|entry| row_of(entry, logos, &hidden)).collect();
            super::sort_rows(&mut rows);
            page.list.set_rows(rows);
            if vault.width.get() == Width::Full {
                page.list.select_first();
            }
        });
    }

    fn width(&self, width: Width) {
        self.list.set_narrow(width != Width::Full);
    }

    fn add(&self) {
        Passwords::form(&self.vault, None);
    }

    fn find(&self) {
        self.list.search.grab_focus();
    }

    fn select(&self, id: &str) {
        self.list.select_id(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_gets_a_scheme_the_extension_can_match() {
        assert_eq!(with_scheme("example.org/login"), "https://example.org/login");
        assert_eq!(with_scheme("http://nas.home.arpa"), "http://nas.home.arpa");
        assert_eq!(with_scheme(""), "");
    }
}
