//! Bringing an authenticator code in: scanning one from the camera, the screen or a picture, typing or pasting it, moving everything from
//! BackMyCodes by its transfer codes, or importing another app's file. Whatever is read is parsed here, off the window's thread, and shown
//! before anything is saved; a single link goes to the service, which parses it strictly.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{gdk, gio, glib, gtk, t, ui};
use amethystora_vault_core::bmc::{self, Added, BmcError, Collector};
use amethystora_vault_core::import::{self, Imported, ImportError, Source};
use amethystora_vault_core::items::{Account, BackupService, OtpKind};
use amethystora_vault_core::otp::Algorithm;
use amethystora_vault_core::otpauth::{self, ManualEntry, ManualError};
use serde_json::{json, Value};
use zeroize::Zeroizing;

use super::Vault;
use crate::icons::icon;
use crate::rpc::{self, Trouble};
use crate::scan::{self, Seen};
use crate::secret;
use crate::widgets::{self, btn, say};

/// One of the ways in, as a tile
fn way(glyph: &str, title: &str, text: &str, on_click: impl Fn() + 'static) -> gtk::Button {
    let column = ui::vbox(6, &[]);
    let mark = icon(glyph, 20);
    mark.set_halign(gtk::Align::Start);
    column.append(&mark);
    column.append(&ui::line(title, &["way-title"]));
    column.append(&ui::label(text, &["soft", "small"]));
    let button = gtk::Button::builder().child(&column).css_classes(["plain", "way-tile"]).hexpand(true).build();
    button.update_property(&[gtk::accessible::Property::Label(&format!("{title}. {text}"))]);
    button.connect_clicked(move |_| on_click());
    button
}

/// Names the vault what is added goes to: the one chosen in the sheet of ways in, or the only one open
fn into(vault: &Vault, request: &mut Value) {
    match &*vault.add_target.borrow() {
        Some(target) => target.name(request),
        None => vault.target().name(request),
    }
}

pub fn show(vault: &Rc<Vault>) {
    let sheet = widgets::sheet(&t!("Add an authenticator code"), 600);
    sheet.body.append(&widgets::p(&t!("A site shows a QR code when you turn on two-step sign-in. Bring it in any of these ways."), &["soft"]));
    // Which vault it goes to, when more than one is open; whatever way it comes in
    let target = vault.target();
    if let Some(field) = target.field() {
        sheet.body.append(&field);
    }
    vault.add_target.replace(Some(target));
    let closing = |act: fn(&Rc<Vault>)| {
        let (vault, dialog) = (vault.clone(), sheet.dialog.clone());
        move || {
            dialog.close();
            act(&vault);
        }
    };
    let grid = widgets::columns(
        2,
        10,
        &[
            way("camera", &t!("Scan with the camera"), &t!("Hold the code up to your webcam."), closing(from_camera)).upcast(),
            way("screen", &t!("Scan the screen"), &t!("Pick the code in a window that is showing it."), closing(from_screen)).upcast(),
            way("picture", &t!("Open a picture"), &t!("A screenshot or a saved QR code."), closing(from_picture)).upcast(),
            way("pass", &t!("Type or paste it"), &t!("The setup key or an otpauth link."), closing(typed)).upcast(),
            way("codes", &t!("Move from BackMyCodes"), &t!("Scan its transfer codes, with their password."), closing(|vault| transfer(vault, Vec::new()))).upcast(),
            way("refresh", &t!("Import from another app"), &t!("Google Authenticator, Aegis, 2FAS, Bitwarden."), closing(from_file)).upcast(),
        ],
    );
    sheet.body.append(&grid);
    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    sheet.pin(&widgets::actions(&[cancel.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

// --- What was read --------------------------------------------------------------------------------------------------------------

/// What the codes in a picture or on a screen said: the parts of a transfer of BackMyCodes' (all of them, a page of them being one
/// transfer), a whole transfer of Google's, or links to add one by one
fn route(vault: &Rc<Vault>, codes: Vec<Zeroizing<String>>) {
    if codes.is_empty() {
        vault.say(&t!("No QR code was found there."));
        return;
    }
    let kind = |text: &str| text.get(..24).unwrap_or(text).to_ascii_lowercase();
    let (parts, others): (Vec<_>, Vec<_>) = codes.into_iter().partition(|text| {
        let lower = kind(text);
        lower.starts_with("bmc1:") || lower.starts_with("bmcp:")
    });
    if !parts.is_empty() {
        transfer(vault, parts);
        return;
    }
    if let Some(text) = others.iter().find(|text| kind(text).starts_with("otpauth-migration:")) {
        match import::read_migration(text) {
            Some((accounts, skipped)) => preview(vault, Imported { source: Source::Google, accounts, services: Vec::new(), skipped, plain: false }, false),
            None => vault.say(&t!("That QR code could not be read.")),
        }
        return;
    }
    add_links(vault, others);
}

/// Each link is an account of its own, and is added if it reads. Several in one picture are all added, and it says how many.
fn add_links(vault: &Rc<Vault>, links: Vec<Zeroizing<String>>) {
    vault.clone().spawn(async move |vault| {
        let (mut added, mut same, mut unread) = (0usize, 0usize, 0usize);
        let mut last = None;
        for link in &links {
            // Only for what it says of it: the service parses it again, and strictly
            if otpauth::read_link(link).is_none() {
                unread += 1;
                continue;
            }
            let mut request = json!({"op": "auth-add"});
            request["link"] = json!(link.as_str());
            into(&vault, &mut request);
            match vault.call(request).await {
                Ok(answer) => {
                    if answer["added"] == json!(false) {
                        same += 1;
                    } else {
                        added += 1;
                    }
                    last = answer["id"].as_str().map(str::to_string);
                }
                Err(trouble) => {
                    vault.say(&trouble.words());
                    return;
                }
            }
        }
        if links.len() == 1 {
            vault.say(&if unread > 0 {
                t!("That is not a link Vault can read.")
            } else if same > 0 {
                t!("That account is here already.")
            } else {
                t!("Added.")
            });
        } else {
            vault.say(&t!(
                "{added, plural, one {# account added} other {# accounts added}}, {same, plural, one {# was here already} other {# were here already}}, {unread, plural, one {# could not be read} other {# could not be read}}.",
                added = added,
                same = same,
                unread = unread
            ));
        }
        vault.count_items();
        vault.reload_and_select(last);
    });
}

fn from_screen(vault: &Rc<Vault>) {
    vault.clone().spawn(async move |vault| match scan::screen().await {
        Ok(codes) => {
            if let Some(this) = vault.me.upgrade() {
                route(&this, codes);
            }
        }
        Err(()) => vault.say(&t!("The screen could not be read.")),
    });
}

fn open_picture(vault: &Rc<Vault>, then: impl Fn(&Rc<Vault>, Vec<Zeroizing<String>>) + 'static) {
    vault.clone().spawn(async move |vault| {
        let Some(path) = scan::choose_file(&t!("Open a picture"), None).await else { return };
        match scan::picture(path).await {
            Ok(codes) => then(&vault, codes),
            Err(()) => vault.say(&t!("That file could not be read as a picture.")),
        }
    });
}

fn from_picture(vault: &Rc<Vault>) {
    open_picture(vault, route);
}

/// The camera, in a sheet: what it sees, and what it found. `found` says whether to go on looking.
fn camera_sheet(vault: &Rc<Vault>, found: impl Fn(&Rc<Vault>, Zeroizing<String>) -> bool + 'static) {
    let sheet = widgets::sheet(&t!("Scan with the camera"), 600);
    sheet.body.append(&widgets::p(&t!("Hold the code up to your webcam. Nothing it sees is kept."), &["soft"]));
    let picture = gtk::Picture::builder().content_fit(gtk::ContentFit::Contain).height_request(360).css_classes(["camera"]).build();
    picture.update_property(&[gtk::accessible::Property::Label(&t!("What the camera sees"))]);
    sheet.body.append(&picture);
    let error = widgets::problem();
    sheet.body.append(&error);
    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    sheet.pin(&widgets::actions(&[cancel.upcast()]));
    let running: Rc<RefCell<Option<scan::Camera>>> = Rc::new(RefCell::new(None));
    let saw_frame = Rc::new(Cell::new(false));
    let (vault_seen, shown, failing, keep, dialog, seen_frame) = (vault.clone(), picture.clone(), error.clone(), running.clone(), sheet.dialog.clone(), saw_frame.clone());
    let camera = scan::camera(move |seen| match seen {
        Seen::Frame(frame) => {
            seen_frame.set(true);
            let bytes = glib::Bytes::from_owned(frame.grey);
            let texture = gdk::MemoryTexture::new(scan::CAMERA_WIDTH as i32, scan::CAMERA_HEIGHT as i32, gdk::MemoryFormat::G8, &bytes, scan::CAMERA_WIDTH);
            shown.set_paintable(Some(&texture));
        }
        Seen::Code(text) => {
            if !found(&vault_seen, text) {
                keep.borrow_mut().take();
                dialog.close();
            }
        }
        Seen::Stopped => {
            // A portal that said no is the same for the window as a camera that is not there, or blocked
            if !seen_frame.get() {
                say(&failing, &t!("The camera could not be opened. If you turned cameras off, `ame security block-webcam off` turns them on again."));
            }
        }
    });
    running.replace(camera);
    if running.borrow().is_none() {
        say(&error, &t!("The camera could not be opened."));
    }
    sheet.dialog.connect_closed(move |_| {
        running.borrow_mut().take();
    });
    sheet.dialog.present(Some(&vault.shell.window));
}

fn from_camera(vault: &Rc<Vault>) {
    camera_sheet(vault, |vault, text| {
        route(vault, vec![text]);
        false
    });
}

// --- Typed ----------------------------------------------------------------------------------------------------------------------

fn typed(vault: &Rc<Vault>) {
    let sheet = widgets::sheet(&t!("Type or paste it"), 540);
    let (service, account, key) = (widgets::entry("", ""), widgets::entry("", ""), widgets::password_entry());
    sheet.body.append(&widgets::field(&t!("Setup key, or an otpauth link"), &key, &[]));
    sheet.body.append(&widgets::field(&t!("Service"), &service, &[]));
    sheet.body.append(&widgets::field(&t!("Account"), &account, &[]));
    let kinds = gtk::DropDown::from_strings(&[&t!("Time-based"), &t!("Counter-based"), &t!("Steam Guard")]);
    let algorithms = gtk::DropDown::from_strings(&["SHA-1", "SHA-224", "SHA-256", "SHA-384", "SHA-512"]);
    let digits = gtk::DropDown::from_strings(&["6", "8"]);
    let period = gtk::SpinButton::with_range(1.0, 3600.0, 1.0);
    period.set_value(30.0);
    let counter = gtk::SpinButton::with_range(0.0, 9_007_199_254_740_991.0, 1.0);
    counter.set_visible(false);
    let more = ui::vbox(12, &[]);
    more.append(&widgets::field(&t!("Kind"), &kinds, &[]));
    more.append(&widgets::columns(2, 12, &[widgets::field(&t!("Algorithm"), &algorithms, &[]).upcast(), widgets::field(&t!("Digits"), &digits, &[]).upcast()]));
    let period_field = widgets::field(&t!("Every so many seconds"), &period, &[]);
    more.append(&period_field);
    let counter_field = widgets::field(&t!("Counter"), &counter, &[]);
    counter_field.set_visible(false);
    more.append(&counter_field);
    sheet.body.append(&more);
    let (shown_period, shown_counter, inner_counter, shown_algorithms, shown_digits) = (period_field.clone(), counter_field.clone(), counter.clone(), algorithms.clone(), digits.clone());
    kinds.connect_selected_notify(move |kinds| {
        let which = kinds.selected();
        shown_period.set_visible(which == 0);
        shown_counter.set_visible(which == 1);
        inner_counter.set_visible(which == 1);
        // Steam has one way of making codes
        shown_algorithms.set_sensitive(which != 2);
        shown_digits.set_sensitive(which != 2);
    });
    let error = widgets::problem();
    sheet.body.append(&error);
    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    let (vault_add, dialog) = (vault.clone(), sheet.dialog.clone());
    let add = btn(&t!("Add it"), None, &["primary"], move || {
        let typed = secret::read_text(&key);
        let trimmed = typed.trim();
        if trimmed.contains("://") {
            dialog.close();
            route(&vault_add, vec![Zeroizing::new(trimmed.to_string())]);
            return;
        }
        let kind = match kinds.selected() {
            1 => OtpKind::Hotp,
            2 => OtpKind::Steam,
            _ => OtpKind::Totp,
        };
        let algorithm = [Algorithm::Sha1, Algorithm::Sha224, Algorithm::Sha256, Algorithm::Sha384, Algorithm::Sha512][algorithms.selected().min(4) as usize];
        let (issuer, label) = (service.text(), account.text());
        let entry = ManualEntry {
            kind,
            issuer: &issuer,
            label: &label,
            key: trimmed,
            algorithm,
            digits: if digits.selected() == 1 { 8 } else { 6 },
            period: period.value() as i64,
            counter: counter.value() as i64,
        };
        let made = match otpauth::manual_account(&entry) {
            Ok(made) => made,
            Err(problem) => {
                say(&error, &manual_words(problem));
                return;
            }
        };
        let Ok(value) = serde_json::to_value(&made) else { return };
        let mut request = json!({"op": "auth-add"});
        request["account"] = value;
        into(&vault_add, &mut request);
        let (vault, dialog, error) = (vault_add.clone(), dialog.clone(), error.clone());
        glib::spawn_future_local(async move {
            match vault.call(request).await {
                Ok(answer) => {
                    dialog.close();
                    vault.say(&if answer["added"] == json!(false) { t!("That account is here already.") } else { t!("Added.") });
                    vault.count_items();
                    vault.reload_and_select(answer["id"].as_str().map(str::to_string));
                }
                Err(trouble) => say(&error, &trouble.words()),
            }
        });
    });
    sheet.pin(&widgets::actions(&[cancel.upcast(), add.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

fn manual_words(problem: ManualError) -> String {
    match problem {
        ManualError::KeyRequired => t!("Type the setup key."),
        ManualError::KeyNotBase32 => t!("That is not a setup key: it has letters A to Z and digits 2 to 7, and at least 16 of them."),
        ManualError::NameRequired => t!("Give it a service or an account name."),
        ManualError::PeriodOutOfRange => t!("The seconds are out of range."),
        ManualError::CounterOutOfRange => t!("The counter is out of range."),
        ManualError::DigitsOutOfRange => t!("A code has 6 or 8 digits."),
    }
}

// --- BackMyCodes' transfer ------------------------------------------------------------------------------------------------------

/// The codes in any order, a chip for each, then the password, then what they hold, shown before anything is saved
fn transfer(vault: &Rc<Vault>, first: Vec<Zeroizing<String>>) {
    let sheet = widgets::sheet(&t!("Move from BackMyCodes"), 600);
    sheet.body.append(&widgets::p(&t!("Scan its transfer codes, in any order. Each one is ticked off here."), &["soft"]));
    let chips = ui::wrap(6, 6, &["transfer-chips"]);
    sheet.body.append(&chips);
    let warning = widgets::callout(&t!("This transfer has no password: anyone who photographed these codes can read it."));
    warning.set_visible(false);
    sheet.body.append(&warning);
    let password = widgets::password_entry();
    let password_field = widgets::field(&t!("The transfer's password"), &password, &[]);
    password_field.set_visible(false);
    sheet.body.append(&password_field);
    let error = widgets::problem();
    sheet.body.append(&error);
    let collector = Rc::new(RefCell::new(Collector::new()));
    let open = btn(&t!("Open the transfer"), None, &["primary"], || {});
    open.set_sensitive(false);

    let add_code: Rc<dyn Fn(&str) -> bool> = {
        let (collector, chips, error, warning, password_field, open) = (collector.clone(), chips.clone(), error.clone(), warning.clone(), password_field.clone(), open.clone());
        Rc::new(move |text: &str| {
            let added = collector.borrow_mut().add(text);
            match added {
                Added::NotAChunk => say(&error, &t!("That code is not part of a transfer.")),
                Added::OtherTransfer => say(&error, &t!("That code is from another transfer. Start again to read that one.")),
                Added::Again => say(&error, ""),
                Added::New => say(&error, ""),
            }
            let held = collector.borrow();
            ui::clear_wrap(&chips);
            for at in 0..held.total() {
                let have = held.has(at);
                let chip = widgets::chip(&t!("{number} of {total}", number = at + 1, total = held.total()), if have { "ok" } else { "" });
                chips.append(&chip);
            }
            let done = held.is_complete();
            warning.set_visible(done && held.is_plain());
            password_field.set_visible(done && !held.is_plain());
            open.set_sensitive(done);
            !done
        })
    };
    for code in &first {
        add_code(code);
    }
    let scans = ui::wrap(8, 8, &[]);
    let (vault_scan, adding) = (vault.clone(), add_code.clone());
    scans.append(&btn(&t!("Scan with the camera"), Some("camera"), &[], move || {
        let adding = adding.clone();
        camera_sheet(&vault_scan, move |_, text| adding(&text));
    }));
    let (vault_scan, adding) = (vault.clone(), add_code.clone());
    scans.append(&btn(&t!("Scan the screen"), Some("screen"), &[], move || {
        let adding = adding.clone();
        vault_scan.clone().spawn(async move |vault| match scan::screen().await {
            Ok(codes) => codes.iter().for_each(|code| {
                adding(code);
            }),
            Err(()) => vault.say(&t!("The screen could not be read.")),
        });
    }));
    let (vault_scan, adding) = (vault.clone(), add_code.clone());
    scans.append(&btn(&t!("Open a picture"), Some("picture"), &[], move || {
        let adding = adding.clone();
        open_picture(&vault_scan, move |_, codes| {
            codes.iter().for_each(|code| {
                adding(code);
            });
        });
    }));
    sheet.body.append(&scans);

    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    let (vault_open, dialog, failing) = (vault.clone(), sheet.dialog.clone(), error.clone());
    open.connect_clicked(move |open| {
        let typed = secret::read_text(&password);
        let held = collector.replace(Collector::new());
        let (vault, dialog, failing, open, collector) = (vault_open.clone(), dialog.clone(), failing.clone(), open.clone(), collector.clone());
        open.set_sensitive(false);
        glib::spawn_future_local(async move {
            let now = (amethystora_ui::i18n::now() * 1000.0) as i64;
            let answered = gio::spawn_blocking(move || {
                let read = held.read(&typed, now);
                (held, read)
            })
            .await;
            open.set_sensitive(true);
            let Ok((held, read)) = answered else { return };
            collector.replace(held);
            match read {
                Ok(contents) => {
                    let plain = contents.plain;
                    dialog.close();
                    preview(&vault, Imported { source: Source::BackMyCodes, accounts: contents.accounts, services: contents.services, skipped: contents.skipped, plain }, false);
                }
                Err(problem) => say(&failing, &bmc_words(problem)),
            }
        });
    });
    sheet.pin(&widgets::actions(&[cancel.upcast(), open.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

pub fn bmc_words(problem: BmcError) -> String {
    match problem {
        BmcError::BadPassword => t!("Wrong password, or the file has been changed."),
        BmcError::UnsupportedVersion => t!("This was made by a newer version than this one reads."),
        BmcError::Unreadable => t!("That is not a file Vault can read."),
        BmcError::TooLarge => t!("That is too large to read."),
        BmcError::Incomplete | BmcError::NoChunks => t!("Some codes are still missing."),
        BmcError::WeakPassword => t!("A password here has at least 8 characters."),
        BmcError::NotJson | BmcError::NotAnExport => t!("That is not a file Vault can read."),
        BmcError::Crypto => t!("This computer's cryptography failed."),
    }
}

// --- Another app's file ---------------------------------------------------------------------------------------------------------

fn from_file(vault: &Rc<Vault>) {
    vault.clone().spawn(async move |vault| {
        let Some(path) = scan::choose_file(&t!("Import from another app"), None).await else { return };
        read_file(&vault, path, None);
    });
}

/// Blocks: at most what a file may be
fn read_bounded(path: &PathBuf) -> Option<Zeroizing<Vec<u8>>> {
    use std::io::Read;
    let mut bytes = Zeroizing::new(Vec::new());
    std::fs::File::open(path).ok()?.take(bmc::MAX_FILE as u64 + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() <= bmc::MAX_FILE).then_some(bytes)
}

fn read_file(vault: &Rc<Vault>, path: PathBuf, password: Option<Zeroizing<String>>) {
    vault.clone().spawn(async move |vault| {
        let asked = path.clone();
        let tried = password.clone();
        // scrypt or PBKDF2 for an encrypted one, and any parsing, off the window's thread
        let read = gio::spawn_blocking(move || {
            let bytes = read_bounded(&asked).ok_or(ImportError::TooLarge)?;
            let empty = Zeroizing::new(String::new());
            import::read_file(&bytes, tried.as_ref().unwrap_or(&empty))
        })
        .await
        .unwrap_or(Err(ImportError::Crypto));
        match read {
            Ok(imported) => {
                if let Some(this) = vault.me.upgrade() {
                    preview(&this, imported, false);
                }
            }
            Err(ImportError::PasswordRequired) => ask_file_password(&vault, path, None),
            Err(ImportError::BadPassword) => ask_file_password(&vault, path, Some(t!("Wrong password."))),
            Err(problem) => vault.say(&import_words(problem)),
        }
    });
}

fn ask_file_password(vault: &Vault, path: PathBuf, problem: Option<String>) {
    let sheet = widgets::sheet(&t!("This file has a password"), 480);
    let input = widgets::password_entry();
    sheet.body.append(&widgets::field(&t!("Its password"), &input, &[]));
    let error = widgets::problem();
    if let Some(problem) = &problem {
        say(&error, problem);
    }
    sheet.body.append(&error);
    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    let (weak, dialog, reading) = (vault.me.clone(), sheet.dialog.clone(), input.clone());
    let go = Rc::new(move || {
        let Some(this) = weak.upgrade() else { return };
        let password = secret::read_text(&reading);
        dialog.close();
        read_file(&this, path.clone(), Some(password));
    });
    let run = go.clone();
    let open = btn(&t!("Open"), None, &["primary"], move || run());
    input.connect_activate(move |_| go());
    sheet.pin(&widgets::actions(&[cancel.upcast(), open.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

fn import_words(problem: ImportError) -> String {
    match problem {
        ImportError::Unsupported => t!("That is not a file Vault can read."),
        ImportError::Empty => t!("That file holds no accounts."),
        ImportError::PasswordRequired | ImportError::BadPassword => t!("Wrong password."),
        ImportError::BitwardenEncrypted => t!("This Bitwarden export is encrypted. Export it again unencrypted."),
        ImportError::UnsupportedVersion => t!("This was made by a newer version than this one reads."),
        ImportError::TooMuchMemory => t!("This file asks for more memory to open than Vault will give it."),
        ImportError::TooLarge => t!("That is too large to read."),
        ImportError::Crypto => t!("This computer's cryptography failed."),
    }
}

// --- What it holds, before anything is saved --------------------------------------------------------------------------------------

/// One line of the service's size takes this much of what was read, at most
const BATCH: usize = 256 * 1024;
/// And the service takes this many accounts, and this many sets of backup codes, in one
const MOST_AT_ONCE: usize = 256;

fn to_values<T: serde::Serialize>(items: &[T]) -> Vec<Value> {
    items.iter().filter_map(|item| serde_json::to_value(item).ok()).collect()
}

/// Requests the service takes: at most a line's size, and at most 256 accounts and 256 sets of codes each. Accounts, then services.
fn batches(accounts: &[Account], services: &[BackupService]) -> Vec<(Vec<Value>, Vec<Value>)> {
    split(to_values(accounts), to_values(services))
}

fn split(accounts: Vec<Value>, services: Vec<Value>) -> Vec<(Vec<Value>, Vec<Value>)> {
    let mut out = Vec::new();
    let (mut these, mut those, mut size) = (Vec::new(), Vec::new(), 0usize);
    for value in accounts {
        size += value.to_string().len();
        these.push(value);
        if size > BATCH || these.len() >= MOST_AT_ONCE {
            out.push((std::mem::take(&mut these), Vec::new()));
            size = 0;
        }
    }
    for value in services {
        size += value.to_string().len();
        those.push(value);
        if size > BATCH || those.len() >= MOST_AT_ONCE {
            out.push((std::mem::take(&mut these), std::mem::take(&mut those)));
            size = 0;
        }
    }
    if !these.is_empty() || !those.is_empty() || out.is_empty() {
        out.push((these, those));
    }
    out
}

/// A request to merge, with what is merged moved into it rather than copied
fn merge_request(accounts: Vec<Value>, services: Vec<Value>, source: &str) -> Value {
    let mut request = json!({"op": "import-merge", "source": source});
    request["accounts"] = Value::Array(accounts);
    request["services"] = Value::Array(services);
    request
}

fn preview(vault: &Rc<Vault>, imported: Imported, plain: bool) {
    let plain = plain || imported.plain;
    let sheet = widgets::sheet(&t!("What this holds"), 540);
    let count_words = t!("{accounts, plural, one {# account} other {# accounts}} and {services, plural, one {# set of backup codes} other {# sets of backup codes}}.", accounts = imported.accounts.len(), services = imported.services.len());
    sheet.body.append(&widgets::p(&count_words, &[]));
    if imported.skipped > 0 {
        sheet.body.append(&widgets::callout(&t!("{count, plural, one {# entry} other {# entries}} could not be read, and will be left out.", count = imported.skipped)));
    }
    if plain {
        sheet.body.append(&widgets::callout(&t!("This transfer had no password: anyone who photographed its codes can read it.")));
    }
    let names = gtk::StringList::new(&[]);
    for account in &imported.accounts {
        let name = if account.issuer.is_empty() { account.label.clone() } else if account.label.is_empty() { account.issuer.clone() } else { format!("{}: {}", account.issuer, account.label) };
        names.append(&name);
    }
    for service in &imported.services {
        names.append(&service.service_name);
    }
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
            item.set_child(Some(&widgets::data("", &["preview-row"])));
        }
    });
    factory.connect_bind(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
        let words = item.item().and_then(|object| object.downcast::<gtk::StringObject>().ok()).map(|object| object.string().to_string()).unwrap_or_default();
        if let Some(label) = item.child().and_then(|child| child.downcast::<gtk::Label>().ok()) {
            label.set_label(&words);
        }
    });
    let view = gtk::ListView::builder().model(&gtk::NoSelection::new(Some(names))).factory(&factory).css_classes(["preview"]).build();
    let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).height_request(200).child(&view).build();
    sheet.body.append(&scroller);
    let replacing = Rc::new(Cell::new(false));
    let chosen = replacing.clone();
    let summary = ui::label("", &["soft", "small"]);
    sheet.body.append(&widgets::seg(&[("merge", t!("Add to what is here")), ("replace", t!("Replace what is here"))], "merge", move |value| chosen.set(value == "replace")));
    sheet.body.append(&summary);
    let error = widgets::problem();
    sheet.body.append(&error);
    let held = Rc::new(imported);
    // What is new: the same account is the same issuer and secret, which only the service can tell
    let (vault_dry, held_dry, saying) = (vault.clone(), held.clone(), summary.clone());
    glib::spawn_future_local(async move {
        let source = if held_dry.source == Source::BackMyCodes { "backmycodes" } else { "other" };
        let (mut added, mut same) = (0u64, 0u64);
        for (accounts, services) in batches(&held_dry.accounts, &held_dry.services) {
            let mut request = merge_request(accounts, services, source);
            request["dry"] = json!(true);
            into(&vault_dry, &mut request);
            let Ok(answer) = vault_dry.call(request).await else { return };
            added += answer["added"].as_u64().unwrap_or(0) + answer["added_services"].as_u64().unwrap_or(0);
            same += answer["same"].as_u64().unwrap_or(0) + answer["same_services"].as_u64().unwrap_or(0);
        }
        saying.set_label(&t!("{added, plural, one {# is new} other {# are new}}, {same, plural, one {# is here already} other {# are here already}}.", added = added, same = same));
    });
    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    let (vault_go, dialog, failing) = (vault.clone(), sheet.dialog.clone(), error.clone());
    let save = btn(&t!("Save them"), None, &["primary"], move || {
        let (vault, held, dialog, failing, replace) = (vault_go.clone(), held.clone(), dialog.clone(), failing.clone(), replacing.get());
        glib::spawn_future_local(async move {
            let source = if held.source == Source::BackMyCodes { "backmycodes" } else { "other" };
            let parts = batches(&held.accounts, &held.services);
            // Replacing takes things away, and the service does it in one request: more than one would leave the vault half replaced if
            // the window, the service or the vault were to end between them. Nothing is removed for a file that is too large for one.
            if replace && parts.len() > 1 {
                say(&failing, &t!("This file is too large to replace what is here in one step. Add it to what is here instead."));
                return;
            }
            if replace {
                let target = vault.add_target.borrow().as_ref().and_then(|target| target.id());
                let name = target.map(|id| vault.vault_name(&id)).filter(|name| !name.is_empty());
                let body = match name {
                    Some(name) => t!("Every account and set of backup codes in “{vault}” that the file does not hold will be removed.", vault = name),
                    None => t!("Every account here that the file does not hold will be removed."),
                };
                if !vault.confirm(&t!("Replace what is here?"), &body, &t!("Replace"), true).await {
                    return;
                }
            }
            let mut added = 0u64;
            for (accounts, services) in parts {
                let mut request = merge_request(accounts, services, source);
                if replace {
                    request["replace"] = json!(true);
                }
                into(&vault, &mut request);
                match vault.call(request).await {
                    Ok(answer) => added += answer["added"].as_u64().unwrap_or(0) + answer["added_services"].as_u64().unwrap_or(0),
                    Err(Trouble::Refused { wait: 0, problem: None }) => return,
                    Err(trouble) => {
                        say(&failing, &trouble.words());
                        return;
                    }
                }
            }
            dialog.close();
            vault.say(&t!("{added, plural, one {# item was saved.} other {# items were saved.}}", added = added));
            vault.count_items();
            vault.reload_current();
        });
    });
    sheet.pin(&widgets::actions(&[cancel.upcast(), save.upcast()]));
    sheet.dialog.present(Some(&vault.shell.window));
}

#[allow(unused_imports)]
use rpc as _;

#[cfg(test)]
mod tests {
    use super::*;

    fn many(count: usize) -> Vec<Value> {
        (0..count).map(|at| json!({"issuer": format!("service {at}"), "label": "me"})).collect()
    }

    #[test]
    fn a_file_of_three_hundred_goes_in_requests_of_at_most_256() {
        let parts = split(many(300), many(10));
        assert_eq!(parts.iter().map(|(accounts, _)| accounts.len()).collect::<Vec<_>>(), vec![256, 44]);
        assert_eq!(parts.iter().map(|(_, services)| services.len()).sum::<usize>(), 10);
        assert!(parts.iter().all(|(accounts, services)| accounts.len() <= MOST_AT_ONCE && services.len() <= MOST_AT_ONCE));
        // Exactly a batch is one, and nothing at all is one empty request
        assert_eq!(split(many(256), Vec::new()).len(), 1);
        assert_eq!(split(Vec::new(), Vec::new()).len(), 1);
        // The sets of backup codes are cut the same way
        assert_eq!(split(Vec::new(), many(600)).iter().map(|(_, services)| services.len()).collect::<Vec<_>>(), vec![256, 256, 88]);
    }

    #[test]
    fn a_request_takes_what_it_is_given() {
        let request = merge_request(many(2), Vec::new(), "other");
        assert_eq!(request["accounts"].as_array().map(Vec::len), Some(2));
        assert_eq!(request["op"], "import-merge");
    }
}
