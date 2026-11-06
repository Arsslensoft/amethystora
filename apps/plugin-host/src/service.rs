//! The host as a user service: `org.amethystora.Plugins` on the session bus (dbus/org.amethystora.Plugins.xml), a timer for each
//! plugin that is on, and the runs themselves on threads of their own. It stops by itself when no plugin is on (off costs
//! nothing), and it is told of `ame plugin on` and `off` by watching the account's list of what is on.
//!
//! Callers are known by the file they run, as the phone link's program does: GNOME Shell, Control and Security. A program of the
//! account that owns the name first, or calls it, is nobody. TODO(phase 0): confirm /proc/PID/exe of those callers is readable
//! from this unit (it has no namespace setting, for that reason).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_util::StreamExt;
use gio::prelude::*;
use glib::variant::ToVariant;
use serde_json::{Value, json};

use amethystora_plugin_host::host::{Env, Runner, load};
use amethystora_plugin_host::manifest::{valid_action, valid_id};

const BUS_NAME: &str = "org.amethystora.Plugins";
const PATH: &str = "/org/amethystora/Plugins";
const INTERFACE: &str = "org.amethystora.Plugins";
const NOT_AVAILABLE: &str = "org.amethystora.Plugins.Error.NotAvailable";
const XML: &str = include_str!("../dbus/org.amethystora.Plugins.xml");
/// The programs that may call: the shell's module, and the two windows that show a plugin's reach
const CALLERS: &[&str] = &["/usr/bin/gnome-shell", "/usr/lib/amethystora-control/amethystora-control", "/usr/lib/amethystora-security/amethystora-security"];
/// A plugin that fails this many runs in a row is paused, and says so
const FAILURES: u32 = 5;

enum Msg {
    Ran(String, Result<Value, String>),
    Over(String, String),
}

struct State {
    env: Arc<Env>,
    runner: Arc<Runner>,
    bus: gio::DBusConnection,
    system_bus: Option<gio::DBusConnection>,
    plugins: RefCell<HashMap<String, Value>>,
    paused: RefCell<HashSet<String>>,
    failures: RefCell<HashMap<String, u32>>,
    /// The plugins started, each with its interval's timer: a plugin turned off and on again before its timer ticked would
    /// otherwise have two, and every second run would fail as one too soon
    scheduled: RefCell<HashMap<String, Option<glib::SourceId>>>,
    clients: RefCell<HashSet<String>>,
    callers: RefCell<HashMap<String, bool>>,
    tx: Mutex<UnboundedSender<Msg>>,
    main_loop: glib::MainLoop,
}

impl State {
    fn emit(&self, signal: &str, args: glib::Variant) {
        for client in self.clients.borrow().iter() {
            if let Err(e) = self.bus.emit_signal(Some(client), PATH, INTERFACE, signal, Some(&args)) {
                eprintln!("amethystora-plugin-host: cannot send {signal}: {}", e.message());
            }
        }
    }

    fn plugin_json(&self, id: &str) -> Value {
        self.plugins.borrow().get(id).cloned().unwrap_or_else(|| json!({"id": id}))
    }

    fn changed(&self, id: &str) {
        let text = self.plugin_json(id).to_string();
        self.emit("Updated", (id, text.as_str()).to_variant());
    }

    /// Whether the account's screen is locked: interval commands wait, since nobody sees what they show
    fn locked(&self) -> bool {
        let Some(bus) = &self.system_bus else { return false };
        let reply = bus.call_sync(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1/session/auto",
            "org.freedesktop.DBus.Properties",
            "Get",
            Some(&("org.freedesktop.login1.Session", "LockedHint").to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            1000,
            None::<&gio::Cancellable>,
        );
        reply.ok().and_then(|r| r.get::<(glib::Variant,)>()).and_then(|(v,)| v.get::<bool>()).unwrap_or(false)
    }

    fn run(self: &Rc<Self>, id: &str, action: Option<String>) {
        if self.paused.borrow().contains(id) {
            return;
        }
        let (runner, id, tx) = (self.runner.clone(), id.to_string(), self.tx.lock().unwrap().clone());
        std::thread::spawn(move || {
            let result = runner.run(&id, action.as_deref());
            let _ = tx.unbounded_send(Msg::Ran(id, result));
        });
    }

    /// Starts a plugin's timer: one run now, then one each interval while it stays on
    fn start(self: &Rc<Self>, id: &str) {
        if self.scheduled.borrow().contains_key(id) {
            return;
        }
        self.scheduled.borrow_mut().insert(id.to_string(), None);
        let loaded = load(&self.env, id);
        let interval = loaded.as_ref().ok().and_then(|l| l.manifest.interval);
        {
            let mut plugins = self.plugins.borrow_mut();
            let mut entry = json!({"id": id, "output": null, "error": "", "paused": false});
            if let Ok(l) = &loaded {
                entry["name"] = l.manifest.name.clone().into();
                entry["version"] = l.manifest.version.clone().into();
            } else if let Err(e) = &loaded {
                entry["error"] = e.clone().into();
            }
            plugins.insert(id.to_string(), entry);
        }
        self.changed(id);
        if loaded.is_err() {
            // Checked again when the list changes: a plugin that failed its digest stays stopped until a person decides
            return;
        }
        self.run(id, None);
        if let Some(seconds) = interval {
            let (state, ticking) = (self.clone(), id.to_string());
            let timer = glib::timeout_add_seconds_local(seconds as u32, move || {
                if !state.locked() {
                    state.run(&ticking, None);
                }
                glib::ControlFlow::Continue
            });
            if let Some(slot) = self.scheduled.borrow_mut().get_mut(id) {
                *slot = Some(timer);
            }
        }
    }

    /// Reads the list of plugins that are on again: starts the new, forgets the gone, and ends when none is left
    fn rescan(self: &Rc<Self>) {
        let on = self.env.enabled();
        for id in &on {
            self.start(id);
        }
        self.plugins.borrow_mut().retain(|id, _| on.contains(id));
        let gone: Vec<String> = self.scheduled.borrow().keys().filter(|id| !on.contains(*id)).cloned().collect();
        for id in gone {
            if let Some(Some(timer)) = self.scheduled.borrow_mut().remove(&id) {
                timer.remove();
            }
        }
        if on.is_empty() {
            self.main_loop.quit();
        }
    }

    fn handle(self: &Rc<Self>, msg: Msg) {
        match msg {
            Msg::Ran(id, result) => {
                if !self.plugins.borrow().contains_key(&id) {
                    return;
                }
                let mut failures = self.failures.borrow_mut();
                let count = failures.entry(id.clone()).or_default();
                {
                    let mut plugins = self.plugins.borrow_mut();
                    let entry = plugins.get_mut(&id).unwrap();
                    match result {
                        Ok(output) => {
                            *count = 0;
                            entry["output"] = output;
                            entry["error"] = "".into();
                        }
                        Err(why) => {
                            *count += 1;
                            entry["error"] = why.into();
                            if *count >= FAILURES {
                                self.paused.borrow_mut().insert(id.clone());
                                entry["paused"] = true.into();
                            }
                        }
                    }
                }
                drop(failures);
                self.changed(&id);
            }
            Msg::Over(id, host) => self.emit("OverLimit", (id.as_str(), host.as_str()).to_variant()),
        }
    }

    /// Whether the connection is one of the programs that may call, by the file it runs: asked once for each connection
    fn caller_allowed(&self, sender: Option<&str>) -> bool {
        let Some(sender) = sender else { return false };
        if let Some(known) = self.callers.borrow().get(sender) {
            return *known;
        }
        let reply = self.bus.call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetConnectionCredentials",
            Some(&(sender,).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            2000,
            None::<&gio::Cancellable>,
        );
        let allowed = reply
            .ok()
            .and_then(|r| {
                let credentials = glib::VariantDict::new(Some(&r.child_value(0)));
                let pid = credentials.lookup::<u32>("ProcessID").ok().flatten()?;
                let user = credentials.lookup::<u32>("UnixUserID").ok().flatten()?;
                // SAFETY: getuid has no arguments and cannot fail
                (user == unsafe { libc::getuid() }).then_some(pid)
            })
            .and_then(|pid| std::fs::read_link(format!("/proc/{pid}/exe")).ok())
            .is_some_and(|exe| CALLERS.contains(&exe.to_string_lossy().as_ref()));
        self.callers.borrow_mut().insert(sender.to_string(), allowed);
        allowed
    }
}

fn id_arg(params: &glib::Variant, at: usize) -> Option<String> {
    params.child_value(at).get::<String>().filter(|id| valid_id(id, true).is_ok())
}

pub fn serve(env: Env) -> ExitCode {
    // Before any thread exists, so that none of them takes a signal the main loop is to hear
    let stop = stop_signals();
    if env.conf().allow == amethystora_plugin_host::conf::Allow::Off {
        eprintln!("amethystora-plugin-host: plugins are turned off on this machine");
        return ExitCode::SUCCESS;
    }
    if env.enabled().is_empty() {
        return ExitCode::SUCCESS;
    }
    let bus = match gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) {
        Ok(bus) => bus,
        Err(e) => {
            eprintln!("amethystora-plugin-host: no session bus: {}", e.message());
            return ExitCode::FAILURE;
        }
    };
    let (tx, mut rx) = unbounded::<Msg>();
    let over = Mutex::new(tx.clone());
    let env = Arc::new(env);
    let runner = Arc::new(Runner::new(
        env.clone(),
        Arc::new(move |id, host| {
            let _ = over.lock().unwrap().unbounded_send(Msg::Over(id.to_string(), host.to_string()));
        }),
    ));
    let main_loop = glib::MainLoop::new(None, false);
    let state = Rc::new(State {
        env: env.clone(),
        runner,
        bus: bus.clone(),
        system_bus: gio::bus_get_sync(gio::BusType::System, None::<&gio::Cancellable>).ok(),
        plugins: RefCell::default(),
        paused: RefCell::default(),
        failures: RefCell::default(),
        scheduled: RefCell::default(),
        clients: RefCell::default(),
        callers: RefCell::default(),
        tx: Mutex::new(tx),
        main_loop: main_loop.clone(),
    });
    let node = gio::DBusNodeInfo::for_xml(XML).expect("the contract is valid XML");
    let interface = node.lookup_interface(INTERFACE).expect("the contract has the interface");
    let weak = Rc::downgrade(&state);
    let registered = bus
        .register_object(PATH, &interface)
        .method_call(move |_, sender, _, _, method, params, invocation| {
            let Some(state) = weak.upgrade() else { return invocation.return_dbus_error(NOT_AVAILABLE, "the host is stopping") };
            if !state.caller_allowed(sender) {
                return invocation.return_dbus_error(NOT_AVAILABLE, "caller not allowed");
            }
            if let Some(sender) = sender {
                state.clients.borrow_mut().insert(sender.to_string());
            }
            match method {
                "List" => {
                    let all: Vec<Value> = state.env.enabled().iter().map(|id| state.plugin_json(id)).collect();
                    invocation.return_value(Some(&(Value::Array(all).to_string(),).to_variant()));
                }
                "Action" => match (id_arg(&params, 0), params.child_value(1).get::<String>()) {
                    (Some(id), Some(action)) if valid_action(&action) && state.plugins.borrow().contains_key(&id) => {
                        state.run(&id, Some(action));
                        invocation.return_value(None);
                    }
                    _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", "not a plugin that is on, or not an action"),
                },
                "Pause" | "Resume" => match id_arg(&params, 0) {
                    Some(id) if state.plugins.borrow().contains_key(&id) => {
                        if method == "Pause" {
                            state.paused.borrow_mut().insert(id.clone());
                        } else {
                            state.paused.borrow_mut().remove(&id);
                            state.failures.borrow_mut().remove(&id);
                        }
                        if let Some(entry) = state.plugins.borrow_mut().get_mut(&id) {
                            entry["paused"] = (method == "Pause").into();
                        }
                        state.changed(&id);
                        invocation.return_value(None);
                    }
                    _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", "not a plugin that is on"),
                },
                _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", "no such method"),
            }
        })
        .build();
    if let Err(e) = registered {
        eprintln!("amethystora-plugin-host: cannot put the host on the bus: {}", e.message());
        return ExitCode::FAILURE;
    }
    let lost = Rc::new(RefCell::new(false));
    let (quit, lost_flag) = (main_loop.clone(), lost.clone());
    let _owner = gio::bus_own_name(
        gio::BusType::Session,
        BUS_NAME,
        gio::BusNameOwnerFlags::DO_NOT_QUEUE,
        |_, _| {},
        |_, _| {},
        move |_, name| {
            eprintln!("amethystora-plugin-host: the name {name} is taken or was lost; another copy is running, or the bus is gone");
            *lost_flag.borrow_mut() = true;
            quit.quit();
        },
    );
    let handler = state.clone();
    glib::spawn_future_local(async move {
        while let Some(msg) = rx.next().await {
            handler.handle(msg);
        }
    });
    // What `ame plugin on` and `off` write: the list is read again whenever its folder changes
    let _ = std::fs::create_dir_all(&env.state_root);
    let monitor = gio::File::for_path(&env.state_root).monitor_directory(gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>).ok();
    if let Some(monitor) = &monitor {
        let watching = state.clone();
        monitor.connect_changed(move |_, file, _, _| {
            if file.basename().is_some_and(|n| n == std::path::Path::new("enabled")) {
                watching.rescan();
            }
        });
    }
    if let Some(fd) = stop {
        // A thread that does nothing but wait for the signal, and says so to the main loop
        let (tell, heard) = futures_channel::oneshot::channel::<()>();
        std::thread::spawn(move || {
            use std::io::Read;
            let mut record = [0u8; 128];
            let _ = std::fs::File::from(fd).read(&mut record);
            let _ = tell.send(());
        });
        let quit = main_loop.clone();
        glib::spawn_future_local(async move {
            let _ = heard.await;
            quit.quit();
        });
    }
    state.rescan();
    main_loop.run();
    if *lost.borrow() { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

/// SIGTERM, SIGINT and SIGHUP, blocked and read from a file descriptor, so that the main loop hears them
fn stop_signals() -> Option<std::os::fd::OwnedFd> {
    use std::os::fd::FromRawFd;
    // SAFETY: the set is initialised by sigemptyset before it is used; signalfd returns a new descriptor that this function owns
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            libc::sigaddset(&mut set, signal);
        }
        if libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) != 0 {
            return None;
        }
        let fd = libc::signalfd(-1, &set, libc::SFD_CLOEXEC);
        (fd >= 0).then(|| std::os::fd::OwnedFd::from_raw_fd(fd))
    }
}
