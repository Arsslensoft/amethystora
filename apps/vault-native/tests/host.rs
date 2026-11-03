//! The host against a stand-in for the Vault's service, speaking the socket's real framing: what each message reaches and what it
//! never does, hostile frames included. The real service is suite 74's (see plans/vault/work/ext.md).

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use amethystora_vault_native::frame::{self, Frame, MAX_MESSAGE};
use amethystora_vault_native::host::{serve, Places};
use amethystora_vault_native::install;
use amethystora_vault_native::site;
use base64::Engine;
use serde_json::{json, Value};

const BINARY: &str = env!("CARGO_BIN_EXE_amethystora-vault-native");
const GITHUB: &str = "https://github.com";

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let path = std::env::temp_dir().join(format!("vault-host-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Folder(path)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The socket's other end: every request it hears, and an open vault with one login and two authenticator entries
#[derive(Default)]
struct Standin {
    heard: Vec<Value>,
    /// What every request is refused as, when the vault is not open
    refuse: Option<&'static str>,
    /// Holds its answers: a person is thinking
    silent: bool,
    hung_up: bool,
}

fn answer(request: &Value) -> Value {
    let encode = |text: &str| base64::engine::general_purpose::STANDARD.encode(text);
    match request["op"].as_str().unwrap_or("") {
        "list" if request["kind"] == "password" => json!({"ok": true, "items": [
            {"id": "gh", "kind": "password", "name": "GitHub", "meta": {"username": "me", "address": "https://github.com/login"}, "approval": "while-open"},
            {"id": "gist", "kind": "password", "name": "Gist", "meta": {"username": "other", "address": "https://gist.github.com"}, "approval": "while-open"},
            {"id": "local", "kind": "password", "name": "Router", "meta": {"username": "admin", "address": "http://192.168.1.1"}, "approval": "while-open"},
            {"id": "evil", "kind": "password", "name": "Evil", "meta": {"username": "x", "address": "https://evil.example"}, "approval": "while-open"},
            {"id": "gone", "kind": "password", "name": "Removed", "meta": {"username": "x", "address": "https://github.com", "deleted_at": 5}, "approval": "while-open"},
            {"id": "bare", "kind": "password", "name": "No address", "meta": null, "approval": "while-open"},
        ]}),
        "list" => json!({"ok": true, "items": [
            {"id": "a-gh", "kind": "authenticator", "name": "GitHub", "meta": {"type": "totp", "issuer": "GitHub", "label": "me"}, "approval": "while-open"},
            {"id": "a-bank", "kind": "authenticator", "name": "My Bank", "meta": {"type": "hotp", "issuer": "My Bank", "label": "me@bank"}, "approval": "while-open"},
        ]}),
        "password-copy" => json!({"ok": true, "secret": encode("hunter2"), "copy": "secret"}),
        "auth-code" => json!({"ok": true, "code": "482913", "seconds_left": 12, "period": 30}),
        "password-generate" => json!({"ok": true, "secret": encode("generated-password"), "bits": 120}),
        "put" => json!({"ok": true, "kind": "password", "id": request["id"], "vault": "v"}),
        _ => json!({"ok": false, "why": "bad-request"}),
    }
}

fn stand_in(socket: &Path, state: Arc<Mutex<Standin>>) {
    let listener = UnixListener::bind(socket).unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let state = state.clone();
            std::thread::spawn(move || {
                let mut writer = stream.try_clone().unwrap();
                let mut lines = BufReader::new(stream);
                loop {
                    let mut line = String::new();
                    if lines.read_line(&mut line).unwrap_or(0) == 0 {
                        state.lock().unwrap().hung_up = true;
                        return;
                    }
                    let request: Value = serde_json::from_str(&line).unwrap();
                    let (reply, silent) = {
                        let mut state = state.lock().unwrap();
                        state.heard.push(request.clone());
                        (state.refuse.map_or_else(|| answer(&request), |why| json!({"ok": false, "why": why})), state.silent)
                    };
                    if !silent {
                        writer.write_all(format!("{reply}\n").as_bytes()).unwrap();
                    }
                }
            });
        }
    });
}

struct Lab {
    folder: Folder,
    state: Arc<Mutex<Standin>>,
    browser: UnixStream,
    host: Option<std::thread::JoinHandle<()>>,
}

impl Lab {
    fn new(name: &str) -> Lab {
        let folder = Folder::new(name);
        let socket = folder.0.join("vault.sock");
        let state = Arc::new(Mutex::new(Standin::default()));
        stand_in(&socket, state.clone());
        std::fs::write(folder.0.join("directory.json"), r#"{"entries":[["GitHub","github.com",0,0]]}"#).unwrap();
        let mut lab = Lab { folder, state, browser: UnixStream::pair().unwrap().0, host: None };
        lab.start(Path::new(site::SUFFIX_LIST).to_path_buf());
        lab
    }

    fn start(&mut self, suffixes: PathBuf) {
        let (browser, mut host_side) = UnixStream::pair().unwrap();
        self.browser = browser;
        let (socket, directory) = (self.folder.0.join("vault.sock"), self.folder.0.join("directory.json"));
        let window = self.folder.0.join("window");
        self.host = Some(std::thread::spawn(move || {
            let places = Places { socket: &socket, suffixes: &suffixes, directory: &directory, window: &window };
            let mut output = host_side.try_clone().unwrap();
            serve(&mut host_side, &mut output, &places).unwrap();
        }));
    }

    fn say(&mut self, message: &Value) -> Value {
        frame::write(&mut self.browser, message.to_string().as_bytes()).unwrap();
        match frame::read(&mut self.browser).unwrap() {
            Frame::Message(bytes) => serde_json::from_slice(&bytes).unwrap(),
            _ => panic!("no answer"),
        }
    }

    fn ask(&mut self, op: &str, origin: &str, more: Value) -> Value {
        let mut message = json!({"v": 1, "op": op, "origin": origin, "top": origin});
        for (key, value) in more.as_object().unwrap() {
            message[key] = value.clone();
        }
        self.say(&message)
    }

    fn ops(&self) -> Vec<String> {
        self.state.lock().unwrap().heard.iter().map(|request| request["op"].as_str().unwrap().to_string()).collect()
    }

    fn secrets_asked(&self) -> usize {
        self.ops().iter().filter(|op| *op == "password-copy" || *op == "auth-code").count()
    }
}

fn has_list() -> bool {
    let there = Path::new(site::SUFFIX_LIST).exists();
    if !there {
        eprintln!("skipped: no {} here", site::SUFFIX_LIST);
    }
    there
}

fn ids(rows: &Value) -> Vec<&str> {
    rows.as_array().unwrap().iter().map(|row| row["id"].as_str().unwrap()).collect()
}

#[test]
fn a_page_is_offered_its_own_site_alone() {
    if !has_list() {
        return;
    }
    let mut lab = Lab::new("state");
    let state = lab.ask("state", GITHUB, json!({}));
    assert_eq!(state["ok"], true);
    assert_eq!(state["vault"], "open");
    assert_eq!(state["site"], "github.com");
    assert_eq!(ids(&state["logins"]), ["gh", "gist"]);
    assert_eq!(state["logins"][0]["exact"], true);
    assert_eq!(state["logins"][0]["user"], "me");
    assert_eq!(state["logins"][1]["exact"], false);
    assert_eq!(ids(&state["codes"]), ["a-gh"]);
    assert_eq!(ids(&state["others"]), ["a-bank"]);
    // Looking asks for no secret
    assert_eq!(lab.secrets_asked(), 0);
    // A look-alike is offered no login and no code: only the choice the person has to make
    let state = lab.ask("state", "https://github.com.evil.example", json!({}));
    assert_eq!(state["site"], "evil.example");
    assert_eq!(ids(&state["logins"]), ["evil"]);
    assert!(ids(&state["codes"]).is_empty());
    assert_eq!(ids(&state["others"]), ["a-gh", "a-bank"]);
    let state = lab.ask("state", "https://github.xyz", json!({}));
    assert!(ids(&state["logins"]).is_empty());
    assert!(ids(&state["codes"]).is_empty());
}

#[test]
fn a_login_is_handed_over_only_where_it_fits() {
    if !has_list() {
        return;
    }
    let mut lab = Lab::new("login");
    let filled = lab.ask("login", GITHUB, json!({"id": "gh"}));
    assert_eq!(filled["ok"], true);
    assert_eq!(filled["user"], "me");
    assert_eq!(filled["password"], "hunter2");
    assert_eq!(lab.secrets_asked(), 1);
    for (origin, id) in [
        ("https://github.com.evil.example", "gh"),
        ("https://evil.example", "gh"),
        ("https://githab.com", "gh"),
        (GITHUB, "evil"),
        // https, and the page is not
        ("http://github.com", "gh"),
        // A removed one, one with no address, one that does not exist
        (GITHUB, "gone"),
        (GITHUB, "bare"),
        (GITHUB, "nothing"),
    ] {
        let answer = lab.ask("login", origin, json!({"id": id}));
        assert_eq!(answer["ok"], false, "{origin} {id}");
        assert_eq!(answer["why"], "refused");
        assert_eq!(answer["problem"], "no-match");
    }
    // None of those reached the service's secrets
    assert_eq!(lab.secrets_asked(), 1);
}

#[test]
fn a_frame_from_another_site_gets_nothing() {
    if !has_list() {
        return;
    }
    let mut lab = Lab::new("frame");
    for (origin, top) in [("https://evil.example", GITHUB), (GITHUB, "https://evil.example"), ("https://me.github.io", "https://you.github.io"), (GITHUB, "http://github.com")] {
        for op in ["state", "login", "code", "generate", "save", "open"] {
            let answer = lab.say(&json!({"v": 1, "op": op, "origin": origin, "top": top, "id": "gh", "user": "u", "password": "p"}));
            assert_eq!(answer["ok"], false, "{op} {origin} {top}");
            assert_eq!(answer["problem"], "cross-site");
        }
    }
    assert!(lab.ops().is_empty(), "the service heard {:?}", lab.ops());
    // A frame of the same site is one site. Saving is the top-level page's alone.
    assert_eq!(lab.say(&json!({"v": 1, "op": "login", "origin": "https://gist.github.com", "top": GITHUB, "id": "gh"}))["ok"], true);
    assert_eq!(lab.say(&json!({"v": 1, "op": "save", "origin": "https://gist.github.com", "top": GITHUB, "user": "u", "password": "p"}))["problem"], "cross-site");
}

#[test]
fn a_message_without_the_browsers_origin_is_refused() {
    let mut lab = Lab::new("origin");
    for message in [
        json!({"v": 1, "op": "state"}),
        json!({"v": 1, "op": "state", "origin": GITHUB}),
        json!({"v": 1, "op": "state", "top": GITHUB}),
        json!({"v": 1, "op": "login", "id": "gh"}),
        json!({"v": 1, "op": "open"}),
        json!({"v": 1, "op": "state", "origin": null, "top": GITHUB}),
        json!({"v": 1, "op": "state", "origin": 5, "top": GITHUB}),
        json!({"v": 1, "op": "state", "origin": "null", "top": GITHUB}),
        json!({"v": 1, "op": "state", "origin": "about:blank", "top": GITHUB}),
        json!({"v": 1, "op": "state", "origin": "moz-extension://abc", "top": "moz-extension://abc"}),
        json!({"v": 1, "op": "state", "origin": "https://github.com/login", "top": GITHUB}),
        json!({"v": 1, "op": "state", "origin": "https://me@github.com", "top": GITHUB}),
        json!({"v": 2, "op": "state", "origin": GITHUB, "top": GITHUB}),
        json!({"op": "state", "origin": GITHUB, "top": GITHUB}),
        json!({"v": 1, "origin": GITHUB, "top": GITHUB}),
        json!({"v": 1, "op": 5, "origin": GITHUB, "top": GITHUB}),
        json!({"v": 1, "op": "get", "kind": "password", "id": "gh", "origin": GITHUB, "top": GITHUB}),
        json!({"v": 1, "op": "forget-everything", "origin": GITHUB, "top": GITHUB}),
    ] {
        let answer = lab.say(&message);
        assert_eq!(answer["ok"], false, "{message}");
        assert_eq!(answer["why"], "bad-request", "{message}");
    }
    assert!(lab.ops().is_empty(), "the service heard {:?}", lab.ops());
}

#[test]
fn a_code_goes_to_its_site_or_to_the_persons_pick() {
    if !has_list() {
        return;
    }
    let mut lab = Lab::new("code");
    assert_eq!(lab.ask("code", GITHUB, json!({"id": "a-gh"}))["code"], "482913");
    // Not this site's: refused until the person has picked it
    let refused = lab.ask("code", GITHUB, json!({"id": "a-bank"}));
    assert_eq!(refused["problem"], "no-match");
    let refused = lab.ask("code", "https://github.xyz", json!({"id": "a-gh"}));
    assert_eq!(refused["problem"], "no-match");
    assert_eq!(lab.secrets_asked(), 1);
    assert_eq!(lab.ask("code", GITHUB, json!({"id": "a-bank", "picked": true}))["code"], "482913");
    // The only fill that moves a counter, and it is HOTP's
    let heard = lab.state.lock().unwrap().heard.clone();
    let codes: Vec<&Value> = heard.iter().filter(|request| request["op"] == "auth-code").collect();
    assert!(codes[0].get("step").is_none());
    assert_eq!(codes[1]["step"], true);
    // A pick that is not `true` is no pick
    assert_eq!(lab.ask("code", GITHUB, json!({"id": "a-bank", "picked": "true"}))["problem"], "no-match");
}

#[test]
fn saving_writes_the_browsers_origin_and_never_overwrites() {
    if !has_list() {
        return;
    }
    let mut lab = Lab::new("save");
    let saved = lab.say(&json!({"v": 1, "op": "save", "origin": "https://new.example", "top": "https://new.example", "user": "me", "password": "pw", "address": "https://evil.example"}));
    assert_eq!(saved["ok"], true);
    let heard = lab.state.lock().unwrap().heard.clone();
    let put = heard.iter().find(|request| request["op"] == "put").unwrap();
    assert_eq!(put["kind"], "password");
    assert_eq!(put["name"], "new.example");
    assert_eq!(put["meta"], json!({"username": "me", "address": "https://new.example"}));
    assert_eq!(put["value"]["address"], "https://new.example");
    assert_eq!(put["value"]["password"], "pw");
    assert_eq!(put["value"]["id"], put["id"]);
    assert_eq!(put["id"].as_str().unwrap().len(), 36);
    assert!(put["meta"].get("password").is_none());
    assert_eq!(lab.ask("save", GITHUB, json!({"user": "me", "password": "pw"}))["problem"], "exists");
    for bad in [json!({"user": "me"}), json!({"user": "me", "password": ""}), json!({"user": "a\nb", "password": "p"}), json!({"user": "me", "password": 5}), json!({"user": "x".repeat(1025), "password": "p"}), json!({"user": "me", "password": "x".repeat(1025)})] {
        assert_eq!(lab.ask("save", "https://new.example", bad.clone())["why"], "bad-request", "{bad}");
    }
}

#[test]
fn http_does_not_hide_an_account_the_vault_has_over_https() {
    if !has_list() {
        return;
    }
    let mut lab = Lab::new("http-exists");
    // GitHub's item is saved for https://github.com: the same user on its http page is the same account
    assert_eq!(lab.ask("save", "http://github.com", json!({"user": "me", "password": "pw"}))["problem"], "exists");
    assert!(!lab.ops().contains(&"put".to_string()));
    assert_eq!(lab.ask("save", "http://github.com", json!({"user": "someone-else", "password": "pw"}))["ok"], true);
}

#[test]
fn a_generated_password_is_kept_before_it_is_handed_over() {
    if !has_list() {
        return;
    }
    let mut lab = Lab::new("generate");
    let made = lab.ask("generate", "https://new.example", json!({"user": "me"}));
    assert_eq!(made["password"], "generated-password");
    // Generated, then put, and only then answered: a page that submits first never leaves an account nobody has the password of
    assert_eq!(lab.ops(), ["list", "password-generate", "put"]);
    let heard = lab.state.lock().unwrap().heard.clone();
    let put = heard.last().unwrap();
    assert_eq!(put["value"]["password"], "generated-password");
    assert_eq!(put["meta"], json!({"username": "me", "address": "https://new.example"}));
    assert_eq!(put["name"], "new.example");
    // Not for an account the vault has (the window changes those), not from a frame, not without saying who
    assert_eq!(lab.ask("generate", GITHUB, json!({"user": "me"}))["problem"], "exists");
    assert_eq!(lab.say(&json!({"v": 1, "op": "generate", "origin": "https://gist.github.com", "top": GITHUB, "user": "new"}))["problem"], "cross-site");
    assert_eq!(lab.ask("generate", "https://new.example", json!({}))["why"], "bad-request");
    assert_eq!(lab.ops().iter().filter(|op| *op == "put").count(), 1);
}

#[test]
fn generating_takes_only_a_sensible_length() {
    if !has_list() {
        return;
    }
    let mut lab = Lab::new("length");
    assert_eq!(lab.ask("generate", "https://new.example", json!({"user": "a"}))["password"], "generated-password");
    assert_eq!(lab.ask("generate", "https://new.example", json!({"user": "b", "length": 32}))["ok"], true);
    let heard = lab.state.lock().unwrap().heard.clone();
    assert_eq!(heard.iter().rev().find(|request| request["op"] == "password-generate").unwrap()["length"], 32);
    for bad in [json!(7), json!(129), json!(-1), json!("20"), json!(1e30)] {
        assert_eq!(lab.ask("generate", "https://new.example", json!({"user": "c", "length": bad}))["why"], "bad-request", "{bad}");
    }
}

#[test]
fn a_form_that_posts_to_another_site_is_not_filled() {
    if !has_list() {
        return;
    }
    let mut lab = Lab::new("action");
    for actions in [json!(["https://evil.example"]), json!(["https://github.com.evil.example"]), json!(["https://github.com", "https://evil.example"]), json!(["http://github.com"]), json!([null]), json!(["null"])] {
        for (op, more) in [("login", json!({"id": "gh"})), ("code", json!({"id": "a-gh"})), ("generate", json!({"user": "new"}))] {
            let mut more = more.clone();
            more["actions"] = actions.clone();
            let answer = lab.ask(op, GITHUB, more);
            assert_eq!(answer["problem"], "other-site-form", "{op} {actions}");
        }
        let mut state_wanted = json!({});
        state_wanted["actions"] = actions.clone();
        let state = lab.ask("state", GITHUB, state_wanted);
        assert_eq!(state["form"], "other-site", "{actions}");
    }
    assert_eq!(lab.secrets_asked(), 0);
    assert!(!lab.ops().contains(&"put".to_string()) && !lab.ops().contains(&"password-generate".to_string()));
    // Its own site, another host of it, and https from an http page are fine; so is no action at all
    for actions in [json!([]), json!(["https://github.com"]), json!(["https://gist.github.com", "https://github.com"])] {
        assert_eq!(lab.ask("login", GITHUB, json!({"id": "gh", "actions": actions}))["ok"], true, "{actions}");
    }
    assert_eq!(lab.ask("state", GITHUB, json!({}))["form"], "ours");
    assert_eq!(lab.ask("login", GITHUB, json!({"id": "gh", "actions": "https://evil.example"}))["why"], "bad-request");
    assert_eq!(lab.ask("login", GITHUB, json!({"id": "gh", "actions": vec!["https://github.com"; 9]}))["why"], "bad-request");
}

#[test]
fn a_closed_vault_says_so_and_a_missing_service_fails() {
    if !has_list() {
        return;
    }
    let mut lab = Lab::new("closed");
    for (why, vault) in [("closed", "closed"), ("not-set-up", "not-set-up")] {
        lab.state.lock().unwrap().refuse = Some(why);
        let state = lab.ask("state", GITHUB, json!({}));
        assert_eq!(state["ok"], true);
        assert_eq!(state["vault"], vault);
        assert!(state.get("logins").is_none());
        assert_eq!(lab.ask("login", GITHUB, json!({"id": "gh"}))["why"], why);
    }
    std::fs::remove_file(lab.folder.0.join("vault.sock")).unwrap();
    assert_eq!(lab.ask("state", GITHUB, json!({}))["why"], "failed");
}

#[test]
fn without_the_suffix_list_nothing_fills() {
    let mut lab = Lab::new("list");
    let missing = lab.folder.0.join("no-list");
    lab.start(missing.clone());
    for op in ["state", "login", "code", "generate", "save"] {
        let answer = lab.ask(op, GITHUB, json!({"id": "gh", "user": "u", "password": "p"}));
        assert_eq!(answer["ok"], false);
        assert_eq!(answer["problem"], "no-suffix-list", "{op}");
    }
    assert!(lab.ops().is_empty());
    // A file that is not one is the same
    std::fs::write(&missing, "com\nco.uk\n").unwrap();
    lab.start(missing);
    assert_eq!(lab.ask("state", GITHUB, json!({}))["problem"], "no-suffix-list");
}

#[test]
fn open_starts_the_window_and_nothing_else() {
    if !has_list() {
        return;
    }
    let mut lab = Lab::new("open");
    let mark = lab.folder.0.join("started");
    let window = lab.folder.0.join("window");
    std::fs::write(&window, format!("#!/usr/bin/bash\nprintf %s \"$#\" > {}\n", mark.display())).unwrap();
    Command::new("chmod").arg("755").arg(&window).status().unwrap();
    // Not from a frame of another site
    assert_eq!(lab.say(&json!({"v": 1, "op": "open", "origin": "https://evil.example", "top": GITHUB}))["problem"], "cross-site");
    assert!(!mark.exists());
    assert_eq!(lab.ask("open", GITHUB, json!({"id": "; rm -rf /"}))["ok"], true);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !mark.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(std::fs::read_to_string(&mark).unwrap(), "0", "started with no arguments");
    assert!(lab.ops().is_empty());
    std::fs::remove_file(&window).unwrap();
    assert_eq!(lab.ask("open", GITHUB, json!({}))["why"], "failed");
}

#[test]
fn the_host_goes_when_the_browser_closes_even_while_a_person_is_asked() {
    if !has_list() {
        return;
    }
    let mut lab = Lab::new("gone");
    // Between messages
    let host = lab.host.take().unwrap();
    drop(std::mem::replace(&mut lab.browser, UnixStream::pair().unwrap().0));
    let started = Instant::now();
    host.join().unwrap();
    assert!(started.elapsed() < Duration::from_secs(2));
    // Waiting for a prompt's answer: the service sees its client hang up, which cancels the prompt
    lab.start(Path::new(site::SUFFIX_LIST).to_path_buf());
    lab.state.lock().unwrap().silent = true;
    frame::write(&mut lab.browser, json!({"v": 1, "op": "state", "origin": GITHUB, "top": GITHUB}).to_string().as_bytes()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while lab.state.lock().unwrap().heard.is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let host = lab.host.take().unwrap();
    drop(std::mem::replace(&mut lab.browser, UnixStream::pair().unwrap().0));
    let started = Instant::now();
    host.join().unwrap();
    assert!(started.elapsed() < Duration::from_secs(2), "took {:?}", started.elapsed());
    let deadline = Instant::now() + Duration::from_secs(5);
    while !lab.state.lock().unwrap().hung_up && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(lab.state.lock().unwrap().hung_up);
}

// --- Hostile frames, against the real program ----------------------------------------------------------------------------------

fn firefox_arguments() -> [String; 2] {
    [format!("/home/a/.mozilla/native-messaging-hosts/{}.json", install::HOST), install::FIREFOX_IDS[0].to_string()]
}

/// Runs the real host with these bytes on its stdin, then closes it. What it wrote and how it ended.
fn feed(bytes: &[u8]) -> (Vec<u8>, i32, Duration) {
    let started = Instant::now();
    let mut child = Command::new(BINARY).args(firefox_arguments()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    // It may stop reading early: a broken pipe here is its right
    let _ = child.stdin.take().unwrap().write_all(bytes);
    let mut out = Vec::new();
    child.stdout.take().unwrap().read_to_end(&mut out).unwrap();
    let status = child.wait().unwrap();
    (out, status.code().unwrap_or(-1), started.elapsed())
}

fn frames(mut bytes: &[u8]) -> Vec<Value> {
    let mut all = Vec::new();
    while !bytes.is_empty() {
        match frame::read(&mut bytes).unwrap() {
            Frame::Message(message) => all.push(serde_json::from_slice(&message).unwrap()),
            _ => panic!("the host wrote something that is not a frame"),
        }
    }
    all
}

fn framed(body: &[u8]) -> Vec<u8> {
    let mut bytes = (body.len() as u32).to_ne_bytes().to_vec();
    bytes.extend_from_slice(body);
    bytes
}

#[test]
fn nothing_said_means_a_clean_end() {
    let (out, code, took) = feed(b"");
    assert!(out.is_empty());
    assert_eq!(code, 0);
    assert!(took < Duration::from_secs(2));
}

#[test]
fn a_length_past_the_limit_stops_it_before_anything_is_allocated() {
    for length in [u32::MAX, 0x8000_0000, MAX_MESSAGE as u32 + 1] {
        let (out, code, took) = feed(&length.to_ne_bytes());
        assert_eq!(frames(&out)[0]["why"], "too-large", "{length}");
        assert_eq!(code, 0);
        assert!(took < Duration::from_secs(2));
    }
}

#[test]
fn a_frame_that_stops_halfway_ends_it() {
    for bytes in [&[1u8][..], &[1, 0, 0][..], &framed(b"{\"v\":1}")[..5]] {
        let (out, code, _) = feed(bytes);
        assert_eq!(frames(&out)[0]["why"], "too-large");
        assert_eq!(code, 0);
    }
}

#[test]
fn what_is_not_a_request_is_refused_and_it_goes_on() {
    let mut bytes = Vec::new();
    let bad: Vec<Vec<u8>> = vec![
        Vec::new(),
        b" ".to_vec(),
        b"null".to_vec(),
        b"5".to_vec(),
        b"[]".to_vec(),
        b"\"state\"".to_vec(),
        b"{}".to_vec(),
        b"{".to_vec(),
        vec![0xff, 0xfe, 0x00, 0x80],
        "[".repeat(200_000).into_bytes(),
        format!("{{\"v\":1,\"op\":\"state\",\"origin\":\"{}\"}}", "a".repeat(500_000)).into_bytes(),
    ];
    for body in &bad {
        bytes.extend(framed(body));
    }
    let (out, code, _) = feed(&bytes);
    let answers = frames(&out);
    assert_eq!(answers.len(), bad.len());
    assert!(answers.iter().all(|answer| answer["ok"] == false && answer["why"] == "bad-request"), "{answers:?}");
    assert_eq!(code, 0);
}

#[test]
fn the_largest_frame_is_read_whole() {
    let mut body = b"{\"v\":1,\"pad\":\"".to_vec();
    body.resize(MAX_MESSAGE - 2, b'x');
    body.extend_from_slice(b"\"}");
    let (out, code, _) = feed(&framed(&body));
    assert_eq!(frames(&out)[0]["why"], "bad-request");
    assert_eq!(code, 0);
}

#[test]
fn it_refuses_any_extension_but_ours_before_reading_a_thing() {
    let manifest = firefox_arguments()[0].clone();
    for arguments in [
        vec![],
        vec![manifest.clone()],
        vec![manifest.clone(), "other@example.com".to_string()],
        vec!["/tmp/other.json".to_string(), install::FIREFOX_IDS[0].to_string()],
        vec!["chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/".to_string()],
    ] {
        let mut child = Command::new(BINARY).args(&arguments).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        // Its stdin stays open: it does not wait for it
        let code = child.wait().unwrap().code();
        assert_eq!(code, Some(64), "{arguments:?}");
        let mut out = Vec::new();
        child.stdout.take().unwrap().read_to_end(&mut out).unwrap();
        assert!(out.is_empty());
    }
}

#[test]
fn it_sleeps_on_its_pipe_with_no_wakeups() {
    let mut child = Command::new(BINARY).args(firefox_arguments()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let switches = |pid: u32| -> Option<u64> {
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        let count = |name: &str| status.lines().find(|line| line.starts_with(name)).and_then(|line| line.split_whitespace().last()?.parse::<u64>().ok());
        Some(count("voluntary_ctxt_switches")? + count("nonvoluntary_ctxt_switches")?)
    };
    std::thread::sleep(Duration::from_millis(300));
    let before = switches(child.id()).unwrap();
    std::thread::sleep(Duration::from_secs(3));
    let after = switches(child.id()).unwrap();
    assert_eq!(before, after, "woke up {} times in 3 s", after - before);
    drop(child.stdin.take());
    assert_eq!(child.wait().unwrap().code(), Some(0));
}

#[test]
fn the_real_program_writes_and_takes_away_the_manifests() {
    let home = Folder::new("program");
    let run = |arguments: &[&str]| Command::new(BINARY).arg("browser").args(arguments).env("HOME", &home.0).output().unwrap();
    assert_eq!(String::from_utf8(run(&["status"]).stdout).unwrap(), "off\n");
    assert!(run(&["on"]).status.success());
    assert_eq!(String::from_utf8(run(&["status"]).stdout).unwrap(), "on\n");
    let path = home.0.join(".mozilla/native-messaging-hosts").join(format!("{}.json", install::HOST));
    assert_eq!(std::fs::read(&path).unwrap(), run(&["manifest", "firefox"]).stdout);
    assert!(run(&["off"]).status.success());
    assert!(!home.0.join(".mozilla").exists());
    assert_eq!(run(&["sideways"]).status.code(), Some(64));
    assert_eq!(run(&["on", "extra"]).status.code(), Some(64));
    assert_eq!(run(&["manifest", "safari"]).status.code(), Some(64));
}

/// `psl`, libpsl's own tool, as the check on the list's reading, for every rule on the machine and hosts below it. It is told to load the
/// very file the host reads: left to itself it takes the newer of that and the list built into libpsl, which on Fedora 45 had dropped
/// the Italian regional names (aquila.it...) that the file still holds. Skipped where
/// Fedora's `psl` and `publicsuffix-list` are not installed.
#[test]
fn the_list_reads_as_libpsl_reads_it() {
    let (Ok(text), true) = (std::fs::read_to_string("/usr/share/publicsuffix/public_suffix_list.dat"), Path::new("/usr/bin/psl").exists()) else {
        assert!(std::env::var_os("VAULT_VERIFIERS_REQUIRED").is_none(), "psl and publicsuffix-list are not installed");
        eprintln!("skipped: no public_suffix_list.dat or psl here");
        return;
    };
    let list = site::Suffixes::load(Path::new(site::SUFFIX_LIST)).unwrap();
    let mut hosts = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with("//")) {
        let rule = line.trim_start_matches('!').trim_start_matches("*.");
        let Some(rule) = site::ascii_host(rule) else { continue };
        for prefix in ["", "a.", "b.a.", "c.b.a."] {
            hosts.push(format!("{prefix}{rule}"));
        }
    }
    let mut child = Command::new("psl").args(["--print-reg-domain", "-b", "--load-psl-file", site::SUFFIX_LIST]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut input = child.stdin.take().unwrap();
    let writing = hosts.join("\n");
    std::thread::spawn(move || {
        input.write_all(writing.as_bytes()).unwrap();
    });
    let out = child.wait_with_output().unwrap().stdout;
    let theirs: Vec<&str> = std::str::from_utf8(&out).unwrap().lines().collect();
    assert_eq!(theirs.len(), hosts.len());
    let mut differ = 0;
    for (host, their) in hosts.iter().zip(theirs) {
        let their = if their == "(null)" { None } else { Some(their) };
        if list.registrable(host) != their {
            differ += 1;
            eprintln!("{host}: ours {:?}, libpsl {their:?}", list.registrable(host));
        }
    }
    assert_eq!(differ, 0, "of {}", hosts.len());
}

#[test]
fn the_store_package_carries_no_test_key_and_no_site() {
    // What goes to the stores is the extension's folder without tests/ and tools/: its manifest must not hold the test key (the Chrome
    // Web Store refuses a manifest with a key in it) nor any site's permission (the person grants those, one site at a time)
    let manifest = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../vault-extension/manifest.json")).unwrap();
    let manifest: Value = serde_json::from_str(&manifest).unwrap();
    assert!(manifest.get("key").is_none());
    assert!(manifest.get("host_permissions").is_none());
    assert!(manifest.get("externally_connectable").is_none() && manifest.get("web_accessible_resources").is_none());
    assert_eq!(manifest["permissions"], json!(["activeTab", "nativeMessaging", "scripting"]));
}
