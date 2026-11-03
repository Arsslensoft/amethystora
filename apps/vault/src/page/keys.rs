//! Keys: the list and one in detail, its public forms, which need no approval, and the page that makes one, with no list beside it, as
//! the board draws it. Keys are made in the service; the window only ever holds the public part.

use crate::scan;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{gio, glib, gtk, process, t, ui};
use amethystora_vault_core::keys;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use super::{export, Page, Vault, Width};
use crate::list::{self, ListPage, Row};
use crate::rpc::{self, Trouble};
use crate::secret;
use crate::widgets::{self, btn, say};

/// A type's name and what it is for
fn type_words(name: &str) -> (String, String) {
    match name {
        "ed25519" => ("Ed25519".into(), t!("The default: small and fast")),
        "ecdsa-p256" => ("ECDSA P-256".into(), t!("Widely accepted")),
        "ecdsa-p384" => ("ECDSA P-384".into(), t!("A larger curve")),
        "ecdsa-p521" => ("ECDSA P-521".into(), t!("A larger curve")),
        "rsa-3072" => ("RSA 3072".into(), t!("For older systems")),
        "rsa-4096" => ("RSA 4096".into(), t!("For older systems, larger")),
        "ml-dsa-65" => ("ML-DSA-65".into(), t!("Against quantum computers")),
        "x25519" => ("X25519".into(), t!("Key exchange")),
        "ml-kem-768" => ("ML-KEM-768".into(), t!("Post-quantum")),
        "aes-256" => ("AES-256".into(), t!("One shared key")),
        other => (other.to_string(), String::new()),
    }
}

fn kind_words(kind: &str) -> String {
    match kind {
        "ssh" => t!("SSH"),
        "signing" => t!("Signing"),
        "exchange" => t!("Exchange"),
        "symmetric" => t!("Symmetric"),
        other => other.to_string(),
    }
}

fn row_of(entry: &Value) -> Row {
    let meta = &entry["meta"];
    let (name, _) = type_words(meta["type"].as_str().unwrap_or(""));
    let sub = t!("{kind}, {key_type}", kind = kind_words(meta["key_kind"].as_str().unwrap_or("")), key_type = name);
    Row::new(entry, list::text(entry, "name"), sub, None)
}

pub struct Keys {
    vault: Rc<Vault>,
    list: Rc<ListPage>,
    me: RefCell<std::rc::Weak<Keys>>,
    making: Cell<bool>,
    columns: RefCell<Option<gtk::Box>>,
}

impl Keys {
    pub fn new(vault: &Rc<Vault>) -> Rc<dyn Page> {
        let holder: Rc<RefCell<std::rc::Weak<Keys>>> = Rc::new(RefCell::new(std::rc::Weak::new()));
        let weak = holder.clone();
        let list = ListPage::new(
            &t!("Search keys"),
            &t!("Make a key"),
            move || {
                if let Some(page) = weak.borrow().upgrade() {
                    page.make();
                }
            },
            &t!("No keys yet. Make one with the plus above."),
        );
        let page = Rc::new(Keys { vault: vault.clone(), list: list.clone(), me: RefCell::new(std::rc::Weak::new()), making: Cell::new(false), columns: RefCell::new(None) });
        page.me.replace(Rc::downgrade(&page));
        holder.replace(Rc::downgrade(&page));
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
        let meta = &row.meta;
        let body = Vault::detail_body();
        body.append(&self.vault.detail_head(&self.list, row, &row.sub, &[]));
        let public = widgets::card();
        public.append(&widgets::h2(&t!("The public key")));
        public.append(&widgets::p(&t!("Hand this out freely. It says nothing about the private key."), &["soft", "small"]));
        let pairs = widgets::Pairs::new();
        if let Some(fingerprint) = meta["ssh_sha256"].as_str().or(meta["sha256"].as_str()) {
            pairs.add(&t!("Fingerprint"), fingerprint, true);
        }
        public.append(&pairs.grid);
        let form = widgets::code("", &["small"]);
        form.set_visible(false);
        public.append(&form);
        let (vault, id, shown) = (self.vault.clone(), row.id.clone(), form.clone());
        let copying = meta["openssh"].as_str().map(str::to_string);
        let copy_vault = self.vault.clone();
        let actions = ui::wrap(8, 8, &[]);
        if let Some(line) = copying {
            actions.append(&btn(&t!("Copy for SSH"), Some("copy"), &["small"], move || {
                process::copy(&line);
                copy_vault.say(&t!("Copied."));
            }));
        }
        if meta["type"] != "aes-256" {
            actions.append(&btn(&t!("Show as PEM"), Some("eye"), &["small"], move || {
                let (vault, id, shown) = (vault.clone(), id.clone(), shown.clone());
                glib::spawn_future_local(async move {
                    match vault.call(json!({"op": "key-public", "id": id})).await {
                        Ok(answer) => {
                            shown.set_label(answer["public"]["pem"].as_str().unwrap_or(""));
                            shown.set_visible(true);
                        }
                        Err(trouble) => vault.say(&trouble.words()),
                    }
                });
            }));
        }
        public.append(&actions);
        body.append(&public);

        let private = widgets::card();
        private.append(&widgets::h2(&t!("The private key")));
        if meta["exportable"] == json!(false) {
            private.append(&widgets::p(&t!("This key can never leave the vault."), &["soft", "small"]));
        } else {
            private.append(&widgets::p(&t!("Exporting asks you every time, and from then on the key also lives outside the vault."), &["soft", "small"]));
            let (vault, id, name) = (self.vault.clone(), row.id.clone(), row.title.clone());
            let export_button = btn(&t!("Export it"), Some("down"), &["small"], move || export_key(&vault, id.clone(), name.clone()));
            export_button.set_halign(gtk::Align::Start);
            private.append(&export_button);
        }
        body.append(&private);
        body.append(&self.vault.approval_card("key", row));
        body.append(&self.vault.sync_card("key", row));
        let (weak, id, title) = (self.me.borrow().clone(), row.id.clone(), row.title.clone());
        let remove = btn(&t!("Remove"), Some("trash"), &["small", "danger"], move || {
            let Some(page) = weak.upgrade() else { return };
            let reload = page.me.borrow().clone();
            page.vault.remove_with_undo(&page.list, "key", &id, &title, move || {
                if let Some(page) = reload.upgrade() {
                    page.reload();
                }
            });
        });
        remove.set_halign(gtk::Align::Start);
        body.append(&remove);
        self.list.show(&body);
    }

    /// The page that makes a key, in the place of the list and the detail
    fn make(&self) {
        self.making.set(true);
        self.list.pane.set_visible(false);
        self.list.detail.set_visible(true);
        let this = self.me.borrow().clone();
        let vault = self.vault.clone();
        vault.clone().spawn(async move |_| {
            let types = vault.call(json!({"op": "key-types"})).await.map(|answer| answer.0.clone()).unwrap_or(Value::Null);
            if let Some(page) = this.upgrade() {
                page.make_page(types);
            }
        });
    }

    fn make_page(&self, types: Value) {
        let page = ui::vbox(18, &["page"]);
        let head = ui::hbox(12, &["page-head"]);
        let title = widgets::h1(&t!("Make a key"));
        title.set_hexpand(true);
        head.append(&title);
        page.append(&head);
        let columns = gtk::Box::builder().spacing(16).homogeneous(false).build();
        columns.set_orientation(if self.vault.width.get() == Width::Full { gtk::Orientation::Horizontal } else { gtk::Orientation::Vertical });
        self.columns.replace(Some(columns.clone()));
        let chosen_kind = Rc::new(RefCell::new("signing".to_string()));
        let chosen_type: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let exportable = Rc::new(Cell::new(true));
        let approval = Rc::new(RefCell::new("ask".to_string()));
        let left = widgets::card();
        left.set_hexpand(true);
        let name = widgets::entry("", "");
        left.append(&widgets::field(&t!("Name"), &name, &[]));
        // A key goes to the vault chosen, which is the first unless another is: it is named in the request
        let target = self.vault.target();
        if let Some(field) = target.field() {
            left.append(&field);
        }
        left.append(&ui::line(&t!("What it is for"), &["field-label"]));
        let tiles = ui::vbox(0, &[]);
        let draw_tiles: Rc<dyn Fn(&str)> = {
            let (tiles, types, chosen_type) = (tiles.clone(), types.clone(), chosen_type.clone());
            Rc::new(move |kind: &str| {
                ui::clear(&tiles);
                let names: Vec<String> = types[kind].as_array().into_iter().flatten().filter_map(|name| name.as_str().map(str::to_string)).collect();
                // Should the service not say, the kinds as core has them
                let names = if names.is_empty() { fallback_types(kind) } else { names };
                let group = RefCell::new(None);
                let mut made: Vec<gtk::Widget> = Vec::new();
                for (at, type_name) in names.iter().enumerate() {
                    let (shown, sub) = type_words(type_name);
                    let (keep, value) = (chosen_type.clone(), type_name.clone());
                    let tile = widgets::tile(&shown, &sub, &group, move || {
                        keep.replace(value.clone());
                    });
                    if at == 0 {
                        tile.set_active(true);
                        chosen_type.replace(type_name.clone());
                    }
                    made.push(tile.upcast());
                }
                tiles.append(&widgets::columns(if made.len() > 3 { 2 } else { made.len().max(1) as i32 }, 8, &made));
            })
        };
        let (keep_kind, redraw) = (chosen_kind.clone(), draw_tiles.clone());
        let kinds = [("ssh", kind_words("ssh")), ("signing", kind_words("signing")), ("exchange", kind_words("exchange")), ("symmetric", kind_words("symmetric"))];
        left.append(&widgets::seg(&kinds, "signing", move |value| {
            keep_kind.replace(value.to_string());
            redraw(value);
        }));
        left.append(&tiles);
        draw_tiles("signing");
        columns.append(&left);

        let right = ui::vbox(14, &[]);
        right.set_hexpand(true);
        let private = widgets::card();
        private.append(&widgets::h2(&t!("The private key")));
        let keep = exportable.clone();
        private.append(&widgets::check(&t!("Can be exported later"), true, move |on| keep.set(on)));
        private.append(&widgets::p(&t!("Turn this off and the key can never leave the vault. This cannot be changed afterwards."), &["soft", "small"]));
        private.append(&ui::line(&t!("Ask before each use"), &["field-label"]));
        let keep = approval.clone();
        private.append(&widgets::seg(&[("ask", t!("Every time")), ("while-open", t!("While open"))], "ask", move |value| {
            keep.replace(value.to_string());
        }));
        right.append(&private);
        let you_get = widgets::card();
        you_get.append(&widgets::h2(&t!("What you get")));
        you_get.append(&widgets::p(&t!("A public key you can hand out freely, and a fingerprint to recognize it by."), &["soft", "small"]));
        right.append(&you_get);
        let error = widgets::problem();
        right.append(&error);
        let (weak, reading) = (self.me.borrow().clone(), name.clone());
        let cancel = btn(&t!("Cancel"), None, &[], move || {
            if let Some(page) = weak.upgrade() {
                page.done_making(None);
            }
        });
        let (weak, failing) = (self.me.borrow().clone(), error.clone());
        let make = btn(&t!("Make the key"), Some("key"), &["primary"], || {});
        make.connect_clicked(move |make| {
            let Some(page) = weak.upgrade() else { return };
            let title = reading.text().trim().to_string();
            if title.is_empty() {
                say(&failing, &t!("Give it a name."));
                return;
            }
            let mut request = json!({"op": "key-new", "name": title, "key_kind": *chosen_kind.borrow(), "type": *chosen_type.borrow(), "exportable": exportable.get(), "approval": *approval.borrow()});
            target.name(&mut request);
            let (failing, make) = (failing.clone(), make.clone());
            make.set_sensitive(false);
            glib::spawn_future_local(async move {
                let answer = page.vault.call(request).await;
                make.set_sensitive(true);
                match answer {
                    Ok(answer) => {
                        page.vault.count_items();
                        page.done_making(answer["id"].as_str().map(str::to_string));
                    }
                    Err(trouble) => say(&failing, &trouble.words()),
                }
            });
        });
        let row = widgets::actions(&[cancel.upcast(), make.upcast()]);
        right.append(&row);
        columns.append(&right);
        page.append(&columns);
        let (vault, weak) = (self.vault.clone(), self.me.borrow().clone());
        let import = widgets::link(&t!("Bring in an existing SSH key"), move || import_key(&vault, weak.clone()));
        import.set_halign(gtk::Align::Start);
        page.append(&import);
        self.list.show(&page);
        name.grab_focus();
    }

    fn done_making(&self, select: Option<String>) {
        self.making.set(false);
        self.columns.replace(None);
        self.list.pane.set_visible(true);
        self.list.clear();
        self.list.current.replace(None);
        self.reload_selecting(select);
        self.list.apply_narrow();
    }

    fn reload_selecting(&self, select: Option<String>) {
        let (vault, weak) = (self.vault.clone(), self.me.borrow().clone());
        vault.clone().spawn(async move |_| {
            let Ok(answer) = vault.call(json!({"op": "list", "kind": "key"})).await else { return };
            let Some(page) = weak.upgrade() else { return };
            let mut rows: Vec<Row> = answer["items"].as_array().into_iter().flatten().map(row_of).collect();
            super::sort_rows(&mut rows);
            page.list.set_rows(rows);
            if page.making.get() {
                return;
            }
            match &select {
                Some(id) => page.list.select_id(id),
                None if vault.width.get() == Width::Full => page.list.select_first(),
                None => {}
            }
        });
    }
}

fn fallback_types(kind: &str) -> Vec<String> {
    let kind = match kind {
        "ssh" => keys::KeyKind::Ssh,
        "exchange" => keys::KeyKind::Exchange,
        "symmetric" => keys::KeyKind::Symmetric,
        _ => keys::KeyKind::Signing,
    };
    kind.types().into_iter().map(|key_type| key_type.name().to_string()).collect()
}

/// PKCS#8, encrypted under a passphrase unless the person says otherwise
fn export_key(vault: &Rc<Vault>, id: String, name: String) {
    let sheet = widgets::sheet(&t!("Export the private key"), 480);
    sheet.body.append(&widgets::callout(&t!("From now on this key also lives outside the vault, where any program of yours can read the file.")));
    let passphrase = widgets::password_entry();
    sheet.body.append(&widgets::field(&t!("A passphrase for the file"), &passphrase, &[]));
    let plain = widgets::check(&t!("No passphrase: anyone who can read the file has the key"), false, |_| {});
    sheet.body.append(&plain);
    let error = widgets::problem();
    sheet.body.append(&error);
    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    let (vault_go, dialog) = (vault.clone(), sheet.dialog.clone());
    let go = btn(&t!("Export it"), Some("down"), &["primary"], move || {
        let typed = secret::read_text(&passphrase);
        let mut request = json!({"op": "key-export", "id": id});
        if plain.is_active() {
            request["plain"] = json!(true);
        } else if typed.is_empty() {
            say(&error, &t!("Choose a passphrase, or say that you want none."));
            return;
        } else {
            request["passphrase"] = rpc::encode(typed.as_bytes());
        }
        let (vault, dialog, error, name) = (vault_go.clone(), dialog.clone(), error.clone(), name.clone());
        glib::spawn_future_local(async move {
            match vault.call(request).await {
                Ok(answer) => {
                    let Some(pem) = rpc::secret_of(&answer, "data") else { return };
                    dialog.close();
                    if export::save_to(&vault, &format!("{name}.pem"), Zeroizing::new(pem.as_bytes().to_vec())).await.is_some() {
                        vault.say(&t!("Saved."));
                    }
                }
                Err(Trouble::Refused { wait: 0, problem: None }) => {}
                Err(trouble) => say(&error, &trouble.words()),
            }
        });
    });
    sheet.pin(&widgets::actions(&[cancel.upcast(), go.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

/// An SSH key from a file: read here, off the window's thread, its private part handed to the service, and the file offered protection
fn import_key(vault: &Rc<Vault>, page: std::rc::Weak<Keys>) {
    let sheet = widgets::sheet(&t!("Bring in an existing SSH key"), 480);
    let path_shown = ui::label("", &["mono", "small"]);
    path_shown.set_direction(gtk::TextDirection::Ltr);
    let chosen: Rc<RefCell<Option<std::path::PathBuf>>> = Rc::new(RefCell::new(None));
    let (keep, shown) = (chosen.clone(), path_shown.clone());
    let pick = btn(&t!("Pick the key's file"), Some("key"), &["small"], move || {
        let (keep, shown) = (keep.clone(), shown.clone());
        glib::spawn_future_local(async move {
            if let Some(path) = scan::choose_file(&t!("Pick the key's file"), None).await {
                shown.set_label(&path.to_string_lossy());
                keep.replace(Some(path));
            }
        });
    });
    pick.set_halign(gtk::Align::Start);
    sheet.body.append(&pick);
    sheet.body.append(&path_shown);
    let name = widgets::entry("", "");
    sheet.body.append(&widgets::field(&t!("Name"), &name, &[]));
    let passphrase = widgets::password_entry();
    sheet.body.append(&widgets::field(&t!("Its passphrase, if it has one"), &passphrase, &[]));
    let target = vault.target();
    if let Some(field) = target.field() {
        sheet.body.append(&field);
    }
    let error = widgets::problem();
    sheet.body.append(&error);
    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    let (vault_go, dialog) = (vault.clone(), sheet.dialog.clone());
    let go = btn(&t!("Bring it in"), None, &["primary"], move || {
        let Some(path) = chosen.borrow().clone() else {
            say(&error, &t!("Pick the key's file."));
            return;
        };
        let title = name.text().trim().to_string();
        if title.is_empty() {
            say(&error, &t!("Give it a name."));
            return;
        }
        let typed = secret::read_text(&passphrase);
        let (vault, dialog, error, page, target) = (vault_go.clone(), dialog.clone(), error.clone(), page.clone(), target.clone());
        glib::spawn_future_local(async move {
            let at = path.clone();
            let read = gio::spawn_blocking(move || {
                let bytes = Zeroizing::new(std::fs::read(&at).ok()?);
                keys::import_openssh(&bytes, typed.as_bytes()).ok().map(|imported| (imported.key.key_type().name(), rpc::encode(imported.key.as_bytes()), imported.comment))
            })
            .await
            .ok()
            .flatten();
            let Some((key_type, private, comment)) = read else {
                say(&error, &t!("That file is not a key Vault can read, or its passphrase is wrong."));
                return;
            };
            let mut request = json!({"op": "key-import", "name": title, "type": key_type, "comment": comment, "key_kind": "ssh"});
            request["private"] = private;
            target.name(&mut request);
            match vault.call(request).await {
                Ok(answer) => {
                    dialog.close();
                    vault.count_items();
                    if let Some(page) = page.upgrade() {
                        page.done_making(answer["id"].as_str().map(str::to_string));
                    }
                    let shown = path.to_string_lossy().into_owned();
                    let toast = amethystora_ui::adw::Toast::builder().title(t!("Brought in. The file is still where it was.")).use_markup(false).button_label(t!("Protect the file")).timeout(10).build();
                    toast.connect_button_clicked(move |_| process::in_terminal(&["ame", "vault", "protect", &shown]));
                    vault.add_toast(toast, None);
                }
                Err(trouble) => say(&error, &trouble.words()),
            }
        });
    });
    sheet.pin(&widgets::actions(&[cancel.upcast(), go.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

impl Page for Keys {
    fn widget(&self) -> gtk::Widget {
        self.list.root.clone().upcast()
    }

    fn reload(&self) {
        self.reload_selecting(self.list.current.borrow().clone());
    }

    fn width(&self, width: Width) {
        if self.making.get() {
            if let Some(columns) = &*self.columns.borrow() {
                columns.set_orientation(if width == Width::Full { gtk::Orientation::Horizontal } else { gtk::Orientation::Vertical });
            }
        } else {
            self.list.set_narrow(width != Width::Full);
        }
    }

    fn add(&self) {
        self.make();
    }

    fn find(&self) {
        self.list.search.grab_focus();
    }

    fn select(&self, id: &str) {
        self.list.select_id(id);
    }
}
