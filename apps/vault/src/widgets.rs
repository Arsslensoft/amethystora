//! What the boards draw every page with: buttons, chips, cards, fields, the stone of letters in front of an item, a row of choices,
//! a sheet over the window. Each has the class vault.css styles; nothing here has words of its own.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use amethystora_ui::{adw, gtk, pango, ui};
use adw::prelude::*;
use gtk::subclass::prelude::*;
use gtk::glib;

use crate::icons::icon;

/// Characters that draw nothing or turn the text around: bidirectional controls, zero-width and format characters, line and
/// paragraph separators, tags and every control character. A name is shown without them, so that one cannot read as another
/// (`elgoog.www` under a right-to-left override) or leave a blank stone.
fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{ad}' | '\u{61c}' | '\u{180e}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}' | '\u{fff9}'..='\u{fffb}' | '\u{e0000}'..='\u{e007f}'
        )
}

/// What somebody else wrote, as it is shown: see `hidden`
pub fn clean(text: &str) -> String {
    text.chars().filter(|c| !hidden(*c)).collect()
}

/// A button with an icon before its words: `primary`, `danger` or `small` among the classes
pub fn btn(label: &str, glyph: Option<&str>, classes: &[&str], on_click: impl Fn() + 'static) -> gtk::Button {
    let mut all = vec!["btn"];
    all.extend_from_slice(classes);
    let button = ui::button(label, glyph.map(|name| icon(name, 15)), &all, on_click);
    if let Some(content) = button.child().and_then(|child| child.downcast::<gtk::Box>().ok()) {
        content.set_spacing(7);
        content.set_halign(gtk::Align::Center);
    }
    button
}

/// A square button with one icon: its label is a tooltip, and what a screen reader says
pub fn icon_btn(glyph: &str, label: &str, on_click: impl Fn() + 'static) -> gtk::Button {
    let button = gtk::Button::builder().child(&icon(glyph, 16)).css_classes(["plain", "icon-btn"]).tooltip_text(label).valign(gtk::Align::Center).build();
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    button.connect_clicked(move |_| on_click());
    button
}

/// The label of a button made by `icon_btn`, changed: its tooltip and what a screen reader says
pub fn relabel(button: &gtk::Button, label: &str) {
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
}

pub fn link(label: &str, on_click: impl Fn() + 'static) -> gtk::Button {
    ui::button(label, None, &["link"], on_click)
}

pub fn h1(text: &str) -> gtk::Label {
    let label = ui::label(text, &["h1"]);
    label.set_accessible_role(gtk::AccessibleRole::Heading);
    label
}

pub fn h2(text: &str) -> gtk::Label {
    let label = ui::label(text, &["h2"]);
    label.set_accessible_role(gtk::AccessibleRole::Heading);
    label
}

/// A sentence: `soft` and `small` among the classes as the page wants it
pub fn p(text: &str, classes: &[&str]) -> gtk::Label {
    ui::label(text, classes)
}

/// Text from outside, shown as text and never as markup, in the direction of its own letters
pub fn data(text: &str, classes: &[&str]) -> gtk::Label {
    let label = ui::line(&clean(text), classes);
    label.set_ellipsize(pango::EllipsizeMode::End);
    label
}

/// A value that reads from left to right in any language: a path, a key, a number
pub fn code(text: &str, classes: &[&str]) -> gtk::Label {
    let label = ui::label(text, &["mono"]);
    for class in classes {
        label.add_css_class(class);
    }
    label.set_direction(gtk::TextDirection::Ltr);
    label.set_selectable(true);
    label
}

pub fn card() -> gtk::Box {
    ui::vbox(12, &["card"])
}

/// A pill: `ok`, `warn`, `bad` or `accent`, or none for a plain one
pub fn chip(text: &str, tone: &str) -> gtk::Label {
    let label = ui::line(text, &["chip"]);
    if !tone.is_empty() {
        label.add_css_class(tone);
    }
    label.set_valign(gtk::Align::Center);
    label.set_halign(gtk::Align::Start);
    label
}

/// A warning in a frame of its own
pub fn callout(text: &str) -> gtk::Box {
    let row = ui::hbox(12, &["callout"]);
    let mark = icon("warn", 18);
    mark.add_css_class("warn");
    mark.set_valign(gtk::Align::Start);
    row.append(&mark);
    let words = ui::label(text, &[]);
    words.set_hexpand(true);
    row.append(&words);
    row
}

/// A label over what it names, which a screen reader reads with it
pub fn field(name: &str, control: &impl IsA<gtk::Widget>, after: &[gtk::Widget]) -> gtk::Box {
    let column = ui::vbox(5, &["field"]);
    let label = ui::line(name, &["field-label"]);
    column.append(&label);
    let row = ui::hbox(8, &[]);
    control.set_hexpand(true);
    control.upcast_ref::<gtk::Widget>().update_relation(&[gtk::accessible::Relation::LabelledBy(&[label.upcast_ref()])]);
    row.append(control);
    for widget in after {
        row.append(widget);
    }
    column.append(&row);
    column
}

pub fn entry(value: &str, placeholder: &str) -> gtk::Entry {
    let entry = gtk::Entry::builder().text(value).css_classes(["input"]).build();
    if !placeholder.is_empty() {
        entry.set_placeholder_text(Some(placeholder));
    }
    entry
}

pub fn password_entry() -> gtk::PasswordEntry {
    gtk::PasswordEntry::builder().show_peek_icon(true).css_classes(["input", "mono"]).build()
}

/// A row of two columns, the name and its value, in a grid a card holds
pub struct Pairs {
    pub grid: gtk::Grid,
    row: Cell<i32>,
}

impl Pairs {
    pub fn new() -> Pairs {
        Pairs { grid: gtk::Grid::builder().column_spacing(10).row_spacing(10).css_classes(["pairs"]).build(), row: Cell::new(0) }
    }

    pub fn add(&self, name: &str, value: &str, mono: bool) {
        let key = ui::line(name, &["pair-key"]);
        key.set_valign(gtk::Align::Baseline);
        let shown = ui::label(&clean(value), &["pair-value"]);
        shown.set_hexpand(true);
        shown.set_selectable(true);
        shown.set_valign(gtk::Align::Baseline);
        if mono {
            shown.add_css_class("mono");
            shown.set_direction(gtk::TextDirection::Ltr);
        }
        shown.update_relation(&[gtk::accessible::Relation::LabelledBy(&[key.upcast_ref()])]);
        let row = self.row.get();
        self.grid.attach(&key, 0, row, 1, 1);
        self.grid.attach(&shown, 1, row, 1, 1);
        self.row.set(row + 1);
    }
}

/// Choices side by side, one of them on, as a group of toggle buttons. `on_change` hears the value chosen, not the one it starts on.
pub fn seg(choices: &[(&str, String)], current: &str, on_change: impl Fn(&str) + 'static) -> gtk::Box {
    let row = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(2).css_classes(["seg"]).homogeneous(true).build();
    let on_change: Rc<dyn Fn(&str)> = Rc::new(on_change);
    let mut group: Option<gtk::ToggleButton> = None;
    for (value, label) in choices {
        let button = gtk::ToggleButton::builder().label(label.as_str()).css_classes(["plain", "seg-button"]).active(*value == current).build();
        if let Some(group) = &group {
            button.set_group(Some(group));
        } else {
            group = Some(button.clone());
        }
        let (value, on_change) = (value.to_string(), on_change.clone());
        button.connect_toggled(move |button| {
            if button.is_active() {
                on_change(&value);
            }
        });
        row.append(&button);
    }
    row
}

/// A choice drawn as a tile with a line under its name, one of a group: the kinds of key
pub fn tile(name: &str, sub: &str, group: &RefCell<Option<gtk::ToggleButton>>, on_chosen: impl Fn() + 'static) -> gtk::ToggleButton {
    let words = ui::vbox(2, &[]);
    words.append(&ui::line(name, &["tile-name"]));
    let line = ui::label(sub, &["tile-sub"]);
    words.append(&line);
    let button = gtk::ToggleButton::builder().child(&words).css_classes(["plain", "tile"]).hexpand(true).build();
    button.update_property(&[gtk::accessible::Property::Label(&format!("{name}. {sub}"))]);
    if let Some(first) = &*group.borrow() {
        button.set_group(Some(first));
    }
    if group.borrow().is_none() {
        group.replace(Some(button.clone()));
    }
    button.connect_toggled(move |button| {
        if button.is_active() {
            on_chosen();
        }
    });
    button
}

/// Children in rows of `columns`, of the same width
pub fn columns(columns: i32, spacing: i32, children: &[gtk::Widget]) -> gtk::Grid {
    let grid = gtk::Grid::builder().column_homogeneous(true).column_spacing(spacing).row_spacing(spacing).build();
    for (at, child) in children.iter().enumerate() {
        grid.attach(child, at as i32 % columns, at as i32 / columns, 1, 1);
    }
    grid
}

/// A check box with its words, which a click on them also turns
pub fn check(name: &str, on: bool, on_change: impl Fn(bool) + 'static) -> gtk::CheckButton {
    let button = gtk::CheckButton::builder().label(name).active(on).css_classes(["check"]).build();
    button.connect_toggled(move |button| on_change(button.is_active()));
    button
}

/// A sheet over the window, with its heading: the dialogs the boards draw over a dimmed page. libadwaita traps the keyboard in it,
/// closes it on Escape, and makes it the whole width of a narrow window.
pub struct Sheet {
    pub dialog: adw::Dialog,
    pub body: gtk::Box,
    /// The buttons, which stay in view while the body scrolls
    pub foot: gtk::Box,
}

impl Sheet {
    pub fn pin(&self, buttons: &impl IsA<gtk::Widget>) {
        self.foot.append(buttons);
    }
}

thread_local! {
    /// Every sheet that is up, held weakly, so that closing the vault can close them all
    static SHEETS: RefCell<Vec<glib::WeakRef<adw::Dialog>>> = const { RefCell::new(Vec::new()) };
}

/// Notes a dialog over the window, for `close_sheets`
pub fn track(dialog: &adw::Dialog) {
    SHEETS.with(|sheets| {
        let mut sheets = sheets.borrow_mut();
        sheets.retain(|sheet| sheet.upgrade().is_some());
        sheets.push(dialog.downgrade());
    });
}

/// Whether a sheet or a question is up over the window
pub fn any_sheet() -> bool {
    SHEETS.with(|sheets| sheets.borrow().iter().any(|sheet| sheet.upgrade().is_some_and(|dialog| dialog.is_mapped())))
}

/// Closes every sheet and question that is up: what they hold (a password being edited, a camera, a setting of a hidden vault) goes
/// with the vault
pub fn close_sheets() {
    let all: Vec<adw::Dialog> = SHEETS.with(|sheets| sheets.borrow_mut().drain(..).filter_map(|sheet| sheet.upgrade()).collect());
    for dialog in all {
        dialog.force_close();
    }
}

pub fn sheet(title: &str, width: i32) -> Sheet {
    let title = &clean(title);
    let body = ui::vbox(16, &["sheet-body"]);
    body.append(&h1(title));
    let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).propagate_natural_height(true).vexpand(true).child(&body).build();
    let foot = ui::vbox(0, &["sheet-foot"]);
    let column = ui::vbox(0, &[]);
    column.append(&scroller);
    column.append(&foot);
    let dialog = adw::Dialog::builder().title(title).content_width(width).child(&column).css_classes(["vault-sheet"]).build();
    // The buttons' handlers hold the dialog and the dialog holds the buttons: taking the child away when it closes ends that, and
    // frees what the sheet held
    dialog.connect_closed(|dialog| dialog.set_child(gtk::Widget::NONE));
    track(&dialog);
    Sheet { dialog, body, foot }
}

/// A row of buttons at the end of a sheet or a card
pub fn actions(children: &[gtk::Widget]) -> gtk::Box {
    let row = ui::hbox(8, &["actions"]);
    row.set_halign(gtk::Align::End);
    for child in children {
        row.append(child);
    }
    row
}

/// A label that says what went wrong, hidden while nothing has
pub fn problem() -> gtk::Label {
    let label = ui::label("", &["problem"]);
    label.set_visible(false);
    label.set_accessible_role(gtk::AccessibleRole::Alert);
    label
}

pub fn say(label: &gtk::Label, text: &str) {
    label.set_label(text);
    label.set_visible(!text.is_empty());
    if text.is_empty() {
        return;
    }
    // A sheet keeps the size it was shown at, so what was said last is scrolled into view
    let label = label.clone();
    glib::idle_add_local_once(move || {
        let mut parent = label.parent();
        while let Some(widget) = parent {
            if widget.has_css_class("sheet-body") {
                if let Some(scroller) = widget.parent().and_then(|view| view.parent()).and_then(|scroller| scroller.downcast::<gtk::ScrolledWindow>().ok()) {
                    let adjustment = scroller.vadjustment();
                    adjustment.set_value(adjustment.upper() - adjustment.page_size());
                }
                return;
            }
            parent = widget.parent();
        }
    });
}

// --- The stone of letters in front of an item --------------------------------------------------------------------------------

/// `hsl(h s% l%)`
fn hsl(hue: f32, saturation: f32, lightness: f32) -> gtk::gdk::RGBA {
    let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = lightness - c / 2.0;
    let (r, g, b) = match hue as u32 {
        0..60 => (c, x, 0.0),
        60..120 => (x, c, 0.0),
        120..180 => (0.0, c, x),
        180..240 => (0.0, x, c),
        240..300 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    gtk::gdk::RGBA::new(r + m, g + m, b + m, 1.0)
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Avatar {
        pub letter: RefCell<String>,
        pub hue: Cell<u32>,
        pub logo: RefCell<Option<gtk::gdk::Texture>>,
        /// The site whose logo this stone is waiting for or showing
        pub host: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Avatar {
        const NAME: &'static str = "AmethystoraVaultAvatar";
        type Type = super::Avatar;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("avatar");
            klass.set_accessible_role(gtk::AccessibleRole::Presentation);
        }
    }

    impl ObjectImpl for Avatar {
        fn constructed(&self) {
            self.parent_constructed();
            // The corners are the stylesheet's, which the desktop style scales
            self.obj().set_overflow(gtk::Overflow::Hidden);
            self.obj().set_valign(gtk::Align::Center);
        }
    }

    impl WidgetImpl for Avatar {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let (width, height) = (widget.width() as f32, widget.height() as f32);
            let bounds = gtk::graphene::Rect::new(0.0, 0.0, width, height);
            if let Some(logo) = &*self.logo.borrow() {
                // A logo is a picture of its own, on a ground that shows any of them, as large as fits and centred
                snapshot.append_color(&gtk::gdk::RGBA::new(0.97, 0.97, 0.98, 1.0), &bounds);
                let (logo_width, logo_height) = (logo.width() as f32, logo.height() as f32);
                let scale = (width / logo_width).min(height / logo_height) * 0.78;
                let (shown_width, shown_height) = (logo_width * scale, logo_height * scale);
                let fitted = gtk::graphene::Rect::new((width - shown_width) / 2.0, (height - shown_height) / 2.0, shown_width, shown_height);
                snapshot.append_scaled_texture(logo, gtk::gsk::ScalingFilter::Trilinear, &fitted);
                return;
            }
            let light = widget.root().is_some_and(|root| root.has_css_class("light"));
            let hue = self.hue.get() as f32 % 360.0;
            let (ground, ink) = if light { (hsl(hue, 0.6, 0.9), hsl(hue, 0.55, 0.28)) } else { (hsl(hue, 0.45, 0.28), hsl(hue, 0.8, 0.86)) };
            snapshot.append_color(&ground, &bounds);
            let layout = widget.create_pango_layout(Some(self.letter.borrow().as_str()));
            let (shown_width, shown_height) = layout.pixel_size();
            snapshot.save();
            snapshot.translate(&gtk::graphene::Point::new((width - shown_width as f32) / 2.0, (height - shown_height as f32) / 2.0));
            snapshot.append_layout(&layout, &ink);
            snapshot.restore();
        }
    }
}

glib::wrapper! {
    /// A letter on a colour of its own, or the service's logo once it has one
    pub struct Avatar(ObjectSubclass<imp::Avatar>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Avatar {
    pub fn new(name: &str, hue: u32) -> Avatar {
        let avatar: Avatar = glib::Object::new();
        avatar.set(name, hue, None);
        avatar
    }

    pub fn set(&self, name: &str, hue: u32, logo: Option<gtk::gdk::Texture>) {
        let imp = self.imp();
        imp.letter.replace(first_letter(name));
        imp.hue.set(hue);
        imp.logo.replace(logo);
        imp.host.replace(None);
        self.queue_draw();
    }

    /// The site this stone stands for, which a logo that arrives late is checked against
    pub fn set_host(&self, host: Option<String>) {
        self.imp().host.replace(host);
    }

    pub fn host(&self) -> Option<String> {
        self.imp().host.borrow().clone()
    }

    pub fn set_logo(&self, logo: Option<gtk::gdk::Texture>) {
        self.imp().logo.replace(logo);
        self.queue_draw();
    }
}

/// What stands for a name: its first letter, as a capital
pub fn first_letter(name: &str) -> String {
    clean(name).trim().chars().next().map_or_else(|| "?".to_string(), |letter| letter.to_uppercase().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_stands_for_itself_by_a_capital() {
        assert_eq!(first_letter(" github"), "G");
        assert_eq!(first_letter(""), "?");
        assert_eq!(first_letter("éa"), "É");
        // A zero-width first character would leave a blank stone
        assert_eq!(first_letter("\u{200b}\u{202e}google"), "G");
    }

    #[test]
    fn what_draws_nothing_or_turns_the_text_is_taken_out() {
        assert_eq!(clean("\u{202e}elgoog.www"), "elgoog.www");
        assert_eq!(clean("a\u{200b}b\u{feff}c\u{2066}d\u{2029}e\u{e0041}f\n"), "abcdef");
        assert_eq!(clean("plain é 名 ال"), "plain é 名 ال");
    }
}
