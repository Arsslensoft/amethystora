//! The two lists that can be hundreds long: a folder's entries as they were, and the timeline's points in
//! time. Each is one model a gtk::ListView reads, with rows the list view makes once and binds again to
//! whatever scrolls into sight, so that nothing makes a widget per entry. The rows are the page's own
//! (`.file-row`, `.moment` under the `.day` heading it is in), with the same classes for the stylesheet.

use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use amethystora_ui::i18n::{self, DateStyle};
use amethystora_ui::icon::Icon;
use amethystora_ui::{gio, glib, gtk, pango, t, ui};
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::backend::{Entry, Point};
use crate::icons::{self, icon};
use crate::layout::Tight;

/// What a row asks of the window
pub enum Ask {
    /// The entry of this name ticked or not
    Select(String, bool),
    /// The entry of this name ticked or not, by a click on its row
    Toggle(String),
    /// The folder of this name opened
    Open(String),
    /// The point of this id shown
    Point(String),
}

pub type Asker = Rc<dyn Fn(Ask)>;

/// A size the language's way: "4.2 kB", "512 byte"
pub fn size(bytes: Option<f64>) -> String {
    let Some(bytes) = bytes else { return String::new() };
    let units = [i18n::Unit::Byte, i18n::Unit::Kilobyte, i18n::Unit::Megabyte, i18n::Unit::Gigabyte, i18n::Unit::Terabyte];
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    i18n::unit(value, units[unit], i18n::Width::Short, 0, if unit > 0 { 1 } else { 0 })
}

pub fn time(seconds: f64) -> String {
    i18n::date(seconds, DateStyle::Time)
}

pub fn when(seconds: f64) -> String {
    i18n::date(seconds, DateStyle::MediumShort)
}

pub fn source_label(point_source: &str) -> String {
    if point_source == "local" { t!("On this machine") } else { t!("In the backup") }
}

pub fn source_icon(point_source: &str, size: i32) -> Icon {
    let mark = icon(if point_source == "local" { "disk" } else { "cloud" }, size);
    mark.add_css_class("source");
    mark
}

/// A letter of a script written right to left: Hebrew, Arabic, Syriac, Thaana and their presentation forms
fn rtl_letter(c: char) -> bool {
    matches!(c, '\u{0590}'..='\u{08ff}' | '\u{fb1d}'..='\u{fdff}' | '\u{fe70}'..='\u{feff}')
}

/// Whether a text is written right to left, as its first strong letter says
fn right_to_left(text: &str) -> bool {
    text.chars().find(|c| c.is_alphabetic()).is_some_and(rtl_letter)
}

/// Text set apart from the sentence around it, as `<bdi>` was: a name in another direction than the
/// language's keeps its own. Left as it is where it reads the same either way, so that a screen reader and
/// the tests read the words.
pub fn bdi(text: &str) -> String {
    if text.chars().any(rtl_letter) || i18n::is_rtl_now() { format!("\u{2068}{text}\u{2069}") } else { text.to_string() }
}

/// "Today", "Yesterday" or "Tuesday 6 October", in the language's words
pub fn day_of(seconds: f64) -> String {
    let days = i18n::days_from_today(seconds);
    if days == 0 || days == -1 {
        return i18n::capital(&i18n::days(days));
    }
    let same_year = i18n::local(seconds).map(|l| l.year) == i18n::local(i18n::now()).map(|l| l.year);
    i18n::date(seconds, if same_year { DateStyle::WeekdayDayMonth } else { DateStyle::WeekdayDayMonthYear })
}

/// A tick box as Chromium drew `<input type="checkbox">` with accent-color, as a button that a screen reader
/// is told is a check box, and that the accessibility tree can press (a GtkCheckButton offers no action
/// there): the box alone, or with its words after it, all of which ticks it
pub struct Tick {
    pub button: gtk::Button,
    mark: Icon,
}

impl Tick {
    pub fn new(words: Option<&str>, classes: &[&str]) -> Tick {
        let square = ui::hbox(0, &["tick"]);
        square.set_valign(gtk::Align::Center);
        square.set_hexpand(false);
        let mark = Icon::new(icons::data("check"), 14, 3.4);
        mark.set_hexpand(true);
        mark.set_visible(false);
        square.append(&mark);
        let content = ui::hbox(8, &[]);
        content.append(&square);
        if let Some(words) = words {
            content.append(&ui::line(words, &[]));
        }
        let button = gtk::Button::builder().accessible_role(gtk::AccessibleRole::Checkbox).child(&content).css_classes(["plain", "tick-button"]).build();
        for class in classes {
            button.add_css_class(class);
        }
        button.set_valign(gtk::Align::Center);
        let tick = Tick { button, mark };
        tick.set(false);
        tick
    }

    pub fn set(&self, on: bool) {
        self.mark.set_visible(on);
        if on {
            self.button.add_css_class("ticked");
        } else {
            self.button.remove_css_class("ticked");
        }
        self.button.update_state(&[gtk::accessible::State::Checked(if on { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False })]);
    }

    pub fn is_on(&self) -> bool {
        self.button.has_css_class("ticked")
    }
}

fn boxed<T: 'static>(value: T) -> glib::BoxedAnyObject {
    glib::BoxedAnyObject::new(value)
}

// --- The folder's entries ------------------------------------------------------------------------------

mod file_row {
    use super::*;

    pub struct Parts {
        pub check: Tick,
        pub glyph: Icon,
        pub folder: gtk::Button,
        pub folder_name: gtk::Label,
        pub name: gtk::Label,
        pub tag: gtk::Label,
        pub when: gtk::Label,
        pub when_column: Tight,
        pub size: gtk::Label,
    }

    #[derive(Default)]
    pub struct FileRow {
        pub parts: OnceCell<Parts>,
        pub entry: RefCell<Option<Rc<Entry>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FileRow {
        const NAME: &'static str = "AmethystoraBackupsFileRow";
        type Type = super::FileRow;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for FileRow {}
    impl WidgetImpl for FileRow {}
    impl BoxImpl for FileRow {}
}

glib::wrapper! {
    pub struct FileRow(ObjectSubclass<file_row::FileRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl FileRow {
    fn new(ask: Asker) -> FileRow {
        let row: FileRow = glib::Object::builder().property("orientation", gtk::Orientation::Horizontal).property("spacing", 12).build();
        row.add_css_class("file-row");
        let check = Tick::new(None, &[]);
        let glyph = icon("file", 20);
        glyph.add_css_class("file-icon");
        let folder_name = ui::line("", &[]);
        folder_name.set_ellipsize(pango::EllipsizeMode::End);
        let folder = gtk::Button::builder().child(&folder_name).css_classes(["plain", "file-name", "folder"]).build();
        // The whole of its column, as the page's button filled its grid cell
        folder.set_valign(gtk::Align::Center);
        folder.set_hexpand(true);
        let name = ui::line("", &["file-name"]);
        name.set_ellipsize(pango::EllipsizeMode::End);
        name.set_hexpand(true);
        let tag = ui::line("", &["tag"]);
        tag.set_valign(gtk::Align::Center);
        // Columns 150 and 76 pixels wide, as the stylesheet's min-width makes them, whatever the words in them
        let when = ui::line("", &["file-when", "faint", "small"]);
        when.set_ellipsize(pango::EllipsizeMode::End);
        let size = ui::line("", &["file-size", "faint", "small"]);
        size.set_ellipsize(pango::EllipsizeMode::Start);
        size.set_xalign(1.0);
        row.append(&check.button);
        row.append(&glyph);
        row.append(&folder);
        row.append(&name);
        row.append(&tag);
        let when_column = Tight::new(&when);
        row.append(&when_column);
        row.append(&Tight::new(&size));

        let named = |row: &FileRow| row.imp().entry.borrow().as_ref().map(|entry| entry.name.clone());
        {
            let (ask, weak) = (ask.clone(), row.downgrade());
            check.button.connect_clicked(move |_| {
                if let Some(name) = weak.upgrade().and_then(|row| named(&row)) {
                    ask(Ask::Toggle(name));
                }
            });
        }
        {
            let (ask, weak) = (ask.clone(), row.downgrade());
            folder.connect_clicked(move |_| {
                if let Some(name) = weak.upgrade().and_then(|row| named(&row)) {
                    ask(Ask::Open(name));
                }
            });
        }
        // The whole row is the tick's label, as the page's <label> was: a click anywhere on it ticks it,
        // and a double click on a folder's row opens it
        let click = gtk::GestureClick::new();
        click.set_button(1);
        {
            let weak = row.downgrade();
            click.connect_released(move |gesture, presses, _, _| {
                let Some(row) = weak.upgrade() else { return };
                let Some(entry) = row.imp().entry.borrow().clone() else { return };
                gesture.set_state(gtk::EventSequenceState::Claimed);
                ask(Ask::Toggle(entry.name.clone()));
                if presses == 2 && entry.kind == "dir" {
                    ask(Ask::Open(entry.name.clone()));
                }
            });
        }
        row.add_controller(click);
        let _ = row.imp().parts.set(file_row::Parts { check, glyph, folder, folder_name, name, tag, when, when_column, size });
        row
    }

    fn parts(&self) -> &file_row::Parts {
        self.imp().parts.get().expect("made in new")
    }

    pub fn entry(&self) -> Option<Rc<Entry>> {
        self.imp().entry.borrow().clone()
    }

    fn bind(&self, entry: &Rc<Entry>, first: bool, selected: bool, narrow: bool) {
        self.imp().entry.replace(Some(entry.clone()));
        let parts = self.parts();
        let dir = entry.kind == "dir";
        let glyph = if dir { "folder" } else if entry.kind == "link" { "link" } else { "file" };
        parts.glyph.set_data(icons::data(glyph));
        parts.glyph.set_css_classes(&["file-icon", glyph]);
        parts.folder.set_visible(dir);
        parts.name.set_visible(!dir);
        parts.folder_name.set_label(&entry.name);
        parts.name.set_label(&entry.name);
        // text-overflow: ellipsis cuts at the end of the line, which in a right-to-left window is its left:
        // the beginning of a name written left to right
        let cut = if i18n::is_rtl_now() && !right_to_left(&entry.name) { pango::EllipsizeMode::Start } else { pango::EllipsizeMode::End };
        parts.folder_name.set_ellipsize(cut);
        parts.name.set_ellipsize(cut);
        let (tag, tone) = match entry.now.as_str() {
            "missing" => (t!("Not in your folder now"), "on"),
            "changed" => (t!("Changed since"), "check"),
            "same" => (t!("Same as now"), ""),
            _ => (String::new(), ""),
        };
        parts.tag.set_visible(!tag.is_empty());
        parts.tag.set_label(&tag);
        let classes: Vec<&str> = if tone.is_empty() { vec!["tag"] } else { vec!["tag", tone] };
        parts.tag.set_css_classes(&classes);
        parts.when.set_label(&when(entry.mtime));
        parts.when_column.set_visible(!narrow);
        parts.size.set_label(&size(entry.size));
        parts.check.button.update_property(&[gtk::accessible::Property::Label(&t!("Select {name}", name = &entry.name))]);
        if first {
            self.add_css_class("first");
        } else {
            self.remove_css_class("first");
        }
        self.set_selected(selected);
    }

    /// Ticked or not, as the page's selection has it
    pub fn set_selected(&self, selected: bool) {
        let parts = self.parts();
        if parts.check.is_on() != selected {
            parts.check.set(selected);
        }
        if selected {
            self.add_css_class("selected");
        } else {
            self.remove_css_class("selected");
        }
    }
}

/// The folder's entries in a list view, with the rows it has made, to tick and untick them in place
pub struct Files {
    pub view: gtk::ListView,
    pub store: gio::ListStore,
    pub rows: Rc<RefCell<Vec<glib::WeakRef<FileRow>>>>,
    /// In a narrow window the rows have no column for when each was changed
    pub narrow: Rc<std::cell::Cell<bool>>,
}

impl Files {
    pub fn new(ask: Asker, selected: Rc<dyn Fn(&str) -> bool>) -> Files {
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let rows: Rc<RefCell<Vec<glib::WeakRef<FileRow>>>> = Rc::default();
        let narrow: Rc<std::cell::Cell<bool>> = Rc::default();
        let factory = gtk::SignalListItemFactory::new();
        {
            let rows = rows.clone();
            factory.connect_setup(move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
                let row = FileRow::new(ask.clone());
                item.set_child(Some(&row));
                item.set_activatable(false);
                item.set_focusable(false);
                rows.borrow_mut().retain(|weak| weak.upgrade().is_some());
                rows.borrow_mut().push(row.downgrade());
            });
        }
        let narrowed = narrow.clone();
        factory.connect_bind(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            let Some(row) = item.child().and_downcast::<FileRow>() else { return };
            let Some(boxed) = item.item().and_downcast::<glib::BoxedAnyObject>() else { return };
            let entry = boxed.borrow::<Rc<Entry>>().clone();
            let state = match entry.now.as_str() {
                "missing" => t!("Not in your folder now"),
                "changed" => t!("Changed since"),
                "same" => t!("Same as now"),
                _ => String::new(),
            };
            // What a screen reader says of the row: its name, and how it is now
            item.set_accessible_label(&if state.is_empty() { entry.name.clone() } else { format!("{}, {state}", entry.name) });
            row.bind(&entry, item.position() == 0, selected(&entry.name), narrowed.get());
        });
        factory.connect_unbind(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            if let Some(row) = item.child().and_downcast::<FileRow>() {
                row.imp().entry.replace(None);
                // A row kept for later says nothing of what it showed
                row.parts().check.button.update_property(&[gtk::accessible::Property::Label("")]);
            }
            item.set_accessible_label("");
        });
        let view = gtk::ListView::new(Some(gtk::NoSelection::new(Some(store.clone()))), Some(factory));
        view.add_css_class("file-rows");
        view.update_property(&[gtk::accessible::Property::Label(&t!("Folder"))]);
        Files { view, store, rows, narrow }
    }

    /// The rows made narrow or wide, as the window is
    pub fn set_narrow(&self, narrow: bool) {
        self.narrow.set(narrow);
        for row in self.rows.borrow().iter().filter_map(|weak| weak.upgrade()) {
            row.parts().when_column.set_visible(!narrow);
        }
    }

    pub fn set(&self, entries: &[Rc<Entry>]) {
        let items: Vec<glib::BoxedAnyObject> = entries.iter().map(|entry| boxed(entry.clone())).collect();
        self.store.splice(0, self.store.n_items(), &items);
    }

    /// Each row shown ticked as the selection has it
    pub fn refresh(&self, selected: &dyn Fn(&str) -> bool) {
        for row in self.rows.borrow().iter().filter_map(|weak| weak.upgrade()) {
            if let Some(entry) = row.entry() {
                row.set_selected(selected(&entry.name));
            }
        }
    }
}

// --- The timeline --------------------------------------------------------------------------------------

/// What one place in the timeline shows: a day's heading, or a point under it
pub enum Slot {
    Day(String),
    Moment { point: Rc<Point>, day: String, host: String },
}

mod timeline_row {
    use super::*;

    pub struct Parts {
        pub heading: gtk::Label,
        pub moment: gtk::Button,
        pub time: gtk::Label,
        pub host: gtk::Label,
        pub source: Icon,
    }

    #[derive(Default)]
    pub struct TimelineRow {
        pub parts: OnceCell<Parts>,
        pub id: RefCell<Option<String>>,
        pub day: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TimelineRow {
        const NAME: &'static str = "AmethystoraBackupsTimelineRow";
        type Type = super::TimelineRow;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for TimelineRow {}
    impl WidgetImpl for TimelineRow {}
    impl BoxImpl for TimelineRow {}
}

glib::wrapper! {
    pub struct TimelineRow(ObjectSubclass<timeline_row::TimelineRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl TimelineRow {
    fn new(ask: Asker) -> TimelineRow {
        let row: TimelineRow = glib::Object::builder().property("orientation", gtk::Orientation::Vertical).build();
        row.add_css_class("timeline-row");
        let heading = gtk::Label::builder().xalign(0.0).accessible_role(gtk::AccessibleRole::Heading).css_classes(["day-heading"]).build();
        heading.set_ellipsize(pango::EllipsizeMode::End);
        let content = ui::hbox(10, &["content"]);
        let dot = ui::hbox(0, &["moment-dot"]);
        dot.set_valign(gtk::Align::Center);
        dot.set_hexpand(false);
        content.append(&dot);
        let time = ui::line("", &["moment-time"]);
        content.append(&time);
        let host = ui::line("", &["faint", "small"]);
        host.set_hexpand(true);
        host.set_ellipsize(pango::EllipsizeMode::End);
        content.append(&host);
        let source = source_icon("local", 16);
        content.append(&source);
        let moment = gtk::Button::builder().child(&content).css_classes(["plain", "moment"]).build();
        {
            let weak = row.downgrade();
            moment.connect_clicked(move |_| {
                if let Some(id) = weak.upgrade().and_then(|row| row.imp().id.borrow().clone()) {
                    ask(Ask::Point(id));
                }
            });
        }
        row.append(&heading);
        row.append(&moment);
        let _ = row.imp().parts.set(timeline_row::Parts { heading, moment, time, host, source });
        row
    }

    fn parts(&self) -> &timeline_row::Parts {
        self.imp().parts.get().expect("made in new")
    }

    /// The day this row's place is in
    pub fn day(&self) -> String {
        self.imp().day.borrow().clone()
    }

    /// Whether this row is a day's heading
    pub fn is_heading(&self) -> bool {
        self.parts().heading.is_visible()
    }

    fn bind(&self, slot: &Slot, current: &str, last: bool) {
        let parts = self.parts();
        match slot {
            Slot::Day(day) => {
                self.imp().id.replace(None);
                self.imp().day.replace(day.clone());
                parts.heading.set_label(day);
                parts.heading.set_visible(true);
                parts.moment.set_visible(false);
            }
            Slot::Moment { point, day, host } => {
                self.imp().id.replace(Some(point.id.clone()));
                self.imp().day.replace(day.clone());
                parts.heading.set_visible(false);
                parts.moment.set_visible(true);
                parts.moment.set_css_classes(&["plain", "moment", if point.source == "local" { "local" } else { "backup" }]);
                parts.time.set_label(&time(point.time));
                parts.host.set_label(&bdi(host));
                parts.source.set_data(icons::data(if point.source == "local" { "disk" } else { "cloud" }));
                let source = source_label(&point.source);
                let title = if host.is_empty() { source } else { format!("{source} ({host})") };
                parts.moment.set_tooltip_text(Some(&title));
                // What a screen reader says of it: the day, the time and where it is kept, as the page
                // showed them around it
                parts.moment.update_property(&[gtk::accessible::Property::Label(&format!("{day}, {}, {title}", time(point.time)))]);
                self.set_current(current);
            }
        }
        if last {
            self.add_css_class("last");
        } else {
            self.remove_css_class("last");
        }
    }

    /// The point shown marked as aria-current was
    pub fn set_current(&self, current: &str) {
        let parts = self.parts();
        let here = self.imp().id.borrow().as_deref() == Some(current);
        if here {
            parts.moment.add_css_class("current");
        } else {
            parts.moment.remove_css_class("current");
        }
        parts.moment.update_state(&[gtk::accessible::State::Selected(Some(here))]);
    }
}

/// The points, a day's heading wherever the day changes, in a list view
pub struct Points {
    pub view: gtk::ListView,
    pub store: gio::ListStore,
    pub rows: Rc<RefCell<Vec<glib::WeakRef<TimelineRow>>>>,
}

impl Points {
    pub fn new(ask: Asker, current: Rc<dyn Fn() -> String>) -> Points {
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let rows: Rc<RefCell<Vec<glib::WeakRef<TimelineRow>>>> = Rc::default();
        let factory = gtk::SignalListItemFactory::new();
        {
            let rows = rows.clone();
            factory.connect_setup(move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
                let row = TimelineRow::new(ask.clone());
                item.set_child(Some(&row));
                item.set_activatable(false);
                item.set_focusable(false);
                rows.borrow_mut().retain(|weak| weak.upgrade().is_some());
                rows.borrow_mut().push(row.downgrade());
            });
        }
        {
            let store = store.clone();
            factory.connect_bind(move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
                let Some(row) = item.child().and_downcast::<TimelineRow>() else { return };
                let Some(boxed) = item.item().and_downcast::<glib::BoxedAnyObject>() else { return };
                let slot = boxed.borrow::<Slot>();
                let last = item.position() + 1 == store.n_items();
                if let Slot::Day(day) = &*slot {
                    item.set_accessible_label(day);
                }
                row.bind(&slot, &current(), last);
            });
        }
        factory.connect_unbind(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            if let Some(row) = item.child().and_downcast::<TimelineRow>() {
                // A row kept for later says nothing of the point it showed
                row.imp().id.replace(None);
                row.parts().moment.update_property(&[gtk::accessible::Property::Label("")]);
            }
            item.set_accessible_label("");
        });
        let view = gtk::ListView::new(Some(gtk::NoSelection::new(Some(store.clone()))), Some(factory));
        view.add_css_class("timeline-list");
        view.update_property(&[gtk::accessible::Property::Label(&t!("Points in time"))]);
        Points { view, store, rows }
    }

    /// The points, newest first, under the day each is in; `hostname` is this machine's, whose own
    /// backups say no name
    pub fn set(&self, points: &[crate::backend::Point], hostname: &str) {
        let mut items = Vec::with_capacity(points.len() + 16);
        let mut last_day: Option<String> = None;
        for point in points {
            let day = day_of(point.time);
            if last_day.as_ref() != Some(&day) {
                items.push(boxed(Slot::Day(day.clone())));
                last_day = Some(day.clone());
            }
            // A point made by another machine that backs up to the same repository says which
            let host = match point.host.as_deref() {
                Some(host) if !host.is_empty() && !hostname.is_empty() && host != hostname => host.to_string(),
                _ => String::new(),
            };
            items.push(boxed(Slot::Moment { point: Rc::new(point.clone()), day, host }));
        }
        self.store.splice(0, self.store.n_items(), &items);
    }

    pub fn refresh(&self, current: &str) {
        for row in self.rows.borrow().iter().filter_map(|weak| weak.upgrade()) {
            row.set_current(current);
        }
    }

    /// The timeline scrolled to a point, which a walk through the past may have gone beyond
    pub fn show(&self, id: &str) {
        let at = (0..self.store.n_items()).find(|at| {
            self.store.item(*at).and_downcast::<glib::BoxedAnyObject>().is_some_and(|boxed| matches!(&*boxed.borrow::<Slot>(), Slot::Moment { point, .. } if point.id == id))
        });
        if let Some(at) = at {
            self.view.scroll_to(at, gtk::ListScrollFlags::NONE, None);
        }
    }

    /// The row at a height in the list's own view, from its top
    pub fn row_at(&self, y: f64) -> Option<TimelineRow> {
        let x = self.view.width() as f64 / 2.0;
        self.view.pick(x, y, gtk::PickFlags::DEFAULT).and_then(|widget| widget.ancestor(TimelineRow::static_type())).and_downcast::<TimelineRow>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_as_the_page_wrote_them() {
        assert_eq!(size(None), "");
        assert_eq!(size(Some(24.0)), "24 byte");
        assert_eq!(size(Some(4210.0)), "4.2 kB");
        assert_eq!(size(Some(48211.0)), "48.2 kB");
        assert_eq!(size(Some(3481022.0)), "3.5 MB");
        assert_eq!(size(Some(33554432.0)), "33.6 MB");
        assert_eq!(size(Some(999.0)), "999 byte");
        assert_eq!(size(Some(1000.0)), "1 kB");
    }
}
