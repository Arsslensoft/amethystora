//! The settings, as notes.js drew them: encryption and passkeys, locking, the reminders, imports, backups
//! and the export, and where the notes are.

use std::rc::Rc;

use serde_json::Value;

use amethystora_ui::{adw, gio, glib, gtk, i18n, t, ui};
use gtk::prelude::*;

use super::widgets::{self, button, key_status, password, row_icon, say, select, switch, toast, Action, Dialog, PassphraseFields};
use super::Notes;
use crate::backend::{self, Change, Passkey};
use crate::data::{Fields, Kind};
use crate::import::Counts;
use crate::dates;
use crate::passkeys;

/// What an import brought in, by the kinds the backend counts
fn counted(kind: &str, count: u64) -> String {
    match kind {
        "notes" => t!("{count, plural, one {# note} other {# notes}}", count = count),
        "notebooks" => t!("{count, plural, one {# notebook} other {# notebooks}}", count = count),
        "tasks" => t!("{count, plural, one {# task} other {# tasks}}", count = count),
        "projects" => t!("{count, plural, one {# project} other {# projects}}", count = count),
        _ => t!("{count, plural, one {# attachment} other {# attachments}}", count = count),
    }
}

/// A row of a card: its icon, its words and what it has at its end
fn setting_row(glyph: &str, tone: &str, heading: &str, detail: &str, controls: Vec<gtk::Widget>, first: bool) -> gtk::Box {
    let row = ui::hbox(14, &["row"]);
    if first {
        row.add_css_class("first");
    }
    row.append(&row_icon(glyph, tone, 20));
    let text = ui::vbox(2, &["row-text", "grow"]);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);
    text.append(&ui::label(heading, &["row-title"]));
    if !detail.is_empty() {
        let detail = ui::label(detail, &["row-detail"]);
        detail.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        text.append(&ui::limit(&detail, 520));
    }
    row.append(&text);
    if !controls.is_empty() {
        let side = ui::hbox(8, &["row-side"]);
        side.set_valign(gtk::Align::Center);
        for control in controls {
            control.set_valign(gtk::Align::Center);
            side.append(&control);
        }
        row.append(&side);
    }
    row
}

fn card(rows: Vec<gtk::Box>) -> gtk::Box {
    let card = ui::vbox(0, &["card", "list"]);
    card.set_overflow(gtk::Overflow::Hidden);
    for (at, row) in rows.into_iter().enumerate() {
        if at == 0 {
            row.add_css_class("first");
        }
        card.append(&row);
    }
    card
}

fn section(title: &str, body: &impl IsA<gtk::Widget>) -> gtk::Box {
    let section = ui::vbox(12, &["section"]);
    // In capitals the language's own way: GTK does not measure the stylesheet's text-transform
    section.append(&ui::label(&title.to_uppercase(), &["h2"]));
    section.append(body);
    section
}

impl Notes {
    pub fn render_settings(&self) {
        self.controls_for(None);
        let (encrypted, passkeys, dir) = {
            let ui = self.ui.borrow();
            (ui.encrypted, ui.passkeys.clone(), ui.dir.clone())
        };
        let minutes = self.setting("autoLock").and_then(|v| v.as_f64()).unwrap_or(10.0);
        let background = self.setting("background") != Some(Value::Bool(false));
        let page = ui::vbox(30, &["page"]);
        let head = ui::vbox(0, &["page-head"]);
        head.append(&ui::label(&t!("Settings"), &["h1"]));
        head.append(&ui::limit(&ui::label(&t!("Everything stays on this computer. Nothing is synced, and the app never connects to the internet."), &["lead"]), 600));
        page.append(&head);

        // Encryption
        let mut rows = Vec::new();
        if encrypted {
            let detail = if passkeys.is_empty() {
                t!("AES-256-GCM, under a key that opens only with your passphrase (scrypt, 128 MiB a guess). There is no public-key cryptography in it for a quantum computer to break, and AES-256 keeps 128-bit strength against one.")
            } else {
                t!("AES-256-GCM, under a key that opens only with your passphrase (scrypt, 128 MiB a guess) or a passkey. There is no public-key cryptography in it for a quantum computer to break, and AES-256 keeps 128-bit strength against one.")
            };
            let change = button(&t!("Change passphrase"), Some("key"), &["small"], self.on(|this| this.encryption_dialog(Change::Passphrase { new_key: true })));
            rows.push(setting_row("shield", "ok", &t!("Encrypted"), &detail, vec![change.upcast()], false));
            let add = button(&t!("Add a passkey"), Some("plus"), &["small"], self.on(|this| this.passkey_dialog()));
            rows.push(setting_row("key", "info", &t!("Passkeys"), &t!("A security key opens your notes with its PIN or your fingerprint, so that you need not type the passphrase. The passphrase keeps working."), vec![add.upcast()], false));
        } else {
            let enable = button(&t!("Turn on encryption"), Some("lock"), &["small", "primary"], self.on(|this| this.encryption_dialog(Change::Enable)));
            rows.push(setting_row("unlock", "off", &t!("Not encrypted"), &t!("Your notes are on disk as they are. Anyone who can read your files can read them."), vec![enable.upcast()], false));
        }
        for passkey in &passkeys {
            let detail = if passkey.protected {
                t!("Added {date}", date = dates::stamp(passkey.created))
            } else {
                t!("Added {date}, before passkeys needed the PIN or a fingerprint every time. Remove it and add it again.", date = dates::stamp(passkey.created))
            };
            let chosen = passkey.clone();
            let remove = button(&t!("Remove"), None, &["small"], self.on(move |this| this.remove_passkey(chosen.clone())));
            rows.push(setting_row("key", if passkey.protected { "ok" } else { "warn" }, &passkey.name, &detail, vec![remove.upcast()], false));
        }
        if encrypted {
            let mut choices: Vec<(String, String)> = [5, 10, 15, 30].iter().map(|count| (count.to_string(), t!("{count, plural, one {# minute} other {# minutes}}", count = *count))).collect();
            choices.push(("60".into(), t!("{count, plural, one {# hour} other {# hours}}", count = 1)));
            choices.push(("0".into(), t!("Only then")));
            let weak = self.me.clone();
            let lock_after = select(&t!("Lock after"), &choices, &crate::data::js_number(minutes), &["compact"], move |value| {
                if let (Some(this), Ok(minutes)) = (weak.upgrade(), value.parse::<i64>()) {
                    this.set_setting("autoLock", Value::from(minutes));
                }
            });
            rows.push(setting_row("lock", "info", &t!("Lock when away"), &t!("Locks after this long without using Notes, and whenever the screen locks, the computer sleeps or the window closes."), vec![lock_after.upcast()], false));
            let disable = button(&t!("Turn off"), None, &["small"], self.on(|this| this.encryption_dialog(Change::Disable)));
            rows.push(setting_row("unlock", "info", &t!("Turn off encryption"), &t!("Writes everything back to disk unencrypted."), vec![disable.upcast()], false));
        }
        page.append(&section(&t!("Encryption"), &card(rows)));

        // Reminders
        let weak = self.me.clone();
        let keep = switch(background, &t!("Keep running for reminders"), move |on| {
            if let Some(this) = weak.upgrade() {
                this.set_setting("background", Value::Bool(on));
            }
        });
        page.append(&section(&t!("Reminders"), &card(vec![setting_row(
            "bell",
            "accent",
            &t!("Keep running for reminders"),
            &t!("When you close the window, Notes stays running in the background until the reminders still to come have gone off. Reminders only go off while Notes is running."),
            vec![keep.upcast()],
            false,
        )])));

        // Imports: Imported is the notebook's name in every language, by which the backend finds it again
        let import = || button(&t!("Import…"), Some("upload"), &["small"], self.on(|this| this.run_import()));
        page.append(&section(&t!("Import"), &card(vec![
            setting_row("notes", "accent", &t!("From Joplin"), &t!("In Joplin, choose File, Export all, JEX, then import the .jex file here. Notebooks, notes, tags, attachments and to-dos all come across."), vec![import().upcast()], false),
            setting_row("done", "accent", &t!("From Planify"), &t!("In Planify, open Preferences, Backup and create a backup, then import its .json file here. Planify keeps them in {folder}.", folder = "~/.var/app/io.github.alainm23.planify/data/io.github.alainm23.planify/backups"), vec![import().upcast()], false),
            setting_row("note", "accent", &t!("Markdown files, or a Notes backup"), &t!("Markdown files go into a notebook called {notebook}. A backup is added to what is here, and asks for the passphrase it was made with.", notebook = "Imported"), vec![import().upcast()], false),
        ])));

        // Back up and export
        let backup_detail = if encrypted {
            t!("One file with everything in it, encrypted like your notes. It opens with the passphrase you have now, on any computer.")
        } else {
            t!("One file with everything in it. It is not encrypted, because your notes are not.")
        };
        page.append(&section(&t!("Back up and export"), &card(vec![
            setting_row("archive", "accent", &t!("Back up to a file"), &backup_detail, vec![button(&t!("Back up…"), Some("download"), &["small"], self.on(|this| this.run_backup())).upcast()], false),
            setting_row("download", "info", &t!("Export as Markdown"), &t!("Every note as a Markdown file in a folder per notebook, and every project as a checklist. The files are not encrypted."), vec![button(&t!("Export…"), Some("download"), &["small"], self.on(|this| this.run_export())).upcast()], false),
        ])));

        let command = ui::hbox(0, &["command"]);
        let code = ui::label(&dir, &["mono"]);
        code.set_selectable(true);
        code.set_wrap_mode(gtk::pango::WrapMode::Char);
        code.set_direction(gtk::TextDirection::Ltr);
        code.set_hexpand(true);
        command.append(&code);
        let place = ui::vbox(10, &[]);
        place.append(&command);
        if let Some(store) = &self.backend.borrow().store {
            let counts = Counts {
                notes: store.values(Kind::Notes).filter(|note| !note.is("trashed")).count() as u64,
                notebooks: store.kind(Kind::Notebooks).len() as u64,
                tasks: store.kind(Kind::Tasks).len() as u64,
                projects: store.kind(Kind::Projects).len() as u64,
                attachments: store.resources.len() as u64,
            };
            let parts: Vec<String> = counts.list().into_iter().map(|(kind, n)| counted(kind, n)).collect();
            place.append(&ui::label(&i18n::list(&parts), &["soft"]));
        }
        page.append(&section(&t!("Where they are"), &place));

        let clamp = adw::Clamp::builder().maximum_size(860).tightening_threshold(860).child(&page).build();
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).hexpand(true).child(&clamp).build();
        ui::clear(&self.main);
        self.main.append(&scroller);
    }

    fn encryption_dialog(&self, action: Change) {
        self.spawn(async move |this| {
            let current = action != Change::Enable;
            let fields = Rc::new(PassphraseFields::new(current));
            let (title, text, confirm) = match action {
                Change::Enable => (t!("Turn on encryption"), t!("Choose a passphrase. From now on your notes, tasks and attachments are encrypted on disk."), t!("Encrypt")),
                Change::Passphrase { .. } => (t!("Change the passphrase"), t!("Your notes stay encrypted, and open with the new passphrase from now on."), t!("Change")),
                Change::Disable => (t!("Turn off encryption"), t!("Your notes, tasks and attachments are written to disk as they are, readable by anyone who can read your files."), t!("Turn off")),
            };
            // A new key as well, unless asked not to: an old vault.json, from a snapshot or a backup, would
            // still open the same key with the old passphrase
            let new_key = Rc::new(std::cell::Cell::new(true));
            let toggled = new_key.clone();
            let key_switch = switch(true, &t!("Change the key as well"), move |on| toggled.set(on));
            let passkeys = !this.ui.borrow().passkeys.is_empty();
            let detail = format!(
                "{}{}",
                t!("Your notes and attachments are sealed anew, so that an old copy of vault.json, from a snapshot or a backup, cannot open them with the old passphrase."),
                if passkeys { format!(" {}", t!("Your passkeys have to be added again.")) } else { String::new() }
            );
            let key_row = setting_row("key", "info", &t!("Change the key as well"), &detail, vec![key_switch.upcast()], true);
            key_row.add_css_class("dialog-row");
            let nodes = fields.nodes();
            let body: Vec<gtk::Widget> = match action {
                Change::Enable => nodes,
                Change::Passphrase { .. } => {
                    let mut body: Vec<gtk::Widget> = nodes[..nodes.len() - 1].to_vec();
                    body.push(key_row.upcast());
                    body.push(fields.error.clone().upcast());
                    body
                }
                Change::Disable => vec![fields.old.clone().expect("asked for").upcast(), fields.error.clone().upcast()],
            };
            let (copy, keyed, backend) = (fields.clone(), new_key.clone(), this.backend.clone());
            let result = widgets::modal(Dialog {
                title,
                text: Some(text),
                body,
                actions: vec![
                    Action::plain(&t!("Cancel"), None),
                    Action::primary(&confirm, action == Change::Disable, move || {
                        let (fields, keyed, backend) = (copy.clone(), keyed.clone(), backend.clone());
                        Box::pin(async move {
                            let next = if action == Change::Disable { String::new() } else { fields.check()? };
                            let old = fields.old.as_ref().map(|old| old.text().to_string()).unwrap_or_default();
                            if fields.old.is_some() && old.is_empty() {
                                fields.say(&t!("Enter the current passphrase."));
                                return None;
                            }
                            fields.say(&t!("Working…"));
                            let change = match action {
                                Change::Passphrase { .. } => Change::Passphrase { new_key: keyed.get() },
                                other => other,
                            };
                            match backend::set_encryption(&backend, change, old, next).await {
                                Err(error) => {
                                    fields.say(&error);
                                    None
                                }
                                Ok(()) => Some(Some(change)),
                            }
                        })
                    }),
                ],
                wide: false,
            })
            .await
            .flatten();
            let Some(change) = result else { return };
            {
                let backend = this.backend.borrow();
                let mut ui = this.ui.borrow_mut();
                ui.encrypted = backend.encrypted();
                ui.passkeys = backend.passkeys();
            }
            this.render();
            toast(&match change {
                Change::Enable => t!("Encryption is on."),
                Change::Passphrase { new_key: true } => t!("The passphrase and the key are changed."),
                Change::Passphrase { new_key: false } => t!("The passphrase is changed."),
                Change::Disable => t!("Encryption is off."),
            }, None);
        });
    }

    /// Adds the security key that is plugged in as a passkey. It is asked twice, once to make the passkey
    /// and once to use it, so it blinks twice.
    fn passkey_dialog(&self) {
        self.spawn(async |this| {
            let current = password(&t!("Current passphrase"));
            let pin = password(&t!("Security key PIN"));
            let hint = ui::label(&t!("Leave the PIN empty for a key that reads your fingerprint."), &["hint", "small"]);
            let error = ui::label("", &["error", "small"]);
            error.set_visible(false);
            let (reading, pinning, saying, backend) = (current.clone(), pin.clone(), error.clone(), this.backend.clone());
            let result = widgets::modal(Dialog {
                title: t!("Add a passkey"),
                text: Some(t!("Plug in your security key. It will open your notes with its PIN or your fingerprint, and your passphrase keeps working.")),
                body: vec![current.upcast(), pin.upcast(), hint.upcast(), error.upcast()],
                actions: vec![
                    Action::plain(&t!("Cancel"), None),
                    Action::primary(&t!("Add"), false, move || {
                        let (reading, pinning, saying, backend) = (reading.clone(), pinning.clone(), saying.clone(), backend.clone());
                        Box::pin(async move {
                            if backend.borrow().touching.is_some() {
                                return None;
                            }
                            let current = reading.text().to_string();
                            if current.is_empty() {
                                say(&saying, &t!("Enter the current passphrase."));
                                return None;
                            }
                            key_status(&saying, &t!("Touch your security key each time it blinks: twice."), true);
                            let pin = pinning.text().to_string();
                            match passkeys::with_key(&backend, async || passkeys::add(&backend, current, pin).await).await {
                                Err(problem) => {
                                    key_status(&saying, &problem, false);
                                    None
                                }
                                Ok(()) => Some(Some(())),
                            }
                        })
                    }),
                ],
                wide: false,
            })
            .await
            .flatten();
            if result.is_none() {
                // Closed while the key was still waiting to be touched
                passkeys::cancel(&this.backend);
                return;
            }
            let passkeys = this.backend.borrow().passkeys();
            this.ui.borrow_mut().passkeys = passkeys;
            this.render();
            toast(&t!("The passkey is added."), None);
        });
    }

    fn remove_passkey(&self, passkey: Passkey) {
        self.spawn(async move |this| {
            let sure = widgets::confirm(
                &t!("Remove {name}?", name = passkey.name.clone()),
                &t!("It will no longer open your notes. Your passphrase keeps working. If the security key was lost, change the passphrase and the key as well: an old copy of vault.json, from a snapshot or a backup, still opens them with it."),
                &t!("Remove"),
                true,
            )
            .await;
            if !sure {
                return;
            }
            match passkeys::remove(&this.backend, &passkey.id) {
                Err(error) => toast(&error, None),
                Ok(()) => {
                    let passkeys = this.backend.borrow().passkeys();
                    this.ui.borrow_mut().passkeys = passkeys;
                    this.render();
                }
            }
        });
    }

    fn run_import(&self) {
        self.commit();
        self.spawn(async |this| {
            if this.backend.borrow().store.is_none() {
                toast(&t!("Unlock your notes first."), None);
                return;
            }
            let filter = gtk::FileFilter::new();
            filter.set_name(Some(&t!("Joplin export, Planify backup, Markdown or Notes backup")));
            for suffix in ["jex", "json", "md", "markdown", "txt", "amnotes"] {
                filter.add_suffix(suffix);
            }
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);
            let dialog = gtk::FileDialog::builder().title(t!("Import")).filters(&filters).modal(true).build();
            let Ok(files) = dialog.open_multiple_future(Some(&this.shell.window)).await else { return };
            let paths: Vec<std::path::PathBuf> = (0..files.n_items()).filter_map(|at| files.item(at).and_downcast::<gio::File>()).filter_map(|file| file.path()).collect();
            let imported = match backend::import_files(&this.backend, paths).await {
                Ok(imported) => imported,
                Err(error) => {
                    toast(&error, None);
                    return;
                }
            };
            let mut counts = Some(imported.counts.clone());
            if let Some(name) = imported.passphrase.clone() {
                let input = password(&t!("Passphrase"));
                let error = ui::label("", &["error", "small"]);
                error.set_visible(false);
                let (reading, saying, backend) = (input.clone(), error.clone(), this.backend.clone());
                counts = widgets::modal(Dialog {
                    title: t!("Open the backup"),
                    text: Some(t!("{file} is encrypted. Enter the passphrase your notes had when it was made.", file = name)),
                    body: vec![input.upcast(), error.upcast()],
                    actions: vec![
                        Action::plain(&t!("Cancel"), None),
                        Action::primary(&t!("Import"), false, move || {
                            let (reading, saying, backend) = (reading.clone(), saying.clone(), backend.clone());
                            Box::pin(async move {
                                say(&saying, &t!("Opening…"));
                                match backend::import_pending(&backend, reading.text().to_string()).await {
                                    Err(error) => {
                                        say(&saying, &error);
                                        None
                                    }
                                    Ok(counts) => Some(Some(counts)),
                                }
                            })
                        }),
                    ],
                    wide: false,
                })
                .await
                .flatten();
                if counts.is_none() {
                    return;
                }
            }
            this.render();
            if let Some(counts) = counts {
                let parts: Vec<String> = counts.list().into_iter().filter(|(_, n)| *n > 0).map(|(kind, n)| counted(kind, n)).collect();
                toast(&if parts.is_empty() { t!("Nothing was found to import.") } else { t!("Imported {items}.", items = i18n::list(&parts)) }, None);
            }
            // Each one a sentence of the backend's, which says which file it was and why
            for skipped in imported.skipped {
                toast(&skipped, None);
            }
        });
    }

    fn run_backup(&self) {
        self.commit();
        self.spawn(async |this| {
            if this.backend.borrow().store.is_none() {
                toast(&t!("Unlock your notes first."), None);
                return;
            }
            let filter = gtk::FileFilter::new();
            filter.set_name(Some(&t!("Notes backup")));
            filter.add_suffix("amnotes");
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);
            let name = format!("Notes backup {}.amnotes", dates::today());
            let dialog = gtk::FileDialog::builder().title(t!("Back up to a file")).initial_name(&name).filters(&filters).modal(true).build();
            dialog.set_initial_folder(Some(&gio::File::for_path(widgets::special_dir(glib::UserDirectory::Documents))));
            let Ok(file) = dialog.save_future(Some(&this.shell.window)).await else { return };
            let Some(path) = file.path() else { return };
            match backend::backup(&this.backend, path.clone()).await {
                Ok(_) => toast(&t!("Backed up to {path}", path = path.display().to_string()), None),
                Err(error) => toast(&error, None),
            }
        });
    }

    fn run_export(&self) {
        self.commit();
        self.spawn(async |this| {
            if this.backend.borrow().store.is_none() {
                toast(&t!("Unlock your notes first."), None);
                return;
            }
            let dialog = gtk::FileDialog::builder().title(t!("Export to a folder")).modal(true).build();
            let Ok(folder) = dialog.select_folder_future(Some(&this.shell.window)).await else { return };
            let Some(path) = folder.path() else { return };
            match backend::export_markdown(&this.backend, path).await {
                Ok(exported) => toast(&t!(
                    "Exported {notes, plural, one {# note} other {# notes}} and {tasks, plural, one {# task} other {# tasks}} to {path}",
                    notes = exported.notes,
                    tasks = exported.tasks,
                    path = exported.path.display().to_string()
                ), None),
                Err(error) => toast(&error, None),
            }
        });
    }
}
