//! Amethystora Security: the security report and a virus scanner, in a window of its own. It takes the place
//! of the report's terminal launcher and of ClamUI. backend.rs says what it reads and runs and why; page.rs
//! draws it; this holds the window, its state and the scans started from it.

mod art;
mod backend;
mod icons;
mod page;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use amethystora_ui::adw::prelude::*;
use amethystora_ui::app::{self, App, Shell, Spec};
use amethystora_ui::{gdk, gio, glib, gtk, process, t};

use backend::{Entry, Events, Found, Inventory, Label, Network, Permissions, Report, Scanner, Signatures, Switches};

pub fn main() -> glib::ExitCode {
    app::run(
        Spec {
            name: "security",
            id: "org.amethystora.Security",
            css: include_str!("security.css"),
            width: 1180,
            height: 820,
            min_width: 720,
            min_height: 540,
        },
        Security::new,
    )
}

/// What the result page is about: a scan started here, or the weekly one
#[derive(Debug, Clone)]
pub(crate) enum Shown {
    Entry(Box<Entry>),
    Machine,
}

/// What the page is told about the scan running now
#[derive(Debug, Clone, Default)]
pub(crate) struct Progress {
    pub label: Label,
    /// Milliseconds, as the history keeps it
    pub started: f64,
    pub files: f64,
    pub found: usize,
    pub current: String,
    pub loading: bool,
}

/// The scan running now, as main.js kept it
struct Scan {
    id: String,
    kind: String,
    label: Label,
    single: bool,
    targets: Vec<String>,
    /// Milliseconds
    started: f64,
    signatures: Option<Signatures>,
    files: f64,
    errors: f64,
    found: Vec<Found>,
    current: String,
    loading: bool,
    stopped: bool,
    timer: Option<glib::SourceId>,
    child: Option<gio::Subprocess>,
}

impl Scan {
    fn progress(&self) -> Progress {
        Progress {
            label: self.label.clone(),
            started: self.started,
            files: self.files,
            found: self.found.len(),
            current: self.current.clone(),
            loading: self.loading,
        }
    }
}

/// What came of asking for a scan
pub(crate) enum Begun {
    Canceled,
    Error(String),
    Started(Progress),
}

/// What the page keeps between drawings, as security.js's `state`
/// The events the Events page draws at first, and each time it is asked for more
pub(crate) const EVENTS_AT_ONCE: usize = 50;

#[derive(Default)]
pub(crate) struct PageState {
    pub page: String,
    pub events_shown: usize,
    pub report: Option<Report>,
    pub scanner: Option<Scanner>,
    /// The scan in progress, as the window last heard of it
    pub running: Option<Progress>,
    /// What the result page is about
    pub result: Option<Shown>,
    /// Why the last thing asked for did not happen
    pub notice: String,
    /// When the whole-machine scan was started from here, before systemd says it is running, in seconds
    pub machine_asked: f64,
    /// Network protection: what it blocked and noticed, and whether it runs
    pub network: Option<Network>,
    /// The containers amethystora-pkg manages
    pub inventory: Option<Inventory>,
    /// What each Flatpak can reach beyond its sandbox
    pub permissions: Option<Permissions>,
    /// Every switch: the detection ones in security.conf, and what the image enforces from the start
    pub switches: Option<Switches>,
    /// What the security watcher found, for the members of wheel
    pub events: Option<Events>,
}

pub struct Security {
    me: Weak<Security>,
    shell: Rc<Shell>,
    frame: page::Frame,
    /// /var/home/<name>
    home: String,
    // --- What main.js kept --------------------------------------------------------------------------------
    /// The scans started here, newest first
    history: RefCell<Vec<Entry>>,
    scan: RefCell<Option<Scan>>,
    /// The apps the permissions list last read, which reset and network act on
    permissions: RefCell<Vec<backend::App>>,
    // --- What security.js kept ------------------------------------------------------------------------------
    state: RefCell<PageState>,
    live: RefCell<page::Live>,
    reloading: Cell<bool>,
    machine_timer: RefCell<Option<glib::SourceId>>,
    /// The system bus, kept for as long as the window hears from systemd over it
    machine_bus: RefCell<Option<gio::DBusConnection>>,
    /// The page has been opened once: a later launch only changes pages
    opened: Cell<bool>,
    /// How many columns the grids have: three, then two and one in a narrow window
    columns: Cell<i32>,
    /// Fallback for when terminal-completion monitoring is unavailable: a change was launched, so every
    /// focus reloads until the completion callback fires and clears it (focus never consumes it).
    pending_reload: Cell<bool>,
    /// A reload was asked for while one was already running: run another afterwards, so a change always
    /// gets a read that lands after it completed
    reload_again: Cell<bool>,
}

impl App for Security {
    /// `amethystora-security scan` opens the virus scanner, `amethystora-security network` network protection,
    /// `amethystora-security apps` what each app can reach, `amethystora-security containers` what is installed
    /// outside Flatpak, `amethystora-security events` what the watcher found, `amethystora-security settings`
    /// every switch, and `amethystora-security settings/CATEGORY` one category of them
    fn open(&self, args: &[String]) {
        let page = args.iter().find(|arg| !arg.starts_with('-')).filter(|page| backend::openable(page)).cloned();
        if !self.opened.replace(true) {
            self.state.borrow_mut().page = page.unwrap_or_else(|| "overview".into());
            self.render();
            let this = self.rc();
            glib::spawn_future_local(async move { this.reload().await });
        } else if let Some(page) = page {
            self.state.borrow_mut().notice.clear();
            self.show(&page, None);
        }
    }

    /// Back from a terminal where something may have been fixed: reload if a change launched here is still
    /// unreflected (the completion-callback fallback), or if the report is old
    fn focus(&self) {
        let old = self.state.borrow().report.as_ref().is_some_and(|report| backend::now() - report.checked > 15.0);
        if self.pending_reload.get() || old {
            let this = self.rc();
            glib::spawn_future_local(async move { this.reload().await });
        }
    }

    fn reload(&self) {
        let this = self.rc();
        glib::spawn_future_local(async move { this.reload().await });
    }
}

impl Security {
    fn new(shell: Rc<Shell>) -> Rc<Security> {
        let this = Rc::new_cyclic(|me| Security {
            me: me.clone(),
            shell: shell.clone(),
            frame: page::Frame::new(me),
            home: backend::home(),
            history: RefCell::new(Vec::new()),
            scan: RefCell::new(None),
            permissions: RefCell::new(Vec::new()),
            state: RefCell::new(PageState { page: "overview".into(), events_shown: EVENTS_AT_ONCE, ..Default::default() }),
            live: RefCell::new(page::Live::default()),
            reloading: Cell::new(false),
            machine_timer: RefCell::new(None),
            machine_bus: RefCell::new(None),
            opened: Cell::new(false),
            columns: Cell::new(3),
            pending_reload: Cell::new(false),
            reload_again: Cell::new(false),
        });
        this.history.replace(backend::load_history(&this.home));
        shell.window.set_title(Some(&t!("Amethystora Security")));
        shell.window.set_content(Some(&this.frame.root));
        this.narrow();
        this.drops();
        // Nothing has the keyboard's focus until it is used, as on the page: otherwise the first command one can
        // select text in would take it, and show its caret
        shell.window.connect_map(|window| {
            let window = window.clone();
            glib::idle_add_local_once(move || gtk::prelude::GtkWindowExt::set_focus(&window, None::<&gtk::Widget>));
        });

        this.hear_machine();
        // The time a scan has run, every second
        let weak = Rc::downgrade(&this);
        glib::timeout_add_local(Duration::from_secs(1), move || {
            let Some(this) = weak.upgrade() else { return glib::ControlFlow::Break };
            this.tick();
            glib::ControlFlow::Continue
        });
        // A scan belongs to the window that started it; the whole-machine scan is systemd's and carries on
        let weak = Rc::downgrade(&this);
        shell.window.connect_close_request(move |_| {
            if let Some(this) = weak.upgrade() {
                if let Some(child) = this.scan.borrow().as_ref().and_then(|scan| scan.child.clone()) {
                    child.send_signal(libc::SIGTERM);
                }
            }
            glib::Propagation::Proceed
        });
        this
    }

    /// This window, from a method that only has &self
    fn rc(&self) -> Rc<Security> {
        self.me.upgrade().expect("the window is open")
    }

    /// Three columns, then two below 1040 pixels and one below 860, as the page's media queries had them
    fn narrow(&self) {
        for (width, class) in [(1040.0, "narrow"), (860.0, "narrower")] {
            let condition = amethystora_ui::adw::BreakpointCondition::new_length(
                amethystora_ui::adw::BreakpointConditionLengthType::MaxWidth,
                width,
                amethystora_ui::adw::LengthUnit::Px,
            );
            let breakpoint = amethystora_ui::adw::Breakpoint::new(condition);
            let weak = self.me.clone();
            breakpoint.connect_apply(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.shell.window.add_css_class(class);
                    this.relayout();
                }
            });
            let weak = self.me.clone();
            breakpoint.connect_unapply(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.shell.window.remove_css_class(class);
                    this.relayout();
                }
            });
            self.shell.window.add_breakpoint(breakpoint);
        }
    }

    fn relayout(&self) {
        let window = &self.shell.window;
        let columns = if window.has_css_class("narrower") { 1 } else if window.has_css_class("narrow") { 2 } else { 3 };
        if self.columns.replace(columns) != columns {
            // Drawn again once the breakpoint has settled, not in the middle of its change
            let weak = self.me.clone();
            glib::idle_add_local_once(move || {
                if let Some(this) = weak.upgrade() {
                    this.render();
                }
            });
        }
    }

    // --- Dropping files -------------------------------------------------------------------------------------

    /// A file or folder dropped anywhere on the window is scanned, never opened in it
    fn drops(&self) {
        let target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        let shown = self.frame.drop.clone();
        target.connect_enter(move |_, _, _| {
            shown.set_visible(true);
            gdk::DragAction::COPY
        });
        let hidden = self.frame.drop.clone();
        target.connect_leave(move |_| hidden.set_visible(false));
        let weak = self.me.clone();
        let hidden = self.frame.drop.clone();
        target.connect_drop(move |_, value, _, _| {
            hidden.set_visible(false);
            let Some(this) = weak.upgrade() else { return false };
            let Ok(files) = value.get::<gdk::FileList>() else { return false };
            // A dropped file's path is only known here, as a file of this machine
            let paths: Vec<String> = files.files().iter().filter_map(|file| file.path()).map(|path| path.to_string_lossy().into_owned()).collect();
            if !paths.is_empty() {
                let begin = this.clone();
                glib::spawn_future_local(async move {
                    let scanning = begin.clone();
                    begin.begin(async move { scanning.scan_paths(paths).await }).await;
                });
            }
            true
        });
        self.shell.window.add_controller(target);
    }

    // --- Scans started here -----------------------------------------------------------------------------------

    fn progress(&self) -> Option<Progress> {
        self.scan.borrow().as_ref().map(Scan::progress)
    }

    async fn start_scan(&self, kind: &str, targets: Vec<String>, label: Label) -> Begun {
        if self.scan.borrow().is_some() {
            return Begun::Error(t!("A scan is already running."));
        }
        if backend::scanner_off().await {
            return Begun::Error(t!("The virus scanner is turned off. Turn it on in Settings first."));
        }
        let Some(signatures) = backend::signature_age().await else {
            return Begun::Error(t!(
                "There are no virus signatures to scan with yet. They download on their own soon after the machine is installed."
            ));
        };
        // Asked twice while the two questions above were being answered
        if self.scan.borrow().is_some() {
            return Begun::Error(t!("A scan is already running."));
        }
        // Gone already: the scanner says so
        let single = targets.len() == 1 && std::fs::metadata(&targets[0]).map(|m| m.is_file()).unwrap_or(false);
        let scan = Scan {
            id: backend::scan_id(),
            kind: kind.to_string(),
            label,
            single,
            targets,
            started: (backend::now() * 1000.0).floor(),
            signatures: Some(signatures),
            files: 0.0,
            errors: 0.0,
            found: Vec::new(),
            current: String::new(),
            loading: false,
            stopped: false,
            timer: None,
            child: None,
        };
        let progress = scan.progress();
        self.scan.replace(Some(scan));
        self.start_scanner(backend::daemon_reachable());
        Begun::Started(self.progress().unwrap_or(progress))
    }

    fn start_scanner(&self, daemon: bool) {
        let targets = self.scan.borrow().as_ref().map(|scan| scan.targets.clone()).unwrap_or_default();
        // Paths are absolute, so none of them can be taken for an option
        let mut argv: Vec<String> = if daemon {
            // --fdpass opens each file here, as the user, and hands clamd the open file: clamd runs as its own
            // user and could not read into a home directory itself
            ["clamdscan", "--fdpass", "--multiscan", "--no-summary"].iter().map(|s| s.to_string()).collect()
        } else {
            let mut argv: Vec<String> = ["clamscan", "--recursive", "--no-summary"].iter().map(|s| s.to_string()).collect();
            argv.extend(backend::exclusions());
            argv
        };
        argv.extend(targets);
        let errors: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let os: Vec<&std::ffi::OsStr> = argv.iter().map(std::ffi::OsStr::new).collect();
        let process = match gio::Subprocess::newv(&os, gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE) {
            Ok(process) => process,
            Err(error) => {
                errors.borrow_mut().push(error.message().to_string());
                // As the child process's error event came: after this call has returned
                let weak = self.me.clone();
                glib::idle_add_local_once(move || {
                    if let Some(this) = weak.upgrade() {
                        this.scanner_ended(daemon, -1, errors);
                    }
                });
                return;
            }
        };
        if let Some(scan) = self.scan.borrow_mut().as_mut() {
            scan.child = Some(process.clone());
            // clamscan says nothing until it has loaded every signature, which takes a while
            scan.loading = !daemon;
        }
        let stderr = process.stderr_pipe().map(|pipe| {
            let errors = errors.clone();
            glib::spawn_future_local(async move {
                read_lines(pipe, |line| errors.borrow_mut().push(line.to_string())).await;
            })
        });
        let weak = self.me.clone();
        let stdout = process.stdout_pipe();
        glib::spawn_future_local(async move {
            if let Some(pipe) = stdout {
                let reader = weak.clone();
                read_lines(pipe, move |line| {
                    if let Some(this) = reader.upgrade() {
                        this.read_line(line);
                    }
                })
                .await;
            }
            if let Some(stderr) = stderr {
                let _ = stderr.await;
            }
            let _ = process.wait_future().await;
            // A signal is no status, as the child process's close event had it
            let code = if process.has_exited() { process.exit_status() } else { -1 };
            if let Some(this) = weak.upgrade() {
                this.scanner_ended(daemon, code, errors);
            }
        });
    }

    fn scanner_ended(&self, daemon: bool, code: i32, errors: Rc<RefCell<Vec<String>>>) {
        let retry = self.scan.borrow().as_ref().is_some_and(|scan| daemon && !scan.stopped && scan.files == 0.0 && code != 0 && code != 1);
        // clamd not running, or its socket refusing the connection: scan without it instead
        if retry {
            self.start_scanner(false);
            return;
        }
        let errors = errors.borrow().clone();
        let this = self.rc();
        glib::spawn_future_local(async move { this.finish_scan(code, errors).await });
    }

    fn read_line(&self, line: &str) {
        let schedule = {
            let mut scan = self.scan.borrow_mut();
            let Some(scan) = scan.as_mut() else { return };
            if line.ends_with(" ERROR") {
                scan.errors += 1.0;
                return;
            }
            // Empty files, links it does not follow, what it leaves out
            let Some((path, signature)) = backend::parse_result(line) else { return };
            scan.loading = false;
            scan.files += 1.0;
            scan.current = path.clone();
            if let Some(signature) = signature {
                if scan.found.len() < backend::FINDINGS_KEPT && backend::within(&path, &scan.targets) {
                    let credit = backend::yara_credit(&signature);
                    scan.found.push(Found { path, signature, credit, outcome: None });
                }
            }
            scan.timer.is_none()
        };
        if schedule {
            let weak = self.me.clone();
            let source = glib::timeout_add_local_once(Duration::from_millis(150), move || {
                let Some(this) = weak.upgrade() else { return };
                let progress = {
                    let mut scan = this.scan.borrow_mut();
                    let Some(scan) = scan.as_mut() else { return };
                    scan.timer = None;
                    scan.progress()
                };
                this.on_progress(progress);
            });
            if let Some(scan) = self.scan.borrow_mut().as_mut() {
                scan.timer = Some(source);
            }
        }
    }

    async fn finish_scan(&self, code: i32, errors: Vec<String>) {
        let Some(mut entry) = ({
            let mut scan = self.scan.borrow_mut();
            scan.as_mut().map(|scan| {
                if let Some(timer) = scan.timer.take() {
                    timer.remove();
                }
                // Some files unreadable is exit status 2 as well, so only a scan that got nowhere failed
                let failed = !scan.stopped && scan.files == 0.0 && code != 0 && code != 1;
                let status = if !scan.found.is_empty() || code == 1 {
                    "found"
                } else if scan.stopped {
                    "stopped"
                } else if failed {
                    "failed"
                } else {
                    "clean"
                };
                Entry {
                    id: scan.id.clone(),
                    kind: scan.kind.clone(),
                    label: scan.label.clone(),
                    single: scan.single,
                    targets: scan.targets.clone(),
                    started: scan.started,
                    seconds: ((backend::now() * 1000.0 - scan.started) / 1000.0).round(),
                    files: scan.files,
                    errors: scan.errors,
                    signatures: scan.signatures.clone(),
                    found: scan.found.clone(),
                    stopped: scan.stopped,
                    // What it found is what matters, even of a scan stopped before the end
                    status: status.into(),
                    message: failed.then(|| {
                        errors
                            .iter()
                            .find(|line| !line.trim().is_empty())
                            .cloned()
                            .unwrap_or_else(|| t!("The scanner stopped with status {code}.", code = code.to_string()))
                    }),
                    ..Default::default()
                }
            })
        }) else {
            return;
        };
        backend::act(&mut entry).await;
        self.scan.replace(None);
        {
            let mut history = self.history.borrow_mut();
            history.retain(|item| item.id != entry.id);
            history.insert(0, entry.clone());
            history.truncate(backend::HISTORY_LENGTH);
            backend::save_history(&history);
        }
        self.on_finished(entry);
    }

    pub(crate) fn stop(&self) {
        if let Some(scan) = self.scan.borrow_mut().as_mut() {
            if let Some(child) = &scan.child {
                scan.stopped = true;
                child.send_signal(libc::SIGTERM);
            }
        }
    }

    pub(crate) async fn scan_home(&self) -> Begun {
        let targets = vec![self.home.clone()];
        let label = backend::label_for(&targets, &self.home);
        self.start_scan("home", targets, label).await
    }

    pub(crate) async fn scan_pick(&self, folders: bool) -> Begun {
        let dialog = gtk::FileDialog::builder()
            .title(if folders { t!("Scan folders") } else { t!("Scan files") })
            .accept_label(t!("Scan"))
            .modal(true)
            .initial_folder(&gio::File::for_path(&self.home))
            .build();
        let window = self.shell.gtk_window();
        let chosen = if folders { dialog.select_multiple_folders_future(Some(&window)).await } else { dialog.open_multiple_future(Some(&window)).await };
        let Ok(list) = chosen else { return Begun::Canceled };
        let paths: Vec<String> = (0..list.n_items())
            .filter_map(|at| list.item(at).and_downcast::<gio::File>())
            .filter_map(|file| file.path())
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        if paths.is_empty() {
            return Begun::Canceled;
        }
        let label = backend::label_for(&paths, &self.home);
        self.start_scan("pick", paths, label).await
    }

    pub(crate) async fn scan_paths(&self, paths: Vec<String>) -> Begun {
        let targets = backend::existing(&paths);
        if targets.is_empty() {
            return Begun::Error(t!("Nothing that was dropped is a file or folder on this machine."));
        }
        let label = backend::label_for(&targets, &self.home);
        self.start_scan("pick", targets, label).await
    }

    pub(crate) async fn scan_again(&self, id: &str) -> Begun {
        let Some(entry) = self.history.borrow().iter().find(|item| item.id == id).cloned() else {
            return Begun::Error(t!("That scan is no longer in the list."));
        };
        let targets = backend::existing(&entry.targets);
        if targets.is_empty() {
            let label = &entry.label;
            return Begun::Error(t!(
                "{place, select, home {Your home folder is not there any more.} items {{items, plural, one {# item is} other {# items are}} not there any more.} other {{name} is not there any more.}}",
                place = label.place.as_str(),
                items = label.items.unwrap_or(f64::NAN),
                name = label.name.clone().unwrap_or_default()
            ));
        }
        self.start_scan(&entry.kind, targets, entry.label.clone()).await
    }

    pub(crate) fn dismiss(&self, id: &str) {
        let mut history = self.history.borrow_mut();
        if let Some(entry) = history.iter_mut().find(|item| item.id == id) {
            entry.dismissed = true;
            backend::save_history(&history);
        }
    }

    /// Only a file a scan here found, and that is still there
    pub(crate) fn show_in_files(&self, file: &str) {
        let known = self.history.borrow().iter().any(|entry| entry.found.iter().any(|item| item.path == file));
        if known && std::path::Path::new(file).exists() {
            let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(file)));
            launcher.open_containing_folder(Some(&self.shell.gtk_window()), gio::Cancellable::NONE, |_| {});
        }
    }

    // --- What the page hears ----------------------------------------------------------------------------------

    fn on_progress(&self, progress: Progress) {
        self.state.borrow_mut().running = Some(progress);
        let live = self.state.borrow().page == "scanning" && self.live.borrow().files.is_some();
        if live {
            self.update_live();
        } else {
            self.render_nav();
        }
    }

    fn on_finished(&self, entry: Entry) {
        self.state.borrow_mut().running = None;
        if self.state.borrow().page == "scanning" {
            self.show("result", Some(Shown::Entry(Box::new(entry))));
        } else {
            self.render();
        }
    }

    // --- Talking to the machine ---------------------------------------------------------------------------------

    /// A scan asked for: what went wrong is said on the scanning page's way in
    pub(crate) async fn begin(&self, request: impl std::future::Future<Output = Begun>) {
        self.state.borrow_mut().notice.clear();
        match request.await {
            Begun::Canceled => {}
            Begun::Error(error) => {
                // A second drop while the first one's scan starts is refused, and that scan stays in view
                let running = self.scan.borrow().is_some();
                self.state.borrow_mut().notice = error;
                self.show(if running { "scanning" } else { "scan" }, None);
            }
            Begun::Started(progress) => {
                self.state.borrow_mut().running = Some(progress);
                self.show("scanning", None);
            }
        }
    }

    /// A whole-machine scan started anywhere else: systemd says when its unit changes, which costs nothing between
    /// changes, and the page then asks after it
    fn hear_machine(&self) {
        let weak = self.me.clone();
        glib::spawn_future_local(async move {
            let Ok(bus) = gio::bus_get_future(gio::BusType::System).await else { return };
            // systemd says a unit changed only once somebody has asked it to
            let _ = bus
                .call_future(
                    Some("org.freedesktop.systemd1"),
                    "/org/freedesktop/systemd1",
                    "org.freedesktop.systemd1.Manager",
                    "Subscribe",
                    None,
                    None,
                    gio::DBusCallFlags::NONE,
                    -1,
                )
                .await;
            let heard = weak.clone();
            #[allow(deprecated)] // the stream form would need the app to poll it for a callback this small
            bus.signal_subscribe(
                Some("org.freedesktop.systemd1"),
                Some("org.freedesktop.DBus.Properties"),
                Some("PropertiesChanged"),
                Some(backend::MACHINE_UNIT_PATH),
                Some("org.freedesktop.systemd1.Unit"),
                gio::DBusSignalFlags::NONE,
                move |_, _, _, _, _, _| {
                    let Some(this) = heard.upgrade() else { return };
                    if this.machine_timer.borrow().is_none() {
                        this.watch_machine();
                    }
                },
            );
            if let Some(this) = weak.upgrade() {
                this.machine_bus.replace(Some(bus));
            }
        });
    }

    /// The whole-machine scan is systemd's, so the page asks after it rather than being told
    pub(crate) fn watch_machine(&self) {
        if let Some(timer) = self.machine_timer.take() {
            timer.remove();
        }
        let weak = self.me.clone();
        let source = glib::timeout_add_local_once(Duration::from_secs(3), move || {
            let Some(this) = weak.upgrade() else { return };
            this.machine_timer.take();
            glib::spawn_future_local(async move {
                let was_running = this.state.borrow().scanner.as_ref().is_some_and(|s| s.machine.running);
                let scanner = backend::scanner().await;
                let running = scanner.machine.running;
                this.state.borrow_mut().scanner = Some(scanner);
                if running {
                    // Started elsewhere: the page says so as well as the sidebar
                    if was_running {
                        this.render_nav();
                    } else {
                        this.render();
                    }
                    this.watch_machine();
                    return;
                }
                if was_running {
                    let report = backend::report().await;
                    this.state.borrow_mut().report = Some(report);
                    let scanning = {
                        let state = this.state.borrow();
                        state.page == "scanning" && state.running.is_none()
                    };
                    if scanning {
                        this.show("result", Some(Shown::Machine));
                        return;
                    }
                }
                this.render();
            });
        });
        self.machine_timer.replace(Some(source));
    }

    pub(crate) async fn scan_machine(&self) {
        self.state.borrow_mut().notice.clear();
        if let Err(error) = backend::scan_machine().await {
            self.state.borrow_mut().notice = error;
            self.show("scan", None);
            return;
        }
        let mut scanner = backend::scanner().await;
        // systemctl --no-block returns before systemd has started the unit
        scanner.machine.running = true;
        {
            let mut state = self.state.borrow_mut();
            state.machine_asked = backend::now();
            state.scanner = Some(scanner);
        }
        self.show("scanning", None);
        self.watch_machine();
    }

    /// Launch a switch or profile change in a terminal, and reload the page the moment the command
    /// finishes, so the UI reflects the new state at once rather than only on the next focus
    pub(crate) fn change(&self, kind: backend::Kind, key: &str, value: &str, argument: &str) {
        let Some(command) = backend::change_command(self.state.borrow().switches.as_ref(), kind, key, value, argument) else {
            return;
        };
        let args: Vec<&str> = command.iter().map(String::as_str).collect();
        // Primary: reload the instant the command finishes. Fallback (if terminal-completion monitoring is
        // unavailable, when in_terminal_then never calls back): the next focus reloads, via pending_reload.
        self.pending_reload.set(true);
        let weak = self.me.clone();
        process::in_terminal_then(&args, move || {
            if let Some(this) = weak.upgrade() {
                this.pending_reload.set(false);
                glib::spawn_future_local(async move { this.reload().await });
            }
        });
    }

    /// Everything read again, once at a time. A reload requested while one is running is queued (via
    /// reload_again) and run afterwards, so a change always gets a read that lands after it completed.
    pub(crate) async fn reload(&self) {
        if self.reloading.replace(true) {
            self.reload_again.set(true);
            return;
        }
        loop {
            self.reload_again.set(false);
            let report = glib::spawn_future_local(backend::report());
            let scanner = glib::spawn_future_local(backend::scanner());
            let network = glib::spawn_future_local(backend::network());
            let inventory = glib::spawn_future_local(backend::inventory());
            let permissions = glib::spawn_future_local(backend::permissions());
            let switches = glib::spawn_future_local(backend::switches());
            let events = glib::spawn_future_local(backend::events());
            let report = report.await.unwrap_or_default();
            let scanner = scanner.await.unwrap_or_default();
            let network = network.await.unwrap_or_default();
            let inventory = inventory.await.unwrap_or_default();
            let permissions = permissions.await.unwrap_or_default();
            let switches = switches.await.unwrap_or_default();
            let events = events.await.unwrap_or_default();
            self.permissions.replace(permissions.apps.clone());
            let machine_running = scanner.machine.running;
            {
                let mut state = self.state.borrow_mut();
                state.report = Some(report);
                state.scanner = Some(scanner);
                state.network = Some(network);
                state.inventory = Some(inventory);
                state.permissions = Some(permissions);
                state.switches = Some(switches);
                state.events = Some(events);
                if let Some(progress) = self.progress() {
                    state.running = Some(progress);
                }
            }
            if machine_running {
                self.watch_machine();
            }
            self.render();
            if !self.reload_again.get() {
                break;
            }
        }
        self.reloading.set(false);
    }

    /// The permissions list read again after a change, for both the page and what acts on it
    pub(crate) async fn reread_permissions(&self) -> Permissions {
        let permissions = backend::permissions().await;
        self.permissions.replace(permissions.apps.clone());
        permissions
    }

    pub(crate) fn permission_apps(&self) -> Vec<backend::App> {
        self.permissions.borrow().clone()
    }

    fn tick(&self) {
        let (label, since) = {
            let live = self.live.borrow();
            (live.elapsed.clone(), live.since)
        };
        if let (Some(label), Some(since)) = (label, since) {
            label.set_label(&page::elapsed(since));
        }
    }
}

/// The lines a pipe gives, as readline split them, until it closes
async fn read_lines(pipe: gio::InputStream, mut on_line: impl FnMut(&str)) {
    let reader = gio::DataInputStream::new(&pipe);
    reader.set_newline_type(gio::DataStreamNewlineType::Any);
    while let Ok(Some(line)) = reader.read_line_future(glib::Priority::DEFAULT).await {
        on_line(&String::from_utf8_lossy(&line));
    }
}
