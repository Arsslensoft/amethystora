//! Making a password: random or words, with the real number of bits it was made with. It is made here, with core's generators and the
//! kernel's random source, so nothing goes over the socket until it is saved. Like anything secret, it goes from the screen after 30
//! seconds and when the window loses the focus.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::{gtk, i18n, t, ui};
use amethystora_vault_core::generate::{self, Charsets};
use amethystora_vault_core::items::Secret;
use zeroize::Zeroizing;

use super::Vault;
use crate::widgets::{self, btn};

struct State {
    words: Cell<bool>,
    length: Cell<usize>,
    count: Cell<usize>,
    sets: Cell<Charsets>,
    made: RefCell<Option<Secret>>,
    bits: Cell<f64>,
    held: Cell<u64>,
}

/// How strong so many bits are, in words
fn strength(bits: f64) -> String {
    let bits_shown = bits.floor() as u64;
    if bits >= 100.0 {
        t!("{bits} bits, very strong", bits = bits_shown)
    } else if bits >= 70.0 {
        t!("{bits} bits, strong", bits = bits_shown)
    } else {
        t!("{bits} bits, fair", bits = bits_shown)
    }
}

/// The sheet: `chosen` gets what was made and its bits
pub fn show(vault: &Rc<Vault>, chosen: impl Fn(Zeroizing<String>, f64) + 'static) {
    let sheet = widgets::sheet(&t!("Make a password"), 540);
    let state = Rc::new(State {
        words: Cell::new(false),
        length: Cell::new(generate::DEFAULT_LENGTH),
        count: Cell::new(generate::DEFAULT_WORDS),
        sets: Cell::new(Charsets::default()),
        made: RefCell::new(None),
        bits: Cell::new(0.0),
        held: Cell::new(0),
    });
    let shown = ui::label("", &["mono", "made"]);
    shown.set_direction(gtk::TextDirection::Ltr);
    shown.set_hexpand(true);
    // Not selectable: a selection is on the primary selection at once. Copy is the way out.
    let strength_chip = widgets::chip("", "ok");
    let use_it = btn(&t!("Use this password"), None, &["primary"], || {});

    // What is on the screen now follows every control, at once
    let refresh: Rc<dyn Fn()> = {
        let (state, shown, strength_chip, use_it, vault) = (state.clone(), shown.clone(), strength_chip.clone(), use_it.clone(), vault.clone());
        Rc::new(move || {
            let made = if state.words.get() { generate::words(state.count.get(), '-') } else { generate::password(state.length.get(), state.sets.get()) };
            let Ok(made) = made else {
                // No set of characters ticked
                state.made.replace(None);
                shown.set_label("");
                strength_chip.set_visible(false);
                use_it.set_sensitive(false);
                return;
            };
            shown.set_label(made.password.as_str());
            state.bits.set(made.bits);
            state.made.replace(Some(made.password));
            strength_chip.set_label(&strength(made.bits));
            strength_chip.set_visible(true);
            use_it.set_sensitive(true);
            vault.secrets.release(state.held.get());
            let (state_hide, shown_hide, chip_hide, use_hide) = (state.clone(), shown.clone(), strength_chip.clone(), use_it.clone());
            state.held.set(vault.secrets.hold(move || {
                state_hide.made.replace(None);
                state_hide.held.set(0);
                shown_hide.set_label(&t!("Hidden. Make another to see one."));
                chip_hide.set_visible(false);
                use_hide.set_sensitive(false);
            }));
        })
    };

    let (state_seg, refresh_seg) = (state.clone(), refresh.clone());
    let length_row = ui::vbox(6, &[]);
    let length_shown = ui::line("", &["mono"]);
    let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, generate::MIN_LENGTH as f64, generate::MAX_LENGTH as f64, 1.0);
    slider.set_value(generate::DEFAULT_LENGTH as f64);
    slider.set_draw_value(false);
    let sets_grid = {
        let sets_box = ui::hbox(0, &[]);
        let grid = widgets::columns(
            2,
            10,
            &[
                set_box(&t!("Capital letters"), &state, &refresh, |sets, on| sets.upper = on).upcast(),
                set_box(&t!("Small letters"), &state, &refresh, |sets, on| sets.lower = on).upcast(),
                set_box(&t!("Digits"), &state, &refresh, |sets, on| sets.digits = on).upcast(),
                set_box(&t!("Symbols"), &state, &refresh, |sets, on| sets.symbols = on).upcast(),
            ],
        );
        sets_box.append(&grid);
        grid.set_hexpand(true);
        sets_box
    };
    let (length_label, sets_seen, slider_seen, length_seen) = (ui::line(&t!("Length"), &["slider-name"]), sets_grid.clone(), slider.clone(), length_shown.clone());
    let name_seen = length_label.clone();
    let segment = widgets::seg(&[("random", t!("Random")), ("words", t!("Words"))], "random", move |value| {
        let words = value == "words";
        state_seg.words.set(words);
        sets_seen.set_visible(!words);
        name_seen.set_label(&if words { t!("How many words") } else { t!("Length") });
        // Each keeps its own number
        let (low, high, at) = if words {
            (generate::MIN_WORDS, generate::MAX_WORDS, state_seg.count.get())
        } else {
            (generate::MIN_LENGTH, generate::MAX_LENGTH, state_seg.length.get())
        };
        slider_seen.set_range(low as f64, high as f64);
        slider_seen.set_value(at as f64);
        length_seen.set_label(&i18n::number(at as f64));
        refresh_seg();
    });
    sheet.body.append(&segment);

    let value_row = ui::hbox(10, &["made-row"]);
    value_row.set_direction(gtk::TextDirection::Ltr);
    value_row.append(&shown);
    let refresh_again = refresh.clone();
    value_row.append(&widgets::icon_btn("refresh", &t!("Make another"), move || refresh_again()));
    let (state_copy, vault_copy) = (state.clone(), vault.clone());
    value_row.append(&widgets::icon_btn("copy", &t!("Copy it"), move || {
        if let Some(made) = &*state_copy.made.borrow() {
            vault_copy.copy_secret(made.as_str().as_bytes());
        }
    }));
    sheet.body.append(&value_row);

    let chips = ui::wrap(8, 8, &[]);
    chips.append(&strength_chip);
    chips.append(&widgets::chip(&t!("Worked out from how it was made"), ""));
    sheet.body.append(&chips);

    let name_row = ui::hbox(8, &[]);
    length_label.set_hexpand(true);
    name_row.append(&length_label);
    length_shown.set_label(&i18n::number(generate::DEFAULT_LENGTH as f64));
    name_row.append(&length_shown);
    length_row.append(&name_row);
    slider.update_relation(&[gtk::accessible::Relation::LabelledBy(&[length_label.upcast_ref()])]);
    length_row.append(&slider);
    sheet.body.append(&length_row);
    let (state_len, refresh_len, length_value) = (state.clone(), refresh.clone(), length_shown.clone());
    slider.connect_value_changed(move |slider| {
        let at = slider.value() as usize;
        let same = if state_len.words.get() { state_len.count.replace(at) == at } else { state_len.length.replace(at) == at };
        if !same {
            length_value.set_label(&i18n::number(at as f64));
            refresh_len();
        }
    });
    sheet.body.append(&sets_grid);

    let dialog = sheet.dialog.clone();
    let cancel = btn(&t!("Cancel"), None, &[], move || {
        dialog.close();
    });
    let (state_use, dialog) = (state.clone(), sheet.dialog.clone());
    use_it.connect_clicked(move |_| {
        if let Some(made) = state_use.made.borrow().as_ref() {
            chosen(Zeroizing::new(made.as_str().to_string()), state_use.bits.get());
        }
        dialog.close();
    });
    sheet.pin(&widgets::actions(&[cancel.upcast(), use_it.upcast()]));
    let (state_end, vault_end) = (state.clone(), vault.clone());
    sheet.dialog.connect_closed(move |_| {
        vault_end.secrets.release(state_end.held.replace(0));
        state_end.made.replace(None);
    });
    sheet.dialog.present(Some(&vault.shell.window));
    refresh();
}

fn set_box(name: &str, state: &Rc<State>, refresh: &Rc<dyn Fn()>, set: fn(&mut Charsets, bool)) -> gtk::CheckButton {
    let (state, refresh) = (state.clone(), refresh.clone());
    widgets::check(name, true, move |on| {
        let mut sets = state.sets.get();
        set(&mut sets, on);
        state.sets.set(sets);
        refresh();
    })
}
