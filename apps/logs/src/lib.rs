//! Amethystora Logs: what the system, the desktop and the apps have written to the journal, in a window of
//! its own. It takes the place of GNOME Logs, which the upstream Flatpak list installed. backend.rs reads the
//! journal, as main.js did; list.rs is the list of entries, which can be tens of thousands long and is only
//! ever drawn as far as it is seen; page.rs draws the rest, as logs.js did.
//!
//! Everything is built as widgets with text in them and never as markup: a log message is whatever any
//! program on the machine chose to write.

mod backend;
mod icons;
mod layout;
mod list;
mod page;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::Duration;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::app::{self, App, Shell, Spec};
use amethystora_ui::i18n;
use amethystora_ui::{gdk, glib, gtk, t};

use backend::{Entry, Follower, Journal, Overview, Request, Source, Sources};
use list::{Matcher, SlotRow};

pub fn main() -> glib::ExitCode {
    page::mark_start();
    app::run(
        Spec {
            name: "logs",
            id: "org.amethystora.Logs",
            css: include_str!("logs.css"),
            width: 1240,
            height: 840,
            min_width: 760,
            min_height: 540,
        },
        Logs::new,
    )
}

/// The ranges that end now, so that new entries belong in them
const LIVE_RANGES: [&str; 5] = ["boot", "hour", "day", "week", "all"];
/// A live view keeps this many entries, and forgets the oldest
const LIVE_KEPT: usize = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageKind {
    Logs,
    /// The apps and services
    Sources,
}

/// The moment after an export: its tone and its sentence
#[derive(Debug, Clone)]
struct Notice {
    tone: &'static str,
    text: String,
}

/// What the page keeps between drawings, as logs.js's `state`
struct State {
    page: PageKind,
    category: String,
    /// What the view is narrowed to, or None for everything in the view
    source: Option<Source>,
    range: String,
    level: i32,
    search: String,
    regex: bool,
    entries: Vec<Rc<Entry>>,
    by_cursor: HashMap<Rc<str>, Rc<Entry>>,
    more: bool,
    loading: bool,
    loading_more: bool,
    error: String,
    command: String,
    /// The cursor of the entry open beside the list
    selected: Rc<str>,
    live: bool,
    /// Entries that came in live while the list was scrolled away from the top
    fresh: usize,
    /// Live entries that came in while the list was being read
    buffered: Vec<Rc<Entry>>,
    overview: Option<Overview>,
    /// When the overview was last read
    overview_read: f64,
    sources: Option<Sources>,
    source_filter: String,
    notice: Option<Notice>,
    /// Each read of the list counts up, so that an answer to an older one is dropped
    request: u64,
    /// Whether an entry can be handed to the AI agent: the agentic features are on for this account
    agent: bool,
    /// The window has been opened once, with its first view
    started: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            page: PageKind::Logs,
            category: "important".into(),
            source: None,
            range: "boot".into(),
            level: 7,
            search: String::new(),
            regex: false,
            entries: Vec::new(),
            by_cursor: HashMap::new(),
            more: false,
            loading: false,
            loading_more: false,
            error: String::new(),
            command: String::new(),
            selected: "".into(),
            live: false,
            fresh: 0,
            buffered: Vec::new(),
            overview: None,
            overview_read: 0.0,
            sources: None,
            source_filter: String::new(),
            notice: None,
            request: 0,
            agent: false,
            started: false,
        }
    }
}

pub struct Logs {
    me: Weak<Logs>,
    shell: Rc<Shell>,
    journal: Rc<Journal>,
    state: RefCell<State>,
    follower: RefCell<Option<Follower>>,
    /// Each start of following counts up, so that one that ends late is not taken for the current one
    following: Cell<u64>,
    /// What a row needs to be drawn, kept apart from the state so that the list can be drawn at any time
    matcher: RefCell<Option<Rc<Matcher>>>,
    mark: RefCell<gdk::RGBA>,
    /// The rows the list view made, which are drawn again in place when the open entry changes
    rows: RefCell<Vec<glib::WeakRef<SlotRow>>>,
    parts: page::Parts,
    live: RefCell<page::Live>,
    notice_timer: RefCell<Option<glib::SourceId>>,
    search_timer: RefCell<Option<glib::SourceId>>,
    filter_pause: amethystora_ui::ui::Pause,
    /// The window's height, for what the page sized in vh
    vh: Cell<i32>,
    /// Where the list was scrolled to, and how tall it was then, to keep its place when it is resized
    list_top: Cell<f64>,
    list_page: Cell<f64>,
    list_restoring: Cell<bool>,
}

impl App for Logs {
    fn open(&self, args: &[String]) {
        let view = backend::view_argument(args);
        let started = self.state.borrow().started;
        if !started {
            self.state.borrow_mut().started = true;
            self.start(&view);
        } else if !view.is_empty() {
            self.open_view(&view);
        }
    }

    // Back from elsewhere: the counts in the sidebar may have moved on
    fn focus(&self) {
        let stale = {
            let state = self.state.borrow();
            state.overview.is_some() && i18n::now() - state.overview_read > 30.0
        };
        if stale {
            self.state.borrow_mut().overview_read = i18n::now();
            let this = self.rc();
            glib::spawn_future_local(async move { this.refresh_overview().await });
        }
    }

    fn palette(&self) {
        self.remark();
    }

    // Ctrl+= and Ctrl+-: the widths the page set in code follow the stylesheet's
    fn zoomed(&self) {
        self.relayout();
    }

    // Ctrl+R and F5
    fn reload(&self) {
        if self.state.borrow().page == PageKind::Sources {
            self.load_sources();
        } else {
            self.load();
        }
        let this = self.rc();
        glib::spawn_future_local(async move { this.refresh_overview().await });
    }
}

impl Logs {
    fn new(shell: Rc<Shell>) -> Rc<Logs> {
        let this = Rc::new_cyclic(|me: &Weak<Logs>| Logs {
            me: me.clone(),
            shell: shell.clone(),
            journal: Rc::new(Journal::default()),
            state: RefCell::new(State::default()),
            follower: RefCell::new(None),
            following: Cell::new(0),
            matcher: RefCell::new(None),
            mark: RefCell::new(gdk::RGBA::new(0.91, 0.75, 0.42, 0.34)),
            rows: RefCell::new(Vec::new()),
            parts: page::Parts::new(me),
            live: RefCell::new(page::Live::default()),
            notice_timer: RefCell::new(None),
            search_timer: RefCell::new(None),
            filter_pause: amethystora_ui::ui::Pause::default(),
            vh: Cell::new(840),
            list_top: Cell::new(0.0),
            list_page: Cell::new(0.0),
            list_restoring: Cell::new(false),
        });
        shell.window.set_title(Some(&t!("Amethystora Logs")));
        shell.window.set_content(Some(&this.parts.root));
        // A narrow window: the entry opens over the list (max-width: 1100px); narrower, the sidebar too
        shell.narrow(1100.0, "narrow");
        shell.narrow(900.0, "narrower");
        let weak = Rc::downgrade(&this);
        shell.window.connect_notify_local(Some("css-classes"), move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.relayout();
            }
        });
        this.remark();
        this.keys();
        let weak = Rc::downgrade(&this);
        shell.window.connect_close_request(move |_| {
            if let Some(this) = weak.upgrade() {
                this.journal.stop_reading();
                this.follower.take();
            }
            glib::Propagation::Proceed
        });
        this
    }

    /// This window, from a method that only has &self
    fn rc(&self) -> Rc<Logs> {
        self.me.upgrade().expect("the window is open")
    }

    /// The colour the search's marks are drawn in: the theme's yellow at 34%, as <mark> was
    fn remark(&self) {
        let warn = self
            .shell
            .palette()
            .and_then(|palette| palette.colors.get("color3").cloned())
            .unwrap_or_else(|| if self.shell.light() { "#8a6100".into() } else { "#e8c06a".into() });
        let mut color = gdk::RGBA::parse(&warn).unwrap_or(gdk::RGBA::new(0.91, 0.75, 0.42, 1.0));
        color.set_alpha(0.34);
        self.mark.replace(color);
        let matcher = self.matcher.borrow().clone();
        for row in self.rows.borrow().iter().filter_map(|row| row.upgrade()) {
            row.remark(matcher.as_deref(), &color);
        }
        self.redraw_detail_marks();
    }

    fn set_matcher(&self) {
        let matcher = {
            let state = self.state.borrow();
            Matcher::new(&state.search, state.regex).map(Rc::new)
        };
        self.matcher.replace(matcher);
    }

    // --- Reading the journal -----------------------------------------------------------------------

    /// What is shown, as backend.rs takes it
    fn request(&self) -> Request {
        let state = self.state.borrow();
        Request {
            category: state.category.clone(),
            source: state.source.clone(),
            range: state.range.clone(),
            level: state.level,
            search: state.search.clone(),
            regex: state.regex,
        }
    }

    fn can_follow(&self) -> bool {
        LIVE_RANGES.contains(&self.state.borrow().range.as_str())
    }

    fn load(&self) {
        let id = {
            let mut state = self.state.borrow_mut();
            state.request += 1;
            state.loading = true;
            state.error.clear();
            state.more = false;
            state.fresh = 0;
            state.buffered.clear();
            state.request
        };
        self.set_matcher();
        self.render_list();
        if self.state.borrow().live {
            if self.can_follow() {
                self.start_following();
            } else {
                self.set_live(false);
            }
        }
        let this = self.rc();
        let request = self.request();
        glib::spawn_future_local(async move {
            let asked = page::since_start();
            let result = this.journal.query(&request, None).await;
            glib::g_debug!("amethystora-logs", "read {} entries from {asked} ms to {} ms after start", result.entries.len(), page::since_start());
            if id != this.state.borrow().request || result.stopped {
                return;
            }
            let close_detail = {
                let mut state = this.state.borrow_mut();
                state.loading = false;
                state.by_cursor.clear();
                let seen: std::collections::HashSet<&str> = result.entries.iter().map(|entry| &*entry.cursor).collect();
                let newest = result.entries.first().map(|entry| entry.time).unwrap_or(0);
                // What came in live while this was read, and is newer than what it found
                let buffered = std::mem::take(&mut state.buffered);
                let mut entries: Vec<Rc<Entry>> =
                    buffered.into_iter().filter(|entry| !seen.contains(&*entry.cursor) && entry.time >= newest).rev().collect();
                drop(seen);
                entries.extend(result.entries.iter().cloned());
                state.entries = entries;
                let remembered: HashMap<Rc<str>, Rc<Entry>> = state.entries.iter().map(|entry| (entry.cursor.clone(), entry.clone())).collect();
                state.by_cursor = remembered;
                state.more = result.more;
                state.error = result.error.clone();
                if !result.command.is_empty() {
                    state.command = result.command.clone();
                }
                let gone = !state.selected.is_empty() && !state.by_cursor.contains_key(&state.selected);
                if gone {
                    state.selected = "".into();
                }
                gone
            };
            if close_detail {
                this.render_detail();
            }
            this.render_hint();
            // The command an administrator can read this view with is the one just read
            this.render_access();
            this.render_list();
            this.glance("list shown");
        });
    }

    /// For timing the window by: G_MESSAGES_DEBUG=amethystora-logs prints when the list was drawn
    fn glance(&self, what: &str) {
        if std::env::var_os("G_MESSAGES_DEBUG").is_none() {
            return;
        }
        let count = self.state.borrow().entries.len();
        let what = what.to_string();
        // Once the next frame, the first with the list in it, has been painted
        self.shell.window.add_tick_callback(move |_, clock| {
            let handler: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
            let (once, what) = (handler.clone(), what.clone());
            let id = clock.connect_after_paint(move |clock| {
                glib::g_debug!("amethystora-logs", "{what}: {count} entries, {} ms after start", page::since_start());
                if let Some(id) = once.take() {
                    clock.disconnect(id);
                }
            });
            handler.replace(Some(id));
            glib::ControlFlow::Break
        });
    }

    fn load_older(&self) {
        let (last, id) = {
            let state = self.state.borrow();
            match state.entries.last() {
                Some(last) if !state.loading_more && !state.loading => (last.cursor.clone(), state.request),
                _ => return,
            }
        };
        self.state.borrow_mut().loading_more = true;
        self.render_foot();
        let this = self.rc();
        let request = self.request();
        glib::spawn_future_local(async move {
            let result = this.journal.query(&request, Some(&last)).await;
            this.state.borrow_mut().loading_more = false;
            if id != this.state.borrow().request || result.stopped {
                this.render_foot();
                return;
            }
            let (older, previous) = {
                let mut state = this.state.borrow_mut();
                let older: Vec<Rc<Entry>> = result.entries.iter().filter(|entry| !state.by_cursor.contains_key(&entry.cursor)).cloned().collect();
                for entry in &older {
                    state.by_cursor.insert(entry.cursor.clone(), entry.clone());
                }
                state.more = result.more && !older.is_empty();
                let previous = state.entries.last().map(|entry| entry.time);
                state.entries.extend(older.iter().cloned());
                if !result.error.is_empty() {
                    state.error = result.error.clone();
                }
                (older, previous)
            };
            if this.live.borrow().list_shown {
                this.parts.model.append(&older, previous);
            }
            this.render_foot();
            this.glance("older shown");
        });
    }

    fn start_following(&self) {
        self.follower.take();
        self.following.set(self.following.get() + 1);
        let serial = self.following.get();
        let this = self.rc();
        let request = self.request();
        glib::spawn_future_local(async move {
            let weak = Rc::downgrade(&this);
            let ended = Rc::downgrade(&this);
            let result = this
                .journal
                .follow(
                    &request,
                    move |list| {
                        if let Some(this) = weak.upgrade() {
                            this.arrived(list);
                        }
                    },
                    move || {
                        let Some(this) = ended.upgrade() else { return };
                        if this.following.get() == serial {
                            this.follower.take();
                            if this.state.borrow().live {
                                this.set_live(false);
                            }
                        }
                    },
                )
                .await;
            match result {
                // Stopped, or another started, while it was being set up
                Ok(follower) if this.following.get() == serial && this.state.borrow().live => {
                    this.follower.replace(Some(follower));
                }
                Ok(_) => {}
                Err(_) => {
                    if this.following.get() == serial {
                        this.set_live(false);
                    }
                }
            }
        });
    }

    fn stop_following(&self) {
        self.following.set(self.following.get() + 1);
        self.follower.take();
    }

    fn set_live(&self, on: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.live = on;
            state.fresh = 0;
        }
        if on {
            self.start_following();
        } else {
            self.stop_following();
        }
        self.render_actions();
        self.render_fresh();
    }

    /// Entries written since, newest last, as backend.rs hands them over
    fn arrived(&self, list: Vec<Rc<Entry>>) {
        let (fresh, was_empty, trimmed) = {
            let mut state = self.state.borrow_mut();
            if !state.live {
                return;
            }
            if state.loading {
                state.buffered.extend(list);
                return;
            }
            let fresh: Vec<Rc<Entry>> = list.into_iter().filter(|entry| !state.by_cursor.contains_key(&entry.cursor)).rev().collect();
            if fresh.is_empty() {
                return;
            }
            for entry in &fresh {
                state.by_cursor.insert(entry.cursor.clone(), entry.clone());
            }
            let was_empty = state.entries.is_empty();
            let mut entries = fresh.clone();
            entries.append(&mut state.entries);
            state.entries = entries;
            let mut trimmed = false;
            if state.entries.len() > LIVE_KEPT {
                for entry in state.entries.split_off(LIVE_KEPT) {
                    state.by_cursor.remove(&entry.cursor);
                }
                state.more = true;
                trimmed = true;
            }
            (fresh, was_empty, trimmed)
        };
        if was_empty || !self.live.borrow().list_shown {
            self.render_list();
            return;
        }
        let adjustment = self.parts.scroller.vadjustment();
        let away = adjustment.value() > 8.0;
        self.parts.model.prepend(&fresh);
        if trimmed {
            // Rows beyond what is kept go, with a heading left without rows under it
            self.parts.model.keep(LIVE_KEPT);
        }
        if away {
            // Scrolled away to read something: it stays where it is
            self.state.borrow_mut().fresh += fresh.len();
            self.render_fresh();
        } else {
            self.parts.list_view.scroll_to(0, gtk::ListScrollFlags::NONE, None);
            adjustment.set_value(0.0);
        }
        self.render_count();
    }

    // --- One entry ---------------------------------------------------------------------------------

    fn close_entry(&self) {
        self.state.borrow_mut().selected = "".into();
        self.mark_selected();
        self.render_detail();
    }

    fn mark_selected(&self) {
        let selected = self.state.borrow().selected.clone();
        let mut rows = self.rows.borrow_mut();
        rows.retain(|row| row.upgrade().is_some());
        for row in rows.iter().filter_map(|row| row.upgrade()) {
            row.set_selected(&selected);
        }
    }

    fn open_entry(&self, cursor: &str, focus: bool) {
        let cursor = {
            let state = self.state.borrow();
            match state.by_cursor.get_key_value(cursor) {
                Some((key, _)) => key.clone(),
                None => return,
            }
        };
        self.state.borrow_mut().selected = cursor.clone();
        self.mark_selected();
        if let Some(position) = self.parts.model.position(&cursor) {
            self.reveal(position, &cursor, focus);
        }
        self.render_detail();
    }

    /// The next entry up or down the list from the open one
    fn step(&self, by: i64) {
        let (next, more) = {
            let state = self.state.borrow();
            let index = state.entries.iter().position(|entry| entry.cursor == state.selected);
            let at = match index {
                None => 0,
                Some(index) => index as i64 + by,
            };
            let next = (at >= 0).then(|| state.entries.get(at as usize)).flatten().map(|entry| entry.cursor.clone());
            (next, state.more)
        };
        match next {
            Some(cursor) => self.open_entry(&cursor, true),
            None if by > 0 && more => self.load_older(),
            None => {}
        }
    }

    // --- What changes what is read -----------------------------------------------------------------

    /// Anything that changes what is read draws the view again and reads it
    fn show(&self, change: impl FnOnce(&mut State), keep_selected: bool) {
        {
            let mut state = self.state.borrow_mut();
            let selected = state.selected.clone();
            change(&mut state);
            state.page = PageKind::Logs;
            if !keep_selected {
                state.selected = "".into();
            } else if state.selected.is_empty() {
                state.selected = selected;
            }
        }
        self.render();
        self.load();
    }

    fn open_category(&self, category: &str) {
        let category = category.to_string();
        self.show(
            move |state| {
                state.category = category;
                state.source = None;
                if state.range.starts_with("around:") {
                    state.range = "boot".into();
                }
            },
            false,
        );
    }

    fn set_source(&self, source: Option<Source>) {
        self.show(
            move |state| {
                if source.is_some() {
                    state.category = "all".into();
                }
                state.source = source;
            },
            false,
        );
    }

    fn set_range(&self, range: &str) {
        let range = range.to_string();
        self.show(move |state| state.range = range, true);
    }

    fn set_level(&self, level: i32) {
        self.show(move |state| state.level = level, true);
    }

    /// Everything else on the machine in the two minutes around one entry, with that entry still open
    fn around(&self, entry: &Entry) {
        let (time, cursor) = (entry.time, entry.cursor.clone());
        self.show(
            move |state| {
                state.category = "all".into();
                state.source = None;
                state.search.clear();
                state.level = 7;
                state.range = format!("around:{}", time.div_euclid(1000));
                state.selected = cursor;
                state.live = false;
            },
            true,
        );
        self.stop_following();
    }

    fn export(&self) {
        let this = self.rc();
        glib::spawn_future_local(async move {
            let request = this.request();
            let window = this.shell.gtk_window();
            match this.journal.export(&request, &window).await {
                backend::Exported::Saved { file, folder } => {
                    // A folder reads left to right in any language
                    let text = page::sentence(amethystora_ui::t_parts!("Saved as {file} in {folder}.", file = &file, folder = &folder), |name| {
                        if name == "file" { file.clone() } else { format!("\u{2066}{folder}\u{2069}") }
                    });
                    this.set_notice(Some(Notice { tone: "ok", text }));
                }
                // ...and what went wrong in whichever way its words run
                backend::Exported::Failed(error) => {
                    let text = page::sentence(amethystora_ui::t_parts!("Nothing was saved: {error}", error = &error), |_| format!("\u{2068}{error}\u{2069}"));
                    this.set_notice(Some(Notice { tone: "check", text }));
                }
                backend::Exported::Canceled => {}
            }
        });
    }

    fn set_notice(&self, notice: Option<Notice>) {
        if let Some(timer) = self.notice_timer.take() {
            timer.remove();
        }
        let shown = notice.is_some();
        self.state.borrow_mut().notice = notice;
        self.render_notice();
        if shown {
            let weak = self.me.clone();
            let timer = glib::timeout_add_local_once(Duration::from_secs(8), move || {
                if let Some(this) = weak.upgrade() {
                    this.notice_timer.take();
                    this.set_notice(None);
                }
            });
            self.notice_timer.replace(Some(timer));
        }
    }

    async fn refresh_overview(&self) {
        let overview = Journal::overview().await;
        self.state.borrow_mut().overview = Some(overview);
        self.render_nav();
        // The list of boots and who may read what are part of the view
        if self.state.borrow().page == PageKind::Logs {
            self.render_range();
            self.render_access();
        }
    }

    fn load_sources(&self) {
        let this = self.rc();
        glib::spawn_future_local(async move {
            let sources = this.journal.sources().await;
            this.state.borrow_mut().sources = Some(sources);
            if this.state.borrow().page == PageKind::Sources {
                this.render();
            }
        });
    }

    fn open_view(&self, name: &str) {
        if name == "sources" {
            self.state.borrow_mut().page = PageKind::Sources;
            self.render();
            self.load_sources();
        } else if backend::CATEGORIES.contains(&name) {
            self.open_category(name);
        }
    }

    fn start(&self, view: &str) {
        self.glance("window drawn");
        let this = self.rc();
        // Whether an entry can be handed to the agent, which only an open entry shows
        glib::spawn_future_local(async move {
            let on = backend::agent_on().await;
            this.state.borrow_mut().agent = on;
            if !this.state.borrow().selected.is_empty() {
                this.render_detail();
            }
        });
        if view == "sources" {
            self.state.borrow_mut().page = PageKind::Sources;
            self.render();
            self.load_sources();
        } else {
            self.state.borrow_mut().category = if view.is_empty() { "important".into() } else { view.to_string() };
            self.render();
            self.load();
        }
        let this = self.rc();
        glib::spawn_future_local(async move {
            this.refresh_overview().await;
            this.state.borrow_mut().overview_read = i18n::now();
        });
    }

    // --- Keys --------------------------------------------------------------------------------------

    /// Ctrl+F or / searches, Escape closes the entry, the arrows and j and k go through the list; Ctrl+R, F5
    /// and the zoom are the window's
    fn keys(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.me.clone();
        keys.connect_key_pressed(move |controller, key, _, modifiers| {
            let Some(this) = weak.upgrade() else { return glib::Propagation::Proceed };
            let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
            let focus = controller.widget().and_then(|window| window.root()).and_then(|root| root.focus());
            let typing = focus.as_ref().is_some_and(|focus| {
                focus.is::<gtk::Text>() || focus.ancestor(gtk::Popover::static_type()).is_some() || focus.ancestor(gtk::MenuButton::static_type()).is_some()
            });
            if (ctrl && matches!(key, gdk::Key::f | gdk::Key::F)) || (key == gdk::Key::slash && !ctrl && !typing) {
                this.focus_search();
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::Escape {
                // The search empties itself first, as its own key handler did
                let in_search = focus.as_ref().and_then(|f| f.downcast_ref::<gtk::Text>().map(|text| !text.text().is_empty())).unwrap_or(false);
                if !in_search && !this.state.borrow().selected.is_empty() {
                    this.close_entry();
                    return glib::Propagation::Stop;
                }
                return glib::Propagation::Proceed;
            }
            let logs = this.state.borrow().page == PageKind::Logs;
            // Home, End and the page keys scroll the list, as they scrolled the page's
            if logs && !typing && this.live.borrow().list_shown {
                let adjustment = this.parts.scroller.vadjustment();
                let (end, page) = (adjustment.upper() - adjustment.page_size(), adjustment.page_size());
                let to = match key {
                    gdk::Key::Home => Some(0.0),
                    gdk::Key::End => Some(end),
                    gdk::Key::Page_Down => Some(adjustment.value() + page * 0.875),
                    gdk::Key::Page_Up => Some(adjustment.value() - page * 0.875),
                    _ => None,
                };
                if let Some(to) = to {
                    adjustment.set_value(to.clamp(0.0, end.max(0.0)));
                    return glib::Propagation::Stop;
                }
            }
            if logs && !typing && (key == gdk::Key::Down || (key == gdk::Key::j && !ctrl)) {
                this.step(1);
                return glib::Propagation::Stop;
            }
            if logs && !typing && (key == gdk::Key::Up || (key == gdk::Key::k && !ctrl)) {
                this.step(-1);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.shell.window.add_controller(keys);
    }
}
