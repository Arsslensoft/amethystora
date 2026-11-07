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

use amethystora_plugin_host::host::{Env, Runner, classify, load};
use amethystora_plugin_host::manifest::{valid_action, valid_id};

const BUS_NAME: &str = "org.amethystora.Plugins";
const PATH: &str = "/org/amethystora/Plugins";
const INTERFACE: &str = "org.amethystora.Plugins";
const NOT_AVAILABLE: &str = "org.amethystora.Plugins.Error.NotAvailable";
const XML: &str = include_str!("../dbus/org.amethystora.Plugins.xml");
/// The programs that may call: the shell's module, and the two windows that show a plugin's reach
const CALLERS: &[&str] = &["/usr/bin/gnome-shell", "/usr/lib/amethystora-control/amethystora-control", "/usr/lib/amethystora-security/amethystora-security"];
/// The rules that pause a plugin by itself (R7-09), each said once with its reason: this many runs of one kind in a row. A run that
/// waits for the Vault is no fault and counts for none. Too many updates cannot come from a timer (a plugin runs at most once in five
/// seconds), but a click storm or a script's flood can, so more than UPDATES results in a minute pauses it too
const FAILURES: u32 = 5;
const SLOW: u32 = 3;
const MEMORY: u32 = 2;
const UPDATES: usize = 30;

enum Msg {
    /// A run ended: the plugin, what it printed or why not, and the action it was run for (a person's click), if any
    Ran(String, Result<Value, String>, Option<String>),
    Over(String, String),
    /// A plugin asked the shell to open an address or copy a text, after a click: (id, kind, text)
    Asked(String, String, String),
    /// A person said yes to giving a file from Files' menu to a plugin: (id, the file)
    Handed(String, std::path::PathBuf),
}

struct State {
    env: Arc<Env>,
    runner: Arc<Runner>,
    bus: gio::DBusConnection,
    system_bus: Option<gio::DBusConnection>,
    plugins: RefCell<HashMap<String, Value>>,
    paused: RefCell<HashSet<String>>,
    failures: RefCell<HashMap<String, (String, u32)>>,
    /// When each plugin's results arrived in the last minute
    recent: RefCell<HashMap<String, std::collections::VecDeque<std::time::Instant>>>,
    /// A click that found the plugin's run still going: the latest, for each plugin
    queued: RefCell<HashMap<String, String>>,
    /// The plugins started, each with its interval's timer: a plugin turned off and on again before its timer ticked would
    /// otherwise have two, and every second run would fail as one too soon
    scheduled: RefCell<HashMap<String, Option<glib::SourceId>>>,
    /// News waiting for a plugin's next run (`events`): the latest of it, and whether a run for it is already timed
    news: RefCell<HashMap<String, (String, String)>>,
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

    /// News from the system (plans/PLAN-plugins.md, the permissions: `events`): each plugin that is on and was granted `events=NAME`
    /// runs once for it, with no click, and its `plugin event` says what happened. A burst of news is one run with the latest of it,
    /// after the five seconds between runs; while the screen is locked only the unlock itself is news
    fn event(self: &Rc<Self>, name: &str, value: &str) {
        if name != "screen" && self.locked() {
            return;
        }
        let wanted = format!("events={name}");
        let ids: Vec<String> = self
            .plugins
            .borrow()
            .iter()
            .filter(|(id, p)| !self.paused.borrow().contains(*id) && p["granted"].as_array().is_some_and(|g| g.iter().any(|x| x.as_str() == Some(&wanted))))
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            let timed = self.news.borrow_mut().insert(id.clone(), (name.to_string(), value.to_string())).is_some();
            if timed {
                continue;
            }
            let wait = self.runner.gap_left(&id) + std::time::Duration::from_millis(300);
            let weak = Rc::downgrade(self);
            glib::timeout_add_local_once(wait, move || {
                let Some(state) = weak.upgrade() else { return };
                let Some((name, value)) = state.news.borrow_mut().remove(&id) else { return };
                if !state.plugins.borrow().contains_key(&id) || state.paused.borrow().contains(&id) {
                    return;
                }
                let (runner, tx) = (state.runner.clone(), state.tx.lock().unwrap().clone());
                std::thread::spawn(move || {
                    let result = runner.set_event(&id, &name, &value).and_then(|_| runner.run_as(&id, None, false));
                    let _ = tx.unbounded_send(Msg::Ran(id, result, None));
                });
            });
        }
    }

    /// Where the news comes from: NetworkManager's connectivity, UPower's battery, the screen's lock and the colour scheme. Only read, and
    /// only while the host runs, which is while a plugin is on
    fn listen(self: &Rc<Self>) -> (Vec<gio::SignalSubscription>, Option<gio::Settings>) {
        let mut subscriptions = Vec::new();
        let props = |v: &glib::Variant| -> Option<glib::VariantDict> { (v.n_children() >= 2).then(|| glib::VariantDict::new(Some(&v.child_value(1)))) };
        if let Some(system) = &self.system_bus {
            let weak = Rc::downgrade(self);
            subscriptions.push(system.subscribe_to_signal(
                Some("org.freedesktop.NetworkManager"),
                Some("org.freedesktop.DBus.Properties"),
                Some("PropertiesChanged"),
                Some("/org/freedesktop/NetworkManager"),
                None,
                gio::DBusSignalFlags::NONE,
                move |signal| {
                    let Some(state) = weak.upgrade() else { return };
                    if let Some(c) = props(signal.parameters).and_then(|d| d.lookup::<u32>("Connectivity").ok().flatten()) {
                        state.event("network", match c { 4 => "online", 2 | 3 => "limited", 1 => "offline", _ => "unknown" });
                    }
                },
            ));
            let weak = Rc::downgrade(self);
            subscriptions.push(system.subscribe_to_signal(
                Some("org.freedesktop.UPower"),
                Some("org.freedesktop.DBus.Properties"),
                Some("PropertiesChanged"),
                Some("/org/freedesktop/UPower"),
                None,
                gio::DBusSignalFlags::NONE,
                move |signal| {
                    let Some(state) = weak.upgrade() else { return };
                    if let Some(on) = props(signal.parameters).and_then(|d| d.lookup::<bool>("OnBattery").ok().flatten()) {
                        state.event("power", if on { "battery" } else { "mains" });
                    }
                },
            ));
        }
        let weak = Rc::downgrade(self);
        subscriptions.push(self.bus.subscribe_to_signal(
            Some("org.gnome.ScreenSaver"),
            Some("org.gnome.ScreenSaver"),
            Some("ActiveChanged"),
            Some("/org/gnome/ScreenSaver"),
            None,
            gio::DBusSignalFlags::NONE,
            move |signal| {
                let Some(state) = weak.upgrade() else { return };
                if let Some((active,)) = signal.parameters.get::<(bool,)>() {
                    state.event("screen", if active { "locked" } else { "unlocked" });
                }
            },
        ));
        // The colour scheme, which the theme and the Dark Style switch both set
        let interface = gio::SettingsSchemaSource::default()
            .and_then(|s| s.lookup("org.gnome.desktop.interface", true))
            .map(|_| gio::Settings::new("org.gnome.desktop.interface"));
        if let Some(settings) = &interface {
            use gio::prelude::SettingsExt;
            // GSettings reports a change only of a key read once
            let _ = settings.string("color-scheme");
            let weak = Rc::downgrade(self);
            settings.connect_changed(Some("color-scheme"), move |settings, key| {
                let Some(state) = weak.upgrade() else { return };
                let scheme = settings.string(key);
                state.event("theme", if scheme.contains("dark") { "dark" } else { "light" });
            });
        }
        (subscriptions, interface)
    }

    fn run(self: &Rc<Self>, id: &str, action: Option<String>) {
        let click = action.is_some();
        self.run_as(id, action, click);
    }

    fn run_as(self: &Rc<Self>, id: &str, action: Option<String>, click: bool) {
        if self.paused.borrow().contains(id) {
            return;
        }
        let (runner, id, tx) = (self.runner.clone(), id.to_string(), self.tx.lock().unwrap().clone());
        std::thread::spawn(move || {
            let result = runner.run_as(&id, action.as_deref(), click);
            let _ = tx.unbounded_send(Msg::Ran(id, result, action));
        });
    }

    /// Whether a plugin that is on declared this action: one it did not is refused at the call, so that no program of the account can
    /// fill a plugin's error with refusals until it is paused
    fn declares(&self, id: &str, action: &str) -> bool {
        valid_action(action) && self.plugins.borrow().get(id).and_then(|p| p["actions"].as_array()).is_some_and(|a| a.iter().any(|x| x.as_str() == Some(action)))
    }

    /// Starts a plugin's timer: one run now, then one each interval while it stays on
    fn start(self: &Rc<Self>, id: &str) {
        // (below)
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
                entry["places"] = Value::Object(l.manifest.places.clone());
                entry["author"] = l.manifest.author.clone().into();
                entry["draft"] = l.draft.into();
                entry["shipped"] = l.grant.shipped.into();
                entry["granted"] = l.perms().into();
                entry["actions"] = l.manifest.actions.clone().into();
                if l.draft {
                    // What the agent said it would show, for the run that finds nothing to reach
                    entry["sample"] = std::fs::read_to_string(l.folder.join("sample.json")).ok().and_then(|t| amethystora_plugin_host::output::parse(&t, &l.manifest.actions).ok()).unwrap_or(Value::Null);
                }
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
            // Turned off, it is forgotten whole: turning it on again is a fresh start, which is how a plugin that paused itself is resumed from Control
            self.paused.borrow_mut().remove(&id);
            self.failures.borrow_mut().remove(&id);
            self.recent.borrow_mut().remove(&id);
            self.queued.borrow_mut().remove(&id);
        }
        if on.is_empty() {
            self.main_loop.quit();
        }
    }

    fn handle(self: &Rc<Self>, msg: Msg) {
        match msg {
            Msg::Ran(id, result, action) => {
                if !self.plugins.borrow().contains_key(&id) {
                    return;
                }
                // A click that came while a run was going waits for it (one at a time, the latest), and goes when that run is over
                if result.as_ref().is_err_and(|why| why.starts_with("it is still running")) {
                    if let Some(a) = action {
                        self.queued.borrow_mut().insert(id.clone(), a);
                    }
                    return;
                }
                if let Some(a) = self.queued.borrow_mut().remove(&id) {
                    self.run(&id, Some(a));
                }
                // A run that was refused for coming too soon changed nothing: it is neither an error nor a result
                if result.as_ref().is_err_and(|why| classify(why) == "gap") {
                    return;
                }
                let mut pause = None;
                {
                    let mut plugins = self.plugins.borrow_mut();
                    let entry = plugins.get_mut(&id).unwrap();
                    let mut failures = self.failures.borrow_mut();
                    match result {
                        Ok(output) => {
                            failures.remove(&id);
                            entry["output"] = output;
                            entry["error"] = "".into();
                            entry["waiting"] = Value::Null;
                            // More than UPDATES results in a minute: not a plugin, a flood
                            let mut recent = self.recent.borrow_mut();
                            let seen = recent.entry(id.clone()).or_default();
                            let now = std::time::Instant::now();
                            seen.push_back(now);
                            while seen.front().is_some_and(|t| now.duration_since(*t).as_secs() >= 60) {
                                seen.pop_front();
                            }
                            if seen.len() > UPDATES {
                                pause = Some("updates");
                            }
                        }
                        Err(why) => {
                            let kind = classify(&why);
                            entry["error"] = why.clone().into();
                            if kind == "vault" {
                                // Waits, and tries again at the next tick: nothing is paused for a closed Vault
                                entry["waiting"] = "vault".into();
                            } else {
                                let count = match failures.get_mut(&id) {
                                    Some((k, n)) if k.as_str() == kind => {
                                        *n += 1;
                                        *n
                                    }
                                    _ => {
                                        failures.insert(id.clone(), (kind.to_string(), 1));
                                        1
                                    }
                                };
                                pause = match kind {
                                    "slow" if count >= SLOW => Some("slow"),
                                    "memory" if count >= MEMORY => Some("memory"),
                                    "other" if count >= FAILURES => Some("crash"),
                                    _ => None,
                                };
                            }
                        }
                    }
                    if let Some(reason) = pause {
                        entry["paused"] = true.into();
                        entry["reason"] = reason.into();
                    }
                }
                if let Some(reason) = pause {
                    self.paused.borrow_mut().insert(id.clone());
                    // Said once, when it happens: the shell shows it with Ask the agent to fix it
                    self.emit("Paused", (id.as_str(), reason).to_variant());
                }
                self.changed(&id);
            }
            Msg::Over(id, host) => self.emit("OverLimit", (id.as_str(), host.as_str()).to_variant()),
            // A notice's button pressed: the person's click, for an action the broker already held to the manifest
            Msg::Asked(id, kind, text) if kind == "action" => {
                if valid_action(&text) && self.plugins.borrow().contains_key(&id) {
                    self.run(&id, Some(text));
                }
            }
            Msg::Asked(id, kind, text) => self.emit("Asked", (id.as_str(), kind.as_str(), text.as_str()).to_variant()),
            Msg::Handed(id, path) => {
                let action = self.plugins.borrow().get(&id).and_then(|p| p["places"]["files-menu"]["action"].as_str().map(str::to_string));
                if let Some(action) = action.filter(|a| valid_action(a)) {
                    if self.runner.hand(&id, path).is_ok() {
                        self.run(&id, Some(action));
                    }
                }
            }
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
    let asked = Mutex::new(tx.clone());
    let runner = Arc::new(
        Runner::new(
            env.clone(),
            Arc::new(move |id, host| {
                let _ = over.lock().unwrap().unbounded_send(Msg::Over(id.to_string(), host.to_string()));
            }),
        )
        .with_requests(Arc::new(move |id, kind, text| {
            let _ = asked.lock().unwrap().unbounded_send(Msg::Asked(id.to_string(), kind.to_string(), text.to_string()));
        })),
    );
    let main_loop = glib::MainLoop::new(None, false);
    let state = Rc::new(State {
        env: env.clone(),
        runner,
        bus: bus.clone(),
        system_bus: gio::bus_get_sync(gio::BusType::System, None::<&gio::Cancellable>).ok(),
        plugins: RefCell::default(),
        paused: RefCell::default(),
        failures: RefCell::default(),
        recent: RefCell::default(),
        queued: RefCell::default(),
        scheduled: RefCell::default(),
        news: RefCell::default(),
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
            // Hand is Files' menu, any program of the account's: nothing is given before the person says yes in a question the host itself
            // asks, naming the plugin and the file
            if method == "Hand" {
                return match (id_arg(&params, 0), params.child_value(1).get::<String>()) {
                    (Some(id), Some(path)) if path.starts_with('/') && path.len() < 4096 && std::path::Path::new(&path).is_file() => {
                        let name = state.plugins.borrow().get(&id).filter(|p| p["places"]["files-menu"].is_object()).and_then(|p| p["name"].as_str().map(str::to_string));
                        let Some(name) = name else {
                            return invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", "not a plugin that is on and takes files");
                        };
                        let tx = state.tx.lock().unwrap().clone();
                        std::thread::spawn(move || {
                            let file = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                            let question = format!("Give “{}” to the plugin {}? It gets this file's contents, and nothing else of yours.", amethystora_plugin_host::output::clean(&file, 200), amethystora_plugin_host::output::clean(&name, 80));
                            let yes = std::process::Command::new("timeout")
                                .args(["120", "zenity", "--question", "--no-markup", "--title", "Give a file to a plugin", "--text", &question])
                                .stdin(std::process::Stdio::null())
                                .status()
                                .is_ok_and(|s| s.success());
                            if yes {
                                let _ = tx.unbounded_send(Msg::Handed(id, path.into()));
                            }
                        });
                        invocation.return_value(None)
                    }
                    _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", "not a plugin that is on, or not a file"),
                };
            }
            // Ask is the agent's `plugin` tool, any program of the account's: the plugin's `tool` action, no click, and what it printed as the
            // answer, once the run ends
            if method == "Ask" {
                let (Some(id), Some(action)) = (id_arg(&params, 0), params.child_value(1).get::<String>()) else {
                    return invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", "an id and an action");
                };
                let offered = state.plugins.borrow().get(&id).is_some_and(|p| p["places"]["tool"]["action"].as_str() == Some(action.as_str()));
                if !offered || state.paused.borrow().contains(&id) {
                    return invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", "not a tool of a plugin that is on");
                }
                let (tell, heard) = futures_channel::oneshot::channel::<Result<Value, String>>();
                let (runner, tx, ran) = (state.runner.clone(), state.tx.lock().unwrap().clone(), id.clone());
                std::thread::spawn(move || {
                    let result = runner.run_as(&ran, Some(&action), false);
                    let _ = tell.send(result.clone());
                    let _ = tx.unbounded_send(Msg::Ran(ran, result, None));
                });
                glib::spawn_future_local(async move {
                    let answer = match heard.await {
                        Ok(Ok(output)) => json!({"output": output}),
                        Ok(Err(why)) => json!({"error": why}),
                        Err(_) => json!({"error": "the run did not end"}),
                    };
                    invocation.return_value(Some(&(answer.to_string(),).to_variant()));
                });
                return;
            }
            // Run is any program of the account's (`ame plugin run`, a script): it is no click, so it opens, copies and controls nothing
            if method == "Run" {
                return match (id_arg(&params, 0), params.child_value(1).get::<String>()) {
                    (Some(id), Some(action)) if (action.is_empty() || state.declares(&id, &action)) && state.plugins.borrow().contains_key(&id) => {
                        state.run_as(&id, (!action.is_empty()).then_some(action), false);
                        invocation.return_value(None)
                    }
                    _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", "not a plugin that is on, or not an action"),
                };
            }
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
                // The app in front, which only the shell knows, told only while a plugin that is on was granted events=focused-app
                "Focused" => match params.child_value(0).get::<String>() {
                    Some(app) if app.len() <= 255 && app.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) => {
                        state.event("focused-app", &app);
                        invocation.return_value(None);
                    }
                    _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.InvalidArgs", "not an app's id"),
                },
                "Action" => match (id_arg(&params, 0), params.child_value(1).get::<String>()) {
                    // The panel's action opens its window, beside the plugin's usual runs; what the window does is not an output to show
                    (Some(id), Some(action))
                        if valid_action(&action) && state.plugins.borrow().get(&id).is_some_and(|p| p["places"]["panel"]["action"].as_str() == Some(action.as_str())) =>
                    {
                        let runner = state.runner.clone();
                        std::thread::spawn(move || {
                            if let Err(why) = runner.panel(&id, &action) {
                                eprintln!("amethystora-plugin-host: plugin {id}'s window: {why}");
                            }
                        });
                        invocation.return_value(None);
                    }
                    (Some(id), Some(action)) if state.declares(&id, &action) => {
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
                            state.recent.borrow_mut().remove(&id);
                        }
                        if let Some(entry) = state.plugins.borrow_mut().get_mut(&id) {
                            entry["paused"] = (method == "Pause").into();
                            entry["reason"] = if method == "Pause" { "person".into() } else { Value::Null };
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
    let _listening = state.listen();
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
