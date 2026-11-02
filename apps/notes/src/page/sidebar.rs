//! The sidebar, as index.html and notes.js built it: the wordmark, Notes or Tasks, the search, the sections of
//! the one shown (notebooks and tags, or the task views, projects and labels), and the settings, the lock and
//! the manual at its foot.

use std::rc::Rc;

use amethystora_ui::{brand, gdk, glib, gtk, i18n, process, t, ui};
use gtk::prelude::*;
use serde_json::Value;

use super::widgets::{self, icon_button};
use super::{Mode, Notes, View};
use crate::data::{Fields, Kind};
use crate::icons::icon;

/// What a row of the sidebar is: its words, its icon or colour, its count, whether it is the view shown,
/// what a click does, its menu, its depth and twisty, and the notes dropped on it
pub struct NavItem {
    pub label: String,
    pub glyph: &'static str,
    pub color: Option<String>,
    pub count: Option<usize>,
    pub current: bool,
    pub on_click: Rc<dyn Fn()>,
    pub on_menu: Option<Rc<dyn Fn(&gtk::Button)>>,
    pub depth: usize,
    pub toggle: Option<gtk::Widget>,
    pub drop: Option<Rc<dyn Fn(String)>>,
}

impl NavItem {
    pub fn new(label: &str, glyph: &'static str, on_click: impl Fn() + 'static) -> NavItem {
        NavItem { label: label.into(), glyph, color: None, count: None, current: false, on_click: Rc::new(on_click), on_menu: None, depth: 0, toggle: None, drop: None }
    }
}

/// A note being dragged, which only a notebook in the sidebar takes: application/x-note
#[derive(Clone, glib::Boxed)]
#[boxed_type(name = "AmethystoraNoteDrag")]
pub struct NoteDrag(pub String);

pub fn nav_item(item: NavItem) -> gtk::Widget {
    let content = ui::hbox(10, &[]);
    if let Some(toggle) = &item.toggle {
        content.append(toggle);
    }
    match &item.color {
        Some(color) => content.append(&widgets::dot(color, &[])),
        None => content.append(&icon(item.glyph, 18)),
    }
    let label = widgets::cut(&widgets::in_page_direction(&item.label), &["grow", "label"]);
    label.set_hexpand(true);
    content.append(&label);
    if let Some(count) = item.count {
        content.append(&ui::line(&i18n::number(count as f64), &["count"]));
    }
    let button = gtk::Button::builder().child(&content).css_classes(["plain", "nav-item"]).hexpand(true).build();
    if item.current {
        button.add_css_class("current");
    }
    button.update_state(&[gtk::accessible::State::Selected(Some(item.current))]);
    if item.depth > 0 {
        button.add_css_class(&format!("depth-{}", item.depth.min(5)));
    }
    if item.toggle.is_some() {
        button.add_css_class("twisted");
    }
    let click = item.on_click.clone();
    button.connect_clicked(move |_| click());
    let row = gtk::Overlay::new();
    row.add_css_class("nav-row");
    row.set_child(Some(&button));
    if let Some(on_menu) = item.on_menu.clone() {
        let more = icon_button("more", &t!("More"), &["nav-more"], move |button| on_menu(button));
        let more_icon = more.child();
        if let Some(mark) = more_icon.and_then(|c| c.downcast::<amethystora_ui::icon::Icon>().ok()) {
            mark.set_size(16);
        }
        more.update_property(&[gtk::accessible::Property::Label(&t!("More for {name}", name = item.label.clone()))]);
        more.set_halign(gtk::Align::End);
        more.set_valign(gtk::Align::Center);
        row.add_overlay(&more);
    }
    if let Some(drop) = item.drop.clone() {
        let target = gtk::DropTarget::new(NoteDrag::static_type(), gdk::DragAction::MOVE);
        let (enter_row, leave_row, drop_row) = (row.clone(), row.clone(), row.clone());
        target.connect_enter(move |_, _, _| {
            enter_row.add_css_class("drop-target");
            gdk::DragAction::MOVE
        });
        target.connect_leave(move |_| leave_row.remove_css_class("drop-target"));
        target.connect_drop(move |_, value, _, _| {
            drop_row.remove_css_class("drop-target");
            match value.get::<NoteDrag>() {
                Ok(NoteDrag(id)) if !id.is_empty() => {
                    drop(id);
                    true
                }
                _ => false,
            }
        });
        row.add_controller(target);
    }
    row.upcast()
}

pub fn nav_heading(label: &str, add: Option<(String, Rc<dyn Fn()>)>) -> gtk::Widget {
    let heading = ui::hbox(6, &["nav-heading"]);
    // In capitals the language's own way: GTK does not measure the stylesheet's text-transform
    let words = ui::line(&label.to_uppercase(), &["grow"]);
    words.set_hexpand(true);
    heading.append(&words);
    if let Some((label, add)) = add {
        let button = icon_button("plus", &label, &["nav-add"], move |_| add());
        if let Some(mark) = button.child().and_then(|c| c.downcast::<amethystora_ui::icon::Icon>().ok()) {
            mark.set_size(16);
        }
        heading.append(&button);
    }
    heading.upcast()
}

impl Notes {
    pub(super) fn build_sidebar(&self, sidebar: &gtk::Box) {
        // The wordmark of the boot splash and the login screen, the glowing stone as its "a"
        let mark = brand::brand(&t!("Notes"), 21.0);
        mark.update_property(&[gtk::accessible::Property::Label(&t!("Amethystora Notes"))]);
        mark.set_halign(gtk::Align::Start);
        sidebar.append(&mark);

        let modes = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(4).homogeneous(true).css_classes(["modes"]).build();
        modes.update_property(&[gtk::accessible::Property::Label(&t!("Notes or tasks"))]);
        for (at, (mode, glyph, label)) in [(Mode::Notes, "notes", t!("Notes")), (Mode::Tasks, "done", t!("Tasks"))].into_iter().enumerate() {
            let content = ui::hbox(8, &[]);
            content.set_halign(gtk::Align::Center);
            content.append(&icon(glyph, 17));
            content.append(&ui::line(&label, &[]));
            if mode == Mode::Tasks {
                self.today_count.set_visible(false);
                content.append(&self.today_count);
            }
            let button = &self.mode_buttons[at];
            button.set_child(Some(&content));
            button.update_property(&[gtk::accessible::Property::Label(&label)]);
            button.connect_clicked(self.on_button(move |this| this.set_mode(mode)));
            modes.append(button);
        }
        sidebar.append(&modes);

        let search_box = ui::hbox(8, &["search"]);
        search_box.append(&icon("search", 17));
        self.search.set_placeholder_text(Some(&t!("Search")));
        search_box.append(&self.search);
        sidebar.append(&search_box);
        let weak = self.me.clone();
        self.search.connect_changed(move |search| {
            let Some(this) = weak.upgrade() else { return };
            if search.text().trim().is_empty() {
                this.search_pause.cancel();
                this.searched();
            } else {
                let again = this.clone();
                this.search_pause.after_typing(move || again.searched());
            }
        });
        let keys = gtk::EventControllerKey::new();
        let weak = self.me.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(this) = weak.upgrade() else { return glib::Propagation::Proceed };
            match key {
                gdk::Key::Escape if !this.search.text().is_empty() => {
                    this.search.set_text("");
                    glib::Propagation::Stop
                }
                gdk::Key::Down => {
                    this.flush_search();
                    this.focus_first_item();
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        self.search.add_controller(keys);
        let weak = self.me.clone();
        self.search.connect_activate(move |_| {
            if let Some(this) = weak.upgrade() {
                this.flush_search();
                this.focus_first_item();
            }
        });

        self.nav.update_property(&[gtk::accessible::Property::Label(&t!("Sections"))]);
        sidebar.append(&self.nav_scroll);

        let foot = ui::vbox(2, &["sidebar-foot"]);
        for (button, glyph, label) in [(&self.settings_button, "settings", t!("Settings")), (&self.lock_button, "lock", t!("Lock"))] {
            let content = ui::hbox(10, &[]);
            content.append(&icon(glyph, 18));
            content.append(&ui::line(&label, &[]));
            button.set_child(Some(&content));
            foot.append(button);
        }
        self.settings_button.connect_clicked(self.on_button(|this| this.set_mode(Mode::Settings)));
        self.lock_button.connect_clicked(self.on_button(|this| this.lock_now()));
        self.lock_button.set_visible(false);
        let manual = widgets::link_button(&t!("Notes in the manual"), Some("book"), &["quiet"], || process::manual("notes"));
        if let Some(mark) = manual.child().and_then(|c| c.first_child()).and_then(|c| c.downcast::<amethystora_ui::icon::Icon>().ok()) {
            mark.set_size(18);
        }
        foot.append(&manual);
        sidebar.append(&foot);
    }

    pub(super) fn on_button(&self, f: impl Fn(&Rc<Notes>) + 'static) -> impl Fn(&gtk::Button) + 'static {
        let act = self.on(f);
        move |_| act()
    }

    /// The search changed: the notes or tasks that have every word
    /// A key that acts on the results acts on what has been typed
    fn flush_search(&self) {
        if self.search_pause.cancel() {
            self.searched();
        }
    }

    fn searched(&self) {
        self.commit();
        let query = self.search.text().trim().to_string();
        if self.ui.borrow().query == query {
            return;
        }
        {
            let mut ui = self.ui.borrow_mut();
            ui.query = query;
            if ui.mode == Mode::Settings {
                ui.mode = Mode::Notes;
            }
        }
        let mode = self.ui.borrow().mode;
        if mode == Mode::Tasks {
            self.ui.borrow_mut().task = None;
            self.render_tasks(false);
        } else {
            let view = self.ui.borrow().notes_view.clone();
            let list = self.sorted_notes(&view);
            let keep = self.ui.borrow().note.as_ref().is_some_and(|id| list.contains(id));
            if !keep {
                self.ui.borrow_mut().note = list.first().cloned();
            }
            self.render_notes(false);
        }
        self.render_nav();
    }

    /// Whether the view is the one shown, as sameView()
    pub(super) fn same_view(&self, a: &View, b: &View) -> bool {
        let ui = self.ui.borrow();
        ui.query.is_empty() && ui.mode != Mode::Settings && a.kind == b.kind && a.id == b.id
    }

    pub fn render_nav(&self) {
        let scroll = self.nav_scroll.vadjustment().value();
        ui::clear(&self.nav);
        let mode = self.ui.borrow().mode;
        let items = if mode == Mode::Tasks { self.tasks_nav() } else { self.notes_nav() };
        for item in items {
            self.nav.append(&item);
        }
        let adjustment = self.nav_scroll.vadjustment();
        glib::idle_add_local_once(move || adjustment.set_value(scroll));
        for (at, button) in self.mode_buttons.iter().enumerate() {
            let current = (at == 0 && mode == Mode::Notes) || (at == 1 && mode == Mode::Tasks);
            widgets::set_class(button, "current", current);
            button.update_state(&[gtk::accessible::State::Selected(Some(current))]);
        }
        let today = crate::dates::today();
        let due = self.db(|store| store.values(Kind::Tasks).filter(|t| !t.is("done") && t.opt("due").is_some_and(|d| d <= today.as_str())).count()).unwrap_or(0);
        self.today_count.set_visible(due > 0);
        self.today_count.set_label(&i18n::number(due as f64));
    }

    // --- Notes -------------------------------------------------------------------------------------------

    pub fn show_notes(&self, view: View) {
        self.commit();
        {
            let mut ui = self.ui.borrow_mut();
            ui.mode = Mode::Notes;
            ui.notes_view = view.clone();
        }
        self.clear_query();
        let list = self.sorted_notes(&view);
        let keep = self.ui.borrow().note.as_ref().is_some_and(|id| list.contains(id));
        if !keep {
            self.ui.borrow_mut().note = list.first().cloned();
        }
        self.remember();
        self.render();
    }

    /// The notebooks under a notebook, or at the top, by their names
    pub fn child_notebooks(&self, parent: Option<&str>) -> Vec<Value> {
        self.db(|store| {
            let mut list: Vec<Value> = store
                .values(Kind::Notebooks)
                .filter(|notebook| {
                    let own = notebook.opt("parent").filter(|p| store.has(Kind::Notebooks, p));
                    own == parent
                })
                .cloned()
                .collect();
            sort_by_title(&mut list);
            list
        })
        .unwrap_or_default()
    }

    fn notes_nav(&self) -> Vec<gtk::Widget> {
        let live: Vec<Value> = self.db(|store| store.values(Kind::Notes).filter(|n| !n.is("trashed")).cloned().collect()).unwrap_or_default();
        let view = self.ui.borrow().notes_view.clone();
        let mut items = Vec::new();
        let mut all = NavItem::new(&t!("All notes"), "notes", self.on(|this| this.show_notes(View::of("all"))));
        all.count = Some(live.len()).filter(|n| *n > 0);
        all.current = self.same_view(&view, &View::of("all"));
        items.push(nav_item(all));
        items.push(nav_heading(&t!("Notebooks"), Some((t!("New notebook"), Rc::new(self.on(|this| this.new_notebook(None)))))));
        let mut seen = std::collections::HashSet::new();
        self.walk_notebooks(None, 0, &mut seen, &live, &view, &mut items);
        let tags: Vec<Value> = self.db(|store| {
            let mut tags: Vec<Value> = store.values(Kind::Tags).cloned().collect();
            sort_by_title(&mut tags);
            tags
        }).unwrap_or_default();
        if !tags.is_empty() {
            items.push(nav_heading(&t!("Tags"), None));
            for tag in tags {
                let id = tag.id().to_string();
                let count = live.iter().filter(|n| n.list("tags").contains(&id)).count();
                let (show, menu) = (id.clone(), tag.clone());
                let mut item = NavItem::new(tag.title(), "hash", self.on(move |this| this.show_notes(View::with("tag", &show))));
                item.count = Some(count).filter(|n| *n > 0);
                item.current = self.same_view(&view, &View::with("tag", &id));
                let weak = self.me.clone();
                item.on_menu = Some(Rc::new(move |anchor| {
                    if let Some(this) = weak.upgrade() {
                        this.tag_menu(anchor, &menu);
                    }
                }));
                items.push(nav_item(item));
            }
        }
        let trashed = self.db(|store| store.values(Kind::Notes).filter(|n| n.is("trashed")).count()).unwrap_or(0);
        items.push(ui::hbox(0, &["nav-gap"]).upcast());
        let mut trash = NavItem::new(&t!("Trash"), "trash", self.on(|this| this.show_notes(View::of("trash"))));
        trash.count = Some(trashed).filter(|n| *n > 0);
        trash.current = self.same_view(&view, &View::of("trash"));
        items.push(nav_item(trash));
        items
    }

    fn walk_notebooks(&self, parent: Option<&str>, depth: usize, seen: &mut std::collections::HashSet<String>, live: &[Value], view: &View, items: &mut Vec<gtk::Widget>) {
        for notebook in self.child_notebooks(parent) {
            let id = notebook.id().to_string();
            if !seen.insert(id.clone()) {
                continue;
            }
            let children = self.child_notebooks(Some(&id));
            let collapsed = self.ui.borrow().collapsed.contains(&id);
            let toggle: gtk::Widget = if children.is_empty() {
                ui::hbox(0, &["twisty"]).upcast()
            } else {
                let mark = if collapsed { icon("chevron", 14).mirror_in_rtl() } else { crate::glyphs::chevron_down(14) };
                let twisty = gtk::Button::builder().child(&mark).css_classes(["plain", "twisty"]).valign(gtk::Align::Center).build();
                twisty.update_property(&[gtk::accessible::Property::Label(&if collapsed { t!("Expand") } else { t!("Collapse") })]);
                let toggled = id.clone();
                twisty.connect_clicked(self.on_button(move |this| {
                    {
                        let mut ui = this.ui.borrow_mut();
                        if ui.collapsed.contains(&toggled) {
                            ui.collapsed.retain(|c| *c != toggled);
                        } else {
                            ui.collapsed.push(toggled.clone());
                        }
                    }
                    this.remember();
                    this.render_nav();
                }));
                twisty.upcast()
            };
            let count = live.iter().filter(|n| n.str("notebook") == id).count();
            let title = if notebook.title().is_empty() { t!("Untitled") } else { notebook.title().to_string() };
            let (show, menu, target) = (id.clone(), notebook.clone(), id.clone());
            let mut item = NavItem::new(&title, "folder", self.on(move |this| this.show_notes(View::with("notebook", &show))));
            item.depth = depth;
            item.toggle = Some(toggle);
            item.count = Some(count).filter(|n| *n > 0);
            item.current = self.same_view(view, &View::with("notebook", &id));
            let weak = self.me.clone();
            item.on_menu = Some(Rc::new(move |anchor| {
                if let Some(this) = weak.upgrade() {
                    this.notebook_menu(anchor, &menu);
                }
            }));
            let weak = self.me.clone();
            item.drop = Some(Rc::new(move |note| {
                if let Some(this) = weak.upgrade() {
                    this.move_note(&note, &target);
                }
            }));
            items.push(nav_item(item));
            if !collapsed {
                self.walk_notebooks(Some(&id), depth + 1, seen, live, view, items);
            }
        }
    }

    // --- Tasks -------------------------------------------------------------------------------------------

    pub fn show_tasks(&self, view: View) {
        self.commit();
        {
            let mut ui = self.ui.borrow_mut();
            ui.mode = Mode::Tasks;
            ui.tasks_view = view;
            ui.task = None;
        }
        self.clear_query();
        self.remember();
        self.render();
    }

    fn tasks_nav(&self) -> Vec<gtk::Widget> {
        let open: Vec<Value> = self.open_tasks();
        let now = crate::dates::today();
        let view = self.ui.borrow().tasks_view.clone();
        let count = |n: usize| Some(n).filter(|n| *n > 0);
        let mut items = Vec::new();
        let views: [(&str, String, &'static str, Option<usize>); 5] = [
            ("inbox", t!("Inbox"), "inbox", count(open.iter().filter(|t| !t.is("project") && !t.is("parent")).count())),
            ("today", t!("Today"), "today", count(open.iter().filter(|t| t.opt("due").is_some_and(|d| d <= now.as_str())).count())),
            ("upcoming", t!("Upcoming"), "calendar", None),
            ("pinned", t!("Pinned"), "pin", count(open.iter().filter(|t| t.is("pinned")).count())),
            ("completed", t!("Completed"), "done", None),
        ];
        for (kind, label, glyph, n) in views {
            let mut item = NavItem::new(&label, glyph, self.on(move |this| this.show_tasks(View::of(kind))));
            item.count = n;
            item.current = self.same_view(&view, &View::of(kind));
            items.push(nav_item(item));
        }
        items.push(nav_heading(&t!("Projects"), Some((t!("New project"), Rc::new(self.on(|this| this.new_project()))))));
        let projects: Vec<Value> = self.db(|store| {
            let mut list: Vec<Value> = store.values(Kind::Projects).cloned().collect();
            with_compare(|compare| {
                list.sort_by(|a, b| {
                    a.num_or_zero("order")
                        .partial_cmp(&b.num_or_zero("order"))
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| compare(&crate::data::text(a.get("title")), &crate::data::text(b.get("title"))))
                })
            });
            list
        }).unwrap_or_default();
        for project in projects {
            let id = project.id().to_string();
            let title = if project.title().is_empty() { t!("Untitled") } else { project.title().to_string() };
            let (show, menu) = (id.clone(), project.clone());
            let mut item = NavItem::new(&title, "folder", self.on(move |this| this.show_tasks(View::with("project", &show))));
            item.color = Some(project.opt("color").unwrap_or("violet").to_string());
            item.count = count(open.iter().filter(|t| t.str("project") == id && !t.is("parent")).count());
            item.current = self.same_view(&view, &View::with("project", &id));
            let weak = self.me.clone();
            item.on_menu = Some(Rc::new(move |anchor| {
                if let Some(this) = weak.upgrade() {
                    this.project_menu(anchor, &menu);
                }
            }));
            items.push(nav_item(item));
        }
        items.push(nav_heading(&t!("Labels"), Some((t!("New label"), Rc::new(self.on(|this| this.new_label()))))));
        let labels: Vec<Value> = self.db(|store| {
            let mut list: Vec<Value> = store.values(Kind::Labels).cloned().collect();
            sort_by_title(&mut list);
            list
        }).unwrap_or_default();
        for label in labels {
            let id = label.id().to_string();
            let (show, menu) = (id.clone(), label.clone());
            let mut item = NavItem::new(label.title(), "tag", self.on(move |this| this.show_tasks(View::with("label", &show))));
            item.count = count(open.iter().filter(|t| t.list("labels").contains(&id)).count());
            item.current = self.same_view(&view, &View::with("label", &id));
            let weak = self.me.clone();
            item.on_menu = Some(Rc::new(move |anchor| {
                if let Some(this) = weak.upgrade() {
                    this.label_menu(anchor, &menu);
                }
            }));
            items.push(nav_item(item));
        }
        items
    }
}

/// localeCompare(b, undefined, { sensitivity: 'base', numeric: true }) for `f`: case and accents aside,
/// numbers by their value
pub fn with_compare<R>(f: impl FnOnce(&dyn Fn(&str, &str) -> std::cmp::Ordering) -> R) -> R {
    i18n::with(|i18n| {
        let collator = i18n.collator();
        let compare = |a: &str, b: &str| match &collator {
            Some(collator) => collator.compare(a, b),
            None => a.to_lowercase().cmp(&b.to_lowercase()),
        };
        f(&compare)
    })
}

/// byTitle()
pub fn sort_by_title(list: &mut [Value]) {
    with_compare(|compare| list.sort_by(|a, b| compare(&crate::data::text(a.get("title")), &crate::data::text(b.get("title")))));
}
