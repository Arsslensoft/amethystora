//! Amethystora Control: Amethystora's own settings outside security, in a window of their own, for what is
//! otherwise only in `ame`. backend.rs says what it runs and why; page.rs draws the list it shows.
//!
//! The window is locked down the way the Electron one was: the page only ever hands back an entry's id and
//! one of the values the list gave for it, and backend.rs runs what the list has for that.

mod backend;
mod icons;
mod layout;
mod page;
mod welcome;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::future::Future;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use amethystora_ui::adw::prelude::*;
use amethystora_ui::app::{self, App, Shell, Spec};
use amethystora_ui::frame::{self, NavButton};
use amethystora_ui::{gdk, gio, glib, gtk, process, t, ui};
use serde_json::Value;

use backend::{List, Outcome};
use icons::icon;
use layout::{Overflow, Tight};

/// How long the typing has to pause before the results are drawn
const SEARCH_PAUSE: std::time::Duration = std::time::Duration::from_millis(180);

pub fn main() -> glib::ExitCode {
    app::run(
        Spec {
            name: "control",
            id: "org.amethystora.Control",
            css: include_str!("control.css"),
            width: 1180,
            height: 840,
            min_width: 760,
            min_height: 540,
        },
        Control::new,
    )
}

/// What `ame setup diff` said, or that it is being asked
#[derive(Debug, Clone, Default)]
enum Diff {
    /// Not asked since the list was last read
    #[default]
    Unread,
    Reading,
    Changes(Vec<Value>),
    Error(String),
}

/// What the last command run from a card said
#[derive(Debug, Clone)]
struct Said {
    ok: bool,
    output: String,
}

/// At the top of the page, after a command opened a terminal or the list could not be read again
#[derive(Debug, Clone)]
struct Banner {
    failed: bool,
    text: String,
}

/// The theme maker's palette as it is being made, kept between drawings of the page and written nowhere until Save
#[derive(Debug, Clone, Default)]
struct Maker {
    /// The palette it started from, by the list's value; empty before the first drawing
    start: String,
    colors: BTreeMap<String, String>,
    name: String,
}

/// What the page keeps between drawings, as control.js's `state`
struct PageState {
    page: &'static str,
    /// The entry the window was opened on, scrolled to once its card is drawn
    reveal: Option<String>,
    /// The list as the page was last given it
    list: Option<Rc<List>>,
    error: String,
    search: String,
    /// The entries whose command runs now, and what the last one run here said, by id
    busy: HashSet<String>,
    said: HashMap<String, Said>,
    banner: Option<Banner>,
    diff: Diff,
    /// The trust card's own fields: the repository typed, the name of the key file chosen, and the
    /// repository a trust was started for, whose next steps it then shows
    repository: String,
    key: String,
    trusted: String,
    /// The new widget's fields
    widget_name: String,
    widget_example: String,
    /// The text shortcuts' form (the abbreviation and text typed, and the shortcut whose text it is editing), and the
    /// field to try them in: its text, an abbreviation it saw taken away and what was left, and what it last saw
    /// expanded. Nothing of it is written anywhere
    shortcut_abbr: String,
    shortcut_text: String,
    shortcut_editing: Option<String>,
    try_text: String,
    try_waiting: Option<(String, String)>,
    try_said: Option<(String, String)>,
    /// The Keyboard page's shortcuts: what their own search field holds, and the shortcut whose new keys are being
    /// recorded (its schema, key and words), while the window keeps the system's shortcuts from GNOME
    kb_search: String,
    recording: Option<(String, String, String)>,
    /// When the list was last read, and whether it is being read now
    read: f64,
    reading: bool,
    /// Whether the window has been opened on its first page yet
    started: bool,
    /// What was picked on the welcome page, by entry, until the list says so too
    picked: HashMap<String, String>,
    /// The theme maker's palette
    maker: Maker,
}

impl Default for PageState {
    fn default() -> Self {
        PageState {
            page: "appearance",
            reveal: None,
            list: None,
            error: String::new(),
            search: String::new(),
            busy: HashSet::new(),
            said: HashMap::new(),
            banner: None,
            diff: Diff::Unread,
            repository: String::new(),
            key: String::new(),
            trusted: String::new(),
            widget_name: String::new(),
            widget_example: String::new(),
            shortcut_abbr: String::new(),
            shortcut_text: String::new(),
            shortcut_editing: None,
            try_text: String::new(),
            try_waiting: None,
            try_said: None,
            kb_search: String::new(),
            recording: None,
            read: 0.0,
            reading: false,
            started: false,
            picked: HashMap::new(),
            maker: Maker::default(),
        }
    }
}

/// The parts of the window drawn once, around the page
struct Parts {
    root: gtk::Widget,
    /// The sidebar and the search, which the welcome page goes without
    sidebar: gtk::Widget,
    search_field: gtk::Widget,
    view: gtk::Box,
    scroller: gtk::ScrolledWindow,
    banner: gtk::Box,
    search: gtk::Text,
    pages: Vec<(&'static str, NavButton)>,
    updates_state: gtk::Label,
    backups: gtk::Button,
}

pub struct Control {
    me: Weak<Control>,
    shell: Rc<Shell>,
    // --- What main.js kept ------------------------------------------------------------------------------
    /// The list as the script last gave it: the only entries, choices and commands anything here runs
    known: RefCell<Option<Rc<List>>>,
    /// The public key picked for an image of one's own, which the page knows only by its file's name
    chosen_key: RefCell<Option<PathBuf>>,
    // --- What control.js kept ---------------------------------------------------------------------------
    state: RefCell<PageState>,
    parts: Parts,
    /// The wallpapers' thumbnails, decoded once at the size they are shown
    thumbnails: RefCell<HashMap<String, gdk::Texture>>,
    /// The colours of the themes' swatches, each a class in its own stylesheet
    colors: RefCell<BTreeSet<String>>,
    color_css: gtk::CssProvider,
    /// The theme preview's colours, those of the theme pointed at
    preview_css: gtk::CssProvider,
    /// The search field is being emptied by the page itself
    clearing: Cell<bool>,
    /// The drawing of the results, waiting for the typing to pause
    search_wait: Cell<Option<glib::SourceId>>,
    /// The welcome page's preview, kept from one drawing of the page to the next
    welcome_preview: RefCell<Option<Rc<welcome::Preview>>>,
    /// The window was closed while a command ran: it goes once the last one has finished
    closing: Cell<bool>,
}

impl App for Control {
    fn open(&self, args: &[String]) {
        // Opened again while a closed window waited for its last command: it is presented again and stays open
        self.closing.set(false);
        let page = backend::page_argument(args);
        let entry = page.and_then(|_| backend::entry_argument(args));
        let this = self.rc();
        let first = !self.state.borrow().started;
        if first {
            // The page to open: `amethystora-control system` opens on the system's settings
            {
                let mut state = self.state.borrow_mut();
                state.started = true;
                state.page = page.unwrap_or("appearance");
                state.reveal = entry;
            }
            self.render();
            this.load(false);
        } else if let Some(page) = page {
            this.show(page);
            self.state.borrow_mut().reveal = entry;
            this.reveal();
        }
    }

    /// Back from a terminal, or from anywhere a setting may have changed: read the list again
    fn focus(&self) {
        let again = {
            let state = self.state.borrow();
            state.list.is_some() && amethystora_ui::i18n::now() - state.read > 1.5
        };
        if again {
            self.rc().load(true);
        }
    }

    fn reload(&self) {
        self.rc().load(true);
    }
}

impl Control {
    fn new(shell: Rc<Shell>) -> Rc<Control> {
        let color_css = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &gtk::prelude::WidgetExt::display(&shell.window),
            &color_css,
            gtk::STYLE_PROVIDER_PRIORITY_USER + 4,
        );
        // Over the page's own stylesheet, as the swatches' colours are
        let preview_css = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &gtk::prelude::WidgetExt::display(&shell.window),
            &preview_css,
            gtk::STYLE_PROVIDER_PRIORITY_USER + 5,
        );
        let this = Rc::new_cyclic(|me: &Weak<Control>| Control {
            me: me.clone(),
            shell: shell.clone(),
            known: RefCell::new(None),
            chosen_key: RefCell::new(None),
            state: RefCell::new(PageState::default()),
            parts: Self::build(me),
            thumbnails: RefCell::new(HashMap::new()),
            colors: RefCell::new(BTreeSet::new()),
            color_css,
            preview_css,
            clearing: Cell::new(false),
            search_wait: Cell::new(None),
            welcome_preview: RefCell::new(None),
            closing: Cell::new(false),
        });
        shell.window.set_title(Some(&t!("Amethystora Control")));
        shell.window.set_content(Some(&this.parts.root));
        shell.narrow(900.0, "narrow");
        this.keys();
        // Closed while a command still runs here (a theme being applied after Pick a look's Done): the window goes,
        // the program stays until the command has finished, so that it is never cut off half done
        let me = this.me.clone();
        shell.window.connect_close_request(move |window| {
            let Some(this) = me.upgrade() else { return glib::Propagation::Proceed };
            if this.state.borrow().busy.is_empty() {
                return glib::Propagation::Proceed;
            }
            this.closing.set(true);
            window.set_visible(false);
            glib::Propagation::Stop
        });
        this
    }

    /// This window, from a method that only has &self
    fn rc(&self) -> Rc<Control> {
        self.me.upgrade().expect("the window is open")
    }

    fn known(&self) -> Option<Rc<List>> {
        self.known.borrow().clone()
    }

    /// The sidebar, the search and the page's place, which stay; the page itself is render()'s
    fn build(me: &Weak<Control>) -> Parts {
        // The wordmark of the boot splash and the login screen, the glowing stone as its "a"
        let brand = amethystora_ui::brand::brand(&t!("Control"), 21.0);
        brand.update_property(&[gtk::accessible::Property::Label(&t!("Amethystora Control"))]);
        // white-space: nowrap: in a narrow window it runs past the sidebar rather than widening it
        let holder = ui::hbox(0, &[]);
        holder.append(&Overflow::new(&brand));
        let sidebar = frame::Sidebar::new(&holder, 18);
        let nav = frame::nav(3, None, &t!("Settings"));
        let mut pages = Vec::new();
        for (page, name, title) in [
            ("appearance", "palette", t!("Appearance")),
            ("desktop", "desktop", t!("Desktop")),
            ("keyboard", "key", t!("Keyboard")),
            ("apps", "apps", t!("Apps")),
            ("agent", "sparkle", t!("AI agent")),
            ("system", "chip", t!("System")),
            ("setup", "file", t!("Your setup")),
        ] {
            let button = NavButton::new(icon(name, 20), &title, 12);
            let me = me.clone();
            button.button.connect_clicked(move |_| {
                if let Some(this) = me.upgrade() {
                    this.show(page);
                }
            });
            nav.append(&button.button);
            pages.push((page, button));
        }
        sidebar.body.append(&nav);

        let elsewhere = frame::nav(3, Some(&t!("Elsewhere")), &t!("Elsewhere"));
        elsewhere.add_css_class("elsewhere");
        let away = |name: &str, title: &str| NavButton::new(icon(name, 20), title, 12);
        let external = || {
            let mark = icon("external", 16).mirror_in_rtl();
            mark.add_css_class("faint");
            mark
        };
        let security = away("shield", &t!("Security settings"));
        security.append(&external());
        security.button.connect_clicked({
            let me = me.clone();
            move |_| {
                if let Some(this) = me.upgrade() {
                    this.open_link("security");
                }
            }
        });
        let updates = away("refresh", &t!("Updates"));
        let updates_state = ui::line("", &["nav-state"]);
        updates_state.set_tooltip_text(Some(&t!("Automatic updates")));
        updates.append(&updates_state);
        updates.append(&external());
        updates.button.connect_clicked({
            let me = me.clone();
            move |_| {
                if let Some(this) = me.upgrade() {
                    this.open_link("updates");
                }
            }
        });
        let backups = away("archive", &t!("Backups"));
        backups.append(&external());
        backups.button.set_visible(false);
        backups.button.connect_clicked({
            let me = me.clone();
            move |_| {
                if let Some(this) = me.upgrade() {
                    this.open_link("backups");
                }
            }
        });
        elsewhere.append(&security.button);
        elsewhere.append(&updates.button);
        elsewhere.append(&backups.button);
        sidebar.body.append(&elsewhere);

        sidebar.foot.set_spacing(12);
        let manual = ui::button(&t!("Control in the manual"), Some(icon("book", 18)), &["link", "quiet"], || process::manual("control"));
        manual.set_halign(gtk::Align::Start);
        sidebar.foot.append(&manual);
        sidebar.foot.append(&frame::terminal(&t!("Also in a terminal:"), "ame control"));

        // The search, with the window's own buttons beside it
        let search = gtk::Text::builder().accessible_role(gtk::AccessibleRole::TextBox).placeholder_text(t!("Search the settings")).hexpand(true).build();
        search.update_property(&[gtk::accessible::Property::Label(&t!("Search the settings"))]);
        let field = ui::hbox(9, &["search"]);
        field.append(&icon("search", 18));
        field.append(&search);
        let topbar = ui::hbox(0, &["topbar"]);
        let spacer = ui::hbox(0, &[]);
        spacer.set_hexpand(true);
        topbar.append(&spacer);
        let field = Tight::new(&field);
        field.set_valign(gtk::Align::Center);
        topbar.append(&field);
        let controls = frame::controls();
        controls.set_valign(gtk::Align::Center);
        topbar.append(&controls);
        search.connect_changed({
            let me = me.clone();
            move |text| {
                if let Some(this) = me.upgrade() {
                    this.searched(&text.text());
                }
            }
        });

        let view = ui::vbox(0, &["view"]);
        let scroller = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&view).build();
        let banner = ui::vbox(0, &["banner-slot"]);
        banner.set_visible(false);
        let main = ui::vbox(0, &["main"]);
        main.append(&topbar);
        main.append(&banner);
        main.append(&scroller);

        // As wide as the stylesheet makes it, whatever the words in it would like, and scrolling in a window
        // too short for it, as `overflow-y: auto` had it
        sidebar.root.set_vexpand(true);
        let column = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_width(true)
            .child(&sidebar.root)
            .build();
        let column = Tight::new(&column);
        let root = frame::with_sidebar(&column, &main, -1, false);
        Parts {
            root,
            sidebar: column.upcast(),
            search_field: field.upcast(),
            view,
            scroller,
            banner,
            search,
            pages,
            updates_state,
            backups: backups.button,
        }
    }

    /// Ctrl+F or / searches, Escape leaves the search; Ctrl+R, F5 and the zoom are the window's
    fn keys(&self) {
        let keys = gtk::EventControllerKey::new();
        let me = self.me.clone();
        keys.connect_key_pressed(move |controller, key, _, modifiers| {
            let Some(this) = me.upgrade() else { return glib::Propagation::Proceed };
            let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
            let typing = controller
                .widget()
                .and_then(|window| window.root())
                .and_then(|root| root.focus())
                .is_some_and(|focus| focus.is::<gtk::Text>() || focus.ancestor(gtk::DropDown::static_type()).is_some());
            if (ctrl && matches!(key, gdk::Key::f | gdk::Key::F)) || (key == gdk::Key::slash && !ctrl && !typing) {
                this.parts.search.grab_focus();
                this.parts.search.select_region(0, -1);
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::Escape && !this.state.borrow().search.is_empty() {
                this.state.borrow_mut().search.clear();
                this.clearing.set(true);
                this.parts.search.set_text("");
                this.clearing.set(false);
                this.render();
                this.read_diff();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.shell.window.add_controller(keys);

        // The keys pressed for a shortcut being recorded, before any widget of the window sees them
        let record = gtk::EventControllerKey::new();
        record.set_propagation_phase(gtk::PropagationPhase::Capture);
        let me = self.me.clone();
        record.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(this) = me.upgrade() else { return glib::Propagation::Proceed };
            if this.state.borrow().recording.is_none() {
                return glib::Propagation::Proceed;
            }
            this.recorded(key, modifiers);
            glib::Propagation::Stop
        });
        self.shell.window.add_controller(record);
    }

    fn searched(self: &Rc<Self>, text: &str) {
        if self.clearing.get() {
            return;
        }
        self.state.borrow_mut().search = text.trim().to_string();
        if let Some(waiting) = self.search_wait.take() {
            waiting.remove();
        }
        if self.state.borrow().search.is_empty() {
            self.render();
            self.read_diff();
            return;
        }
        // Drawn once the typing pauses: a page of results takes longer to lay out than a key takes to press
        let this = self.clone();
        let waiting = glib::timeout_add_local_once(SEARCH_PAUSE, move || {
            this.search_wait.set(None);
            this.render();
        });
        self.search_wait.set(Some(waiting));
    }

    // --- The list -------------------------------------------------------------------------------------

    fn load(self: &Rc<Self>, quiet: bool) {
        let this = self.clone();
        glib::spawn_future_local(async move { this.load_list(quiet).await });
    }

    async fn load_list(self: &Rc<Self>, quiet: bool) {
        {
            let mut state = self.state.borrow_mut();
            if state.reading {
                return;
            }
            state.reading = true;
            if !quiet {
                state.list = None;
                state.error.clear();
            }
        }
        if !quiet {
            self.render();
        }
        let result = backend::read_list().await;
        // Read again quietly (the window coming back to the front) and found the same: nothing is drawn again, which
        // would close an open drop-down whose popover took the window's focus for a moment
        let mut unchanged = false;
        {
            let mut state = self.state.borrow_mut();
            state.reading = false;
            state.read = amethystora_ui::i18n::now();
            match result {
                Ok(list) if quiet && state.banner.is_none() && state.list.as_deref() == Some(&list) => {
                    unchanged = true;
                }
                Ok(list) => {
                    let list = Rc::new(list);
                    self.known.replace(Some(list.clone()));
                    state.list = Some(list);
                    state.error.clear();
                    // Set up again, or saved: what it says has to be compared once more
                    state.diff = Diff::Unread;
                }
                Err(error) => {
                    self.known.replace(None);
                    // A list read before stays on screen; only the first one failing takes its place
                    if state.list.is_none() {
                        state.error = error;
                    } else {
                        state.banner = Some(Banner { failed: true, text: error });
                    }
                }
            }
        }
        if unchanged {
            return;
        }
        self.render();
        self.reveal();
        self.read_diff();
    }

    /// The card of the entry the window was opened on, scrolled to the top of the page once the page has been laid out
    /// (its place is only known then); once
    fn reveal(self: &Rc<Self>) {
        let Some(id) = ({
            let state = self.state.borrow();
            state.list.is_some().then(|| state.reveal.clone()).flatten()
        }) else {
            return;
        };
        self.state.borrow_mut().reveal = None;
        let name = format!("entry:{id}");
        let view = self.parts.view.clone();
        let adjustment = self.parts.scroller.vadjustment();
        let tries = Cell::new(0);
        self.parts.scroller.add_tick_callback(move |_, _| {
            tries.set(tries.get() + 1);
            let card = find_named(view.upcast_ref(), &name);
            match card.as_ref().and_then(|card| card.compute_point(&view, &amethystora_ui::graphene::Point::new(0.0, 0.0))) {
                Some(point) if card.as_ref().is_some_and(|card| card.width() > 0) => {
                    adjustment.set_value((point.y() as f64 - 24.0).max(0.0));
                    glib::ControlFlow::Break
                }
                _ if tries.get() < 60 => glib::ControlFlow::Continue,
                _ => glib::ControlFlow::Break,
            }
        });
    }

    /// The setup page compares the machine with the file, which takes a moment, once it is shown
    fn read_diff(self: &Rc<Self>) {
        {
            let mut state = self.state.borrow_mut();
            let saved = state.list.as_ref().and_then(|list| list.items.iter().find(|item| item.kind == "setup")).is_some_and(|item| item.saved.is_some());
            if state.page != "setup" || !state.search.is_empty() || !saved || !matches!(state.diff, Diff::Unread) {
                return;
            }
            state.diff = Diff::Reading;
        }
        self.render();
        let this = self.clone();
        glib::spawn_future_local(async move {
            let diff = match backend::setup_diff().await {
                Ok(changes) => Diff::Changes(changes),
                Err(error) => Diff::Error(error),
            };
            this.state.borrow_mut().diff = diff;
            this.render();
        });
    }

    fn show(self: &Rc<Self>, page: &'static str) {
        if !backend::PAGES.contains(&page) && page != backend::WELCOME {
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            state.page = page;
            state.search.clear();
            state.banner = None;
        }
        self.clearing.set(true);
        self.parts.search.set_text("");
        self.clearing.set(false);
        self.render();
        self.parts.scroller.vadjustment().set_value(0.0);
        self.read_diff();
    }

    // --- Running what the list says -------------------------------------------------------------------

    /// A change, an action or a widget's command, handed to backend.rs. A command that opened a terminal
    /// is said at the top of the page; one run here shows what it said under its card, and the list is read
    /// again.
    fn perform(self: &Rc<Self>, id: &str, call: impl Future<Output = Outcome> + 'static) {
        {
            let mut state = self.state.borrow_mut();
            if state.busy.contains(id) {
                return;
            }
            state.busy.insert(id.to_string());
            state.said.remove(id);
            state.banner = None;
        }
        self.render();
        let this = self.clone();
        let id = id.to_string();
        glib::spawn_future_local(async move {
            let result = call.await;
            // Whether it ran here, after which the list is read again
            let ran = {
                let mut state = this.state.borrow_mut();
                state.busy.remove(&id);
                match result {
                    Outcome::Terminal => {
                        state.banner = Some(Banner {
                            failed: false,
                            text: t!("It opened in a terminal. This page reads the settings again when you come back to it."),
                        });
                        false
                    }
                    Outcome::Error(error) => {
                        state.said.insert(id.clone(), Said { ok: false, output: error });
                        false
                    }
                    Outcome::Ran { ok, output } => {
                        if !output.is_empty() || !ok {
                            state.said.insert(id.clone(), Said { ok, output });
                        }
                        true
                    }
                }
            };
            if this.closing.get() {
                // Closed meanwhile: gone once the last command has finished
                if this.state.borrow().busy.is_empty() {
                    this.shell.window.close();
                }
            } else if ran {
                this.load_list(true).await;
                // A list that came back the same draws nothing, and this card still has to lose its spinner and say what
                // the command said: a command that fails usually changes nothing
                this.render();
            } else {
                this.render();
            }
        });
    }

    /// The window another link opens, each by the one command that opens it
    fn open_link(&self, link: &str) {
        if let Some(program) = backend::link_program(link, self.known().as_deref()) {
            process::launch(program, &[]);
        }
    }

    /// The key's file, from the window's own file dialog: the page only ever learns its name
    fn choose_key(self: &Rc<Self>) {
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        let keys = gtk::FileFilter::new();
        keys.set_name(Some(&t!("Public keys")));
        keys.add_suffix("pub");
        keys.add_suffix("pem");
        filters.append(&keys);
        let every = gtk::FileFilter::new();
        every.set_name(Some(&t!("Every file")));
        every.add_pattern("*");
        filters.append(&every);
        let dialog = gtk::FileDialog::builder().title(t!("Choose the public key")).accept_label(t!("Choose")).modal(true).filters(&filters).default_filter(&keys).build();
        let this = self.clone();
        glib::spawn_future_local(async move {
            let Ok(file) = dialog.open_future(Some(&this.shell.window)).await else { return };
            let Some(path) = file.path() else { return };
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            this.chosen_key.replace(Some(path));
            if !name.is_empty() {
                this.state.borrow_mut().key = name;
                this.render();
            }
        });
    }
}

impl Control {
    /// A colour file from elsewhere, from the window's own file dialog, imported in a terminal that shows it and asks
    fn import_theme(self: &Rc<Self>, card: &str) {
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        let colours = gtk::FileFilter::new();
        colours.set_name(Some(&t!("Colour files")));
        for suffix in ["conf", "toml", "yaml", "yml"] {
            colours.add_suffix(suffix);
        }
        colours.add_pattern("*Xresources*");
        filters.append(&colours);
        let every = gtk::FileFilter::new();
        every.set_name(Some(&t!("Every file")));
        every.add_pattern("*");
        filters.append(&every);
        let dialog = gtk::FileDialog::builder().title(t!("Import a theme")).accept_label(t!("Import")).modal(true).filters(&filters).default_filter(&colours).build();
        let (this, card) = (self.clone(), card.to_string());
        glib::spawn_future_local(async move {
            let Ok(file) = dialog.open_future(Some(&this.shell.window)).await else { return };
            let Some(path) = file.path() else { return };
            let me = this.clone();
            this.perform(&card, async move { backend::perform(backend::import_theme(me.known().as_deref(), Some(&path))).await });
        });
    }
}

/// The first widget under `root` (itself included) with that name
fn find_named(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    if root.widget_name() == name {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = find_named(&widget, name) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}
