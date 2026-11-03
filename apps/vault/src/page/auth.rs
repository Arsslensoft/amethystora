//! The authenticator: one account at a time, its code in two halves and the seconds left. The secret stays in the service, which hands
//! back only the code. The one timer here runs while this page is shown, the window has the focus and a code is showing, and it changes
//! the seconds, not the page.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{glib, gtk, i18n, process, t, ui};
use amethystora_vault_core::items::KIND_ACCOUNT;
use amethystora_vault_core::otp;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use super::{add, export, Page, Vault, Width};
use crate::list::{self, ListPage, Row};
use crate::qr;
use crate::rpc::Trouble;
use crate::widgets::{self, btn, say};

/// What stands in for a code that is not showing
fn dots(digits: usize) -> String {
    otp::group_code(&"•".repeat(digits))
}

/// The list's line for an account
pub fn row_of(entry: &Value) -> Row {
    let meta = &entry["meta"];
    let (issuer, label) = (meta["issuer"].as_str().unwrap_or(""), meta["label"].as_str().unwrap_or(""));
    let name = list::text(entry, "name");
    let title = if !issuer.is_empty() {
        issuer.to_string()
    } else if !label.is_empty() {
        label.to_string()
    } else {
        name
    };
    let sub = if issuer.is_empty() { String::new() } else { label.to_string() };
    Row::new(entry, title, sub, None)
}

fn now_ms() -> i64 {
    (i18n::now() * 1000.0) as i64
}

/// What is on the screen of the account chosen
struct Shown {
    id: String,
    /// `totp`, `hotp` or `steam`
    kind: String,
    digits: usize,
    period: i64,
    code: gtk::Label,
    left: gtk::Label,
    bar: gtk::LevelBar,
    next: gtk::Label,
    timing: gtk::Box,
    show: gtk::Button,
    /// Set to ask every time: a click shows it, and nothing else does or asks again
    ask: bool,
    /// The seconds last drawn, so that a new period can be told from them
    last_left: Cell<i64>,
    showing: Cell<bool>,
    fetched_next: Cell<bool>,
    /// What `secrets` holds the code on the screen by, so that it goes after 30 seconds like any secret
    hold: Cell<u64>,
}

pub struct Authenticator {
    vault: Rc<Vault>,
    list: Rc<ListPage>,
    shown: RefCell<Option<Rc<Shown>>>,
    /// The account whose code was on the screen when the window lost the focus, if it may come back by itself
    resume: RefCell<Option<String>>,
    tick: RefCell<Option<glib::SourceId>>,
    visible: Cell<bool>,
    clock: gtk::Box,
    since_export: gtk::Label,
    me: RefCell<std::rc::Weak<Authenticator>>,
}

impl Authenticator {
    pub fn new(vault: &Rc<Vault>) -> Rc<dyn Page> {
        let weak = Rc::downgrade(vault);
        let list = ListPage::new(
            &t!("Search codes"),
            &t!("Add an authenticator code"),
            move || {
                if let Some(vault) = weak.upgrade() {
                    add::show(&vault);
                }
            },
            &t!("No authenticator codes yet. Add one with the plus above."),
        );
        // The clock, said once a page is shown, and how many accounts no export holds yet
        let clock = widgets::callout(&t!("This computer's clock is not synchronized, so these codes may be wrong."));
        clock.set_visible(false);
        list.above.append(&clock);
        let foot = ui::vbox(4, &["list-foot"]);
        let since_export = ui::label("", &["soft", "small"]);
        since_export.set_visible(false);
        foot.append(&since_export);
        let weak = Rc::downgrade(vault);
        foot.append(&widgets::link(&t!("Move or save your codes"), move || {
            if let Some(vault) = weak.upgrade() {
                export::show(&vault);
            }
        }));
        list.pane.append(&foot);
        let page = Rc::new(Authenticator {
            vault: vault.clone(),
            list: list.clone(),
            shown: RefCell::new(None),
            resume: RefCell::new(None),
            tick: RefCell::new(None),
            visible: Cell::new(false),
            clock,
            since_export,
            me: RefCell::new(std::rc::Weak::new()),
        });
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
        self.stop_tick();
        self.shown.replace(None);
        self.vault.secrets.hide_all();
        let Some(row) = row else {
            self.list.clear();
            return;
        };
        let meta = &row.meta;
        let kind = meta["type"].as_str().unwrap_or("totp").to_string();
        let digits = if kind == "steam" { 5 } else { meta["digits"].as_u64().unwrap_or(6) as usize };
        let period = meta["period"].as_i64().unwrap_or(30).max(1);
        let body = Vault::detail_body();
        body.append(&self.vault.detail_head(&self.list, row, &row.sub, &[]));

        let card = widgets::card();
        let top = ui::hbox(16, &[]);
        let code = ui::line(&dots(digits), &["mono", "code-big"]);
        code.set_hexpand(true);
        code.set_direction(gtk::TextDirection::Ltr);
        // Not selectable: a selection is on the primary selection at once. Copy is the way out.
        top.append(&code);
        let timing = ui::vbox(2, &[]);
        timing.set_valign(gtk::Align::Center);
        let left = ui::line("", &["seconds"]);
        left.set_xalign(0.5);
        timing.append(&left);
        let seconds_label = ui::line(&t!("seconds left"), &["soft", "tiny"]);
        seconds_label.set_xalign(0.5);
        timing.append(&seconds_label);
        timing.set_visible(false);
        top.append(&timing);
        card.append(&top);
        let bar = gtk::LevelBar::builder().min_value(0.0).max_value(1.0).value(0.0).css_classes(["countdown"]).build();
        bar.set_visible(kind != "hotp");
        bar.update_property(&[gtk::accessible::Property::Label(&t!("Seconds left"))]);
        card.append(&bar);
        let next = ui::line("", &["mono", "soft", "small"]);
        next.set_direction(gtk::TextDirection::Ltr);
        next.set_visible(false);
        card.append(&next);

        let actions = ui::wrap(8, 8, &[]);
        let ask = row.approval == "ask";
        // A counter shows its code as it stands, and has a button of its own to move it on: looking must not use one up
        let show = btn(&t!("Show the code"), Some("eye"), &["primary"], || {});
        actions.append(&show);
        let step = (kind == "hotp").then(|| btn(&t!("Next code"), Some("refresh"), &[], || {}));
        if let Some(step) = &step {
            actions.append(step);
        }
        let copy_id = row.id.clone();
        let vault = self.vault.clone();
        let weak = self.me.borrow().clone();
        let is_hotp = kind == "hotp";
        actions.append(&btn(&t!("Copy the code"), Some("copy"), &[], move || {
            let (vault, id, weak) = (vault.clone(), copy_id.clone(), weak.clone());
            glib::spawn_future_local(async move {
                // A counter moves only when asked for: the code on the screen, if there is one, is what is meant
                if is_hotp {
                    let current = weak.upgrade().and_then(|page| page.shown.borrow().as_ref().filter(|shown| shown.showing.get()).map(|shown| shown.code.text().to_string()));
                    if let Some(current) = current {
                        vault.copy_secret(current.replace(' ', "").as_bytes());
                    } else {
                        vault.say(&t!("Ask for the next code first."));
                    }
                    return;
                }
                match vault.call(json!({"op": "auth-code", "id": id})).await {
                    Ok(answer) => {
                        let code = Zeroizing::new(answer["code"].as_str().unwrap_or("").to_string());
                        vault.copy_secret(code.as_bytes());
                        if !ask {
                            vault.approved.borrow_mut().insert(id);
                        }
                    }
                    Err(Trouble::Refused { wait: 0, problem: None }) => {}
                    Err(trouble) => vault.say(&trouble.words()),
                }
            });
        }));
        let (vault, qr_id, qr_title) = (self.vault.clone(), row.id.clone(), row.title.clone());
        actions.append(&btn(&t!("Show QR code"), Some("qr"), &[], move || show_qr(&vault, qr_id.clone(), qr_title.clone())));
        card.append(&actions);
        body.append(&card);

        let about = widgets::card();
        about.append(&widgets::h2(&t!("About this code")));
        let pairs = widgets::Pairs::new();
        let digits_shown = meta["digits"].as_u64().unwrap_or(6);
        let words = match kind.as_str() {
            "hotp" => t!("Counter-based, {digits} digits", digits = digits_shown),
            "steam" => t!("Steam Guard, 5 characters"),
            _ => t!("Time-based, {digits} digits, every {period, plural, one {# second} other {# seconds}}", digits = digits_shown, period = period),
        };
        pairs.add(&t!("Kind"), &words, false);
        pairs.add(&t!("Algorithm"), &meta["algorithm"].as_str().unwrap_or("SHA1").replace("SHA", "SHA-"), false);
        if let Some(added) = meta["added_at"].as_i64().filter(|added| *added > 0) {
            pairs.add(&t!("Added"), &list::long_date(added), false);
        }
        pairs.add(&t!("Sync"), &t!("Not connected"), false);
        let tags: Vec<&str> = meta["tags"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        if !tags.is_empty() {
            pairs.add(&t!("Tags"), &i18n::list(&tags.iter().map(|tag| tag.to_string()).collect::<Vec<_>>()), false);
        }
        about.append(&pairs.grid);
        body.append(&about);
        if meta["has_note"] == json!(true) {
            body.append(&self.note_card(&row.id));
        }
        body.append(&self.vault.approval_card(KIND_ACCOUNT, row));
        let foot = ui::hbox(8, &[]);
        let (weak, id) = (self.me.borrow().clone(), row.id.clone());
        foot.append(&btn(&t!("Edit"), Some("edit"), &["small"], move || {
            if let Some(page) = weak.upgrade() {
                page.edit(&id);
            }
        }));
        let (weak, id, title) = (self.me.borrow().clone(), row.id.clone(), row.title.clone());
        foot.append(&btn(&t!("Remove"), Some("trash"), &["small", "danger"], move || {
            let Some(page) = weak.upgrade() else { return };
            let reload = page.me.borrow().clone();
            page.vault.remove_with_undo(&page.list, KIND_ACCOUNT, &id, &title, move || {
                if let Some(page) = reload.upgrade() {
                    page.reload();
                }
            });
        }));
        body.append(&foot);
        self.list.show(&body);

        let shown = Rc::new(Shown {
            id: row.id.clone(),
            kind,
            digits,
            period,
            code,
            left,
            bar,
            next,
            timing,
            show: show.clone(),
            ask,
            last_left: Cell::new(-1),
            showing: Cell::new(false),
            fetched_next: Cell::new(false),
            hold: Cell::new(0),
        });
        self.shown.replace(Some(shown.clone()));
        let (weak, clicked) = (self.me.borrow().clone(), shown.clone());
        show.connect_clicked(move |_| {
            if let Some(page) = weak.upgrade() {
                page.fetch(&clicked, false, true);
            }
        });
        if let Some(step) = &step {
            let (weak, clicked) = (self.me.borrow().clone(), shown.clone());
            step.connect_clicked(move |_| {
                if let Some(page) = weak.upgrade() {
                    // A counter moves only by being asked, here
                    page.fetch(&clicked, true, true);
                }
            });
        }
        // What was on the screen when the window went away comes back, if it may. An account that asks every time never does: it waits
        // for a click. Nor does a counter's, which a look would not use up but nothing needs to show unasked.
        let back = self.resume.borrow().as_deref() == Some(row.id.as_str());
        let unasked = !ask && shown.kind != "hotp" && (row.approval == "never" || back || self.vault.approved.borrow().contains(&row.id));
        if back {
            self.resume.take();
        }
        if unasked && self.vault.shell.window.is_active() {
            self.fetch(&shown, false, true);
        }
    }

    /// The code from the service, and for a code on a clock, its seconds
    /// `fresh`: the person asked, which starts the 30 seconds; a new period fetching its own code does not
    fn fetch(&self, shown: &Rc<Shown>, step: bool, fresh: bool) {
        let (vault, weak, shown) = (self.vault.clone(), self.me.borrow().clone(), shown.clone());
        vault.clone().spawn(async move |_| {
            let mut request = json!({"op": "auth-code", "id": shown.id});
            if step {
                request["step"] = json!(true);
            }
            let answer = match vault.call(request).await {
                Ok(answer) => answer,
                Err(Trouble::Refused { wait: 0, problem: None }) => return,
                Err(trouble) => {
                    vault.say(&trouble.words());
                    return;
                }
            };
            // A prompt takes the focus while it is answered, and gives it back a moment after: an answer waits for that, and one that
            // comes to a window nobody is looking at is not shown
            if !vault.active_soon().await {
                return;
            }
            let Some(page) = weak.upgrade() else { return };
            // Moved on to another account, or another page, while it was asking
            if !page.visible.get() || !page.shown.borrow().as_ref().is_some_and(|current| Rc::ptr_eq(current, &shown)) {
                return;
            }
            // A code for the next period is for the one on the screen: with none showing, nobody asked
            if !fresh && !shown.showing.get() {
                return;
            }
            if !shown.ask {
                vault.approved.borrow_mut().insert(shown.id.clone());
            }
            shown.code.set_label(&otp::group_code(answer["code"].as_str().unwrap_or("")));
            shown.showing.set(true);
            if fresh {
                vault.secrets.release(shown.hold.replace(0));
                let hiding = weak.clone();
                shown.hold.set(vault.secrets.hold(move || {
                    if let Some(page) = hiding.upgrade() {
                        page.hide_code();
                    }
                }));
            }
            shown.show.set_visible(false);
            shown.next.set_visible(false);
            shown.fetched_next.set(false);
            if shown.kind != "hotp" {
                shown.timing.set_visible(true);
                shown.last_left.set(-1);
                page.draw_seconds(&shown);
                page.start_tick();
            }
        });
    }

    /// The one label and the bar that change each second, and the next code when under 5 seconds are left
    fn draw_seconds(&self, shown: &Rc<Shown>) {
        let left = otp::seconds_left(shown.period, now_ms());
        if shown.last_left.get() >= 0 && left > shown.last_left.get() {
            // A new period: its code. The rest stays as it is. One that asks every time is not asked for again unasked: it ends here.
            shown.last_left.set(left);
            if shown.ask {
                self.hide_code();
            } else {
                self.fetch(shown, false, false);
            }
            return;
        }
        shown.last_left.set(left);
        shown.left.set_label(&i18n::number(left as f64));
        shown.bar.set_value(left as f64 / shown.period as f64);
        if left < 5 && !shown.ask && !shown.fetched_next.replace(true) {
            let (vault, shown) = (self.vault.clone(), shown.clone());
            vault.clone().spawn(async move |_| {
                if let Ok(answer) = vault.call(json!({"op": "auth-code", "id": shown.id})).await {
                    if let Some(next) = answer["next"].as_str() {
                        shown.next.set_label(&t!("Next: {code}", code = otp::group_code(next)));
                        shown.next.set_visible(true);
                    }
                }
            });
        }
    }

    /// Runs only while this page is shown, the window has the focus and a code on a clock is showing: every second on the second
    fn start_tick(&self) {
        let code_on_clock = self.shown.borrow().as_ref().is_some_and(|shown| shown.showing.get() && shown.kind != "hotp");
        if self.tick.borrow().is_some() || !self.visible.get() || !self.vault.shell.window.is_active() || !code_on_clock {
            return;
        }
        let weak = self.me.borrow().clone();
        let to_second = 1000 - (now_ms() % 1000) as u64;
        let source = glib::timeout_add_local_once(Duration::from_millis(to_second), move || {
            let Some(page) = weak.upgrade() else { return };
            let weak = page.me.borrow().clone();
            let each = glib::timeout_add_local(Duration::from_secs(1), move || {
                let Some(page) = weak.upgrade() else { return glib::ControlFlow::Break };
                let shown = page.shown.borrow().clone();
                if let Some(shown) = shown {
                    page.draw_seconds(&shown);
                }
                glib::ControlFlow::Continue
            });
            page.tick.replace(Some(each));
            let shown = page.shown.borrow().clone();
            if let Some(shown) = shown {
                page.draw_seconds(&shown);
            }
        });
        self.tick.replace(Some(source));
    }

    fn stop_tick(&self) {
        if let Some(source) = self.tick.take() {
            source.remove();
        }
    }

    /// The code off the screen, and the timer stopped, until the window is back
    fn hide_code(&self) {
        self.stop_tick();
        if let Some(shown) = &*self.shown.borrow() {
            self.vault.secrets.release(shown.hold.replace(0));
            shown.code.set_label(&dots(shown.digits));
            shown.showing.set(false);
            shown.timing.set_visible(false);
            shown.next.set_visible(false);
            shown.bar.set_value(0.0);
            shown.last_left.set(-1);
            shown.show.set_visible(true);
        }
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
                match vault.call(json!({"op": "auth-note", "id": id})).await {
                    Ok(answer) => {
                        if !vault.active_soon().await {
                            return;
                        }
                        shown.set_label(answer["note"].as_str().unwrap_or(""));
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

    /// Its names, note, tags and place in the list
    fn edit(&self, id: &str) {
        let found = self.list.row_meta(id).unwrap_or(Value::Null);
        let issuer = found["issuer"].as_str().unwrap_or("");
        let label = found["label"].as_str().unwrap_or("");
        let tags = found["tags"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(", ");
        let has_note = found["has_note"] == json!(true);
        let sheet = widgets::sheet(&t!("Edit the account"), 480);
        let (service, account, tag_field) = (widgets::entry(issuer, ""), widgets::entry(label, ""), widgets::entry(&tags, ""));
        sheet.body.append(&widgets::field(&t!("Service"), &service, &[]));
        sheet.body.append(&widgets::field(&t!("Account"), &account, &[]));
        sheet.body.append(&widgets::field(&t!("Tags, with commas between"), &tag_field, &[]));
        let note = gtk::TextView::builder().wrap_mode(gtk::WrapMode::WordChar).accepts_tab(false).css_classes(["note-input"]).build();
        sheet.body.append(&widgets::field(&t!("Note"), &note, &[]));
        let error = widgets::problem();
        // The note as it was read: it is sent only if it was read and is not that any more. A note that was refused or has not come yet
        // is left as it is, and cannot be written over.
        let original: Rc<RefCell<Option<Zeroizing<String>>>> = Rc::new(RefCell::new(None));
        if has_note {
            note.set_editable(false);
            let (vault, reading, asked, kept, failing) = (self.vault.clone(), note.clone(), id.to_string(), original.clone(), error.clone());
            glib::spawn_future_local(async move {
                match vault.call(json!({"op": "auth-note", "id": asked})).await {
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
        let pin_box = widgets::check(&t!("Keep at the top of the list"), found["pinned"] == json!(true), |_| {});
        sheet.body.append(&pin_box);
        let no_codes_box = widgets::check(&t!("This service offers no backup codes"), found["no_backup_codes"] == json!(true), |_| {});
        sheet.body.append(&no_codes_box);
        let order = ui::hbox(8, &[]);
        let (weak, here) = (self.me.borrow().clone(), id.to_string());
        order.append(&btn(&t!("Move up"), Some("up"), &["small"], move || {
            if let Some(page) = weak.upgrade() {
                page.move_by(&here, -1);
            }
        }));
        let (weak, here) = (self.me.borrow().clone(), id.to_string());
        order.append(&btn(&t!("Move down"), Some("down"), &["small"], move || {
            if let Some(page) = weak.upgrade() {
                page.move_by(&here, 1);
            }
        }));
        sheet.body.append(&order);
        sheet.body.append(&error);
        let dialog = sheet.dialog.clone();
        let cancel = btn(&t!("Cancel"), None, &[], move || {
            dialog.close();
        });
        let (weak, dialog, id) = (self.me.borrow().clone(), sheet.dialog.clone(), id.to_string());
        let (was_issuer, was_label) = (issuer.to_string(), label.to_string());
        let save = btn(&t!("Save"), None, &["primary"], move || {
            let Some(page) = weak.upgrade() else { return };
            let buffer = note.buffer();
            let words = Zeroizing::new(buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string());
            let tags: Vec<String> = tag_field.text().split(',').map(|tag| tag.trim().to_string()).filter(|tag| !tag.is_empty()).collect();
            let mut request = json!({
                "op": "auth-update",
                "id": id,
                "tags": tags,
                "pinned": pin_box.is_active(),
                "no_backup_codes": no_codes_box.is_active(),
            });
            // A name is what a prompt shows, so changing one asks: it is sent only when it was changed
            if service.text().as_str() != was_issuer {
                request["issuer"] = json!(service.text().as_str());
            }
            if account.text().as_str() != was_label {
                request["label"] = json!(account.text().as_str());
            }
            // The note: left alone unless it was read and is not what it was, and taken away by null
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
                    Err(trouble) => say(&error, &trouble.words()),
                }
            });
        });
        sheet.pin(&widgets::actions(&[cancel.upcast(), save.upcast()]));
        sheet.dialog.present(Some(&self.vault.shell.window));
    }

    /// One place up or down: the numbers of the rows that moved, which `order` keeps
    fn move_by(&self, id: &str, by: i64) {
        let mut order = self.list.orders();
        let Some(at) = order.iter().position(|(other, _)| other == id) else { return };
        let there = at as i64 + by;
        if there < 0 || there >= order.len() as i64 {
            return;
        }
        order.swap(at, there as usize);
        let writes = changed_places(&order);
        let (vault, weak) = (self.vault.clone(), self.me.borrow().clone());
        vault.clone().spawn(async move |_| {
            for (id, place) in writes {
                if vault.call(json!({"op": "auth-update", "id": id, "order": place})).await.is_err() {
                    break;
                }
            }
            if let Some(page) = weak.upgrade() {
                page.reload();
            }
        });
    }
}

/// The accounts in the order they are to be in, with the number each has now: those whose number is not their place, which are the
/// only ones to write. Every account gets one the first time, and after that a move changes two.
fn changed_places(order: &[(String, Option<i64>)]) -> Vec<(String, usize)> {
    order.iter().enumerate().filter(|(place, (_, number))| *number != Some(*place as i64)).map(|(place, (id, _))| (id.clone(), place)).collect()
}

/// A QR code for another app, behind a blurred stand-in until it is asked for, which takes an approval every time. It is never written
/// to a file, and goes after 30 seconds and when the window loses the focus. The key beside it, for an app that cannot scan.
fn show_qr(vault: &Rc<Vault>, id: String, title: String) {
    let sheet = widgets::sheet(&t!("Add {name} to another app", name = title), 480);
    sheet.body.append(&widgets::callout(&t!("Anyone who sees this code can make your sign-in codes. Scan it, then close this.")));
    let frame = ui::hbox(0, &["qr-frame"]);
    frame.set_halign(gtk::Align::Center);
    let code = qr::Code::new();
    code.update_property(&[gtk::accessible::Property::Label(&t!("QR code to scan with another app"))]);
    frame.append(&code);
    sheet.body.append(&frame);
    sheet.body.append(&widgets::p(&t!("Hides in 30 seconds, and if you leave this window."), &["soft", "small"]));
    let key = ui::label("", &["mono"]);
    key.set_direction(gtk::TextDirection::Ltr);
    key.set_visible(false);
    sheet.body.append(&key);
    let error = widgets::problem();
    sheet.body.append(&error);
    let held: Rc<RefCell<Option<Zeroizing<String>>>> = Rc::new(RefCell::new(None));
    let token = Rc::new(Cell::new(0));
    let show = btn(&t!("Show the code"), Some("eye"), &["primary"], || {});
    let show_key = btn(&t!("Show the key as text"), Some("eye"), &[], || {});
    show_key.set_sensitive(false);
    let (vault_show, reading, failing, keeping, tokens, pressing, key_button, key_label) = (vault.clone(), code.clone(), error.clone(), held.clone(), token.clone(), show.clone(), show_key.clone(), key.clone());
    show.connect_clicked(move |_| {
        let (vault, code, error, held, token, pressing, key_button, key_label, id) =
            (vault_show.clone(), reading.clone(), failing.clone(), keeping.clone(), tokens.clone(), pressing.clone(), key_button.clone(), key_label.clone(), id.clone());
        glib::spawn_future_local(async move {
            pressing.set_sensitive(false);
            let answer = match vault.call(json!({"op": "auth-link", "id": id})).await {
                Ok(answer) => answer,
                Err(Trouble::Refused { wait: 0, problem: None }) => {
                    pressing.set_sensitive(true);
                    return;
                }
                Err(trouble) => {
                    pressing.set_sensitive(true);
                    say(&error, &trouble.words());
                    return;
                }
            };
            let link = Zeroizing::new(answer["link"].as_str().unwrap_or("").as_bytes().to_vec());
            held.replace(Some(Zeroizing::new(answer["key"].as_str().unwrap_or("").to_string())));
            drop(answer);
            // Nobody to look at it: not shown
            if !vault.active_soon().await {
                held.replace(None);
                pressing.set_sensitive(true);
                return;
            }
            let Some(modules) = qr::encode(link).await else {
                held.replace(None);
                pressing.set_sensitive(true);
                say(&error, &t!("The QR code could not be drawn."));
                return;
            };
            code.set(Some(modules));
            key_button.set_sensitive(true);
            let (hide_code, hide_key, hide_held, hide_press, hide_button, hide_token) = (code.clone(), key_label.clone(), held.clone(), pressing.clone(), key_button.clone(), token.clone());
            token.set(vault.secrets.hold(move || {
                hide_code.set(None);
                hide_key.set_label("");
                hide_key.set_visible(false);
                hide_held.replace(None);
                hide_press.set_sensitive(true);
                hide_button.set_sensitive(false);
                hide_token.set(0);
            }));
        });
    });
    let (reading, showing) = (held.clone(), key.clone());
    show_key.connect_clicked(move |_| {
        if let Some(text) = &*reading.borrow() {
            showing.set_label(text);
            showing.set_visible(true);
        }
    });
    let dialog = sheet.dialog.clone();
    let done = btn(&t!("Done"), None, &[], move || {
        dialog.close();
    });
    let (vault_end, token_end, code_end, held_end) = (vault.clone(), token.clone(), code.clone(), held.clone());
    sheet.dialog.connect_closed(move |_| {
        vault_end.secrets.release(token_end.replace(0));
        code_end.set(None);
        held_end.replace(None);
    });
    sheet.pin(&widgets::actions(&[show_key.upcast(), done.upcast(), show.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

impl Page for Authenticator {
    fn widget(&self) -> gtk::Widget {
        self.list.root.clone().upcast()
    }

    fn reload(&self) {
        let (vault, weak) = (self.vault.clone(), self.me.borrow().clone());
        vault.clone().spawn(async move |_| {
            crate::logos::refresh().await;
            let Ok(answer) = vault.call(json!({"op": "list", "kind": KIND_ACCOUNT})).await else { return };
            let Some(page) = weak.upgrade() else { return };
            let items = answer["items"].as_array().cloned().unwrap_or_default();
            let exported = export::last_export();
            let unsaved = items.iter().filter(|item| item["meta"]["added_at"].as_i64().unwrap_or(0) > exported).count();
            page.since_export.set_visible(exported > 0 && unsaved > 0);
            if exported > 0 && unsaved > 0 {
                page.since_export.set_label(&t!("{count, plural, one {# account has} other {# accounts have}} been added since your last export.", count = unsaved));
            }
            let mut rows: Vec<Row> = items.iter().map(row_of).collect();
            super::sort_rows(&mut rows);
            // Those kept at the top, then each person's own order, then by name
            rows.sort_by_key(|row| (row.meta["pinned"] != json!(true), row.meta["order"].as_i64().unwrap_or(i64::MAX)));
            page.list.set_rows(rows);
            if vault.width.get() == Width::Full {
                page.list.select_first();
            }
            crate::logos::fill(&vault, &page.list).await;
        });
    }

    fn shown(&self) {
        self.visible.set(true);
        self.start_tick();
        // A wrong clock makes wrong codes, and timedatectl says so
        let clock = self.clock.clone();
        let above = self.list.above.clone();
        glib::spawn_future_local(async move {
            let said = process::run("timedatectl", &["show", "-p", "NTPSynchronized", "--value"]).await;
            let wrong = said.ok() && said.stdout.trim() == "no";
            clock.set_visible(wrong);
            above.set_visible(wrong);
        });
    }

    fn left(&self) {
        self.visible.set(false);
        self.hide_code();
    }

    fn focus(&self, active: bool) {
        if !active {
            // What was showing may come back with the window, if it is not one that asks every time
            let showing = self.shown.borrow().as_ref().filter(|shown| shown.showing.get() && !shown.ask && shown.kind != "hotp").map(|shown| shown.id.clone());
            if showing.is_some() {
                self.resume.replace(showing);
            }
            self.hide_code();
            return;
        }
        // Back: only what was showing when it went shows again
        let back = self.resume.take();
        let shown = self.shown.borrow().clone();
        if let (Some(back), Some(shown)) = (back, shown) {
            if shown.id == back && !shown.ask && shown.kind != "hotp" && !shown.showing.get() && self.visible.get() {
                self.fetch(&shown, false, true);
            }
        }
    }

    fn width(&self, width: Width) {
        self.list.set_narrow(width != Width::Full);
    }

    fn add(&self) {
        add::show(&self.vault);
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
    fn a_move_writes_only_what_moved() {
        // Numbers that are their places, and a swap of two: two writes
        let order: Vec<(String, Option<i64>)> = vec![("a".into(), Some(0)), ("c".into(), Some(2)), ("b".into(), Some(1)), ("d".into(), Some(3))];
        assert_eq!(changed_places(&order), vec![("c".to_string(), 1), ("b".to_string(), 2)]);
        // None numbered yet: all are written, once
        let fresh: Vec<(String, Option<i64>)> = vec![("a".into(), None), ("b".into(), None)];
        assert_eq!(changed_places(&fresh).len(), 2);
    }

    #[test]
    fn a_hidden_code_keeps_its_halves() {
        assert_eq!(dots(6), "••• •••");
    }

    #[test]
    fn an_account_is_named_by_its_service() {
        let row = row_of(&json!({"id": "a", "name": "x", "meta": {"issuer": "GitHub", "label": "me"}}));
        assert_eq!((row.title.as_str(), row.sub.as_str()), ("GitHub", "me"));
        let row = row_of(&json!({"id": "a", "name": "x", "meta": {"label": "me"}}));
        assert_eq!((row.title.as_str(), row.sub.as_str()), ("me", ""));
    }
}
