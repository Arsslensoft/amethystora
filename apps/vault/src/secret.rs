//! A secret on the screen and on the clipboard. Whatever shows one says how to take it away again, and it goes after 30 seconds,
//! when the window loses the focus and when the vault closes. GTK keeps its own copy of what a label shows, which cannot be wiped:
//! that is why it is only there while it is shown.
//!
//! The clipboard offers the text and `x-kde-passwordManagerHint`, which Amethystora Clipboard, the phone link and the shell's keys
//! leave alone, and is emptied after 30 seconds if the text is still ours.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use amethystora_ui::{gdk, glib, gtk};
use gtk::prelude::*;
use zeroize::Zeroizing;

pub const SHOWN_FOR: Duration = Duration::from_secs(30);

struct Shown {
    id: u64,
    hide: Rc<dyn Fn()>,
    timer: Option<glib::SourceId>,
}

/// What is on the screen now
#[derive(Clone, Default)]
pub struct Secrets {
    next: Rc<Cell<u64>>,
    shown: Rc<RefCell<Vec<Shown>>>,
}

impl Secrets {
    /// `hide` runs once, when the time is up, the focus goes or the vault closes. Returns what `release` takes, for one taken away
    /// sooner by the person.
    pub fn hold(&self, hide: impl Fn() + 'static) -> u64 {
        let id = self.next.get() + 1;
        self.next.set(id);
        let hide: Rc<dyn Fn()> = Rc::new(hide);
        let this = self.clone();
        let timer = glib::timeout_add_local_once(SHOWN_FOR, move || {
            let found = this.take(id);
            if let Some(mut found) = found {
                // It is this very timer, which is over by now
                found.timer = None;
                (found.hide)();
            }
        });
        self.shown.borrow_mut().push(Shown { id, hide, timer: Some(timer) });
        id
    }

    fn take(&self, id: u64) -> Option<Shown> {
        let mut shown = self.shown.borrow_mut();
        let at = shown.iter().position(|shown| shown.id == id)?;
        Some(shown.remove(at))
    }

    /// The person hid it themselves
    pub fn release(&self, id: u64) {
        if let Some(mut found) = self.take(id) {
            if let Some(timer) = found.timer.take() {
                timer.remove();
            }
        }
    }

    pub fn hide_all(&self) {
        let all = std::mem::take(&mut *self.shown.borrow_mut());
        for mut shown in all {
            if let Some(timer) = shown.timer.take() {
                timer.remove();
            }
            (shown.hide)();
        }
    }
}

thread_local! {
    static ON_CLIPBOARD: RefCell<Option<gdk::ContentProvider>> = const { RefCell::new(None) };
    static CLEARING: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
}

/// A secret on the clipboard, as one. `glib::Bytes` frees what it holds through the owner it is given, which wipes it.
pub fn copy(text: Zeroizing<Vec<u8>>) {
    let Some(display) = gdk::Display::default() else { return };
    let bytes = glib::Bytes::from_owned(text);
    let hint = glib::Bytes::from_static(b"secret");
    // Both names, as a program asking for either should get the text
    let provider = gdk::ContentProvider::new_union(&[
        gdk::ContentProvider::for_bytes("text/plain;charset=utf-8", &bytes),
        gdk::ContentProvider::for_bytes("text/plain", &bytes),
        gdk::ContentProvider::for_bytes("x-kde-passwordManagerHint", &hint),
    ]);
    let clipboard = display.clipboard();
    if clipboard.set_content(Some(&provider)).is_err() {
        return;
    }
    ON_CLIPBOARD.with(|held| held.replace(Some(provider)));
    let clear = glib::timeout_add_local_once(SHOWN_FOR, move || {
        CLEARING.with(|timer| timer.replace(None));
        clear_if_ours();
    });
    if let Some(earlier) = CLEARING.with(|timer| timer.replace(Some(clear))) {
        earlier.remove();
    }
}

/// Empties the clipboard if what is on it is still what was copied here: something copied since is the person's
pub fn clear_if_ours() {
    let Some(ours) = ON_CLIPBOARD.with(|held| held.borrow_mut().take()) else { return };
    let Some(display) = gdk::Display::default() else { return };
    let clipboard = display.clipboard();
    if clipboard.is_local() && clipboard.content().is_some_and(|content| content == ours) {
        let _ = clipboard.set_content(gdk::ContentProvider::NONE);
    }
}

/// What a person typed, for as long as it is needed, and out of the field. GTK's own copy in the `GString` it hands back is the
/// toolkit's; a password field keeps its text in memory of its own, which it wipes.
pub fn take_text(field: &impl IsA<gtk::Editable>) -> Zeroizing<String> {
    let text = Zeroizing::new(field.text().to_string());
    field.set_text("");
    text
}

pub fn read_text(field: &impl IsA<gtk::Editable>) -> Zeroizing<String> {
    Zeroizing::new(field.text().to_string())
}
