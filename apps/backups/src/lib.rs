//! Amethystora Backups: your home folder as it was at any hour the hourly snapshots or the daily restic
//! backup kept, on one timeline, and putting back what was lost. backend.rs says what it runs and why;
//! page.rs draws it, with the two long lists in list.rs.
//!
//! The window is locked down the way the Electron one was: the page only ever hands back a point in time
//! the helper listed, a path it listed (or a folder above one, or the folder the window was opened on), and
//! one of three ways to put back, and backend.rs checks each before the helper sees it.

mod backend;
mod icons;
mod layout;
mod list;
mod page;
mod past;
mod relative;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::Duration;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::app::{self, App, Shell, Spec};
use amethystora_ui::process::{self, Follow};
use amethystora_ui::{gdk, gio, glib, gtk, i18n, t};

use backend::{Answer, Failure, Listing, Status, Timeline, Versions};
use list::Ask;

pub fn main() -> glib::ExitCode {
    app::run(
        Spec {
            name: "backups",
            id: "org.amethystora.Backups",
            css: include_str!("backups.css"),
            width: 1240,
            height: 840,
            min_width: 820,
            min_height: 560,
        },
        Backups::new,
    )
}

/// A folder as it was: not read yet, being read, read, or what went wrong
#[derive(Debug, Clone, Default)]
pub enum Folder {
    #[default]
    None,
    Reading,
    Read(Rc<Listing>),
    Failed(Failure),
}

/// The versions of one file, while they are looked for and then
#[derive(Debug, Clone)]
pub enum VersionsState {
    Reading,
    Read(Answer<Versions>),
}

/// A question being asked before putting back
#[derive(Debug, Clone)]
pub enum Dialog {
    /// The whole folder shown, put back as it was
    Folder,
    /// The folder as it was at the point the past view shows, put back beside what is there now
    Past,
    /// These names, some of which are in the folder now
    Conflict(Vec<String>),
}

/// What the last thing done here came to
#[derive(Debug, Clone)]
pub struct Banner {
    pub failed: bool,
    pub text: String,
    pub detail: String,
}

/// The backup started from here: running with what it logged so far, and then how it went
#[derive(Debug, Clone)]
pub enum BackupRun {
    Running(Vec<String>),
    Done { ok: bool, lines: String },
}

/// What the page keeps between drawings, as backups.js's `state`
pub struct PageState {
    pub page: &'static str,
    /// amethystora-restore status, and whether the agentic features are on
    pub status: Option<Rc<Status>>,
    /// amethystora-restore points, and this machine's name
    pub timeline: Option<Rc<Timeline>>,
    pub error: String,
    /// The point shown, by id, and the folder, relative to the home folder ("" is the home folder)
    pub point: String,
    pub folder: String,
    pub listing: Folder,
    /// The names ticked, in the order they were
    pub selected: Vec<String>,
    pub hidden: bool,
    /// Back in time, visibly: the folder's past as windows receding (past.rs)
    pub past: bool,
    /// The versions of one file: the file, and what was found
    pub versions: Option<(String, VersionsState)>,
    pub dialog: Option<Dialog>,
    pub mode: &'static str,
    /// A sentence while something takes a while, and what the last thing done here came to
    pub working: String,
    pub banner: Option<Banner>,
    pub backup: Option<BackupRun>,
    /// What the backup logged when it last failed, read once the status says so
    pub failed_lines: Option<String>,
    pub daily_busy: bool,
    pub read: f64,
    /// Whether the window has been opened on its first page yet
    pub started: bool,
}

impl Default for PageState {
    fn default() -> Self {
        PageState {
            page: "browse",
            status: None,
            timeline: None,
            error: String::new(),
            point: String::new(),
            folder: String::new(),
            listing: Folder::None,
            selected: Vec::new(),
            hidden: false,
            past: false,
            versions: None,
            dialog: None,
            mode: "keep-both",
            working: String::new(),
            banner: None,
            backup: None,
            failed_lines: None,
            daily_busy: false,
            read: 0.0,
            started: false,
        }
    }
}

/// The backup started here, while it runs: its journal followed, and its state read every moment
struct Backing {
    _journal: Option<Follow>,
    timer: Option<glib::SourceId>,
}

pub struct Backups {
    me: Weak<Backups>,
    shell: Rc<Shell>,
    // --- What main.js kept --------------------------------------------------------------------------------
    restore: backend::Restore,
    backing: RefCell<Option<Backing>>,
    home: PathBuf,
    // --- What backups.js kept -----------------------------------------------------------------------------
    state: RefCell<PageState>,
    parts: page::Parts,
    /// The listing, and whether hidden files were shown, the list view was last given
    shown: RefCell<Option<(Rc<Listing>, bool)>>,
    shown_timeline: RefCell<Option<Rc<Timeline>>>,
    /// The next listing shown from the top: another folder was opened
    scroll_to_top: Cell<bool>,
    dialog_layer: RefCell<Option<gtk::Widget>>,
    /// The log of the backup running, changed in place as its lines come
    live_log: RefCell<Option<gtk::Label>>,
    /// Where a walk through the past has got to, before the folder there is read, and what reads it once it settles
    past_target: RefCell<Option<String>>,
    past_timer: RefCell<Option<glib::SourceId>>,
    /// The point the past view shows is chosen, and the folder there has not been read yet: the list on the screen is still
    /// the one of the point before, so nothing is put back or looked at from it meanwhile
    past_pending: Cell<bool>,
    /// What tells the window that a snapshot was taken, or the backup set up or switched, elsewhere
    follow: RefCell<Option<Rc<amethystora_ui::live::Live>>>,
}

impl App for Backups {
    // A folder is relative to where the launch was made from, the second launch's own folder included
    fn open_in(&self, args: &[String], cwd: Option<PathBuf>) {
        let this = self.rc();
        let page = backend::page_argument(args);
        let cwd = cwd.or_else(|| std::env::current_dir().ok()).unwrap_or_else(|| self.home.clone());
        let folder = if page.is_some() { None } else { backend::folder_argument(args, &cwd, &self.home) };
        let point = backend::point_argument(args);
        // Files' Earlier versions: the file's own versions, over the folder it is in
        let file = backend::versions_argument(args, &cwd, &self.home);
        let folder = file.as_deref().map(|file| backend::parent(file).to_string()).or(folder);
        if let Some(file) = &file {
            self.restore.known.borrow_mut().remember(file);
        }
        if !self.state.borrow().started {
            if let Some(folder) = &folder {
                self.restore.known.borrow_mut().remember(folder);
            }
            {
                let mut state = self.state.borrow_mut();
                state.started = true;
                state.page = if folder.is_some() { "browse" } else { page.unwrap_or("browse") };
                state.folder = folder.unwrap_or_default();
                // Opened on a point in time, which load() keeps when the timeline has it, and otherwise
                // the newest
                state.point = point;
            }
            self.render();
            if let Some(file) = &file {
                self.show_versions(file);
            }
            this.load(false, false);
            return;
        }
        // Opened again on a folder or a page, from Files or the command line
        if folder.is_some() || !point.is_empty() {
            let folder = folder.unwrap_or_default();
            self.restore.known.borrow_mut().remember(&folder);
            self.state.borrow_mut().page = "browse";
            let opener = this.clone();
            glib::spawn_future_local(async move { opener.open_folder_now(&folder).await });
            // The file's versions come in a moment, and do not wait for a backup's folder: spawned after the line above, whose
            // first step clears the versions
            if let Some(file) = file {
                let shower = this.clone();
                glib::spawn_future_local(async move { shower.show_versions(&file) });
            }
            // A point in time asked for, such as the snapshot from before something rewrote the files
            let known = self.state.borrow().timeline.as_ref().is_some_and(|timeline| timeline.points.iter().any(|p| p.id == point));
            if !point.is_empty() && known {
                glib::spawn_future_local(async move { this.choose_point_now(&point, false).await });
            }
        } else if let Some(page) = page {
            this.show(page);
        }
    }

    /// Back from a terminal or from Security, where something may have been set up or switched
    fn focus(&self) {
        let (due, running) = {
            let state = self.state.borrow();
            (state.status.is_some() && i18n::now() - state.read > 3.0, matches!(state.backup, Some(BackupRun::Running(_))))
        };
        if !due || running {
            return;
        }
        let this = self.rc();
        glib::spawn_future_local(async move {
            let set_up = |this: &Backups| this.state.borrow().status.as_ref().map(|status| (status.backup.set_up, status.snapshots.on));
            let before = set_up(&this);
            this.load_now(true, true).await;
            if set_up(&this) != before {
                this.load_now(true, false).await;
            }
        });
    }

    fn reload(&self) {
        self.rc().load(true, false);
    }
}

impl Backups {
    fn new(shell: Rc<Shell>) -> Rc<Backups> {
        let this = Rc::new_cyclic(|me: &Weak<Backups>| Backups {
            me: me.clone(),
            shell: shell.clone(),
            restore: backend::Restore::default(),
            backing: RefCell::new(None),
            home: backend::home(),
            state: RefCell::new(PageState::default()),
            parts: page::Parts::new(me),
            shown: RefCell::new(None),
            shown_timeline: RefCell::new(None),
            scroll_to_top: Cell::new(false),
            dialog_layer: RefCell::new(None),
            live_log: RefCell::new(None),
            past_target: RefCell::new(None),
            past_timer: RefCell::new(None),
            past_pending: Cell::new(false),
            follow: RefCell::new(None),
        });
        shell.window.set_title(Some(&t!("Amethystora Backups")));
        shell.window.set_content(Some(&this.parts.root));
        shell.narrow(1000.0, "narrow");
        // ...where the folder's rows have no column for when each was changed, as display: none had it
        let weak = Rc::downgrade(&this);
        shell.window.connect_notify_local(Some("css-classes"), move |window, _| {
            if let Some(this) = weak.upgrade() {
                let narrow = window.has_css_class("narrow");
                if this.parts.files.narrow.get() != narrow {
                    this.parts.files.set_narrow(narrow);
                }
            }
        });
        this.keys();
        // What Ctrl+R and F5 do, as an action of the application's too, which the tests start over D-Bus
        let reload = gio::SimpleAction::new("reload", None);
        let weak = Rc::downgrade(&this);
        reload.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.load(true, false);
            }
        });
        shell.app.add_action(&reload);
        this.follow();
        // The backup is systemd's and carries on without the window
        let weak = Rc::downgrade(&this);
        shell.window.connect_close_request(move |_| {
            if let Some(this) = weak.upgrade() {
                this.stop_following();
            }
            glib::Propagation::Proceed
        });
        this
    }

    /// A snapshot taken, the backup set up, run or switched, the snapshots turned on or off in Security: read again at
    /// once, quietly, keeping the folder and point shown
    fn follow(&self) {
        use amethystora_ui::live;
        let me = self.me.clone();
        let paths = vec![
            "/var/home/.snapshots".into(),
            live::home(".config/amethystora"),
            live::home(".local/state/amethystora/backup-last-success"),
        ];
        let watch = live::watch(paths, live::Units { system: true, user: true }, move || {
            let Some(this) = me.upgrade() else { return };
            let running = matches!(this.state.borrow().backup, Some(BackupRun::Running(_)));
            if this.state.borrow().status.is_some() && !running {
                this.load(true, false);
            }
        });
        self.follow.replace(Some(watch));
    }

    /// This window, from a method that only has &self
    fn rc(&self) -> Rc<Backups> {
        self.me.upgrade().expect("the window is open")
    }

    /// Escape, Alt+Up or Backspace, and Ctrl+A; Ctrl+R, F5 and the zoom are the window's
    fn keys(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let me = self.me.clone();
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(this) = me.upgrade() else { return glib::Propagation::Proceed };
            let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
            let alt = modifiers.contains(gdk::ModifierType::ALT_MASK);
            let (dialog, versions, browsing, folder, listed, past) = {
                let state = this.state.borrow();
                (state.dialog.is_some(), state.versions.is_some(), state.page == "browse", state.folder.clone(), matches!(state.listing, Folder::Read(_)), state.past)
            };
            if past && !dialog && browsing && !versions {
                if key == gdk::Key::Page_Down {
                    this.walk_past(1);
                    return glib::Propagation::Stop;
                }
                if key == gdk::Key::Page_Up {
                    this.walk_past(-1);
                    return glib::Propagation::Stop;
                }
                if key == gdk::Key::Escape {
                    this.set_past(false);
                    return glib::Propagation::Stop;
                }
            }
            if key == gdk::Key::Escape && dialog {
                this.state.borrow_mut().dialog = None;
                this.render_dialog();
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::Escape && versions {
                this.state.borrow_mut().versions = None;
                this.render();
                return glib::Propagation::Stop;
            }
            if ((alt && key == gdk::Key::Up) || (key == gdk::Key::BackSpace && !ctrl && !alt)) && browsing && !folder.is_empty() && !dialog {
                this.open_folder(backend::parent(&folder));
                return glib::Propagation::Stop;
            }
            if ctrl && matches!(key, gdk::Key::a | gdk::Key::A) && browsing && listed && !dialog {
                let names: Vec<String> = this.entries().iter().map(|entry| entry.name.clone()).collect();
                this.state.borrow_mut().selected = names;
                this.render();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.shell.window.add_controller(keys);
    }

    // --- Reading what there is ------------------------------------------------------------------------

    fn load(&self, quiet: bool, status_only: bool) {
        let this = self.rc();
        glib::spawn_future_local(async move { this.load_now(quiet, status_only).await });
    }

    /// What is set up, and, unless only that is wanted, every point in time again
    async fn load_now(self: &Rc<Self>, quiet: bool, status_only: bool) {
        if !quiet {
            {
                let mut state = self.state.borrow_mut();
                state.status = None;
                state.timeline = None;
                state.error.clear();
            }
            self.render();
        }
        let this = self.clone();
        let status = glib::spawn_future_local(async move { this.restore.status().await });
        let timeline: Answer<Option<Rc<Timeline>>> = if status_only { Ok(self.state.borrow().timeline.clone()) } else { self.restore.points().await.map(|timeline| Some(Rc::new(timeline))) };
        let status = status.await.unwrap_or_else(|_| Err(Failure { error: "failed".into(), detail: String::new() }));
        self.state.borrow_mut().read = i18n::now();
        let (status, timeline) = match (status, timeline) {
            (Ok(status), Ok(timeline)) => (status, timeline),
            (status, timeline) => {
                let detail = |failure: Option<&Failure>| failure.map(|f| f.detail.clone()).unwrap_or_default();
                let (first, second) = (status.as_ref().err(), timeline.as_ref().err());
                let error = [detail(first), detail(second)].into_iter().find(|d| !d.is_empty()).unwrap_or_else(|| page::error_text(first.or(second).expect("one failed")));
                self.state.borrow_mut().error = error;
                self.render();
                return;
            }
        };
        let failed = status.backup.failed || status.backup.stale;
        {
            let mut state = self.state.borrow_mut();
            state.status = Some(Rc::new(status));
            state.timeline = timeline;
            state.error.clear();
        }
        // What a failed or stale backup last logged, read once
        if !failed {
            self.state.borrow_mut().failed_lines = None;
        } else if self.state.borrow().failed_lines.is_none() {
            let lines = backend::last_lines().await;
            self.state.borrow_mut().failed_lines = Some(lines);
        }
        // The newest point, unless the one shown is still there
        {
            let mut state = self.state.borrow_mut();
            let points = state.timeline.as_ref().map(|timeline| timeline.points.clone()).unwrap_or_default();
            if !points.iter().any(|point| point.id == state.point) {
                state.point = points.first().map(|point| point.id.clone()).unwrap_or_default();
                state.listing = Folder::None;
            }
        }
        self.render();
        let (point, unread) = {
            let state = self.state.borrow();
            (state.point.clone(), matches!(state.listing, Folder::None))
        };
        if !point.is_empty() && (unread || !quiet) {
            self.load_folder(quiet).await;
        }
    }

    async fn load_folder(self: &Rc<Self>, quiet: bool) {
        let (point, folder) = {
            let state = self.state.borrow();
            (state.point.clone(), state.folder.clone())
        };
        if point.is_empty() {
            return;
        }
        if !quiet {
            self.state.borrow_mut().listing = Folder::Reading;
            self.render();
        }
        let result = self.restore.ls(&point, &folder).await;
        {
            let mut state = self.state.borrow_mut();
            // Another point or folder was picked meanwhile: that one's answer is the one to show
            if state.point != point || state.folder != folder {
                return;
            }
            // What is selected and still there stays selected
            let names: Vec<String> = result.as_ref().map(|listing| listing.entries.iter().map(|entry| entry.name.clone()).collect()).unwrap_or_default();
            state.selected.retain(|name| names.contains(name));
            state.listing = match result {
                Ok(listing) => Folder::Read(Rc::new(listing)),
                Err(failure) => Folder::Failed(failure),
            };
        }
        self.render();
    }

    async fn choose_point_now(self: &Rc<Self>, id: &str, keep: bool) {
        // A listing read in the foreground takes over from a quiet one still out
        self.past_pending.set(false);
        {
            let mut state = self.state.borrow_mut();
            if !keep {
                state.selected.clear();
            }
            state.point = id.to_string();
            state.banner = None;
        }
        self.load_folder(false).await;
    }

    /// A point the walk through the past settled on: the windows have moved already, so the folder there takes over from the
    /// one shown when it has been read, with no moment of nothing
    pub(crate) async fn choose_point_quietly(self: &Rc<Self>, id: &str) {
        {
            let mut state = self.state.borrow_mut();
            state.selected.clear();
            state.point = id.to_string();
            state.banner = None;
        }
        // The point is the new one, the list still the old: nothing acts on it until the folder there has been read
        self.past_pending.set(true);
        self.render();
        self.load_folder(true).await;
        if self.state.borrow().point == id {
            self.past_pending.set(false);
            self.render();
        }
    }

    fn choose_point(&self, id: &str) {
        let (this, id) = (self.rc(), id.to_string());
        glib::spawn_future_local(async move { this.choose_point_now(&id, false).await });
    }

    async fn open_folder_now(self: &Rc<Self>, folder: &str) {
        self.past_pending.set(false);
        {
            let mut state = self.state.borrow_mut();
            state.folder = folder.to_string();
            state.selected.clear();
            state.versions = None;
            state.banner = None;
        }
        self.scroll_to_top.set(true);
        self.load_folder(false).await;
    }

    pub(crate) fn open_folder(&self, folder: &str) {
        let (this, folder) = (self.rc(), folder.to_string());
        glib::spawn_future_local(async move { this.open_folder_now(&folder).await });
    }

    pub(crate) fn show(&self, page: &'static str) {
        if !backend::PAGES.contains(&page) {
            return;
        }
        // The past is left as soon as the person leaves the list
        if page != "browse" && self.state.borrow().past {
            self.set_past(false);
        }
        {
            let mut state = self.state.borrow_mut();
            state.page = page;
            state.banner = None;
        }
        self.render();
        self.scroll_page_top();
    }

    /// What a row of a list asked for
    pub(crate) fn asked(&self, ask: Ask) {
        match ask {
            Ask::Select(name, on) => {
                {
                    let mut state = self.state.borrow_mut();
                    let had = state.selected.iter().any(|s| *s == name);
                    if on && !had {
                        state.selected.push(name);
                    } else if !on && had {
                        state.selected.retain(|s| *s != name);
                    } else {
                        return;
                    }
                }
                self.render();
            }
            Ask::Toggle(name) => {
                let on = !self.state.borrow().selected.iter().any(|s| *s == name);
                self.asked(Ask::Select(name, on));
            }
            Ask::Open(name) => {
                let folder = backend::join(&self.state.borrow().folder, &name);
                self.open_folder(&folder);
            }
            Ask::Point(id) => self.choose_point(&id),
        }
    }

    // --- Versions, looking and putting back -----------------------------------------------------------

    pub(crate) fn show_versions(&self, file: &str) {
        self.state.borrow_mut().versions = Some((file.to_string(), VersionsState::Reading));
        self.render();
        let (this, file) = (self.rc(), file.to_string());
        glib::spawn_future_local(async move {
            let here = this.restore.versions_here(&file).await;
            // A backup can take minutes to answer: what this machine holds is shown meanwhile
            let later = here.as_ref().is_ok_and(|versions| versions.backup == "later");
            if !this.show_found(&file, here) || !later {
                return;
            }
            let result = this.restore.versions(&file).await;
            this.show_found(&file, result);
        });
    }

    /// Whether the file's versions were still what the window was showing
    fn show_found(&self, file: &str, result: Answer<Versions>) -> bool {
        let current = self.state.borrow().versions.as_ref().is_some_and(|(shown, _)| shown == file);
        if current {
            self.state.borrow_mut().versions = Some((file.to_string(), VersionsState::Read(result)));
            self.render();
        }
        current
    }

    /// One version, kept beside the file there is now, which it never touches
    pub(crate) fn keep_version(&self, file: &str, id: &str) {
        let from_backup = self.restore.known.borrow().point(id).is_some_and(|point| point.source == "backup");
        {
            let mut state = self.state.borrow_mut();
            state.banner = None;
            state.working = if from_backup { t!("Getting it from the backup and putting it back…") } else { t!("Putting it back…") };
        }
        self.render();
        let (this, file, id) = (self.rc(), file.to_string(), id.to_string());
        glib::spawn_future_local(async move {
            let result = this.restore.put_back(&id, std::slice::from_ref(&file), "keep-both").await;
            {
                let mut state = this.state.borrow_mut();
                state.working.clear();
                state.banner = Some(match &result {
                    Err(failure) => Banner { failed: true, text: page::error_text(failure), detail: failure.detail.clone() },
                    Ok(done) => Banner { failed: false, text: page::put_back_text(done, "keep-both"), detail: String::new() },
                });
            }
            this.load_folder(true).await;
            this.render();
        });
    }

    /// From a version to the folder it is in, at that point, with it selected
    pub(crate) fn go_to(&self, id: &str, name: &str) {
        {
            let mut state = self.state.borrow_mut();
            state.versions = None;
            state.selected = vec![name.to_string()];
        }
        let (this, id) = (self.rc(), id.to_string());
        glib::spawn_future_local(async move { this.choose_point_now(&id, true).await });
    }

    pub(crate) fn ask(&self, dialog: Dialog) {
        {
            let mut state = self.state.borrow_mut();
            state.dialog = Some(dialog);
            state.mode = "keep-both";
        }
        self.render_dialog();
    }

    /// Only a file that is still there and differs, or a folder still there, can meet something of the same
    /// name: then the question is asked, once for all of them
    pub(crate) fn start_put_back(&self, names: Vec<String>) {
        if self.past_pending.get() {
            return;
        }
        let (conflict, folder) = {
            let state = self.state.borrow();
            let conflict = match &state.listing {
                Folder::Read(listing) => listing.entries.iter().any(|entry| names.contains(&entry.name) && (entry.now == "changed" || entry.now == "present")),
                _ => false,
            };
            (conflict, state.folder.clone())
        };
        if conflict {
            self.ask(Dialog::Conflict(names));
        } else {
            self.put_back(names.iter().map(|name| backend::join(&folder, name)).collect(), "keep-both");
        }
    }

    pub(crate) fn put_back(&self, files: Vec<String>, mode: &'static str) {
        if self.past_pending.get() {
            return;
        }
        let point = self.current_point();
        self.state.borrow_mut().dialog = None;
        self.render_dialog();
        {
            let mut state = self.state.borrow_mut();
            state.banner = None;
            state.working = if point.as_ref().is_some_and(|p| p.source == "backup") { t!("Getting it from the backup and putting it back…") } else { t!("Putting it back…") };
        }
        self.render();
        let this = self.rc();
        glib::spawn_future_local(async move {
            let id = this.state.borrow().point.clone();
            let result = this.restore.put_back(&id, &files, mode).await;
            {
                let mut state = this.state.borrow_mut();
                state.working.clear();
                state.banner = Some(match &result {
                    Err(failure) => Banner { failed: true, text: page::error_text(failure), detail: failure.detail.clone() },
                    Ok(done) => Banner { failed: false, text: page::put_back_text(done, mode), detail: String::new() },
                });
                state.selected.clear();
            }
            this.load_folder(true).await;
        });
    }

    pub(crate) fn look_at(&self, file: &str, id: Option<&str>) {
        if id.is_none() && self.past_pending.get() {
            return;
        }
        let id = id.map(str::to_string).unwrap_or_else(|| self.state.borrow().point.clone());
        let backup = self.state.borrow().timeline.as_ref().and_then(|timeline| timeline.points.iter().find(|p| p.id == id).map(|p| p.source == "backup")).unwrap_or(false);
        {
            let mut state = self.state.borrow_mut();
            state.banner = None;
            state.working = if backup { t!("Getting a copy from the backup…") } else { String::new() };
        }
        self.render();
        let (this, file) = (self.rc(), file.to_string());
        glib::spawn_future_local(async move {
            let result = this.restore.look(&id, &file).await;
            {
                let mut state = this.state.borrow_mut();
                state.working.clear();
                state.banner = Some(match &result {
                    Err(failure) => Banner { failed: true, text: page::error_text(failure), detail: failure.detail.clone() },
                    Ok(_) => Banner { failed: false, text: t!("Opened read-only. Copy out of it what you want, or put it back from here."), detail: String::new() },
                });
            }
            this.render();
        });
    }

    pub(crate) fn set_daily(&self, on: bool) {
        self.state.borrow_mut().daily_busy = true;
        self.render();
        let this = self.rc();
        glib::spawn_future_local(async move {
            let (ok, output) = backend::daily(on).await;
            {
                let mut state = this.state.borrow_mut();
                state.daily_busy = false;
                if !ok {
                    state.banner = Some(Banner { failed: true, text: t!("That did not work."), detail: output });
                }
            }
            this.load_now(true, true).await;
        });
    }

    // --- Backing up now -------------------------------------------------------------------------------

    pub(crate) fn backup_now(&self) {
        self.state.borrow_mut().backup = Some(BackupRun::Running(Vec::new()));
        self.render();
        let this = self.rc();
        glib::spawn_future_local(async move {
            if let Err(detail) = this.start_backup().await {
                this.state.borrow_mut().backup = Some(BackupRun::Done { ok: false, lines: detail });
                this.render();
            }
        });
    }

    /// The same run the daily timer starts, now, followed in the journal as Updates follows uupd. A run
    /// already going is followed rather than started twice.
    async fn start_backup(self: &Rc<Self>) -> Result<(), String> {
        if self.backing.borrow().is_some() {
            return Ok(());
        }
        let before = backend::unit_state().await;
        let already = backend::running(&before);
        // Followed from the moment before it starts: its first line comes before journalctl could be
        // following
        let since = format!("--since=@{}", i18n::now().floor() as i64);
        if !already {
            backend::start_backup().await?;
        }
        let unit = format!("--user-unit={}", backend::UNIT);
        let weak = self.me.clone();
        let journal = process::follow("journalctl", &["--user", &unit, "--follow", &since, "--output=cat", "--quiet"], move |line| {
            if !line.trim().is_empty() {
                if let Some(this) = weak.upgrade() {
                    this.on_backup_line(line);
                }
            }
        });
        let began = i18n::now();
        let invocation = before.get("InvocationID").cloned().unwrap_or_default();
        let checking = Rc::new(Cell::new(false));
        let weak = self.me.clone();
        let timer = glib::timeout_add_local(Duration::from_millis(1500), move || {
            let Some(this) = weak.upgrade() else { return glib::ControlFlow::Break };
            if checking.replace(true) {
                return glib::ControlFlow::Continue;
            }
            let (checking, invocation) = (checking.clone(), invocation.clone());
            glib::spawn_future_local(async move {
                let now = backend::unit_state().await;
                checking.set(false);
                // Done once the run this started (a new invocation) or the one it found has ended. A run too
                // quick to be seen running is done after a few seconds of nothing.
                let ours = already || now.get("InvocationID") != Some(&invocation) || i18n::now() - began > 8.0;
                if this.backing.borrow().is_none() || backend::running(&now) || !ours {
                    return;
                }
                this.stop_following();
                let ok = now.get("ActiveState").map(String::as_str) != Some("failed") && now.get("Result").map(String::as_str) == Some("success");
                let lines = if ok { String::new() } else { backend::last_lines().await };
                this.on_backup_done(ok, lines);
            });
            glib::ControlFlow::Continue
        });
        self.backing.replace(Some(Backing { _journal: journal, timer: Some(timer) }));
        Ok(())
    }

    fn stop_following(&self) {
        if let Some(mut backing) = self.backing.take() {
            if let Some(timer) = backing.timer.take() {
                timer.remove();
            }
        }
    }

    fn on_backup_line(&self, line: &str) {
        {
            let mut state = self.state.borrow_mut();
            let Some(BackupRun::Running(lines)) = state.backup.as_mut() else { return };
            lines.push(line.to_string());
            if lines.len() > 200 {
                lines.remove(0);
            }
        }
        self.render_lines();
    }

    fn on_backup_done(&self, ok: bool, lines: String) {
        {
            let mut state = self.state.borrow_mut();
            state.backup = Some(BackupRun::Done { ok, lines });
            state.failed_lines = None;
        }
        // Said at once, not after the status is read again, which takes restic a few seconds
        self.render();
        self.load(true, false);
    }
}
