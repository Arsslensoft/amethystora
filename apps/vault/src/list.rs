//! The list every kind of item has: a search over a `gtk::ListView`, which only ever makes the rows on the screen, and the pane
//! beside it. A row is what the service's `list` said of an item, which holds no secret, and searching only reads what was folded
//! once when the row was made, so it stays well under a frame at 1,000 items.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::time::Instant;

use amethystora_ui::{gdk, gio, glib, gtk, i18n, t, ui};
use gtk::prelude::*;
use serde_json::Value;

use crate::icons::icon;
use crate::widgets::{self, Avatar};

/// One item, as the list shows it
pub struct Row {
    pub id: String,
    pub approval: String,
    pub title: String,
    pub sub: String,
    pub hue: u32,
    /// A chip at the end of the row, and its tone
    pub tag: Option<(String, &'static str)>,
    /// What the search looks through: title, line and tags, in lowercase
    pub words: String,
    pub meta: Value,
    /// When it last changed, in seconds, as the service keeps it
    pub changed: i64,
    /// The vault it is in
    pub vault: String,
    /// The site a logo is asked of, when logos are on and the vault is not a hidden one
    pub host: Option<String>,
}

impl Row {
    /// From an entry of `list`, with the words of the page's own kind
    pub fn new(entry: &Value, title: String, sub: String, tag: Option<(String, &'static str)>) -> Row {
        let meta = entry.get("meta").cloned().unwrap_or(Value::Null);
        // Somebody else's words are shown without what draws nothing or turns the text around
        let (title, sub) = (widgets::clean(&title), widgets::clean(&sub));
        let mut words = format!("{title}\n{sub}").to_lowercase();
        for tag in meta["tags"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            words.push('\n');
            words.push_str(&tag.to_lowercase());
        }
        Row {
            id: text(entry, "id"),
            approval: text(entry, "approval"),
            hue: amethystora_vault_core::codes::hue(&title),
            title,
            sub,
            tag,
            words,
            meta,
            changed: entry["changed"].as_i64().unwrap_or(0),
            vault: text(entry, "vault"),
            host: None,
        }
    }

    fn matches(&self, query: &[String]) -> bool {
        query.iter().all(|word| self.words.contains(word.as_str()))
    }
}

pub fn text(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or("").to_string()
}

fn row_of(object: &glib::Object) -> Option<glib::BoxedAnyObject> {
    object.clone().downcast::<glib::BoxedAnyObject>().ok()
}

/// A row on the screen: stone, two lines, a chip
fn make_row() -> gtk::Box {
    let row = ui::hbox(11, &["item"]);
    row.append(&Avatar::new("", 0));
    let lines = ui::vbox(0, &["item-lines"]);
    lines.set_hexpand(true);
    lines.set_valign(gtk::Align::Center);
    lines.append(&widgets::data("", &["item-title"]));
    lines.append(&widgets::data("", &["item-sub"]));
    row.append(&lines);
    let tag = widgets::chip("", "");
    tag.set_visible(false);
    row.append(&tag);
    row
}

fn bind_row(row: &gtk::Box, item: &Row) {
    let Some(avatar) = row.first_child().and_then(|child| child.downcast::<Avatar>().ok()) else { return };
    avatar.set(&item.title, item.hue, None);
    crate::logos::attach(&avatar, item.host.as_deref());
    let Some(lines) = avatar.next_sibling() else { return };
    if let Some(title) = lines.first_child().and_then(|child| child.downcast::<gtk::Label>().ok()) {
        title.set_label(&item.title);
        if let Some(sub) = title.next_sibling().and_then(|child| child.downcast::<gtk::Label>().ok()) {
            sub.set_label(&item.sub);
            sub.set_visible(!item.sub.is_empty());
        }
    }
    if let Some(tag) = lines.next_sibling().and_then(|child| child.downcast::<gtk::Label>().ok()) {
        for tone in ["ok", "warn", "bad", "accent"] {
            tag.remove_css_class(tone);
        }
        match &item.tag {
            Some((words, tone)) => {
                tag.set_label(words);
                if !tone.is_empty() {
                    tag.add_css_class(tone);
                }
                tag.set_visible(true);
            }
            None => tag.set_visible(false),
        }
    }
}

/// A list of one kind of item and the detail of the one chosen
pub struct ListPage {
    /// The whole page: list and detail, or one of them in a narrow window
    pub root: gtk::Box,
    pub pane: gtk::Box,

    /// What goes over the list: a warning, the coverage
    pub above: gtk::Box,
    pub search: gtk::Entry,
    pub store: gio::ListStore,
    pub filter: gtk::CustomFilter,
    pub selection: gtk::SingleSelection,
    pub view: gtk::ListView,
    pub detail: gtk::ScrolledWindow,
    /// A row's id, when one is shown
    pub current: RefCell<Option<String>>,
    /// One pane at a time, in a narrow window
    pub narrow: Cell<bool>,
    pub empty: gtk::Label,
    query: Rc<RefCell<Vec<String>>>,
    on_select: RefCell<Option<Rc<dyn Fn(Option<&Row>)>>>,
    /// While a page has hidden a row that it is about to remove, for its Undo
    pub hidden_id: Rc<RefCell<HashSet<String>>>,
    /// A detail is waiting to be drawn once the frame is
    drawing: Cell<bool>,
    /// An item asked for before the list came
    wanted: RefCell<Option<String>>,
    me: RefCell<std::rc::Weak<ListPage>>,
}

impl ListPage {
    pub fn new(hint: &str, add_label: &str, on_add: impl Fn() + 'static, empty_words: &str) -> Rc<ListPage> {
        let root = ui::hbox(0, &["list-page"]);
        let pane = ui::vbox(12, &["list-pane"]);
        // The search says it would like the room, which would make the list as wide as the detail beside it
        pane.set_hexpand(false);
        pane.update_property(&[gtk::accessible::Property::Label(hint)]);

        let head = ui::hbox(8, &["list-head"]);
        let find = ui::hbox(8, &["find"]);
        find.set_hexpand(true);
        find.append(&icon("search", 15));
        let search = gtk::Entry::builder().placeholder_text(hint).css_classes(["plain"]).hexpand(true).build();
        search.update_property(&[gtk::accessible::Property::Label(hint)]);
        find.append(&search);
        head.append(&find);
        head.append(&widgets::icon_btn("plus", add_label, on_add));
        pane.append(&head);
        let above = ui::vbox(8, &[]);
        above.set_visible(false);
        pane.append(&above);

        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let query: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let hidden_id: Rc<RefCell<HashSet<String>>> = Rc::new(RefCell::new(HashSet::new()));
        let (words, hidden) = (query.clone(), hidden_id.clone());
        let filter = gtk::CustomFilter::new(move |object| {
            let Some(boxed) = object.downcast_ref::<glib::BoxedAnyObject>() else { return false };
            let row = boxed.borrow::<Row>();
            !hidden.borrow().contains(&row.id) && row.matches(&words.borrow())
        });
        let filtered = gtk::FilterListModel::new(Some(store.clone()), Some(filter.clone()));
        let selection = gtk::SingleSelection::builder().model(&filtered).autoselect(false).can_unselect(true).build();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
                item.set_child(Some(&make_row()));
            }
        });
        factory.connect_bind(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            let (Some(row), Some(boxed)) = (item.child().and_then(|child| child.downcast::<gtk::Box>().ok()), item.item().as_ref().and_then(row_of)) else { return };
            let data = boxed.borrow::<Row>();
            bind_row(&row, &data);
            item.set_accessible_label(&if data.sub.is_empty() { data.title.clone() } else { format!("{}, {}", data.title, data.sub) });
        });
        let view = gtk::ListView::builder().model(&selection).factory(&factory).css_classes(["items"]).build();
        view.update_property(&[gtk::accessible::Property::Label(hint)]);
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&view).build();
        pane.append(&scroller);
        let empty = ui::label(empty_words, &["soft", "small", "empty-list"]);
        empty.set_visible(false);
        pane.append(&empty);
        root.append(&pane);
        let detail = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).hexpand(true).vexpand(true).build();
        detail.add_css_class("detail-pane");
        root.append(&detail);

        let page = Rc::new(ListPage {
            root,
            pane,

            above,
            search,
            store,
            filter,
            selection,
            view,
            detail,
            current: RefCell::new(None),
            narrow: Cell::new(false),
            empty,
            query,
            on_select: RefCell::new(None),
            hidden_id,
            drawing: Cell::new(false),
            wanted: RefCell::new(None),
            me: RefCell::new(std::rc::Weak::new()),
        });
        page.me.replace(Rc::downgrade(&page));
        let weak = Rc::downgrade(&page);
        page.selection.connect_selected_item_notify(move |selection| {
            if let Some(page) = weak.upgrade() {
                page.selected(selection.selected_item().as_ref());
            }
        });
        let weak = Rc::downgrade(&page);
        page.search.connect_changed(move |search| {
            if let Some(page) = weak.upgrade() {
                let started = Instant::now();
                let words: Vec<String> = search.text().to_lowercase().split_whitespace().map(str::to_string).collect();
                // A row matches when each word is in it: a query whose every word is inside one of the other's words matches
                // no more rows than the other, so only the rows still shown, or only the rows hidden, are looked at again
                let inside = |wide: &[String], narrow: &[String]| narrow.iter().all(|word| wide.iter().any(|wider| wider.contains(word.as_str())));
                let change = {
                    let old = page.query.borrow();
                    if inside(&words, &old) {
                        gtk::FilterChange::MoreStrict
                    } else if inside(&old, &words) {
                        gtk::FilterChange::LessStrict
                    } else {
                        gtk::FilterChange::Different
                    }
                };
                page.query.replace(words);
                page.filter.changed(change);
                crate::measure::took("filter changed", started);
                page.after_filter();
                crate::measure::search(&page.view, page.selection.n_items(), started);
            }
        });
        // Down from the search goes to the list, Escape empties it
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&page);
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(page) = weak.upgrade() else { return glib::Propagation::Proceed };
            match key {
                gdk::Key::Escape if !page.search.text().is_empty() => {
                    page.search.set_text("");
                    glib::Propagation::Stop
                }
                gdk::Key::Down => {
                    if page.selection.n_items() > 0 {
                        page.view.scroll_to(0, gtk::ListScrollFlags::FOCUS | gtk::ListScrollFlags::SELECT, None);
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        page.search.add_controller(keys);
        page
    }

    /// What to draw in the detail when the row chosen changes, None when nothing is chosen
    pub fn on_select(&self, render: impl Fn(Option<&Row>) + 'static) {
        self.on_select.replace(Some(Rc::new(render)));
    }

    fn selected(&self, object: Option<&glib::Object>) {
        let boxed = object.and_then(row_of);
        let row = boxed.as_ref().map(|boxed| boxed.borrow::<Row>());
        let id = row.as_ref().map(|row| row.id.clone());
        if *self.current.borrow() == id {
            return;
        }
        self.current.replace(id);
        self.apply_narrow();
        // After the frame that shows the choice, so that typing in the search never waits for a detail it is about to replace
        if !self.drawing.replace(true) {
            let weak = self.me.borrow().clone();
            glib::idle_add_local_once(move || {
                if let Some(page) = weak.upgrade() {
                    page.draw_detail();
                }
            });
        }
    }

    fn draw_detail(&self) {
        self.drawing.set(false);
        let render = self.on_select.borrow().clone();
        let Some(render) = render else { return };
        let boxed = self.selection.selected_item().as_ref().and_then(row_of);
        let row = boxed.as_ref().map(|boxed| boxed.borrow::<Row>());
        let started = Instant::now();
        render(row.as_deref());
        crate::measure::took("detail drawn", started);
    }

    /// The rows anew: the one chosen stays chosen if it is still there, and its detail is drawn again
    pub fn set_rows(&self, rows: Vec<Row>) {
        let keep = self.current.borrow().clone();
        let objects: Vec<glib::BoxedAnyObject> = rows.into_iter().map(glib::BoxedAnyObject::new).collect();
        self.current.replace(None);
        self.store.splice(0, self.store.n_items(), &objects);
        self.after_filter();
        let at = keep.and_then(|keep| self.position_of(&keep));
        self.selection.set_selected(at.unwrap_or(gtk::INVALID_LIST_POSITION));
        // set_selected says nothing when the position is what it was, though the row is another
        if self.current.borrow().is_none() {
            self.selected(self.selection.selected_item().as_ref());
        }
        // Something was asked for before the list was here
        if let Some(wanted) = self.wanted.take() {
            if self.position_of(&wanted).is_some() {
                self.select_id(&wanted);
            }
        }
    }

    /// Gives rows their sites once they are known (they come from a file read off the window's thread), and draws the rows again with
    /// them. The row chosen stays chosen.
    pub fn set_hosts(&self, host_of: impl Fn(&Row) -> Option<String>) {
        let count = self.store.n_items();
        for at in 0..count {
            if let Some(boxed) = self.store.item(at).as_ref().and_then(row_of) {
                let host = host_of(&boxed.borrow::<Row>());
                boxed.borrow_mut::<Row>().host = host;
            }
        }
        let keep = self.current.borrow().clone();
        self.store.items_changed(0, count, count);
        if let Some(keep) = keep {
            if self.current.borrow().is_none() {
                if let Some(at) = self.position_of(&keep) {
                    self.selection.set_selected(at);
                }
            }
        }
    }

    fn after_filter(&self) {
        let none = self.selection.n_items() == 0;
        self.empty.set_visible(none);
        if none && self.store.n_items() > 0 {
            self.empty.set_label(&t!("Nothing here matches that."));
        }
    }

    pub fn position_of(&self, id: &str) -> Option<u32> {
        (0..self.selection.n_items()).find(|at| {
            self.selection.item(*at).as_ref().and_then(row_of).is_some_and(|boxed| boxed.borrow::<Row>().id == id)
        })
    }

    /// What the list says of an item, its row's `meta`
    pub fn row_meta(&self, id: &str) -> Option<Value> {
        (0..self.store.n_items()).find_map(|at| {
            let boxed = self.store.item(at).as_ref().and_then(row_of)?;
            let row = boxed.borrow::<Row>();
            (row.id == id).then(|| row.meta.clone())
        })
    }

    /// Every item's id and the number its `order` has now, in the list's order
    pub fn orders(&self) -> Vec<(String, Option<i64>)> {
        (0..self.store.n_items())
            .filter_map(|at| {
                let boxed = self.store.item(at).as_ref().and_then(row_of)?;
                let row = boxed.borrow::<Row>();
                Some((row.id.clone(), row.meta["order"].as_i64()))
            })
            .collect()
    }

    /// The name and vault of every item, for finding the sites they stand for
    pub fn titles(&self) -> Vec<(String, String)> {
        (0..self.store.n_items())
            .filter_map(|at| {
                let boxed = self.store.item(at).as_ref().and_then(row_of)?;
                let row = boxed.borrow::<Row>();
                Some((row.title.clone(), row.vault.clone()))
            })
            .collect()
    }

    /// Chooses an item; one that is not in the list yet is chosen when the list next comes
    pub fn select_id(&self, id: &str) {
        if let Some(at) = self.position_of(id) {
            self.selection.set_selected(at);
            self.view.scroll_to(at, gtk::ListScrollFlags::NONE, None);
            self.wanted.replace(None);
        } else {
            self.wanted.replace(Some(id.to_string()));
        }
    }

    pub fn select_first(&self) {
        if self.selection.n_items() > 0 && self.selection.selected() == gtk::INVALID_LIST_POSITION {
            self.selection.set_selected(0);
        }
    }

    /// Hides a row while its removal can be undone
    pub fn hide_row(&self, id: &str) {
        self.hidden_id.borrow_mut().insert(id.to_string());
        self.filter.changed(gtk::FilterChange::Different);
        self.after_filter();
    }

    /// Shows a row again: its removal was undone, or has been done
    pub fn show_row(&self, id: &str) {
        self.hidden_id.borrow_mut().remove(id);
        self.filter.changed(gtk::FilterChange::Different);
        self.after_filter();
    }

    /// Replaces what the detail shows
    pub fn show(&self, content: &impl IsA<gtk::Widget>) {
        self.detail.set_child(Some(content));
    }

    pub fn clear(&self) {
        if self.store.n_items() == 0 {
            self.detail.set_child(gtk::Widget::NONE);
            return;
        }
        let words = ui::label(&t!("Choose one from the list."), &["soft", "empty-detail"]);
        words.set_halign(gtk::Align::Center);
        words.set_valign(gtk::Align::Center);
        self.detail.set_child(Some(&words));
    }

    /// In a narrow window, a row chosen shows its detail in place of the list
    pub fn set_narrow(&self, narrow: bool) {
        let before = self.narrow.replace(narrow);
        self.apply_narrow();
        // The detail has a way back to the list only in a narrow window, so it is drawn again
        if before != narrow && self.current.borrow().is_some() && !self.drawing.replace(true) {
            let weak = self.me.borrow().clone();
            glib::idle_add_local_once(move || {
                if let Some(page) = weak.upgrade() {
                    page.draw_detail();
                }
            });
        }
    }

    pub fn apply_narrow(&self) {
        let detail = self.current.borrow().is_some();
        self.pane.set_visible(!self.narrow.get() || !detail);
        self.detail.set_visible(!self.narrow.get() || detail);
        if self.narrow.get() {
            self.pane.add_css_class("alone");
        } else {
            self.pane.remove_css_class("alone");
        }
    }

    /// Back to the list, from a detail in a narrow window
    pub fn back(&self) {
        self.selection.set_selected(gtk::INVALID_LIST_POSITION);
    }
}

/// The date something happened, in the language's own way
pub fn long_date(unix_ms: i64) -> String {
    i18n::date(unix_ms as f64 / 1000.0, i18n::DateStyle::DayMonthYear)
}
