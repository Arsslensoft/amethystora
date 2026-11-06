//! What a plugin may ask the host for (plans/PLAN-plugins.md, the permissions table): the broker's side. A sandbox has no bus, no
//! display and no home, so a plugin that wants the time's format, the disk's fullness or what plays asks `plugin state NAME`, and
//! one that wants to tell a person something asks `plugin notify`. The sandbox sees one socket, `/broker/broker.sock`, held for it
//! by the host; every request is one line of JSON and gets one line back, and is judged by grants.json as it is *now*, so that a
//! revoke applies at once.
//!
//!   {"op":"state","name":"clock"}              {"ok":true,"data":{...}}      needs state=clock
//!   {"op":"control","name":"media","arg":"next"}                              needs control=media, and a click on the plugin's own element
//!   {"op":"notify","title":"...","body":"..."} needs notify, once a minute at most
//!   {"op":"open","url":"https://..."}          needs open, and a click; the shell says the address
//!   {"op":"clipboard","text":"..."}            needs clipboard-write, and a click; there is no clipboard read
//!   anything refused                           {"ok":false,"why":"..."}
//!
//! "A click" is a run the person started from the plugin's own element (an action): a run on the plugin's timer never changes
//! anything outside the sandbox. State is what the host reads for it with the account's authority; the plugin never sees a bus.
//! A draft holds no grant, so it is answered from samples and can change nothing.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::host::Env;
use crate::output::clean;

/// One request line, at most
const MAX_LINE: usize = 8 * 1024;
/// A plugin's notices: one a minute
const NOTIFY_GAP: Duration = Duration::from_secs(60);
/// How long a command of the host's own, asked for on a plugin's behalf, may take
const TOOL_SECONDS: &str = "2";

/// What the shell is asked to do for a plugin, which only it can: (plugin id, kind, text)
pub type Told = Arc<dyn Fn(&str, &str, &str) + Send + Sync>;

pub struct Broker {
    env: Arc<Env>,
    id: String,
    /// Whether the run in progress was started by a click on the plugin's own element
    pub clicked: AtomicBool,
    last_notify: Mutex<Option<Instant>>,
    told: Told,
}

fn tool(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("timeout").arg(TOOL_SECONDS).arg(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn gsetting(schema: &str, key: &str) -> Option<String> {
    tool("gsettings", &["get", schema, key])
}

/// The players on the session bus, as MPRIS names them
fn players() -> Vec<String> {
    let Some(text) = tool("busctl", &["--user", "call", "--json=short", "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "ListNames"]) else { return Vec::new() };
    let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    value["data"][0].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|n| n.starts_with("org.mpris.MediaPlayer2.")).map(str::to_string).collect()
}

fn mpris(player: &str, property: &str) -> Value {
    tool("busctl", &["--user", "get-property", "--json=short", player, "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player", property])
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

/// What plays: the player playing, else the one paused, else null. Titles are the player's own words, cleaned here and drawn as text
fn media() -> Value {
    let mut paused = None;
    let mut chosen = None;
    for player in players() {
        match mpris(&player, "PlaybackStatus")["data"].as_str() {
            Some("Playing") => {
                chosen = Some((player, "Playing"));
                break;
            }
            Some("Paused") if paused.is_none() => paused = Some((player, "Paused")),
            _ => {}
        }
    }
    let Some((player, status)) = chosen.or(paused) else { return Value::Null };
    let meta = mpris(&player, "Metadata");
    let title = clean(meta["data"]["xesam:title"]["data"].as_str().unwrap_or(""), 80);
    let artist = meta["data"]["xesam:artist"]["data"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    json!({"status": status, "title": title, "artist": clean(&artist, 80)})
}

fn disk(home: &std::path::Path) -> Value {
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = std::ffi::CString::new(home.as_os_str().as_bytes()) else { return Value::Null };
    // SAFETY: a zeroed statvfs is a valid out parameter, and the path is a NUL-terminated string for the call
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 || stat.f_blocks == 0 {
        return Value::Null;
    }
    let (total, free) = (stat.f_blocks as f64 * stat.f_frsize as f64, stat.f_bavail as f64 * stat.f_frsize as f64);
    json!({"used_percent": ((total - free) * 100.0 / total).round() as u64, "total_gb": ((total / 1e9) * 10.0).round() / 10.0, "free_gb": ((free / 1e9) * 10.0).round() / 10.0})
}

/// What a name's state is, or why it cannot be said: only names the host knows, and nothing a plugin says reaches a command
pub fn state(env: &Env, name: &str) -> Result<Value, String> {
    match name {
        "clock" => Ok(json!({"format": if gsetting("org.gnome.desktop.interface", "clock-format").is_some_and(|f| f.contains("12h")) { "12h" } else { "24h" }})),
        "disk" => Ok(disk(&env.home)),
        "media" => Ok(media()),
        "do-not-disturb" => Ok(json!({"on": gsetting("org.gnome.desktop.notifications", "show-banners").is_some_and(|v| v == "false")})),
        other => Err(format!("this computer cannot say {other} to a plugin yet")),
    }
}

/// What a draft is told: samples, so that it can be tried before it is kept
fn sample(name: &str) -> Value {
    match name {
        "clock" => json!({"format": "24h"}),
        "disk" => json!({"used_percent": 42, "total_gb": 500.0, "free_gb": 290.0}),
        "media" => json!({"status": "Playing", "title": "A sample song", "artist": "A sample artist"}),
        "do-not-disturb" => json!({"on": false}),
        _ => Value::Null,
    }
}

fn control(name: &str, arg: &str) -> Result<(), String> {
    match (name, arg) {
        ("media", "play-pause" | "next" | "previous") => {
            let method = match arg {
                "next" => "Next",
                "previous" => "Previous",
                _ => "PlayPause",
            };
            let current = media();
            let player = players().into_iter().find(|p| mpris(p, "PlaybackStatus")["data"].as_str() == Some(current["status"].as_str().unwrap_or("-")));
            let player = player.ok_or("no player is open")?;
            tool("busctl", &["--user", "call", &player, "/org/mpris/MediaPlayer2", "org.mpris.MediaPlayer2.Player", method]).map(|_| ()).ok_or_else(|| "the player did not answer".into())
        }
        ("do-not-disturb", "on" | "off") => {
            // Do Not Disturb on is banners off
            let value = if arg == "on" { "false" } else { "true" };
            tool("gsettings", &["set", "org.gnome.desktop.notifications", "show-banners", value]).map(|_| ()).ok_or_else(|| "it could not be changed".into())
        }
        _ => Err("that is not something a plugin may control".into()),
    }
}

impl Broker {
    pub fn new(env: Arc<Env>, id: &str, told: Told) -> Arc<Broker> {
        Arc::new(Broker { env, id: id.into(), clicked: AtomicBool::new(false), last_notify: Mutex::new(None), told })
    }

    /// The permissions it holds now, read again for every request (a revoke applies at once), and whether it is a draft
    fn held(&self) -> (Vec<String>, bool) {
        match self.env.store.grants(self.env.uid).ok().and_then(|mut g| g.plugins.remove(&self.id)) {
            Some(g) => (g.perms, false),
            None => (Vec::new(), true),
        }
    }

    fn name_of(&self) -> String {
        crate::host::load(&self.env, &self.id).map(|l| l.manifest.name).unwrap_or_else(|_| self.id.clone())
    }

    /// One request's answer
    pub fn handle(&self, request: &Value) -> Value {
        let (perms, draft) = self.held();
        let has = |p: &str| perms.iter().any(|x| x == p);
        let text = |key: &str, max: usize| clean(request[key].as_str().unwrap_or(""), max);
        let answer: Result<Value, String> = (|| match request["op"].as_str().unwrap_or("") {
            "state" => {
                let name = request["name"].as_str().unwrap_or("");
                if draft {
                    return Ok(sample(name));
                }
                if !has(&format!("state={name}")) {
                    return Err(format!("it was not allowed to read {name}"));
                }
                state(&self.env, name)
            }
            op @ ("control" | "open" | "clipboard" | "notify") if draft => Err(format!("a draft cannot {op}: it has no permissions until you keep it")),
            "control" => {
                let name = request["name"].as_str().unwrap_or("");
                if !has(&format!("control={name}")) {
                    return Err(format!("it was not allowed to control {name}"));
                }
                if !self.clicked.load(Ordering::Relaxed) {
                    return Err("this is only done after a click on the plugin's own element".into());
                }
                control(name, request["arg"].as_str().unwrap_or("")).map(|_| Value::Null)
            }
            "notify" => {
                if !has("notify") {
                    return Err("it was not allowed to notify".into());
                }
                let mut last = self.last_notify.lock().unwrap();
                if last.is_some_and(|t| t.elapsed() < NOTIFY_GAP) {
                    return Err("a plugin may notify once a minute".into());
                }
                *last = Some(Instant::now());
                let (title, body) = (text("title", 80), text("body", 200));
                if title.is_empty() {
                    return Err("a notice has a title".into());
                }
                // Under the plugin's name, never the system's
                let name = self.name_of();
                let _ = Command::new("notify-send").args(["--app-name", &name, "--", &format!("{name}: {title}"), &body]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
                Ok(Value::Null)
            }
            "open" | "clipboard" => {
                let (kind, key, perm) = if request["op"] == "open" { ("open", "url", "open") } else { ("clipboard", "text", "clipboard-write") };
                if !has(perm) {
                    return Err(format!("it was not allowed to {perm}"));
                }
                if !self.clicked.load(Ordering::Relaxed) {
                    return Err("this is only done after a click on the plugin's own element".into());
                }
                let value = text(key, if kind == "open" { 500 } else { 2000 });
                if kind == "open" && !(value.starts_with("https://") || value.starts_with("http://")) {
                    return Err("only a web address can be opened".into());
                }
                (self.told)(&self.id, kind, &value);
                Ok(Value::Null)
            }
            _ => Err("not something a plugin can ask".into()),
        })();
        match answer {
            Ok(data) => json!({"ok": true, "data": data}),
            Err(why) => json!({"ok": false, "why": why}),
        }
    }

    fn serve_one(self: &Arc<Self>, stream: UnixStream) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
        let mut line = Vec::new();
        let Ok(clone) = stream.try_clone() else { return };
        if BufReader::new(clone.take(MAX_LINE as u64)).read_until(b'\n', &mut line).is_err() {
            return;
        }
        let request: Value = serde_json::from_slice(&line).unwrap_or(Value::Null);
        let answer = self.handle(&request);
        let mut stream = stream;
        let _ = stream.write_all(format!("{answer}\n").as_bytes());
    }

    /// Answers on the socket until the host ends. A thread for each connection, which a plugin's task limit already bounds
    pub fn serve(self: &Arc<Self>, listener: UnixListener) {
        for stream in listener.incoming().flatten() {
            let broker = self.clone();
            let _ = std::thread::Builder::new().spawn(move || broker.serve_one(stream));
        }
    }
}

/// `plugin ...` inside the sandbox: asks the socket one thing and prints the answer, or says why it was refused
pub fn client(args: &[String]) -> i32 {
    let Some(op) = args.first().map(String::as_str) else {
        eprintln!("usage: plugin state NAME | control NAME ARG | notify TITLE [BODY] | open URL | clipboard TEXT");
        return 64;
    };
    let arg = |n: usize| args.get(n).cloned().unwrap_or_default();
    let request = match op {
        "state" => json!({"op": "state", "name": arg(1)}),
        "control" => json!({"op": "control", "name": arg(1), "arg": arg(2)}),
        "notify" => json!({"op": "notify", "title": arg(1), "body": arg(2)}),
        "open" => json!({"op": "open", "url": arg(1)}),
        "clipboard" => {
            let mut text = arg(1);
            if text == "-" {
                text.clear();
                let _ = std::io::stdin().take(4000).read_to_string(&mut text);
            }
            json!({"op": "clipboard", "text": text})
        }
        _ => {
            eprintln!("plugin: {op} is not something a plugin can ask");
            return 64;
        }
    };
    let socket = std::env::var("PLUGIN_BROKER").unwrap_or_else(|_| "/broker/broker.sock".into());
    let Ok(mut stream) = UnixStream::connect(&socket) else {
        eprintln!("plugin: this plugin was given nothing to ask for");
        return 2;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.write_all(format!("{request}\n").as_bytes());
    let mut line = String::new();
    let _ = BufReader::new(stream).read_line(&mut line);
    let answer: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
    if answer["ok"] == true {
        match &answer["data"] {
            Value::Null => {}
            Value::String(s) => println!("{s}"),
            data => println!("{data}"),
        }
        0
    } else {
        eprintln!("plugin: {}", answer["why"].as_str().unwrap_or("no answer"));
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{Grant, Grants, Store};

    fn env_with(perms: &[&str]) -> (Arc<Env>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("amethystora-broker-{}-{}", std::process::id(), perms.join("").len()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store { root: dir.clone(), shipped: dir.join("shipped") };
        let mut grants = Grants::default();
        grants.plugins.insert("dev.example.t".into(), Grant { digest: "d".into(), perms: perms.iter().map(|p| p.to_string()).collect(), ..Grant::default() });
        store.write_grants(1000, &grants).unwrap();
        let env = Env {
            store,
            uid: 1000,
            home: dir.clone(),
            runtime: dir.join("run"),
            state_root: dir.join("state"),
            conf_path: dir.join("none.conf"),
            seccomp: dir.join("s.bpf"),
            drafts_root: dir.join("drafts"),
            vault_socket: dir.join("vault.sock"),
        };
        (Arc::new(env), dir)
    }

    fn broker(perms: &[&str]) -> (Arc<Broker>, Arc<Mutex<Vec<(String, String)>>>, std::path::PathBuf) {
        let (env, dir) = env_with(perms);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        (Broker::new(env, "dev.example.t", Arc::new(move |_, kind, text| sink.lock().unwrap().push((kind.into(), text.into())))), seen, dir)
    }

    #[test]
    fn nothing_is_answered_that_was_not_granted() {
        let (b, _, dir) = broker(&["state=disk"]);
        assert_eq!(b.handle(&json!({"op": "state", "name": "disk"}))["ok"], true);
        assert_eq!(b.handle(&json!({"op": "state", "name": "clock"}))["ok"], false);
        assert_eq!(b.handle(&json!({"op": "notify", "title": "x"}))["ok"], false);
        assert_eq!(b.handle(&json!({"op": "root"}))["ok"], false);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_change_outside_the_sandbox_needs_a_click() {
        let (b, seen, dir) = broker(&["open", "clipboard-write", "control=media"]);
        for request in [json!({"op": "open", "url": "https://example.org/a"}), json!({"op": "clipboard", "text": "x"}), json!({"op": "control", "name": "media", "arg": "next"})] {
            assert_eq!(b.handle(&request)["ok"], false, "{request}");
        }
        assert!(seen.lock().unwrap().is_empty());
        b.clicked.store(true, Ordering::Relaxed);
        assert_eq!(b.handle(&json!({"op": "open", "url": "https://example.org/a"}))["ok"], true);
        assert_eq!(b.handle(&json!({"op": "open", "url": "file:///etc/shadow"}))["ok"], false);
        assert_eq!(b.handle(&json!({"op": "open", "url": "javascript:1"}))["ok"], false);
        assert_eq!(b.handle(&json!({"op": "clipboard", "text": "a\u{202e}b"}))["ok"], true);
        assert_eq!(*seen.lock().unwrap(), [("open".to_string(), "https://example.org/a".to_string()), ("clipboard".to_string(), "ab".to_string())]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_draft_is_answered_from_samples_and_changes_nothing() {
        let (env, dir) = env_with(&[]);
        let b = Broker::new(env, "dev.example.draft", Arc::new(|_, _, _| panic!("a draft cannot ask the shell")));
        assert_eq!(b.handle(&json!({"op": "state", "name": "disk"}))["data"]["used_percent"], 42);
        b.clicked.store(true, Ordering::Relaxed);
        for op in ["open", "clipboard", "control", "notify"] {
            assert_eq!(b.handle(&json!({"op": op, "url": "https://example.org", "title": "x", "name": "media"}))["ok"], false, "{op}");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_state_that_is_not_known_is_refused() {
        let (env, dir) = env_with(&[]);
        assert!(state(&env, "battery").is_err());
        assert!(state(&env, "disk").unwrap()["used_percent"].is_u64());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
