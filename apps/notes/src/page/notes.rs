//! The notes, as notes.js drew them: the list of the view shown (a gtk::ListView, which only ever makes the
//! rows in sight, so that hundreds of notes stay quick), and the open note's editor: its notebook, the
//! layout, its title, tags and formatting, the Markdown and its preview, and its history.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use regex::Regex;
use serde_json::{json, Map, Value};

use amethystora_ui::markdown::{self, Block};
use amethystora_ui::{gdk, gio, glib, gtk, pango, process, t, ui};
use gtk::prelude::*;

use super::sidebar::{sort_by_title, NoteDrag};
use super::widgets::{self, button, icon_button, item, link_button, menu, toast, Action, Dialog, Entry};
use super::{Mode, Notes, View};
use crate::backend;
use crate::data::{self, Fields, Kind};
use crate::dates;
use crate::icons::icon;
use crate::layout::{Cap, Exact, Flex, Grow};

/// The panes of the notes, kept across renders so that typing is never interrupted
pub struct Parts {
    pub layout: gtk::Box,
    head: gtk::Box,
    model: gtk::StringList,
    list: gtk::ListView,
    scroller: gtk::ScrolledWindow,
    stack: gtk::Stack,
    empty: gtk::Box,
    editor_pane: gtk::Box,
    refresh: RefCell<Option<glib::SourceId>>,
    /// The ids the list shows, for the rows it binds
    ids: Rc<RefCell<Vec<String>>>,
}

/// The open note's editor
pub struct Editor {
    pub id: String,
    trashed: bool,
    title: gtk::Entry,
    pub buffer: gtk::TextBuffer,
    pub text: gtk::TextView,
    body: gtk::Box,
    source: gtk::Widget,
    preview_scroll: gtk::ScrolledWindow,
    preview: gtk::Box,
    preview_cap: Cap,
    toolbar: Flex,
    segment: Vec<(String, gtk::Button)>,
    foot: gtk::Label,
    tag_row: Flex,
    tag_input: gtk::Entry,
    placeholder: gtk::Label,
    /// Set while the page itself sets the text, which is not a change typed
    quiet: Rc<Cell<bool>>,
    preview_timer: RefCell<Option<glib::SourceId>>,
}

/// The margins of the preview's blocks, as notes.css set them on its 15 px text (markdown.rs collapses them)
fn margins(block: Block) -> (i32, i32) {
    match block {
        Block::H1 => (33, 11),
        Block::H2 => (27, 9),
        Block::H3 => (23, 8),
        Block::H4 => (20, 7),
        Block::Lead | Block::P | Block::List | Block::Code | Block::Quote | Block::Table => (0, 13),
        // A picture is in a paragraph and on its baseline: under it the rest of the line, 7px of 25.5
        Block::Image => (0, 20),
        Block::Item => (2, 0),
        Block::Hr => (24, 24),
    }
}

/// A colour as Pango markup takes it
fn hex(color: &gdk::RGBA) -> String {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}{:02x}", byte(color.red()), byte(color.green()), byte(color.blue()), byte(color.alpha()))
}

/// The first line of text, without the Markdown around it
pub fn snippet(body: &str) -> String {
    thread_local! {
        static PATTERNS: [Regex; 5] = [
            Regex::new(r"(?s)```.*?```").expect("a pattern"),
            Regex::new(r"!\[[^\]]*\]\([^)]*\)").expect("a pattern"),
            Regex::new(r"\[([^\]]*)\]\([^)]*\)").expect("a pattern"),
            Regex::new(r"(?m)^\s*(#{1,6}|>|[-*+]|[0-9]+[.)])\s+(\[[ xX]\]\s+)?").expect("a pattern"),
            Regex::new(r"[*_`~]").expect("a pattern"),
        ];
    }
    let text = PATTERNS.with(|p| {
        let text = p[0].replace_all(body, " ");
        let text = p[1].replace_all(&text, " ");
        let text = p[2].replace_all(&text, "$1");
        let text = p[3].replace_all(&text, "");
        p[4].replace_all(&text, "").into_owned()
    });
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    data::js_slice(&collapsed, 140)
}

/// The words in a note: runs of letters and digits
pub fn words(text: &str) -> usize {
    let mut count = 0;
    let mut inside = false;
    for c in text.chars() {
        let word = c.is_alphanumeric();
        if word && !inside {
            count += 1;
        }
        inside = word;
    }
    count
}

/// The nth task list item's box, flipped in the Markdown itself; fenced code does not count
pub fn toggle_checkbox(body: &str, index: usize) -> String {
    thread_local! {
        static ITEM: Regex = Regex::new(r"^(\s*(?:>\s*)*(?:[-*+]|[0-9]+[.)])\s+\[)([ xX])\]").expect("a pattern");
    }
    let mut lines: Vec<String> = body.split('\n').map(str::to_string).collect();
    let mut count: i64 = -1;
    let mut fenced = false;
    for at in 0..lines.len() {
        let trimmed = lines[at].trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let found = ITEM.with(|item| item.captures(&lines[at]).map(|c| (c[1].to_string(), c[2].to_string())));
        if let Some((head, mark)) = found {
            count += 1;
            if count as usize == index {
                let rest = lines[at][head.len() + 1..].to_string();
                lines[at] = format!("{head}{}{rest}", if mark == " " { "x" } else { " " });
                return lines.join("\n");
            }
        }
    }
    body.to_string()
}

/// Markdown's special characters escaped, for a version's title made a heading
fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\`*_{}[]()#+-.!<>|".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

impl Notes {
    // --- What is shown -----------------------------------------------------------------------------------

    fn live_notes(&self) -> Vec<Value> {
        self.db(|store| store.values(Kind::Notes).filter(|n| !n.is("trashed")).cloned().collect()).unwrap_or_default()
    }

    /// A notebook and every notebook inside it
    fn descendants(&self, id: &str) -> HashSet<String> {
        let mut found: HashSet<String> = HashSet::from([id.to_string()]);
        self.db(|store| {
            let mut grew = true;
            while grew {
                grew = false;
                for notebook in store.values(Kind::Notebooks) {
                    if notebook.opt("parent").is_some_and(|p| found.contains(p)) && !found.contains(notebook.id()) {
                        found.insert(notebook.id().to_string());
                        grew = true;
                    }
                }
            }
        });
        found
    }

    fn terms(&self) -> Vec<String> {
        self.ui.borrow().query.to_lowercase().split_whitespace().map(str::to_string).collect()
    }

    fn notes_in(&self, view: &View) -> Vec<Value> {
        if !self.ui.borrow().query.is_empty() {
            let words = self.terms();
            return self
                .db(|store| {
                    store
                        .values(Kind::Notes)
                        .filter(|n| !n.is("trashed"))
                        .filter(|note| {
                            let tags: Vec<String> = note.list("tags").iter().map(|id| store.get(Kind::Tags, id).map(|t| data::text(t.get("title"))).unwrap_or_default()).collect();
                            let text = format!("{}\n{}\n{}", data::text(note.get("title")), data::text(note.get("body")), tags.join(" ")).to_lowercase();
                            words.iter().all(|word| text.contains(word.as_str()))
                        })
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
        }
        match view.kind.as_str() {
            "trash" => self.db(|store| store.values(Kind::Notes).filter(|n| n.is("trashed")).cloned().collect()).unwrap_or_default(),
            "notebook" => {
                let inside = self.descendants(view.id.as_deref().unwrap_or(""));
                self.live_notes().into_iter().filter(|n| inside.contains(n.str("notebook"))).collect()
            }
            "tag" => {
                let id = view.id.clone().unwrap_or_default();
                self.live_notes().into_iter().filter(|n| n.list("tags").contains(&id)).collect()
            }
            _ => self.live_notes(),
        }
    }

    fn sort_notes(&self, list: &mut [Value]) {
        let order = self.setting("noteSort").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "updated".into());
        if order == "title" {
            sort_by_title(list);
            return;
        }
        let trash = self.ui.borrow().notes_view.kind == "trash" && self.ui.borrow().query.is_empty();
        let field = if trash { "trashed".to_string() } else { order };
        list.sort_by(|a, b| b.num_or_zero(&field).partial_cmp(&a.num_or_zero(&field)).unwrap_or(std::cmp::Ordering::Equal));
    }

    /// sortNotes(notesIn(view)), by id
    pub fn sorted_notes(&self, view: &View) -> Vec<String> {
        let mut list = self.notes_in(view);
        self.sort_notes(&mut list);
        list.iter().map(|note| note.id().to_string()).collect()
    }

    fn view_title(&self) -> String {
        if !self.ui.borrow().query.is_empty() {
            return t!("Search");
        }
        let view = self.ui.borrow().notes_view.clone();
        match view.kind.as_str() {
            "notebook" => self.item(Kind::Notebooks, view.id.as_deref().unwrap_or("")).map(|n| n.title().to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| t!("Untitled")),
            "tag" => format!("#{}", self.item(Kind::Tags, view.id.as_deref().unwrap_or("")).map(|t| t.title().to_string()).unwrap_or_default()),
            "trash" => t!("Trash"),
            _ => t!("All notes"),
        }
    }

    // --- The panes ---------------------------------------------------------------------------------------

    fn notes_parts(&self) -> Parts {
        let layout = ui::hbox(0, &["notes-layout"]);
        let list_pane = ui::vbox(0, &["list-pane"]);
        list_pane.update_property(&[gtk::accessible::Property::Label(&t!("Notes"))]);
        let head = ui::hbox(6, &["list-head"]);
        list_pane.append(&head);
        let model = gtk::StringList::new(&[]);
        let ids: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            let row = ui::vbox(3, &["note-item"]);
            // Dragged onto a notebook in the sidebar, the note moves there
            let drag = gtk::DragSource::new();
            drag.set_actions(gdk::DragAction::MOVE);
            let source = row.clone();
            drag.connect_prepare(move |_, _, _| {
                let id = source.widget_name().to_string();
                (data::is_id(&id) && !source.has_css_class("trashed")).then(|| gdk::ContentProvider::for_value(&NoteDrag(id).to_value()))
            });
            row.add_controller(drag);
            item.set_child(Some(&row));
        });
        let weak = self.me.clone();
        factory.connect_bind(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            let (Some(this), Some(row)) = (weak.upgrade(), item.child().and_then(|c| c.downcast::<gtk::Box>().ok())) else { return };
            let id = item.item().and_then(|o| o.downcast::<gtk::StringObject>().ok()).map(|s| s.string().to_string()).unwrap_or_default();
            this.bind_note_row(&row, &id);
        });
        let selection = gtk::NoSelection::new(Some(model.clone()));
        let list = gtk::ListView::builder().model(&selection).factory(&factory).single_click_activate(true).css_classes(["notes-list"]).build();
        let weak = self.me.clone();
        let positions = ids.clone();
        list.connect_activate(move |_, position| {
            let id = positions.borrow().get(position as usize).cloned();
            if let (Some(this), Some(id)) = (weak.upgrade(), id) {
                this.open_note(&id);
            }
        });
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&list).build();
        let empty = ui::vbox(8, &["empty"]);
        empty.set_valign(gtk::Align::Start);
        let stack = gtk::Stack::builder().vexpand(true).vhomogeneous(false).build();
        stack.add_named(&scroller, Some("list"));
        stack.add_named(&empty, Some("empty"));
        list_pane.append(&stack);
        let column = Exact::new(&list_pane);
        column.set_hexpand(false);
        layout.append(&column);
        let editor_pane = ui::vbox(0, &["editor-pane"]);
        editor_pane.update_property(&[gtk::accessible::Property::Label(&t!("Note"))]);
        editor_pane.set_hexpand(true);
        layout.append(&editor_pane);
        Parts { layout, head, model, list, scroller, stack, empty, editor_pane, refresh: RefCell::new(None), ids }
    }

    pub fn render_notes(&self, force: bool) {
        let mut force = force;
        if self.notes_parts.borrow().is_none() {
            self.notes_parts.replace(Some(self.notes_parts()));
        }
        let layout = self.notes_parts.borrow().as_ref().map(|p| p.layout.clone()).expect("made");
        if self.main.first_child().as_ref() != Some(layout.upcast_ref::<gtk::Widget>()) {
            ui::clear(&self.main);
            self.main.append(&layout);
            force = true;
        }
        let view = self.ui.borrow().notes_view.clone();
        let list = self.sorted_notes(&view);
        let note = self.ui.borrow().note.clone().filter(|id| self.db(|s| s.has(Kind::Notes, id)).unwrap_or(false));
        if note.is_none() {
            self.ui.borrow_mut().note = None;
        }
        self.render_note_list(&list);
        let current = self.editor.borrow().as_ref().map(|e| e.id.clone());
        if force || current != note {
            let pane = self.notes_parts.borrow().as_ref().map(|p| p.editor_pane.clone()).expect("made");
            ui::clear(&pane);
            match note {
                Some(id) => {
                    let editor = self.build_editor(&id);
                    pane.append(&editor);
                }
                None => {
                    self.editor.replace(None);
                    pane.append(&self.empty_editor());
                    self.controls_for(None);
                }
            }
        }
    }

    fn render_note_list(&self, list: &[String]) {
        let parts = self.notes_parts.borrow();
        let Some(parts) = parts.as_ref() else { return };
        let trash = self.ui.borrow().notes_view.kind == "trash" && self.ui.borrow().query.is_empty();
        let sort = self.setting("noteSort").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "updated".into());
        ui::clear(&parts.head);
        let words = ui::vbox(0, &["grow"]);
        words.set_hexpand(true);
        let title = widgets::cut(&self.view_title(), &["h2"]);
        title.set_hexpand(true);
        words.append(&title);
        words.append(&ui::line(&t!("{count, plural, one {# note} other {# notes}}", count = list.len()), &["faint", "small"]));
        parts.head.append(&words);
        let weak = self.me.clone();
        let current = sort.clone();
        parts.head.append(&icon_button("sort", &t!("Sort"), &[], move |anchor| {
            let Some(this) = weak.upgrade() else { return };
            let entries = [("updated", t!("Last changed")), ("created", t!("Created")), ("title", t!("Title"))]
                .into_iter()
                .map(|(value, label)| {
                    let weak = this.me.clone();
                    Entry::Item(widgets::Item {
                        label,
                        glyph: None,
                        color: None,
                        checked: current == value,
                        danger: false,
                        run: Rc::new(move || {
                            if let Some(this) = weak.upgrade() {
                                this.set_setting("noteSort", Value::from(value));
                                this.render_notes(false);
                            }
                        }),
                    })
                })
                .collect();
            menu(anchor, entries);
        }));
        if trash {
            if !list.is_empty() {
                let ids = list.to_vec();
                let empty = button(&t!("Empty"), Some("trash"), &["small"], self.on(move |this| this.delete_forever(ids.clone())));
                empty.set_valign(gtk::Align::Center);
                parts.head.append(&empty);
            }
        } else {
            let label = t!("{action} ({key})", action = t!("New note"), key = "Ctrl+N");
            parts.head.append(&icon_button("file-plus", &label, &["accent"], {
                let act = self.on(|this| this.new_note());
                move |_| act()
            }));
        }
        // The rows: only those in sight are made, as they come into sight
        let scroll = parts.scroller.vadjustment().value();
        // Every row drawn again from the note it shows: new items, which the list binds anew (the same
        // ones it would keep as they were drawn)
        parts.ids.replace(list.to_vec());
        let strings: Vec<&str> = list.iter().map(String::as_str).collect();
        parts.model.splice(0, parts.model.n_items(), &strings);
        let adjustment = parts.scroller.vadjustment();
        glib::idle_add_local_once(move || adjustment.set_value(scroll));
        ui::clear(&parts.empty);
        if list.is_empty() {
            let query = !self.ui.borrow().query.is_empty();
            let text = if query { t!("No note has all of those words.") } else if trash { t!("The trash is empty.") } else { t!("No notes here yet.") };
            parts.empty.append(&ui::label(&text, &["small"]));
            if !query && !trash {
                parts.empty.append(&link_button(&t!("Start one"), Some("file-plus"), &[], self.on(|this| this.new_note())));
            }
            parts.stack.set_visible_child_name("empty");
        } else {
            parts.stack.set_visible_child_name("list");
        }
    }

    /// One row of the list, for the note it now shows
    fn bind_note_row(&self, row: &gtk::Box, id: &str) {
        ui::clear(row);
        row.set_widget_name(id);
        let Some(note) = self.item(Kind::Notes, id) else { return };
        let (trash, sort, view_kind, query) = {
            let ui = self.ui.borrow();
            let sort = self.setting("noteSort").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "updated".into());
            (ui.notes_view.kind == "trash" && ui.query.is_empty(), sort, ui.notes_view.kind.clone(), !ui.query.is_empty())
        };
        widgets::set_class(row, "current", self.ui.borrow().note.as_deref() == Some(id));
        widgets::set_class(row, "trashed", trash);
        let title = if note.title().is_empty() { t!("Untitled") } else { note.title().to_string() };
        let title_label = widgets::cut(&widgets::in_page_direction(&title), &["note-title"]);
        row.append(&title_label);
        let text = snippet(&data::text(note.get("body")));
        let snippet_label = ui::label(&widgets::in_page_direction(&if text.is_empty() { t!("No text") } else { text.clone() }), &["note-snippet"]);
        snippet_label.set_lines(2);
        snippet_label.set_ellipsize(pango::EllipsizeMode::End);
        snippet_label.set_wrap_mode(pango::WrapMode::WordChar);
        row.append(&snippet_label);
        let meta = Flex::new(10, 4, &["note-meta"]);
        let when = if trash { note.num_or_zero("trashed") } else { note.num_or_zero(if sort == "created" { "created" } else { "updated" }) };
        meta.append(&ui::line(&dates::when_label(when), &[]));
        if view_kind != "notebook" || query {
            if let Some(place) = self.item(Kind::Notebooks, note.str("notebook")).map(|n| n.title().to_string()).filter(|t| !t.is_empty()) {
                let folder = ui::hbox(4, &["meta-folder"]);
                folder.append(&icon("folder", 13));
                folder.append(&ui::line(&widgets::in_page_direction(&place), &[]));
                meta.append(&folder);
            }
        }
        if !backend::resource_links(note.str("body")).is_empty() {
            meta.append(&icon("clip", 13));
        }
        let tags: Vec<String> = note.list("tags").iter().filter_map(|tag| self.item(Kind::Tags, tag)).map(|tag| tag.title().to_string()).take(2).collect();
        for tag in tags {
            let chip = gtk::Box::builder().css_classes(["chip"]).build();
            chip.append(&widgets::cut(&widgets::in_page_direction(&tag), &[]));
            let limited = ui::limit(&chip, 160);
            meta.append(&limited);
        }
        row.append(&meta);
    }

    pub fn focus_first_item(&self) {
        match self.ui.borrow().mode {
            Mode::Tasks => self.focus_first_task(),
            _ => {
                if let Some(parts) = self.notes_parts.borrow().as_ref() {
                    parts.list.grab_focus();
                }
            }
        }
    }

    fn empty_editor(&self) -> gtk::Widget {
        let empty = ui::vbox(14, &["editor-empty"]);
        empty.set_valign(gtk::Align::Center);
        empty.set_vexpand(true);
        empty.append(&icon("notes", 44));
        let text = if self.live_notes().is_empty() { t!("Your notes will be here.") } else { t!("Pick a note, or start a new one.") };
        let words = ui::label(&text, &[]);
        words.set_xalign(0.5);
        words.set_justify(gtk::Justification::Center);
        empty.append(&words);
        let start = button(&t!("New note"), Some("file-plus"), &["primary"], self.on(|this| this.new_note()));
        start.set_halign(gtk::Align::Center);
        empty.append(&start);
        let hint = ui::label(&t!("{key} starts a note from anywhere in the window.", key = "Ctrl+N"), &["faint", "small"]);
        hint.set_xalign(0.5);
        empty.append(&hint);
        empty.upcast()
    }

    // --- Notes, notebooks and tags ------------------------------------------------------------------------

    /// The first notebook, made when there is none
    fn first_notebook(&self) -> String {
        if let Some(found) = self.child_notebooks(None).first() {
            return found.id().to_string();
        }
        let id = data::new_id();
        let now = data::ms(data::now_ms());
        self.insert(Kind::Notebooks, json!({ "id": id, "title": t!("Notes"), "parent": null, "created": now, "updated": now }), true);
        id
    }

    pub fn new_note(&self) {
        self.commit();
        let leave = { let ui = self.ui.borrow(); !ui.query.is_empty() || ui.notes_view.kind == "trash" };
        if leave {
            self.ui.borrow_mut().notes_view = View::of("all");
            self.clear_query();
        }
        let view = self.ui.borrow().notes_view.clone();
        let notebook = match (&view.kind[..], &view.id) {
            ("notebook", Some(id)) if self.db(|s| s.has(Kind::Notebooks, id)).unwrap_or(false) => id.clone(),
            _ => self.first_notebook(),
        };
        let now = data::ms(data::now_ms());
        let tags = if view.kind == "tag" { json!([view.id]) } else { json!([]) };
        let id = data::new_id();
        self.insert(Kind::Notes, json!({ "id": id, "notebook": notebook, "title": "", "body": "", "tags": tags, "created": now, "updated": now, "trashed": null }), true);
        {
            let mut ui = self.ui.borrow_mut();
            ui.mode = Mode::Notes;
            ui.note = Some(id);
        }
        self.remember();
        self.render();
        if let Some(editor) = self.editor.borrow().as_ref() {
            let title = editor.title.clone();
            glib::idle_add_local_once(move || {
                title.grab_focus();
            });
        }
    }

    fn open_note(&self, id: &str) {
        self.commit();
        self.ui.borrow_mut().note = Some(id.to_string());
        self.remember();
        self.render_notes(false);
    }

    pub fn move_note(&self, id: &str, notebook: &str) {
        let Some(target) = self.item(Kind::Notebooks, notebook) else { return };
        if self.item(Kind::Notes, id).is_none() {
            return;
        }
        let mut changes = Map::new();
        changes.insert("notebook".into(), Value::from(notebook));
        changes.insert("trashed".into(), Value::Null);
        self.edit_note(id, changes);
        self.render_nav();
        self.render_notes(true);
        let name = if target.title().is_empty() { t!("Untitled") } else { target.title().to_string() };
        toast(&t!("Moved to {notebook}", notebook = name), None);
    }

    /// The note that takes this one's place in the list when it goes
    fn neighbour(&self, id: &str) -> Option<String> {
        let view = self.ui.borrow().notes_view.clone();
        let list = self.sorted_notes(&view);
        let at = list.iter().position(|other| other == id)?;
        list.get(at + 1).or(if at > 0 { list.get(at - 1) } else { None }).cloned()
    }

    fn trash_note(&self, id: &str) {
        let next = self.neighbour(id);
        let mut changes = Map::new();
        changes.insert("trashed".into(), data::ms(data::now_ms()));
        self.edit_note(id, changes);
        self.commit();
        self.ui.borrow_mut().note = next;
        self.render_nav();
        self.render_notes(true);
        let weak = self.me.clone();
        let undone = id.to_string();
        toast(&t!("Moved to the trash"), Some(Rc::new(move || {
            if let Some(this) = weak.upgrade() {
                let mut changes = Map::new();
                changes.insert("trashed".into(), Value::Null);
                this.edit_note(&undone, changes);
                this.ui.borrow_mut().note = Some(undone.clone());
                this.render();
            }
        })));
    }

    fn restore_note(&self, id: &str) {
        let Some(note) = self.item(Kind::Notes, id) else { return };
        let notebook = if self.db(|s| s.has(Kind::Notebooks, note.str("notebook"))).unwrap_or(false) { note.str("notebook").to_string() } else { self.first_notebook() };
        let mut changes = Map::new();
        changes.insert("trashed".into(), Value::Null);
        changes.insert("notebook".into(), Value::from(notebook.clone()));
        self.edit_note(id, changes);
        let next = self.neighbour(id);
        self.ui.borrow_mut().note = next;
        self.render();
        let name = self.item(Kind::Notebooks, &notebook).map(|n| n.title().to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| t!("Untitled"));
        toast(&t!("Restored to {notebook}", notebook = name), None);
    }

    fn delete_forever(&self, ids: Vec<String>) {
        self.spawn(async move |this| {
            let sure = widgets::confirm(
                &t!("{count, plural, =1 {Delete this note for good?} other {Delete # notes for good?}}", count = ids.len()),
                &t!("They cannot be brought back, and neither can their earlier versions or attachments."),
                &t!("Delete"),
                true,
            )
            .await;
            if !sure {
                return;
            }
            let revisions: Vec<String> = this.db(|store| store.values(Kind::Revisions).filter(|r| ids.iter().any(|id| id == r.str("note"))).map(|r| r.id().to_string()).collect()).unwrap_or_default();
            this.remove(Kind::Revisions, &revisions);
            this.remove(Kind::Notes, &ids);
            this.ui.borrow_mut().note = None;
            this.render();
        });
    }

    pub(super) fn new_notebook(&self, parent: Option<String>) {
        self.spawn(async move |this| {
            let title = if parent.is_some() { t!("New notebook inside") } else { t!("New notebook") };
            let Some(name) = widgets::ask(&title, &t!("Name"), "", Some(&t!("Create")), None).await else { return };
            let id = data::new_id();
            let now = data::ms(data::now_ms());
            this.insert(Kind::Notebooks, json!({ "id": id, "title": name, "parent": parent, "created": now, "updated": now }), true);
            if let Some(parent) = &parent {
                this.ui.borrow_mut().collapsed.retain(|c| c != parent);
            }
            this.show_notes(View::with("notebook", &id));
        });
    }

    pub(super) fn notebook_menu(&self, anchor: &gtk::Button, notebook: &Value) {
        let id = notebook.id().to_string();
        let mut entries = vec![
            item(&t!("New note here"), "file-plus", self.on({
                let id = id.clone();
                move |this| {
                    this.ui.borrow_mut().notes_view = View::with("notebook", &id);
                    this.new_note();
                }
            })),
            item(&t!("New notebook inside"), "folder", self.on({
                let id = id.clone();
                move |this| this.new_notebook(Some(id.clone()))
            })),
            item(&t!("Rename"), "edit", self.on({
                let id = id.clone();
                move |this| {
                    let id = id.clone();
                    this.spawn(async move |this| {
                        let current = this.item(Kind::Notebooks, &id).map(|n| n.title().to_string()).unwrap_or_default();
                        if let Some(title) = widgets::ask(&t!("Rename notebook"), &t!("Name"), &current, None, None).await {
                            let mut changes = Map::new();
                            changes.insert("title".into(), Value::from(title));
                            changes.insert("updated".into(), data::ms(data::now_ms()));
                            this.update(Kind::Notebooks, &id, &changes);
                            this.save(Kind::Notebooks, &id, false);
                            this.render();
                        }
                    });
                }
            })),
        ];
        if notebook.is("parent") {
            entries.push(item(&t!("Move to the top level"), "upload", self.on({
                let id = id.clone();
                move |this| {
                    let mut changes = Map::new();
                    changes.insert("parent".into(), Value::Null);
                    changes.insert("updated".into(), data::ms(data::now_ms()));
                    this.update(Kind::Notebooks, &id, &changes);
                    this.save(Kind::Notebooks, &id, false);
                    this.render();
                }
            })));
        }
        entries.push(Entry::Separator);
        entries.push(widgets::danger(&t!("Delete"), "trash", self.on(move |this| this.delete_notebook(&id))));
        menu(anchor, entries);
    }

    fn delete_notebook(&self, id: &str) {
        let id = id.to_string();
        self.spawn(async move |this| {
            let Some(notebook) = this.item(Kind::Notebooks, &id) else { return };
            let inside = this.descendants(&id);
            let notes: Vec<String> = this.live_notes().into_iter().filter(|n| inside.contains(n.str("notebook"))).map(|n| n.id().to_string()).collect();
            let title = if notebook.title().is_empty() { t!("Delete this notebook?") } else { t!("Delete {name}?", name = notebook.title()) };
            let text = if inside.len() > 1 {
                t!("{count, plural, =0 {It has no notes, and the notebooks inside it are deleted too.} one {# note in it goes to the trash, and the notebooks inside it are deleted too.} other {# notes in it go to the trash, and the notebooks inside it are deleted too.}}", count = notes.len())
            } else {
                t!("{count, plural, =0 {It has no notes.} one {# note in it goes to the trash.} other {# notes in it go to the trash.}}", count = notes.len())
            };
            if !widgets::confirm(&title, &text, &t!("Delete"), true).await {
                return;
            }
            for note in &notes {
                let mut changes = Map::new();
                changes.insert("trashed".into(), data::ms(data::now_ms()));
                this.edit_note(note, changes);
            }
            let inside: Vec<String> = inside.into_iter().collect();
            this.remove(Kind::Notebooks, &inside);
            let shown = this.ui.borrow().notes_view.id.clone();
            if shown.is_some_and(|shown| inside.contains(&shown)) {
                this.ui.borrow_mut().notes_view = View::of("all");
            }
            this.render();
        });
    }

    pub(super) fn tag_menu(&self, anchor: &gtk::Button, tag: &Value) {
        let id = tag.id().to_string();
        let rename = id.clone();
        menu(anchor, vec![
            item(&t!("Rename"), "edit", self.on(move |this| {
                let id = rename.clone();
                this.spawn(async move |this| {
                    let current = this.item(Kind::Tags, &id).map(|t| t.title().to_string()).unwrap_or_default();
                    if let Some(title) = widgets::ask(&t!("Rename tag"), &t!("Name"), &current, None, None).await {
                        let mut changes = Map::new();
                        changes.insert("title".into(), Value::from(title));
                        this.update(Kind::Tags, &id, &changes);
                        this.save(Kind::Tags, &id, false);
                        this.render();
                    }
                });
            })),
            widgets::danger(&t!("Delete"), "trash", self.on(move |this| {
                let id = id.clone();
                this.spawn(async move |this| {
                    let name = this.item(Kind::Tags, &id).map(|t| t.title().to_string()).unwrap_or_default();
                    if widgets::confirm(&t!("Delete the tag {tag}?", tag = name), &t!("The notes stay; only the tag is taken off them."), &t!("Delete"), true).await {
                        let tagged: Vec<(String, Vec<String>)> = this.db(|store| store.values(Kind::Notes).filter(|n| n.list("tags").contains(&id)).map(|n| (n.id().to_string(), n.list("tags"))).collect()).unwrap_or_default();
                        for (note, tags) in tagged {
                            let mut changes = Map::new();
                            changes.insert("tags".into(), json!(tags.into_iter().filter(|t| *t != id).collect::<Vec<_>>()));
                            this.update(Kind::Notes, &note, &changes);
                            this.save(Kind::Notes, &note, false);
                        }
                        this.remove(Kind::Tags, std::slice::from_ref(&id));
                        if this.ui.borrow().notes_view.id.as_deref() == Some(id.as_str()) {
                            this.ui.borrow_mut().notes_view = View::of("all");
                        }
                        this.render();
                    }
                });
            })),
        ]);
    }

    /// The tag of this name, made when there is none: names match case aside
    fn tag_for(&self, title: &str) -> String {
        let lower = title.to_lowercase();
        if let Some(found) = self.db(|store| store.values(Kind::Tags).find(|t| data::text(t.get("title")).to_lowercase() == lower).map(|t| t.id().to_string())).flatten() {
            return found;
        }
        let id = data::new_id();
        self.insert(Kind::Tags, json!({ "id": id, "title": title }), true);
        id
    }

    // --- The editor ----------------------------------------------------------------------------------------

    fn build_editor(&self, id: &str) -> gtk::Widget {
        let note = self.item(Kind::Notes, id).unwrap_or(Value::Null);
        let trashed = note.is("trashed");
        let layout = self.setting("layout").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "split".into());
        let root = ui::vbox(0, &["editor"]);
        root.set_vexpand(true);

        // The head: the notebook, the layout, the history and the rest
        let head = ui::hbox(6, &["editor-head"]);
        let crumb = ui::hbox(6, &["crumb"]);
        crumb.append(&icon("folder", 16));
        let mut notebooks: Vec<(String, String)> = Vec::new();
        self.notebook_choices(None, 0, &mut HashSet::new(), &mut notebooks);
        let weak = self.me.clone();
        let moving = id.to_string();
        let select = widgets::select(&t!("Notebook"), &notebooks, note.str("notebook"), &["crumb-select"], move |notebook| {
            if let Some(this) = weak.upgrade() {
                this.move_note(&moving, &notebook);
            }
        });
        select.set_sensitive(!trashed);
        crumb.append(&ui::limit(&select, 260));
        head.append(&crumb);
        let grow = ui::hbox(0, &[]);
        grow.set_hexpand(true);
        head.append(&grow);
        let mut segment_buttons = Vec::new();
        if !trashed {
            let segment = ui::hbox(2, &["segment"]);
            segment.update_property(&[gtk::accessible::Property::Label(&t!("Layout"))]);
            for (value, glyph, label) in [("edit", "edit", t!("Editor")), ("split", "split", t!("Editor and preview")), ("preview", "eye", t!("Preview"))] {
                let mark = icon(glyph, 17);
                mark.set_hexpand(true);
                let button = gtk::Button::builder().child(&mark).css_classes(["plain"]).tooltip_text(t!("{action} ({key})", action = label.clone(), key = "Ctrl+E")).build();
                button.update_property(&[gtk::accessible::Property::Label(&label)]);
                widgets::set_on(&button, value == layout);
                button.set_hexpand(false);
                button.connect_clicked(self.on_click(move |this| this.set_layout(value)));
                segment.append(&button);
                segment_buttons.push((value.to_string(), button));
            }
            head.append(&segment);
            let history_id = id.to_string();
            head.append(&icon_button("history", &t!("Earlier versions"), &[], self.on_clicked(move |this| this.show_history(&history_id))));
        }
        let more_id = id.to_string();
        head.append(&icon_button("more", &t!("More"), &[], self.on_anchor(move |this, anchor| {
            let id = more_id.clone();
            let trashed = this.item(Kind::Notes, &id).is_some_and(|n| n.is("trashed"));
            let entries = if trashed {
                vec![
                    item(&t!("Restore"), "undo", this.on({ let id = id.clone(); move |this| this.restore_note(&id) })),
                    widgets::danger(&t!("Delete for good"), "trash", this.on({ let id = id.clone(); move |this| this.delete_forever(vec![id.clone()]) })),
                ]
            } else {
                vec![
                    item(&t!("Export as PDF"), "download", this.on({ let id = id.clone(); move |this| this.export_pdf(&id) })),
                    item(&t!("Earlier versions"), "history", this.on({ let id = id.clone(); move |this| this.show_history(&id) })),
                    Entry::Separator,
                    widgets::danger(&t!("Move to the trash"), "trash", this.on({ let id = id.clone(); move |this| this.trash_note(&id) })),
                ]
            };
            menu(anchor, entries);
        })));
        self.controls_for(Some(&head));
        root.append(&head);

        if trashed {
            let banner = ui::hbox(12, &["callout", "caution", "banner"]);
            banner.append(&icon("trash", 20));
            let words = ui::label(&t!("This note is in the trash."), &["p"]);
            words.set_hexpand(true);
            banner.append(&words);
            let (a, b) = (id.to_string(), id.to_string());
            banner.append(&button(&t!("Restore"), Some("undo"), &["small"], self.on(move |this| this.restore_note(&a))));
            banner.append(&button(&t!("Delete for good"), None, &["small", "danger"], self.on(move |this| this.delete_forever(vec![b.clone()]))));
            for child in [banner.last_child(), banner.last_child().and_then(|c| c.prev_sibling())].into_iter().flatten() {
                child.set_valign(gtk::Align::Center);
            }
            root.append(&banner);
        }

        // The title, laid out by its own text, Arabic or English whatever the window's language is
        let title = gtk::Entry::builder().css_classes(["plain", "title-input"]).placeholder_text(t!("Title")).editable(!trashed).build();
        title.update_property(&[gtk::accessible::Property::Label(&t!("Title"))]);
        title.set_text(note.str("title"));
        widgets::auto_direction(&title, note.str("title"));
        root.append(&title);

        let tag_row = Flex::new(6, 6, &["tag-row"]);
        let tag_input = gtk::Entry::builder().css_classes(["plain", "tag-input"]).placeholder_text(t!("Add a tag")).build();
        tag_input.update_property(&[gtk::accessible::Property::Label(&t!("Add a tag"))]);
        root.append(&tag_row);

        // The formatting tools
        let toolbar = Flex::new(2, 2, &["toolbar"]);
        toolbar.update_property(&[gtk::accessible::Property::Label(&t!("Formatting"))]);
        root.append(&toolbar);

        // The text and its preview
        let buffer = gtk::TextBuffer::new(None);
        let text = gtk::TextView::builder()
            .buffer(&buffer)
            .wrap_mode(gtk::WrapMode::WordChar)
            .css_classes(["plain", "source"])
            .editable(!trashed)
            .pixels_inside_wrap(0)
            .bottom_margin(40)
            .accepts_tab(false)
            .build();
        text.update_property(&[gtk::accessible::Property::Label(&t!("Note"))]);
        let quiet = Rc::new(Cell::new(true));
        buffer.begin_irreversible_action();
        buffer.set_text(note.str("body"));
        buffer.end_irreversible_action();
        quiet.set(false);
        widgets::auto_direction(&text, note.str("body"));
        let placeholder = ui::line(&t!("Write in Markdown…"), &["placeholder", "source-placeholder"]);
        placeholder.set_halign(gtk::Align::Start);
        placeholder.set_valign(gtk::Align::Start);
        placeholder.set_can_target(false);
        placeholder.set_visible(note.str("body").is_empty());
        let source_overlay = gtk::Overlay::new();
        source_overlay.set_child(Some(&text));
        source_overlay.add_overlay(&placeholder);
        let source_box = ui::vbox(0, &["source-box"]);
        source_box.append(&source_overlay);
        source_overlay.set_vexpand(true);
        let source_scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).hexpand(true).child(&source_box).build();
        let preview = ui::vbox(0, &["preview", "markdown"]);
        preview.update_property(&[gtk::accessible::Property::Label(&t!("Preview"))]);
        let preview_limit = Cap::new(&preview, 0);
        let preview_scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).hexpand(true).child(&preview_limit).css_classes(["preview-scroll"]).build();
        let body = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).homogeneous(true).vexpand(true).css_classes(["editor-body"]).build();
        body.append(&source_scroll);
        body.append(&preview_scroll);
        root.append(&body);
        let foot = ui::label("", &["editor-foot", "faint", "small"]);
        root.append(&foot);

        let editor = Editor {
            id: id.to_string(),
            trashed,
            title: title.clone(),
            buffer: buffer.clone(),
            text: text.clone(),
            body: body.clone(),
            source: source_scroll.clone().upcast(),
            preview_scroll: preview_scroll.clone(),
            preview: preview.clone(),
            preview_cap: preview_limit.clone(),
            toolbar: toolbar.clone(),
            segment: segment_buttons,
            foot: foot.clone(),
            tag_row: tag_row.clone(),
            tag_input: tag_input.clone(),
            placeholder: placeholder.clone(),
            quiet: quiet.clone(),
            preview_timer: RefCell::new(None),
        };
        self.editor.replace(Some(editor));
        self.build_toolbar(&toolbar);
        self.apply_layout(&layout);
        self.render_tags();
        self.update_preview();
        self.update_foot();
        self.wire_editor(&title, &text, &buffer, &tag_input, &body, quiet);
        root.upcast()
    }

    /// The notebooks for the crumb's select, each under its parent, indented by an em space a level
    fn notebook_choices(&self, parent: Option<&str>, depth: usize, seen: &mut HashSet<String>, out: &mut Vec<(String, String)>) {
        for notebook in self.child_notebooks(parent) {
            let id = notebook.id().to_string();
            if seen.insert(id.clone()) {
                let title = if notebook.title().is_empty() { t!("Untitled") } else { notebook.title().to_string() };
                out.push((id.clone(), format!("{}{title}", "\u{2003}".repeat(depth))));
                self.notebook_choices(Some(&id), depth + 1, seen, out);
            }
        }
    }

    fn on_click(&self, f: impl Fn(&Rc<Notes>) + 'static) -> impl Fn(&gtk::Button) + 'static {
        let act = self.on(f);
        move |_| act()
    }

    fn on_clicked(&self, f: impl Fn(&Rc<Notes>) + 'static) -> impl Fn(&gtk::Button) + 'static {
        self.on_click(f)
    }

    fn on_anchor(&self, f: impl Fn(&Rc<Notes>, &gtk::Button) + 'static) -> impl Fn(&gtk::Button) + 'static {
        let weak = self.me.clone();
        move |anchor| {
            if let Some(this) = weak.upgrade() {
                f(&this, anchor);
            }
        }
    }

    fn wire_editor(&self, title: &gtk::Entry, text: &gtk::TextView, buffer: &gtk::TextBuffer, tag_input: &gtk::Entry, body: &gtk::Box, quiet: Rc<Cell<bool>>) {
        let weak = self.me.clone();
        title.connect_changed(move |entry| {
            let Some(this) = weak.upgrade() else { return };
            let Some(id) = this.editor.borrow().as_ref().filter(|e| !e.trashed).map(|e| e.id.clone()) else { return };
            let mut changes = Map::new();
            changes.insert("title".into(), Value::from(entry.text().to_string()));
            this.edit_note(&id, changes);
            widgets::auto_direction(entry, &entry.text());
            this.refresh_list_later();
        });
        // Enter or the down arrow in the title: on to the text, at its start
        let to_start = |view: &gtk::TextView| {
            view.grab_focus();
            let buffer = view.buffer();
            buffer.place_cursor(&buffer.start_iter());
        };
        let to_text = text.clone();
        title.connect_activate(move |_| to_start(&to_text));
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let to_text = text.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Down {
                to_start(&to_text);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        title.add_controller(keys);

        let weak = self.me.clone();
        let changed_quiet = quiet.clone();
        buffer.connect_changed(move |buffer| {
            let Some(this) = weak.upgrade() else { return };
            if changed_quiet.get() {
                return;
            }
            let Some((id, view, placeholder)) = this.editor.borrow().as_ref().filter(|e| !e.trashed).map(|e| (e.id.clone(), e.text.clone(), e.placeholder.clone())) else { return };
            let body = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
            placeholder.set_visible(body.is_empty());
            widgets::auto_direction(&view, &body);
            let mut changes = Map::new();
            changes.insert("body".into(), Value::from(body));
            this.edit_note(&id, changes);
            this.preview_later();
            this.update_foot();
            this.refresh_list_later();
        });

        // Markdown shortcuts in the note: bold, italic and links, lists that carry on with Enter, and Tab
        // that indents rather than leaving the field
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.me.clone();
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(this) = weak.upgrade() else { return glib::Propagation::Proceed };
            this.editor_key(key, modifiers)
        });
        text.add_controller(keys);

        // A picture pasted becomes an attachment, as a pasted file did
        let weak = self.me.clone();
        text.connect_paste_clipboard(move |view| {
            let Some(this) = weak.upgrade() else { return };
            let clipboard = view.clipboard();
            if clipboard.formats().contains_type(gdk::Texture::static_type()) {
                view.stop_signal_emission_by_name("paste-clipboard");
                let this = this.clone();
                clipboard.read_texture_async(gio::Cancellable::NONE, move |texture| {
                    if let Ok(Some(texture)) = texture {
                        let bytes = texture.save_to_png_bytes().to_vec();
                        this.attach_dropped(vec![("image.png".into(), "image/png".into(), bytes)]);
                    }
                });
            }
        });

        // Files dropped on the note, ahead of the text view inside it, which would type their paths
        let target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        target.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.me.clone();
        target.connect_drop(move |_, value, _, _| {
            let (Some(this), Ok(list)) = (weak.upgrade(), value.get::<gdk::FileList>()) else { return false };
            if this.editor.borrow().as_ref().is_none_or(|e| e.trashed) {
                return false;
            }
            let mut files = Vec::new();
            for file in list.files() {
                let Some(path) = file.path() else { continue };
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > backend::ATTACHMENT_LIMIT {
                    toast(&t!("{file} is larger than 200 MB and was not attached.", file = name), None);
                    continue;
                }
                if let Ok(bytes) = std::fs::read(&path) {
                    let mime = backend::type_of(&name);
                    files.push((name, mime, bytes));
                }
            }
            this.attach_dropped(files);
            true
        });
        body.add_controller(target);

        // Tags: Enter or a comma adds one, Backspace in the empty field takes the last off
        let keys = gtk::EventControllerKey::new();
        let weak = self.me.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(this) = weak.upgrade() else { return glib::Propagation::Proceed };
            match key {
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::comma => {
                    this.add_tag();
                    glib::Propagation::Stop
                }
                gdk::Key::BackSpace => {
                    let Some((id, input)) = this.editor.borrow().as_ref().map(|e| (e.id.clone(), e.tag_input.clone())) else { return glib::Propagation::Proceed };
                    let tags = this.item(Kind::Notes, &id).map(|n| n.list("tags")).unwrap_or_default();
                    if input.text().is_empty() && !tags.is_empty() {
                        let mut changes = Map::new();
                        changes.insert("tags".into(), json!(tags[..tags.len() - 1]));
                        this.edit_note(&id, changes);
                        this.render_tags();
                        this.render_nav();
                        this.focus_tag_input();
                        return glib::Propagation::Stop;
                    }
                    glib::Propagation::Proceed
                }
                _ => glib::Propagation::Proceed,
            }
        });
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        tag_input.add_controller(keys);
        // The tags there are, offered as the name is typed, as the datalist did
        let weak = self.me.clone();
        tag_input.connect_changed(move |input| {
            if let Some(this) = weak.upgrade() {
                this.offer_tags(input);
            }
        });
    }

    /// The tags whose names start with what is typed, in a list under the field
    fn offer_tags(&self, input: &gtk::Entry) {
        let typed = input.text().trim().trim_start_matches('#').to_lowercase();
        let popover = match input.first_child().and_then(|c| c.next_sibling()).and_then(|c| c.downcast::<gtk::Popover>().ok()) {
            Some(popover) => popover,
            None => {
                let popover = gtk::Popover::builder().has_arrow(false).autohide(false).css_classes(["choices"]).position(gtk::PositionType::Bottom).halign(gtk::Align::Start).build();
                popover.set_parent(input);
                let focus = gtk::EventControllerFocus::new();
                let hiding = popover.clone();
                focus.connect_leave(move |_| {
                    let hiding = hiding.clone();
                    glib::timeout_add_local_once(Duration::from_millis(150), move || hiding.popdown());
                });
                input.add_controller(focus);
                popover
            }
        };
        if typed.is_empty() {
            popover.popdown();
            return;
        }
        let mut tags: Vec<Value> = self.db(|store| store.values(Kind::Tags).cloned().collect()).unwrap_or_default();
        sort_by_title(&mut tags);
        let have = self.editor.borrow().as_ref().and_then(|e| self.item(Kind::Notes, &e.id)).map(|n| n.list("tags")).unwrap_or_default();
        let matching: Vec<String> = tags
            .iter()
            .filter(|tag| !have.contains(&tag.id().to_string()))
            .map(|tag| tag.title().to_string())
            .filter(|title| title.to_lowercase().starts_with(&typed) && title.to_lowercase() != typed)
            .take(8)
            .collect();
        if matching.is_empty() {
            popover.popdown();
            return;
        }
        let list = ui::vbox(0, &[]);
        for title in matching {
            let option = gtk::Button::builder().child(&ui::line(&title, &[])).css_classes(["plain", "option"]).build();
            let (weak, input, popover_copy) = (self.me.clone(), input.clone(), popover.clone());
            option.connect_clicked(move |_| {
                popover_copy.popdown();
                input.set_text(&title);
                if let Some(this) = weak.upgrade() {
                    this.add_tag();
                }
            });
            list.append(&option);
        }
        popover.set_child(Some(&list));
        popover.popup();
    }

    fn focus_tag_input(&self) {
        if let Some(input) = self.editor.borrow().as_ref().map(|e| e.tag_input.clone()) {
            glib::idle_add_local_once(move || {
                input.grab_focus();
            });
        }
    }

    fn add_tag(&self) {
        let Some((id, input)) = self.editor.borrow().as_ref().map(|e| (e.id.clone(), e.tag_input.clone())) else { return };
        let value = input.text().trim_start_matches('#').trim().to_string();
        input.set_text("");
        if value.is_empty() {
            return;
        }
        let tag = self.tag_for(&value);
        let tags = self.item(Kind::Notes, &id).map(|n| n.list("tags")).unwrap_or_default();
        if !tags.contains(&tag) {
            let mut next = tags;
            next.push(tag);
            let mut changes = Map::new();
            changes.insert("tags".into(), json!(next));
            self.edit_note(&id, changes);
        }
        self.render_tags();
        self.render_nav();
        self.focus_tag_input();
    }

    fn render_tags(&self) {
        let Some((id, row, input, trashed)) = self.editor.borrow().as_ref().map(|e| (e.id.clone(), e.tag_row.clone(), e.tag_input.clone(), e.trashed)) else { return };
        if input.parent().is_some() {
            input.unparent();
        }
        row.clear();
        let hash = icon("hash", 15);
        hash.add_css_class("faint");
        row.append(&hash);
        let tags: Vec<Value> = self.item(Kind::Notes, &id).map(|n| n.list("tags")).unwrap_or_default().iter().filter_map(|tag| self.item(Kind::Tags, tag)).collect();
        for tag in tags {
            let chip = ui::hbox(3, &["chip"]);
            chip.append(&widgets::cut(tag.title(), &[]));
            if !trashed {
                let tag_id = tag.id().to_string();
                let x = icon_button("close", &t!("Remove {name}", name = tag.title()), &["chip-x"], self.on_click(move |this| {
                    let Some(id) = this.editor.borrow().as_ref().map(|e| e.id.clone()) else { return };
                    let tags = this.item(Kind::Notes, &id).map(|n| n.list("tags")).unwrap_or_default();
                    let mut changes = Map::new();
                    changes.insert("tags".into(), json!(tags.into_iter().filter(|t| *t != tag_id).collect::<Vec<_>>()));
                    this.edit_note(&id, changes);
                    this.render_tags();
                    this.render_nav();
                }));
                x.set_tooltip_text(None);
                if let Some(mark) = x.child().and_then(|c| c.downcast::<amethystora_ui::icon::Icon>().ok()) {
                    mark.set_size(12);
                }
                chip.append(&x);
            }
            row.append(&ui::limit(&chip, 160));
        }
        if !trashed {
            row.append_with(&input, Grow { grow: 1.0, basis: None, min: Some(120), push: false });
        }
    }

    fn build_toolbar(&self, toolbar: &Flex) {
        let tool = |glyph: &'static str, label: String, run: Rc<dyn Fn(&Notes)>| -> gtk::Button {
            let mark = icon(glyph, 17);
            mark.set_hexpand(true);
            let button = gtk::Button::builder().child(&mark).css_classes(["plain", "tool"]).tooltip_text(&label).build();
            button.update_property(&[gtk::accessible::Property::Label(&label)]);
            let weak = self.me.clone();
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    run(&this);
                }
            });
            button
        };
        let sep = || ui::hbox(0, &["tool-sep"]);
        toolbar.append(&tool("bold", t!("{action} ({key})", action = t!("Bold"), key = "Ctrl+B"), Rc::new(|this| this.surround("**", "**", &t!("bold text")))));
        toolbar.append(&tool("italic", t!("{action} ({key})", action = t!("Italic"), key = "Ctrl+I"), Rc::new(|this| this.surround("_", "_", &t!("italic text")))));
        toolbar.append(&tool("heading", t!("Heading"), Rc::new(|this| this.prefix_lines("## ", false, false))));
        toolbar.append(&sep());
        toolbar.append(&tool("list", t!("List"), Rc::new(|this| this.prefix_lines("- ", false, false))));
        toolbar.append(&tool("checklist", t!("Checklist"), Rc::new(|this| this.prefix_lines("- [ ] ", false, false))));
        toolbar.append(&tool("quote", t!("Quote"), Rc::new(|this| this.prefix_lines("> ", false, false))));
        toolbar.append(&tool("code", t!("Code"), Rc::new(|this| {
            if this.selection().contains('\n') {
                this.surround("```\n", "\n```", &t!("code"));
            } else {
                this.surround("`", "`", &t!("code"));
            }
        })));
        toolbar.append(&tool("link", t!("{action} ({key})", action = t!("Link"), key = "Ctrl+K"), Rc::new(|this| this.surround("[", "](https://)", &t!("link text")))));
        toolbar.append(&sep());
        toolbar.append(&tool("clip", t!("Attach a file"), Rc::new(|this| this.attach_chosen())));
    }

    fn set_layout(&self, next: &str) {
        self.set_setting("layout", Value::from(next));
        self.apply_layout(next);
        if next != "edit" {
            self.update_preview();
        }
    }

    pub(super) fn cycle_layout(&self) {
        let layout = self.setting("layout").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "split".into());
        self.set_layout(match layout.as_str() {
            "edit" => "split",
            "split" => "preview",
            _ => "edit",
        });
    }

    /// The editor and the preview as the layout has them: one, the other, or both, side by side or, in a
    /// narrow window, one above the other
    fn apply_layout(&self, layout: &str) {
        let editor = self.editor.borrow();
        let Some(editor) = editor.as_ref() else { return };
        for (value, button) in &editor.segment {
            widgets::set_on(button, value == layout);
            button.update_state(&[gtk::accessible::State::Pressed(if value == layout { gtk::AccessibleTristate::True } else { gtk::AccessibleTristate::False })]);
        }
        editor.toolbar.set_visible(layout != "preview" && !editor.trashed);
        editor.source.set_visible(layout != "preview");
        editor.preview_scroll.set_visible(layout != "edit");
        for class in ["split", "preview-only", "edit-only"] {
            editor.body.remove_css_class(class);
        }
        editor.body.add_css_class(match layout {
            "split" => "split",
            "preview" => "preview-only",
            _ => "edit-only",
        });
        let narrow = self.shell.window.has_css_class("narrow");
        editor.body.set_orientation(if layout == "split" && narrow { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal });
        // In the preview alone, the text at most 820 pixels wide
        editor.preview_cap.set_max(if layout == "preview" { 820 } else { 0 });
    }

    /// The window became narrow or wide again: the split editor turns, as the page's media query turned it
    pub(super) fn narrowed(&self) {
        if self.editor.borrow().is_some() {
            let layout = self.setting("layout").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "split".into());
            self.apply_layout(&layout);
        }
    }

    fn update_foot(&self) {
        let Some((id, foot, trashed)) = self.editor.borrow().as_ref().map(|e| (e.id.clone(), e.foot.clone(), e.trashed)) else { return };
        let Some(note) = self.item(Kind::Notes, &id) else { return };
        let when = if trashed {
            t!("In the trash since {date}", date = dates::stamp(note.num_or_zero("trashed")))
        } else {
            t!("Changed {date}", date = dates::stamp(note.num_or_zero("updated")))
        };
        let count = t!("{count, plural, one {# word} other {# words}}", count = words(&data::text(note.get("body"))));
        foot.set_label(&format!("{when} · {count}"));
    }

    fn preview_later(&self) {
        let editor = self.editor.borrow();
        let Some(editor) = editor.as_ref() else { return };
        if let Some(timer) = editor.preview_timer.take() {
            timer.remove();
        }
        let weak = self.me.clone();
        editor.preview_timer.replace(Some(glib::timeout_add_local_once(Duration::from_millis(150), move || {
            if let Some(this) = weak.upgrade() {
                if let Some(editor) = this.editor.borrow().as_ref() {
                    editor.preview_timer.take();
                }
                this.update_preview();
            }
        })));
    }

    fn refresh_list_later(&self) {
        let parts = self.notes_parts.borrow();
        let Some(parts) = parts.as_ref() else { return };
        if let Some(timer) = parts.refresh.take() {
            timer.remove();
        }
        let weak = self.me.clone();
        parts.refresh.replace(Some(glib::timeout_add_local_once(Duration::from_millis(400), move || {
            let Some(this) = weak.upgrade() else { return };
            if let Some(parts) = this.notes_parts.borrow().as_ref() {
                parts.refresh.take();
            }
            let view = this.ui.borrow().notes_view.clone();
            let list = this.sorted_notes(&view);
            this.render_note_list(&list);
        })));
    }

    pub(super) fn refresh_preview(&self) {
        if self.editor.borrow().is_some() {
            self.update_preview();
        }
    }

    /// The note's Markdown drawn into the preview
    fn update_preview(&self) {
        let Some((id, preview, trashed, scroll)) = self.editor.borrow().as_ref().map(|e| (e.id.clone(), e.preview.clone(), e.trashed, e.preview_scroll.clone())) else { return };
        if !scroll.is_visible() {
            return;
        }
        let body = self.item(Kind::Notes, &id).map(|n| data::text(n.get("body"))).unwrap_or_default();
        let at = scroll.vadjustment().value();
        ui::clear(&preview);
        let toggle: Option<Rc<dyn Fn(usize)>> = if trashed {
            None
        } else {
            let weak = self.me.clone();
            Some(Rc::new(move |index| {
                let Some(this) = weak.upgrade() else { return };
                let this = this.clone();
                // After the click that asked for it has been handled
                glib::idle_add_local_once(move || this.toggle_box(index));
            }))
        };
        let rendered = self.render_markdown(&body, toggle);
        preview.append(&rendered);
        let adjustment = scroll.vadjustment();
        glib::idle_add_local_once(move || adjustment.set_value(at));
    }

    fn toggle_box(&self, index: usize) {
        let Some((id, buffer, quiet)) = self.editor.borrow().as_ref().map(|e| (e.id.clone(), e.buffer.clone(), e.quiet.clone())) else { return };
        let body = self.item(Kind::Notes, &id).map(|n| data::text(n.get("body"))).unwrap_or_default();
        let next = toggle_checkbox(&body, index);
        quiet.set(true);
        buffer.set_text(&next);
        quiet.set(false);
        let mut changes = Map::new();
        changes.insert("body".into(), Value::from(next));
        self.edit_note(&id, changes);
        self.update_preview();
        self.update_foot();
    }

    /// marked, set to show a note's HTML as text, and to reach attachments through their ids: pictures
    /// decrypted here, links to them as chips that save a copy
    pub(super) fn render_markdown(&self, body: &str, toggle: Option<Rc<dyn Fn(usize)>>) -> gtk::Widget {
        let weak = self.me.clone();
        let mut options = markdown::Options::new(
            Box::new(|href| {
                let id = href.strip_prefix(":/").filter(|id| data::is_id(id));
                match id {
                    Some(id) => format!("res://{id}/"),
                    None => href.to_string(),
                }
            }),
            Rc::new(move |href| {
                if let Some(this) = weak.upgrade() {
                    this.follow(href);
                }
            }),
            margins,
        );
        options.breaks = true;
        options.code_pad = 1;
        options.code_radius = 5.0;
        options.attachment = Some(Rc::new(|href: &str| href.starts_with("res://")));
        options.clip = Some(crate::icons::data("clip"));
        options.attachment_color = hex(&self.link_color());
        options.task = toggle;
        let weak = self.me.clone();
        options.image = Some(Rc::new(move |href: &str, _alt: &str| {
            let this = weak.upgrade()?;
            this.picture(href)
        }));
        let rendered = markdown::render(body, &options);
        // Each block is laid out by its own text, Arabic or English whatever the window's language is,
        // and code always from left to right
        let zoom = amethystora_ui::icon::zoom();
        let mut first = true;
        let mut child = rendered.root.first_child();
        while let Some(block) = child {
            if first {
                // .markdown > :first-child { margin-top: 0.4em }
                block.set_margin_top((6.0 * zoom).round() as i32);
                first = false;
            }
            if block.has_css_class("code") {
                block.set_direction(gtk::TextDirection::Ltr);
            } else {
                let text = block_text(&block);
                widgets::auto_direction(&block, &text);
            }
            child = block.next_sibling();
        }
        keep_quote_margins(rendered.root.upcast_ref(), zoom);
        self.attachment_titles(body, rendered.root.upcast_ref());
        rendered.root.upcast()
    }

    /// An attachment's chip says what it is when the pointer is on it, as its link's title did: its name
    /// and size, and that a click saves a copy
    fn attachment_titles(&self, body: &str, root: &gtk::Widget) {
        use pulldown_cmark::{Event, Parser, Tag, TagEnd};
        let mut links: Vec<(String, String)> = Vec::new();
        let mut current: Option<(String, String)> = None;
        for event in Parser::new_ext(body, pulldown_cmark::Options::ENABLE_TABLES | pulldown_cmark::Options::ENABLE_TASKLISTS) {
            match event {
                Event::Start(Tag::Link { dest_url, .. }) => {
                    current = dest_url.strip_prefix(":/").filter(|id| data::is_id(id)).map(|id| (String::new(), id.to_string()));
                }
                Event::Text(text) | Event::Code(text) => {
                    if let Some((words, _)) = current.as_mut() {
                        words.push_str(&text);
                    }
                }
                Event::End(TagEnd::Link) => {
                    if let Some(link) = current.take().filter(|(words, _)| !words.is_empty()) {
                        links.push(link);
                    }
                }
                _ => {}
            }
        }
        if links.is_empty() {
            return;
        }
        let titles: Vec<(String, String)> = links
            .into_iter()
            .filter_map(|(words, id)| {
                let resource = self.db(|store| store.resources.get(&id).cloned()).flatten()?;
                Some((words, t!("{name}, {size}. Click to save a copy.", name = data::text(resource.get("name")), size = size(resource.num_or_zero("size")))))
            })
            .collect();
        let mut labels = Vec::new();
        collect_labels(root, &mut labels);
        for label in labels {
            let text = label.text().to_string();
            let ranges: Vec<(usize, usize, String)> =
                titles.iter().flat_map(|(words, title)| text.match_indices(words.as_str()).map(|(at, _)| (at, at + words.len(), title.clone())).collect::<Vec<_>>()).collect();
            if ranges.is_empty() {
                continue;
            }
            label.set_has_tooltip(true);
            label.connect_query_tooltip(move |label, x, y, _, tooltip| {
                let (ox, oy) = label.layout_offsets();
                let (inside, index, _) = label.layout().xy_to_index((x - ox) * pango::SCALE, (y - oy) * pango::SCALE);
                let index = index as usize;
                match ranges.iter().find(|(from, to, _)| inside && *from <= index && index < *to) {
                    Some((_, _, title)) => {
                        tooltip.set_text(Some(title));
                        true
                    }
                    None => false,
                }
            });
        }
    }

    fn link_color(&self) -> gdk::RGBA {
        let probe = ui::line("", &["probe-chip-icon"]);
        let holder = ui::vbox(0, &["markdown"]);
        holder.append(&probe);
        holder.set_visible(false);
        self.main.append(&holder);
        let color = probe.color();
        self.main.remove(&holder);
        color
    }

    /// An attachment's picture, or one written into the note: anything else, a file: address in a note
    /// somebody shared, would show any picture on this machine
    fn picture(&self, href: &str) -> Option<gtk::Widget> {
        let texture = if let Some(id) = href.strip_prefix("res://").map(|rest| rest.trim_end_matches('/')) {
            let backend = self.backend.borrow();
            if !data::is_id(id) || !backend.store.as_ref().is_some_and(|s| s.resources.contains_key(id)) {
                return None;
            }
            let bytes = backend.read_resource(id).ok()?;
            gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)).ok()?
        } else if let Some(rest) = href.strip_prefix("data:image/") {
            let (_, encoded) = rest.split_once(";base64,")?;
            gdk::Texture::from_bytes(&glib::Bytes::from_owned(crate::vault::unbase64(encoded))).ok()?
        } else {
            return None;
        };
        // At most its own size, and never wider than the page
        Some(crate::layout::Image::new(&texture).upcast())
    }

    /// Links in a rendered note: attachments are saved, web links go to the browser, anything else stays put
    fn follow(&self, href: &str) {
        if let Some(id) = href.strip_prefix("res://").map(|rest| rest.trim_end_matches('/').to_string()) {
            self.save_attachment(id);
        } else {
            let lower = href.to_lowercase();
            if lower.starts_with("http:") || lower.starts_with("https:") || lower.starts_with("mailto:") {
                process::open_uri(Some(&self.shell.gtk_window()), href);
            }
        }
    }

    fn save_attachment(&self, id: String) {
        self.spawn(async move |this| {
            let Some(resource) = this.db(|store| store.resources.get(&id).cloned()).flatten() else {
                toast(&t!("That attachment is not here any more."), None);
                return;
            };
            let name = data::text(resource.get("name")).replace(['/', '\\'], "-");
            let dialog = gtk::FileDialog::builder().title(t!("Save attachment")).initial_name(&name).modal(true).build();
            dialog.set_initial_folder(Some(&gio::File::for_path(widgets::special_dir(glib::UserDirectory::Downloads))));
            let Ok(file) = dialog.save_future(Some(&this.shell.window)).await else { return };
            let Some(path) = file.path() else { return };
            match backend::save_resource(&this.backend, &id, path.clone()).await {
                Ok(()) => toast(&t!("Saved to {path}", path = path.display().to_string()), None),
                Err(error) => toast(&error, None),
            }
        });
    }

    // --- Editing the text ----------------------------------------------------------------------------------

    fn editor_key(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        let control = modifiers.contains(gdk::ModifierType::CONTROL_MASK) || modifiers.contains(gdk::ModifierType::META_MASK);
        let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
        let alt = modifiers.contains(gdk::ModifierType::ALT_MASK);
        if self.editor.borrow().as_ref().is_none_or(|e| e.trashed) {
            return glib::Propagation::Proceed;
        }
        let name = key.to_lower().name().map(|n| n.to_string()).unwrap_or_default();
        if control && name == "b" {
            self.surround("**", "**", &t!("bold text"));
            return glib::Propagation::Stop;
        }
        if control && name == "i" {
            self.surround("_", "_", &t!("italic text"));
            return glib::Propagation::Stop;
        }
        if control && name == "k" {
            self.surround("[", "](https://)", &t!("link text"));
            return glib::Propagation::Stop;
        }
        if matches!(key, gdk::Key::Tab | gdk::Key::ISO_Left_Tab) && !control && !alt {
            if shift || key == gdk::Key::ISO_Left_Tab {
                self.prefix_lines("    ", false, true);
            } else if !self.selection().is_empty() {
                self.prefix_lines("    ", true, false);
            } else {
                self.insert_text("    ");
            }
            return glib::Propagation::Stop;
        }
        if matches!(key, gdk::Key::Return | gdk::Key::KP_Enter) && !shift && !control {
            return self.continue_list();
        }
        glib::Propagation::Proceed
    }

    /// The text, and the selection as character offsets
    fn text_and_selection(&self) -> Option<(Vec<char>, usize, usize, gtk::TextBuffer)> {
        let buffer = self.editor.borrow().as_ref()?.buffer.clone();
        let text: Vec<char> = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).chars().collect();
        let (start, end) = match buffer.selection_bounds() {
            Some((a, b)) => (a.offset().min(b.offset()) as usize, a.offset().max(b.offset()) as usize),
            None => {
                let at = buffer.iter_at_mark(&buffer.get_insert()).offset() as usize;
                (at, at)
            }
        };
        Some((text, start, end, buffer))
    }

    fn selection(&self) -> String {
        match self.text_and_selection() {
            Some((text, start, end, _)) => text[start..end].iter().collect(),
            None => String::new(),
        }
    }

    /// Replaces characters from..to with `with`, as one step that Ctrl+Z undoes, as insertText did
    fn replace_range(&self, from: usize, to: usize, with: &str) {
        let Some(buffer) = self.editor.borrow().as_ref().map(|e| e.buffer.clone()) else { return };
        buffer.begin_user_action();
        let mut start = buffer.iter_at_offset(from as i32);
        let mut end = buffer.iter_at_offset(to as i32);
        buffer.delete(&mut start, &mut end);
        let mut at = buffer.iter_at_offset(from as i32);
        buffer.insert(&mut at, with);
        buffer.end_user_action();
    }

    fn select_range(&self, from: usize, to: usize) {
        let Some((buffer, view)) = self.editor.borrow().as_ref().map(|e| (e.buffer.clone(), e.text.clone())) else { return };
        buffer.select_range(&buffer.iter_at_offset(from as i32), &buffer.iter_at_offset(to as i32));
        view.grab_focus();
    }

    fn insert_text(&self, text: &str) {
        let Some((_, start, end, _)) = self.text_and_selection() else { return };
        self.replace_range(start, end, text);
        let at = start + text.chars().count();
        self.select_range(at, at);
    }

    /// Through one undoable step, as insertText did
    fn surround(&self, before: &str, after: &str, placeholder: &str) {
        let Some((text, start, end, _)) = self.text_and_selection() else { return };
        let selected: String = text[start..end].iter().collect();
        let selected = if selected.is_empty() { placeholder.to_string() } else { selected };
        self.replace_range(start, end, &format!("{before}{selected}{after}"));
        let from = start + before.chars().count();
        self.select_range(from, from + selected.chars().count());
    }

    /// Adds the prefix to every selected line, or takes it off when they all have it
    fn prefix_lines(&self, prefix: &str, add: bool, take: bool) {
        let Some((text, sel_start, sel_end, _)) = self.text_and_selection() else { return };
        // value.lastIndexOf('\n', selectionStart - 1) + 1, a negative start looking at the first character
        let look = sel_start.saturating_sub(1);
        let start = (0..=look.min(text.len().saturating_sub(1))).rev().find(|&at| text.get(at) == Some(&'\n')).map(|at| at + 1).unwrap_or(0);
        let from = sel_end - usize::from(sel_end > sel_start);
        let end = (from..text.len()).find(|&at| text[at] == '\n').unwrap_or(text.len());
        let start = start.min(end);
        let block: String = text[start..end].iter().collect();
        let lines: Vec<&str> = block.split('\n').collect();
        let all = lines.iter().all(|line| line.starts_with(prefix));
        let next = lines
            .iter()
            .map(|line| {
                if take || (all && !add) {
                    match line.strip_prefix(prefix) {
                        Some(rest) => rest.to_string(),
                        None => {
                            let spaces = line.chars().take(4).take_while(|c| *c == ' ').count();
                            line[spaces..].to_string()
                        }
                    }
                } else {
                    format!("{prefix}{line}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.replace_range(start, end, &next);
        self.select_range(start, start + next.chars().count());
    }

    /// Enter in a list carries it on with the next item; in an empty item it ends the list
    fn continue_list(&self) -> glib::Propagation {
        thread_local! {
            static ITEM: Regex = Regex::new(r"^(\s*(?:>\s*)?)([-*+]|([0-9]+)([.)]))(\s+)(\[[ xX]\]\s+)?").expect("a pattern");
        }
        let Some((text, sel_start, sel_end, _)) = self.text_and_selection() else { return glib::Propagation::Proceed };
        if sel_start != sel_end {
            return glib::Propagation::Proceed;
        }
        let look = sel_start.saturating_sub(1);
        let start = if sel_start == 0 { if text.first() == Some(&'\n') { 1 } else { 0 } } else { (0..=look).rev().find(|&at| text[at] == '\n').map(|at| at + 1).unwrap_or(0) };
        let start = start.min(sel_start);
        let line: String = text[start..sel_start].iter().collect();
        let found = ITEM.with(|item| {
            item.captures(&line).map(|c| {
                (
                    c.get(0).map(|m| m.as_str().to_string()).unwrap_or_default(),
                    c[1].to_string(),
                    c[2].to_string(),
                    c.get(3).map(|m| m.as_str().to_string()),
                    c.get(4).map(|m| m.as_str().to_string()).unwrap_or_default(),
                    c[5].to_string(),
                    c.get(6).is_some(),
                )
            })
        });
        let Some((whole, lead, bullet, number, dot, space, task)) = found else { return glib::Propagation::Proceed };
        if line.chars().count() == whole.chars().count() {
            // An empty item ends the list
            self.replace_range(start, sel_start, "");
            self.select_range(start, start);
            return glib::Propagation::Stop;
        }
        let marker = match number {
            Some(number) => format!("{}{dot}", number.parse::<u64>().unwrap_or(0) + 1),
            None => bullet,
        };
        self.insert_text(&format!("\n{lead}{marker}{space}{}", if task { "[ ] " } else { "" }));
        glib::Propagation::Stop
    }

    // --- Attachments -------------------------------------------------------------------------------------

    fn insert_attachments(&self, resources: &[Value]) {
        let text = resources
            .iter()
            .map(|resource| {
                let name = data::text(resource.get("name")).replace(['[', ']'], "");
                if data::text(resource.get("mime")).starts_with("image/") { format!("![{name}](:/{})", resource.id()) } else { format!("[{name}](:/{})", resource.id()) }
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            if let Some(view) = self.editor.borrow().as_ref().map(|e| e.text.clone()) {
                view.grab_focus();
            }
            self.insert_text(&text);
        }
    }

    fn attach_chosen(&self) {
        self.spawn(async |this| {
            if this.backend.borrow().store.is_none() {
                return;
            }
            let dialog = gtk::FileDialog::builder().title(t!("Attach files")).modal(true).build();
            let Ok(files) = dialog.open_multiple_future(Some(&this.shell.window)).await else { return };
            let paths: Vec<std::path::PathBuf> = (0..files.n_items()).filter_map(|at| files.item(at).and_downcast::<gio::File>()).filter_map(|file| file.path()).collect();
            let added = backend::attach_files(&this.backend, paths).await;
            this.insert_attachments(&added);
        });
    }

    fn attach_dropped(&self, files: Vec<(String, String, Vec<u8>)>) {
        self.spawn(async move |this| {
            let mut added = Vec::new();
            for (name, mime, bytes) in files {
                match backend::attach_data(&this.backend, name, mime, bytes).await {
                    Ok(resource) => added.push(resource),
                    Err(error) if !error.is_empty() => toast(&error, None),
                    Err(_) => {}
                }
            }
            this.insert_attachments(&added);
        });
    }

    // --- History -----------------------------------------------------------------------------------------

    fn show_history(&self, id: &str) {
        self.commit();
        let mut revisions: Vec<Value> = self.db(|store| store.values(Kind::Revisions).filter(|r| r.str("note") == id).cloned().collect()).unwrap_or_default();
        revisions.sort_by(|a, b| b.num_or_zero("saved").partial_cmp(&a.num_or_zero("saved")).unwrap_or(std::cmp::Ordering::Equal));
        if revisions.is_empty() {
            toast(&t!("No earlier versions yet. One is kept each time you come back to change a note after ten minutes away from it."), None);
            return;
        }
        let id = id.to_string();
        self.spawn(async move |this| {
            let chosen = Rc::new(RefCell::new(revisions[0].clone()));
            let list = ui::vbox(2, &["history-list"]);
            let preview = ui::vbox(0, &["markdown", "history-preview"]);
            let list_scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&list).css_classes(["history-list-scroll"]).build();
            let preview_scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&preview).hexpand(true).build();
            preview_scroll.add_css_class("history-preview-scroll");
            let list_column = Exact::new(&list_scroll);
            list_scroll.set_size_request(240, -1);
            let history = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).css_classes(["history"]).build();
            history.append(&list_column);
            history.append(&preview_scroll);
            history.set_overflow(gtk::Overflow::Hidden);
            // min(60vh, 560px)
            let height = ((this.shell.window.height() as f64 * 0.6) as i32).min(560);
            history.set_size_request(-1, height);
            let show: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
            let (weak, list_copy, preview_copy, chosen_copy, show_copy) = (this.me.clone(), list.clone(), preview.clone(), chosen.clone(), show.clone());
            let draw: Rc<dyn Fn()> = Rc::new(move || {
                let Some(this) = weak.upgrade() else { return };
                ui::clear(&list_copy);
                for revision in &revisions {
                    let content = ui::vbox(1, &[]);
                    content.append(&ui::line(&dates::stamp(revision.num_or_zero("saved")), &[]));
                    let title = if revision.title().is_empty() { t!("Untitled") } else { revision.title().to_string() };
                    content.append(&widgets::cut(&title, &["faint", "small"]));
                    let row = gtk::Button::builder().child(&content).css_classes(["plain", "history-item"]).build();
                    widgets::set_class(&row, "current", revision.id() == chosen_copy.borrow().id());
                    let (picked, chosen, show) = (revision.clone(), chosen_copy.clone(), show_copy.clone());
                    row.connect_clicked(move |_| {
                        chosen.replace(picked.clone());
                        if let Some(show) = show.borrow().clone() {
                            show();
                        }
                    });
                    list_copy.append(&row);
                }
                let chosen = chosen_copy.borrow().clone();
                let title = if chosen.title().is_empty() { t!("Untitled") } else { chosen.title().to_string() };
                ui::clear(&preview_copy);
                preview_copy.append(&this.render_markdown(&format!("# {}\n\n{}", escape_markdown(&title), data::text(chosen.get("body"))), None));
            });
            show.replace(Some(draw.clone()));
            draw();
            let picked = chosen.clone();
            let restore = widgets::modal(Dialog {
                title: t!("Earlier versions"),
                text: Some(t!("A version is kept each time you come back to change a note after ten minutes away from it, up to the last 30.")),
                body: vec![history.upcast()],
                actions: vec![
                    Action::plain(&t!("Close"), None),
                    Action::primary(&t!("Restore this version"), false, move || {
                        let chosen = picked.borrow().clone();
                        Box::pin(async move { Some(Some(chosen)) })
                    }),
                ],
                wide: true,
            })
            .await
            .flatten();
            show.replace(None);
            let Some(restore) = restore else { return };
            // What is there now becomes a version of its own, so restoring can itself be undone
            if let Some(note) = this.item(Kind::Notes, &id) {
                this.mark_sent(&id, note.get("title").cloned().unwrap_or(Value::Null), note.get("body").cloned().unwrap_or(Value::Null), 0.0);
            }
            let mut changes = Map::new();
            changes.insert("title".into(), restore.get("title").cloned().unwrap_or(Value::Null));
            changes.insert("body".into(), restore.get("body").cloned().unwrap_or(Value::Null));
            this.edit_note(&id, changes);
            this.commit();
            this.render_notes(true);
            this.render_nav();
            toast(&t!("Restored. The version it replaced is in the history too."), None);
        });
    }

    /// Printed to a PDF, as the page's print sheet was
    fn export_pdf(&self, id: &str) {
        self.commit();
        let Some(note) = self.item(Kind::Notes, id) else { return };
        let title = if note.title().is_empty() { t!("Untitled") } else { note.title().to_string() };
        self.spawn(async move |this| {
            // The name the file is offered under, English like every name the app gives a file
            let file_name = format!("{}.pdf", crate::export::file_name(note.title(), "Untitled"));
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("PDF"));
            filter.add_suffix("pdf");
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);
            let dialog = gtk::FileDialog::builder().title(t!("Export as PDF")).initial_name(&file_name).filters(&filters).modal(true).build();
            dialog.set_initial_folder(Some(&gio::File::for_path(widgets::special_dir(glib::UserDirectory::Documents))));
            let Ok(file) = dialog.save_future(Some(&this.shell.window)).await else { return };
            let Some(path) = file.path() else { return };
            let pictures = this.note_pictures(&data::text(note.get("body")));
            match crate::print::pdf(&this.shell.window, &title, &data::text(note.get("body")), &pictures, &path) {
                Ok(()) => toast(&t!("Saved to {path}", path = path.display().to_string()), None),
                Err(error) => toast(&error, None),
            }
        });
    }

    /// The pictures a note shows, decrypted, by the address its Markdown gives them
    fn note_pictures(&self, body: &str) -> HashMap<String, gdk::Texture> {
        let mut out = HashMap::new();
        for id in backend::resource_links(body) {
            let backend = self.backend.borrow();
            if let Ok(bytes) = backend.read_resource(&id) {
                if let Ok(texture) = gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)) {
                    out.insert(format!("res://{id}/"), texture);
                }
            }
        }
        out
    }
}

/// A size the way the language writes it: "512 bytes", "1.4 MB"
pub fn size(bytes: f64) -> String {
    use amethystora_ui::i18n::{Unit, Width};
    let units = [Unit::Byte, Unit::Kilobyte, Unit::Megabyte, Unit::Gigabyte];
    let mut value = if bytes.is_finite() { bytes } else { 0.0 };
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let digits = if unit > 0 && value < 10.0 { 1 } else { 0 };
    amethystora_ui::i18n::unit(value, units[unit], if unit > 0 { Width::Short } else { Width::Long }, digits, digits)
}

/// A quote keeps its last paragraph's margin inside its bar: the page had no blockquote > :last-child rule,
/// which the shared renderer applies, as the Manual's stylesheet has it
fn keep_quote_margins(widget: &gtk::Widget, zoom: f64) {
    if widget.has_css_class("blockquote") {
        if let Some(last) = widget.last_child() {
            last.set_margin_bottom((margins(Block::P).1 as f64 * zoom).round() as i32);
        }
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        keep_quote_margins(&current, zoom);
        child = current.next_sibling();
    }
}

fn collect_labels(widget: &gtk::Widget, out: &mut Vec<gtk::Label>) {
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        out.push(label.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        collect_labels(&current, out);
        child = current.next_sibling();
    }
}

/// The text of a block, its labels' in order, for the direction it is laid out in
fn block_text(widget: &gtk::Widget) -> String {
    let mut out = String::new();
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        out.push_str(&label.text());
        return out;
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        out.push_str(&block_text(&current));
        if !out.is_empty() && pango::find_base_dir(&out) != pango::Direction::Neutral {
            return out;
        }
        child = current.next_sibling();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippets_as_notes_js() {
        assert_eq!(snippet("# Title\n\nSome **bold** text with [a link](https://x) and ![pic](:/abc).\n```\ncode\n```\n- [ ] task"), "Title Some bold text with a link and . task");
        assert_eq!(words("Hello, world — 3 times; مرحبا"), 5);
        assert_eq!(toggle_checkbox("- [ ] a\n```\n- [ ] in code\n```\n* [x] b", 1), "- [ ] a\n```\n- [ ] in code\n```\n* [ ] b");
        assert_eq!(toggle_checkbox("> - [ ] quoted", 0), "> - [x] quoted");
        assert_eq!(escape_markdown("a#b*c"), "a\\#b\\*c");
        assert_eq!(size(512.0), "512 bytes");
        assert_eq!(size(1468006.0), "1.4 MB");
        assert_eq!(size(17154.0), "17 kB");
    }
}
