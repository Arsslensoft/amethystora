//! The building blocks notes.js made its page of: buttons, icon buttons and links, the menus that open under
//! an anchor, the modal dialogs and the toasts, and the form controls Chromium drew (a <select>, a switch, the
//! passphrase fields with their meter). Each has the classes the page's element had, which notes.css styles.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use amethystora_ui::ui::{self, Bar};
use amethystora_ui::{gdk, glib, gtk, pango, t};
use gtk::prelude::*;

use crate::icons::icon;
use crate::layout::Fit;
use crate::sync;

/// A button with an icon before its words, `.btn` with `small`, `primary` and `danger` as the page's were
pub fn button(label: &str, glyph: Option<&str>, classes: &[&str], on_click: impl Fn() + 'static) -> gtk::Button {
    let mut all = vec!["btn"];
    all.extend_from_slice(classes);
    let size = 16;
    ui::button(label, glyph.map(|name| icon(name, size)), &all, on_click)
}

/// `.link`, quiet or small, with an icon before its words
pub fn link_button(label: &str, glyph: Option<&str>, classes: &[&str], on_click: impl Fn() + 'static) -> gtk::Button {
    let mut all = vec!["link"];
    all.extend_from_slice(classes);
    let button = ui::button(label, glyph.map(|name| icon(name, 16)), &all, on_click);
    button.set_halign(gtk::Align::Start);
    button
}

/// `.icon-btn`: an icon in a square, its label a tooltip and what a screen reader says
pub fn icon_button(glyph: &str, label: &str, classes: &[&str], on_click: impl Fn(&gtk::Button) + 'static) -> gtk::Button {
    let mark = icon(glyph, 18);
    mark.set_hexpand(true);
    let button = gtk::Button::builder().child(&mark).css_classes(["plain", "icon-btn"]).tooltip_text(label).valign(gtk::Align::Center).build();
    for class in classes {
        button.add_css_class(class);
    }
    button.set_hexpand(false);
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button.connect_clicked(move |button| on_click(button));
    button
}

/// The folder a save dialog opens in, as app.getPath gave it: the XDG folder, or the home folder when none
/// is set, which is what xdg-user-dirs' lookup answers
pub fn special_dir(which: glib::UserDirectory) -> std::path::PathBuf {
    glib::user_special_dir(which).unwrap_or_else(glib::home_dir)
}

/// aria-pressed as the class `on`
pub fn set_on(widget: &impl IsA<gtk::Widget>, on: bool) {
    if on {
        widget.add_css_class("on");
    } else {
        widget.remove_css_class("on");
    }
}

pub fn set_class(widget: &impl IsA<gtk::Widget>, class: &str, on: bool) {
    if on {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
}

/// A line of text that never wraps and is cut with an ellipsis where it ends
pub fn cut(text: &str, classes: &[&str]) -> gtk::Label {
    let label = ui::line(text, classes);
    label.set_ellipsize(pango::EllipsizeMode::End);
    label.set_hexpand(false);
    label
}

/// Text in the page's own direction: an element without dir took the page's as its paragraph's, where Pango
/// takes the first letter's, so an Arabic title in an English window would otherwise sit on the right
pub fn in_page_direction(text: &str) -> String {
    match (amethystora_ui::i18n::is_rtl_now(), pango::find_base_dir(text)) {
        (true, pango::Direction::Ltr) => format!("\u{200f}{text}"),
        (false, pango::Direction::Rtl) => format!("\u{200e}{text}"),
        _ => text.to_string(),
    }
}

/// Text laid out by itself, Arabic or English whatever the window's language is: dir="auto". A GTK widget
/// with no direction of its own takes the default one, not its parent's, so everything inside is given it,
/// but for code, which always reads from left to right.
pub fn auto_direction(widget: &impl IsA<gtk::Widget>, text: &str) {
    let direction = match pango::find_base_dir(text) {
        pango::Direction::Ltr => gtk::TextDirection::Ltr,
        pango::Direction::Rtl => gtk::TextDirection::Rtl,
        _ => gtk::TextDirection::None,
    };
    set_direction_deep(widget.upcast_ref(), direction);
}

fn set_direction_deep(widget: &gtk::Widget, direction: gtk::TextDirection) {
    widget.set_direction(direction);
    let mut child = widget.first_child();
    while let Some(current) = child {
        if !current.has_css_class("code") {
            set_direction_deep(&current, direction);
        }
        child = current.next_sibling();
    }
}

/// A colour's dot, as `.dot` with data-color
pub fn dot(color: &str, classes: &[&str]) -> gtk::Box {
    let dot = ui::hbox(0, &["dot"]);
    dot.add_css_class(&format!("color-{}", color_name(color)));
    for class in classes {
        dot.add_css_class(class);
    }
    dot.set_valign(gtk::Align::Center);
    dot.set_halign(gtk::Align::Center);
    dot.set_hexpand(false);
    dot
}

/// One of the nine colours, or violet
pub fn color_name(color: &str) -> &str {
    if crate::page::COLORS.contains(&color) { color } else { "violet" }
}

/// The tinted square a row's icon sits in, as `.row-icon`
pub fn row_icon(glyph: &str, tone: &str, size: i32) -> gtk::Box {
    let square = ui::hbox(0, &["row-icon", "tone", tone]);
    square.set_valign(gtk::Align::Center);
    square.set_hexpand(false);
    let mark = icon(glyph, size);
    mark.set_hexpand(true);
    square.append(&mark);
    square
}

// --- A <select> ------------------------------------------------------------------------------------------

/// A <select>: its choice shown with Chromium's arrow, its choices in a list under it. The page sends back
/// the value of the choice, as the select's change event did.
pub fn select(accessible: &str, choices: &[(String, String)], value: &str, classes: &[&str], on_change: impl Fn(String) + 'static) -> gtk::MenuButton {
    let list = ui::vbox(0, &[]);
    let popover = gtk::Popover::builder().has_arrow(false).css_classes(["choices"]).position(gtk::PositionType::Bottom).build();
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .propagate_natural_width(true)
        .max_content_height(360)
        .child(&list)
        .build();
    popover.set_child(Some(&scroller));
    let on_change = Rc::new(on_change);
    // As wide as its widest choice, as Chromium drew a <select>
    let words = gtk::Stack::builder().hhomogeneous(true).vhomogeneous(true).hexpand(true).build();
    let shown = choices.iter().position(|(choice, _)| choice == value).unwrap_or(0);
    for (at, (_, label)) in choices.iter().enumerate() {
        let line = cut(&in_page_direction(label), &[]);
        line.set_xalign(0.0);
        words.add_named(&line, Some(&at.to_string()));
    }
    words.set_visible_child_name(&shown.to_string());
    for (at, (choice, label)) in choices.iter().enumerate() {
        let option = gtk::Button::builder().child(&ui::line(label, &[])).css_classes(["plain", "option"]).build();
        if choice == value {
            option.add_css_class("current");
        }
        let (popover, on_change, chosen, face, others) = (popover.clone(), on_change.clone(), choice.clone(), words.clone(), list.clone());
        option.connect_clicked(move |option| {
            // The choice shows at once: a page that is not drawn again would keep saying the last one
            face.set_visible_child_name(&at.to_string());
            let mut other = others.first_child();
            while let Some(widget) = other {
                widget.remove_css_class("current");
                other = widget.next_sibling();
            }
            option.add_css_class("current");
            popover.popdown();
            on_change(chosen.clone());
        });
        list.append(&option);
    }
    let content = ui::hbox(6, &[]);
    content.append(&words);
    content.append(&crate::glyphs::select_arrow(14));
    let button = gtk::MenuButton::builder().child(&content).popover(&popover).css_classes(["plain", "select"]).valign(gtk::Align::Center).build();
    // A MenuButton draws its own button inside: the page's look is on that one
    if let Some(inner) = button.first_child() {
        inner.add_css_class("plain");
        inner.add_css_class("select");
        for class in classes {
            inner.add_css_class(class);
        }
    }
    button.remove_css_class("select");
    button.update_property(&[gtk::accessible::Property::Label(accessible)]);
    button
}

// --- A switch ------------------------------------------------------------------------------------------

/// role="switch": the knob at the start or, on, at the end
pub fn switch(on: bool, label: &str, on_change: impl Fn(bool) + 'static) -> gtk::Button {
    let knob = ui::hbox(0, &["knob"]);
    knob.set_halign(if on { gtk::Align::End } else { gtk::Align::Start });
    knob.set_valign(gtk::Align::Center);
    knob.set_hexpand(true);
    let button = gtk::Button::builder().child(&knob).css_classes(["plain", "switch"]).valign(gtk::Align::Center).halign(gtk::Align::Center).build();
    set_on(&button, on);
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button.update_state(&[gtk::accessible::State::Checked(if on { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False })]);
    let state = Rc::new(Cell::new(on));
    button.connect_clicked(move |button| {
        let next = !state.get();
        state.set(next);
        set_on(button, next);
        knob.set_halign(if next { gtk::Align::End } else { gtk::Align::Start });
        on_change(next);
    });
    button
}

// --- Menus -----------------------------------------------------------------------------------------------

/// One line of a menu: a command, with its icon or colour, and a check when it is the current choice
pub struct Item {
    pub label: String,
    pub glyph: Option<&'static str>,
    pub color: Option<String>,
    pub checked: bool,
    pub danger: bool,
    pub run: Rc<dyn Fn()>,
}

pub enum Entry {
    Item(Item),
    Separator,
}

pub fn item(label: &str, glyph: &'static str, run: impl Fn() + 'static) -> Entry {
    Entry::Item(Item { label: label.into(), glyph: Some(glyph), color: None, checked: false, danger: false, run: Rc::new(run) })
}

pub fn danger(label: &str, glyph: &'static str, run: impl Fn() + 'static) -> Entry {
    Entry::Item(Item { label: label.into(), glyph: Some(glyph), color: None, checked: false, danger: true, run: Rc::new(run) })
}

thread_local! {
    static OPEN_MENU: RefCell<Option<gtk::Popover>> = const { RefCell::new(None) };
}

pub fn close_menu() {
    if let Some(menu) = OPEN_MENU.with(|open| open.borrow_mut().take()) {
        menu.popdown();
    }
}

pub fn menu_open() -> bool {
    OPEN_MENU.with(|open| open.borrow().as_ref().is_some_and(|menu| menu.is_visible()))
}

/// A menu under the anchor, lined up with its start, as notes.js's menu() placed it
pub fn menu(anchor: &impl IsA<gtk::Widget>, entries: Vec<Entry>) {
    close_menu();
    let list = ui::vbox(0, &[]);
    let popover = gtk::Popover::builder().has_arrow(false).css_classes(["menu"]).position(gtk::PositionType::Bottom).halign(gtk::Align::Start).build();
    popover.set_offset(0, 6);
    let mut first: Option<gtk::Button> = None;
    for entry in entries {
        match entry {
            Entry::Separator => list.append(&ui::hbox(0, &["menu-sep"])),
            Entry::Item(item) => {
                let content = ui::hbox(10, &[]);
                if let Some(color) = &item.color {
                    content.append(&dot(color, &[]));
                } else if let Some(glyph) = item.glyph {
                    content.append(&icon(glyph, 16));
                } else {
                    content.append(&ui::hbox(0, &["menu-gap"]));
                }
                let words = ui::line(&item.label, &[]);
                words.set_hexpand(true);
                content.append(&words);
                if item.checked {
                    let check = icon("mark-ok", 15);
                    check.add_css_class("menu-check");
                    content.append(&check);
                }
                let button = gtk::Button::builder().child(&content).css_classes(["plain", "item"]).build();
                if item.danger {
                    button.add_css_class("danger");
                }
                let (popover, run) = (popover.clone(), item.run.clone());
                button.connect_clicked(move |_| {
                    popover.popdown();
                    let run = run.clone();
                    // After the menu has gone, as closeMenu() came before run()
                    glib::idle_add_local_once(move || run());
                });
                if first.is_none() {
                    first = Some(button.clone());
                }
                list.append(&button);
            }
        }
    }
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .propagate_natural_width(true)
        .max_content_height(480)
        .child(&list)
        .build();
    popover.set_child(Some(&scroller));
    popover.set_parent(anchor);
    // Lined up with the anchor's start, which is its right edge in a language written right to left, and
    // kept 8 pixels inside the window, as menu() placed it
    if let Some(root) = anchor.root() {
        let anchor_widget = anchor.upcast_ref::<gtk::Widget>();
        let window_width = root.width() as f64;
        // .menu { min-width: 210px; padding: 6px; border: 1px }
        let width = (scroller.measure(gtk::Orientation::Horizontal, -1).1 as f64 + 14.0).max(210.0);
        if let Some(origin) = anchor_widget.compute_point(&root, &amethystora_ui::graphene::Point::new(0.0, 0.0)) {
            let left = origin.x() as f64;
            let rtl = amethystora_ui::i18n::is_rtl_now();
            let start = if rtl { left + anchor_widget.width() as f64 - width } else { left };
            let placed = start.min(window_width - width - 8.0).max(8.0);
            // With halign start a popover's start edge meets the rectangle's: its left, or in a right to
            // left window its right
            let x = if rtl { placed + width - left - 1.0 } else { placed - left };
            popover.set_halign(gtk::Align::Start);
            popover.set_pointing_to(Some(&gdk::Rectangle::new(x.round() as i32, 0, 1, anchor_widget.height())));        }
    }
    popover.connect_closed(|popover| {
        let popover = popover.clone();
        glib::idle_add_local_once(move || popover.unparent());
    });
    OPEN_MENU.with(|open| open.replace(Some(popover.clone())));
    popover.popup();
    if let Some(first) = first {
        first.grab_focus();
    }
}

// --- Dialogs ---------------------------------------------------------------------------------------------

/// One of a dialog's buttons: what it says, and what choosing it does. `value` answers the dialog, or None
/// to keep it open for a value that is not right yet.
pub struct Action<T> {
    pub label: String,
    pub primary: bool,
    pub danger: bool,
    pub value: Rc<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<T>>>>>,
}

impl<T: 'static> Action<T> {
    pub fn plain(label: &str, value: T) -> Action<T>
    where
        T: Clone,
    {
        Action { label: label.into(), primary: false, danger: false, value: Rc::new(move || { let value = value.clone(); Box::pin(async move { Some(value) }) }) }
    }

    pub fn primary(label: &str, danger: bool, value: impl Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<T>>>> + 'static) -> Action<T> {
        Action { label: label.into(), primary: true, danger, value: Rc::new(value) }
    }
}

/// What a dialog shows: its title, its sentence, what it holds, its buttons, and whether it is the wide one
pub struct Dialog<T> {
    pub title: String,
    pub text: Option<String>,
    pub body: Vec<gtk::Widget>,
    pub actions: Vec<Action<T>>,
    pub wide: bool,
}

thread_local! {
    static LAYER: RefCell<Option<gtk::Overlay>> = const { RefCell::new(None) };
    static OPEN_DIALOGS: RefCell<Vec<(gtk::Widget, Rc<dyn Fn()>)>> = const { RefCell::new(Vec::new()) };
    static PAGE: RefCell<Option<gtk::Widget>> = const { RefCell::new(None) };
}

/// Where the dialogs open: over the whole window, the page under them out of reach
pub fn set_layer(layer: &gtk::Overlay, page: &impl IsA<gtk::Widget>) {
    LAYER.with(|cell| cell.replace(Some(layer.clone())));
    PAGE.with(|cell| cell.replace(Some(page.clone().upcast())));
}

pub fn dialog_open() -> bool {
    OPEN_DIALOGS.with(|open| !open.borrow().is_empty())
}

/// Every dialog closed, as cancelled: when the notes lock
pub fn close_dialogs() {
    let open: Vec<Rc<dyn Fn()>> = OPEN_DIALOGS.with(|open| open.borrow().iter().map(|(_, cancel)| cancel.clone()).collect());
    for cancel in open {
        cancel();
    }
}

fn page_reachable(reachable: bool) {
    PAGE.with(|cell| {
        if let Some(page) = &*cell.borrow() {
            page.set_can_target(reachable);
            page.set_can_focus(reachable);
        }
    });
}

/// A modal dialog. An action's value is what it resolves to; returning None keeps the dialog open, for a
/// value that is not right yet. Escape, as the dialog's cancel, answers None.
pub async fn modal<T: 'static>(dialog: Dialog<T>) -> Option<T> {
    let Some(layer) = LAYER.with(|cell| cell.borrow().clone()) else { return None };
    let (sender, receiver) = sync::oneshot::<Option<T>>();
    let form = ui::vbox(12, &["modal-form"]);
    form.append(&ui::label(&dialog.title, &["h2"]));
    if let Some(text) = &dialog.text {
        form.append(&ui::label(text, &["modal-text"]));
    }
    for child in &dialog.body {
        form.append(child);
    }
    let actions = ui::hbox(10, &["modal-actions"]);
    actions.set_halign(gtk::Align::End);
    let busy = Rc::new(Cell::new(false));
    let mut primary_button: Option<gtk::Button> = None;
    for action in dialog.actions {
        let mut classes = vec![];
        if action.primary {
            classes.push("primary");
        }
        if action.danger {
            classes.push("danger");
        }
        let (sender, busy, value) = (sender.clone(), busy.clone(), action.value.clone());
        let button = button(&action.label, None, &classes, move || {
            if busy.get() || sender.is_sent() {
                return;
            }
            busy.set(true);
            let (sender, busy, value) = (sender.clone(), busy.clone(), value.clone());
            glib::spawn_future_local(async move {
                let chosen = value().await;
                busy.set(false);
                if let Some(chosen) = chosen {
                    sender.send(Some(chosen));
                }
            });
        });
        if action.primary {
            primary_button = Some(button.clone());
        }
        actions.append(&button);
    }
    form.append(&actions);
    let card = Fit::new(&form, if dialog.wide { 920 } else { 460 }, if dialog.wide { 0.94 } else { 0.92 });
    card.add_css_class("modal");
    card.set_halign(gtk::Align::Center);
    card.set_valign(gtk::Align::Center);
    card.set_overflow(gtk::Overflow::Hidden);
    let backdrop = gtk::Box::builder().css_classes(["backdrop"]).hexpand(true).vexpand(true).build();
    let holder = gtk::Overlay::new();
    holder.set_child(Some(&backdrop));
    holder.add_overlay(&card);
    // Enter in a field chooses the primary action, as the form's submit did
    if let Some(primary) = primary_button.clone() {
        for child in &dialog.body {
            wire_enter(child, &primary);
        }
    }
    // Escape cancels
    let keys = gtk::EventControllerKey::new();
    let cancel_sender = sender.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            cancel_sender.send(None);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    holder.add_controller(keys);
    layer.add_overlay(&holder);
    let cancel_sender = sender.clone();
    let cancel: Rc<dyn Fn()> = Rc::new(move || cancel_sender.send(None));
    OPEN_DIALOGS.with(|open| open.borrow_mut().push((holder.clone().upcast(), cancel)));
    page_reachable(false);
    // The first field, or the primary button
    let first_field = dialog.body.iter().find_map(first_focusable);
    glib::idle_add_local_once(move || match (first_field, primary_button) {
        (Some(field), _) => {
            field.grab_focus();
        }
        (None, Some(button)) => {
            button.grab_focus();
        }
        _ => {}
    });
    let answer = receiver.await;
    layer.remove_overlay(&holder);
    OPEN_DIALOGS.with(|open| open.borrow_mut().retain(|(widget, _)| widget != holder.upcast_ref::<gtk::Widget>()));
    if !dialog_open() {
        page_reachable(true);
    }
    answer
}

fn first_focusable(widget: &gtk::Widget) -> Option<gtk::Widget> {
    if widget.is::<gtk::Entry>() || widget.is::<gtk::PasswordEntry>() || widget.is::<gtk::TextView>() {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(found) = first_focusable(&current) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}

fn wire_enter(widget: &gtk::Widget, primary: &gtk::Button) {
    if let Some(entry) = widget.downcast_ref::<gtk::Entry>() {
        let primary = primary.clone();
        entry.connect_activate(move |_| primary.emit_clicked());
        return;
    }
    if let Some(entry) = widget.downcast_ref::<gtk::PasswordEntry>() {
        let primary = primary.clone();
        entry.connect_activate(move |_| primary.emit_clicked());
        return;
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        wire_enter(&current, primary);
        child = current.next_sibling();
    }
}

/// A text field as the page's `.field`
pub fn field(placeholder: &str) -> gtk::Entry {
    let entry = gtk::Entry::builder().placeholder_text(placeholder).css_classes(["field"]).build();
    entry.update_property(&[gtk::accessible::Property::Label(placeholder)]);
    entry
}

/// A password field as the page's `.field`
pub fn password(placeholder: &str) -> gtk::Entry {
    let entry = field(placeholder);
    entry.set_visibility(false);
    entry.set_input_purpose(gtk::InputPurpose::Password);
    entry
}

/// ask(): a name, or None when it was cancelled or left empty
pub async fn ask(title: &str, label: &str, value: &str, confirm: Option<&str>, text: Option<&str>) -> Option<String> {
    let input = field(label);
    input.set_text(value);
    let reading = input.clone();
    modal(Dialog {
        title: title.into(),
        text: text.map(str::to_string),
        body: vec![input.upcast()],
        actions: vec![
            Action::plain(&t!("Cancel"), None),
            Action::primary(confirm.map(str::to_string).unwrap_or_else(|| t!("Save")).as_str(), false, move || {
                let value = reading.text().trim().to_string();
                Box::pin(async move { if value.is_empty() { None } else { Some(Some(value)) } })
            }),
        ],
        wide: false,
    })
    .await
    .flatten()
}

/// confirmDialog(): true when it was confirmed
pub async fn confirm(title: &str, text: &str, confirm: &str, danger: bool) -> bool {
    modal(Dialog {
        title: title.into(),
        text: Some(text.into()),
        body: Vec::new(),
        actions: vec![Action::plain(&t!("Cancel"), false), Action::primary(confirm, danger, || Box::pin(async { Some(true) }))],
        wide: false,
    })
    .await
    .unwrap_or(false)
}

// --- Toasts ----------------------------------------------------------------------------------------------

thread_local! {
    static TOASTS: RefCell<Option<gtk::Box>> = const { RefCell::new(None) };
}

pub fn set_toasts(toasts: &gtk::Box) {
    TOASTS.with(|cell| cell.replace(Some(toasts.clone())));
}

/// Nothing of the notes is left in the page, and nothing left to undo
pub fn clear_toasts() {
    TOASTS.with(|cell| {
        if let Some(toasts) = &*cell.borrow() {
            ui::clear(toasts);
        }
    });
}

/// A line at the bottom of the window for a few seconds, with Undo when there is something to undo
pub fn toast(message: &str, undo: Option<Rc<dyn Fn()>>) {
    let Some(toasts) = TOASTS.with(|cell| cell.borrow().clone()) else { return };
    let node = ui::hbox(16, &["toast"]);
    let words = ui::label(message, &["grow"]);
    words.set_hexpand(true);
    words.set_max_width_chars(60);
    node.append(&words);
    let limited = ui::limit(&node, 560);
    limited.set_halign(gtk::Align::Center);
    if let Some(undo) = &undo {
        let (undo, limited_weak) = (undo.clone(), limited.downgrade());
        let link = link_button(&t!("Undo"), None, &[], move || {
            undo();
            if let Some(limited) = limited_weak.upgrade() {
                if let Some(parent) = limited.parent().and_then(|p| p.downcast::<gtk::Box>().ok()) {
                    parent.remove(&limited);
                }
            }
        });
        link.set_valign(gtk::Align::Center);
        node.append(&link);
    }
    toasts.append(&limited);
    let weak = limited.downgrade();
    glib::timeout_add_local_once(Duration::from_millis(if undo.is_some() { 7000 } else { 5000 }), move || {
        if let Some(limited) = weak.upgrade() {
            if let Some(parent) = limited.parent().and_then(|p| p.downcast::<gtk::Box>().ok()) {
                parent.remove(&limited);
            }
        }
    });
}

// --- Passphrases -----------------------------------------------------------------------------------------

/// A rough count of the guesses a passphrase would take, in bits, from its length and the kinds of
/// character in it. Enough to tell a weak one from a strong one, which is all it is used for.
pub fn strength(passphrase: &str) -> (u32, &'static str, String) {
    let lower = passphrase.chars().any(|c| c.is_ascii_lowercase());
    let upper = passphrase.chars().any(|c| c.is_ascii_uppercase());
    let digit = passphrase.chars().any(|c| c.is_ascii_digit());
    let other = passphrase.chars().any(|c| !c.is_ascii_alphanumeric());
    let pool = [(lower, 26), (upper, 26), (digit, 10), (other, 33)].iter().filter(|(has, _)| *has).map(|(_, n)| *n).sum::<u32>();
    // JavaScript's length and Set of a string count UTF-16 code units
    let units: Vec<u16> = passphrase.encode_utf16().collect();
    let unique = units.iter().collect::<std::collections::HashSet<_>>().len();
    let bits = ((units.len() as f64).min(unique as f64 * 1.5) * (pool.max(1) as f64).log2()).round() as u32;
    if bits < 45 {
        (bits, "weak", t!("Weak: add more words"))
    } else if bits < 65 {
        (bits, "fair", t!("Fair: a few more words would help"))
    } else if bits < 85 {
        (bits, "good", t!("Good"))
    } else {
        (bits, "strong", t!("Strong"))
    }
}

/// The fields for choosing a passphrase, and a check that they agree
pub struct PassphraseFields {
    pub old: Option<gtk::Entry>,
    pub first: gtk::Entry,
    pub second: gtk::Entry,
    pub meter: Bar,
    pub hint: gtk::Label,
    pub error: gtk::Label,
}

impl PassphraseFields {
    pub fn new(current: bool) -> PassphraseFields {
        let old = current.then(|| password(&t!("Current passphrase")));
        let first = password(&t!("New passphrase"));
        let second = password(&t!("The same again"));
        let meter = Bar::new();
        meter.add_css_class("meter");
        meter.set_fraction(Some(0.0));
        let hint = ui::label(&t!("Use at least 8 characters. A few unrelated words make a passphrase that is long and still easy to remember."), &["hint", "small"]);
        let error = ui::label("", &["error", "small"]);
        error.set_visible(false);
        error.update_property(&[gtk::accessible::Property::Label("")]);
        let (meter_copy, hint_copy) = (meter.clone(), hint.clone());
        first.connect_changed(move |entry| {
            let text = entry.text();
            for level in ["weak", "fair", "good", "strong"] {
                meter_copy.remove_css_class(level);
            }
            if text.is_empty() {
                meter_copy.set_fraction(Some(0.0));
                return;
            }
            let (_, level, label) = strength(&text);
            meter_copy.add_css_class(level);
            meter_copy.set_fraction(Some(match level {
                "weak" => 0.25,
                "fair" => 0.5,
                "good" => 0.75,
                _ => 1.0,
            }));
            hint_copy.set_label(&label);
        });
        PassphraseFields { old, first, second, meter, hint, error }
    }

    /// In the order the page had them: the current one, the new one twice, the meter, the hint, the error
    pub fn nodes(&self) -> Vec<gtk::Widget> {
        let mut nodes: Vec<gtk::Widget> = Vec::new();
        if let Some(old) = &self.old {
            nodes.push(old.clone().upcast());
        }
        nodes.push(self.first.clone().upcast());
        nodes.push(self.second.clone().upcast());
        nodes.push(self.meter.clone().upcast());
        nodes.push(self.hint.clone().upcast());
        nodes.push(self.error.clone().upcast());
        nodes
    }

    pub fn say(&self, text: &str) {
        say(&self.error, text);
    }

    /// The new passphrase, once it is long enough and typed the same twice
    pub fn check(&self) -> Option<String> {
        let first = self.first.text().to_string();
        if first.encode_utf16().count() < 8 {
            self.say(&t!("Use at least 8 characters."));
            self.first.grab_focus();
            return None;
        }
        if first != self.second.text().as_str() {
            self.say(&t!("The two passphrases are not the same."));
            self.second.grab_focus();
            return None;
        }
        self.say("");
        Some(first)
    }
}

/// What a `.error` line says: in red what went wrong, nothing when nothing did
pub fn say(label: &gtk::Label, text: &str) {
    label.set_label(text);
    label.set_visible(!text.is_empty());
}

/// keyStatus(): in red what went wrong, plainly what it is waited for
pub fn key_status(label: &gtk::Label, text: &str, waiting: bool) {
    set_class(label, "hint", waiting);
    set_class(label, "error", !waiting);
    say(label, text);
}
