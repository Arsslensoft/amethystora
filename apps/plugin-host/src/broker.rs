//! What a plugin may ask the host for (plans/done/PLAN-plugins.md, the permissions table): the broker's side. A sandbox has no bus, no
//! display and no home, so a plugin that wants the time's format, the disk's fullness or what plays asks `plugin state NAME`, and
//! one that wants to tell a person something asks `plugin notify`. The sandbox sees one socket, `/broker/broker.sock`, held for it
//! by the host; every request is one line of JSON and gets one line back, and is judged by grants.json as it is *now*, so that a
//! revoke applies at once.
//!
//!   {"op":"state","name":"clock"}              {"ok":true,"data":{...}}      needs state=clock
//!   {"op":"control","name":"media","arg":"next"}                              needs control=media, and a click on the plugin's own element
//!   {"op":"notify","title":"...","body":"..."} needs notify, once a minute at most; with "action" (one of the manifest's) and
//!                                              "label", the notice has that button, and pressing it is a click that runs the action
//!   {"op":"open","url":"https://..."}          needs open, and a click; the shell says the address
//!   {"op":"clipboard","text":"..."}            needs clipboard-write, and a click; there is no clipboard read
//!   {"op":"change","entry":"...","value":"..."} needs change=ENTRY, and a click: Control's entry set as the agent's change sets it
//!                                              (amethystora-agent-tools plugin-change: no terminal, the never list, recorded)
//!   {"op":"event"}                             needs events=NAME: what started this run, {"name","value"}, or null for a run of the timer
//!   {"op":"pick"}                              needs pick, and a click: the file a person chose (in a chooser the host shows, or the one
//!                                              they gave the plugin in Files), as {"ok":true,"data":{"name","size"}} and then exactly
//!                                              that many bytes. The plugin never sees a path or anything beside the file
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
    /// The file a person gave the plugin in Files' menu for the run in progress, which its `pick` takes without a chooser
    pub handed: Mutex<Option<std::path::PathBuf>>,
    /// The news that started the run in progress, if it was news (`events`): {"name", "value"}
    pub event: Mutex<Option<Value>>,
}

/// The largest file a plugin is given
pub const PICK_BYTES: u64 = 8 * 1024 * 1024;
/// How long the chooser waits for a person: less than a run's 30 seconds
const PICK_SECONDS: &str = "25";

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
        Arc::new(Broker { env, id: id.into(), clicked: AtomicBool::new(false), last_notify: Mutex::new(None), told, handed: Mutex::new(None), event: Mutex::new(None) })
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

    /// `pick`: the file handed over, or one the person chooses now, read here (a plain file, never past PICK_BYTES). The run's click
    /// is spent by it, as by an open
    pub fn pick(&self) -> Result<(String, Vec<u8>), String> {
        let (perms, draft) = self.held();
        if draft || !perms.iter().any(|p| p == "pick") {
            return Err("it was not allowed to ask for a file".into());
        }
        if !self.clicked.swap(false, Ordering::Relaxed) {
            return Err("a file is only asked for after a click on the plugin's own element, once for each click".into());
        }
        let path = match self.handed.lock().unwrap().take() {
            Some(path) => path,
            None => {
                let title = format!("{}: choose one file to give it", self.name_of());
                let out = Command::new("timeout")
                    .args([PICK_SECONDS, "zenity", "--file-selection", "--title", &title])
                    .stdin(Stdio::null())
                    .stderr(Stdio::null())
                    .output()
                    .map_err(|_| "no file chooser could be shown".to_string())?;
                let chosen = String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_string();
                if !out.status.success() || chosen.is_empty() {
                    return Err("no file was chosen".into());
                }
                std::path::PathBuf::from(chosen)
            }
        };
        let file = std::fs::File::open(&path).map_err(|e| format!("the file cannot be read: {e}"))?;
        let meta = file.metadata().map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.len() > PICK_BYTES {
            return Err("only a plain file of at most 8 MiB is given to a plugin".into());
        }
        let mut data = Vec::with_capacity(meta.len() as usize);
        file.take(PICK_BYTES + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
        if data.len() as u64 > PICK_BYTES {
            return Err("only a plain file of at most 8 MiB is given to a plugin".into());
        }
        let name = clean(&path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), 200);
        Ok((name, data))
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
            "event" => {
                if draft {
                    return Ok(Value::Null);
                }
                if !perms.iter().any(|p| p.starts_with("events=")) {
                    return Err("it was not allowed any news".into());
                }
                Ok(self.event.lock().unwrap().clone().unwrap_or(Value::Null))
            }
            "change" if !draft => {
                let (entry, value) = (request["entry"].as_str().unwrap_or(""), request["value"].as_str().unwrap_or(""));
                let shaped = |t: &str, max: usize| !t.is_empty() && t.len() <= max && t.bytes().all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b));
                if !shaped(entry, 64) || !shaped(value, 64) {
                    return Err("a change names an entry and one of its values".into());
                }
                if !has(&format!("change={entry}")) {
                    return Err(format!("it was not allowed to change {entry}"));
                }
                if !self.clicked.swap(false, Ordering::Relaxed) {
                    return Err("a change is only made after a click on the plugin's own element, once for each click".into());
                }
                let out = Command::new("/usr/libexec/amethystora-agent-tools")
                    .args(["plugin-change", &self.id, entry, value])
                    .stdin(Stdio::null())
                    .output()
                    .map_err(|_| "the change could not be made".to_string())?;
                let said = clean(&String::from_utf8_lossy(if out.status.success() { &out.stdout } else { &out.stderr }), 200);
                if out.status.success() { Ok(Value::String(said)) } else { Err(said) }
            }
            op @ ("control" | "open" | "clipboard" | "notify" | "change") if draft => Err(format!("a draft cannot {op}: it has no permissions until you keep it")),
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
                // GNOME draws a notice's body as markup: what a plugin wrote is text, so its angle brackets and ampersands are escaped
                let esc = |t: &str| t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                let mut args = vec!["--app-name".to_string(), name.clone()];
                // One button, which can only name an action of the plugin's own manifest: notify-send waits for the answer on a thread
                // of its own, and the press reaches the host as a click on the plugin's element would
                let action = request["action"].as_str().unwrap_or("").to_string();
                let button = if action.is_empty() {
                    None
                } else {
                    let actions = crate::host::load(&self.env, &self.id).map(|l| l.manifest.actions).unwrap_or_default();
                    if !actions.contains(&action) {
                        return Err(format!("{action:?} is not one of the plugin's actions"));
                    }
                    let label = text("label", 40);
                    if label.is_empty() {
                        return Err("a notice's button has a label".into());
                    }
                    args.extend(["--wait".to_string(), format!("--action={action}={}", esc(&label))]);
                    Some(action)
                };
                args.extend(["--".to_string(), esc(&format!("{name}: {title}")), esc(&body)]);
                let mut command = Command::new("notify-send");
                command.args(&args).stdin(Stdio::null()).stderr(Stdio::null());
                match button {
                    None => {
                        let _ = command.stdout(Stdio::null()).status();
                    }
                    Some(action) => {
                        let (told, id) = (self.told.clone(), self.id.clone());
                        let _ = std::thread::Builder::new().spawn(move || {
                            let Ok(out) = command.stdout(Stdio::piped()).output() else { return };
                            if String::from_utf8_lossy(&out.stdout).trim() == action {
                                told(&id, "action", &action);
                            }
                        });
                    }
                }
                Ok(Value::Null)
            }
            "open" | "clipboard" => {
                let (kind, key, perm) = if request["op"] == "open" { ("open", "url", "open") } else { ("clipboard", "text", "clipboard-write") };
                if !has(perm) {
                    return Err(format!("it was not allowed to {perm}"));
                }
                // One click is one open or one copy: the flag is spent by the first, so a loop cannot open tabs or rewrite the clipboard
                if !self.clicked.swap(false, Ordering::Relaxed) {
                    return Err("this is only done after a click on the plugin's own element, once for each click".into());
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
        let mut stream = stream;
        if request["op"] == "pick" {
            // The chooser waits for a person, longer than a request's usual five seconds
            let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
            match self.pick() {
                Ok((name, data)) => {
                    let _ = stream.write_all(format!("{}\n", json!({"ok": true, "data": {"name": name, "size": data.len()}})).as_bytes());
                    let _ = stream.write_all(&data);
                }
                Err(why) => {
                    let _ = stream.write_all(format!("{}\n", json!({"ok": false, "why": why})).as_bytes());
                }
            }
            return;
        }
        let answer = self.handle(&request);
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

/// `plugin pick FILE` inside the sandbox: the file a person chose, written to FILE (in /tmp or the plugin's state), and its name printed
fn pick_into(target: &str) -> i32 {
    if target.is_empty() {
        eprintln!("usage: plugin pick FILE (where the chosen file is written, such as /tmp/picked)");
        return 64;
    }
    let socket = std::env::var("PLUGIN_BROKER").unwrap_or_else(|_| "/broker/broker.sock".into());
    let Ok(mut stream) = UnixStream::connect(&socket) else {
        eprintln!("plugin: this plugin was given nothing to ask for");
        return 2;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = stream.write_all(b"{\"op\":\"pick\"}\n");
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let _ = reader.read_line(&mut line);
    let answer: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
    if answer["ok"] != true {
        eprintln!("plugin: {}", answer["why"].as_str().unwrap_or("no answer"));
        return 1;
    }
    let size = answer["data"]["size"].as_u64().unwrap_or(0).min(PICK_BYTES);
    let mut data = Vec::new();
    if reader.take(size).read_to_end(&mut data).is_err() || data.len() as u64 != size {
        eprintln!("plugin: the file did not arrive whole");
        return 1;
    }
    if let Err(e) = std::fs::write(target, &data) {
        eprintln!("plugin: {target}: {e}");
        return 1;
    }
    println!("{}", answer["data"]["name"].as_str().unwrap_or(""));
    0
}

/// `plugin ...` inside the sandbox: asks the socket one thing and prints the answer, or says why it was refused
pub fn client(args: &[String]) -> i32 {
    let Some(op) = args.first().map(String::as_str) else {
        eprintln!("usage: plugin state NAME | control NAME ARG | notify TITLE [BODY [ACTION LABEL]] | open URL | clipboard TEXT | pick FILE | event | change ENTRY VALUE");
        return 64;
    };
    let arg = |n: usize| args.get(n).cloned().unwrap_or_default();
    let request = match op {
        "state" => json!({"op": "state", "name": arg(1)}),
        "control" => json!({"op": "control", "name": arg(1), "arg": arg(2)}),
        "notify" => json!({"op": "notify", "title": arg(1), "body": arg(2), "action": arg(3), "label": arg(4)}),
        "open" => json!({"op": "open", "url": arg(1)}),
        "event" => json!({"op": "event"}),
        "change" => json!({"op": "change", "entry": arg(1), "value": arg(2)}),
        "pick" => return pick_into(&arg(1)),
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
        static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("amethystora-broker-{}-{}", std::process::id(), COUNT.fetch_add(1, Ordering::Relaxed)));
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
        // One click, one open: the second asks again
        assert_eq!(b.handle(&json!({"op": "open", "url": "https://example.org/a"}))["ok"], false);
        b.clicked.store(true, Ordering::Relaxed);
        assert_eq!(b.handle(&json!({"op": "open", "url": "file:///etc/shadow"}))["ok"], false);
        assert_eq!(b.handle(&json!({"op": "open", "url": "javascript:1"}))["ok"], false);
        b.clicked.store(true, Ordering::Relaxed);
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
    fn the_client_in_the_sandbox_asks_the_socket_and_says_what_it_heard() {
        let dir = std::env::temp_dir().join(format!("amethystora-client-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("broker.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            for answer in [r#"{"ok":true,"data":{"format":"24h"}}"#, r#"{"ok":false,"why":"it was not allowed to read x"}"#] {
                let (stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(stream.try_clone().unwrap()).read_line(&mut line).unwrap();
                assert!(line.contains(r#""op":"state""#));
                (&stream).write_all(format!("{answer}\n").as_bytes()).unwrap();
            }
        });
        // SAFETY of the environment: no other test reads PLUGIN_BROKER
        unsafe { std::env::set_var("PLUGIN_BROKER", &socket) };
        assert_eq!(client(&["state".into(), "clock".into()]), 0);
        assert_eq!(client(&["state".into(), "x".into()]), 1);
        assert_eq!(client(&["fly".into()]), 64);
        server.join().unwrap();
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
