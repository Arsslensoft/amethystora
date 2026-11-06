//! What the tools changed: one JSON line per change in ~/.local/state/amethystora/agent-tools/actions.jsonl
//! (0600), with the state before it and how to undo it, never what a command printed. A change that takes
//! longer than a tool call may (an install, a root command, a terminal the user answers) is a job: the call
//! returns its id at once, and `history` with that id waits for it.

use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::policy;

pub fn state_dir() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| policy::home().join(".local/state"));
    base.join("amethystora/agent-tools")
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// A short id nobody else picks: eight hex digits from the kernel's random numbers
pub fn new_id() -> String {
    let mut bytes = [0u8; 4];
    if let Ok(mut random) = std::fs::File::open("/dev/urandom") {
        let _ = std::io::Read::read_exact(&mut random, &mut bytes);
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Adds a record. It is the account's own file, readable by it alone.
pub fn record(entry: &Value) -> Result<(), String> {
    let dir = state_dir();
    std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(dir.join("actions.jsonl"))
        .map_err(|error| error.to_string())?;
    // One write, so that a reader following the file never sees half a record
    file.write_all(format!("{entry}\n").as_bytes()).map_err(|error| error.to_string())
}

/// Every record, oldest first
pub fn records() -> Vec<Value> {
    std::fs::read_to_string(state_dir().join("actions.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// The latest record of a change, by its id: a job writes its record again when it ends
pub fn find(id: &str) -> Option<Value> {
    records().into_iter().rev().find(|record| record["id"] == id)
}

/// How long ago, in a few characters: 40s, 5m, 3h, 2d
pub fn ago(seconds: u64) -> String {
    let elapsed = now().saturating_sub(seconds);
    match elapsed {
        0..60 => format!("{elapsed}s"),
        60..3600 => format!("{}m", elapsed / 60),
        3600..86400 => format!("{}h", elapsed / 3600),
        _ => format!("{}d", elapsed / 86400),
    }
}

/// A record as one line for people and agents
pub fn line(record: &Value) -> String {
    let what = match record["tool"].as_str().unwrap_or("") {
        "change" => format!("change {}={} (was {})", record["entry"].as_str().unwrap_or("?"), record["value"].as_str().unwrap_or("?"),
            record["before"].as_str().unwrap_or("?")),
        "software" => format!("software {} {} {}", record["action"].as_str().unwrap_or("?"), record["source"].as_str().unwrap_or("?"),
            record["name"].as_str().unwrap_or("?")),
        "run" => format!("run{} {}", if record["root"] == true { " as root" } else { "" }, words(&record["argv"])),
        "undo" => format!("undo of {}", record["of"].as_str().unwrap_or("?")),
        other => other.to_string(),
    };
    let result = match record["result"].as_str().unwrap_or("?") {
        "running" | "opened" if !server_alive(record) => "did not finish",
        other => other,
    };
    let undo = if record["undo"].is_object() { ", can be undone" } else { "" };
    format!("{} {} ago: {what} -> {result}{undo}", record["id"].as_str().unwrap_or("?"), ago(record["time"].as_u64().unwrap_or(0)))
}

/// Whether the tool server that wrote a record still runs. One that ended left its unfinished records as they were.
pub fn server_alive(record: &Value) -> bool {
    record["server"].as_u64().is_some_and(|pid| std::path::Path::new(&format!("/proc/{pid}")).exists())
}

/// An argument vector as one line, each word quoted when it needs to be
pub fn words(argv: &Value) -> String {
    argv.as_array()
        .map(|list| list.iter().filter_map(Value::as_str).map(quote).collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

/// A word as a shell would need it to be the one word it is
pub fn quote(word: &str) -> String {
    if !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':' | '=' | '@' | '+' | ',')) {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// The jobs of this server, by id: running until their result is set
#[derive(Default, Clone)]
pub struct Jobs {
    done: Arc<(Mutex<HashMap<String, Option<String>>>, Condvar)>,
}

impl Jobs {
    pub fn start(&self, id: &str) {
        self.done.0.lock().expect("not poisoned").insert(id.to_string(), None);
    }

    pub fn finish(&self, id: &str, result: String) {
        let (lock, wake) = &*self.done;
        lock.lock().expect("not poisoned").insert(id.to_string(), Some(result));
        wake.notify_all();
    }

    pub fn known(&self, id: &str) -> bool {
        self.done.0.lock().expect("not poisoned").contains_key(id)
    }

    /// Waits until every job of this process has ended, at most `wait` in all: a command line that started one
    /// (an undo from the live pill) must not exit while it runs
    pub fn wait_all(&self, wait: Duration) {
        let (lock, wake) = &*self.done;
        let jobs = lock.lock().expect("not poisoned");
        let _ = wake.wait_timeout_while(jobs, wait, |jobs| jobs.values().any(Option::is_none)).expect("not poisoned");
    }

    /// The job's result, waiting at most `wait` for it; None while it still runs
    pub fn wait(&self, id: &str, wait: Duration) -> Option<String> {
        let (lock, wake) = &*self.done;
        let jobs = lock.lock().expect("not poisoned");
        let (jobs, _) = wake.wait_timeout_while(jobs, wait, |jobs| matches!(jobs.get(id), Some(None))).expect("not poisoned");
        jobs.get(id).cloned().flatten()
    }
}

/// The folder of the account's running tool servers, in its runtime folder: one file each, named by its process
/// number, there while it runs. The live pill reads it to tell the agent's session from the ones before, and Stop
/// (amethystora-agent stop) ends the servers it names.
pub fn sessions_dir() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })));
    runtime.join("amethystora/agent-sessions")
}

/// This tool server's file in sessions_dir(), taken away when it ends: at the end of its input, and when a signal ends
/// it (an agent's terminal closed sends SIGHUP, an agent or systemd stopping it SIGTERM)
pub struct Session(Option<PathBuf>);

static SESSION_FILE: std::sync::OnceLock<std::ffi::CString> = std::sync::OnceLock::new();

// Only what may run in a signal handler: unlink and _exit
extern "C" fn session_ended(signal: libc::c_int) {
    if let Some(path) = SESSION_FILE.get() {
        unsafe { libc::unlink(path.as_ptr()) };
    }
    unsafe { libc::_exit(128 + signal) };
}

/// What this server is doing, for the live pill's one line (R7-13): the read tool it runs and the topic of `state`, both from
/// fixed lists and never an argument, and the last few it finished. Written into the session's own file, in the account's
/// runtime folder (memory), by a rename, so that the pill's file monitor never reads half of it.
struct Notes {
    path: PathBuf,
    started: u64,
    now: Option<Value>,
    recent: Vec<Value>,
}

static NOTES: Mutex<Option<Notes>> = Mutex::new(None);
/// How many finished steps the file keeps
const RECENT: usize = 3;

fn write_notes(notes: &Notes) -> std::io::Result<()> {
    let body = json!({"pid": std::process::id(), "started": notes.started, "now": notes.now, "recent": notes.recent});
    let new = notes.path.with_extension("new");
    let mut file = std::fs::OpenOptions::new().create(true).truncate(true).write(true).mode(0o600).open(&new)?;
    writeln!(file, "{body}")?;
    std::fs::rename(&new, &notes.path)
}

fn step(tool: &str, topic: Option<&str>) -> Value {
    json!({"tool": tool, "topic": topic, "at": now()})
}

/// A read tool starts: only the names the tools themselves have, and `state`'s topic from its own list
pub fn working(tool: &str, topic: Option<&str>) {
    if !crate::server::READ_ONLY.contains(&tool) {
        return;
    }
    let topic = topic.filter(|topic| tool == "state" && crate::state::TOPICS.contains(topic));
    if let Some(notes) = NOTES.lock().expect("not poisoned").as_mut() {
        notes.now = Some(step(tool, topic));
        let _ = write_notes(notes);
    }
}

/// The tool that was running has finished, or failed
pub fn worked() {
    if let Some(notes) = NOTES.lock().expect("not poisoned").as_mut() {
        if let Some(done) = notes.now.take() {
            notes.recent.push(done);
            if notes.recent.len() > RECENT {
                notes.recent.remove(0);
            }
        }
        let _ = write_notes(notes);
    }
}

impl Session {
    pub fn start() -> Session {
        let dir = sessions_dir();
        let path = dir.join(std::process::id().to_string());
        let started = now();
        let written = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir).and_then(|_| {
            let mut file = std::fs::OpenOptions::new().create(true).truncate(true).write(true).mode(0o600).open(&path)?;
            writeln!(file, "{}", json!({"pid": std::process::id(), "started": started, "now": null, "recent": []}))
        });
        if written.is_ok() {
            *NOTES.lock().expect("not poisoned") = Some(Notes { path: path.clone(), started, now: None, recent: Vec::new() });
            if let Ok(name) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) {
                let _ = SESSION_FILE.set(name);
                for signal in [libc::SIGTERM, libc::SIGHUP, libc::SIGINT] {
                    unsafe { libc::signal(signal, session_ended as extern "C" fn(libc::c_int) as libc::sighandler_t) };
                }
            }
        }
        Session(written.ok().map(|_| path))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            *NOTES.lock().expect("not poisoned") = None;
            let _ = std::fs::remove_file(path);
            let _ = std::fs::remove_file(path.with_extension("new"));
        }
    }
}

/// A record of a change as it starts
pub fn started(id: &str, tool: &str, fields: Value) -> Value {
    let mut entry = json!({"id": id, "time": now(), "tool": tool, "result": "running", "server": std::process::id()});
    if let (Some(entry), Some(fields)) = (entry.as_object_mut(), fields.as_object()) {
        for (key, value) in fields {
            entry.insert(key.clone(), value.clone());
        }
    }
    entry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_and_says_records() {
        assert_eq!(quote("rpm-ostree"), "rpm-ostree");
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("it's"), "'it'\\''s'");
        assert_eq!(quote(""), "''");
        let record = json!({"id": "ab12cd34", "time": now(), "tool": "change", "entry": "layout", "value": "classic", "before": "tiling",
            "result": "ok", "undo": {"tool": "change", "entry": "layout", "value": "tiling"}});
        assert_eq!(line(&record), "ab12cd34 0s ago: change layout=classic (was tiling) -> ok, can be undone");
        assert_eq!(words(&json!(["rpm-ostree", "install", "a b"])), "rpm-ostree install 'a b'");
        let mut running = started("ab12cd34", "undo", json!({"of": "ef56"}));
        assert!(line(&running).ends_with("undo of ef56 -> running"));
        running["server"] = json!(u32::MAX);
        assert!(line(&running).ends_with("undo of ef56 -> did not finish"));
    }

    #[test]
    fn the_session_file_says_which_read_tool_runs() {
        let dir = std::env::temp_dir().join(format!("amethystora-notes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("4242");
        *NOTES.lock().unwrap() = Some(Notes { path: path.clone(), started: 7, now: None, recent: Vec::new() });
        let read = || serde_json::from_str::<Value>(&std::fs::read_to_string(&path).unwrap()).unwrap();
        // A topic outside the list, and a tool that changes things, are never written: only fixed names are
        working("state", Some("drivers; rm"));
        assert_eq!(read()["now"]["tool"], "state");
        assert_eq!(read()["now"]["topic"], Value::Null);
        working("state", Some("updates"));
        assert_eq!(read()["now"]["topic"], "updates");
        worked();
        assert_eq!(read()["now"], Value::Null);
        assert_eq!(read()["recent"][0]["topic"], "updates");
        working("software", None);
        assert_eq!(read()["now"], Value::Null);
        working("search", Some("updates"));
        assert_eq!(read()["now"]["topic"], Value::Null);
        for _ in 0..5 {
            worked();
            working("read", None);
        }
        worked();
        assert_eq!(read()["recent"].as_array().unwrap().len(), RECENT);
        // Never anything of what was asked for or read
        assert!(!read().to_string().contains("rm"));
        *NOTES.lock().unwrap() = None;
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn jobs_wait_for_their_result() {
        let jobs = Jobs::default();
        jobs.start("x");
        assert_eq!(jobs.wait("x", Duration::from_millis(20)), None);
        let other = jobs.clone();
        let thread = std::thread::spawn(move || other.finish("x", "done".into()));
        assert_eq!(jobs.wait("x", Duration::from_secs(5)).as_deref(), Some("done"));
        thread.join().unwrap();
        assert!(!jobs.known("y"));
        // wait_all returns once every job has ended
        jobs.start("z");
        let other = jobs.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            other.finish("z", "ended".into());
        });
        jobs.wait_all(Duration::from_secs(5));
        assert_eq!(jobs.wait("z", Duration::ZERO).as_deref(), Some("ended"));
        thread.join().unwrap();
    }
}
