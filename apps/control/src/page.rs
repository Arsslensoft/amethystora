//! The page of Amethystora Control, as control.js drew it. What it shows is the list backend.rs reads from
//! /usr/libexec/amethystora-control-list: every entry, its words, its state and its choices. A change hands
//! back the entry's id and the value of a choice the list gave, and backend.rs runs the command the list
//! has for it. An entry's words are the script's, already in the session's language through gettext, and
//! are shown as they come; only the page's own words are looked up in its catalog. Everything is built as
//! widgets with text in them, never as markup.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use amethystora_ui::gem::Gem;
use amethystora_ui::i18n::{self, DateStyle, Unit, Width};
use amethystora_ui::{adw, gdk, gio, glib, gtk, process, t, ui};
use glib::Unichar;
use gtk::prelude::*;
use serde_json::Value;

use crate::backend::{self, Item, Outcome, PICTURES};
use crate::icons::icon;
use amethystora_control_core::{Clash, Keybinding};
use crate::layout::{Aspect, AutoGrid, Flex, Grow, Pretty, SwatchBar};
use crate::{Banner, Control, Diff};

/// A colour of the list, when it is one: #rrggbb
fn hex(color: Option<&String>) -> Option<(f64, f64, f64)> {
    let color = color?;
    let digits = color.strip_prefix('#').filter(|d| d.len() == 6 && d.chars().all(|c| c.is_ascii_hexdigit()))?;
    let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).map(f64::from).ok();
    Some((channel(0)?, channel(2)?, channel(4)?))
}

fn css_color((r, g, b): (f64, f64, f64)) -> String {
    format!("#{:02x}{:02x}{:02x}", r.round() as u8, g.round() as u8, b.round() as u8)
}

fn mix(a: (f64, f64, f64), b: (f64, f64, f64), t: f64) -> (f64, f64, f64) {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, a.2 + (b.2 - a.2) * t)
}

fn contrast(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    let luminance = |(r, g, b): (f64, f64, f64)| {
        let channel = |c: f64| {
            let c = c / 255.0;
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    };
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// The preview's colours for a theme, as palette.awk works them out: the bar on the header bar's ground (the ground
/// mixed 6% toward the text on a dark theme, 35% toward white on a light one) at 80%, the window on the ground at 90%,
/// and the accent's label white, else the ground, else the text, the first that reads at 4.5:1
fn preview_css(choice: &amethystora_control_core::Choice) -> String {
    let get = |key: &str| hex(choice.colors.get(key));
    let (Some(background), Some(foreground)) = (get("background"), get("foreground")) else { return String::new() };
    let accent = get("accent").unwrap_or(foreground);
    let white = (255.0, 255.0, 255.0);
    let headerbar = if choice.light { mix(background, white, 0.35) } else { mix(background, foreground, 0.06) };
    let label = [white, background, foreground].into_iter().find(|&c| contrast(c, accent) >= 4.5).unwrap_or(white);
    let (bg, fg, accent, label, headerbar) =
        (css_color(background), css_color(foreground), css_color(accent), css_color(label), css_color(headerbar));
    format!(
        ".theme-preview .pv-bar {{ background-color: alpha({headerbar}, 0.8); color: {fg}; }}\n\
         .theme-preview .pv-pill, .theme-preview .pv-add {{ background-color: {accent}; color: {label}; }}\n\
         .theme-preview .pv-check {{ background-color: {accent}; }}\n\
         .theme-preview .pv-window {{ background-color: alpha({bg}, 0.9); color: {fg}; }}\n"
    )
}

/// A palette the theme maker starts from, with every key a theme has
fn start_palette(start: &amethystora_control_core::Start) -> std::collections::BTreeMap<String, String> {
    amethystora_control_core::contrast::complete(&start.colors)
}

/// How a button looks, as the page's button() was told
#[derive(Debug, Clone, Copy, Default)]
struct Look {
    icon: Option<&'static str>,
    small: bool,
    primary: bool,
    danger: bool,
    disabled: bool,
}

/// The pages of the sidebar, with what each is called and says about itself
fn page_words(page: &str) -> (String, String) {
    match page {
        "appearance" => (t!("Appearance"), t!("The theme, the wallpaper, and how a new one is revealed.")),
        "desktop" => (t!("Desktop"), t!("How windows are arranged, the top bar and the terminal.")),
        "keyboard" => (t!("Keyboard"), t!("Every shortcut in one place, and the keyboard settings GNOME keeps out of sight.")),
        "apps" => (t!("Apps"), t!("Games, Android apps, drawing tablets and web apps, and what else installs with one command.")),
        "agent" => (t!("The AI agent"), t!("An AI agent that knows this system, and which one opens.")),
        "system" => (t!("System"), t!("The image this machine runs and the stream it follows, the CPU scheduler, and the firmware.")),
        "setup" => (t!("Your setup"), t!("Your setup as a file, to keep, to compare, and to apply here or on another machine.")),
        _ => (String::new(), String::new()),
    }
}

/// The areas of a setup file's differences (ame setup diff), by the names the script gives them
fn area(name: &str) -> Option<String> {
    Some(match name {
        "image" => t!("System image"),
        "flatpak" => t!("App"),
        "tap" => t!("Homebrew tap"),
        "formula" => t!("Homebrew formula"),
        "cask" => t!("Homebrew cask"),
        "container" => t!("Container"),
        "packages" => t!("Packages in a container"),
        "extension" => t!("Extension"),
        "theme" => t!("Theme"),
        "layout" => t!("Window layout"),
        "favorites" => t!("The dock's apps"),
        "keybinding" => t!("Keybinding"),
        _ => return None,
    })
}

/// A box with a role a screen reader is told, which only a new widget can be given
fn role_box(orientation: gtk::Orientation, spacing: i32, classes: &[&str], role: gtk::AccessibleRole) -> gtk::Box {
    let container = gtk::Box::builder().orientation(orientation).spacing(spacing).accessible_role(role).build();
    for class in classes {
        container.add_css_class(class);
    }
    container
}

/// A heading, which wraps as ui::label's text does; its role can only be given as it is made
pub(crate) fn heading(text: &str, class: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .accessible_role(gtk::AccessibleRole::Heading)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .natural_wrap_mode(gtk::NaturalWrapMode::None)
        .xalign(0.0)
        .halign(gtk::Align::Fill)
        .css_classes([class])
        .build()
}

fn centered(label: gtk::Label) -> gtk::Label {
    label.set_justify(gtk::Justification::Center);
    label.set_xalign(0.5);
    label
}

/// A paragraph the page gave `text-wrap: pretty`, which never leaves one word alone on its last line
fn pretty(text: &str, classes: &[&str]) -> Pretty {
    Pretty::new(&ui::label(text, classes))
}

/// Text the page sets left to right in every language: paths, commands, ids
fn ltr(label: gtk::Label) -> gtk::Label {
    label.set_direction(gtk::TextDirection::Ltr);
    label
}

/// One line of a row, cut short with an ellipsis rather than wrapped
fn row_line(text: &str, classes: &[&str]) -> gtk::Label {
    let label = ui::line(text, classes);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_hexpand(true);
    label
}

/// Milliseconds as seconds, the language's way: "1.2 sec"
fn seconds(ms: f64) -> String {
    // t.number(ms / 1000, { style: 'unit', unit: 'second', unitDisplay: 'short', maximumFractionDigits: 1 })
    i18n::unit(ms / 1000.0, Unit::Second, Width::Short, 0, 1)
}

/// A date and time, the language's way
fn when(seconds: f64) -> String {
    i18n::date(seconds, DateStyle::MediumShort)
}

/// Letters without their accents, in lower case, so that "écran" is found by "ecran"
pub fn plain(text: &str) -> String {
    glib::normalize(text, glib::NormalizeMode::Default).chars().filter(|c| !c.is_mark()).collect::<String>().to_lowercase()
}

/// What a card was drawn from: all a card of the plain kinds shows
#[derive(PartialEq)]
pub(crate) struct Shown {
    item: Item,
    busy: bool,
    said: Option<(bool, String)>,
}

/// The page in the window, by its cards
#[derive(Default)]
pub(crate) struct Drawn {
    page: &'static str,
    ids: Vec<String>,
    cards: HashMap<String, (Shown, gtk::Box)>,
}

/// The kinds of card that also show what is being typed or picked in them, drawn again with every change
fn reads_more(kind: &str) -> bool {
    !matches!(kind, "switch" | "choice" | "range" | "link" | "action" | "themes" | "wallpapers")
}

/// The cards drawn at once when a page opens; the others follow, a few each frame
const CARDS_AT_ONCE: usize = 3;
const CARDS_PER_FRAME: usize = 2;
/// The height a card has at the least, for how many it takes to reach a place in a page that is scrolled
const CARD_AT_LEAST: f64 = 90.0;

/// The most settings a search draws at once
const SEARCH_CARDS: usize = 30;

fn matches(item: &Item, words: &[String]) -> bool {
    let mut parts: Vec<String> = vec![
        item.title.clone(),
        item.text.clone().unwrap_or_default(),
        item.detail.clone().unwrap_or_default(),
        item.group.clone().unwrap_or_default(),
        item.usage.clone().unwrap_or_default(),
        page_words(&item.page).0,
    ];
    parts.extend(item.choices.iter().map(|choice| format!("{} {}", choice.title, choice.text.clone().unwrap_or_default())));
    // Every shortcut is found by its words, and its card then shows the ones that match
    parts.extend(item.keybindings.iter().map(|binding| binding.title.clone()));
    let haystack = plain(&parts.join(" "));
    words.iter().all(|word| haystack.contains(word.as_str()))
}

// --- The Keyboard page's shortcuts ------------------------------------------------------------------------

/// The most rows the shortcuts' card draws at once; its search finds the rest
const KEYBINDING_ROWS: usize = 150;
const KEYBINDINGS_AT_ONCE: usize = 10;
const KEYBINDINGS_PER_FRAME: usize = 3;

/// A shortcut's keys as GTK names them in the session's language, each its own cap: Super, Alt, V
fn key_caps(accel: &str) -> Vec<String> {
    let label = gtk::accelerator_parse(accel)
        .map(|(key, mods)| gtk::accelerator_get_label(key, mods).to_string())
        .unwrap_or_else(|| accel.to_string());
    split_label(&label)
}

/// "Ctrl+Alt+T" as its caps; "Ctrl++" is Ctrl and the plus key
fn split_label(label: &str) -> Vec<String> {
    let mut caps = Vec::new();
    let mut rest = label;
    while let Some(at) = rest.find('+').filter(|at| *at > 0) {
        caps.push(rest[..at].to_string());
        rest = &rest[at + 1..];
    }
    caps.push(rest.to_string());
    caps.retain(|cap| !cap.is_empty());
    caps
}

/// Whose a shortcut is, as its tag says: Yours once the account changed it
fn owner_words(binding: &Keybinding) -> String {
    if binding.changed {
        return t!("Yours");
    }
    match binding.owner.as_str() {
        "gnome" => t!("GNOME"),
        "tiling" => t!("Tiling"),
        "custom" => t!("Yours"),
        _ => t!("Desktop"),
    }
}

/// The line under a shortcut that clashes, naming the other one and whose it is
fn clash_words(clash: &Clash) -> String {
    match clash.owner.as_str() {
        "gnome" => t!("Clash: GNOME's {title}", title = &clash.title),
        "tiling" => t!("Clash: the tiling's {title}", title = &clash.title),
        "custom" => t!("Clash: your {title}", title = &clash.title),
        _ => t!("Clash: the desktop's {title}", title = &clash.title),
    }
}

/// What a shortcut is found by: its words, whose it is, its keys and its key's name
fn keybinding_words(binding: &Keybinding) -> String {
    let caps: Vec<String> = binding.value.iter().flat_map(|accel| key_caps(accel)).collect();
    plain(&format!("{} {} {} {}", binding.title, owner_words(binding), caps.join(" "), binding.key))
}

/// The keys recorded as a shortcut holds them, such as <Super><Alt>v: None for a modifier alone, which is waited past
pub fn recorded_accel(key: gdk::Key, modifiers: gdk::ModifierType) -> Option<String> {
    let lone = [
        gdk::Key::Shift_L, gdk::Key::Shift_R, gdk::Key::Control_L, gdk::Key::Control_R, gdk::Key::Alt_L, gdk::Key::Alt_R,
        gdk::Key::Super_L, gdk::Key::Super_R, gdk::Key::Meta_L, gdk::Key::Meta_R, gdk::Key::Hyper_L, gdk::Key::Hyper_R,
        gdk::Key::ISO_Level3_Shift, gdk::Key::Caps_Lock, gdk::Key::Num_Lock,
    ];
    if lone.contains(&key) {
        return None;
    }
    let mods = modifiers & gtk::accelerator_get_default_mod_mask();
    Some(gtk::accelerator_name(key.to_lower(), mods).to_string())
}

// --- What one difference of the setup file is about -----------------------------------------------------

fn value_text(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn value_words(value: Option<&Value>) -> Option<Vec<String>> {
    value?.as_array().map(|list| list.iter().map(|word| value_text(Some(word)).unwrap_or_default()).collect())
}

/// What one difference between the machine and the setup file is about, in the file's own names
pub fn change_name(change: &Value) -> String {
    let field = |name: &str| change.get(name);
    match field("area").and_then(Value::as_str).unwrap_or("") {
        "packages" => format!("{}: {}", value_text(field("container")).unwrap_or_default(), value_words(field("packages")).unwrap_or_default().join(", ")),
        "favorites" => value_words(field("to")).or_else(|| value_words(field("from"))).unwrap_or_default().join(", "),
        "keybinding" => {
            let key = value_text(field("key")).unwrap_or_default();
            match value_text(field("schema")).as_deref() {
                Some("custom") => key,
                schema => format!("{} {key}", schema.unwrap_or_default()),
            }
        }
        _ => ["app", "uuid", "key", "name"]
            .iter()
            .find_map(|name| value_text(field(name)))
            .or_else(|| field("to").and_then(Value::as_str).filter(|to| !to.is_empty()).map(str::to_string))
            .unwrap_or_default(),
    }
}

/// cannot, add, set or remove
pub fn change_kind(change: &Value) -> String {
    let ok = match change.get("ok") {
        Some(Value::Bool(b)) => *b,
        Some(Value::Null) | None => false,
        Some(_) => true,
    };
    if ok { change.get("action").and_then(Value::as_str).unwrap_or("").to_string() } else { "cannot".into() }
}

fn change_order(kind: &str) -> u8 {
    match kind {
        "cannot" => 0,
        "add" => 1,
        "set" => 2,
        "remove" => 3,
        _ => 4,
    }
}

/// The field to try text shortcuts in: each change of its text, against the one before, says when an abbreviation and
/// its space were taken away and a text put in their place. Only the line under it is drawn again, so that the field
/// keeps the keyboard; what it holds is the window's memory only
fn try_field_watch(field: &gtk::Entry, me: std::rc::Weak<Control>, abbrs: Vec<String>, line: glib::WeakRef<gtk::Label>) {
    field.connect_changed(move |field| {
        let Some(this) = me.upgrade() else { return };
        let now = field.text().to_string();
        let mut state = this.state.borrow_mut();
        let waiting = state.try_waiting.take();
        let seen = backend::tried(&state.try_text, &now, waiting.as_ref().map(|(abbr, left)| (abbr.as_str(), left.as_str())), &abbrs);
        state.try_text = now;
        match seen {
            backend::Tried::Waiting { abbr, left } => state.try_waiting = Some((abbr, left)),
            backend::Tried::Expanded { abbr, text } => {
                let text = backend::first_line(&text);
                if let Some(line) = line.upgrade() {
                    line.set_text(&t!("Expanded: {abbr} became “{text}”.", abbr = &abbr, text = &text));
                }
                state.try_said = Some((abbr, text));
            }
            backend::Tried::Nothing => {}
        }
    });
}

impl Control {
    // --- Building blocks ----------------------------------------------------------------------------------

    fn button(&self, label: &str, look: Look, on_click: impl Fn(&Rc<Control>) + 'static) -> gtk::Button {
        let mut classes = vec!["btn"];
        if look.small {
            classes.push("small");
        }
        if look.primary {
            classes.push("primary");
        }
        if look.danger {
            classes.push("danger");
        }
        let me = self.me.clone();
        let button = ui::button(label, look.icon.map(|name| icon(name, 16)), &classes, move || {
            if let Some(this) = me.upgrade() {
                on_click(&this);
            }
        });
        button.set_sensitive(!look.disabled);
        button.set_valign(gtk::Align::Center);
        button
    }

    fn link_button(&self, label: &str, name: &'static str, on_click: impl Fn(&Rc<Control>) + 'static) -> gtk::Button {
        let me = self.me.clone();
        let button = ui::button(label, Some(icon(name, 15)), &["link"], move || {
            if let Some(this) = me.upgrade() {
                on_click(&this);
            }
        });
        button.set_halign(gtk::Align::Start);
        button.set_valign(gtk::Align::Center);
        button
    }

    /// A button that asks once more before it does what it says, for what cannot be put back by clicking
    /// again
    fn confirm_button(&self, label: &str, ask: &str, name: &'static str, disabled: bool, on_confirm: impl Fn(&Rc<Control>) + 'static) -> gtk::Button {
        let button = ui::button(label, Some(icon(name, 15)), &["btn", "small"], || {});
        button.set_sensitive(!disabled);
        button.set_valign(gtk::Align::Center);
        let armed: Rc<std::cell::RefCell<Option<glib::SourceId>>> = Rc::default();
        let (label, ask) = (label.to_string(), ask.to_string());
        let me = self.me.clone();
        button.connect_clicked(move |button| {
            if let Some(timer) = armed.borrow_mut().take() {
                timer.remove();
                if let Some(this) = me.upgrade() {
                    on_confirm(&this);
                }
                return;
            }
            button.add_css_class("danger");
            if let Some(words) = ui::button_words(button) {
                words.set_label(&ask);
            }
            let weak = button.downgrade();
            let (label, again) = (label.clone(), armed.clone());
            let timer = glib::timeout_add_local_once(std::time::Duration::from_secs(4), move || {
                again.borrow_mut().take();
                if let Some(button) = weak.upgrade() {
                    button.remove_css_class("danger");
                    if let Some(words) = ui::button_words(&button) {
                        words.set_label(&label);
                    }
                }
            });
            armed.replace(Some(timer));
        });
        button
    }

    /// A button that says it copied, for a moment
    fn copy_button(text: &str) -> gtk::Button {
        let button = ui::copy_button(text, icon("copy", 15), &t!("Copy"), &t!("Copied"), &t!("Copy"), &["btn", "small"]);
        button.set_valign(gtk::Align::Center);
        button
    }

    fn command_block(command: &str) -> gtk::Box {
        let block = ui::hbox(10, &["command"]);
        let code = ltr(ui::label(command, &["code"]));
        code.set_hexpand(true);
        code.set_selectable(true);
        code.set_wrap_mode(gtk::pango::WrapMode::Char);
        block.append(&code);
        block.append(&Self::copy_button(command));
        block
    }

    fn callout(tone: &str, title: Option<&str>, text: Option<&str>, extra: Vec<gtk::Widget>) -> gtk::Box {
        let callout = ui::hbox(12, &["callout", tone]);
        let mark = icon(if tone == "check" { "alert" } else { "info" }, 18);
        mark.set_valign(gtk::Align::Start);
        callout.append(&mark);
        let body = ui::vbox(6, &["grow"]);
        body.set_hexpand(true);
        if let Some(title) = title {
            body.append(&ui::label(title, &["strong"]));
        }
        if let Some(text) = text {
            body.append(&pretty(text, &["p"]));
        }
        for child in extra {
            body.append(&child);
        }
        callout.append(&body);
        callout
    }

    fn actions(children: Vec<gtk::Widget>) -> adw::WrapBox {
        let actions = ui::wrap(8, 8, &["actions"]);
        for child in children {
            actions.append(&child);
        }
        actions
    }

    /// The spinning mark of something running, in a button's place
    fn spinner() -> gtk::Box {
        let place = role_box(gtk::Orientation::Horizontal, 0, &["icon-btn"], gtk::AccessibleRole::Status);
        place.update_property(&[gtk::accessible::Property::Label(&t!("Running"))]);
        place.set_hexpand(false);
        place.set_valign(gtk::Align::Start);
        let mark = icon("spinner", 18);
        mark.set_hexpand(true);
        mark.set_spinning(true);
        place.append(&mark);
        place
    }

    /// A small button with only an icon, which says what it does to a screen reader
    fn icon_button(&self, name: &'static str, size: i32, label: &str, on_click: impl Fn(&Rc<Control>) + 'static) -> gtk::Button {
        let mark = icon(name, size);
        let button = gtk::Button::builder().child(&mark).css_classes(["plain", "icon-btn"]).valign(gtk::Align::Center).build();
        button.update_property(&[gtk::accessible::Property::Label(label)]);
        let me = self.me.clone();
        button.connect_clicked(move |_| {
            if let Some(this) = me.upgrade() {
                on_click(&this);
            }
        });
        button
    }

    /// The page's .switch: a track and a knob, which the next render moves once the change is made
    fn switch(&self, on: bool, label: &str, disabled: bool, on_click: impl Fn(&Rc<Control>) + 'static) -> gtk::Button {
        let knob = ui::hbox(0, &["knob"]);
        knob.set_halign(if on { gtk::Align::End } else { gtk::Align::Start });
        knob.set_valign(gtk::Align::Center);
        let button = gtk::Button::builder().accessible_role(gtk::AccessibleRole::Switch).child(&knob).css_classes(["plain", "switch"]).build();
        if on {
            button.add_css_class("checked");
        }
        button.set_halign(gtk::Align::Start);
        button.set_valign(gtk::Align::Start);
        button.update_state(&[gtk::accessible::State::Checked(if on { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False })]);
        button.update_property(&[gtk::accessible::Property::Label(label)]);
        button.set_sensitive(!disabled);
        let me = self.me.clone();
        button.connect_clicked(move |_| {
            if let Some(this) = me.upgrade() {
                on_click(&this);
            }
        });
        button
    }

    /// One of a set of choices, which says whether it is the one chosen
    fn radio(child: &impl IsA<gtk::Widget>, classes: &[&str], chosen: bool) -> gtk::Button {
        let button = gtk::Button::builder().accessible_role(gtk::AccessibleRole::Radio).child(child).build();
        button.add_css_class("plain");
        for class in classes {
            button.add_css_class(class);
        }
        if chosen {
            button.add_css_class("checked");
        }
        button.update_state(&[gtk::accessible::State::Checked(if chosen { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False })]);
        button
    }

    // --- Running what the list says ---------------------------------------------------------------------

    pub(crate) fn change(self: &Rc<Self>, id: &str, value: &str) {
        let this = self.clone();
        let (key, value) = (id.to_string(), value.to_string());
        self.perform(id, async move { backend::perform(backend::change(this.known().as_deref(), &key, &value)).await });
    }

    pub(crate) fn act(self: &Rc<Self>, id: &str) {
        let this = self.clone();
        let key = id.to_string();
        self.perform(id, async move { backend::perform(backend::act(this.known().as_deref(), &key)).await });
    }

    fn widget(self: &Rc<Self>, card: &str, action: &str, widget: &str) {
        let this = self.clone();
        let (action, widget) = (action.to_string(), widget.to_string());
        self.perform(card, async move { backend::perform(backend::widget(this.known().as_deref(), &action, &widget)).await });
    }

    /// A top bar item moved (the bar entry), or the bar put back as the image has it (id None)
    fn bar_place(self: &Rc<Self>, card: &str, place: Option<(String, &'static str, usize)>) {
        let this = self.clone();
        self.perform(card, async move {
            let list = this.known();
            let plan = match &place {
                Some((id, zone, position)) => backend::bar_place(list.as_deref(), id, zone, *position),
                None => backend::bar_reset(list.as_deref()),
            };
            backend::perform(plan).await
        });
    }

    fn webapp(self: &Rc<Self>, card: &str, action: &str, name: &str) {
        let this = self.clone();
        let (action, name) = (action.to_string(), name.to_string());
        self.perform(card, async move { backend::perform(backend::webapp(this.known().as_deref(), &action, &name)).await });
    }

    /// A text shortcut added or removed. Saved under another abbreviation than the one being edited, the old one is
    /// removed once the new one is in. The form is emptied only once the helper took it
    fn shortcut(self: &Rc<Self>, card: &str, action: &str, abbr: &str, text: &str) {
        let this = self.clone();
        let (action, abbr, text) = (action.to_string(), abbr.to_string(), text.to_string());
        let editing = self.state.borrow().shortcut_editing.clone().filter(|old| *old != abbr && action == "add");
        self.perform(card, async move {
            let outcome = backend::perform(backend::shortcut(this.known().as_deref(), &action, &abbr, &text)).await;
            let took = matches!(outcome, Outcome::Ran { ok: true, .. });
            if took && action == "add" {
                let mut state = this.state.borrow_mut();
                state.shortcut_abbr.clear();
                state.shortcut_text.clear();
                state.shortcut_editing = None;
            }
            match editing {
                Some(old) if took => backend::perform(backend::shortcut(this.known().as_deref(), "remove", &old, "")).await,
                _ => outcome,
            }
        });
    }

    /// Edit: the shortcut's text, read now and only now, into the form
    fn edit_shortcut(self: &Rc<Self>, card: &str, abbr: &str) {
        let this = self.clone();
        let (card, abbr) = (card.to_string(), abbr.to_string());
        glib::spawn_future_local(async move {
            match backend::shortcut_text(this.known(), &abbr).await {
                Ok(text) => {
                    let mut state = this.state.borrow_mut();
                    state.shortcut_abbr = abbr.clone();
                    state.shortcut_text = text;
                    state.shortcut_editing = Some(abbr);
                    state.said.remove(&card);
                }
                Err(error) => {
                    this.state.borrow_mut().said.insert(card, crate::Said { ok: false, output: error });
                }
            }
            this.render();
        });
    }

    fn pool(self: &Rc<Self>, card: &str, action: &str, disk: &str) {
        let this = self.clone();
        let (action, disk) = (action.to_string(), disk.to_string());
        self.perform(card, async move { backend::perform(backend::pool(this.known().as_deref(), &action, &disk)).await });
    }

    fn row_action(self: &Rc<Self>, card: &str, action: &str, row: &str) {
        let this = self.clone();
        let (card_id, action, row) = (card.to_string(), action.to_string(), row.to_string());
        self.perform(card, async move { backend::perform(backend::row(this.known().as_deref(), &card_id, &action, &row)).await });
    }

    fn setup(self: &Rc<Self>, card: &str, action: &str) {
        let this = self.clone();
        let action = action.to_string();
        self.perform(card, async move { backend::perform(backend::setup(this.known().as_deref(), &action)).await });
    }

    // --- One entry --------------------------------------------------------------------------------------

    pub(crate) fn busy(&self, id: &str) -> bool {
        self.state.borrow().busy.contains(id)
    }

    fn terminal_tag() -> gtk::Box {
        let tag = ui::hbox(5, &["tag"]);
        tag.set_tooltip_text(Some(&t!("Opens in a terminal, where it asks before it changes anything")));
        tag.set_valign(gtk::Align::Center);
        tag.append(&icon("terminal", 13));
        tag.append(&ui::line(&t!("Terminal"), &[]));
        tag
    }

    /// What the last command run from this card said
    pub(crate) fn said_box(&self, item: &Item) -> Option<gtk::Widget> {
        let said = self.state.borrow().said.get(&item.id).cloned()?;
        let classes: &[&str] = if said.ok { &["said"] } else { &["said", "failed"] };
        let role = if said.ok { gtk::AccessibleRole::Status } else { gtk::AccessibleRole::Alert };
        let block = role_box(gtk::Orientation::Vertical, 6, classes, role);
        let head = ui::hbox(8, &["said-head"]);
        head.append(&icon(if said.ok { "check" } else { "alert" }, 15));
        let title = ui::label(&if said.ok { t!("Done") } else { t!("That did not work") }, &["grow"]);
        title.set_hexpand(true);
        head.append(&title);
        let id = item.id.clone();
        head.append(&self.icon_button("close", 15, &t!("Dismiss"), move |this| {
            this.state.borrow_mut().said.remove(&id);
            this.render();
        }));
        block.append(&head);
        if !said.output.is_empty() {
            let output = ltr(ui::label(&said.output, &["pre", "code"]));
            output.set_selectable(true);
            output.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            output.set_valign(gtk::Align::Start);
            let scroller = gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Never)
                .max_content_height(180)
                .propagate_natural_height(true)
                .child(&output)
                .build();
            block.append(&scroller);
        }
        Some(block.upcast())
    }

    /// A card: the entry's words and state, its control at the inline end, whatever it shows below them,
    /// and where it is in the manual and in a terminal
    fn card_frame(&self, item: &Item, control: Option<gtk::Widget>, below: Vec<gtk::Widget>, danger: bool, tags: Vec<gtk::Widget>) -> gtk::Box {
        let mut classes = vec!["setting", "card"];
        if !item.available {
            classes.push("unavailable");
        }
        if danger {
            classes.push("danger");
        }
        let card = ui::vbox(14, &classes);
        card.update_property(&[gtk::accessible::Property::Label(&item.title)]);
        let head = ui::hbox(16, &["setting-head"]);
        let words = ui::vbox(3, &["setting-words"]);
        words.set_hexpand(true);
        let title = Flex::new(8, 8, &["setting-title"]);
        title.append(&heading(&item.title, "h3"));
        for tag in tags {
            title.append(&tag);
        }
        words.append(&title);
        if let Some(text) = &item.text {
            words.append(&ui::limit(&pretty(text, &["setting-text"]), 620));
        }
        if let (false, Some(reason)) = (item.available, &item.reason) {
            words.append(&ui::label(reason, &["setting-detail", "check"]));
        }
        if let Some(detail) = &item.detail {
            words.append(&ui::label(detail, &["setting-detail"]));
        }
        head.append(&words);
        if self.busy(&item.id) {
            head.append(&Self::spinner());
        } else if let Some(control) = control {
            control.set_valign(gtk::Align::Start);
            head.append(&control);
        }
        card.append(&head);
        for child in below {
            card.append(&child);
        }
        if let Some(said) = self.said_box(item) {
            card.append(&said);
        }
        // A link's own button is its way to the manual
        let manual = item.manual.clone().filter(|_| item.kind != "link");
        if manual.is_some() || item.usage.is_some() {
            let foot = Flex::new(16, 6, &["setting-foot"]);
            if let Some(page) = manual {
                foot.append(&self.link_button(&t!("In the manual"), "book", move |_| process::manual(&page)));
            }
            if let Some(usage) = &item.usage {
                let line = ui::hbox(0, &[]);
                line.append(&ui::line(&format!("{} ", t!("In a terminal:")), &[]));
                let code = ltr(ui::label(usage, &["code"]));
                code.set_wrap_mode(gtk::pango::WrapMode::Char);
                code.set_hexpand(true);
                line.append(&code);
                foot.append(&line);
            }
            card.append(&foot);
        }
        card
    }

    /// Whether the next click on a switch, the one away from its state, opens a terminal
    fn next_opens_terminal(item: &Item) -> bool {
        item.choices.iter().find(|choice| Some(&choice.value) != item.state.as_ref()).is_some_and(|choice| choice.terminal)
    }

    fn switch_card(&self, item: &Item) -> gtk::Box {
        let busy = self.busy(&item.id) || !item.available;
        let id = item.id.clone();
        let control: gtk::Widget = match item.state.as_deref() {
            Some(state @ ("on" | "off")) => {
                let on = state == "on";
                self.switch(on, &item.title, busy, move |this| this.change(&id, if on { "off" } else { "on" })).upcast()
            }
            // A state only an administrator can see: both ways, each in a terminal
            _ => {
                let (on, off) = (id.clone(), id.clone());
                Self::actions(vec![
                    self.button(&t!("Turn on…"), Look { small: true, disabled: busy, ..Default::default() }, move |this| this.change(&on, "on")).upcast(),
                    self.button(&t!("Turn off…"), Look { small: true, disabled: busy, ..Default::default() }, move |this| this.change(&off, "off")).upcast(),
                ])
                .upcast()
            }
        };
        let tags = if item.available && Self::next_opens_terminal(item) { vec![Self::terminal_tag().upcast()] } else { Vec::new() };
        self.card_frame(item, Some(control), Vec::new(), false, tags)
    }

    fn choice_card(&self, item: &Item) -> gtk::Box {
        // Colours are chosen from a grid of dots, as the boards draw the twelve accents
        if !item.choices.is_empty() && item.choices.iter().all(|choice| choice.color.is_some()) {
            return self.colors_card(item);
        }
        let disabled = self.busy(&item.id) || !item.available;
        let options = AutoGrid::new(220, 8, &["options"]);
        options.update_property(&[gtk::accessible::Property::Label(&item.title)]);
        for choice in &item.choices {
            let chosen = Some(&choice.value) == item.state.as_ref();
            let content = ui::hbox(10, &[]);
            let dot = ui::hbox(0, &["dot"]);
            dot.set_valign(gtk::Align::Start);
            dot.set_hexpand(false);
            if chosen {
                let fill = ui::hbox(0, &["dot-fill"]);
                fill.set_halign(gtk::Align::Center);
                fill.set_valign(gtk::Align::Center);
                fill.set_hexpand(true);
                dot.append(&fill);
            }
            content.append(&dot);
            let words = ui::vbox(1, &["option-words"]);
            words.set_hexpand(true);
            let title = ui::label(&choice.title, &["strong"]);
            // A font is shown in itself
            if let Some(font) = &choice.font {
                let attributes = gtk::pango::AttrList::new();
                attributes.insert(gtk::pango::AttrString::new_family(font));
                title.set_attributes(Some(&attributes));
            }
            words.append(&title);
            if let Some(text) = &choice.text {
                words.append(&pretty(text, &["small"]));
            }
            content.append(&words);
            if choice.terminal && !chosen {
                let mark = icon("terminal", 15);
                mark.add_css_class("faint");
                mark.set_valign(gtk::Align::Start);
                content.append(&mark);
            }
            let option = Self::radio(&content, &["option"], chosen);
            option.set_sensitive(!disabled);
            let (me, id, value) = (self.me.clone(), item.id.clone(), choice.value.clone());
            option.connect_clicked(move |_| {
                if let (Some(this), false) = (me.upgrade(), chosen) {
                    this.change(&id, &value);
                }
            });
            options.append(&option);
        }
        self.card_frame(item, None, vec![options.upcast()], false, Vec::new())
    }

    /// The top bar drawn as four places, Left, Centre, Right and Hidden, each holding its items in order. An item is
    /// dragged to another place or along its own, and each has a menu that does the same from the keyboard. What goes
    /// back to the list is the item's id, one of the four places and a position in it, which control-core checks.
    fn bar_card(&self, item: &Item) -> gtk::Box {
        const ZONES: [&str; 4] = ["left", "center", "right", "hidden"];
        let busy = self.busy(&item.id) || !item.available;
        let names = [t!("Left"), t!("Centre"), t!("Right"), t!("Hidden")];
        let places = ui::vbox(8, &["bar-places"]);
        for (index, zone) in ZONES.iter().enumerate() {
            let entries: Vec<_> = item.bar_items.iter().filter(|entry| entry.zone == *zone).cloned().collect();
            // The name above its items, so that a long row wraps in the card's whole width
            let row = ui::vbox(6, &["bar-place"]);
            let name = ui::label(&names[index], &["bar-place-name", "small"]);
            name.set_xalign(0.0);
            row.append(&name);
            let chips = Flex::new(6, 6, &["bar-chips"]);
            chips.set_hexpand(true);
            chips.update_property(&[gtk::accessible::Property::Label(&names[index])]);
            let shown: Rc<RefCell<Vec<(String, gtk::Widget)>>> = Rc::new(RefCell::new(Vec::new()));
            if entries.is_empty() {
                chips.append(&ui::label(&t!("Nothing here"), &["faint", "small"]));
            }
            for (at, entry) in entries.iter().enumerate() {
                let chip = gtk::MenuButton::builder().label(&entry.name).css_classes(["bar-chip"]).sensitive(!busy).build();
                chip.update_property(&[gtk::accessible::Property::Description(&t!("In {place}, number {n}", place = &names[index], n = &(at + 1).to_string()))]);
                // The menu: to another place (the end of it), or one step along this one
                let menu = ui::vbox(2, &["bar-chip-menu"]);
                let popover = gtk::Popover::builder().child(&menu).build();
                let mut moves: Vec<(String, &'static str, usize)> = Vec::new();
                for (other, label) in [("left", t!("To the left")), ("center", t!("To the centre")), ("right", t!("To the right")), ("hidden", t!("Hide"))] {
                    if other == *zone || (other == "hidden" && entry.fixed) {
                        continue;
                    }
                    let end = item.bar_items.iter().filter(|e| e.zone == other).count();
                    moves.push((label, other, end));
                }
                if at > 0 {
                    moves.push((t!("Earlier"), *zone, at - 1));
                }
                if at + 1 < entries.len() {
                    moves.push((t!("Later"), *zone, at + 1));
                }
                for (label, other, position) in moves {
                    let (me, card, id, pop) = (self.me.clone(), item.id.clone(), entry.id.clone(), popover.downgrade());
                    menu.append(&ui::button(&label, None, &["plain", "bar-chip-move"], move || {
                        if let Some(pop) = pop.upgrade() {
                            pop.popdown();
                        }
                        if let Some(this) = me.upgrade() {
                            this.bar_place(&card, Some((id.clone(), other, position)));
                        }
                    }));
                }
                chip.set_popover(Some(&popover));
                // Dragged by its id, which is all a drop takes
                let source = gtk::DragSource::new();
                source.set_actions(gdk::DragAction::MOVE);
                let id = entry.id.clone();
                source.connect_prepare(move |_, _, _| Some(gdk::ContentProvider::for_value(&id.to_value())));
                chip.add_controller(source);
                shown.borrow_mut().push((entry.id.clone(), chip.clone().upcast()));
                chips.append(&chip);
            }
            // A drop: where among this place's items, by where it landed
            let target = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::MOVE);
            let (me, card, zone, fixed, area) = (self.me.clone(), item.id.clone(), *zone, item.bar_items.iter().filter(|e| e.fixed).map(|e| e.id.clone()).collect::<Vec<_>>(), chips.downgrade());
            target.connect_drop(move |_, value, x, y| {
                let (Some(this), Some(area), Ok(id)) = (me.upgrade(), area.upgrade(), value.get::<String>()) else { return false };
                if busy || (zone == "hidden" && fixed.contains(&id)) {
                    return false;
                }
                let before = shown
                    .borrow()
                    .iter()
                    .filter(|(other, _)| *other != id)
                    .filter(|(_, chip)| {
                        chip.compute_bounds(&area).is_some_and(|b| {
                            let (bx, by, bw, bh) = (f64::from(b.x()), f64::from(b.y()), f64::from(b.width()), f64::from(b.height()));
                            by + bh <= y || (by <= y && bx + bw / 2.0 < x)
                        })
                    })
                    .count();
                this.bar_place(&card, Some((id, zone, before)));
                true
            });
            chips.add_controller(target);
            row.append(&chips);
            places.append(&row);
        }
        let card = item.id.clone();
        let reset = self.button(&t!("As the image has it"), Look { small: true, disabled: busy, ..Default::default() }, move |this| {
            this.bar_place(&card, None);
        });
        reset.set_halign(gtk::Align::Start);
        self.card_frame(item, None, vec![places.upcast(), reset.upcast()], false, Vec::new())
    }

    /// Choices that are colours: two columns of rows, each a dot of its colour and its name, the chosen one shaded
    fn colors_card(&self, item: &Item) -> gtk::Box {
        let disabled = self.busy(&item.id) || !item.available;
        let grid = gtk::Grid::builder().row_spacing(4).column_spacing(12).column_homogeneous(true).build();
        grid.add_css_class("colors");
        grid.update_property(&[gtk::accessible::Property::Label(&item.title)]);
        let mut added = false;
        for (index, choice) in item.choices.iter().enumerate() {
            let chosen = Some(&choice.value) == item.state.as_ref();
            let content = ui::hbox(10, &[]);
            let dot = ui::hbox(0, &["color-dot"]);
            dot.set_valign(gtk::Align::Center);
            if let Some(class) = self.color_class("bg", choice.color.as_ref(), &mut added) {
                dot.add_css_class(&class);
            }
            content.append(&dot);
            let name = ui::label(&choice.title, &["color-name"]);
            name.set_hexpand(true);
            name.set_xalign(0.0);
            content.append(&name);
            let row = Self::radio(&content, &["color-row"], chosen);
            if let Some(text) = &choice.text {
                row.set_tooltip_text(Some(text));
            }
            row.set_sensitive(!disabled);
            let (me, id, value) = (self.me.clone(), item.id.clone(), choice.value.clone());
            row.connect_clicked(move |_| {
                if let (Some(this), false) = (me.upgrade(), chosen) {
                    this.change(&id, &value);
                }
            });
            // A choice of the theme's own ("Follow the theme") has the first row to itself, the colours two to a row under it
            let first = item.choices.first().is_some_and(|choice| choice.value == "theme");
            if first && index == 0 {
                grid.attach(&row, 0, 0, 2, 1);
            } else {
                let at = index - usize::from(first);
                grid.attach(&row, (at % 2) as i32, (at / 2 + usize::from(first)) as i32, 1, 1);
            }
        }
        if added {
            self.refresh_colors();
        }
        self.card_frame(item, None, vec![grid.upcast()], false, Vec::new())
    }

    fn range_card(&self, item: &Item) -> gtk::Box {
        let min = item.min.unwrap_or(0.0);
        let max = item.max.unwrap_or(100.0).max(min);
        let step = item.step.filter(|step| *step > 0.0).unwrap_or(1.0);
        let value = item.state.as_deref().and_then(|state| state.trim().parse::<f64>().ok()).filter(|n| *n != 0.0).unwrap_or(min);
        let ms = item.unit.as_deref() == Some("ms");
        let show = move |n: f64| if ms { seconds(n) } else { i18n::number(n) };
        let output = ui::line(&show(value), &["output"]);
        let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, min, max, step);
        scale.set_value(value);
        scale.set_draw_value(false);
        scale.set_hexpand(true);
        scale.set_increments(step, step);
        scale.update_property(&[gtk::accessible::Property::Label(&item.title)]);
        scale.set_sensitive(!self.busy(&item.id) && item.available);
        // On the steps the list gives, as a range input moves
        scale.connect_change_value(move |scale, _, value| {
            let snapped = (((value - min) / step).round() * step + min).clamp(min, max);
            scale.set_value(snapped);
            glib::Propagation::Stop
        });
        let shown = output.clone();
        scale.connect_value_changed(move |scale| shown.set_label(&show(scale.value())));
        // A range input says it changed once it is let go of, or for each key that moves it
        let sent = std::rc::Rc::new(std::cell::Cell::new(value));
        let events = gtk::EventControllerLegacy::new();
        events.set_propagation_phase(gtk::PropagationPhase::Capture);
        let (me, id, scale_ref) = (self.me.clone(), item.id.clone(), scale.downgrade());
        events.connect_event(move |_, event| {
            let ended = matches!(event.event_type(), gdk::EventType::ButtonRelease | gdk::EventType::TouchEnd | gdk::EventType::KeyRelease);
            if let (true, Some(this), Some(scale)) = (ended, me.upgrade(), scale_ref.upgrade()) {
                let now = scale.value();
                if (now - sent.get()).abs() > f64::EPSILON {
                    sent.set(now);
                    let id = id.clone();
                    let value = format!("{}", now.round() as i64);
                    // Not from inside the scale's own event
                    glib::idle_add_local_once(move || this.change(&id, &value));
                }
            }
            glib::Propagation::Proceed
        });
        scale.add_controller(events);
        // The wheel scrolls the page, as it did over a range input, rather than moving the slider
        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
        let page = self.parts.scroller.vadjustment();
        wheel.connect_scroll(move |controller, _, dy| {
            let delta = if controller.unit() == gdk::ScrollUnit::Wheel { dy * page.page_size().powf(2.0 / 3.0) } else { dy };
            page.set_value(page.value() + delta);
            glib::Propagation::Stop
        });
        scale.add_controller(wheel);
        let row = ui::hbox(14, &["range"]);
        row.append(&scale);
        row.append(&output);
        // The slider at most 420 pixels wide, and what it is set to after it
        self.card_frame(item, None, vec![ui::limit(&row, 420 + 14 + 64).upcast()], false, Vec::new())
    }

    fn action_card(&self, item: &Item) -> gtk::Box {
        let id = item.id.clone();
        let label = item.button.clone().unwrap_or_else(|| t!("Run"));
        let control = self.button(&label, Look { danger: item.danger, disabled: self.busy(&item.id) || !item.available, ..Default::default() }, move |this| this.act(&id));
        let tags = if item.terminal && item.available { vec![Self::terminal_tag().upcast()] } else { Vec::new() };
        self.card_frame(item, Some(control.upcast()), Vec::new(), item.danger, tags)
    }

    fn link_card(&self, item: &Item) -> gtk::Box {
        let page = item.manual.clone().unwrap_or_default();
        let control = self.button(&t!("Open"), Look { icon: Some("book"), ..Default::default() }, move |_| process::manual(&page));
        self.card_frame(item, Some(control.upcast()), Vec::new(), false, Vec::new())
    }

    /// The class that paints a swatch's part in one of its theme's colours, when the colour is one
    fn color_class(&self, prefix: &str, color: Option<&String>, added: &mut bool) -> Option<String> {
        let color = color.filter(|c| c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|d| d.is_ascii_hexdigit()))?.to_lowercase();
        if self.colors.borrow_mut().insert(color.clone()) {
            *added = true;
        }
        Some(format!("{prefix}-{}", &color[1..]))
    }

    fn refresh_colors(&self) {
        let css: String = self
            .colors
            .borrow()
            .iter()
            .map(|color| format!(".bg-{0} {{ background-color: #{0}; }}\n.fg-{0} {{ color: #{0}; }}\n", &color[1..]))
            .collect();
        self.color_css.load_from_string(&css);
    }

    /// A theme as a small window in its own colours: its background, a line of its text, its accent, and
    /// six of its terminal colours
    fn themes_card(&self, item: &Item) -> gtk::Box {
        let disabled = self.busy(&item.id) || !item.available;
        let swatches = AutoGrid::new(150, 12, &["themes"]);
        swatches.update_property(&[gtk::accessible::Property::Label(&item.title)]);
        let mut added = false;
        for choice in &item.choices {
            let colors = &choice.colors;
            let chosen = Some(&choice.value) == item.state.as_ref();
            let picture = ui::vbox(7, &["swatch-picture"]);
            picture.set_overflow(gtk::Overflow::Hidden);
            if let Some(class) = self.color_class("bg", colors.get("background"), &mut added) {
                picture.add_css_class(&class);
            }
            let text = ui::line("Aa", &["swatch-text"]);
            if let Some(class) = self.color_class("fg", colors.get("foreground"), &mut added) {
                text.add_css_class(&class);
            }
            picture.append(&text);
            let bar = SwatchBar::default();
            if let Some(class) = self.color_class("fg", colors.get("accent"), &mut added) {
                bar.add_css_class(&class);
            }
            picture.append(&bar);
            let spacer = ui::hbox(0, &[]);
            spacer.set_vexpand(true);
            picture.append(&spacer);
            let dots = ui::hbox(5, &["swatch-dots"]);
            for key in ["color1", "color2", "color3", "color4", "color5", "color6"] {
                let dot = ui::hbox(0, &["swatch-dot"]);
                if let Some(class) = self.color_class("bg", colors.get(key), &mut added) {
                    dot.add_css_class(&class);
                }
                dots.append(&dot);
            }
            picture.append(&dots);
            let content = ui::vbox(8, &[]);
            content.append(&Aspect::new(&picture, 10.0 / 16.0));
            let name = ui::hbox(6, &["swatch-name"]);
            if chosen {
                name.append(&icon("check", 14));
            }
            let words = ui::label(&choice.title, &[]);
            words.set_hexpand(true);
            name.append(&words);
            content.append(&name);
            let swatch = Self::radio(&content, &["swatch"], chosen);
            swatch.set_valign(gtk::Align::Start);
            swatch.set_sensitive(!disabled);
            let (me, id, value) = (self.me.clone(), item.id.clone(), choice.value.clone());
            swatch.connect_clicked(move |_| {
                if let (Some(this), false) = (me.upgrade(), chosen) {
                    this.change(&id, &value);
                }
            });
            swatches.append(&swatch);
        }
        if added {
            self.refresh_colors();
        }
        let below: Vec<gtk::Widget> = match self.theme_preview(item, &swatches) {
            Some(preview) => vec![preview, swatches.upcast()],
            None => vec![swatches.upcast()],
        };
        self.card_frame(item, None, below, false, Vec::new())
    }

    /// A small desktop drawn from a theme's palette, above the swatches: the theme under the pointer or with the
    /// keyboard's focus, else the one chosen. Nothing changes until a swatch is clicked.
    fn theme_preview(&self, item: &Item, swatches: &AutoGrid) -> Option<gtk::Widget> {
        let chosen = item.choices.iter().find(|choice| Some(&choice.value) == item.state.as_ref()).or(item.choices.first())?.clone();
        let overlay = gtk::Overlay::new();
        overlay.add_css_class("theme-preview");
        overlay.set_overflow(gtk::Overflow::Hidden);
        let picture = gtk::Picture::builder().content_fit(gtk::ContentFit::Cover).can_shrink(true).build();
        overlay.set_child(Some(&picture));

        let bar = gtk::CenterBox::new();
        bar.add_css_class("pv-bar");
        bar.set_start_widget(Some(&ui::label("1", &["pv-pill"])));
        bar.set_center_widget(Some(&ui::label(&i18n::date(i18n::now(), DateStyle::Time), &["pv-clock"])));
        overlay.add_overlay(&bar);

        let window = ui::vbox(0, &["pv-window"]);
        window.set_overflow(gtk::Overflow::Hidden);
        let title = ui::hbox(5, &["pv-title"]);
        let name = ui::label(&t!("Notes"), &[]);
        name.set_hexpand(true);
        name.set_xalign(0.0);
        title.append(&name);
        for color in ["yellow", "green", "red"] {
            let dot = ui::hbox(0, &["pv-dot", color]);
            dot.set_valign(gtk::Align::Center);
            title.append(&dot);
        }
        window.append(&title);
        let body = ui::vbox(8, &["pv-body"]);
        let day = ui::label(&i18n::capital(&i18n::date(i18n::now(), DateStyle::Weekday)), &[]);
        day.set_xalign(0.0);
        body.append(&day);
        let task = ui::hbox(8, &[]);
        let check = ui::hbox(0, &["pv-check"]);
        check.set_valign(gtk::Align::Center);
        task.append(&check);
        let words = ui::label(&t!("Water the plants"), &[]);
        words.set_xalign(0.0);
        task.append(&words);
        body.append(&task);
        let add = ui::label(&t!("Add a task"), &["pv-add"]);
        add.set_halign(gtk::Align::Start);
        body.append(&add);
        window.append(&body);
        overlay.add_overlay(&window);

        // The bar 8 pixels in from each side and 6 from the top, 22 high; the window 18% in from the left, 14% from
        // the right, 40 pixels from the top and 16 from the bottom
        let (bar_widget, window_widget) = (bar.clone().upcast::<gtk::Widget>(), window.clone().upcast::<gtk::Widget>());
        overlay.connect_get_child_position(move |overlay, child| {
            let (width, height) = (overlay.width(), overlay.height());
            if *child == bar_widget {
                Some(gdk::Rectangle::new(8, 6, (width - 16).max(0), 22))
            } else if *child == window_widget {
                // From the start edge: mirrored for a language written right to left
                let mut left = (width as f64 * 0.18).round() as i32;
                let mut right = (width as f64 * 0.14).round() as i32;
                if overlay.direction() == gtk::TextDirection::Rtl {
                    std::mem::swap(&mut left, &mut right);
                }
                Some(gdk::Rectangle::new(left, 40, (width - left - right).max(0), (height - 56).max(0)))
            } else {
                None
            }
        });
        overlay.set_can_target(false);

        let show = {
            let me = self.me.clone();
            let picture = picture.downgrade();
            Rc::new(move |choice: &amethystora_control_core::Choice| {
                let (Some(this), Some(picture)) = (me.upgrade(), picture.upgrade()) else { return };
                this.preview_css.load_from_string(&preview_css(choice));
                match choice.picture.as_deref() {
                    Some(path) => this.thumbnail(path, &picture),
                    None => {
                        picture.set_widget_name("");
                        picture.set_paintable(gdk::Paintable::NONE)
                    }
                }
            })
        };
        show(&chosen);
        // Whether the pointer is over the grid: a swatch losing the keyboard's focus then leaves the preview to it
        let pointed = Rc::new(std::cell::Cell::new(false));
        let mut child = swatches.first_child();
        let mut index = 0;
        while let Some(swatch) = child {
            if let Some(choice) = item.choices.get(index).cloned() {
                let motion = gtk::EventControllerMotion::new();
                let (show_in, choice_in, pointed_in) = (show.clone(), choice.clone(), pointed.clone());
                motion.connect_enter(move |_, _, _| {
                    pointed_in.set(true);
                    show_in(&choice_in);
                });
                swatch.add_controller(motion);
                let focus = gtk::EventControllerFocus::new();
                let (show_in, choice_in) = (show.clone(), choice);
                focus.connect_enter(move |_| show_in(&choice_in));
                let (show_out, chosen_out, pointed_out) = (show.clone(), chosen.clone(), pointed.clone());
                focus.connect_leave(move |_| {
                    if !pointed_out.get() {
                        show_out(&chosen_out);
                    }
                });
                swatch.add_controller(focus);
            }
            index += 1;
            child = swatch.next_sibling();
        }
        let leave = gtk::EventControllerMotion::new();
        let grid = swatches.downgrade();
        leave.connect_leave(move |_| {
            pointed.set(false);
            if grid.upgrade().is_none_or(|grid| grid.focus_child().is_none()) {
                show(&chosen);
            }
        });
        swatches.add_controller(leave);
        // 16:10 in a narrow window; in a wide one no taller than this, so that the swatches stay in view
        let frame = Aspect::new(&overlay, 10.0 / 16.0).with_max(300);
        frame.add_css_class("theme-preview-frame");
        // Only decoration: the swatches say which theme each is
        frame.set_accessible_role(gtk::AccessibleRole::Presentation);
        Some(frame.upcast())
    }

    /// A wallpaper of the current theme, only one the list named and only a picture, decoded once at about
    /// the size it is shown rather than whole
    fn thumbnail(&self, path: &str, picture: &gtk::Picture) {
        // The picture it wants now, by name: a decode that ends after it was asked for another (the preview, pointed at one
        // theme and then the next) is kept for later but not shown
        picture.set_widget_name(path);
        if let Some(texture) = self.thumbnails.borrow().get(path) {
            picture.set_paintable(Some(texture));
            return;
        }
        let extension = std::path::Path::new(path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !PICTURES.contains(&extension.as_str()) {
            return;
        }
        let (me, path, picture) = (self.me.clone(), path.to_string(), picture.downgrade());
        glib::spawn_future_local(async move {
            let Ok(stream) = gio::File::for_path(&path).read_future(glib::Priority::LOW).await else { return };
            let Ok(pixbuf) = gtk::gdk_pixbuf::Pixbuf::from_stream_at_scale_future(&stream, 512, 320, true).await else { return };
            let texture = gdk::Texture::for_pixbuf(&pixbuf);
            if let Some(this) = me.upgrade() {
                this.thumbnails.borrow_mut().insert(path.clone(), texture.clone());
            }
            if let Some(picture) = picture.upgrade().filter(|picture| picture.widget_name() == path.as_str()) {
                picture.set_paintable(Some(&texture));
            }
        });
    }

    /// Make a theme: a palette to start from, the ground, text, accent and selection and the sixteen terminal colours each
    /// picked in GTK's own colour dialog, the three checks measured as a colour is picked (contrast.rs, palette.awk's
    /// sums), each failing one with the nearest colour that passes a click away, and Save, which sends the palette on stdin.
    /// Nothing runs until Save or Import; Preview draws the palette on the theme card's preview.
    fn theme_maker_card(&self, item: &Item) -> gtk::Box {
        if !item.available || item.starts.is_empty() {
            return self.card_frame(item, None, Vec::new(), false, Vec::new());
        }
        let busy = self.busy(&item.id);
        // The first drawing starts from the theme in use, else the first the list gives
        {
            let mut state = self.state.borrow_mut();
            if state.maker.start.is_empty() || !item.starts.iter().any(|start| start.value == state.maker.start) {
                let current = self.known().and_then(|list| list.find("theme").and_then(|theme| theme.state.clone()));
                let start = item.starts.iter().find(|start| Some(&start.value) == current.as_ref()).unwrap_or(&item.starts[0]);
                state.maker.start = start.value.clone();
                state.maker.colors = start_palette(start);
            }
        }
        let (start, colors, name) = {
            let state = self.state.borrow();
            (state.maker.start.clone(), state.maker.colors.clone(), state.maker.name.clone())
        };

        // Start from
        let from = ui::hbox(10, &["maker-from"]);
        let from_label = ui::label(&t!("Start from"), &["strong"]);
        from_label.set_hexpand(true);
        from_label.set_xalign(0.0);
        from.append(&from_label);
        let titles: Vec<&str> = item.starts.iter().map(|start| start.title.as_str()).collect();
        let pick = gtk::DropDown::from_strings(&titles);
        pick.update_property(&[gtk::accessible::Property::Label(&t!("Start from"))]);
        if let Some(at) = item.starts.iter().position(|s| s.value == start) {
            pick.set_selected(at as u32);
        }
        pick.set_sensitive(!busy);
        {
            let (me, starts) = (self.me.clone(), item.starts.clone());
            pick.connect_selected_notify(move |pick| {
                let Some(start) = starts.get(pick.selected() as usize) else { return };
                let Some(this) = me.upgrade() else { return };
                {
                    let mut state = this.state.borrow_mut();
                    if state.maker.start == start.value {
                        return;
                    }
                    state.maker.start = start.value.clone();
                    state.maker.colors = start_palette(start);
                }
                let me = this.me.clone();
                glib::idle_add_local_once(move || {
                    if let Some(this) = me.upgrade() {
                        this.render();
                    }
                });
            });
        }
        from.append(&pick);

        // The four big colours, then the sixteen
        let big = ui::hbox(14, &["maker-big"]);
        for (key, label) in [
            ("background", t!("Ground")),
            ("foreground", t!("Text")),
            ("accent", t!("Accent")),
            ("selection_background", t!("Selection")),
        ] {
            let column = ui::vbox(4, &["maker-colour"]);
            column.append(&self.colour_button(key, &label, colors.get(key), &["maker-swatch", "big"], busy));
            column.append(&ui::label(&label, &["maker-label"]));
            big.append(&column);
        }
        let terminal = Flex::new(6, 6, &["maker-terminal"]);
        for i in 0..16 {
            let key = format!("color{i}");
            let label = t!("Terminal colour {number}", number = i.to_string());
            terminal.append(&self.colour_button(&key, &label, colors.get(&key), &["maker-swatch"], busy));
        }

        // The checks, as palette.awk would give them
        let checks = ui::vbox(5, &["maker-checks"]);
        for check in amethystora_control_core::contrast::checks(&colors) {
            let row = ui::hbox(10, &["maker-check"]);
            let what = match check.name {
                "text" => t!("Text on the surfaces"),
                "label" => t!("A label on the accent"),
                _ => t!("Outlines"),
            };
            let words = ui::label(&what, &["strong"]);
            words.set_hexpand(true);
            words.set_xalign(0.0);
            row.append(&words);
            let ratio = i18n::number((check.ratio * 10.0).floor() / 10.0);
            let need = i18n::number(check.need);
            let said = if check.passes() {
                t!("{ratio} : 1, passes", ratio = &ratio)
            } else {
                t!("{ratio} : 1, needs {need} : 1", ratio = &ratio, need = &need)
            };
            row.append(&ui::label(&said, &[if check.passes() { "ok" } else { "bad" }]));
            if !check.passes() {
                let (key, nearest) = (check.key, check.nearest.clone());
                let fix = self.button(&t!("Use {colour}", colour = &nearest), Look { small: true, disabled: busy, ..Default::default() }, move |this| {
                    this.state.borrow_mut().maker.colors.insert(key.to_string(), nearest.clone());
                    this.render();
                });
                fix.update_property(&[gtk::accessible::Property::Label(&t!("Use {colour}, the nearest colour that passes", colour = &check.nearest))]);
                row.append(&fix);
            }
            checks.append(&row);
        }

        // The name, Preview, Import and Save
        let field = gtk::Entry::builder().text(&name).max_length(40).placeholder_text(t!("Name")).css_classes(["field", "maker-name"]).build();
        field.update_property(&[gtk::accessible::Property::Label(&t!("The theme's name"))]);
        {
            let me = self.me.clone();
            field.connect_changed(move |field| {
                if let Some(this) = me.upgrade() {
                    this.state.borrow_mut().maker.name = field.text().to_string();
                }
            });
        }
        let note = ui::label("", &["form-note", "bad"]);
        note.set_visible(false);
        let buttons = ui::hbox(8, &["maker-buttons"]);
        let card = item.id.clone();
        buttons.append(&self.button(&t!("Import a theme…"), Look { small: true, icon: Some("terminal"), disabled: busy, ..Default::default() }, move |this| this.import_theme(&card)));
        let spacer = ui::hbox(0, &[]);
        spacer.set_hexpand(true);
        buttons.append(&spacer);
        buttons.append(&self.button(&t!("Preview"), Look { small: true, disabled: busy, ..Default::default() }, |this| {
            let maker = this.state.borrow().maker.clone();
            let choice = amethystora_control_core::Choice {
                colors: maker.colors.clone(),
                light: maker.colors.get("background").is_some_and(|bg| amethystora_control_core::contrast::is_light(bg)),
                ..Default::default()
            };
            this.preview_css.load_from_string(&preview_css(&choice));
        }));
        let save_words = if name.trim().is_empty() { t!("Save") } else { t!("Save as {name}", name = name.trim()) };
        let (card, field_weak, note_weak) = (item.id.clone(), field.downgrade(), note.downgrade());
        let save = self.button(&save_words, Look { primary: true, disabled: busy, ..Default::default() }, move |this| {
            let (Some(field), Some(note)) = (field_weak.upgrade(), note_weak.upgrade()) else { return };
            let name = field.text().trim().to_string();
            if !amethystora_control_core::valid_theme_name(&name) {
                note.set_text(&t!("A theme's name is 1 to 40 letters, digits, spaces and dashes."));
                note.set_visible(true);
                return;
            }
            let colors = this.state.borrow().maker.colors.clone();
            let me = this.clone();
            this.perform(&card, async move { backend::perform(backend::save_theme(me.known().as_deref(), &name, &colors)).await });
        });
        save.add_css_class("pill");
        buttons.append(&save);
        // The button says the name as it is typed
        {
            let save = save.downgrade();
            field.connect_changed(move |field| {
                if let Some(save) = save.upgrade() {
                    let name = field.text().trim().to_string();
                    let words = if name.is_empty() { t!("Save") } else { t!("Save as {name}", name = &name) };
                    save.set_label(&words);
                }
            });
        }

        let body = ui::vbox(12, &["maker"]);
        body.append(&from);
        body.append(&big);
        body.append(&ui::label(&t!("Terminal colours"), &["form-note"]));
        body.append(&terminal);
        body.append(&checks);
        body.append(&field);
        body.append(&buttons);
        body.append(&note);
        self.card_frame(item, None, vec![body.upcast()], false, Vec::new())
    }

    /// One colour of the maker's palette, picked in GTK's colour dialog; a pick is kept and the card drawn again
    fn colour_button(&self, key: &str, label: &str, color: Option<&String>, classes: &[&str], busy: bool) -> gtk::Widget {
        let dialog = gtk::ColorDialog::builder().with_alpha(false).modal(true).title(label).build();
        let button = gtk::ColorDialogButton::new(Some(dialog));
        for class in classes {
            button.add_css_class(class);
        }
        if let Some(rgba) = color.and_then(|c| gdk::RGBA::parse(c.as_str()).ok()) {
            button.set_rgba(&rgba);
        }
        button.set_tooltip_text(Some(label));
        button.update_property(&[gtk::accessible::Property::Label(label)]);
        button.set_sensitive(!busy);
        let (me, key) = (self.me.clone(), key.to_string());
        button.connect_rgba_notify(move |button| {
            let rgba = button.rgba();
            let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            let hex = format!("#{:02x}{:02x}{:02x}", channel(rgba.red()), channel(rgba.green()), channel(rgba.blue()));
            let Some(this) = me.upgrade() else { return };
            let changed = this.state.borrow_mut().maker.colors.insert(key.clone(), hex.clone()).as_deref() != Some(hex.as_str());
            if changed {
                // Drawn again once the dialog's signal is over: the button that sent it is replaced
                let me = this.me.clone();
                glib::idle_add_local_once(move || {
                    if let Some(this) = me.upgrade() {
                        this.render();
                    }
                });
            }
        });
        button.upcast()
    }

    fn wallpapers_card(&self, item: &Item) -> gtk::Box {
        let disabled = self.busy(&item.id) || !item.available;
        let below: Vec<gtk::Widget> = if !item.available {
            Vec::new()
        } else if item.choices.is_empty() {
            vec![ui::label(&t!("This theme has no pictures of its own."), &["row-empty"]).upcast()]
        } else {
            let pictures = AutoGrid::new(170, 12, &["pictures"]);
            pictures.update_property(&[gtk::accessible::Property::Label(&item.title)]);
            for choice in &item.choices {
                let chosen = Some(&choice.value) == item.state.as_ref();
                let image = gtk::Picture::builder().content_fit(gtk::ContentFit::Cover).can_shrink(true).build();
                image.set_alternative_text(Some(&choice.title));
                // Daylight's slideshow shows the picture of the hour on its tile
                self.thumbnail(choice.thumbnail.as_deref().unwrap_or(&choice.value), &image);
                let frame = Self::radio(&Aspect::new(&image, 10.0 / 16.0), &["picture"], chosen);
                frame.set_overflow(gtk::Overflow::Hidden);
                frame.set_tooltip_text(Some(&choice.title));
                frame.update_property(&[gtk::accessible::Property::Label(&choice.title)]);
                frame.set_valign(gtk::Align::Start);
                frame.set_sensitive(!disabled);
                let (me, id, value) = (self.me.clone(), item.id.clone(), choice.value.clone());
                frame.connect_clicked(move |_| {
                    if let (Some(this), false) = (me.upgrade(), chosen) {
                        this.change(&id, &value);
                    }
                });
                pictures.append(&frame);
            }
            vec![pictures.upcast()]
        };
        self.card_frame(item, None, below, false, Vec::new())
    }

    fn rows(rows: Vec<gtk::Box>) -> gtk::Box {
        let list = ui::vbox(0, &["rows"]);
        list.set_overflow(gtk::Overflow::Hidden);
        for row in rows {
            list.append(&row);
        }
        list
    }

    fn row_words(title: &str, detail: Option<&str>, detail_ltr: bool, title_ltr: bool) -> gtk::Box {
        let words = ui::vbox(0, &["row-words"]);
        words.set_hexpand(true);
        words.set_valign(gtk::Align::Center);
        let strong = row_line(title, &["strong"]);
        words.append(&if title_ltr { ltr(strong) } else { strong });
        if let Some(detail) = detail {
            let small = row_line(detail, &["small"]);
            words.append(&if detail_ltr { ltr(small) } else { small });
        }
        words
    }

    fn widgets_card(&self, item: &Item) -> gtk::Box {
        let busy = self.busy(&item.id);
        let widgets = &item.widgets;
        let rows: gtk::Widget = if widgets.is_empty() {
            ui::label(&t!("No widgets yet."), &["row-empty"]).upcast()
        } else {
            Self::rows(
                widgets
                    .iter()
                    .map(|widget| {
                        let row = ui::hbox(12, &["row"]);
                        let name = widget.name.clone().unwrap_or_else(|| widget.id.clone());
                        let detail = if widget.broken { t!("{id}: its widget.json is missing or broken", id = &widget.id) } else { widget.id.clone() };
                        row.append(&Self::row_words(&name, Some(&detail), true, false));
                        if item.agentic {
                            let (card, id) = (item.id.clone(), widget.id.clone());
                            row.append(&self.button(&t!("Fix…"), Look { small: true, icon: Some("wrench"), disabled: busy, ..Default::default() }, move |this| {
                                this.widget(&card, "fix", &id)
                            }));
                        }
                        let (card, id) = (item.id.clone(), widget.id.clone());
                        row.append(&self.confirm_button(&t!("Remove"), &t!("Move it to the trash?"), "trash", busy, move |this| this.widget(&card, "remove", &id)));
                        let (card, id, on) = (item.id.clone(), widget.id.clone(), widget.on);
                        let toggle = self.switch(widget.on, &t!("Show {name} in the top bar", name = &name), busy, move |this| {
                            this.widget(&card, if on { "off" } else { "on" }, &id)
                        });
                        toggle.set_valign(gtk::Align::Center);
                        row.append(&toggle);
                        row
                    })
                    .collect(),
            )
            .upcast()
        };

        // A new one from an example, under a name checked as amethystora-widgets checks it
        let examples = item.examples.clone();
        {
            let mut state = self.state.borrow_mut();
            if !examples.contains(&state.widget_example) {
                state.widget_example = examples.first().cloned().unwrap_or_default();
            }
        }
        let (name_now, example_now) = {
            let state = self.state.borrow();
            (state.widget_name.clone(), state.widget_example.clone())
        };
        let valid = backend::valid_widget_name(item, &name_now);
        let card = item.id.clone();
        let add = self.button(&t!("Add"), Look { icon: Some("plus"), disabled: busy || !valid || example_now.is_empty(), ..Default::default() }, move |this| {
            let (name, example) = {
                let mut state = this.state.borrow_mut();
                (std::mem::take(&mut state.widget_name), state.widget_example.clone())
            };
            let that = this.clone();
            this.perform(&card, async move { backend::perform(backend::new_widget(that.known().as_deref(), &name, &example)).await });
        });
        let field = gtk::Entry::builder()
            .text(&name_now)
            // An example of a name, which is lower case letters, digits and dashes in any language
            .placeholder_text("my-widget")
            .css_classes(["field"])
            .build();
        field.set_direction(gtk::TextDirection::Ltr);
        field.update_property(&[gtk::accessible::Property::Label(&t!("The new widget's name"))]);
        if !name_now.is_empty() && !valid {
            field.add_css_class("invalid");
        }
        {
            let (me, add, item) = (self.me.clone(), add.downgrade(), item.clone());
            field.connect_changed(move |field| {
                let Some(this) = me.upgrade() else { return };
                let name = field.text().trim().to_string();
                let ok = backend::valid_widget_name(&item, &name);
                if !name.is_empty() && !ok {
                    field.add_css_class("invalid");
                } else {
                    field.remove_css_class("invalid");
                }
                let example = {
                    let mut state = this.state.borrow_mut();
                    state.widget_name = name;
                    state.widget_example.clone()
                };
                if let Some(add) = add.upgrade() {
                    add.set_sensitive(!busy && ok && !example.is_empty());
                }
            });
        }
        let names: Vec<&str> = examples.iter().map(String::as_str).collect();
        let select = gtk::DropDown::from_strings(&names);
        select.add_css_class("field");
        select.update_property(&[gtk::accessible::Property::Label(&t!("The example it starts from"))]);
        if let Some(at) = examples.iter().position(|example| *example == example_now) {
            select.set_selected(at as u32);
        }
        {
            let (me, examples) = (self.me.clone(), examples.clone());
            select.connect_selected_notify(move |select| {
                if let (Some(this), Some(example)) = (me.upgrade(), examples.get(select.selected() as usize)) {
                    this.state.borrow_mut().widget_example = example.clone();
                }
            });
        }

        let mut below: Vec<gtk::Widget> = Vec::new();
        if !item.bar {
            let card = item.id.clone();
            below.push(
                Self::callout(
                    "check",
                    None,
                    None,
                    vec![Self::actions(vec![
                        self.button(&t!("Turn the extension on"), Look { small: true, ..Default::default() }, move |this| this.widget(&card, "restart", "")).upcast(),
                    ])
                    .upcast()],
                )
                .upcast(),
            );
        }
        if item.available {
            below.push(rows);
            // The gallery (R6-29): the widgets that ship, each added to the bar or the desktop in one click
            if !item.gallery.is_empty() {
                below.push(ui::label(&t!("Ready to add, in the bar or on the desktop. None is on until you add it."), &["form-note"]).upcast());
                below.push(self.widget_gallery(item, busy).upcast());
            }
            let form = Flex::new(8, 8, &["form"]);
            form.append_with(
                &ui::label(&t!("Start one from an example, then change its files in ~/.config/amethystora/widgets:"), &["form-note"]),
                Grow { whole_line: true, ..Default::default() },
            );
            form.append_with(&field, Grow { grow: 1.0, basis: Some(240), whole_line: false });
            form.append_with(&select, Grow { grow: 0.0, basis: Some(180), whole_line: false });
            form.append(&add);
            below.push(form.upcast());
            if item.agentic {
                let card = item.id.clone();
                below.push(
                    Self::actions(vec![
                        self.button(&t!("Make one with the agent…"), Look { icon: Some("sparkle"), ..Default::default() }, move |this| this.widget(&card, "make", "")).upcast(),
                    ])
                    .upcast(),
                );
            }
        }
        self.card_frame(item, None, below, false, Vec::new())
    }

    // One row for each widget of the gallery: its name and what it shows, and a button for each place it may go. The
    // names and words are the list's, already in the session's language (amethystora-control-list)
    fn widget_gallery(&self, item: &Item, busy: bool) -> gtk::Box {
        Self::rows(
            item.gallery
                .iter()
                .map(|entry| {
                    let row = ui::hbox(12, &["row"]);
                    row.append(&Self::row_words(&entry.name, entry.text.as_deref(), false, false));
                    for place in &entry.places {
                        let label = match place.as_str() {
                            "bar" => t!("Add to the bar"),
                            "desktop" => t!("Add to the desktop"),
                            _ => continue,
                        };
                        let (card, example, place) = (item.id.clone(), entry.example.clone(), place.clone());
                        let button = self.button(&label, Look { small: true, icon: Some("plus"), disabled: busy, ..Default::default() }, move |this| {
                            let (example, place) = (example.clone(), place.clone());
                            let that = this.clone();
                            this.perform(&card, async move { backend::perform(backend::gallery_widget(that.known().as_deref(), &example, &place)).await });
                        });
                        button.set_valign(gtk::Align::Center);
                        row.append(&button);
                    }
                    row
                })
                .collect(),
        )
    }

    fn webapps_card(&self, item: &Item) -> gtk::Box {
        let busy = self.busy(&item.id);
        let rows: gtk::Widget = if item.webapps.is_empty() {
            ui::label(&t!("No web apps yet."), &["row-empty"]).upcast()
        } else {
            Self::rows(
                item.webapps
                    .iter()
                    .map(|webapp| {
                        let row = ui::hbox(12, &["row"]);
                        row.append(&Self::row_words(&webapp.name, Some(&webapp.address), true, false));
                        let (card, name) = (item.id.clone(), webapp.name.clone());
                        row.append(&self.confirm_button(&t!("Remove"), &t!("Remove it?"), "trash", busy, move |this| this.webapp(&card, "remove", &name)));
                        row
                    })
                    .collect(),
            )
            .upcast()
        };
        let card = item.id.clone();
        let control = self.button(&t!("Add a web app…"), Look { icon: Some("plus"), disabled: busy || !item.available, ..Default::default() }, move |this| {
            this.webapp(&card, "install", "")
        });
        let below = if item.available { vec![rows] } else { Vec::new() };
        let tags = if item.available { vec![Self::terminal_tag().upcast()] } else { Vec::new() };
        self.card_frame(item, Some(control.upcast()), below, false, tags)
    }

    /// The text shortcuts (board 31): one row for each, its abbreviation and how many lines its text has (the list
    /// never carries a text; Edit reads it), the form to add one or change its text, and a field to try them in
    fn shortcuts_card(&self, item: &Item) -> gtk::Box {
        if !item.available {
            return self.card_frame(item, None, Vec::new(), false, Vec::new());
        }
        let busy = self.busy(&item.id);
        let mut shortcuts = item.shortcuts.clone();
        shortcuts.sort_by(|a, b| a.abbr.cmp(&b.abbr));
        let rows: gtk::Widget = if shortcuts.is_empty() {
            ui::label(&t!("No shortcuts yet. Add one below: for example ;addr for your address."), &["row-empty"]).upcast()
        } else {
            let list = ui::vbox(6, &["shortcut-rows"]);
            for shortcut in &shortcuts {
                let row = ui::hbox(12, &["shortcut-row"]);
                let abbr = ltr(ui::line(&shortcut.abbr, &["shortcut-abbr"]));
                // The board's first column, 120 pixels; a longer abbreviation is cut short with an ellipsis
                abbr.set_ellipsize(gtk::pango::EllipsizeMode::End);
                abbr.set_size_request(120, -1);
                row.append(&abbr);
                let lines = ui::label(&t!("{count, plural, one {One line} other {# lines}}", count = shortcut.lines as usize), &["shortcut-lines"]);
                lines.set_hexpand(true);
                lines.set_valign(gtk::Align::Center);
                row.append(&lines);
                let (card, name) = (item.id.clone(), shortcut.abbr.clone());
                let edit = self.button(&t!("Edit"), Look { small: true, disabled: busy, ..Default::default() }, move |this| this.edit_shortcut(&card, &name));
                edit.add_css_class("flat");
                edit.update_property(&[gtk::accessible::Property::Label(&t!("Edit the shortcut {abbr}", abbr = &shortcut.abbr))]);
                row.append(&edit);
                let (card, name) = (item.id.clone(), shortcut.abbr.clone());
                let remove = self.button(&t!("Remove"), Look { small: true, disabled: busy, ..Default::default() }, move |this| this.shortcut(&card, "remove", &name, ""));
                remove.add_css_class("flat");
                remove.update_property(&[gtk::accessible::Property::Label(&t!("Remove the shortcut {abbr}", abbr = &shortcut.abbr))]);
                row.append(&remove);
                list.append(&row);
            }
            list.upcast()
        };

        // The form: the abbreviation (one line, 32 at most), the text (Enter makes a new line), Add, or Save while
        // a shortcut's text is being changed. What would not be taken is said under it in the helper's own words
        let (abbr_now, text_now, editing) = {
            let state = self.state.borrow();
            (state.shortcut_abbr.clone(), state.shortcut_text.clone(), state.shortcut_editing.clone())
        };
        let field = gtk::Entry::builder().text(&abbr_now).max_length(32).placeholder_text(";addr").css_classes(["field", "shortcut-field"]).build();
        field.set_direction(gtk::TextDirection::Ltr);
        field.update_property(&[gtk::accessible::Property::Label(&t!("Shortcut"))]);
        let buffer = gtk::TextBuffer::new(None);
        buffer.set_text(&text_now);
        let text = gtk::TextView::builder().buffer(&buffer).wrap_mode(gtk::WrapMode::WordChar).accepts_tab(false).css_classes(["shortcut-text"]).build();
        text.update_property(&[gtk::accessible::Property::Label(&t!("Text"))]);
        let note = ui::label("", &["form-note", "bad"]);
        note.set_visible(false);
        let card = item.id.clone();
        let (field_weak, buffer_weak, note_weak) = (field.downgrade(), buffer.downgrade(), note.downgrade());
        let listed: Vec<String> = shortcuts.iter().map(|shortcut| shortcut.abbr.clone()).collect();
        // The abbreviation of another shortcut whose text the person was told a second press replaces
        let confirmed: Rc<std::cell::RefCell<Option<String>>> = Rc::default();
        let save = self.button(
            &if editing.is_some() { t!("Save") } else { t!("Add") },
            Look { primary: true, icon: if editing.is_some() { None } else { Some("plus") }, disabled: busy, ..Default::default() },
            {
                let (confirmed, editing) = (confirmed.clone(), editing.clone());
                move |this| {
                    let (Some(field), Some(buffer), Some(note)) = (field_weak.upgrade(), buffer_weak.upgrade(), note_weak.upgrade()) else { return };
                    let abbr = field.text().to_string();
                    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).replace("\r\n", "\n");
                    if let Some(problem) = backend::shortcut_problem(&abbr, &text) {
                        note.set_text(&problem);
                        note.set_visible(true);
                        return;
                    }
                    // Another shortcut's text is replaced only on a second press, once the person has been told
                    let replaces = listed.contains(&abbr) && editing.as_deref() != Some(abbr.as_str());
                    if replaces && confirmed.borrow().as_deref() != Some(abbr.as_str()) {
                        note.set_text(&t!("You have a shortcut {abbr} already: press it again to replace its text.", abbr = &abbr));
                        note.set_visible(true);
                        confirmed.replace(Some(abbr));
                        return;
                    }
                    this.shortcut(&card, "add", &abbr, &text);
                }
            },
        );
        save.add_css_class("pill");
        save.update_property(&[gtk::accessible::Property::Label(&if editing.is_some() { t!("Save the shortcut") } else { t!("Add the shortcut") })]);
        // What is typed is kept between drawings of the page, never written anywhere
        {
            let me = self.me.clone();
            field.connect_changed(move |field| {
                // Another abbreviation typed: the second press is asked for again
                confirmed.replace(None);
                if let Some(this) = me.upgrade() {
                    this.state.borrow_mut().shortcut_abbr = field.text().to_string();
                }
            });
            let me = self.me.clone();
            buffer.connect_changed(move |buffer| {
                if let Some(this) = me.upgrade() {
                    this.state.borrow_mut().shortcut_text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
                }
            });
        }
        let form = ui::vbox(8, &["shortcut-form"]);
        form.append(&ui::label(&t!("Shortcut"), &["form-note"]));
        form.append(&field);
        form.append(&ui::label(&t!("Text"), &["form-note"]));
        form.append(&text);
        let buttons = ui::hbox(8, &[]);
        buttons.append(&save);
        if editing.is_some() {
            buttons.append(&self.button(&t!("Cancel"), Look { small: true, ..Default::default() }, |this| {
                {
                    let mut state = this.state.borrow_mut();
                    state.shortcut_abbr.clear();
                    state.shortcut_text.clear();
                    state.shortcut_editing = None;
                }
                this.render();
            }));
        }
        form.append(&buttons);
        form.append(&ui::label(&t!("Write {date} for today's date."), &["form-note"]));
        form.append(&note);

        // Try it here: a field of the window's own, which knows an expansion only by what it sees happen to its text
        let expanding = self.known().and_then(|list| list.find("text-shortcuts").and_then(|item| item.state.clone())).as_deref() == Some("on");
        let try_label = ui::label(&t!("Try it here"), &["shortcut-try-title"]);
        let (try_now, said) = {
            let state = self.state.borrow();
            (state.try_text.clone(), state.try_said.clone())
        };
        let first = shortcuts.first().map(|shortcut| shortcut.abbr.clone());
        let try_field = gtk::Entry::builder()
            .text(&try_now)
            .css_classes(["field", "shortcut-try"])
            .placeholder_text(&match &first {
                Some(abbr) => t!("Type {abbr} and a space", abbr = abbr),
                None => t!("Type a shortcut and a space"),
            })
            .sensitive(expanding)
            .build();
        try_field.update_property(&[gtk::accessible::Property::Label(&t!("Try it here"))]);
        let under_words = match &said {
            _ if !expanding => t!("Turn on Expand text shortcuts to try it."),
            Some((abbr, text)) => t!("Expanded: {abbr} became “{text}”.", abbr = abbr, text = text),
            None => t!("Nothing expanded yet."),
        };
        let under = role_box(gtk::Orientation::Vertical, 0, &[], gtk::AccessibleRole::Status);
        let under_line = ui::label(&under_words, &["shortcut-try-said"]);
        under.append(&under_line);
        try_field_watch(&try_field, self.me.clone(), shortcuts.iter().map(|shortcut| shortcut.abbr.clone()).collect(), under_line.downgrade());
        let try_box = ui::vbox(8, &["shortcut-try-box"]);
        try_box.append(&try_label);
        try_box.append(&try_field);
        try_box.append(&under);

        self.card_frame(item, None, vec![rows, form.upcast(), try_box.upcast()], false, Vec::new())
    }

    // --- Every shortcut (R5-13) ------------------------------------------------------------------------------

    /// A shortcut the list named changed: set to the keys recorded, taken off, or put back as the image ships it
    fn keybinding(self: &Rc<Self>, card: &str, action: &str, schema: &str, key: &str, accel: &str) {
        let this = self.clone();
        let (action, schema, key, accel) = (action.to_string(), schema.to_string(), key.to_string(), accel.to_string());
        self.perform(card, async move {
            backend::perform(backend::keybinding(this.known().as_deref(), &action, &schema, &key, &accel)).await
        });
    }

    /// The row of a shortcut waits for its new keys; the window keeps the system's shortcuts from GNOME meanwhile, so
    /// that Super's combinations reach it
    fn start_recording(self: &Rc<Self>, schema: &str, key: &str, title: &str) {
        self.state.borrow_mut().recording = Some((schema.to_string(), key.to_string(), title.to_string()));
        if let Some(toplevel) = self.shell.window.surface().and_then(|surface| surface.downcast::<gdk::Toplevel>().ok()) {
            toplevel.inhibit_system_shortcuts(None::<&gdk::Event>);
        }
        self.render();
    }

    pub(crate) fn stop_recording(self: &Rc<Self>) {
        self.state.borrow_mut().recording = None;
        if let Some(toplevel) = self.shell.window.surface().and_then(|surface| surface.downcast::<gdk::Toplevel>().ok()) {
            toplevel.restore_system_shortcuts();
        }
        self.render();
    }

    /// A key pressed while a shortcut is recorded: Escape gives up, BackSpace turns the shortcut off, a modifier alone is
    /// waited past, and anything else is the shortcut's new keys, which control-core checks before they are written
    pub(crate) fn recorded(self: &Rc<Self>, key: gdk::Key, modifiers: gdk::ModifierType) {
        let Some((schema, name, _)) = self.state.borrow().recording.clone() else { return };
        let bare = (modifiers & gtk::accelerator_get_default_mod_mask()).is_empty();
        let card = "keyboard-shortcuts";
        if key == gdk::Key::Escape && bare {
            self.stop_recording();
            return;
        }
        if key == gdk::Key::BackSpace && bare {
            self.stop_recording();
            self.keybinding(card, "clear", &schema, &name, "");
            return;
        }
        let Some(accel) = recorded_accel(key, modifiers) else { return };
        self.stop_recording();
        self.keybinding(card, "set", &schema, &name, &accel);
    }

    fn keybinding_row(&self, card: &str, binding: &Keybinding, busy: bool, recording: bool) -> gtk::Box {
        let mut classes = vec!["keybinding-row"];
        if !binding.clash.is_empty() {
            classes.push("clash");
        }
        if !binding.active {
            classes.push("inactive");
        }
        if recording {
            classes.push("recording");
        }
        let row = ui::hbox(12, &classes);
        let words = ui::vbox(2, &[]);
        words.set_hexpand(true);
        if recording {
            let prompt = role_box(gtk::Orientation::Vertical, 2, &[], gtk::AccessibleRole::Status);
            prompt.append(&ui::label(&t!("Press the new keys for {title}", title = &binding.title), &["keybinding-title"]));
            prompt.append(&ui::label(&t!("Escape gives up; Backspace turns the shortcut off."), &["form-note"]));
            words.append(&prompt);
            row.append(&words);
            row.append(&ltr(ui::line(&t!("Esc"), &["keybinding-cap"])));
            let cancel = self.button(&t!("Cancel"), Look { small: true, ..Default::default() }, |this| this.stop_recording());
            cancel.add_css_class("flat");
            row.append(&cancel);
            return row;
        }
        let title = ui::label(&binding.title, &["keybinding-title"]);
        words.append(&title);
        for clash in &binding.clash {
            words.append(&ui::label(&clash_words(clash), &["form-note", "bad", "keybinding-clash"]));
        }
        if !binding.active {
            words.append(&ui::label(&t!("Works while the tiling is on."), &["form-note"]));
        }
        row.append(&words);
        let owner = ui::line(&owner_words(binding), &["tag", "keybinding-owner"]);
        owner.set_valign(gtk::Align::Center);
        row.append(&owner);
        let keys = ui::hbox(4, &["keybinding-keys"]);
        keys.set_direction(gtk::TextDirection::Ltr);
        keys.set_valign(gtk::Align::Center);
        if binding.value.is_empty() {
            keys.append(&ui::line(&t!("Off"), &["faint"]));
        }
        // The first combination, as GNOME's Settings shows it; the others are counted, and found by the search
        if let Some(accel) = binding.value.first() {
            for cap in key_caps(accel) {
                keys.append(&ui::line(&cap, &["keybinding-cap"]));
            }
        }
        if binding.value.len() > 1 {
            let others = binding.value.len() - 1;
            keys.append(&ui::line(&t!("{count, plural, one {and one more} other {and # more}}", count = others), &["faint"]));
        }
        row.append(&keys);
        let (schema, key, words_of) = (binding.schema.clone(), binding.key.clone(), binding.title.clone());
        let change = self.button(&t!("Change"), Look { small: true, disabled: busy, ..Default::default() }, move |this| {
            this.start_recording(&schema, &key, &words_of)
        });
        change.add_css_class("flat");
        change.update_property(&[gtk::accessible::Property::Label(&t!("Change the shortcut {title}", title = &binding.title))]);
        row.append(&change);
        if binding.changed {
            let (card, schema, key) = (card.to_string(), binding.schema.clone(), binding.key.clone());
            let reset = self.button(&t!("Reset"), Look { small: true, disabled: busy, ..Default::default() }, move |this| {
                this.keybinding(&card, "reset", &schema, &key, "")
            });
            reset.add_css_class("flat");
            reset.update_property(&[gtk::accessible::Property::Label(&t!("Reset the shortcut {title}", title = &binding.title))]);
            row.append(&reset);
        }
        row
    }

    fn keybindings_card(&self, item: &Item) -> gtk::Box {
        if !item.available {
            return self.card_frame(item, None, Vec::new(), false, Vec::new());
        }
        let busy = self.busy(&item.id);
        let (typed, recording, global) = {
            let state = self.state.borrow();
            (state.kb_search.clone(), state.recording.clone(), state.search.clone())
        };
        let field = gtk::Entry::builder().text(&typed).placeholder_text(t!("Find a shortcut")).css_classes(["field", "keybinding-search"]).build();
        field.update_property(&[gtk::accessible::Property::Label(&t!("Find a shortcut"))]);
        let list = ui::vbox(4, &["keybinding-rows"]);
        // The rows are made when their turn comes, whether they show or not: there are hundreds
        let entries: Rc<Vec<(String, bool)>> = Rc::new(item.keybindings.iter().map(|binding| {
            let here = recording.as_ref().is_some_and(|(schema, key, _)| *schema == binding.schema && *key == binding.key);
            (keybinding_words(binding), here)
        }).collect());
        let rows: Rc<RefCell<Vec<Option<gtk::Box>>>> = Rc::new(RefCell::new(vec![None; entries.len()]));
        let shows: Rc<RefCell<Vec<bool>>> = Rc::new(RefCell::new(vec![true; entries.len()]));
        let more = ui::label("", &["form-note"]);
        let none = ui::label(&t!("No shortcut matches."), &["row-empty"]);
        // Shown are the rows every word matches, and the one being recorded always: KEYBINDING_ROWS at most
        let rows = Rc::new(rows);
        let show = {
            let (entries, rows, shows, more, none) = (entries.clone(), rows.clone(), shows.clone(), more.downgrade(), none.downgrade());
            move |text: &str| {
                let words: Vec<String> = plain(text).split_whitespace().map(str::to_string).collect();
                let (mut shown, mut left) = (0usize, 0usize);
                for (index, (haystack, here)) in entries.iter().enumerate() {
                    let found = *here || words.iter().all(|word| haystack.contains(word.as_str()));
                    let visible = found && (*here || shown < KEYBINDING_ROWS);
                    if visible {
                        shown += 1;
                    } else if found {
                        left += 1;
                    }
                    shows.borrow_mut()[index] = visible;
                    if let Some(row) = &rows.borrow()[index] {
                        row.set_visible(visible);
                    }
                }
                if let Some(more) = more.upgrade() {
                    more.set_text(&t!("{count, plural, one {One more: type to find it.} other {# more: type to find them.}}", count = left));
                    more.set_visible(left > 0);
                }
                if let Some(none) = none.upgrade() {
                    none.set_visible(shown == 0);
                }
            }
        };
        // Found from the window's search too: its words narrow the card until the card's own field says otherwise
        show(if typed.is_empty() { &global } else { &typed });
        let mut steps: std::collections::VecDeque<Box<dyn FnOnce()>> = Default::default();
        let recording_one = recording.is_some();
        for (index, binding) in item.keybindings.iter().enumerate() {
            let here = entries[index].1;
            let make: Box<dyn FnOnce()> = {
                let (me, card, binding, list, rows, shows) = (self.me.clone(), item.id.clone(), binding.clone(), list.clone(), rows.clone(), shows.clone());
                Box::new(move || {
                    let Some(this) = me.upgrade() else { return };
                    let row = this.keybinding_row(&card, &binding, busy, here);
                    row.set_visible(shows.borrow()[index]);
                    list.append(&row);
                    rows.borrow_mut()[index] = Some(row);
                })
            };
            if recording_one || index < KEYBINDINGS_AT_ONCE {
                make();
            } else {
                steps.push_back(make);
            }
        }
        ui::in_frames(&list, KEYBINDINGS_PER_FRAME, steps);
        {
            let me = self.me.clone();
            field.connect_changed(move |field| {
                let text = field.text().to_string();
                if let Some(this) = me.upgrade() {
                    this.state.borrow_mut().kb_search = text.clone();
                }
                show(&text);
            });
        }
        let note = ui::label(
            &t!("Change one by pressing its new keys. It is written where GNOME Settings and the tiling's own window read it."),
            &["form-note"],
        );
        self.card_frame(item, None, vec![field.upcast(), note.upcast(), list.upcast(), none.upcast(), more.upcast()], false, Vec::new())
    }

    fn trust_card(&self, item: &Item) -> gtk::Box {
        let busy = self.busy(&item.id);
        let (repository, key, trusted) = {
            let state = self.state.borrow();
            (state.repository.clone(), state.key.clone(), state.trusted.clone())
        };
        let note = ui::label("", &["form-note"]);
        let card = item.id.clone();
        let trust = self.button(&item.button.clone().unwrap_or_else(|| t!("Trust…")), Look { primary: true, ..Default::default() }, move |this| {
            let repository = this.state.borrow().repository.clone();
            let that = this.clone();
            this.perform(&card, async move {
                let own = std::fs::read_to_string(backend::IMAGE_INFO).ok().and_then(|info| backend::own_repository(&info));
                let key = that.chosen_key.borrow().clone();
                let result = backend::perform(backend::trust_image(that.known().as_deref(), &repository, own.as_deref(), key.as_deref())).await;
                if result == Outcome::Terminal {
                    that.state.borrow_mut().trusted = repository;
                }
                result
            });
        });
        let available = item.available;
        let refresh = {
            let (note, trust) = (note.clone(), trust.downgrade());
            move |repository: &str, key: &str| {
                let typed = !repository.is_empty();
                let ok = backend::valid_repository(repository);
                if typed && !ok {
                    note.add_css_class("bad");
                    note.set_label(&t!("A repository such as ghcr.io/you/your-image: lower case, and no tag."));
                } else {
                    note.remove_css_class("bad");
                    note.set_label(&t!("The repository your image is published to, and the cosign.pub from it."));
                }
                if let Some(trust) = trust.upgrade() {
                    trust.set_sensitive(!busy && available && ok && !key.is_empty());
                }
            }
        };
        let field = gtk::Entry::builder().text(&repository).placeholder_text("ghcr.io/you/your-image").css_classes(["field"]).sensitive(item.available).build();
        field.set_direction(gtk::TextDirection::Ltr);
        field.update_property(&[gtk::accessible::Property::Label(&t!("The repository"))]);
        if !repository.is_empty() && !backend::valid_repository(&repository) {
            field.add_css_class("invalid");
        }
        {
            let (me, refresh) = (self.me.clone(), refresh.clone());
            field.connect_changed(move |field| {
                let Some(this) = me.upgrade() else { return };
                let repository = field.text().trim().to_string();
                if !repository.is_empty() && !backend::valid_repository(&repository) {
                    field.add_css_class("invalid");
                } else {
                    field.remove_css_class("invalid");
                }
                let key = {
                    let mut state = this.state.borrow_mut();
                    state.repository = repository.clone();
                    state.key.clone()
                };
                refresh(&repository, &key);
            });
        }
        let key_label = if key.is_empty() { t!("Choose cosign.pub…") } else { key.clone() };
        let key_button = self.button(&key_label, Look { icon: Some("key"), disabled: busy || !item.available, ..Default::default() }, |this| this.choose_key());
        refresh(&repository, &key);
        let mut below: Vec<gtk::Widget> = Vec::new();
        if item.available {
            let form = Flex::new(8, 8, &["form"]);
            form.append_with(&field, Grow { grow: 1.0, basis: Some(240), whole_line: false });
            form.append(&key_button);
            form.append(&trust);
            form.append_with(&note, Grow { whole_line: true, ..Default::default() });
            below.push(form.upcast());
        }
        if !trusted.is_empty() {
            below.push(
                Self::callout(
                    "info",
                    Some(&t!("Then switch to it, and hand the signature policy back to the image")),
                    None,
                    vec![
                        Self::command_block(&format!("sudo bootc switch --enforce-container-sigpolicy {trusted}:stable")).upcast(),
                        Self::command_block("sudo cp /usr/etc/containers/policy.json /etc/containers/policy.json").upcast(),
                        Self::actions(vec![self.link_button(&t!("Why, in the manual"), "book", |_| process::manual("developer#an-image-of-your-own")).upcast()]).upcast(),
                    ],
                )
                .upcast(),
            );
        }
        let tags = if item.available { vec![Self::terminal_tag().upcast()] } else { Vec::new() };
        self.card_frame(item, None, below, false, tags)
    }

    fn setup_diff_rows(&self) -> gtk::Widget {
        let diff = self.state.borrow().diff.clone();
        let changes = match diff {
            Diff::Unread | Diff::Reading => {
                let line = ui::hbox(4, &["row-empty"]);
                let mark = icon("spinner", 15);
                mark.set_spinning(true);
                line.append(&mark);
                line.append(&ui::label(&t!("Comparing this machine with the file…"), &[]));
                return line.upcast();
            }
            Diff::Error(error) => return Self::callout("check", Some(&t!("The file could not be compared with this machine")), Some(&error), Vec::new()).upcast(),
            Diff::Changes(changes) => changes,
        };
        if changes.is_empty() {
            return Self::callout("info", Some(&t!("This machine is as your setup file says.")), None, Vec::new()).upcast();
        }
        let mut sorted: Vec<&Value> = changes.iter().collect();
        sorted.sort_by_key(|change| change_order(&change_kind(change)));
        let section = ui::vbox(12, &["section"]);
        section.append(&ui::label(
            &t!(
                "{count, plural, one {One difference} other {# differences}} between this machine and the file. Apply adds and sets what is missing; it takes nothing away.",
                count = changes.len()
            ),
            &["soft", "small"],
        ));
        let rows = sorted
            .into_iter()
            .map(|change| {
                let kind = change_kind(change);
                let (mark, words) = match kind.as_str() {
                    "cannot" => ("!", t!("In the file, but this machine cannot have it")),
                    "add" => ("+", t!("In the file, not here")),
                    "set" => ("~", t!("Set differently here")),
                    "remove" => ("\u{2212}", t!("Here, not in the file")),
                    _ => ("", String::new()),
                };
                let row = ui::hbox(12, &["row"]);
                let sign = ui::line(mark, &["mark", &kind]);
                sign.set_xalign(0.5);
                sign.set_valign(gtk::Align::Center);
                sign.set_tooltip_text(Some(&words));
                sign.update_property(&[gtk::accessible::Property::Label(&words)]);
                row.append(&sign);
                let name = change.get("area").and_then(Value::as_str).unwrap_or("").to_string();
                let area = area(&name).unwrap_or(name);
                let ok = matches!(change.get("ok"), Some(Value::Bool(true)));
                let detail = if ok { area } else { format!("{area}: {}", change.get("why").and_then(Value::as_str).unwrap_or("")) };
                row.append(&Self::row_words(&change_name(change), Some(&detail), false, true));
                row
            })
            .collect();
        section.append(&Self::rows(rows));
        section.upcast()
    }

    fn setup_card(&self, item: &Item) -> gtk::Box {
        let busy = self.busy(&item.id);
        let saved = match item.saved {
            Some(saved) => t!("Saved {when}", when = when(saved)),
            None => t!("Not saved yet."),
        };
        let has_save = item.commands.contains_key("save");
        let has_apply = item.commands.contains_key("apply");
        let foot = Flex::new(16, 6, &["setting-foot"]);
        let file = ltr(ui::label(item.file.as_deref().unwrap_or(""), &["code"]));
        file.set_wrap_mode(gtk::pango::WrapMode::Char);
        foot.append(&file);
        foot.append(&ui::label(&saved, &[]));
        let card = item.id.clone();
        let mut actions: Vec<gtk::Widget> = vec![self
            .button(
                &if item.saved.is_some() { t!("Save again") } else { t!("Save") },
                Look { primary: item.saved.is_none(), disabled: busy || !has_save, ..Default::default() },
                move |this| this.setup(&card, "save"),
            )
            .upcast()];
        if item.saved.is_some() {
            let card = item.id.clone();
            actions.push(self.button(&t!("Apply…"), Look { icon: Some("terminal"), disabled: busy || !has_apply, ..Default::default() }, move |this| this.setup(&card, "apply")).upcast());
        }
        let mut below: Vec<gtk::Widget> = vec![foot.upcast(), Self::actions(actions).upcast()];
        if item.saved.is_some() {
            below.push(self.setup_diff_rows());
        }
        self.card_frame(item, None, below, false, Vec::new())
    }

    /// The pools of disks, each with the actions the list gave it
    fn disks_card(&self, item: &Item) -> gtk::Box {
        let busy = self.busy(&item.id);
        let below: Vec<gtk::Widget> = if item.disks.is_empty() {
            Vec::new()
        } else {
            vec![Self::rows(
                item.disks
                    .iter()
                    .map(|disk| {
                        let row = ui::hbox(12, &["row"]);
                        row.append(&Self::row_words(&disk.title, disk.text.as_deref(), false, false));
                        let tag = if disk.state == "check" {
                            let tag = ui::hbox(5, &["tag", "check"]);
                            tag.append(&icon("alert", 13));
                            tag.append(&ui::line(&t!("Needs a look"), &[]));
                            tag
                        } else {
                            let tag = ui::hbox(5, &["tag", "on"]);
                            tag.append(&ui::line(&t!("Fine"), &[]));
                            tag
                        };
                        tag.set_valign(gtk::Align::Center);
                        row.append(&tag);
                        for action in &disk.actions {
                            let (card, id, disk) = (item.id.clone(), action.id.clone(), disk.id.clone());
                            let look = Look { icon: action.terminal.then_some("terminal"), small: true, disabled: busy || !item.available, ..Default::default() };
                            row.append(&self.button(&action.title, look, move |this| this.pool(&card, &id, &disk)));
                        }
                        row
                    })
                    .collect(),
            )
            .upcast()]
        };
        self.card_frame(item, None, below, false, Vec::new())
    }

    /// Rows of one entry, each with its words, the word on its tag and the actions the list gave it: what starts at
    /// login, the apps changed in the app grid
    fn rows_card(&self, item: &Item) -> gtk::Box {
        let busy = self.busy(&item.id);
        let below: Vec<gtk::Widget> = if item.rows.is_empty() {
            Vec::new()
        } else {
            vec![Self::rows(
                item.rows
                    .iter()
                    .map(|entry| {
                        let row = ui::hbox(12, &["row"]);
                        row.append(&Self::row_words(&entry.title, entry.text.as_deref(), false, false));
                        if let Some(word) = entry.tag.as_deref() {
                            let look = match entry.state.as_str() {
                                "on" => "on",
                                "check" => "check",
                                _ => "plain",
                            };
                            let tag = ui::hbox(5, &["tag", look]);
                            tag.append(&ui::line(word, &[]));
                            tag.set_valign(gtk::Align::Center);
                            row.append(&tag);
                        }
                        for action in &entry.actions {
                            let (card, id, which) = (item.id.clone(), action.id.clone(), entry.id.clone());
                            let look = Look { icon: action.terminal.then_some("terminal"), small: true, disabled: busy || !item.available, ..Default::default() };
                            row.append(&self.button(&action.title, look, move |this| this.row_action(&card, &id, &which)));
                        }
                        row
                    })
                    .collect(),
            )
            .upcast()]
        };
        self.card_frame(item, None, below, false, Vec::new())
    }

    /// A card with a line of what was measured under its words: the helper's numbers, from nought up, as they are
    fn graph_card(&self, item: &Item) -> gtk::Box {
        let below: Vec<gtk::Widget> = if item.points.len() < 2 {
            Vec::new()
        } else {
            let area = gtk::DrawingArea::builder().hexpand(true).content_height(86).build();
            area.add_css_class("setting-graph");
            area.set_accessible_role(gtk::AccessibleRole::Img);
            area.update_property(&[gtk::accessible::Property::Label(&item.text.clone().unwrap_or_else(|| item.title.clone()))]);
            let points = item.points.clone();
            area.set_draw_func(move |area, cr, width, height| {
                let (color, width, height) = (area.color(), width as f64, height as f64);
                let (red, green, blue) = (color.red() as f64, color.green() as f64, color.blue() as f64);
                let (bottom, top) = (height - 2.0, 6.0);
                let last = points.iter().map(|point| point.0).fold(0.0, f64::max).max(1.0);
                let most = points.iter().map(|point| point.1).fold(0.0, f64::max).max(1.0);
                cr.set_source_rgba(red, green, blue, 0.25);
                cr.set_line_width(1.0);
                cr.move_to(0.0, bottom + 0.5);
                cr.line_to(width, bottom + 0.5);
                let _ = cr.stroke();
                cr.set_source_rgb(red, green, blue);
                cr.set_line_width(2.5);
                cr.set_line_cap(gtk::cairo::LineCap::Round);
                cr.set_line_join(gtk::cairo::LineJoin::Round);
                for (index, (along, value)) in points.iter().enumerate() {
                    let (x, y) = (2.0 + along / last * (width - 4.0), bottom - value / most * (bottom - top));
                    if index == 0 {
                        cr.move_to(x, y);
                    } else {
                        cr.line_to(x, y);
                    }
                }
                let _ = cr.stroke();
            });
            vec![area.upcast()]
        };
        self.card_frame(item, None, below, false, Vec::new())
    }

    fn card(&self, item: &Item) -> gtk::Box {
        match item.kind.as_str() {
            "switch" => self.switch_card(item),
            "choice" => self.choice_card(item),
            "range" => self.range_card(item),
            "link" => self.link_card(item),
            "themes" => self.themes_card(item),
            "wallpapers" => self.wallpapers_card(item),
            "theme-maker" => self.theme_maker_card(item),
            "widgets" => self.widgets_card(item),
            "webapps" => self.webapps_card(item),
            "shortcuts" => self.shortcuts_card(item),
            "keybindings" => self.keybindings_card(item),
            "trust-image" => self.trust_card(item),
            "setup" => self.setup_card(item),
            "disks" => self.disks_card(item),
            "rows" => self.rows_card(item),
            "graph" => self.graph_card(item),
            "bar" => self.bar_card(item),
            _ => self.action_card(item),
        }
    }

    // --- Pages --------------------------------------------------------------------------------------------

    fn section(&self, title: Option<&str>, items: &[&Item]) -> gtk::Box {
        let section = ui::vbox(12, &["section"]);
        if let Some(title) = title {
            // In capitals the language's own way: GTK does not measure the stylesheet's text-transform
            section.append(&heading(&title.to_uppercase(), "h2"));
        }
        let cards = ui::vbox(12, &["cards"]);
        for item in items {
            if self.draw_now.get() > 0 {
                self.draw_now.set(self.draw_now.get() - 1);
                self.place(item, &cards);
            } else {
                let (me, id, cards) = (self.me.clone(), item.id.clone(), cards.clone());
                self.to_draw.borrow_mut().push_back(Box::new(move || {
                    let Some(this) = me.upgrade() else { return };
                    let list = this.state.borrow().list.clone();
                    if let Some(item) = list.as_ref().and_then(|list| list.items.iter().find(|item| item.id == id)) {
                        this.place(item, &cards);
                    }
                }));
            }
        }
        section.append(&cards);
        section
    }

    fn drawn_as(&self, item: &Item) -> Shown {
        let state = self.state.borrow();
        let said = state.said.get(&item.id).map(|said| (said.ok, said.output.clone()));
        Shown { item: item.clone(), busy: state.busy.contains(&item.id), said }
    }

    fn place(&self, item: &Item, cards: &gtk::Box) {
        let shown = self.drawn_as(item);
        let card = self.card(item);
        // Found by its entry's id when the window is opened on it (amethystora-control PAGE ENTRY)
        card.set_widget_name(&format!("entry:{}", item.id));
        cards.append(&card);
        self.drawn.borrow_mut().cards.insert(item.id.clone(), (shown, card));
    }

    /// The page as it is, with only the cards that changed drawn again. Laying a page of cards out again takes the window
    /// away for most of a second, and a switch changes one card. False when this page is not the one to patch.
    fn patch(&self) -> bool {
        let (page, list, plain_page) = {
            let state = self.state.borrow();
            (state.page, state.list.clone(), state.search.is_empty() && state.error.is_empty())
        };
        let Some(list) = list.filter(|_| plain_page && page != backend::WELCOME) else { return false };
        let items: Vec<&Item> = list.items.iter().filter(|item| item.page == page).collect();
        let ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
        {
            let drawn = self.drawn.borrow();
            if drawn.page != page || drawn.ids != ids {
                return false;
            }
        }
        for item in items {
            let shown = self.drawn_as(item);
            let old = {
                let drawn = self.drawn.borrow();
                match drawn.cards.get(&item.id) {
                    // Its turn has not come yet: it reads the list when it does
                    None => continue,
                    Some((before, _)) if *before == shown && !reads_more(&item.kind) => continue,
                    Some((_, card)) => card.clone(),
                }
            };
            let Some(cards) = old.parent().and_then(|parent| parent.downcast::<gtk::Box>().ok()) else { continue };
            let card = self.card(item);
            card.set_widget_name(&format!("entry:{}", item.id));
            cards.insert_child_after(&card, Some(&old));
            cards.remove(&old);
            self.drawn.borrow_mut().cards.insert(item.id.clone(), (shown, card));
        }
        true
    }

    /// An entry's cards in the groups the list gives them, in its order
    fn sections(&self, list: &[&Item]) -> Vec<gtk::Box> {
        let mut groups: Vec<(String, Vec<&Item>)> = Vec::new();
        for item in list {
            let group = item.group.clone().unwrap_or_default();
            match groups.last_mut() {
                Some((last, items)) if *last == group => items.push(item),
                _ => groups.push((group, vec![item])),
            }
        }
        groups.iter().map(|(group, items)| self.section((!group.is_empty()).then_some(group.as_str()), items)).collect()
    }

    fn banner(&self) -> Option<gtk::Widget> {
        let banner: Banner = self.state.borrow().banner.clone()?;
        let wrap = ui::vbox(0, &["banner-wrap"]);
        let classes: &[&str] = if banner.failed { &["banner", "failed"] } else { &["banner"] };
        let line = role_box(gtk::Orientation::Horizontal, 12, classes, gtk::AccessibleRole::Status);
        line.append(&icon(if banner.failed { "alert" } else { "terminal" }, 18));
        let text = ui::label(&banner.text, &["grow"]);
        text.set_hexpand(true);
        line.append(&text);
        line.append(&self.icon_button("close", 16, &t!("Dismiss"), |this| {
            this.state.borrow_mut().banner = None;
            this.render();
        }));
        wrap.append(&ui::limit_center(&line, 920));
        Some(wrap.upcast())
    }

    /// The column every page is, at most 920 pixels wide in the middle
    fn page(child: &gtk::Box) -> gtk::Widget {
        adw::Clamp::builder().maximum_size(920).tightening_threshold(920).child(child).build().upcast()
    }

    fn empty(children: Vec<gtk::Widget>) -> gtk::Widget {
        let empty = ui::vbox(8, &["empty"]);
        for child in children {
            child.set_halign(if child.is::<gtk::Label>() { gtk::Align::Fill } else { gtk::Align::Center });
            empty.append(&child);
        }
        ui::limit_center(&empty, 480).upcast()
    }

    fn page_head(title: &str, lead: &str) -> gtk::Box {
        let head = ui::vbox(6, &["page-head"]);
        head.append(&heading(title, "h1"));
        head.append(&ui::limit(&pretty(lead, &["lead"]), 640));
        head
    }

    fn page_view(&self) -> gtk::Widget {
        let (page, list) = {
            let state = self.state.borrow();
            (state.page, state.list.clone())
        };
        let (title, lead) = page_words(page);
        let view = ui::vbox(28, &["page"]);
        view.append(&Self::page_head(&title, &lead));
        let items: Vec<&Item> = list.as_ref().map(|list| list.items.iter().filter(|item| item.page == page).collect()).unwrap_or_default();
        {
            let mut drawn = self.drawn.borrow_mut();
            drawn.page = page;
            drawn.ids = items.iter().map(|item| item.id.clone()).collect();
        }
        if items.is_empty() {
            view.append(&Self::empty(vec![ui::label(&t!("Nothing to set here on this machine."), &[]).upcast()]));
        } else {
            for section in self.sections(&items) {
                view.append(&section);
            }
        }
        if page == "system" {
            let line = Flex::new(4, 0, &[]);
            line.append(&ui::label(&t!("Security settings are in Amethystora Security, and automatic updates in Updates."), &["soft", "small"]));
            line.append(&self.link_button(&t!("Open Security"), "shield", |this| this.open_link("security")));
            view.append(&line);
        }
        Self::page(&view)
    }

    fn search_view(&self) -> gtk::Widget {
        let (search, list) = {
            let state = self.state.borrow();
            (state.search.clone(), state.list.clone())
        };
        let words: Vec<String> = plain(&search).split_whitespace().map(str::to_string).collect();
        let mut found: Vec<&Item> = list.as_ref().map(|list| list.items.iter().filter(|item| matches(item, &words)).collect()).unwrap_or_default();
        let view = ui::vbox(28, &["page"]);
        if found.is_empty() {
            let actions = Self::actions(vec![
                self.button(&t!("Open Security"), Look { icon: Some("shield"), small: true, ..Default::default() }, |this| this.open_link("security")).upcast(),
                self.button(&t!("Open Updates"), Look { icon: Some("refresh"), small: true, ..Default::default() }, |this| this.open_link("updates")).upcast(),
            ]);
            view.append(&Self::empty(vec![
                centered(heading(&t!("Nothing matches"), "h2")).upcast(),
                centered(ui::label(&t!("Security settings are in Amethystora Security, and automatic updates in Updates."), &[])).upcast(),
                actions.upcast(),
            ]));
            return Self::page(&view);
        }
        view.append(&Self::page_head(
            &t!("Search"),
            &t!("{count, plural, one {One setting matches} other {# settings match}} “{search}”.", count = found.len(), search = &search),
        ));
        // The settings named by the words first, then as many others as can be laid out without the window stopping
        if found.len() > SEARCH_CARDS {
            found.sort_by_key(|item| {
                let title = plain(&item.title);
                !words.iter().all(|word| title.contains(word.as_str()))
            });
            found.truncate(SEARCH_CARDS);
            view.append(&ui::label(&t!("Showing the first {shown}. Type more to narrow them down.", shown = i18n::number(SEARCH_CARDS as f64)), &["form-note"]));
        }
        for page in backend::PAGES {
            let here: Vec<&Item> = found.iter().copied().filter(|item| item.page == page).collect();
            if !here.is_empty() {
                view.append(&self.section(Some(&page_words(page).0), &here));
            }
        }
        Self::page(&view)
    }

    fn loading_view(&self) -> gtk::Widget {
        let loading = role_box(gtk::Orientation::Vertical, 0, &["loading"], gtk::AccessibleRole::Status);
        // min-height: 50vh, half the window as it is now, or as it opens
        let height = self.shell.window.height();
        loading.set_size_request(-1, if height > 0 { height / 2 } else { 420 });
        loading.set_vexpand(false);
        let middle = ui::vbox(12, &[]);
        middle.set_vexpand(true);
        middle.set_valign(gtk::Align::Center);
        middle.set_halign(gtk::Align::Center);
        let gem = Gem::plain(34.0);
        gem.add_css_class("gem");
        middle.append(&gem);
        middle.append(&ui::line(&t!("Reading the settings…"), &[]));
        loading.append(&middle);
        loading.upcast()
    }

    fn error_view(&self) -> gtk::Widget {
        let error = self.state.borrow().error.clone();
        let view = ui::vbox(28, &["page"]);
        view.append(&Self::empty(vec![
            centered(heading(&t!("The settings could not be read"), "h2")).upcast(),
            centered(ui::label(&error, &[])).upcast(),
            Self::actions(vec![self.button(&t!("Try again"), Look { icon: Some("refresh"), primary: true, ..Default::default() }, |this| this.load(false)).upcast()]).upcast(),
        ]));
        Self::page(&view)
    }

    // --- The window ---------------------------------------------------------------------------------------

    fn render_nav(&self) {
        let state = self.state.borrow();
        // Pick a look goes without the sidebar and the search; any other page has them back
        let welcome = state.page == backend::WELCOME;
        self.parts.sidebar.set_visible(!welcome);
        self.parts.search_field.set_visible(!welcome);
        for (page, button) in &self.parts.pages {
            button.set_current(state.search.is_empty() && *page == state.page);
        }
        let updates = state.list.as_ref().and_then(|list| list.automatic_updates.clone());
        let label = &self.parts.updates_state;
        label.set_label(&match updates.as_deref() {
            Some("on") => t!("On"),
            Some("off") => t!("Off"),
            _ => String::new(),
        });
        if updates.as_deref() == Some("on") {
            label.add_css_class("on");
        } else {
            label.remove_css_class("on");
        }
        self.parts.backups.set_visible(state.list.as_ref().is_some_and(|list| list.backups));
    }

    /// Drawn again in place, at the same scroll, as a change comes back
    pub(crate) fn render(&self) {
        self.render_nav();
        let adjustment = self.parts.scroller.vadjustment();
        let top = adjustment.value();
        // What had the focus goes with the page, and the focus with it to no widget, as it went to the
        // page's body: GTK would otherwise hand it to the next widget it finds, and scroll to that
        let window = &self.shell.window;
        let inside = |area: &gtk::Box| gtk::prelude::GtkWindowExt::focus(window).is_some_and(|focus| focus.is_ancestor(area));
        if inside(&self.parts.banner) {
            gtk::prelude::GtkWindowExt::set_focus(window, None::<&gtk::Widget>);
        }
        ui::clear(&self.parts.banner);
        let banner = self.banner();
        self.parts.banner.set_visible(banner.is_some());
        if let Some(banner) = banner {
            self.parts.banner.append(&banner);
        }
        if self.patch() {
            return;
        }
        if inside(&self.parts.view) {
            gtk::prelude::GtkWindowExt::set_focus(window, None::<&gtk::Widget>);
        }
        ui::clear(&self.parts.view);
        // A window opened on an entry needs it there to scroll to, and a page scrolled down what it is scrolled to
        let at_once = if self.state.borrow().reveal.is_some() { usize::MAX } else { CARDS_AT_ONCE + ((top + adjustment.page_size()) / CARD_AT_LEAST) as usize * usize::from(top > 0.0) };
        self.draw_now.set(at_once);
        self.to_draw.borrow_mut().clear();
        *self.drawn.borrow_mut() = Drawn::default();
        let (has_list, has_error, searching) = {
            let state = self.state.borrow();
            (state.list.is_some(), !state.error.is_empty(), !state.search.is_empty())
        };
        let welcome = self.state.borrow().page == backend::WELCOME;
        let content = if !has_list {
            if has_error { self.error_view() } else { self.loading_view() }
        } else if welcome {
            self.welcome_view()
        } else if searching {
            self.search_view()
        } else {
            self.page_view()
        };
        self.parts.view.append(&content);
        ui::in_frames(&content, CARDS_PER_FRAME, std::mem::take(&mut *self.to_draw.borrow_mut()));
        adjustment.set_value(top);
        // Once the page is laid out again, which may move the focus and with it the scroll
        let adjustment = adjustment.clone();
        glib::idle_add_local_once(move || {
            if (adjustment.value() - top).abs() > 0.5 {
                adjustment.set_value(top);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plain_letters() {
        assert_eq!(plain("Écran Noël"), "ecran noel");
        assert_eq!(plain("Fond d’écran"), "fond d’ecran");
        assert_eq!(plain("SCHEDULER"), "scheduler");
    }

    #[test]
    fn names_the_changes() {
        assert_eq!(change_name(&json!({"area": "packages", "container": "debian", "packages": ["a", "b"]})), "debian: a, b");
        assert_eq!(change_name(&json!({"area": "favorites", "to": ["x.desktop", "y.desktop"], "from": ["x.desktop"]})), "x.desktop, y.desktop");
        assert_eq!(change_name(&json!({"area": "favorites", "action": "remove", "from": ["x.desktop"]})), "x.desktop");
        assert_eq!(change_name(&json!({"area": "keybinding", "schema": "custom", "key": "terminal"})), "terminal");
        assert_eq!(change_name(&json!({"area": "keybinding", "schema": "org.gnome.desktop.wm.keybindings", "key": "close"})), "org.gnome.desktop.wm.keybindings close");
        assert_eq!(change_name(&json!({"area": "flatpak", "app": "org.gimp.GIMP"})), "org.gimp.GIMP");
        assert_eq!(change_name(&json!({"area": "theme", "to": "nord", "from": "amethystora"})), "nord");
        assert_eq!(change_name(&json!({"area": "image", "name": "amethystora", "to": "amethystora:stable"})), "amethystora");
        assert_eq!(change_kind(&json!({"ok": false, "action": "add"})), "cannot");
        assert_eq!(change_kind(&json!({"ok": true, "action": "set"})), "set");
        assert_eq!(change_kind(&json!({"action": "add"})), "cannot");
    }

    #[test]
    fn finds_by_every_word() {
        let item = Item {
            page: "appearance".into(),
            title: "Transition".into(),
            text: Some("How a new théme is revealed.".into()),
            choices: vec![amethystora_control_core::Choice { value: "wave".into(), title: "Wave".into(), text: Some("A wavy edge".into()), ..Default::default() }],
            ..Default::default()
        };
        let words = |text: &str| plain(text).split_whitespace().map(str::to_string).collect::<Vec<_>>();
        assert!(matches(&item, &words("theme")));
        assert!(matches(&item, &words("WAVY  revealed")));
        assert!(matches(&item, &words("appearance")));
        assert!(!matches(&item, &words("wavy scheduler")));
        assert!(matches(&item, &[]));
    }

    #[test]
    fn seconds_as_the_language_writes_them() {
        assert_eq!(seconds(1200.0), "1.2 sec");
        assert_eq!(seconds(5000.0), "5 sec");
        assert_eq!(seconds(250.0), "0.3 sec");
    }
}
