//! The list of entries, which can be thousands long: one model the gtk::ListView reads, with a day's heading
//! wherever the day changes from the entry before and the list's foot at its end, and rows that the list
//! view makes once and binds again to whatever scrolls into sight. Nothing here makes a widget per entry,
//! and the items the list view asks for are made as it asks, from the entries the page already has.
//!
//! The page's `rows()` put a heading above the first entry of each day, newest first, and the rows were
//! buttons: each row here is the heading, the entry or the foot, whichever its slot is.

use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use amethystora_ui::i18n::{self, DateStyle};
use amethystora_ui::{gdk, gio, glib, gtk, pango, t};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use regex::bytes::RegexBuilder;

use crate::backend::Entry;

/// The rows told to the list view at once, then each frame while they show, and each frame once they are below the window
const ROWS_AT_ONCE: usize = 8;
const ROWS_PER_FRAME: usize = 5;
const ROWS_ON_SCREEN: usize = 48;
const ROWS_BELOW_PER_FRAME: usize = 60;

/// What one place in the list shows
#[derive(Debug, Clone)]
pub enum Slot {
    /// The heading of a day, by the time of one of its entries (milliseconds)
    Day(i64),
    Entry(Rc<Entry>),
    /// "Show older", "Reading older entries…" or how far back the list goes
    Foot,
}

// --- The days ------------------------------------------------------------------------------------------

/// Midnight of the day an entry falls on, in seconds, worked out once for a run of entries of one day
#[derive(Default)]
struct Days {
    start: i64,
    end: i64,
}

impl Days {
    fn midnight(&mut self, millis: i64) -> i64 {
        let seconds = millis.div_euclid(1000);
        if self.end <= self.start || seconds < self.start || seconds >= self.end {
            self.start = i18n::midnight(seconds as f64) as i64;
            // The next midnight, whatever the length of the day
            self.end = i18n::midnight((self.start + 26 * 3600) as f64) as i64;
        }
        self.start
    }
}

pub fn midnight(millis: i64) -> i64 {
    Days::default().midnight(millis)
}

/// Rows for entries newest first, with a heading wherever the day changes from the one before them
fn slots(list: &[Rc<Entry>], previous: Option<i64>, out: &mut Vec<Slot>) {
    let mut days = Days::default();
    let mut day = previous.map(|time| days.midnight(time));
    for entry in list {
        let today = days.midnight(entry.time);
        if Some(today) != day {
            out.push(Slot::Day(entry.time));
            day = Some(today);
        }
        out.push(Slot::Entry(entry.clone()));
    }
}

// --- The items ---------------------------------------------------------------------------------------

mod item {
    use super::*;

    #[derive(Default)]
    pub struct Item {
        pub slot: RefCell<Option<Slot>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Item {
        const NAME: &'static str = "AmethystoraLogsItem";
        type Type = super::Item;
    }

    impl ObjectImpl for Item {}
}

glib::wrapper! {
    /// A place in the list, as the list view is handed it
    pub struct Item(ObjectSubclass<item::Item>);
}

impl Item {
    fn new(slot: Slot) -> Item {
        let item: Item = glib::Object::new();
        item.imp().slot.replace(Some(slot));
        item
    }

    pub fn slot(&self) -> Option<Slot> {
        self.imp().slot.borrow().clone()
    }
}

mod rows {
    use super::*;

    #[derive(Default)]
    pub struct Rows {
        pub slots: RefCell<Vec<Slot>>,
        /// The rows that have not been told to the list view yet
        pub waiting: RefCell<std::collections::VecDeque<Slot>>,
        /// Which filling is the current one
        pub turn: std::cell::Cell<u64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Rows {
        const NAME: &'static str = "AmethystoraLogsRows";
        type Type = super::Rows;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for Rows {}

    impl ListModelImpl for Rows {
        fn item_type(&self) -> glib::Type {
            Item::static_type()
        }

        fn n_items(&self) -> u32 {
            self.slots.borrow().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            let slot = self.slots.borrow().get(position as usize).cloned()?;
            Some(Item::new(slot).upcast())
        }
    }
}

glib::wrapper! {
    /// The list's model: the entries, their days' headings and the foot
    pub struct Rows(ObjectSubclass<rows::Rows>) @implements gio::ListModel;
}

impl Default for Rows {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Rows {
    fn slots(&self) -> std::cell::RefMut<'_, Vec<Slot>> {
        self.imp().slots.borrow_mut()
    }

    /// Everything shown, anew, the first rows at once and the others a few each frame: the list view binds what it shows as
    /// it learns of it, and told of them all it stops the window
    pub fn replace_gradually(&self, entries: &[Rc<Entry>], on: &gtk::Widget) {
        let mut next = Vec::with_capacity(entries.len() + entries.len() / 50 + 2);
        slots(entries, None, &mut next);
        if !entries.is_empty() {
            next.push(Slot::Foot);
        }
        let rest = if next.len() > ROWS_AT_ONCE { next.split_off(ROWS_AT_ONCE) } else { Vec::new() };
        let old = {
            let mut list = self.slots();
            let old = list.len();
            *list = next;
            old
        };
        let new = self.imp().slots.borrow().len();
        *self.imp().waiting.borrow_mut() = rest.into();
        let turn = self.imp().turn.get() + 1;
        self.imp().turn.set(turn);
        self.items_changed(0, old as u32, new as u32);
        if self.imp().waiting.borrow().is_empty() {
            return;
        }
        let weak = self.downgrade();
        on.add_tick_callback(move |_, _| {
            let Some(this) = weak.upgrade() else { return glib::ControlFlow::Break };
            if this.imp().turn.get() != turn {
                return glib::ControlFlow::Break;
            }
            // A few while they show, and many once they are below the window
            let at = this.imp().slots.borrow().len();
            let step = if at < ROWS_ON_SCREEN { ROWS_PER_FRAME } else { ROWS_BELOW_PER_FRAME };
            this.bring(step);
            if this.imp().waiting.borrow().is_empty() { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
        });
    }

    fn bring(&self, count: usize) {
        let added = {
            let mut waiting = self.imp().waiting.borrow_mut();
            let count = count.min(waiting.len());
            waiting.drain(..count).collect::<Vec<_>>()
        };
        if added.is_empty() {
            return;
        }
        let (at, count) = {
            let mut list = self.slots();
            let at = list.len();
            let count = added.len();
            list.extend(added);
            (at, count)
        };
        self.items_changed(at as u32, 0, count as u32);
    }

    /// Whatever is still on its way, for what changes the list as a whole
    fn all_here(&self) {
        let left = self.imp().waiting.borrow().len();
        self.bring(left);
    }

    /// Everything shown, anew
    pub fn replace(&self, entries: &[Rc<Entry>]) {
        self.imp().waiting.borrow_mut().clear();
        self.imp().turn.set(self.imp().turn.get() + 1);
        let mut next = Vec::with_capacity(entries.len() + entries.len() / 50 + 2);
        slots(entries, None, &mut next);
        if !entries.is_empty() {
            next.push(Slot::Foot);
        }
        let (old, new) = {
            let mut list = self.slots();
            let old = list.len();
            *list = next;
            (old, list.len())
        };
        self.items_changed(0, old as u32, new as u32);
    }

    pub fn clear(&self) {
        self.replace(&[]);
    }

    /// Entries written since, newest first, above the others: the day's heading moves up to above them
    pub fn prepend(&self, fresh: &[Rc<Entry>]) {
        self.all_here();
        let Some(oldest) = fresh.last() else { return };
        let mut added = Vec::new();
        slots(fresh, None, &mut added);
        let (removed, count) = {
            let mut list = self.slots();
            let removed = match list.first() {
                Some(Slot::Day(time)) if midnight(*time) == midnight(oldest.time) => 1,
                _ => 0,
            };
            let count = added.len();
            list.splice(0..removed, added);
            // The entry that was under the heading is not any more, and is drawn again with its hairline
            let again = usize::from(removed == 1 && list.len() > count);
            (removed + again, count + again)
        };
        self.items_changed(0, removed as u32, count as u32);
    }

    /// Older entries, below the others and above the foot
    pub fn append(&self, older: &[Rc<Entry>], previous: Option<i64>) {
        self.all_here();
        let mut added = Vec::new();
        slots(older, previous, &mut added);
        if added.is_empty() {
            return;
        }
        let (at, count) = {
            let mut list = self.slots();
            let at = if matches!(list.last(), Some(Slot::Foot)) { list.len() - 1 } else { list.len() };
            let count = added.len();
            list.splice(at..at, added);
            (at, count)
        };
        self.items_changed(at as u32, 0, count as u32);
    }

    /// Only the first `kept` entries, and no heading left without entries under it
    pub fn keep(&self, kept: usize) {
        self.all_here();
        let (at, removed) = {
            let mut list = self.slots();
            let mut entries = 0;
            let mut cut = None;
            for (at, slot) in list.iter().enumerate() {
                if let Slot::Entry(_) = slot {
                    entries += 1;
                    if entries > kept {
                        cut = Some(at);
                        break;
                    }
                }
            }
            let Some(mut cut) = cut else { return };
            if cut > 0 && matches!(list[cut - 1], Slot::Day(_)) {
                cut -= 1;
            }
            let end = if matches!(list.last(), Some(Slot::Foot)) { list.len() - 1 } else { list.len() };
            list.drain(cut..end);
            (cut, end - cut)
        };
        self.items_changed(at as u32, removed as u32, 0);
    }

    /// The foot, drawn again
    pub fn foot_changed(&self) {
        let at = {
            let list = self.imp().slots.borrow();
            match list.last() {
                Some(Slot::Foot) => list.len() - 1,
                _ => return,
            }
        };
        self.items_changed(at as u32, 1, 1);
    }

    /// Where an entry is in the list
    pub fn position(&self, cursor: &str) -> Option<u32> {
        self.all_here();
        self.imp().slots.borrow().iter().position(|slot| matches!(slot, Slot::Entry(entry) if &*entry.cursor == cursor)).map(|at| at as u32)
    }

    pub fn slot(&self, position: u32) -> Option<Slot> {
        self.imp().slots.borrow().get(position as usize).cloned()
    }

    pub fn len(&self) -> usize {
        self.imp().slots.borrow().len()
    }
}

// --- Words -----------------------------------------------------------------------------------------------

/// On the 24-hour clock whatever the locale, as journalctl prints it, so that the column lines up
pub fn clock(millis: i64) -> String {
    i18n::date(millis.div_euclid(1000) as f64, DateStyle::Time24Seconds)
}

/// "today" and "yesterday" in the language's own words (the heading sets them in capitals), else the date
pub fn day_name(millis: i64) -> String {
    let days = ((midnight(i18n::now() as i64 * 1000) - midnight(millis)) as f64 / 86400.0).round() as i64;
    if days == 0 || days == 1 {
        return i18n::days(-days);
    }
    let seconds = millis.div_euclid(1000) as f64;
    i18n::date(seconds, if days > 300 { DateStyle::WeekdayDayMonthYear } else { DateStyle::WeekdayDayMonth })
}

/// syslog's priorities, 0 to 7: the word for each and the colour it is drawn in
pub fn priority(level: u8) -> (String, &'static str) {
    match level {
        0 => (t!("Emergency"), "off"),
        1 => (t!("Alert"), "off"),
        2 => (t!("Critical"), "off"),
        3 => (t!("Error"), "off"),
        4 => (t!("Warning"), "check"),
        5 => (t!("Notice"), "accent"),
        6 => (t!("Info"), "info"),
        _ => (t!("Debug"), "debug"),
    }
}

pub fn first_line(text: &str) -> &str {
    text.split('\n').next().unwrap_or("")
}

/// A line as HTML showed it with white-space: nowrap: runs of spaces and tabs as one, none at either end
pub fn collapsed(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars() {
        if c == ' ' || c == '\t' {
            space = !out.is_empty();
        } else {
            if space {
                out.push(' ');
                space = false;
            }
            out.push(c);
        }
    }
    out
}

// --- What the search found ---------------------------------------------------------------------------------

/// The search, for marking it in the messages: journalctl's PCRE and this agree on most of what anyone
/// types, and where they do not nothing is marked
#[derive(Debug)]
pub enum Matcher {
    /// Text as typed, matched without regard to case
    Text(Vec<char>),
    Pattern(regex::bytes::Regex),
}

fn fold(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(single), None) => single,
        _ => c,
    }
}

impl Matcher {
    pub fn new(search: &str, regex: bool) -> Option<Matcher> {
        if search.is_empty() {
            return None;
        }
        if !regex {
            return Some(Matcher::Text(search.chars().map(fold).collect()));
        }
        // Case aside the way JavaScript's 'i' did for the letters of English; \w, \d and . as without 'u'
        RegexBuilder::new(search).unicode(false).case_insensitive(true).size_limit(1 << 20).build().ok().map(Matcher::Pattern)
    }

    /// Where in `text` the search is found, as byte ranges, at most 101 of them, as the page marked
    pub fn find(&self, text: &str) -> Vec<(usize, usize)> {
        const MOST: usize = 101;
        let mut found = Vec::new();
        match self {
            Matcher::Text(needle) => {
                let chars: Vec<(usize, char)> = text.char_indices().collect();
                let mut at = 0;
                while at + needle.len() <= chars.len() && found.len() < MOST {
                    if needle.iter().enumerate().all(|(k, &c)| fold(chars[at + k].1) == c) {
                        let start = chars[at].0;
                        let end = chars.get(at + needle.len()).map(|(i, _)| *i).unwrap_or(text.len());
                        found.push((start, end));
                        at += needle.len();
                    } else {
                        at += 1;
                    }
                }
            }
            Matcher::Pattern(pattern) => {
                for found_match in pattern.find_iter(text.as_bytes()) {
                    if found_match.is_empty() || found.len() >= MOST {
                        break;
                    }
                    // Whole characters, whatever the bytes the pattern stopped at
                    let mut start = found_match.start();
                    while !text.is_char_boundary(start) {
                        start -= 1;
                    }
                    let mut end = found_match.end();
                    while !text.is_char_boundary(end) {
                        end += 1;
                    }
                    found.push((start, end));
                }
            }
        }
        found
    }
}

// --- Right to left ---------------------------------------------------------------------------------------------

/// Text in the page's own direction: an element without dir took the page's as its paragraph's, where Pango
/// takes the first letter's, so a sentence that starts with a name ("fprintd، العملية 7230") is marked
/// right to left in a right-to-left window
pub fn in_page_direction(text: &str) -> String {
    if i18n::is_rtl_now() && pango::find_base_dir(text) == pango::Direction::Ltr {
        format!("\u{200f}{text}")
    } else {
        text.to_string()
    }
}

/// text-overflow: ellipsis cut a name at the line's end edge, which in a right-to-left window is the
/// left: the start of a name that reads left to right
pub fn ellipsize_as_the_page(label: &gtk::Label, text: &str) {
    let start = i18n::is_rtl_now() && label.direction() != gtk::TextDirection::Ltr && pango::find_base_dir(text) == pango::Direction::Ltr;
    label.set_ellipsize(if start { pango::EllipsizeMode::Start } else { pango::EllipsizeMode::End });
}

/// A word broken to wrap it is broken without the hyphen Pango would add there: CSS never adds one
fn no_hyphens() -> pango::AttrList {
    let list = pango::AttrList::new();
    list.insert(pango::AttrInt::new_insert_hyphens(false));
    list
}

/// The same, for a label that wraps
pub fn unhyphenated(label: &gtk::Label) {
    label.set_attributes(Some(&no_hyphens()));
}

/// The text's attributes with what the search found marked in `color`, as `<mark>` was
pub fn marks(text: &str, matcher: Option<&Matcher>, color: &gdk::RGBA) -> Option<pango::AttrList> {
    let list = no_hyphens();
    let found = matcher.map(|matcher| matcher.find(text)).unwrap_or_default();
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 65535.0).round() as u16;
    for (start, end) in found {
        let mut background = pango::AttrColor::new_background(channel(color.red()), channel(color.green()), channel(color.blue()));
        background.set_start_index(start as u32);
        background.set_end_index(end as u32);
        list.insert(background);
        let mut alpha = pango::AttrInt::new_background_alpha(channel(color.alpha()).max(1));
        alpha.set_start_index(start as u32);
        alpha.set_end_index(end as u32);
        list.insert(alpha);
    }
    Some(list)
}

// --- A row ---------------------------------------------------------------------------------------------------

/// The pieces of a row, made once when the list view makes the row
pub struct Parts {
    pub day: gtk::Label,
    pub entry: gtk::Box,
    pub time: gtk::Label,
    pub dot: gtk::Box,
    pub source: gtk::Label,
    pub message: gtk::Label,
    /// Where the list's one foot goes when this row is the last
    pub foot: gtk::Box,
}

mod row {
    use super::*;

    #[derive(Default)]
    pub struct SlotRow {
        pub parts: OnceCell<Parts>,
        pub slot: RefCell<Option<Slot>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SlotRow {
        const NAME: &'static str = "AmethystoraLogsRow";
        type Type = super::SlotRow;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for SlotRow {}
    impl WidgetImpl for SlotRow {}
    impl BoxImpl for SlotRow {}
}

glib::wrapper! {
    pub struct SlotRow(ObjectSubclass<row::SlotRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Default for SlotRow {
    fn default() -> Self {
        Self::new()
    }
}

impl SlotRow {
    pub fn new() -> SlotRow {
        let row: SlotRow = glib::Object::builder().property("orientation", gtk::Orientation::Vertical).build();
        let day = gtk::Label::builder().xalign(0.0).css_classes(["day"]).build();
        let time = gtk::Label::builder().xalign(0.0).css_classes(["entry-time"]).build();
        let dot = gtk::Box::builder().accessible_role(gtk::AccessibleRole::Img).css_classes(["level"]).valign(gtk::Align::Center).hexpand(false).build();
        let source = gtk::Label::builder().xalign(0.0).ellipsize(pango::EllipsizeMode::End).css_classes(["entry-source"]).build();
        let message =
            gtk::Label::builder().xalign(0.0).ellipsize(pango::EllipsizeMode::End).hexpand(true).css_classes(["entry-message"]).build();
        // The page's dir="ltr": the message reads left to right in any language
        message.set_direction(gtk::TextDirection::Ltr);
        let entry = gtk::Box::builder().spacing(14).css_classes(["entry"]).build();
        entry.append(&time);
        entry.append(&dot);
        // 170 pixels wide (130 in a narrow window, the stylesheet's min-width), whatever the name
        entry.append(&crate::layout::Exact::new(&source));
        entry.append(&message);
        let foot = gtk::Box::new(gtk::Orientation::Vertical, 0);
        row.append(&day);
        row.append(&entry);
        row.append(&foot);
        let _ = row.imp().parts.set(Parts { day, entry, time, dot, source, message, foot });
        row
    }

    pub fn parts(&self) -> &Parts {
        self.imp().parts.get().expect("made with the row")
    }

    pub fn slot(&self) -> Option<Slot> {
        self.imp().slot.borrow().clone()
    }

    pub fn cursor(&self) -> Option<Rc<str>> {
        match &*self.imp().slot.borrow() {
            Some(Slot::Entry(entry)) => Some(entry.cursor.clone()),
            _ => None,
        }
    }

    /// The row as one place in the list: `after_day` when a heading is right above it
    pub fn bind(&self, slot: &Slot, after_day: bool, selected: &str, matcher: Option<&Matcher>, mark: &gdk::RGBA, foot: &gtk::Widget) {
        let parts = self.parts();
        self.imp().slot.replace(Some(slot.clone()));
        parts.day.set_visible(matches!(slot, Slot::Day(_)));
        parts.entry.set_visible(matches!(slot, Slot::Entry(_)));
        parts.foot.set_visible(matches!(slot, Slot::Foot));
        match slot {
            Slot::Day(time) => {
                // In capitals the language's own way: GTK does not measure text-transform
                parts.day.set_label(&day_name(*time).to_uppercase());
            }
            Slot::Entry(entry) => self.bind_entry(entry, after_day, selected, matcher, mark),
            Slot::Foot => {
                if foot.parent().as_ref() != Some(parts.foot.upcast_ref()) {
                    foot.unparent();
                    parts.foot.append(foot);
                }
            }
        }
    }

    fn bind_entry(&self, entry: &Entry, after_day: bool, selected: &str, matcher: Option<&Matcher>, mark: &gdk::RGBA) {
        let parts = self.parts();
        let (word, tone) = priority(entry.priority);
        let mut classes = vec!["entry"];
        if &*entry.cursor == selected {
            classes.push("selected");
        }
        if entry.priority <= 3 {
            classes.push("bad");
        }
        if after_day {
            classes.push("after-day");
        }
        parts.entry.set_css_classes(&classes);
        let text = clock(entry.time);
        parts.time.set_label(&text);
        parts.dot.set_css_classes(&["level", tone]);
        parts.dot.set_tooltip_text(Some(&word));
        parts.dot.update_property(&[gtk::accessible::Property::Label(&word)]);
        let source = if entry.source.is_empty() { "—" } else { &entry.source };
        ellipsize_as_the_page(&parts.source, source);
        parts.source.set_label(source);
        parts.source.set_tooltip_text((!entry.source.is_empty()).then_some(entry.source.as_str()));
        let line = collapsed(first_line(entry.crash.as_deref().unwrap_or(&entry.message)));
        parts.message.set_attributes(marks(&line, matcher, mark).as_ref());
        parts.message.set_label(&line);
    }

    /// The open entry's row is tinted
    pub fn set_selected(&self, selected: &str) {
        if let Some(Slot::Entry(entry)) = &*self.imp().slot.borrow() {
            let parts = self.parts();
            if &*entry.cursor == selected {
                parts.entry.add_css_class("selected");
            } else {
                parts.entry.remove_css_class("selected");
            }
        }
    }

    /// The marks again, in the theme's new colour
    pub fn remark(&self, matcher: Option<&Matcher>, mark: &gdk::RGBA) {
        if let Some(Slot::Entry(entry)) = &*self.imp().slot.borrow() {
            let line = collapsed(first_line(entry.crash.as_deref().unwrap_or(&entry.message)));
            self.parts().message.set_attributes(marks(&line, matcher, mark).as_ref());
        }
    }

    pub fn unbind(&self, foot: &gtk::Widget) {
        let parts = self.parts();
        if foot.parent().as_ref() == Some(parts.foot.upcast_ref()) {
            parts.foot.remove(foot);
        }
        self.imp().slot.replace(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_what_the_search_found() {
        let text = Matcher::new("Fail", false).unwrap();
        assert_eq!(text.find("FAILED, then failed"), vec![(0, 4), (13, 17)]);
        let accented = Matcher::new("É", false).unwrap();
        assert_eq!(accented.find("café"), vec![(3, 5)]);
        let pattern = Matcher::new("a.c", true).unwrap();
        assert_eq!(pattern.find("ABC aXc"), vec![(0, 3), (4, 7)]);
        // An empty match stops the marking, as the page's did
        assert!(Matcher::new("x*", true).unwrap().find("abc").is_empty());
        // One that does not compile marks nothing
        assert!(Matcher::new("(", true).is_none());
        assert!(Matcher::new("", false).is_none());
        // A match never splits a character
        assert_eq!(Matcher::new("caf.", true).unwrap().find("café!"), vec![(0, 5)]);
        assert_eq!(Matcher::new("a", false).unwrap().find(&"a".repeat(300)).len(), 101);
    }

    /// The model's places, written as D (a day's heading), the entries' cursors and F (the foot)
    fn shape(rows: &Rows) -> String {
        (0..rows.len() as u32)
            .map(|at| match rows.slot(at).unwrap() {
                Slot::Day(_) => "D".to_string(),
                Slot::Entry(entry) => entry.cursor.to_string(),
                Slot::Foot => "F".to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn headings_where_the_day_changes() {
        // 2026-10-16 and the day before, at noon and after, in UTC (the tests' time zone)
        let today = 1_792_152_000_000;
        let entries = |list: &[(&str, i64)]| list.iter().map(|(cursor, time)| Rc::new(Entry::at(cursor, *time))).collect::<Vec<_>>();
        let rows = Rows::default();
        let changes = Rc::new(RefCell::new(Vec::new()));
        let seen = changes.clone();
        rows.connect_items_changed(move |_, at, removed, added| seen.borrow_mut().push((at, removed, added)));
        rows.replace(&entries(&[("a", today + 3_600_000), ("b", today), ("c", today - 86_400_000)]));
        assert_eq!(shape(&rows), "D a b D c F");
        // New entries of the same day: the heading moves up above them, and the entry under it is drawn again
        rows.prepend(&entries(&[("y", today + 7_200_000), ("x", today + 5_400_000)]));
        assert_eq!(shape(&rows), "D y x a b D c F");
        assert_eq!(changes.borrow().last(), Some(&(0, 2, 4)));
        // Older entries go above the foot, under the heading of their day if it is the last one's
        rows.append(&entries(&[("d", today - 90_000_000), ("e", today - 200_000_000)]), Some(today - 86_400_000));
        assert_eq!(shape(&rows), "D y x a b D c d D e F");
        // Only so many entries are kept, and no heading is left without its entries
        rows.keep(5);
        assert_eq!(shape(&rows), "D y x a b D c F");
        rows.keep(4);
        assert_eq!(shape(&rows), "D y x a b F");
        assert_eq!(rows.position("b"), Some(4));
        rows.clear();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn rows_as_the_page_showed_them() {
        assert_eq!(collapsed("  arsslen : TTY=pts/0 ;\tPWD=/  x  "), "arsslen : TTY=pts/0 ; PWD=/ x");
        assert_eq!(first_line("one\ntwo"), "one");
        assert_eq!(first_line(""), "");
    }
}
