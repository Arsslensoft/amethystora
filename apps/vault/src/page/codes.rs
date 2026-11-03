//! Backup codes: one service at a time, the next code on request, Copy and Mark used with Undo, a warning at two left, and which
//! authenticator accounts have none. Used ones stay, struck through, with the date. Showing them all, or printing them, asks every time.

use crate::scan;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{adw, gio, glib, gtk, i18n, process, t, ui};
use amethystora_vault_core::codes::{self, Directory};
use amethystora_vault_core::items::KIND_BACKUP_CODES;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use super::{Page, Vault, Width};
use crate::list::{self, ListPage, Row};
use crate::print;
use crate::rpc::Trouble;
use crate::secret;
use crate::widgets::{self, btn, say};

/// The bundled copy of 2fa.directory, trimmed to the services' pages for new codes
use crate::logos::DIRECTORY;
const MAX_TEXT_FILE: u64 = 1024 * 1024;
/// A PDF is read for this long at most
const PDF_PATIENCE: std::time::Duration = std::time::Duration::from_secs(20);

fn row_of(entry: &Value) -> Row {
    let meta = &entry["meta"];
    let title = meta["service"].as_str().map_or_else(|| list::text(entry, "name"), str::to_string);
    let (left, total) = (meta["left"].as_u64().unwrap_or(0), meta["total"].as_u64().unwrap_or(0));
    let tag = (meta["low"] == json!(true)).then(|| (t!("Low"), "warn"));
    Row::new(entry, title, left_words(left, total), tag)
}

/// How many are left, said by the number left
fn left_words(left: u64, total: u64) -> String {
    t!("{left, plural, one {# of {total} left} other {# of {total} left}}", left = left, total = total)
}

pub struct Codes {
    vault: Rc<Vault>,
    list: Rc<ListPage>,
    coverage: gtk::Box,
    directory: Rc<RefCell<Option<Directory>>>,
    me: RefCell<std::rc::Weak<Codes>>,
}

impl Codes {
    pub fn new(vault: &Rc<Vault>) -> Rc<dyn Page> {
        let weak = Rc::downgrade(vault);
        let list = ListPage::new(
            &t!("Search codes"),
            &t!("Save backup codes"),
            move || {
                if let Some(vault) = weak.upgrade() {
                    save_sheet(&vault);
                }
            },
            &t!("No backup codes yet. Save some with the plus above."),
        );
        let coverage = ui::vbox(6, &["coverage"]);
        coverage.set_visible(false);
        list.pane.append(&coverage);
        let page = Rc::new(Codes { vault: vault.clone(), list: list.clone(), coverage, directory: Rc::new(RefCell::new(None)), me: RefCell::new(std::rc::Weak::new()) });
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
        let meta = &row.meta;
        let (left, total) = (meta["left"].as_u64().unwrap_or(0), meta["total"].as_u64().unwrap_or(0));
        let account = meta["account"].as_str().unwrap_or("");
        let sub = if account.is_empty() { left_words(left, total) } else { t!("{account}, {left}", account = widgets::clean(account), left = left_words(left, total)) };
        let body = Vault::detail_body();
        body.append(&self.vault.detail_head(&self.list, row, &sub, &[]));
        if meta["low"] == json!(true) {
            body.append(&widgets::callout(&t!("Only a few codes are left. Make new ones at the service while you can still log in.")));
        }
        body.append(&self.next_card(row));
        body.append(&self.all_card(row, total));
        if meta["has_note"] == json!(true) {
            body.append(&self.note_card(&row.id));
        }
        body.append(&self.vault.approval_card(KIND_BACKUP_CODES, row));
        body.append(&self.vault.sync_card(KIND_BACKUP_CODES, &row.id, &row.approval, false));
        let foot = ui::hbox(8, &[]);
        let (vault, id, name) = (self.vault.clone(), row.id.clone(), row.title.clone());
        foot.append(&btn(&t!("Add more codes"), Some("plus"), &["small"], move || add_to(&vault, id.clone(), name.clone())));
        let (weak, id, found) = (self.me.borrow().clone(), row.id.clone(), row.meta.clone());
        foot.append(&btn(&t!("Edit"), Some("edit"), &["small"], move || {
            if let Some(page) = weak.upgrade() {
                page.edit(&id, &found);
            }
        }));
        let (weak, id, title) = (self.me.borrow().clone(), row.id.clone(), row.title.clone());
        foot.append(&btn(&t!("Remove"), Some("trash"), &["small", "danger"], move || {
            let Some(page) = weak.upgrade() else { return };
            let reload = page.me.borrow().clone();
            page.vault.remove_with_undo(&page.list, KIND_BACKUP_CODES, &id, &title, move || {
                if let Some(page) = reload.upgrade() {
                    page.reload();
                }
            });
        }));
        body.append(&foot);
        self.list.show(&body);
    }

    /// "Next code" shows one, with Copy and Mark used, which Undo takes back. It goes after 30 seconds, as anything secret does.
    fn next_card(&self, row: &Row) -> gtk::Box {
        let card = widgets::card();
        card.append(&widgets::h2(&t!("Next code")));
        let line = ui::hbox(10, &["next-code"]);
        let shown = ui::label(&t!("Hidden until you ask."), &["soft"]);
        shown.set_hexpand(true);
        shown.set_direction(gtk::TextDirection::Ltr);
        // Not selectable: a selection is on the primary selection at once. Copy is the way out.
        line.append(&shown);
        let held: Rc<RefCell<Option<(Zeroizing<String>, u64)>>> = Rc::new(RefCell::new(None));
        let token = Rc::new(Cell::new(0u64));
        let copy = widgets::icon_btn("copy", &t!("Copy the code"), || {});
        let mark = btn(&t!("Mark used"), Some("check"), &["small", "primary"], || {});
        let show = btn(&t!("Show the next code"), Some("eye"), &["small"], || {});
        copy.set_visible(false);
        mark.set_visible(false);
        line.append(&show);
        line.append(&copy);
        line.append(&mark);
        card.append(&line);
        card.append(&widgets::p(&t!("Hides again in 30 seconds. Using a code does not count until you mark it."), &["soft", "small"]));
        let hide: Rc<dyn Fn()> = {
            let (shown, held, copy, mark, show) = (shown.clone(), held.clone(), copy.clone(), mark.clone(), show.clone());
            Rc::new(move || {
                shown.set_label(&t!("Hidden until you ask."));
                shown.remove_css_class("code-shown");
                held.replace(None);
                copy.set_visible(false);
                mark.set_visible(false);
                show.set_visible(true);
            })
        };
        let (vault, id, reading, keeping, tokens, hiding, press, copying, marking) = (self.vault.clone(), row.id.clone(), shown.clone(), held.clone(), token.clone(), hide.clone(), show.clone(), copy.clone(), mark.clone());
        show.connect_clicked(move |_| {
            let (vault, id, shown, held, token, hide, show, copy, mark) = (vault.clone(), id.clone(), reading.clone(), keeping.clone(), tokens.clone(), hiding.clone(), press.clone(), copying.clone(), marking.clone());
            glib::spawn_future_local(async move {
                match vault.call(json!({"op": "codes-next", "id": id})).await {
                    Ok(answer) => {
                        let Some(code) = answer["code"].as_str() else {
                            vault.say(&t!("Every code here has been used."));
                            return;
                        };
                        // Nobody looking: not shown
                        if !vault.active_soon().await {
                            return;
                        }
                        shown.set_label(code);
                        shown.add_css_class("code-shown");
                        held.replace(Some((Zeroizing::new(code.to_string()), answer["index"].as_u64().unwrap_or(0))));
                        show.set_visible(false);
                        copy.set_visible(true);
                        mark.set_visible(true);
                        let hide = hide.clone();
                        token.set(vault.secrets.hold(move || hide()));
                    }
                    Err(Trouble::Refused { wait: 0, problem: None }) => {}
                    Err(trouble) => vault.say(&trouble.words()),
                }
            });
        });
        let (vault, reading) = (self.vault.clone(), held.clone());
        copy.connect_clicked(move |_| {
            if let Some((code, _)) = &*reading.borrow() {
                vault.copy_secret(code.as_bytes());
            }
        });
        let (vault, id, reading, weak, tokens, hiding) = (self.vault.clone(), row.id.clone(), held.clone(), self.me.borrow().clone(), token.clone(), hide.clone());
        mark.connect_clicked(move |_| {
            let Some(index) = reading.borrow().as_ref().map(|(_, index)| *index) else { return };
            let (vault, id, weak, tokens, hiding) = (vault.clone(), id.clone(), weak.clone(), tokens.clone(), hiding.clone());
            glib::spawn_future_local(async move {
                match vault.call(json!({"op": "codes-used", "id": id, "index": index, "used": true})).await {
                    Ok(_) => {
                        vault.secrets.release(tokens.replace(0));
                        hiding();
                        let toast = adw::Toast::builder().title(t!("Marked as used.")).use_markup(false).button_label(t!("Undo")).timeout(5).build();
                        let (undoing, undo_id, undo_weak) = (vault.clone(), id.clone(), weak.clone());
                        toast.connect_button_clicked(move |_| {
                            let (vault, id, weak) = (undoing.clone(), undo_id.clone(), undo_weak.clone());
                            glib::spawn_future_local(async move {
                                let _ = vault.call(json!({"op": "codes-used", "id": id, "index": index, "used": false})).await;
                                if let Some(page) = weak.upgrade() {
                                    page.reload();
                                }
                            });
                        });
                        vault.add_toast(toast, None);
                        if let Some(page) = weak.upgrade() {
                            page.reload();
                        }
                    }
                    Err(Trouble::Refused { wait: 0, problem: None }) => {}
                    Err(trouble) => vault.say(&trouble.words()),
                }
            });
        });
        card
    }

    /// Every code, only after asking each time, used ones struck through with the date
    fn all_card(&self, row: &Row, total: u64) -> gtk::Box {
        let card = widgets::card();
        let head = ui::hbox(8, &[]);
        let title = widgets::h2(&t!("All {total}", total = total));
        title.set_hexpand(true);
        head.append(&title);
        let show = btn(&t!("Show all"), Some("eye"), &["small"], || {});
        let print_button = btn(&t!("Print"), Some("print"), &["small"], || {});
        print_button.set_visible(false);
        head.append(&print_button);
        head.append(&show);
        card.append(&head);
        let grid = gtk::Grid::builder().column_homogeneous(true).column_spacing(6).row_spacing(6).build();
        grid.set_visible(false);
        grid.set_direction(gtk::TextDirection::Ltr);
        card.append(&grid);
        let held: Rc<RefCell<Zeroizing<String>>> = Rc::new(RefCell::new(Zeroizing::new(String::new())));
        let (vault, id, name, press, printing, keeping) = (self.vault.clone(), row.id.clone(), row.title.clone(), show.clone(), print_button.clone(), held.clone());
        let filling = grid.clone();
        show.connect_clicked(move |_| {
            let (vault, id, grid, press, printing, keeping) = (vault.clone(), id.clone(), filling.clone(), press.clone(), printing.clone(), keeping.clone());
            glib::spawn_future_local(async move {
                let answer = match vault.call(json!({"op": "codes-all", "id": id})).await {
                    Ok(answer) => answer,
                    Err(Trouble::Refused { wait: 0, problem: None }) => return,
                    Err(trouble) => {
                        vault.say(&trouble.words());
                        return;
                    }
                };
                if !vault.active_soon().await {
                    return;
                }
                let codes = answer["codes"].as_array().map_or(&[][..], Vec::as_slice);
                let half = codes.len().div_ceil(2);
                let mut text = Zeroizing::new(String::new());
                for (at, code) in codes.iter().enumerate() {
                    let line = ui::hbox(10, &["code-row"]);
                    let words = code["code"].as_str().unwrap_or("");
                    let used = code["used"] == json!(true);
                    line.append(&ui::line(&i18n::number((at + 1) as f64), &["faint", "tiny"]));
                    let shown = widgets::data(words, &["mono"]);
                    shown.set_hexpand(true);
                    shown.set_xalign(0.0);
                    line.append(&shown);
                    if used {
                        line.add_css_class("used");
                        // Struck through in the text itself: the stylesheet has no such line
                        let attributes = amethystora_ui::pango::AttrList::new();
                        attributes.insert(amethystora_ui::pango::AttrInt::new_strikethrough(true));
                        shown.set_attributes(Some(&attributes));
                        if let Some(when) = code["used_at"].as_i64() {
                            line.append(&ui::line(&i18n::date(when as f64 / 1000.0, i18n::DateStyle::DayMonthShort), &["faint", "tiny"]));
                        }
                    }
                    grid.attach(&line, (at >= half) as i32, if at >= half { at - half } else { at } as i32, 1, 1);
                    // What is printed is numbered the language's own way, and says which are used
                    text.push_str(&i18n::number((at + 1) as f64));
                    if used {
                        text.push(' ');
                        text.push_str(&t!("(used)"));
                    }
                    text.push(' ');
                    text.push_str(words);
                    text.push('\n');
                }
                keeping.replace(text);
                grid.set_visible(true);
                press.set_sensitive(false);
                printing.set_visible(true);
                let (hide_grid, hide_press, hide_print, hide_keep) = (grid.clone(), press.clone(), printing.clone(), keeping.clone());
                vault.secrets.hold(move || {
                    while let Some(child) = hide_grid.first_child() {
                        hide_grid.remove(&child);
                    }
                    hide_grid.set_visible(false);
                    hide_press.set_sensitive(true);
                    hide_print.set_visible(false);
                    hide_keep.replace(Zeroizing::new(String::new()));
                });
            });
        });
        let reading = held.clone();
        print_button.connect_clicked(move |_| {
            print::text(&name, reading.borrow().as_str());
        });
        card
    }

    fn note_card(&self, id: &str) -> gtk::Box {
        let card = widgets::card();
        card.append(&widgets::h2(&t!("Note")));
        let shown = ui::label(&t!("Hidden until you ask."), &["soft"]);
        card.append(&shown);
        let (vault, id) = (self.vault.clone(), id.to_string());
        let show = btn(&t!("Show the note"), Some("eye"), &["small"], move || {
            let (vault, id, shown) = (vault.clone(), id.clone(), shown.clone());
            glib::spawn_future_local(async move {
                match vault.call(json!({"op": "codes-note", "id": id})).await {
                    Ok(answer) => {
                        if !vault.active_soon().await {
                            return;
                        }
                        shown.set_label(&widgets::clean(answer["note"].as_str().unwrap_or("")));
                        let hiding = shown.clone();
                        vault.secrets.hold(move || hiding.set_label(&t!("Hidden until you ask.")));
                    }
                    Err(Trouble::Refused { wait: 0, problem: None }) => {}
                    Err(trouble) => vault.say(&trouble.words()),
                }
            });
        });
        show.set_halign(gtk::Align::Start);
        card.append(&show);
        card
    }

    /// Its service, account and note. A name is what a prompt shows, so changing one asks: it is sent only when it was changed, and
    /// a note only when it was read and is not what it was.
    fn edit(&self, id: &str, found: &Value) {
        let (service_was, account_was) = (found["service"].as_str().unwrap_or("").to_string(), found["account"].as_str().unwrap_or("").to_string());
        let has_note = found["has_note"] == json!(true);
        let sheet = widgets::sheet(&t!("Edit the backup codes"), 480);
        let (service, account) = (widgets::entry(&service_was, ""), widgets::entry(&account_was, ""));
        sheet.body.append(&widgets::field(&t!("Service"), &service, &[]));
        sheet.body.append(&widgets::field(&t!("Account, if any"), &account, &[]));
        let note = gtk::TextView::builder().wrap_mode(gtk::WrapMode::WordChar).accepts_tab(false).css_classes(["note-input"]).build();
        sheet.body.append(&widgets::field(&t!("Note"), &note, &[]));
        let error = widgets::problem();
        let original: Rc<RefCell<Option<Zeroizing<String>>>> = Rc::new(RefCell::new(None));
        if has_note {
            note.set_editable(false);
            let (vault, reading, asked, kept, failing) = (self.vault.clone(), note.clone(), id.to_string(), original.clone(), error.clone());
            glib::spawn_future_local(async move {
                match vault.call(json!({"op": "codes-note", "id": asked})).await {
                    Ok(answer) => {
                        let text = Zeroizing::new(answer["note"].as_str().unwrap_or("").to_string());
                        reading.buffer().set_text(&text);
                        reading.set_editable(true);
                        kept.replace(Some(text));
                    }
                    Err(_) => say(&failing, &t!("The note could not be read, so it is left as it is.")),
                }
            });
        }
        sheet.body.append(&error);
        let dialog = sheet.dialog.clone();
        let cancel = btn(&t!("Cancel"), None, &[], move || {
            dialog.close();
        });
        let (weak, dialog, id) = (self.me.borrow().clone(), sheet.dialog.clone(), id.to_string());
        let save = btn(&t!("Save"), None, &["primary"], move || {
            let Some(page) = weak.upgrade() else { return };
            let buffer = note.buffer();
            let words = Zeroizing::new(buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string());
            let mut request = json!({"op": "codes-update", "id": id});
            if service.text().as_str() != service_was {
                request["service"] = json!(service.text().as_str());
            }
            if account.text().as_str() != account_was {
                request["account"] = json!(account.text().as_str());
            }
            match &*original.borrow() {
                Some(before) if **before != *words => request["note"] = if words.is_empty() { Value::Null } else { json!(words.as_str()) },
                None if !has_note && !words.is_empty() => request["note"] = json!(words.as_str()),
                _ => {}
            }
            let (dialog, error) = (dialog.clone(), error.clone());
            glib::spawn_future_local(async move {
                match page.vault.call(request).await {
                    Ok(_) => {
                        dialog.close();
                        page.reload();
                    }
                    Err(Trouble::Refused { wait: 0, problem: None }) => {}
                    Err(trouble) => say(&error, &trouble.words()),
                }
            });
        });
        sheet.pin(&widgets::actions(&[cancel.upcast(), save.upcast()]));
        sheet.dialog.present(Some(&self.vault.shell.window));
    }

    /// Which authenticator accounts have no backup codes, from the list: nothing is opened for it. A link to each service's page for new
    /// codes, where the bundled directory knows one, and only ever opened by a click.
    fn draw_coverage(&self, answer: &Value) {
        let coverage = &self.coverage;
        ui::clear(coverage);
        let (covered, total) = (answer["covered"].as_u64().unwrap_or(0), answer["total"].as_u64().unwrap_or(0));
        let missing = answer["missing"].as_array().cloned().unwrap_or_default();
        coverage.set_visible(total > 0);
        if total == 0 {
            return;
        }
        coverage.append(&ui::line(&t!("{covered} of {total} accounts have backup codes", covered = covered, total = total), &["coverage-title"]));
        let directory = self.directory.borrow();
        for account in missing.iter().take(6) {
            let name = account["name"].as_str().unwrap_or("");
            let service = account["service"].as_str().unwrap_or(name);
            let line = ui::hbox(6, &[]);
            let words = widgets::data(name, &["soft", "small"]);
            words.set_hexpand(true);
            words.set_xalign(0.0);
            line.append(&words);
            let page = directory.as_ref().and_then(|directory| directory.find(service)).and_then(|row| row.docs.clone());
            if let Some(page) = page.filter(|page| page.starts_with("https://")) {
                line.append(&widgets::link(&t!("Get new codes"), move || crate::scan::open_uri(&page)));
            }
            coverage.append(&line);
        }
        if missing.len() > 6 {
            coverage.append(&ui::line(&t!("and {count} more", count = missing.len() - 6), &["soft", "small"]));
        }
    }
}

/// Numbering, bullets and blank lines stripped off, off the window's thread for a file: a PDF through `pdftotext`, on its standard output,
/// which is given a time and then given up on (a big PDF takes a long time, and the program is the one that decides how long)
async fn file_text(path: std::path::PathBuf) -> Option<(Zeroizing<String>, bool)> {
    let is_pdf = path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"));
    if is_pdf {
        let path = path.to_string_lossy().into_owned();
        let options = process::Options { timeout: PDF_PATIENCE, ..process::Options::default() };
        let output = process::run_with("pdftotext", &["-layout", "-q", "-l", "30", &path, "-"], options).await;
        return output.ok().then(|| (Zeroizing::new(output.stdout), true));
    }
    gio::spawn_blocking(move || {
        use std::io::Read;
        let mut bytes = Zeroizing::new(Vec::new());
        std::fs::File::open(&path).ok()?.take(MAX_TEXT_FILE).read_to_end(&mut bytes).ok()?;
        Some((Zeroizing::new(String::from_utf8_lossy(&bytes).into_owned()), false))
    })
    .await
    .ok()
    .flatten()
}

fn save_sheet(vault: &Rc<Vault>) {
    codes_sheet(vault, None);
}

fn add_to(vault: &Rc<Vault>, id: String, name: String) {
    codes_sheet(vault, Some((id, name)));
}

/// Paste or pick a file, see how many, and say yes
fn codes_sheet(vault: &Rc<Vault>, to: Option<(String, String)>) {
    let sheet = widgets::sheet(&if to.is_some() { t!("Add more codes") } else { t!("Save backup codes") }, 540);
    let (service, account) = (widgets::entry(to.as_ref().map_or("", |(_, name)| name.as_str()), ""), widgets::entry("", ""));
    // New codes go to the vault chosen; more codes go where the set is
    let target = vault.target();
    if to.is_none() {
        sheet.body.append(&widgets::field(&t!("Service"), &service, &[]));
        sheet.body.append(&widgets::field(&t!("Account, if any"), &account, &[]));
        if let Some(field) = target.field() {
            sheet.body.append(&field);
        }
    }
    let text = gtk::TextView::builder().wrap_mode(gtk::WrapMode::WordChar).accepts_tab(false).monospace(true).css_classes(["note-input", "tall"]).build();
    sheet.body.append(&widgets::field(&t!("The codes, pasted"), &text, &[]));
    let count = ui::line("", &["soft"]);
    let parsed: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let (reading, keeping, saying) = (text.clone(), parsed.clone(), count.clone());
    text.buffer().connect_changed(move |buffer| {
        let _ = &reading;
        let words = Zeroizing::new(buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string());
        let found = codes::parse_codes(&words);
        saying.set_label(&found_words(&found));
        keeping.replace(found_strings(&found));
    });
    let (vault_pick, filling, keeping, saying) = (vault.clone(), text.clone(), parsed.clone(), count.clone());
    let pick = btn(&t!("Pick a text file or a PDF"), Some("picture"), &["small"], move || {
        let (vault, filling, keeping, saying) = (vault_pick.clone(), filling.clone(), keeping.clone(), saying.clone());
        glib::spawn_future_local(async move {
            let Some(path) = scan::choose_file(&t!("Pick a text file or a PDF"), None).await else { return };
            // A PDF can take a while, and the sheet says so
            saying.set_label(&t!("Reading the file…"));
            let Some((words, from_document)) = file_text(path).await else {
                saying.set_label("");
                vault.say(&t!("That file could not be read in time, or is not a text file or a PDF."));
                return;
            };
            if from_document {
                let found = codes::parse_document_codes(&words);
                saying.set_label(&found_words(&found));
                let found = found_strings(&found);
                filling.buffer().set_text(&found.join("\n"));
                keeping.replace(found);
            } else {
                filling.buffer().set_text(&words);
            }
        });
    });
    pick.set_halign(gtk::Align::Start);
    sheet.body.append(&pick);
    sheet.body.append(&count);
    let error = widgets::problem();
    sheet.body.append(&error);
    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    let (vault_save, dialog, failing) = (vault.clone(), sheet.dialog.clone(), error.clone());
    let save = btn(&t!("Save them"), None, &["primary"], move || {
        let found = parsed.borrow().clone();
        if found.is_empty() {
            say(&failing, &t!("No codes found yet."));
            return;
        }
        let mut request = json!({"op": "codes-add", "codes": found});
        match &to {
            Some((id, name)) => {
                request["id"] = json!(id);
                request["service"] = json!(name);
            }
            None => {
                let name = service.text().trim().to_string();
                if name.is_empty() {
                    say(&failing, &t!("Say which service these are for."));
                    return;
                }
                request["service"] = json!(name);
                if !account.text().is_empty() {
                    request["account"] = json!(account.text().as_str());
                }
                target.name(&mut request);
            }
        }
        let (vault, dialog, failing) = (vault_save.clone(), dialog.clone(), failing.clone());
        glib::spawn_future_local(async move {
            match vault.call(request).await {
                Ok(answer) => {
                    dialog.close();
                    vault.say(&t!("{count, plural, one {# code saved.} other {# codes saved.}}", count = answer["added"].as_u64().unwrap_or(0)));
                    vault.count_items();
                    vault.reload_and_select(answer["id"].as_str().map(str::to_string));
                }
                Err(trouble) => say(&failing, &trouble.words()),
            }
        });
    });
    sheet.pin(&widgets::actions(&[cancel.upcast(), save.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

impl Page for Codes {
    fn widget(&self) -> gtk::Widget {
        self.list.root.clone().upcast()
    }

    fn reload(&self) {
        let (vault, weak) = (self.vault.clone(), self.me.borrow().clone());
        let directory = self.directory.clone();
        vault.clone().spawn(async move |_| {
            crate::logos::refresh().await;
            let Ok(answer) = vault.call(json!({"op": "list", "kind": KIND_BACKUP_CODES})).await else { return };
            let Some(page) = weak.upgrade() else { return };
            let mut rows: Vec<Row> = answer["items"].as_array().into_iter().flatten().map(row_of).collect();
            super::sort_rows(&mut rows);
            page.list.set_rows(rows);
            if vault.width.get() == Width::Full {
                page.list.select_first();
            }
            crate::logos::fill(&vault, &page.list).await;
            // The bundled list, the first time it is needed and off the window's thread
            if directory.borrow().is_none() {
                let read = gio::spawn_blocking(|| std::fs::read(DIRECTORY).ok().and_then(|bytes| Directory::parse(&bytes))).await.ok().flatten();
                directory.replace(read);
            }
            if let Ok(answer) = vault.call(json!({"op": "codes-coverage"})).await {
                page.draw_coverage(&answer);
            }
        });
    }

    fn width(&self, width: Width) {
        self.list.set_narrow(width != Width::Full);
    }

    fn add(&self) {
        save_sheet(&self.vault);
    }

    fn find(&self) {
        self.list.search.grab_focus();
    }

    fn select(&self, id: &str) {
        self.list.select_id(id);
    }
}

#[allow(unused_imports)]
use secret as _;

/// What was read, as text the sheet can hold while it is open
fn found_strings(found: &codes::ParsedCodes) -> Vec<String> {
    found.codes.iter().map(|code| code.as_str().to_string()).collect()
}

/// What the sheet says of a reading: how many, and when the text was too long or some codes were left out, that
fn found_words(found: &codes::ParsedCodes) -> String {
    if found.text_too_large {
        return t!("That text is too long to be a list of codes.");
    }
    let count = t!("{count, plural, =0 {No codes found yet} one {# code found} other {# codes found}}", count = found.codes.len());
    if found.dropped == 0 {
        return count;
    }
    t!("{found} {left, plural, one {# was too long or past the most one text may hold, and is left out.} other {# were too long or past the most one text may hold, and are left out.}}", found = count, left = found.dropped)
}
