//! The page of Amethystora Notes, as notes.js drew it. The notes and tasks belong to backend.rs, which keeps
//! them on disk, sealed when they are encrypted; the page changes them in place and hands each change over
//! as it is made, as notes.js sent it to main.js. Everything shown is built as widgets with text in them,
//! except a note's Markdown, which markdown.rs renders with any HTML in it shown as text rather than run.

mod capture;
mod gate;
mod notes;
mod settings;
mod sidebar;
mod tasks;
pub mod widgets;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::Duration;

use serde_json::{json, Map, Value};

use amethystora_ui::app::{App, Shell};
use amethystora_ui::adw::prelude::*;
use amethystora_ui::{frame, gdk, glib, gtk, store as saved, t, ui};

use crate::backend::{self, Backend, Passkey, Shared};
use crate::data::{self, Fields, Kind, Store};
use crate::layout::Exact;
use crate::session::{LockWatch, Notifier};
use crate::search::{self, Search};
use crate::today::{self, Today};
use widgets::toast;

pub use tasks::task_order;

pub const COLORS: [&str; 9] = ["violet", "blue", "teal", "green", "yellow", "orange", "red", "pink", "slate"];
/// A version of a note is kept when it is changed after this long untouched
const REVISION_GAP: f64 = 10.0 * 60000.0;
const REVISIONS_KEPT: usize = 30;

/// What the colour menu calls each of the colours
pub fn color_label(color: &str) -> String {
    match color {
        "violet" => t!("Violet"),
        "blue" => t!("Blue"),
        "teal" => t!("Teal"),
        "green" => t!("Green"),
        "yellow" => t!("Yellow"),
        "orange" => t!("Orange"),
        "red" => t!("Red"),
        "pink" => t!("Pink"),
        _ => t!("Slate"),
    }
}

/// A view of the notes or the tasks: { type, id }
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub kind: String,
    pub id: Option<String>,
}

impl View {
    pub fn of(kind: &str) -> View {
        View { kind: kind.into(), id: None }
    }

    pub fn with(kind: &str, id: &str) -> View {
        View { kind: kind.into(), id: Some(id.into()) }
    }

    fn to_json(&self) -> Value {
        match &self.id {
            Some(id) => json!({ "type": self.kind, "id": id }),
            None => json!({ "type": self.kind }),
        }
    }

    fn from_json(value: &Value) -> Option<View> {
        let kind = value.get("type")?.as_str()?.to_string();
        Some(View { kind, id: value.get("id").and_then(Value::as_str).map(str::to_string) })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Notes,
    Tasks,
    Settings,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Notes => "notes",
            Mode::Tasks => "tasks",
            Mode::Settings => "settings",
        }
    }
}

/// The page's state, as notes.js's `ui`
pub struct Ui {
    pub status: &'static str,
    pub encrypted: bool,
    pub passkeys: Vec<Passkey>,
    pub dir: String,
    pub mode: Mode,
    pub notes_view: View,
    pub note: Option<String>,
    pub tasks_view: View,
    pub task: Option<String>,
    pub query: String,
    pub collapsed: Vec<String>,
    pub show_done: bool,
}

/// A note as it was last handed over, to tell when a change starts a new version
#[derive(Clone)]
struct Sent {
    title: Value,
    body: Value,
    updated: f64,
}

pub struct Notes {
    pub me: Weak<Notes>,
    pub shell: Rc<Shell>,
    pub backend: Shared,
    pub ui: RefCell<Ui>,
    // What has changed and not been handed over yet, by kind and id
    pending: RefCell<Vec<(Kind, String)>>,
    pending_timer: RefCell<Option<glib::SourceId>>,
    sent: RefCell<HashMap<String, Sent>>,
    // The frame
    pub layout: gtk::Box,
    pub sidebar: Exact,
    pub main: gtk::Box,
    pub floating: gtk::WindowControls,
    pub mode_buttons: [gtk::Button; 2],
    pub today_count: gtk::Label,
    pub search: gtk::Entry,
    pub nav: gtk::Box,
    pub nav_scroll: gtk::ScrolledWindow,
    pub settings_button: gtk::Button,
    pub lock_button: gtk::Button,
    // The views, kept across renders so that typing is never interrupted
    pub notes_parts: RefCell<Option<notes::Parts>>,
    pub editor: RefCell<Option<notes::Editor>>,
    pub tasks_parts: RefCell<Option<tasks::Parts>>,
    pub detail_for: RefCell<Option<Option<String>>>,
    pub gate_gem: RefCell<Option<amethystora_ui::gem::Gem>>,
    // The window
    /// Closed, and running in the background for the reminders still to come
    pub hidden: Cell<bool>,
    closing: Cell<bool>,
    shown_day: RefCell<String>,
    notifier: Notifier,
    _lock_watch: RefCell<Option<LockWatch>>,
    first_open: Cell<bool>,
    /// Today's tasks on the session bus for the shell, while the notes are open (today.rs)
    today: RefCell<Option<Rc<Today>>>,
    /// The titles that answer the desktop's search, while the notes are open (search.rs)
    bus_search: RefCell<Option<Rc<Search>>>,
    /// The quick note's window while it is open (capture.rs)
    pub capture: RefCell<Option<capture::Capture>>,
}

/// `amethystora-notes tasks` opens the tasks
fn page_argument(args: &[String]) -> Option<Mode> {
    match args.iter().find(|arg| !arg.starts_with('-')).map(String::as_str) {
        Some("tasks") => Some(Mode::Tasks),
        Some("notes") => Some(Mode::Notes),
        _ => None,
    }
}

/// `amethystora-notes capture` opens the quick note, and only that
fn is_capture(args: &[String]) -> bool {
    args.iter().find(|arg| !arg.starts_with('-')).map(String::as_str) == Some("capture")
}

/// `amethystora-notes remind`: the search's reminder, a task read from the standard input (page/capture.rs)
fn is_remind(args: &[String]) -> bool {
    args.iter().find(|arg| !arg.starts_with('-')).map(String::as_str) == Some("remind")
}

impl App for Notes {
    fn presents(&self, args: &[String]) -> bool {
        !is_capture(args) && !is_remind(args)
    }

    fn command_line(&self, line: &gtk::gio::ApplicationCommandLine) {
        let args: Vec<String> = line.arguments().iter().skip(1).map(|arg| arg.to_string_lossy().into_owned()).collect();
        if is_remind(&args) {
            self.read_remind(line);
        }
    }

    fn open(&self, args: &[String]) {
        if is_remind(args) {
            // As for the quick note: at start the window is made but not shown; the line on stdin does the rest
            if self.first_open.replace(false) {
                self.hidden.set(true);
                self.render();
            }
            return;
        }
        if is_capture(args) {
            // At start, the window is made but not shown, as in the background
            if self.first_open.replace(false) {
                self.hidden.set(true);
                self.render();
            }
            self.open_capture();
            return;
        }
        let page = page_argument(args);
        if self.first_open.replace(false) {
            if let Some(page) = page {
                self.ui.borrow_mut().mode = page;
            }
            self.render();
            return;
        }
        // Opened again: from the background, or with a page to show
        if self.hidden.replace(false) {
            self.render();
        }
        // Read first: a borrow in the `if let` itself would last through set_mode, which writes to the same state
        let status = self.ui.borrow().status;
        if let (Some(page), "open") = (page, status) {
            self.set_mode(page);
        }
    }

    fn focus(&self) {
        self.backend.borrow_mut().last_active = data::now_ms();
    }

    fn palette(&self) {
        self.redraw_markdown();
    }

    fn zoomed(&self) {
        self.redraw_markdown();
    }

    fn reload(&self) {}
}

impl Notes {
    pub fn new(shell: Rc<Shell>) -> Rc<Notes> {
        let backend: Shared = Rc::new(RefCell::new(Backend::new(backend::Paths::standard())));
        let start = backend::start(&backend);
        let search = gtk::Entry::builder().css_classes(["plain"]).hexpand(true).build();
        let nav = ui::vbox(1, &["nav"]);
        let nav_scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&nav).css_classes(["nav-scroll"]).build();
        let main = ui::vbox(0, &["main"]);
        main.set_hexpand(true);
        main.set_vexpand(true);
        let sidebar_box = ui::vbox(16, &["sidebar"]);
        let mode_button = || gtk::Button::builder().css_classes(["plain", "mode"]).hexpand(true).build();
        let today_count = ui::line("", &["count"]);
        today_count.set_xalign(0.5);
        today_count.set_valign(gtk::Align::Center);
        let settings_button = gtk::Button::builder().css_classes(["plain", "foot-button"]).build();
        let lock_button = gtk::Button::builder().css_classes(["plain", "foot-button"]).build();
        let this = Rc::new_cyclic(|me| Notes {
            me: me.clone(),
            shell: shell.clone(),
            backend: backend.clone(),
            ui: RefCell::new(Ui {
                status: "loading",
                encrypted: false,
                passkeys: Vec::new(),
                dir: String::new(),
                mode: Mode::Notes,
                notes_view: View::of("all"),
                note: None,
                tasks_view: View::of("today"),
                task: None,
                query: String::new(),
                collapsed: Vec::new(),
                show_done: false,
            }),
            pending: RefCell::new(Vec::new()),
            pending_timer: RefCell::new(None),
            sent: RefCell::new(HashMap::new()),
            layout: ui::hbox(0, &["layout"]),
            sidebar: Exact::new(&sidebar_box),
            main,
            floating: frame::controls(),
            mode_buttons: [mode_button(), mode_button()],
            today_count,
            search,
            nav,
            nav_scroll,
            settings_button,
            lock_button,
            notes_parts: RefCell::new(None),
            editor: RefCell::new(None),
            tasks_parts: RefCell::new(None),
            detail_for: RefCell::new(None),
            gate_gem: RefCell::new(None),
            hidden: Cell::new(false),
            closing: Cell::new(false),
            shown_day: RefCell::new(crate::dates::today()),
            notifier: Notifier::new(),
            _lock_watch: RefCell::new(None),
            first_open: Cell::new(true),
            today: RefCell::new(None),
            bus_search: RefCell::new(None),
            capture: RefCell::new(None),
        });
        shell.window.set_title(Some(&t!("Notes")));
        shell.narrow(1100.0, "narrow");
        let (weak, was) = (this.me.clone(), Cell::new(false));
        shell.window.connect_notify_local(Some("css-classes"), move |window, _| {
            let narrow = window.has_css_class("narrow");
            if was.replace(narrow) != narrow {
                if let Some(this) = weak.upgrade() {
                    this.narrowed();
                }
            }
        });
        let root = this.frame(&sidebar_box);
        shell.window.set_content(Some(&root));
        this.hooks();
        this.keys();
        this.timers();
        this.window_closing();
        let weak = this.me.clone();
        this._lock_watch.replace(Some(LockWatch::new(Rc::new(move || {
            if let Some(this) = weak.upgrade() {
                this.lock_now();
            }
        }))));
        this.recall();
        if let Err(error) = start {
            // As main.js's error box: the notes cannot be opened, and nothing is changed
            let shell = shell.clone();
            glib::idle_add_local_once(move || {
                let alert = gtk::AlertDialog::builder().modal(true).message(t!("Notes")).detail(error).build();
                let app = shell.app.clone();
                alert.choose(Some(&shell.window), gtk::gio::Cancellable::NONE, move |_| app.quit());
            });
            return this;
        }
        this.apply_state(None);
        this.start_today();
        this
    }

    // --- Today's tasks for the shell ---------------------------------------------------------------------

    fn start_today(&self) {
        let Some(connection) = self.shell.app.dbus_connection() else { return };
        let weak = self.me.clone();
        let answer: today::Answer = Rc::new(move || {
            let this = weak.upgrade()?;
            let day = crate::dates::today();
            let backend = this.backend.borrow();
            let (total, titles) = today::today_titles(backend.store.as_ref()?, &day);
            Some((day, total, titles))
        });
        self.today.replace(Some(Today::new(connection.clone(), today::SHELL_EXE, Some(today::SHELL_NAME), answer)));
        // The desktop's search, on the same terms: titles only, to the shell only, while open
        let weak = self.me.clone();
        let find: search::Answer = Rc::new(move |words| {
            let this = weak.upgrade()?;
            let backend = this.backend.borrow();
            Some(search::find(backend.store.as_ref()?, words))
        });
        let weak = self.me.clone();
        let show: search::Show = Rc::new(move |kind, id| weak.upgrade().is_some_and(|this| this.show_found(kind, id)));
        self.bus_search.replace(Some(Search::new(connection, today::SHELL_EXE, Some(today::SHELL_NAME), find, show)));
        self.sync_today();
        // Off the bus before Notes leaves it
        let weak = self.me.clone();
        self.shell.app.connect_shutdown(move |_| {
            if let Some(this) = weak.upgrade() {
                if let Some(today) = this.today.borrow().as_ref() {
                    today.unregister();
                }
                if let Some(search) = this.bus_search.borrow().as_ref() {
                    search.unregister();
                }
            }
        });
    }

    /// On the bus while the notes are open, off it while they are locked
    pub fn sync_today(&self) {
        let open = self.backend.borrow().store.is_some();
        let today = self.today.borrow().clone();
        if let Some(today) = today {
            today.set_open(open);
        }
        let search = self.bus_search.borrow().clone();
        if let Some(search) = search {
            search.set_open(open);
        }
    }

    /// The desktop's search opened a note or a task: the window, on it, while the notes are open
    fn show_found(&self, kind: &str, id: &str) -> bool {
        let known_kind = if kind == "task" { Kind::Tasks } else { Kind::Notes };
        if self.ui.borrow().status != "open" || !self.backend.borrow().store.as_ref().is_some_and(|store| store.has(known_kind, id)) {
            return false;
        }
        if kind == "task" {
            self.show_task(id);
            return true;
        }
        if self.hidden.replace(false) {
            self.render();
        }
        self.shell.window.present();
        self.commit();
        {
            let mut ui = self.ui.borrow_mut();
            ui.mode = Mode::Notes;
            // Every note but those in the trash, where the search found it
            ui.notes_view = View::of("all");
            ui.note = Some(id.to_string());
        }
        self.clear_query();
        self.remember();
        self.render();
        true
    }

    fn today_changed(&self) {
        let today = self.today.borrow().clone();
        if let Some(today) = today {
            today.changed();
        }
    }

    pub fn rc(&self) -> Rc<Notes> {
        self.me.upgrade().expect("the window is open")
    }

    /// A callback that holds the page weakly
    pub fn on(&self, f: impl Fn(&Rc<Notes>) + 'static) -> impl Fn() + 'static {
        let weak = self.me.clone();
        move || {
            if let Some(this) = weak.upgrade() {
                f(&this);
            }
        }
    }

    /// An async step of the page, which holds it while it runs
    pub fn spawn(&self, work: impl AsyncFnOnce(Rc<Notes>) + 'static) {
        let this = self.rc();
        glib::spawn_future_local(async move { work(this).await });
    }

    // --- The frame ---------------------------------------------------------------------------------------

    fn frame(&self, sidebar_box: &gtk::Box) -> gtk::Widget {
        self.build_sidebar(sidebar_box);
        self.sidebar.set_hexpand(false);
        self.layout.append(&self.sidebar);
        let main_overlay = gtk::Overlay::new();
        main_overlay.set_child(Some(&self.main));
        main_overlay.set_hexpand(true);
        self.floating.add_css_class("floating");
        self.floating.set_halign(gtk::Align::End);
        self.floating.set_valign(gtk::Align::Start);
        main_overlay.add_overlay(&self.floating);
        self.layout.append(&main_overlay);
        let handle = gtk::WindowHandle::builder().child(&self.layout).build();
        let root = gtk::Overlay::new();
        root.set_child(Some(&handle));
        let toasts = ui::vbox(8, &["toasts"]);
        toasts.set_halign(gtk::Align::Center);
        toasts.set_valign(gtk::Align::End);
        toasts.set_can_target(true);
        root.add_overlay(&toasts);
        widgets::set_toasts(&toasts);
        widgets::set_layer(&root, &handle);
        root.upcast()
    }

    /// The window's own buttons: in the row of the page that ends at the window's corner, or over the page
    pub fn controls_for(&self, row: Option<&gtk::Box>) -> Option<gtk::WindowControls> {
        self.floating.set_visible(row.is_none());
        row.map(|row| {
            let controls = frame::controls();
            controls.set_valign(gtk::Align::Center);
            row.append(&controls);
            controls
        })
    }

    fn hooks(&self) {
        let mut backend = self.backend.borrow_mut();
        backend.hooks.problem = Some(Rc::new(|text| toast(&text, None)));
        let weak = self.me.clone();
        backend.hooks.locked = Some(Rc::new(move || {
            if let Some(this) = weak.upgrade() {
                this.on_locked();
            }
        }));
        let weak = self.me.clone();
        backend.hooks.opened = Some(Rc::new(move || {
            if let Some(this) = weak.upgrade() {
                this.sync_today();
            }
        }));
        let weak = self.me.clone();
        backend.hooks.flush_page = Some(Rc::new(move || {
            if let Some(this) = weak.upgrade() {
                this.commit();
            }
        }));
    }

    // --- Keys --------------------------------------------------------------------------------------------

    fn keys(&self) {
        // Using the window keeps it unlocked; the backend counts the time away from it
        let active = gtk::EventControllerKey::new();
        active.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.me.clone();
        active.connect_key_pressed(move |_, _, _, _| {
            if let Some(this) = weak.upgrade() {
                this.backend.borrow_mut().last_active = data::now_ms();
            }
            glib::Propagation::Proceed
        });
        self.shell.window.add_controller(active);
        let click = gtk::GestureClick::new();
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.me.clone();
        click.connect_pressed(move |_, _, _, _| {
            if let Some(this) = weak.upgrade() {
                this.backend.borrow_mut().last_active = data::now_ms();
            }
        });
        self.shell.window.add_controller(click);

        let keys = gtk::EventControllerKey::new();
        let weak = self.me.clone();
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(this) = weak.upgrade() else { return glib::Propagation::Proceed };
            this.key(key, modifiers)
        });
        self.shell.window.add_controller(keys);
    }

    fn key(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        let stop = glib::Propagation::Stop;
        if key == gdk::Key::Escape {
            if widgets::menu_open() {
                widgets::close_menu();
            } else if self.ui.borrow().mode == Mode::Tasks && self.ui.borrow().task.is_some() && !widgets::dialog_open() {
                self.ui.borrow_mut().task = None;
                self.render_tasks(false);
            }
            return glib::Propagation::Proceed;
        }
        if !modifiers.contains(gdk::ModifierType::CONTROL_MASK) && !modifiers.contains(gdk::ModifierType::META_MASK) {
            return glib::Propagation::Proceed;
        }
        if self.ui.borrow().status != "open" || widgets::dialog_open() {
            return glib::Propagation::Proceed;
        }
        let name = key.to_lower().name().map(|n| n.to_string()).unwrap_or_default();
        match name.as_str() {
            "n" => {
                if self.ui.borrow().mode == Mode::Tasks {
                    self.focus_quick(None);
                } else {
                    self.new_note();
                }
                stop
            }
            "f" => {
                self.search.grab_focus();
                self.search.select_region(0, -1);
                stop
            }
            "l" => {
                self.lock_now();
                stop
            }
            "1" | "2" => {
                self.set_mode(if name == "1" { Mode::Notes } else { Mode::Tasks });
                stop
            }
            "e" if self.ui.borrow().mode == Mode::Notes && self.editor.borrow().is_some() => {
                self.cycle_layout();
                stop
            }
            "comma" => {
                self.set_mode(Mode::Settings);
                stop
            }
            _ => glib::Propagation::Proceed,
        }
    }

    // --- Timers ------------------------------------------------------------------------------------------

    fn timers(&self) {
        let weak = self.me.clone();
        glib::timeout_add_local(Duration::from_secs(15), move || {
            let Some(this) = weak.upgrade() else { return glib::ControlFlow::Break };
            this.check_reminders();
            let (open, encrypted, minutes, idle) = {
                let backend = this.backend.borrow();
                let minutes = backend.store.as_ref().and_then(|s| s.setting("autoLock")).and_then(Value::as_f64).unwrap_or(10.0);
                (backend.store.is_some(), backend.encrypted(), minutes, data::now_ms() - backend.last_active)
            };
            if open && encrypted && minutes > 0.0 && idle >= minutes * 60000.0 {
                this.lock_now();
            }
            // In the background only for reminders: once they have all gone off, so does this
            let (upcoming, locking) = {
                let backend = this.backend.borrow();
                (backend.upcoming(), backend.locking)
            };
            if this.hidden.get() && !upcoming && !locking && this.capture.borrow().is_none() {
                this.quit();
            }
            glib::ControlFlow::Continue
        });
        // Past midnight, Today and the overdue ones move on
        let weak = self.me.clone();
        glib::timeout_add_local(Duration::from_secs(60), move || {
            let Some(this) = weak.upgrade() else { return glib::ControlFlow::Break };
            let today = crate::dates::today();
            if *this.shown_day.borrow() != today {
                this.shown_day.replace(today);
                this.today_changed();
                let showing = { let ui = this.ui.borrow(); ui.status == "open" && ui.mode == Mode::Tasks };
                if showing {
                    this.render_nav();
                    this.render_task_list();
                }
            }
            glib::ControlFlow::Continue
        });
    }

    fn check_reminders(&self) {
        let due = self.backend.borrow_mut().due_reminders();
        let now = data::now_ms();
        for reminder in due {
            let time = amethystora_ui::i18n::date((reminder.due / 1000.0).floor(), amethystora_ui::i18n::DateStyle::Time);
            let body = if reminder.due > now + 30000.0 { t!("Due at {time}", time = time.clone()) } else { t!("Due now, {time}", time = time) };
            let weak = self.me.clone();
            let id = reminder.id.clone();
            self.notifier.show(&reminder.title, &body, Rc::new(move || {
                if let Some(this) = weak.upgrade() {
                    this.show_task(&id);
                }
            }));
        }
    }

    /// A reminder was clicked: the window, on the task
    fn show_task(&self, id: &str) {
        if self.hidden.replace(false) {
            self.render();
        }
        self.shell.window.present();
        if self.ui.borrow().mode != Mode::Tasks && self.ui.borrow().status == "open" {
            self.set_mode(Mode::Tasks);
        }
        let known = self.backend.borrow().store.as_ref().is_some_and(|store| store.has(Kind::Tasks, id));
        if self.ui.borrow().status == "open" && known {
            {
                let mut ui = self.ui.borrow_mut();
                ui.mode = Mode::Tasks;
                ui.task = Some(id.to_string());
            }
            self.render();
        }
    }

    // --- Closing -----------------------------------------------------------------------------------------

    /// Closing the window locks the notes. It stays running without one while a reminder is to come,
    /// unless that has been turned off.
    fn window_closing(&self) {
        let weak = self.me.clone();
        self.shell.window.connect_close_request(move |window| {
            let Some(this) = weak.upgrade() else { return glib::Propagation::Proceed };
            if this.closing.replace(true) {
                return glib::Propagation::Stop;
            }
            this.commit();
            let background = this.backend.borrow().store.as_ref().is_none_or(|store| store.setting("background") != Some(&Value::Bool(false)));
            let window = window.clone();
            glib::spawn_future_local(async move {
                backend::flush(&this.backend).await;
                backend::lock(&this.backend).await;
                backend::flush(&this.backend).await;
                this.closing.set(false);
                if !background || !this.backend.borrow().upcoming() {
                    this.quit();
                } else {
                    window.set_visible(false);
                    this.hidden.set(true);
                }
            });
            glib::Propagation::Stop
        });
    }

    fn quit(&self) {
        let this = self.rc();
        glib::spawn_future_local(async move {
            backend::flush(&this.backend).await;
            this.shell.app.quit();
        });
    }

    // --- Saving ------------------------------------------------------------------------------------------

    /// An item changed in the store: handed over in 350 ms, or at once
    pub fn save(&self, kind: Kind, id: &str, now: bool) {
        if kind == Kind::Tasks {
            self.today_changed();
        }
        {
            let mut pending = self.pending.borrow_mut();
            if !pending.iter().any(|(k, i)| *k == kind && i == id) {
                pending.push((kind, id.to_string()));
            }
        }
        if let Some(timer) = self.pending_timer.take() {
            timer.remove();
        }
        let weak = self.me.clone();
        self.pending_timer.replace(Some(glib::timeout_add_local_once(Duration::from_millis(if now { 0 } else { 350 }), move || {
            if let Some(this) = weak.upgrade() {
                this.pending_timer.take();
                this.commit();
            }
        })));
    }

    /// Items taken away, and handed over at once
    pub fn remove(&self, kind: Kind, ids: &[String]) {
        if ids.is_empty() {
            return;
        }
        if kind == Kind::Tasks {
            self.today_changed();
        }
        if let Some(store) = self.backend.borrow_mut().store.as_mut() {
            for id in ids {
                store.kind_mut(kind).remove(id);
            }
        }
        self.pending.borrow_mut().retain(|(k, i)| !(*k == kind && ids.contains(i)));
        self.commit();
        Backend::changed(&self.backend, &[kind], true);
    }

    /// What has changed handed over, a version of each note that comes back after long enough kept
    pub fn commit(&self) {
        if let Some(timer) = self.pending_timer.take() {
            timer.remove();
        }
        let pending = std::mem::take(&mut *self.pending.borrow_mut());
        if pending.is_empty() {
            return;
        }
        let mut kinds: Vec<Kind> = Vec::new();
        let mut added: Vec<String> = Vec::new();
        for (kind, id) in &pending {
            if !kinds.contains(kind) {
                kinds.push(*kind);
            }
            if *kind == Kind::Notes {
                let note = self.backend.borrow().store.as_ref().and_then(|s| s.get(Kind::Notes, id).cloned());
                if let Some(revision) = note.and_then(|note| self.version_of(&note)) {
                    added.push(revision.str("note").to_string());
                    if let Some(store) = self.backend.borrow_mut().store.as_mut() {
                        store.put(Kind::Revisions, revision);
                    }
                    if !kinds.contains(&Kind::Revisions) {
                        kinds.push(Kind::Revisions);
                    }
                }
            }
        }
        Backend::changed(&self.backend, &kinds, false);
        self.prune_revisions(&added);
    }

    /// The note as it was, when this change comes after long enough untouched to count as a new sitting
    fn version_of(&self, note: &Value) -> Option<Value> {
        let id = note.id().to_string();
        let now = Sent { title: note.get("title").cloned().unwrap_or(Value::Null), body: note.get("body").cloned().unwrap_or(Value::Null), updated: note.num_or_zero("updated") };
        let before = self.sent.borrow_mut().insert(id.clone(), now.clone())?;
        if (before.title == now.title && before.body == now.body) || now.updated - before.updated < REVISION_GAP {
            return None;
        }
        Some(json!({ "id": data::new_id(), "note": id, "title": before.title, "body": before.body, "saved": data::ms(before.updated) }))
    }

    fn prune_revisions(&self, notes: &[String]) {
        let mut seen: Vec<&String> = Vec::new();
        for note in notes {
            if seen.contains(&note) {
                continue;
            }
            seen.push(note);
            let old: Vec<String> = {
                let backend = self.backend.borrow();
                let Some(store) = backend.store.as_ref() else { return };
                let mut list: Vec<&Value> = store.values(Kind::Revisions).filter(|r| r.str("note") == note).collect();
                list.sort_by(|a, b| b.num_or_zero("saved").partial_cmp(&a.num_or_zero("saved")).unwrap_or(std::cmp::Ordering::Equal));
                list.into_iter().skip(REVISIONS_KEPT).map(|r| r.id().to_string()).collect()
            };
            self.remove(Kind::Revisions, &old);
        }
    }

    /// Remembers the note as it is before the first change to it here
    pub fn edit_note(&self, id: &str, changes: Map<String, Value>) {
        let note = self.item(Kind::Notes, id);
        let Some(note) = note else { return };
        if !self.sent.borrow().contains_key(id) {
            self.sent.borrow_mut().insert(id.to_string(), Sent {
                title: note.get("title").cloned().unwrap_or(Value::Null),
                body: note.get("body").cloned().unwrap_or(Value::Null),
                updated: note.num_or_zero("updated"),
            });
        }
        let mut changes = changes;
        changes.insert("updated".into(), data::ms(data::now_ms()));
        self.update(Kind::Notes, id, &changes);
        self.save(Kind::Notes, id, false);
    }

    /// The page's own idea of a note as last sent, as `savedNotes.set(note.id, …)`
    pub fn mark_sent(&self, id: &str, title: Value, body: Value, updated: f64) {
        self.sent.borrow_mut().insert(id.to_string(), Sent { title, body, updated });
    }

    pub fn save_task(&self, id: &str, changes: Map<String, Value>, now: bool) {
        self.update(Kind::Tasks, id, &changes);
        self.save(Kind::Tasks, id, now);
    }

    /// Object.assign(item, changes)
    pub fn update(&self, kind: Kind, id: &str, changes: &Map<String, Value>) {
        if let Some(item) = self.backend.borrow_mut().store.as_mut().and_then(|s| s.get_mut(kind, id)) {
            data::assign(item, changes);
        }
    }

    /// A new item in the store
    pub fn insert(&self, kind: Kind, item: Value, now: bool) {
        let id = item.id().to_string();
        if let Some(store) = self.backend.borrow_mut().store.as_mut() {
            store.put(kind, item);
        }
        self.save(kind, &id, now);
    }

    pub fn item(&self, kind: Kind, id: &str) -> Option<Value> {
        self.backend.borrow().store.as_ref().and_then(|s| s.get(kind, id).cloned())
    }

    /// Runs `f` with the store, when the notes are open
    pub fn db<R>(&self, f: impl FnOnce(&Store) -> R) -> Option<R> {
        let backend = self.backend.borrow();
        backend.store.as_ref().map(f)
    }

    pub fn setting(&self, name: &str) -> Option<Value> {
        self.db(|store| store.setting(name).cloned()).flatten()
    }

    pub fn set_setting(&self, name: &str, value: Value) {
        let mut values = Map::new();
        values.insert(name.into(), value);
        Backend::set_settings(&self.backend, &values);
    }

    // --- Remembering the view ----------------------------------------------------------------------------

    pub fn remember(&self) {
        let ui = self.ui.borrow();
        let mode = if ui.mode == Mode::Settings { "notes" } else { ui.mode.name() };
        let value = json!({
            "mode": mode,
            "notesView": ui.notes_view.to_json(),
            "note": ui.note,
            "tasksView": ui.tasks_view.to_json(),
            "collapsed": ui.collapsed,
        });
        saved::save("notes", &value);
    }

    fn recall(&self) {
        let value = saved::load("notes");
        let mut ui = self.ui.borrow_mut();
        match value.get("mode").and_then(Value::as_str) {
            Some("tasks") => ui.mode = Mode::Tasks,
            Some("notes") => ui.mode = Mode::Notes,
            _ => {}
        }
        if let Some(view) = value.get("notesView").and_then(View::from_json) {
            ui.notes_view = view;
        }
        if let Some(view) = value.get("tasksView").and_then(View::from_json) {
            ui.tasks_view = view;
        }
        if let Some(note) = value.get("note") {
            ui.note = note.as_str().map(str::to_string);
        }
        if let Some(collapsed) = value.get("collapsed").and_then(Value::as_array) {
            ui.collapsed = collapsed.iter().filter_map(Value::as_str).map(str::to_string).collect();
        }
    }

    // --- Everything --------------------------------------------------------------------------------------

    pub fn clear_query(&self) {
        self.ui.borrow_mut().query.clear();
        if !self.search.text().is_empty() {
            self.search.set_text("");
        }
    }

    pub fn render(&self) {
        if self.ui.borrow().status != "open" {
            self.render_gate();
            return;
        }
        self.sidebar.set_visible(true);
        let (mode, encrypted) = {
            let ui = self.ui.borrow();
            (ui.mode, ui.encrypted)
        };
        self.lock_button.set_visible(encrypted);
        self.search.set_placeholder_text(Some(&if mode == Mode::Tasks { t!("Search tasks") } else { t!("Search notes") }));
        widgets::set_class(&self.settings_button, "current", mode == Mode::Settings);
        self.render_nav();
        match mode {
            Mode::Settings => {
                self.editor.replace(None);
                self.detail_for.replace(None);
                self.render_settings();
            }
            Mode::Tasks => self.render_tasks(false),
            Mode::Notes => self.render_notes(false),
        }
    }

    /// What the backend says now, after the notes were opened or set up: the views that point at something
    /// no longer there fall back to the defaults
    pub fn apply_state(&self, error: Option<String>) {
        if let Some(error) = error {
            toast(&if error.is_empty() { t!("Something went wrong.") } else { error }, None);
            return;
        }
        {
            let backend = self.backend.borrow();
            let mut ui = self.ui.borrow_mut();
            ui.status = backend.status();
            ui.encrypted = backend.encrypted();
            ui.passkeys = backend.passkeys();
            ui.dir = backend.paths.dir.display().to_string();
        }
        self.sent.borrow_mut().clear();
        let fixed = self.db(|store| {
            let ui = self.ui.borrow();
            let notes_gone = (ui.notes_view.kind == "notebook" && !store.has(Kind::Notebooks, ui.notes_view.id.as_deref().unwrap_or("")))
                || (ui.notes_view.kind == "tag" && !store.has(Kind::Tags, ui.notes_view.id.as_deref().unwrap_or("")));
            let tasks_gone = (ui.tasks_view.kind == "project" && !store.has(Kind::Projects, ui.tasks_view.id.as_deref().unwrap_or("")))
                || (ui.tasks_view.kind == "label" && !store.has(Kind::Labels, ui.tasks_view.id.as_deref().unwrap_or("")));
            let note_gone = !ui.note.as_ref().is_some_and(|id| store.has(Kind::Notes, id));
            (notes_gone, tasks_gone, note_gone)
        });
        if let Some((notes_gone, tasks_gone, note_gone)) = fixed {
            if notes_gone {
                self.ui.borrow_mut().notes_view = View::of("all");
            }
            if tasks_gone {
                self.ui.borrow_mut().tasks_view = View::of("today");
            }
            if note_gone {
                let view = self.ui.borrow().notes_view.clone();
                let first = self.sorted_notes(&view).first().cloned();
                self.ui.borrow_mut().note = first;
            }
        }
        self.editor.replace(None);
        self.detail_for.replace(None);
        self.render();
    }

    pub fn set_mode(&self, mode: Mode) {
        self.commit();
        widgets::close_menu();
        self.ui.borrow_mut().mode = mode;
        self.clear_query();
        self.remember();
        self.render();
    }

    pub fn lock_now(&self) {
        if !self.ui.borrow().encrypted {
            return;
        }
        self.commit();
        let backend = self.backend.clone();
        glib::spawn_future_local(async move { backend::lock(&backend).await });
    }

    /// Nothing of the notes is left in the page, on screen or off it, and nothing left to undo
    fn on_locked(&self) {
        widgets::close_menu();
        widgets::close_dialogs();
        self.pending.borrow_mut().clear();
        if let Some(timer) = self.pending_timer.take() {
            timer.remove();
        }
        self.sent.borrow_mut().clear();
        {
            let mut ui = self.ui.borrow_mut();
            ui.status = "locked";
        }
        self.editor.replace(None);
        self.detail_for.replace(None);
        self.notes_parts.replace(None);
        self.tasks_parts.replace(None);
        ui::clear(&self.nav);
        widgets::clear_toasts();
        self.clear_query();
        self.render();
        // Off the bus, and the quick note asks for the passphrase
        self.sync_today();
        self.capture_state();
    }

    /// The Markdown drawn again, for the colours its code and attachments are drawn in
    fn redraw_markdown(&self) {
        if self.ui.borrow().status == "open" && self.ui.borrow().mode == Mode::Notes {
            self.refresh_preview();
        }
    }
}

/// The page, made with the window
pub fn build(shell: Rc<Shell>) -> Rc<Notes> {
    Notes::new(shell)
}
