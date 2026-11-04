//! Amethystora Updates: the system, the apps and the command-line tools brought up to date in a window of
//! its own. It takes the place of upstream's System Update launcher, a terminal running `ujust update`;
//! `ame update` stays, for the terminal. backend.rs says what it runs and why; page.rs draws it.

mod backend;
mod icons;
mod page;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use amethystora_ui::app::{self, App, Shell, Spec};
use amethystora_ui::process::{self, Follow};
use amethystora_ui::{glib, gtk};
use amethystora_ui::adw::prelude::*;

use backend::{ContainersRun, Containers, Recent, Status, Step, Update};

pub fn main() -> glib::ExitCode {
    app::run(
        Spec {
            name: "update",
            id: "org.amethystora.Updates",
            css: include_str!("update.css"),
            width: 980,
            height: 800,
            min_width: 640,
            min_height: 560,
        },
        Updates::new,
    )
}

/// What the page keeps between drawings, as update.js's `state`
#[derive(Default)]
struct PageState {
    status: Option<Status>,
    /// "Update now" was pressed and the update has not been seen yet
    starting: bool,
    /// Why the last thing asked for did not happen
    notice: String,
    /// Automatic updates are being switched, and GNOME is asking for the password
    switching: bool,
    /// "Keep this version" is waiting for the password, and then has been done
    keeping: bool,
    kept: bool,
    /// "Restart now" is waiting to be confirmed, for a soft reboot, and then is on its way
    confirming: bool,
    restarting: bool,
    /// The updater's log is open
    details: bool,
    /// LVFS is being asked again for firmware
    checking_firmware: bool,
    /// Tonight's restart is being set or taken away
    arming: bool,
    /// Check now is looking for what is waiting
    looking: bool,
    /// Opened as `amethystora-update update` (the pill's Update anyway): the update starts once the status is in
    wants_update: bool,
    /// When the status was last read
    loaded: f64,
}

pub struct Updates {
    me: Weak<Updates>,
    shell: Rc<Shell>,
    view: gtk::Box,
    // --- What main.js kept --------------------------------------------------------------------------------
    /// The update being followed, or the last one that ended while the window was open
    update: RefCell<Option<Update>>,
    serial: Cell<u64>,
    journal: RefCell<Option<Follow>>,
    /// When "Update now" started one that has not been seen running yet
    requested: Cell<f64>,
    ending: RefCell<Option<glib::SourceId>>,
    watching: Cell<bool>,
    /// "Update now" was pressed, so the containers follow the update it started or found running
    containers_wanted: Cell<bool>,
    containers_journal: RefCell<Option<Follow>>,
    last_containers: RefCell<Option<Containers>>,
    last_recent: RefCell<Option<Recent>>,
    send_timer: RefCell<Option<glib::SourceId>>,
    // --- What update.js kept ------------------------------------------------------------------------------
    state: RefCell<PageState>,
    live: RefCell<page::Live>,
}

impl App for Updates {
    fn focus(&self) {
        // Back from elsewhere, where the machine may have been updated or restarted into a new version
        let this = self.rc();
        if this.hero_kind() != "running" && !self.state.borrow().switching && backend::now() - self.state.borrow().loaded > 15.0 {
            glib::spawn_future_local(async move { this.reload().await });
        }
    }

    fn reload(&self) {
        let this = self.rc();
        glib::spawn_future_local(async move { this.reload().await });
    }

    /// `amethystora-update update`: Update anyway, from the pill's Saving data. An update already running is followed.
    fn open(&self, args: &[String]) {
        if args.first().map(String::as_str) != Some("update") {
            return;
        }
        let this = self.rc();
        if this.state.borrow().status.is_some() {
            if this.hero_kind() != "running" {
                this.start_update();
            }
        } else {
            this.state.borrow_mut().wants_update = true;
        }
    }
}

impl Updates {
    fn new(shell: Rc<Shell>) -> Rc<Updates> {
        let view = gtk::Box::new(gtk::Orientation::Vertical, 30);
        let this = Rc::new_cyclic(|me| Updates {
            me: me.clone(),
            shell: shell.clone(),
            view,
            update: RefCell::new(None),
            serial: Cell::new(0),
            journal: RefCell::new(None),
            requested: Cell::new(0.0),
            ending: RefCell::new(None),
            watching: Cell::new(false),
            containers_wanted: Cell::new(false),
            containers_journal: RefCell::new(None),
            last_containers: RefCell::new(None),
            last_recent: RefCell::new(None),
            send_timer: RefCell::new(None),
            state: RefCell::new(PageState::default()),
            live: RefCell::new(page::Live::default()),
        });
        shell.window.set_title(Some(&amethystora_ui::t!("Amethystora Updates")));
        shell.window.set_content(Some(&this.frame()));
        shell.narrow(720.0, "narrow");
        this.render();
        let start = this.clone();
        glib::spawn_future_local(async move {
            start.reload().await;
        });
        // Picks up an update that starts by itself while the window is open, and notices when one ends
        let watcher = this.clone();
        glib::spawn_future_local(async move { watcher.watch().await });
        let weak = Rc::downgrade(&this);
        glib::timeout_add_local(Duration::from_secs(2), move || {
            let Some(this) = weak.upgrade() else { return glib::ControlFlow::Break };
            glib::spawn_future_local(async move { this.watch().await });
            glib::ControlFlow::Continue
        });
        // The update is systemd's and carries on without the window
        let weak = Rc::downgrade(&this);
        shell.window.connect_close_request(move |_| {
            if let Some(this) = weak.upgrade() {
                this.journal.take();
                this.containers_journal.take();
            }
            glib::Propagation::Proceed
        });
        this
    }

    /// This window, from a method that only has &self
    fn rc(&self) -> Rc<Updates> {
        self.me.upgrade().expect("the window is open")
    }

    async fn status(&self) -> Status {
        let machine = glib::spawn_future_local(backend::machine());
        let automatic = glib::spawn_future_local(backend::automatic());
        let units = glib::spawn_future_local(backend::units());
        let containers = glib::spawn_future_local(backend::containers());
        let agent = glib::spawn_future_local(backend::agent_on());
        let firmware = glib::spawn_future_local(backend::firmware());
        let night = glib::spawn_future_local(backend::night());
        let metered = glib::spawn_future_local(backend::metered());
        let machine = machine.await.ok().flatten();
        let booted = machine.as_ref().map(|m| m.running.checksum.clone()).unwrap_or_default();
        let waiting = glib::spawn_future_local(async move { backend::waiting(&booted).await });
        let status = Status {
            machine,
            automatic: automatic.await.unwrap_or_default(),
            recent: backend::recent(&units.await.unwrap_or_default()),
            containers: containers.await.unwrap_or_default(),
            agent: agent.await.unwrap_or(false),
            firmware: firmware.await.unwrap_or_default(),
            night: night.await.unwrap_or_default(),
            metered: metered.await.unwrap_or_default(),
            waiting: waiting.await.ok().flatten(),
        };
        self.last_containers.replace(Some(status.containers.clone()));
        self.last_recent.replace(status.recent.clone());
        status
    }

    async fn reload(&self) {
        self.state.borrow_mut().loaded = backend::now();
        let status = self.status().await;
        self.state.borrow_mut().status = Some(status);
        self.render();
        let wanted = std::mem::take(&mut self.state.borrow_mut().wants_update);
        if wanted && self.hero_kind() != "running" {
            self.rc().start_update();
        }
    }

    /// The update that failed, followed here or found ended, handed to the agent by its unit's name, which
    /// it looks up in the journal itself. Only ever one of the two updater units.
    fn diagnose(&self) {
        let unit = match &*self.update.borrow() {
            Some(update) if update.state == "failed" => Some(update.unit.clone()),
            _ => self.last_recent.borrow().as_ref().filter(|recent| recent.result != "success").map(|recent| recent.unit.clone()),
        };
        if let Some(unit) = unit.filter(|unit| backend::UNITS.contains(&unit.as_str())) {
            process::launch(backend::AGENT, &["diagnose", &unit]);
        }
    }

    // --- The update ---------------------------------------------------------------------------------------

    /// bootc reports its progress many times a second: the page hears of it a few times a second
    fn send_update(&self) {
        if self.send_timer.borrow().is_some() {
            return;
        }
        let weak = Rc::downgrade(&self.rc());
        let source = glib::timeout_add_local_once(Duration::from_millis(150), move || {
            if let Some(this) = weak.upgrade() {
                this.send_timer.take();
                this.on_update();
            }
        });
        self.send_timer.replace(Some(source));
    }

    fn stop_journal(&self) {
        self.journal.take();
    }

    /// Follow one run of an updater unit, from its first line on
    fn follow(&self, unit: &backend::Unit) {
        self.stop_journal();
        if let Some(source) = self.ending.take() {
            source.remove();
        }
        self.requested.set(0.0);
        let id = unit.get("Id").cloned().unwrap_or_default();
        let invocation = unit.get("InvocationID").cloned().unwrap_or_default();
        let retry = self.update.borrow().as_ref().is_some_and(|previous| {
            previous.unit == id && previous.state == "failed" && backend::now() - previous.finished.unwrap_or(0.0) < 5.0 * 60.0
        });
        self.serial.set(self.serial.get() + 1);
        let mut update = Update {
            serial: self.serial.get(),
            unit: id.clone(),
            invocation: invocation.clone(),
            automatic: id != backend::MANUAL_UNIT,
            retry,
            state: "running".into(),
            steps: backend::MODULES.iter().map(|name| (name.to_string(), Step { state: "waiting".into(), ..Default::default() })).collect(),
            ..Default::default()
        };
        if self.containers_wanted.get() && self.last_containers.borrow().as_ref().is_some_and(|c| c.count > 0) {
            update.steps.push(("Distrobox".into(), Step { state: "waiting".into(), ..Default::default() }));
        }
        self.update.replace(Some(update));
        if !invocation.is_empty() {
            let weak = Rc::downgrade(&self.rc());
            let follow = process::follow(
                "journalctl",
                &["--quiet", "--follow", "--no-tail", "--output=cat", &format!("_SYSTEMD_UNIT={id}"), &format!("_SYSTEMD_INVOCATION_ID={invocation}")],
                move |line| {
                    let Some(this) = weak.upgrade() else { return };
                    let changed = {
                        let mut update = this.update.borrow_mut();
                        let Some(update) = update.as_mut() else { return };
                        // Something came through: the journal is readable here
                        update.journal = true;
                        backend::read_line(update, line)
                    };
                    if changed {
                        this.send_update();
                    }
                },
            );
            self.journal.replace(follow);
        }
        self.on_update();
    }

    /// The unit has stopped: wait a moment for the journal to hand over its last lines, then say how it went
    fn end(&self, unit: Option<backend::Unit>) {
        if self.ending.borrow().is_some() {
            return;
        }
        let serial = self.update.borrow().as_ref().map(|u| u.serial).unwrap_or(0);
        let weak = Rc::downgrade(&self.rc());
        let source = glib::timeout_add_local_once(Duration::from_millis(1200), move || {
            let Some(this) = weak.upgrade() else { return };
            this.ending.take();
            this.stop_journal();
            let start_containers = {
                let mut current = this.update.borrow_mut();
                let Some(current) = current.as_mut().filter(|u| u.serial == serial) else { return };
                let result = unit.as_ref().and_then(|u| u.get("Result").cloned()).unwrap_or_default();
                let succeeded = if !current.ended.is_empty() { current.ended == "success" } else { result == "success" };
                current.state = if succeeded && current.failures.is_empty() { "done" } else { "failed" }.into();
                current.finished = Some(backend::now());
                // systemd runs a failed update again a minute later
                current.retrying = unit.as_ref().and_then(|u| u.get("SubState")).map(String::as_str) == Some("auto-restart");
                let ended = !current.ended.is_empty();
                for (_, step) in current.steps.iter_mut() {
                    if step.state == "running" {
                        // uupd moves on to the next module whatever happens, so a module still running when
                        // it stopped finished only if uupd got to the end
                        step.state = if ended { "done" } else { "failed" }.into();
                    }
                    step.percent = None;
                }
                let start = this.containers_wanted.get() && current.step("Distrobox").is_some();
                this.containers_wanted.set(false);
                start
            };
            glib::spawn_future_local(async move {
                let status = this.status().await;
                this.on_status(status);
                if start_containers {
                    this.start_containers(serial).await;
                }
            });
        });
        self.ending.replace(Some(source));
    }

    // --- The containers -----------------------------------------------------------------------------------

    fn stop_containers(&self) {
        self.containers_journal.take();
    }

    /// The account's own unit, which upgrades every container amethystora-pkg manages, followed through the
    /// line it logs for each one
    async fn start_containers(&self, serial: u64) {
        let since = backend::now().floor() as i64 - 1;
        {
            let mut update = self.update.borrow_mut();
            let Some(current) = update.as_mut().filter(|u| u.serial == serial) else { return };
            let count = self.last_containers.borrow().as_ref().map(|c| c.count).unwrap_or(0);
            current.containers = Some(ContainersRun { state: "running".into(), started: backend::now(), count, ..Default::default() });
            if let Some(step) = current.step_mut("Distrobox") {
                *step = Step { state: "running".into(), ..Default::default() };
            }
        }
        self.stop_containers();
        let weak = Rc::downgrade(&self.rc());
        let follow = process::follow(
            "journalctl",
            &["--user", "--quiet", "--follow", "--no-tail", "--output=cat", &format!("--since=@{since}"), &format!("SYSLOG_IDENTIFIER={}", backend::CONTAINERS_LOG)],
            move |line| {
                let Some(this) = weak.upgrade() else { return };
                let (changed, finished) = {
                    let mut update = this.update.borrow_mut();
                    let Some(current) = update.as_mut().filter(|u| u.serial == serial) else { return };
                    // Its lines go to the log only while it is the update the page follows
                    backend::read_containers(current, line, true)
                };
                if finished {
                    // Dropped later, not from inside its own reading
                    let weak = Rc::downgrade(&this);
                    glib::idle_add_local_once(move || {
                        if let Some(this) = weak.upgrade() {
                            this.stop_containers();
                        }
                    });
                }
                if changed {
                    this.send_update();
                }
            },
        );
        self.containers_journal.replace(follow);
        let output = process::run("systemctl", &["--user", "start", "--no-block", backend::CONTAINERS_UNIT]).await;
        if output.code != 0 {
            if let Some(current) = self.update.borrow_mut().as_mut().filter(|u| u.serial == serial) {
                if let Some(boxes) = current.containers.as_mut() {
                    boxes.state = "failed".into();
                }
                if let Some(step) = current.step_mut("Distrobox") {
                    step.state = "failed".into();
                }
            }
            self.stop_containers();
        }
        self.send_update();
    }

    /// A unit that ended without its last line, killed or failed before it could say: how systemd saw it
    async fn check_containers(&self) {
        let (serial, started) = match &*self.update.borrow() {
            Some(update) => match &update.containers {
                Some(boxes) if boxes.state == "running" => (update.serial, boxes.started),
                _ => return,
            },
            None => return,
        };
        if backend::now() - started < 5.0 {
            return;
        }
        let output = process::run("systemctl", &["--user", "show", backend::CONTAINERS_UNIT, "-p", "ActiveState", "-p", "Result"]).await;
        let mut active = String::new();
        let mut result = String::new();
        for line in output.stdout.lines() {
            if let Some(value) = line.strip_prefix("ActiveState=") {
                active = value.to_string();
            } else if let Some(value) = line.strip_prefix("Result=") {
                result = value.to_string();
            }
        }
        if active == "inactive" || active == "failed" {
            // Its last line may still be on its way from the journal
            let weak = Rc::downgrade(&self.rc());
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                let Some(this) = weak.upgrade() else { return };
                let changed = {
                    let mut update = this.update.borrow_mut();
                    let Some(current) = update.as_mut().filter(|u| u.serial == serial) else { return };
                    match current.containers.as_mut() {
                        Some(boxes) if boxes.state == "running" => {
                            boxes.state = if result == "success" { "done" } else { "failed" }.into();
                            let state = boxes.state.clone();
                            if let Some(step) = current.step_mut("Distrobox") {
                                step.state = state;
                            }
                            true
                        }
                        _ => false,
                    }
                };
                if changed {
                    this.stop_containers();
                    this.send_update();
                }
            });
        }
    }

    async fn watch(&self) {
        if self.watching.get() {
            return;
        }
        self.watching.set(true);
        let list = backend::units().await;
        let requested = self.requested.get();
        // One that ended in the two seconds between two looks is only seen by when it started
        let candidate = list.iter().find(|unit| backend::running(unit)).or_else(|| {
            (requested > 0.0)
                .then(|| {
                    list.iter().find(|unit| {
                        unit.get("Id").map(String::as_str) == Some(backend::MANUAL_UNIT)
                            && backend::unix_time(unit.get("ExecMainStartTimestamp")).unwrap_or(0.0) >= requested - 2.0
                    })
                })
                .flatten()
        });
        let (invocation, running_now) = match &*self.update.borrow() {
            Some(update) => (Some(update.invocation.clone()), update.state == "running"),
            None => (None, false),
        };
        // A new run is followed once the last one has been told apart from it
        if let Some(candidate) = candidate.filter(|c| c.get("InvocationID") != invocation.as_ref() && !running_now) {
            self.follow(candidate);
        } else if requested > 0.0 && backend::now() - requested > 30.0 {
            // Started, and never seen running: systemd dropped the job
            self.requested.set(0.0);
            self.containers_wanted.set(false);
            let status = self.status().await;
            self.on_status(status);
        }
        let followed = self.update.borrow().as_ref().filter(|u| u.state == "running").map(|u| (u.unit.clone(), u.invocation.clone()));
        if let Some((unit_id, invocation)) = followed {
            let unit = list.iter().find(|item| item.get("Id") == Some(&unit_id));
            let same = unit.is_some_and(|u| u.get("InvocationID") == Some(&invocation));
            if !same || !unit.is_some_and(backend::running) {
                self.end(if same { unit.cloned() } else { None });
            }
        }
        self.check_containers().await;
        self.watching.set(false);
    }

    async fn start(&self) -> Result<(), String> {
        // The containers are upgraded once the update this finds or starts has finished
        self.containers_wanted.set(true);
        let list = backend::units().await;
        if list.iter().any(backend::running) {
            // Already running: follow that one
            self.watch().await;
            return Ok(());
        }
        let output = process::run("systemctl", &["start", "--no-block", backend::MANUAL_UNIT]).await;
        if output.code != 0 {
            self.containers_wanted.set(false);
            let error = output.stderr.trim();
            return Err(if error.is_empty() { amethystora_ui::t!("The update could not be started.") } else { error.to_string() });
        }
        self.requested.set(backend::now());
        self.watch().await;
        Ok(())
    }

    // --- What the page hears ------------------------------------------------------------------------------

    fn on_update(&self) {
        self.state.borrow_mut().starting = false;
        // Drawn whole only when the page changes shape: the first of the log, or the update ending
        let in_place = {
            let live = self.live.borrow();
            let journal = self.update.borrow().as_ref().is_some_and(|u| u.journal);
            self.state.borrow().status.is_some() && live.kind == "running" && self.hero_kind() == "running" && (live.log.is_some() || !journal)
        };
        if in_place {
            self.update_live();
        } else {
            self.render();
        }
    }

    /// An update ended, or one that was started never ran: everything is read again
    fn on_status(&self, status: Status) {
        {
            let mut state = self.state.borrow_mut();
            state.status = Some(status);
            state.starting = false;
        }
        self.render();
    }
}
