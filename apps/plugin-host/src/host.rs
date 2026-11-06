//! Loading a plugin from the store and running it: the digest is checked against what was approved every time, the
//! sandbox's command line is built by `sandbox.rs` from the grant alone, and what it prints is read by `output.rs`.
//! The host holds no state of its own about what is allowed: every run and every connection read grants.json again, so
//! that a revoke or a block applies at once and nothing the account writes is trusted.

use std::collections::HashMap;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::broker::{self, Broker};
use crate::conf::{Allow, CONF, Conf};
use crate::manifest::Manifest;
use crate::output::{self, MAX_OUTPUT};
use crate::proxy::{Policy, Proxy};
use crate::sandbox::{self, SECCOMP_FD, SECCOMP_FILE, Spec};
use crate::store::{Grant, Store};
use crate::files;

pub const RUN_LIMIT: Duration = Duration::from_secs(30);
pub const MIN_GAP: Duration = Duration::from_secs(5);
/// A timer that fires every five seconds counts from when each run started, a few milliseconds after its tick: without this the timer of a
/// plugin with `interval: 5` would be refused every other time
const GAP_SLACK: Duration = Duration::from_millis(250);
/// What a plugin's state folder may hold before it stops running
pub const STATE_LIMIT: u64 = 8 << 20;

#[derive(Clone)]
pub struct Env {
    pub store: Store,
    pub uid: u32,
    pub home: PathBuf,
    /// $XDG_RUNTIME_DIR/amethystora/plugins: the proxies' sockets and the one-line passwd
    pub runtime: PathBuf,
    /// ~/.local/state/amethystora/plugins: a folder for each plugin, and the list of those that are on
    pub state_root: PathBuf,
    pub conf_path: PathBuf,
    pub seccomp: PathBuf,
    /// ~/.local/share/amethystora/plugin-drafts: what an agent wrote and nobody kept yet. A draft has no permissions and no store
    pub drafts_root: PathBuf,
    /// The Vault's socket, which keeps a plugin's secret settings under a key of its own
    pub vault_socket: PathBuf,
}

impl Env {
    pub fn production() -> Result<Env, String> {
        // SAFETY: getuid has no arguments and cannot fail
        let uid = unsafe { libc::getuid() };
        let home = PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?);
        // The account's own runtime folder only: one inherited from another account (runuser from root keeps /run/user/0) is not
        // where its proxies and passwd belong, and is not writable for it anyway
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|dir| std::fs::metadata(dir).is_ok_and(|m| std::os::unix::fs::MetadataExt::uid(&m) == uid))
            .unwrap_or_else(|| format!("/run/user/{uid}").into());
        let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/state"));
        let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"));
        #[allow(unused_mut)]
        let mut env = Env {
            store: Store::production(),
            uid,
            home,
            runtime: runtime.join("amethystora/plugins"),
            state_root: state.join("amethystora/plugins"),
            conf_path: CONF.into(),
            seccomp: SECCOMP_FILE.into(),
            drafts_root: data.join("amethystora/plugin-drafts"),
            vault_socket: amethystora_vault_client::SOCKET.into(),
        };
        // Only a build for trying the host on a machine that has no image yet (a debug build): a release build has no such seam
        #[cfg(debug_assertions)]
        {
            if let Some(dir) = std::env::var_os("AMETHYSTORA_PLUGIN_TEST_IMAGE") {
                let dir = PathBuf::from(dir);
                env.store = Store { root: dir.join("store-root"), shipped: dir.join("shipped") };
                env.seccomp = dir.join("seccomp.bpf");
                env.conf_path = dir.join("plugins.conf");
                env.vault_socket = dir.join("vault.sock");
            }
        }
        Ok(env)
    }

    pub fn conf(&self) -> Conf {
        Conf::load(&self.conf_path)
    }

    fn enabled_file(&self) -> PathBuf {
        self.state_root.join("enabled")
    }

    pub fn enabled(&self) -> Vec<String> {
        std::fs::read_to_string(self.enabled_file()).unwrap_or_default().lines().map(str::to_string).filter(|l| crate::manifest::valid_id(l, true).is_ok()).collect()
    }

    /// On and off are the account's: they stay inside what was approved, so nothing here needs a password
    pub fn set_enabled(&self, id: &str, on: bool) -> Result<(), String> {
        std::fs::create_dir_all(&self.state_root).map_err(|e| e.to_string())?;
        let mut list = self.enabled();
        list.retain(|i| i != id);
        if on {
            list.push(id.to_string());
        }
        list.sort();
        let temp = self.state_root.join(".enabled.new");
        std::fs::write(&temp, list.iter().map(|i| format!("{i}\n")).collect::<String>()).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, self.enabled_file()).map_err(|e| e.to_string())
    }

    pub fn state_dir(&self, id: &str) -> PathBuf {
        self.state_root.join(id)
    }
}

pub struct Loaded {
    /// An agent's draft: no permissions, no store, wears "Draft" wherever it shows
    pub draft: bool,
    pub id: String,
    pub folder: PathBuf,
    pub manifest: Manifest,
    pub grant: Grant,
}

impl Loaded {
    /// What it may do: what was granted and what its manifest still asks for
    pub fn perms(&self) -> Vec<String> {
        let asked = self.manifest.permissions();
        self.grant.perms.iter().filter(|p| asked.contains(*p)).cloned().collect()
    }
}

/// A plugin as installed, and only if its files are exactly what was approved. One that is not installed may be a draft in the
/// account's own folder: it runs with nothing granted, whatever its manifest asks for
pub fn load(env: &Env, id: &str) -> Result<Loaded, String> {
    let conf = env.conf();
    if conf.allow == Allow::Off {
        return Err("plugins are turned off on this machine (ALLOW=off in /etc/amethystora/plugins.conf)".into());
    }
    let Some(grant) = env.store.grants(env.uid)?.plugins.remove(id) else {
        let folder = env.drafts_root.join(id);
        let text = std::fs::read_to_string(folder.join("manifest.json")).map_err(|_| format!("{id} is not installed"))?;
        let manifest = Manifest::parse_with(&text, false)?;
        if manifest.id != id {
            return Err("the manifest's id is not the one of the draft's folder".into());
        }
        return Ok(Loaded { draft: true, id: id.into(), folder, manifest, grant: Grant::default() });
    };
    let folder = env.store.folder_of(id, &grant);
    env.store.verify(&folder, &grant.digest).map_err(|e| format!("{e}; it is stopped until you decide what to do"))?;
    let text = std::fs::read_to_string(folder.join("manifest.json")).map_err(|e| e.to_string())?;
    let manifest = Manifest::parse_with(&text, grant.shipped)?;
    if manifest.id != id {
        return Err("the manifest's id is not the one installed".into());
    }
    Ok(Loaded { draft: false, id: id.into(), folder, manifest, grant })
}

/// The drafts the account has: folders with a manifest, by id
pub fn drafts(env: &Env) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(&env.drafts_root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|id| crate::manifest::valid_id(id, false).is_ok() && env.drafts_root.join(id).join("manifest.json").is_file())
        .collect();
    found.sort();
    found
}

/// What the proxy may let one plugin reach, from the grants as they are now and the machine's conf
pub fn policy_of(env: &Env, id: &str) -> Policy {
    let conf = env.conf();
    let Some(grant) = env.store.grants(env.uid).ok().and_then(|mut g| g.plugins.remove(id)) else {
        return Policy { machine_off: true, ..Policy::default() };
    };
    Policy {
        allowed: grant.perms.iter().filter_map(|p| p.strip_prefix("network=")).map(str::to_string).collect(),
        blocked_all: grant.blocked,
        blocked_hosts: grant.blocked_hosts,
        limits: grant.limits,
        machine_off: conf.block_network || conf.allow == Allow::Off,
    }
}

fn dir_size(path: &Path, depth: u32) -> u64 {
    if depth > 8 {
        return u64::MAX / 2;
    }
    std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path(), depth + 1),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

type Told = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// Runs plugins, one at a time for each, no oftener than every five seconds
pub struct Runner {
    pub env: Arc<Env>,
    last: Mutex<HashMap<String, Instant>>,
    proxies: Mutex<HashMap<String, Arc<Proxy>>>,
    /// The broker of each plugin that may ask for anything, started the first time it runs
    brokers: Mutex<HashMap<String, Arc<Broker>>>,
    /// What the shell is asked to do for a plugin (open an address, copy a text), which only it can: (id, kind, text)
    requests: Mutex<Option<broker::Told>>,
    /// Called with the plugin's id and the host when sending passes the limit
    over_limit: Told,
    /// Plugins with a run in progress: a second run of one is refused, so that budgets and read-write mounts are never doubled
    running: Mutex<std::collections::HashSet<String>>,
}

struct Running<'a>(&'a Mutex<std::collections::HashSet<String>>, String);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.lock().unwrap().remove(&self.1);
    }
}

/// A block the limit made is kept in the store like a person's: amethystora-plugin-root block, which needs no password for the person at the machine
/// TODO(phase 0): confirm pkexec from this user service counts as the active session for 50-amethystora-plugins.rules
fn persist_block(id: &str, host: &str) {
    let (id, host) = (id.to_string(), host.to_string());
    let _ = std::thread::Builder::new().spawn(move || {
        let _ = Command::new("pkexec").args(["/usr/libexec/amethystora-plugin-root", "block", &id, &host]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    });
}

impl Runner {
    pub fn new(env: Arc<Env>, over_limit: Told) -> Runner {
        Runner {
            env,
            last: Mutex::new(HashMap::new()),
            proxies: Mutex::new(HashMap::new()),
            brokers: Mutex::new(HashMap::new()),
            requests: Mutex::new(None),
            over_limit,
            running: Mutex::new(Default::default()),
        }
    }

    /// Where the shell is told what a plugin asked it to do
    pub fn with_requests(self, told: broker::Told) -> Runner {
        *self.requests.lock().unwrap() = Some(told);
        self
    }

    /// The folder holding this plugin's broker socket, and the broker, started the first time it runs
    fn broker(&self, id: &str) -> Result<(PathBuf, Arc<Broker>), String> {
        let dir = self.env.runtime.join("broker").join(id);
        let mut brokers = self.brokers.lock().unwrap();
        if let Some(b) = brokers.get(id) {
            return Ok((dir, b.clone()));
        }
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        let socket = dir.join("broker.sock");
        let _ = std::fs::remove_file(&socket);
        let listener = std::os::unix::net::UnixListener::bind(&socket).map_err(|e| e.to_string())?;
        let told = self.requests.lock().unwrap().clone().unwrap_or_else(|| Arc::new(|_, _, _| {}));
        let broker = Broker::new(self.env.clone(), id, told);
        let serving = broker.clone();
        std::thread::spawn(move || serving.serve(listener));
        brokers.insert(id.to_string(), broker.clone());
        Ok((dir, broker))
    }

    /// The folder holding this plugin's proxy socket, started the first time a plugin with a network permission runs
    fn proxy_dir(&self, id: &str) -> Result<PathBuf, String> {
        let dir = self.env.runtime.join(id);
        let mut proxies = self.proxies.lock().unwrap();
        if proxies.contains_key(id) {
            return Ok(dir);
        }
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        let socket = dir.join("proxy.sock");
        let _ = std::fs::remove_file(&socket);
        let listener = std::os::unix::net::UnixListener::bind(&socket).map_err(|e| e.to_string())?;
        let (env, told, who) = (self.env.clone(), self.over_limit.clone(), id.to_string());
        let proxy = Proxy::system(
            {
                let (env, id) = (env.clone(), id.to_string());
                Box::new(move || policy_of(&env, &id))
            },
            Box::new({
                let who = who.clone();
                move |host| {
                    persist_block(&who, host);
                    told(&who, host)
                }
            }),
        );
        let serving = proxy.clone();
        std::thread::spawn(move || serving.serve(listener));
        proxies.insert(id.to_string(), proxy);
        Ok(dir)
    }

    /// The command line for one run, and nothing started (also what `sandbox` prints)
    pub fn command_line(&self, loaded: &Loaded, action: Option<&str>, with_proxy: bool) -> Result<Vec<String>, String> {
        let env = &self.env;
        let state = env.state_dir(&loaded.id);
        std::fs::create_dir_all(&state).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        if dir_size(&state, 0) > STATE_LIMIT {
            return Err("its state folder holds more than 8 MiB, so it does not run".into());
        }
        let perms = loaded.perms();
        let mut mounts = Vec::new();
        for perm in perms.iter().filter_map(|p| p.strip_prefix("files=")) {
            let (path, mode) = perm.rsplit_once(':').ok_or("a files permission has a mode")?;
            mounts.push((files::resolve(&env.home, path, mode == "rw")?, mode == "rw"));
        }
        let has_network = perms.iter().any(|p| p.starts_with("network="));
        let proxy_dir = if has_network && with_proxy { Some(self.proxy_dir(&loaded.id)?) } else if has_network { Some(env.runtime.join(&loaded.id)) } else { None };
        // Anything a plugin may ask the host for needs the socket, and a draft has it to be answered from samples
        let asks = loaded.draft || perms.iter().any(|p| p.starts_with("state=") || p.starts_with("control=") || p == "notify" || p == "open" || p == "clipboard-write");
        let broker_dir = if asks && with_proxy { Some(self.broker(&loaded.id)?.0) } else if asks { Some(env.runtime.join("broker").join(&loaded.id)) } else { None };
        std::fs::create_dir_all(&env.runtime).map_err(|e| e.to_string())?;
        let passwd = env.runtime.join("passwd");
        let user = std::env::var("USER").ok().filter(|u| u.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')).unwrap_or_else(|| "user".into());
        std::fs::write(&passwd, format!("{user}:x:{0}:{0}::{1}:/usr/sbin/nologin\n", env.uid, sandbox::STATE_DIR)).map_err(|e| e.to_string())?;
        if let Some(a) = action.filter(|a| !loaded.manifest.actions.iter().any(|x| x == a)) {
            return Err(format!("{a:?} is not one of the plugin's actions"));
        }
        let kept = sandbox::kept_env(std::env::vars());
        let spec = Spec {
            id: &loaded.id,
            folder: &loaded.folder,
            state: &state,
            command: &loaded.manifest.command,
            files: &mounts,
            proxy_dir: proxy_dir.as_deref(),
            broker_dir: broker_dir.as_deref(),
            passwd: &passwd,
            env: &kept,
            action,
        };
        Ok(sandbox::command_line(&spec))
    }

    /// One run: the sandbox, its budgets, 30 seconds, and what it printed, checked
    pub fn run(&self, id: &str, action: Option<&str>) -> Result<Value, String> {
        let loaded = load(&self.env, id)?;
        if !self.running.lock().unwrap().insert(id.to_string()) {
            return Err("it is still running from the last time".into());
        }
        let _running = Running(&self.running, id.to_string());
        let _sent = SentGuard(self, id.to_string());
        {
            let mut last = self.last.lock().unwrap();
            // A person's click is not made to wait for the timer's gap; a flood of them is the update rule's (service.rs)
            if action.is_none() && last.get(id).is_some_and(|t| t.elapsed() + GAP_SLACK < MIN_GAP) {
                return Err("it ran a moment ago: a plugin runs at most once in five seconds".into());
            }
            last.insert(id.to_string(), Instant::now());
        }
        let line = self.command_line(&loaded, action, true)?;
        // A person's click is what lets a run change anything outside the sandbox; the next run on the timer has none
        let _click = ClickGuard(self.brokers.lock().unwrap().get(id).cloned());
        if let Some(b) = &_click.0 {
            b.clicked.store(action.is_some(), Ordering::Relaxed);
        }
        // The settings a person gave it come on a pipe, never in the environment: a closed Vault stops the run and says why
        let settings = if !loaded.draft && !loaded.manifest.settings.is_empty() { Some(settings_pipe(&self.env, id)?) } else { None };
        let filter = std::fs::File::open(&self.env.seccomp).map_err(|_| format!("{} is missing, so nothing is run", self.env.seccomp.display()))?;
        let mut command = Command::new(&line[0]);
        command.args(&line[1..]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        // Both are moved above the two descriptors they are to become, so that dup2 onto one can never overwrite the other (a pipe made
        // while descriptor 9 was free would be 9, and the filter would land on it)
        let high = |fd: i32| -> Result<std::os::fd::OwnedFd, String> {
            use std::os::fd::FromRawFd;
            // SAFETY: F_DUPFD_CLOEXEC returns a new descriptor this function owns, or -1
            let moved = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 20) };
            if moved < 0 { Err(std::io::Error::last_os_error().to_string()) } else { Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(moved) }) }
        };
        let filter_high = high(filter.as_raw_fd())?;
        let settings_high = settings.as_ref().map(|f| high(f.as_raw_fd())).transpose()?;
        let fd = filter_high.as_raw_fd();
        let settings_fd = settings_high.as_ref().map(|f| f.as_raw_fd());
        // SAFETY: only dup2 and fcntl, which are async-signal-safe, between fork and exec
        unsafe {
            command.pre_exec(move || {
                // TODO(phase 0): the descriptor has to get through systemd-run --scope into bwrap; confirm it does
                // No file the plugin writes is larger than its state folder may be
                let limit = libc::rlimit { rlim_cur: STATE_LIMIT, rlim_max: STATE_LIMIT };
                if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::dup2(fd, SECCOMP_FD) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if let Some(read) = settings_fd {
                    if libc::dup2(read, sandbox::SETTINGS_FD) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let ooms = oom_kills();
        let mut child = command.spawn().map_err(|e| format!("cannot start the sandbox: {e}"))?;
        let mut stdout = child.stdout.take().ok_or("no output")?;
        let overflow = Arc::new(AtomicBool::new(false));
        let reader = std::thread::spawn({
            let overflow = overflow.clone();
            move || {
                let mut text = Vec::new();
                let _ = (&mut stdout).take(MAX_OUTPUT as u64 + 1).read_to_end(&mut text);
                if text.len() > MAX_OUTPUT {
                    overflow.store(true, Ordering::Relaxed);
                }
                text
            }
        });
        let started = Instant::now();
        let mut failure = None;
        let state = self.env.state_dir(id);
        let mut ticks = 0u32;
        let status = loop {
            if let Ok(Some(status)) = child.try_wait() {
                break Some(status);
            }
            if started.elapsed() > RUN_LIMIT {
                failure = Some("it took longer than 30 seconds, and was stopped".to_string());
            } else if ticks % 10 == 9 && dir_size(&state, 0) > STATE_LIMIT {
                failure = Some("its state folder grew past 8 MiB, and it was stopped".to_string());
            } else if overflow.load(Ordering::Relaxed) {
                failure = Some("it printed more than 64 KiB, and was stopped".to_string());
            }
            if failure.is_some() {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            ticks += 1;
            std::thread::sleep(Duration::from_millis(50));
        };
        let text = reader.join().unwrap_or_default();
        if let Some(why) = failure {
            return Err(why);
        }
        let text = String::from_utf8_lossy(&text).into_owned();
        match status {
            // The kernel killed what went over (OOMPolicy=continue leaves the rest to end by itself) and the run failed with it
            Some(s) if !s.success() && oom_kills() > ooms => {
                // The user manager says only "the OOM killer killed some processes" of the slice: this says whose, and that it was
                // the plugin's own limit
                eprintln!("amethystora-plugin-host: plugin {id} was stopped: it went over its 256 MB memory limit. Nothing else was affected.");
                Err("it went over its 256 MB memory limit, and was stopped".into())
            }
            Some(s) if !s.success() => Err(format!("it ended with {s}")),
            _ => output::parse(&text, &loaded.manifest.actions),
        }
    }
}

/// Writes, as a run ends, what the plugin's proxy counted going out to each allowed host this hour into the account's
/// `sent.json`, which `amethystora-sends` reads (a count, never what was sent)
struct SentGuard<'a>(&'a Runner, String);

static SENT_WRITE: Mutex<()> = Mutex::new(());

impl Drop for SentGuard<'_> {
    fn drop(&mut self) {
        let Some(proxy) = self.0.proxies.lock().unwrap().get(&self.1).cloned() else { return };
        let _one = SENT_WRITE.lock().unwrap();
        let path = self.0.env.state_root.join("sent.json");
        let mut all: serde_json::Map<String, Value> = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let enabled = self.0.env.enabled();
        all.retain(|id, _| enabled.contains(id));
        all.insert(self.1.clone(), serde_json::json!({"t": crate::store::now(), "hour": proxy.bytes_sent()}));
        let temp = path.with_extension("json.new");
        if std::fs::write(&temp, Value::Object(all).to_string()).is_ok() {
            let _ = std::fs::rename(&temp, &path);
        }
    }
}

/// Ends a run's click: whatever the run did, the next one starts without it
struct ClickGuard(Option<Arc<Broker>>);

impl Drop for ClickGuard {
    fn drop(&mut self) {
        if let Some(b) = &self.0 {
            b.clicked.store(false, Ordering::Relaxed);
        }
    }
}

/// The read end of a pipe holding the plugin's settings as one JSON object, already written (a few KiB fit a pipe's buffer, so
/// nothing waits), with the write end closed
fn settings_pipe(env: &Env, id: &str) -> Result<std::fs::File, String> {
    use std::io::Write;
    use std::os::fd::FromRawFd;
    let all = crate::secrets::all(env, id)?;
    let text = serde_json::to_vec(&all).map_err(|e| e.to_string())?;
    if text.len() > 32 * 1024 {
        return Err("its settings are larger than a pipe holds".into());
    }
    let mut ends = [0i32; 2];
    // SAFETY: pipe2 fills the two descriptors it is given, which this function then owns
    if unsafe { libc::pipe2(ends.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // SAFETY: both descriptors are new and owned here
    let (read, mut write) = unsafe { (std::fs::File::from_raw_fd(ends[0]), std::fs::File::from_raw_fd(ends[1])) };
    write.write_all(&text).map_err(|e| e.to_string())?;
    Ok(read)
}

/// What kind of stop a failed run was, for the rules that pause a plugin: `slow` (it hit the 30 seconds), `memory`, `vault` (its secret
/// settings are in a closed Vault, which is waited for and not a fault), `gap` (it ran a moment ago: nothing happened) or `other`
pub fn classify(failure: &str) -> &'static str {
    if failure.starts_with("it ran a moment ago") || failure.starts_with("it is still running") {
        // Flow control, not a fault: a click and the timer met, or a run was still going
        "gap"
    } else if failure.starts_with("vault-closed") {
        "vault"
    } else if failure.contains("longer than 30 seconds") {
        "slow"
    } else if failure.contains("memory limit") || failure.contains("state folder grew") {
        "memory"
    } else {
        "other"
    }
}

/// How many times the kernel's OOM killer has acted in the plugins' slice (its memory.events, which counts its scopes' kills too),
/// or 0 when it cannot be read: the slice is in the account's user manager, which this process's own cgroup is under
fn oom_kills() -> u64 {
    let Ok(own) = std::fs::read_to_string("/proc/self/cgroup") else { return 0 };
    let Some(path) = own.lines().find_map(|l| l.strip_prefix("0::")) else { return 0 };
    let Some(end) = path.find(".service/").filter(|_| path.contains("/user@")) else { return 0 };
    let events = format!("/sys/fs/cgroup{}/{}/memory.events", &path[..end + ".service".len()], sandbox::SLICE_CGROUP);
    std::fs::read_to_string(events)
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("oom_kill ")?.trim().parse().ok()))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::classify;

    #[test]
    fn a_failure_is_told_by_its_kind_for_the_rules_that_pause_a_plugin() {
        assert_eq!(classify("it took longer than 30 seconds, and was stopped"), "slow");
        assert_eq!(classify("it went over its 256 MB memory limit, and was stopped"), "memory");
        assert_eq!(classify("its state folder grew past 8 MiB, and it was stopped"), "memory");
        assert_eq!(classify("vault-closed: its secret settings are in the Vault, which is closed"), "vault");
        assert_eq!(classify("it ran a moment ago: a plugin runs at most once in five seconds"), "gap");
        assert_eq!(classify("it is still running from the last time"), "gap");
        assert_eq!(classify("it ended with exit status: 1"), "other");
    }
}
