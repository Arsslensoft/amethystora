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

use crate::conf::{Allow, CONF, Conf};
use crate::manifest::Manifest;
use crate::output::{self, MAX_OUTPUT};
use crate::proxy::{Policy, Proxy};
use crate::sandbox::{self, SECCOMP_FD, SECCOMP_FILE, Spec};
use crate::store::{Grant, Store};
use crate::files;

pub const RUN_LIMIT: Duration = Duration::from_secs(30);
pub const MIN_GAP: Duration = Duration::from_secs(5);
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
}

impl Env {
    pub fn production() -> Result<Env, String> {
        // SAFETY: getuid has no arguments and cannot fail
        let uid = unsafe { libc::getuid() };
        let home = PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?);
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(|| format!("/run/user/{uid}").into());
        let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/state"));
        Ok(Env {
            store: Store::production(),
            uid,
            home,
            runtime: runtime.join("amethystora/plugins"),
            state_root: state.join("amethystora/plugins"),
            conf_path: CONF.into(),
            seccomp: SECCOMP_FILE.into(),
        })
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

/// A plugin as installed, and only if its files are exactly what was approved
pub fn load(env: &Env, id: &str) -> Result<Loaded, String> {
    if env.conf().allow == Allow::Off {
        return Err("plugins are turned off on this machine (ALLOW=off in /etc/amethystora/plugins.conf)".into());
    }
    let grant = env.store.grants(env.uid)?.plugins.remove(id).ok_or_else(|| format!("{id} is not installed"))?;
    let folder = env.store.folder_of(id, &grant);
    env.store.verify(&folder, &grant.digest).map_err(|e| format!("{e}; it is stopped until you decide what to do"))?;
    let text = std::fs::read_to_string(folder.join("manifest.json")).map_err(|e| e.to_string())?;
    let manifest = Manifest::parse_with(&text, grant.shipped)?;
    if manifest.id != id {
        return Err("the manifest's id is not the one installed".into());
    }
    Ok(Loaded { id: id.into(), folder, manifest, grant })
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
        Runner { env, last: Mutex::new(HashMap::new()), proxies: Mutex::new(HashMap::new()), over_limit, running: Mutex::new(Default::default()) }
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
        {
            let mut last = self.last.lock().unwrap();
            if last.get(id).is_some_and(|t| t.elapsed() < MIN_GAP) {
                return Err("it ran a moment ago: a plugin runs at most once in five seconds".into());
            }
            last.insert(id.to_string(), Instant::now());
        }
        let line = self.command_line(&loaded, action, true)?;
        let filter = std::fs::File::open(&self.env.seccomp).map_err(|_| format!("{} is missing, so nothing is run", self.env.seccomp.display()))?;
        let mut command = Command::new(&line[0]);
        command.args(&line[1..]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        let fd = filter.as_raw_fd();
        // SAFETY: only dup2 and fcntl, which are async-signal-safe, between fork and exec
        unsafe {
            command.pre_exec(move || {
                // TODO(phase 0): the descriptor has to get through systemd-run --scope into bwrap; confirm it does
                // No file the plugin writes is larger than its state folder may be
                let limit = libc::rlimit { rlim_cur: STATE_LIMIT, rlim_max: STATE_LIMIT };
                if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::dup2(fd, SECCOMP_FD) < 0 || (fd == SECCOMP_FD && libc::fcntl(fd, libc::F_SETFD, 0) < 0) {
                    return Err(std::io::Error::last_os_error());
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
            // The kernel kills one process, and the user manager stops the rest of the scope (SIGTERM to bubblewrap)
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
