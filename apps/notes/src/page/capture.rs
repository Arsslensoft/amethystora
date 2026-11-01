//! The quick note's window (`amethystora-notes capture`, Super+Alt+N): a small window over anything, in which what is
//! typed becomes a task in the inbox (capture.rs reads it). Enter saves, Shift+Enter starts a new line, Esc closes
//! without saving. When the notes are locked it asks for the passphrase (or a passkey) and saves after; it locks them
//! again unless Notes' own window is showing. The text is in the window's buffer only, never written anywhere, until
//! it is saved; closed without saving, it is gone.
//!
//! The window is a plain window of Notes' application, not its application window: the tiling floats it and gives it
//! the keyboard for that (Amethystora UI's tiling/config.js).

use std::rc::Rc;

use serde_json::{json, Map, Value};

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{adw, gdk, glib, gtk, t, ui};

use super::widgets::{button, key_status, link_button, say};
use super::Notes;
use crate::backend;
use crate::passkeys::{self, Unlocked};

/// The window's width, in pixels; its height is what its contents take
const WIDTH: i32 = 376;

pub struct Capture {
    pub window: adw::Window,
    text: gtk::TextView,
    footer: gtk::Box,
    locked: gtk::Box,
    new: gtk::Box,
    entry: gtk::PasswordEntry,
    error: gtk::Label,
    unlock: gtk::Button,
    passkey: gtk::Button,
}

/// A key as the board draws one, in a cap
fn key_cap(name: &str) -> gtk::Label {
    let cap = ui::label(name, &["keycap"]);
    cap.set_valign(gtk::Align::Center);
    cap
}

/// "[Enter] saves to Notes › Inbox": a key in its cap, then the words
fn key_line(key: &str, words: &str) -> gtk::Box {
    let line = ui::hbox(6, &["capture-key-line"]);
    line.append(&key_cap(key));
    line.append(&ui::label(words, &["capture-dim"]));
    line
}

impl Notes {
    /// The quick note's window: brought to the front with its text when it is open already
    pub fn open_capture(&self) {
        if let Some(capture) = self.capture.borrow().as_ref() {
            capture.window.present();
            return;
        }
        let capture = self.build_capture();
        let window = capture.window.clone();
        self.capture.replace(Some(capture));
        self.capture_state();
        window.present();
        if let Some(capture) = self.capture.borrow().as_ref() {
            capture.text.grab_focus();
        }
    }

    fn build_capture(&self) -> Capture {
        let window = adw::Window::builder()
            .application(&self.shell.app)
            .title(t!("Quick note"))
            .default_width(WIDTH)
            .resizable(false)
            .build();
        // libadwaita gives a window 200 pixels of height at least: this one is as tall as what it holds
        window.set_size_request(WIDTH, 120);
        window.add_css_class("amethystora");
        window.add_css_class("capture");

        let card = ui::vbox(10, &["capture-card"]);
        let head = ui::hbox(10, &["capture-head"]);
        let gem = gtk::Image::from_icon_name("amethystora-notes");
        gem.set_pixel_size(22);
        head.append(&gem);
        let title = ui::label(&t!("Quick note"), &["capture-title"]);
        title.set_hexpand(true);
        title.set_xalign(0.0);
        head.append(&title);
        // The keys as they are written on a keyboard, in every language
        for name in ["Super", "Alt", "N"] {
            head.append(&key_cap(name));
        }
        card.append(&head);

        let text = gtk::TextView::builder().wrap_mode(gtk::WrapMode::WordChar).accepts_tab(false).css_classes(["capture-text"]).build();
        text.update_property(&[gtk::accessible::Property::Label(&t!("Quick note"))]);
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .min_content_height(66)
            .max_content_height(132)
            .child(&text)
            .css_classes(["capture-frame"])
            .build();
        card.append(&scroller);

        // Saves to the inbox, or closes
        let footer = ui::hbox(12, &["capture-footer"]);
        let saves = key_line(&t!("Enter"), &t!("saves to Notes › Inbox"));
        saves.set_hexpand(true);
        footer.append(&saves);
        footer.append(&key_line(&t!("Esc"), &t!("closes")));
        card.append(&footer);

        // The notes are locked: the passphrase first
        let locked = ui::vbox(8, &["capture-locked"]);
        let lock_row = ui::hbox(10, &[]);
        let lock = crate::icons::icon("lock", 20);
        lock.add_css_class("capture-lock");
        lock_row.append(&lock);
        lock_row.append(&ui::label(&t!("Notes is locked"), &["capture-title"]));
        locked.append(&lock_row);
        let hint = ui::label(&t!("Unlock it to save this note"), &["capture-dim"]);
        hint.set_xalign(0.0);
        locked.append(&hint);
        // GTK's own field for a password: it never offers the text to copy, and shows the Caps Lock warning
        let entry = gtk::PasswordEntry::builder().placeholder_text(t!("Passphrase")).css_classes(["field", "capture-entry"]).build();
        entry.update_property(&[gtk::accessible::Property::Label(&t!("Passphrase"))]);
        locked.append(&entry);
        let error = ui::label("", &["error", "small"]);
        error.set_xalign(0.0);
        error.set_visible(false);
        locked.append(&error);
        // The button on the end side, and the passkey under it
        let unlock = button(&t!("Unlock and save"), None, &["primary", "capture-unlock"], self.on(|this| this.capture_unlock()));
        unlock.set_halign(gtk::Align::End);
        locked.append(&unlock);
        let passkey = link_button(&t!("Open with a passkey"), Some("key"), &[], self.on(|this| this.capture_passkey()));
        passkey.set_halign(gtk::Align::End);
        locked.append(&passkey);
        card.append(&locked);

        // Notes has never been set up: there is nowhere to save yet
        let new = ui::vbox(10, &["capture-new"]);
        let first = ui::label(&t!("Set up Notes first"), &["capture-title"]);
        first.set_xalign(0.0);
        new.append(&first);
        let open = button(&t!("Open Notes"), None, &["primary"], self.on(|this| this.capture_open_notes()));
        open.set_halign(gtk::Align::End);
        new.append(&open);
        card.append(&new);

        window.set_content(Some(&card));

        // Enter saves, Shift+Enter is a new line; Esc closes without saving
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.me.clone();
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(this) = weak.upgrade() else { return glib::Propagation::Proceed };
            this.capture_key(key, modifiers)
        });
        window.add_controller(keys);
        let weak = self.me.clone();
        entry.connect_activate(move |_| {
            if let Some(this) = weak.upgrade() {
                this.capture_unlock();
            }
        });
        // However it closes, the text goes with it
        let weak = self.me.clone();
        window.connect_close_request(move |_| {
            if let Some(this) = weak.upgrade() {
                this.capture_closed();
            }
            glib::Propagation::Proceed
        });

        Capture { window, text, footer, locked, new, entry, error, unlock, passkey }
    }

    fn capture_key(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        // The password field's own text has the keyboard, not the field
        let Some((text_focused, entry_focused)) = self.capture.borrow().as_ref().map(|c| (c.text.has_focus(), c.entry.state_flags().contains(gtk::StateFlags::FOCUS_WITHIN))) else {
            return glib::Propagation::Proceed;
        };
        if key == gdk::Key::Escape {
            self.close_capture();
            return glib::Propagation::Stop;
        }
        let enter = key == gdk::Key::Return || key == gdk::Key::KP_Enter || key == gdk::Key::ISO_Enter;
        if !enter || entry_focused || modifiers.contains(gdk::ModifierType::SHIFT_MASK) || !text_focused {
            return glib::Propagation::Proceed;
        }
        self.capture_save();
        glib::Propagation::Stop
    }

    /// The window as the notes are now: the footer while they are open, the passphrase while they are locked, the way to
    /// Notes while they were never set up
    pub fn capture_state(&self) {
        let status = self.backend.borrow().status();
        let passkeys = !self.backend.borrow().passkeys().is_empty();
        let Some(capture) = self.capture.borrow().as_ref().map(|c| (c.footer.clone(), c.locked.clone(), c.new.clone(), c.passkey.clone(), c.text.clone())) else {
            return;
        };
        let (footer, locked, new, passkey, text) = capture;
        footer.set_visible(status == "open");
        locked.set_visible(status == "locked");
        new.set_visible(status == "new");
        passkey.set_visible(passkeys);
        text.set_sensitive(status != "new");
    }

    fn capture_text(&self) -> String {
        let Some(buffer) = self.capture.borrow().as_ref().map(|c| c.text.buffer()) else { return String::new() };
        let (start, end) = buffer.bounds();
        buffer.text(&start, &end, false).to_string()
    }

    /// Enter in the text: saved now when the notes are open; when they are locked, the passphrase comes first
    fn capture_save(&self) {
        let (first, _) = crate::capture::split(&self.capture_text());
        if first.is_empty() {
            return;
        }
        match self.backend.borrow().status() {
            "open" => {}
            "locked" => {
                if let Some(entry) = self.capture.borrow().as_ref().map(|c| c.entry.clone()) {
                    entry.grab_focus();
                }
                return;
            }
            _ => return,
        }
        self.capture_store(false);
    }

    /// The task made from the text, saved and written; then the window goes. `unlocked`: the window opened the notes,
    /// and they lock again unless Notes' own window is showing.
    fn capture_store(&self, unlocked: bool) {
        let (first, rest) = crate::capture::split(&self.capture_text());
        if first.is_empty() || self.backend.borrow().store.is_none() {
            return;
        }
        let mut fields: Map<String, Value> = Map::new();
        fields.insert("labels".into(), json!([]));
        for (name, value) in self.parse_quick(&first) {
            fields.insert(name, value);
        }
        if !rest.is_empty() {
            fields.insert("notes".into(), Value::from(rest));
        }
        self.new_task(fields);
        self.commit();
        let shown = self.shell.window.is_visible() && !self.hidden.get();
        if shown {
            self.render_nav();
            if self.ui.borrow().mode == super::Mode::Tasks {
                self.render_task_list();
            }
        }
        // The window goes, and its text with it; Notes leaves once the task is written
        if let Some(capture) = self.capture.take() {
            capture.text.buffer().set_text("");
            capture.window.close();
        }
        self.spawn(async move |this| {
            backend::flush(&this.backend).await;
            if unlocked && !shown {
                backend::lock(&this.backend).await;
                backend::flush(&this.backend).await;
            }
            this.capture_leave();
        });
    }

    /// The passphrase given in the window: the notes open, the note is saved
    fn capture_unlock(&self) {
        let Some((entry, error, unlock)) = self.capture.borrow().as_ref().map(|c| (c.entry.clone(), c.error.clone(), c.unlock.clone())) else {
            return;
        };
        let passphrase = entry.text().to_string();
        if passphrase.is_empty() || !unlock.is_sensitive() {
            return;
        }
        unlock.set_sensitive(false);
        say(&error, "");
        self.spawn(async move |this| {
            match backend::unlock(&this.backend, passphrase).await {
                Err(problem) => {
                    unlock.set_sensitive(true);
                    say(&error, &problem);
                    entry.grab_focus();
                    entry.select_region(0, -1);
                }
                Ok(()) => {
                    entry.set_text("");
                    this.capture_unlocked();
                }
            }
        });
    }

    /// A passkey instead of the passphrase: a key that reads a fingerprint needs nothing more; one that wants its PIN
    /// takes it in the passphrase's field
    fn capture_passkey(&self) {
        if self.backend.borrow().touching.is_some() {
            return;
        }
        let Some((entry, error)) = self.capture.borrow().as_ref().map(|c| (c.entry.clone(), c.error.clone())) else { return };
        self.spawn(async move |this| {
            let pin = if entry.has_css_class("capture-pin") { entry.text().to_string() } else { String::new() };
            key_status(&error, &t!("Looking for your security key. Touch it if it blinks."), true);
            let backend = this.backend.clone();
            let result = passkeys::with_key(&backend, async || passkeys::unlock(&backend, pin).await).await;
            key_status(&error, "", false);
            match result {
                Ok(Unlocked::NeedsPin) => {
                    entry.set_text("");
                    entry.add_css_class("capture-pin");
                    entry.set_placeholder_text(Some(&t!("Security key PIN")));
                    key_status(&error, &t!("Enter the PIN of your security key, then touch the key when it blinks."), true);
                    entry.grab_focus();
                }
                Ok(_) => {
                    entry.set_text("");
                    this.capture_unlocked();
                }
                Err(problem) => say(&error, &problem),
            }
        });
    }

    /// Opened from the window: saved, and locked again unless Notes' own window is showing, where they stay open as
    /// the gate leaves them
    fn capture_unlocked(&self) {
        let shown = self.shell.window.is_visible() && !self.hidden.get();
        if shown {
            self.apply_state(None);
        }
        // The window went while the passphrase was checked (Esc during scrypt): nothing to save, and the notes are not
        // left open behind no window
        if self.capture.borrow().is_none() {
            if !shown {
                self.spawn(async move |this| {
                    backend::lock(&this.backend).await;
                    this.capture_leave();
                });
            }
            return;
        }
        // The window shows the notes open (a quick note opened again during the check has nothing saved yet); closing
        // it locks them again (capture_leave)
        self.capture_state();
        self.capture_store(true);
    }

    fn capture_open_notes(&self) {
        self.close_capture();
        self.hidden.set(false);
        self.render();
        self.shell.window.present();
    }

    fn close_capture(&self) {
        let window = self.capture.borrow().as_ref().map(|c| c.window.clone());
        if let Some(window) = window {
            window.close();
        }
    }

    /// The window went: its text with it
    fn capture_closed(&self) {
        let Some(capture) = self.capture.take() else { return };
        capture.text.buffer().set_text("");
        capture.entry.set_text("");
        let weak = self.me.clone();
        glib::idle_add_local_once(move || {
            if let Some(this) = weak.upgrade() {
                this.capture_leave();
            }
        });
    }

    /// `amethystora-notes remind` (the search's "remind me at 17:00 to ..."): its line read whole from the standard
    /// input, which the launch keeps until it is let go, never from the command line, which every account can read
    pub fn read_remind(&self, line: &gtk::gio::ApplicationCommandLine) {
        let Some(stream) = line.stdin() else { return };
        let line = line.clone();
        self.spawn(async move |this| {
            let mut bytes: Vec<u8> = Vec::new();
            loop {
                match stream.read_bytes_future(4096, glib::Priority::DEFAULT).await {
                    Ok(chunk) if !chunk.is_empty() && bytes.len() <= crate::capture::REMIND_MOST => bytes.extend_from_slice(&chunk),
                    _ => break,
                }
            }
            let _ = stream.close_future(glib::Priority::DEFAULT).await;
            drop(line);
            let text = crate::capture::remind_line(&bytes);
            bytes.fill(0);
            match text {
                Some(text) => this.remind(text),
                None => this.capture_leave(),
            }
        });
    }

    /// The reminder: into the inbox now when the notes are open, said in a notification whose click shows the task;
    /// while they are locked (or were never set up), the quick note's window with the text in it asks first, and saves
    /// it as a quick note is saved
    fn remind(&self, text: String) {
        if self.backend.borrow().status() != "open" {
            let open = self.capture.borrow().as_ref().map(|c| c.text.buffer());
            match open {
                // A quick note being written is never written over
                Some(buffer) if buffer.char_count() > 0 => {
                    self.notifier.show(&t!("Notes"), &t!("The quick note is open: save it first, then ask again."), Rc::new(|| {}));
                    return;
                }
                _ => {}
            }
            self.open_capture();
            let parts = self.capture.borrow().as_ref().map(|c| (c.text.clone(), c.entry.clone()));
            if let Some((view, entry)) = parts {
                view.buffer().set_text(&text);
                self.capture_state();
                if self.backend.borrow().status() == "locked" {
                    entry.grab_focus();
                }
            }
            return;
        }
        let mut fields: Map<String, Value> = Map::new();
        fields.insert("labels".into(), json!([]));
        for (name, value) in self.parse_quick(&text) {
            fields.insert(name, value);
        }
        let title = fields.get("title").and_then(Value::as_str).unwrap_or_default().to_string();
        let due = fields.get("due").and_then(Value::as_str).unwrap_or_default().to_string();
        let time = fields.get("time").and_then(Value::as_str).unwrap_or_default().to_string();
        let id = self.new_task(fields);
        self.commit();
        let shown = self.shell.window.is_visible() && !self.hidden.get();
        if shown {
            self.render_nav();
            if self.ui.borrow().mode == super::Mode::Tasks {
                self.render_task_list();
            }
        }
        let body = if due.is_empty() || time.is_empty() {
            title.clone()
        } else {
            t!("{task}, {day} at {time}", task = title.clone(), day = crate::dates::day_label(&due, false), time = crate::dates::time_label(&time))
        };
        let weak = self.me.clone();
        self.notifier.show(&t!("Reminder set"), &body, Rc::new(move || {
            if let Some(this) = weak.upgrade() {
                this.show_task(&id);
            }
        }));
        self.spawn(async move |this| {
            backend::flush(&this.backend).await;
            this.capture_leave();
        });
    }

    /// Notes started for the quick note goes again once it is done, unless a reminder is still to come
    fn capture_leave(&self) {
        if !self.hidden.get() || self.capture.borrow().is_some() {
            return;
        }
        let (upcoming, locking) = {
            let backend = self.backend.borrow();
            (backend.upcoming(), backend.locking)
        };
        let background = self.setting("background") != Some(Value::Bool(false));
        if locking {
            return;
        }
        // Locked as closing Notes' own window locks them: and Notes goes too unless a reminder is still to come
        let stays = upcoming && background;
        self.spawn(async move |this| {
            backend::lock(&this.backend).await;
            if !stays {
                this.quit();
            }
        });
    }
}
