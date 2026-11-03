//! With `AMETHYSTORA_VAULT_MEASURE` set, the budgets' numbers on standard error, and never a word of what is in the vault: how long
//! to the first frame, and how long a search took to reach the screen.

use std::cell::Cell;
use std::time::Instant;

use amethystora_ui::gtk;
use gtk::prelude::*;

thread_local! {
    static START: Cell<Option<Instant>> = const { Cell::new(None) };
    static ON: Cell<Option<bool>> = const { Cell::new(None) };
}

pub fn on() -> bool {
    ON.with(|on| match on.get() {
        Some(known) => known,
        None => {
            let known = std::env::var_os("AMETHYSTORA_VAULT_MEASURE").is_some();
            on.set(Some(known));
            known
        }
    })
}

pub fn started() {
    if on() {
        START.with(|start| start.set(Some(Instant::now())));
        eprintln!("measure: process started");
    }
}

/// The filter's own time, and the time until the next frame has been drawn
pub fn search(view: &gtk::ListView, rows: u32, started: Instant) {
    if !on() {
        return;
    }
    eprintln!("measure: search filter {} us, {rows} rows", started.elapsed().as_micros());
    let Some(clock) = view.frame_clock() else { return };
    let id = std::rc::Rc::new(Cell::new(None));
    let held = id.clone();
    let handler = clock.connect_after_paint(move |clock| {
        eprintln!("measure: search to frame {} us", started.elapsed().as_micros());
        if let Some(handler) = held.take() {
            clock.disconnect(handler);
        }
    });
    id.set(Some(handler));
}

/// How long a part of the work took, when measuring
pub fn took(what: &str, started: Instant) {
    if on() {
        eprintln!("measure: {what} {} us", started.elapsed().as_micros());
    }
}

/// From the start of the process to the first frame drawn, and to the first screen with names on it
pub fn first_frame(window: &impl IsA<gtk::Widget>) {
    if !on() {
        return;
    }
    let held = std::rc::Rc::new(Cell::new(None));
    let again = held.clone();
    window.connect_realize(move |window| {
        let Some(clock) = window.frame_clock() else { return };
        let held = again.clone();
        let handler = clock.connect_after_paint(move |clock| {
            if let Some(start) = START.with(Cell::get) {
                eprintln!("measure: first frame {} us after the start", start.elapsed().as_micros());
            }
            if let Some(handler) = held.take() {
                clock.disconnect(handler);
            }
        });
        again.set(Some(handler));
    });
}

/// The screen that says what is in the vault (or that it is closed) is up
pub fn screen_ready(what: &str) {
    if on() {
        if let Some(start) = START.with(Cell::get) {
            eprintln!("measure: {what} shown {} us after the start", start.elapsed().as_micros());
        }
    }
}
