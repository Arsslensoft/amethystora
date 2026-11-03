//! Sync through the whole path of the service: a person's requests on the Vault's socket, the service, the network service's
//! socket, `amethystora-vault-net`'s own code, and a server of the test's own (the same one the protocol tests use) over a
//! real TCP connection. polkit and logind are the stand-ins of every service test.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use amethystora_vault_core::items::{Account, BackupCode, BackupService, OtpKind, Password, Secret};
use amethystora_vault_core::slots::fake::FakeChip;
use amethystora_vault_core::sync::api::{Http, Request};
use amethystora_vault_core::sync::cipher::{self, Content, LOGIN};
use amethystora_vault_core::sync::crypto::Kdf;
use amethystora_vault_core::sync::fake::{Fake, EMAIL, PASSWORD as ACCOUNT_PASSWORD, USER_ID};
use amethystora_vault_net::http::Session;
use amethystora_vault_service::outside::say;
use amethystora_vault_service::server::Server;
use amethystora_vault_service::service::{Config, Service};
use common::{b64, fake, Folder, Handle};

const VAULT_PASSWORD: &str = "correct horse battery";

// --- A server of our own on a real port ------------------------------------------------------------------------------

type Shared = Arc<Mutex<Fake>>;

fn method_of(name: &str) -> Option<&'static str> {
    ["GET", "POST", "PUT", "DELETE"].into_iter().find(|method| *method == name)
}

fn handle(stream: TcpStream, fake: Shared) {
    let Ok(clone) = stream.try_clone() else { return };
    let mut reader = BufReader::new(clone);
    let mut writer = stream;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let mut words = line.split_whitespace();
        let (Some(method), Some(path)) = (words.next().and_then(method_of), words.next()) else { return };
        let path = path.to_string();
        let (mut length, mut headers) = (0usize, Vec::new());
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header).unwrap_or(0) == 0 {
                return;
            }
            let header = header.trim_end().to_string();
            if header.is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    length = value.trim().parse().unwrap_or(0);
                }
                headers.push((&*Box::leak(name.trim().to_string().into_boxed_str()), value.trim().to_string()));
            }
        }
        let mut body = vec![0u8; length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        let response = {
            let mut fake = fake.lock().unwrap();
            let url = format!("{}{}", fake.base, path);
            fake.call(Request { method, url, headers, body: zeroize::Zeroizing::new(body) })
        };
        let Ok(response) = response else { return };
        let head = format!("HTTP/1.1 {} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", response.status, response.body.len());
        if writer.write_all(head.as_bytes()).and_then(|()| writer.write_all(&response.body)).is_err() {
            return;
        }
    }
}

fn serve_fake(fake: Shared) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    fake.lock().unwrap().base = format!("http://127.0.0.1:{port}");
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let fake = fake.clone();
            std::thread::spawn(move || handle(stream, fake));
        }
    });
    port
}

/// The network service's own code on a socket of its own, as systemd would hand it
fn serve_net(path: &PathBuf) {
    let listener = UnixListener::bind(path).unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut writer = stream.try_clone().unwrap();
                let mut reader = BufReader::new(stream);
                amethystora_vault_net::serve(&mut reader, &mut writer, &|origins, ca, pin| Session::new(origins, ca, pin));
            });
        }
    });
}

// --- The service ----------------------------------------------------------------------------------------------------------

struct Rig {
    /// Removes its files when the test ends
    #[allow(dead_code)]
    folder: Folder,
    socket: PathBuf,
    outside: Handle,
    fake: Shared,
    port: u16,
    stop: OwnedFd,
    thread: Option<std::thread::JoinHandle<()>>,
}

struct Settings {
    net: bool,
    debounce: Duration,
    interval: Duration,
    metered: Option<PathBuf>,
    /// The account's key settings, if not the default
    kdf: Option<Kdf>,
    run_limit: Duration,
    /// The program that makes Argon2 keys, if not the service's own
    child: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { net: true, debounce: Duration::from_millis(150), interval: Duration::from_secs(3600), metered: None, kdf: None, run_limit: Duration::from_secs(600), child: None }
    }
}

fn rig(name: &str, settings: Settings) -> Rig {
    let folder = Folder::new(name);
    let socket = folder.0.join("vault.sock");
    let net = folder.0.join("vault-net.sock");
    if settings.net {
        serve_net(&net);
    }
    let listener = UnixListener::bind(&socket).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o666)).unwrap();
    std::fs::set_permissions(&folder.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    let state = folder.0.join("state");
    std::fs::create_dir_all(&state).unwrap();
    let fake: Shared = Arc::new(Mutex::new(settings.kdf.map_or_else(Fake::default, Fake::new)));
    let port = serve_fake(fake.clone());
    let mut pipe = [0; 2];
    // SAFETY: pipe2 writes two descriptors, which are then owned
    let (stop_read, stop_write) = unsafe {
        assert_eq!(libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC), 0);
        (OwnedFd::from_raw_fd(pipe[0]), OwnedFd::from_raw_fd(pipe[1]))
    };
    let (outside, handle) = fake_outside();
    let metered = settings.metered.clone();
    let child = settings.child.clone().unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_amethystora-vault-service")));
    let thread = std::thread::spawn(move || {
        let mut service = Service::new(Config { state, log_n: 10, worker: PathBuf::from(env!("CARGO_BIN_EXE_amethystora-vault-work")) }, Arc::new(FakeChip::new()), Box::new(outside));
        service.sync.net = net;
        service.sync.debounce = settings.debounce;
        service.sync.run_limit = settings.run_limit;
        service.sync.kdf_child = child;
        service.sync.interval = settings.interval;
        if let Some(helper) = metered {
            service.sync.metered = helper;
        }
        let server = Server::new(listener, Some(stop_read.as_raw_fd()), service, Duration::from_secs(600)).unwrap();
        server.run();
        drop(stop_read);
    });
    Rig { folder, socket, outside: handle, fake, port, stop: stop_write, thread: Some(thread) }
}

fn fake_outside() -> (common::FakeOutside, Handle) {
    fake()
}

impl Drop for Rig {
    fn drop(&mut self) {
        // SAFETY: one byte, from a live buffer
        unsafe { libc::write(self.stop.as_raw_fd(), [1u8].as_ptr().cast(), 1) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Rig {
    fn ask(&self, mut request: Value) -> Value {
        request["v"] = json!(1);
        amethystora_vault_client::exchange(&self.socket, &request, Duration::from_secs(60)).unwrap()
    }

    fn address(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn set_up(&self) {
        let answer = self.ask(json!({"op": "setup", "method": "password", "name": "Personal", "secret": b64(VAULT_PASSWORD)}));
        assert_eq!(answer["ok"], json!(true), "{answer}");
        let recovery = answer["recovery"].as_str().unwrap().to_string();
        assert_eq!(self.ask(json!({"op": "confirm", "recovery": b64(&recovery)}))["ok"], json!(true));
    }

    /// The kinds only the service writes (authenticator accounts, backup codes, keys) cannot be put over the socket: the test lays one
    /// in the closed vault's folder, as the plugs' writes would, and opens it again
    fn inject(&self, kind: &str, id: &str, name: &str, meta: &Value, value: &Value) {
        use amethystora_vault_core::protocol::Approval;
        use amethystora_vault_core::slots::{self, Table};
        use amethystora_vault_core::store::Store;
        use amethystora_vault_core::vaults::Account;
        assert_eq!(self.ask(json!({"op": "lock"}))["ok"], json!(true));
        let state = self.folder.0.join("state");
        // SAFETY: getuid cannot fail
        let account = Account::load(&state, unsafe { libc::getuid() }).unwrap().unwrap();
        let under = slots::derive(VAULT_PASSWORD.as_bytes(), &account.file.kdf).unwrap();
        let found = slots::search(&under, &account.records(Table::Password), Table::Password);
        let mut store = Store::open(&account.folder_of(&found[0].vault), &found[0].vault, found[0].key.copy()).unwrap();
        store.put(kind, id, name, meta, Approval::WhileOpen, value).unwrap();
        let answer = self.ask(json!({"op": "unlock", "method": "password", "secret": b64(VAULT_PASSWORD)}));
        assert_eq!(answer["ok"], json!(true), "{answer}");
    }

    fn put(&self, kind: &str, id: &str, name: &str, meta: Value, value: Value) {
        if amethystora_vault_core::protocol::service_only(kind) {
            return self.inject(kind, id, name, &meta, &value);
        }
        let answer = self.ask(json!({"op": "put", "kind": kind, "id": id, "name": name, "meta": meta, "value": value}));
        assert_eq!(answer["ok"], json!(true), "{answer}");
    }

    fn password(&self, id: &str, title: &str, user: &str, secret: &str, address: &str) {
        let value = serde_json::to_value(Password { id: id.into(), title: title.into(), username: user.into(), password: Secret::new(secret), address: address.into(), ..Password::default() }).unwrap();
        self.put("password", id, title, json!({"username": user, "address": address}), value);
    }

    fn value_of(&self, kind: &str, id: &str) -> Value {
        let answer = self.ask(json!({"op": "get", "kind": kind, "id": id}));
        assert_eq!(answer["ok"], json!(true), "{answer}");
        answer["value"].clone()
    }

    fn items(&self, kind: &str) -> Vec<Value> {
        self.ask(json!({"op": "list", "kind": kind}))["items"].as_array().cloned().unwrap_or_default()
    }

    /// The email and the master password; the person's yes is asked on the connection this goes over
    fn connect(&self) -> Value {
        self.talk().ask(self.connection_request())
    }

    fn connection_request(&self) -> Value {
        json!({"op": "sync-connect", "address": self.address(), "email": EMAIL, "secret": b64(ACCOUNT_PASSWORD)})
    }

    /// A connection that stays: what a person says yes to is said yes to for it
    fn talk(&self) -> Talk {
        let stream = UnixStream::connect(&self.socket).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(120))).unwrap();
        Talk { reader: BufReader::new(stream.try_clone().unwrap()), stream }
    }

    fn status(&self) -> Value {
        self.ask(json!({"op": "sync-status"}))
    }

    /// Until the status says a run has finished well
    fn synced(&self) -> Value {
        until(|| {
            let status = self.status();
            (status["connected"] == json!(true) && status["last_success_ms"].is_number() && status["running"] != json!(true)).then_some(status)
        })
    }

    fn server_ciphers(&self) -> usize {
        self.fake.lock().unwrap().ciphers.len()
    }
}

struct Talk {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
}

impl Talk {
    fn ask(&mut self, mut request: Value) -> Value {
        request["v"] = json!(1);
        self.stream.write_all(format!("{request}\n").as_bytes()).unwrap();
        let mut answer = String::new();
        self.reader.read_line(&mut answer).unwrap();
        serde_json::from_str(&answer).unwrap()
    }
}

fn until<T>(mut check: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(found) = check() {
            return found;
        }
        assert!(Instant::now() < end, "it never happened");
        std::thread::sleep(Duration::from_millis(40));
    }
}

// --- Tests ----------------------------------------------------------------------------------------------------------------

#[test]
fn a_person_connects_and_the_vault_and_the_account_meet() {
    let rig = rig("meet", Settings::default());
    rig.set_up();
    rig.password("p1", "Example", "person", "hunter2 ✓", "https://example.test/login");
    let codes = BackupService { id: "c1".into(), service_name: "Example".into(), codes: vec![BackupCode::new("1111-2222")], ..BackupService::default() };
    rig.put("backup-codes", "c1", "Example", Value::Null, serde_json::to_value(&codes).unwrap());
    let account = Account { id: "a1".into(), kind: Some(OtpKind::Totp), issuer: "Elsewhere".into(), label: "me".into(), secret: Secret::new("JBSWY3DPEHPK3PXP"), ..Account::default() };
    rig.put("authenticator", "a1", "Elsewhere", Value::Null, serde_json::to_value(&account).unwrap());

    // No program has the service reach out to a server of its choosing: a probe wants a connection that began one
    assert_eq!(rig.ask(json!({"op": "sync-probe", "address": rig.address()}))["ok"], json!(false));
    assert_eq!(rig.status()["connected"], json!(false));

    let answer = rig.connect();
    assert_eq!(answer["ok"], json!(true), "{answer}");
    assert_eq!(answer["connected"], json!(true));
    // The person was asked, with the server's address and the account in the sentence
    let asked = rig.outside.said.lock().unwrap().asked.clone();
    assert!(asked.iter().any(|ask| ask.message == say::SYNC_CONNECT && ask.item == format!("{} ({})", rig.address(), EMAIL)), "{asked:?}");

    let status = rig.synced();
    assert_eq!(status["email"], json!(EMAIL));
    assert_eq!(status["linked"], json!(3));
    assert_eq!(rig.server_ciphers(), 3);
    let (password, codes) = {
        let fake = rig.fake.lock().unwrap();
        let opened: Vec<Content> = fake.ciphers.iter().map(|raw| cipher::open(raw, &fake.user).unwrap().content).collect();
        (opened.iter().find(|c| c.password.is_some()).cloned().unwrap(), opened.iter().find(|c| c.kind == 2).cloned().unwrap())
    };
    assert_eq!((password.password.as_deref(), password.uri.as_deref()), (Some("hunter2 ✓"), Some("https://example.test/login")));
    assert_eq!(codes.notes.as_deref(), Some("[ ] 1111-2222"));

    let links = rig.items("sync-links");
    assert!(links.len() <= 3, "the links are in at most sixteen items, here fewer than the ciphers");
    assert!(!links.is_empty() && links.iter().all(|link| link["name"] == "links" && link["meta"].is_null()));
    let listed = rig.items("sync-account");
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].to_string().contains(EMAIL) && !listed[0].to_string().contains(&rig.address()));
    // And no program of the account gets them: not by get, not by put, not by approval
    assert_eq!(rig.ask(json!({"op": "get", "kind": "sync-account", "id": "account"}))["ok"], json!(false));
    assert_eq!(rig.ask(json!({"op": "put", "kind": "sync-account", "id": "account", "name": "x", "value": {}}))["ok"], json!(false));
    assert_eq!(rig.ask(json!({"op": "set-approval", "kind": "sync-links", "id": "x", "approval": "never"}))["ok"], json!(false));

    // Another client writes: a Login with a totp; sync now takes it
    {
        let mut fake = rig.fake.lock().unwrap();
        let content = Content { totp: Some("otpauth://totp/Other:you?secret=JBSWY3DPEHPK3PXP&issuer=Other".into()), name: "Other".into(), username: Some("you".into()), password: Some("s3cret".into()), kind: LOGIN, ..Content::default() };
        let body = cipher::create_body(&content, USER_ID, &fake.user).unwrap();
        fake.store(body);
    }
    let answer = rig.ask(json!({"op": "sync-now"}));
    assert_eq!(answer["ok"], json!(true), "{answer}");
    let titles: Vec<String> = rig.items("password").iter().map(|item| item["name"].as_str().unwrap().to_string()).collect();
    assert!(titles.contains(&"Other".to_string()) && titles.contains(&"Example".to_string()), "{titles:?}");
    let other = rig.items("password").into_iter().find(|item| item["name"] == "Other").unwrap();
    assert_eq!(other["meta"]["username"], "you", "the extension matches on username and address");
    assert_eq!(rig.items("authenticator").len(), 2);

    // A change here goes after it, a little later and by itself
    rig.password("p1", "Example", "person", "changed here", "https://example.test/login");
    until(|| {
        let fake = rig.fake.lock().unwrap();
        fake.ciphers.iter().filter_map(|raw| cipher::open(raw, &fake.user).ok()).any(|opened| opened.content.password.as_deref() == Some("changed here")).then_some(())
    });

    // Every request that left carried the headers the protocol asks for, and no secret in the clear
    {
        let fake = rig.fake.lock().unwrap();
        let user_hex = amethystora_vault_core::seal::hex(fake.user.bytes());
        for seen in &fake.seen {
            let everything = format!("{} {} {:?} {}", seen.method, seen.url, seen.headers, seen.body);
            for secret in [ACCOUNT_PASSWORD, VAULT_PASSWORD, "hunter2", "changed here", "s3cret", "1111-2222", "JBSWY3DPEHPK3PXP", &user_hex] {
                assert!(!everything.contains(secret), "{secret:?} left in the clear: {} {}", seen.method, seen.url);
            }
            assert!(seen.method != "DELETE" && !seen.url.ends_with("/delete") || seen.method == "PUT", "a permanent delete");
        }
        assert!(fake.seen.iter().any(|seen| seen.headers.iter().any(|(name, value)| name.eq_ignore_ascii_case("bitwarden-client-version") && value == amethystora_vault_core::sync::CLIENT_VERSION)));
    }

    // Off: the person is asked, the links and the connection go, the items stay, and the server keeps what it has
    let before = rig.outside.asked();
    assert_eq!(rig.ask(json!({"op": "sync-off"}))["ok"], json!(true));
    assert_eq!(rig.outside.asked(), before + 1);
    assert_eq!(rig.status()["connected"], json!(false));
    assert!(rig.items("sync-links").is_empty());
    assert_eq!(rig.items("password").len(), 2);
    assert_eq!(rig.server_ciphers(), 4);
    // And nothing runs any more
    let seen = rig.fake.lock().unwrap().seen.len();
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(rig.fake.lock().unwrap().seen.len(), seen);
}

#[test]
fn a_refused_approval_connects_nothing() {
    let rig = rig("refused", Settings::default());
    rig.set_up();
    rig.outside.said.lock().unwrap().refuse = true;
    let answer = rig.connect();
    assert_eq!(answer["ok"], json!(false));
    assert!(rig.fake.lock().unwrap().seen.is_empty(), "the server heard nothing");
}

#[test]
fn a_closed_vault_syncs_nothing_and_says_so() {
    let rig = rig("closed", Settings::default());
    rig.set_up();
    rig.password("p1", "Example", "u", "p", "");
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    assert_eq!(rig.ask(json!({"op": "lock"}))["ok"], json!(true));
    for op in ["sync-status", "sync-now", "sync-off", "sync-probe", "sync-connect"] {
        let answer = rig.ask(json!({"op": op, "address": rig.address()}));
        assert_eq!(answer["why"], json!("closed"), "{op}: {answer}");
    }
    // Locked, nothing is asked of the server by itself, whatever is due
    let seen = rig.fake.lock().unwrap().seen.len();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(rig.fake.lock().unwrap().seen.len(), seen);
    // Opened again, it looks at the server by itself
    assert_eq!(rig.ask(json!({"op": "unlock", "method": "password", "secret": b64(VAULT_PASSWORD)}))["ok"], json!(true));
    until(|| (rig.fake.lock().unwrap().seen.len() > seen).then_some(()));
    rig.synced();
}

#[test]
fn a_hidden_vault_is_never_offered_and_never_synced() {
    let rig = rig("hidden", Settings::default());
    rig.set_up();
    rig.password("p1", "First vault", "u", "p", "");
    // A hidden vault, opened by its password alone: while only it is open, there is no sync to ask for
    let recovery = rig.ask(json!({"op": "new", "name": "Secret", "hidden": true, "secret": b64("another password")}));
    assert_eq!(recovery["ok"], json!(true));
    let typed = recovery["recovery"].as_str().unwrap().to_string();
    assert_eq!(rig.ask(json!({"op": "confirm", "recovery": b64(&typed)}))["ok"], json!(true));
    assert_eq!(rig.ask(json!({"op": "lock"}))["ok"], json!(true));
    assert_eq!(rig.ask(json!({"op": "unlock", "method": "password", "secret": b64("another password")}))["ok"], json!(true));
    for op in ["sync-status", "sync-now", "sync-probe", "sync-connect"] {
        let answer = rig.ask(json!({"op": op, "address": rig.address()}));
        assert_eq!(answer["why"], json!("closed"), "{op}: {answer}");
    }
    // With both open, an item of the hidden vault does not go
    assert_eq!(rig.ask(json!({"op": "unlock", "method": "password", "secret": b64(VAULT_PASSWORD)}))["ok"], json!(true));
    let hidden = rig.ask(json!({"op": "status"}))["vaults"].as_array().unwrap().iter().find(|vault| vault["first"] != json!(true) && vault["open"] == json!(true)).cloned().unwrap();
    let value = serde_json::to_value(Password { id: "h1".into(), title: "Hidden one".into(), password: Secret::new("never leaves"), ..Password::default() }).unwrap();
    let answer = rig.ask(json!({"op": "put", "kind": "password", "id": "h1", "name": "Hidden one", "value": value, "vault": hidden["id"]}));
    assert_eq!(answer["ok"], json!(true), "{answer}");
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    assert_eq!(rig.server_ciphers(), 1);
    let fake = rig.fake.lock().unwrap();
    assert!(fake.seen.iter().all(|seen| !seen.body.contains("Hidden") && !seen.body.contains("never leaves")));
    let name = cipher::open(&fake.ciphers[0], &fake.user).unwrap().content.name;
    assert_eq!(name, "First vault");
}

#[test]
fn a_server_that_is_down_stops_nothing() {
    // The network service is not there at all
    let rig = rig("down", Settings { net: false, ..Settings::default() });
    rig.set_up();
    rig.password("p1", "Example", "u", "p", "");
    let answer = rig.connect();
    assert_eq!(answer["ok"], json!(false));
    assert_eq!(answer["problem"], json!("unreachable"));
    assert_eq!(rig.status()["connected"], json!(false));
    // The vault works as it did
    assert_eq!(rig.value_of("password", "p1")["password"], "p");
    rig.password("p2", "Another", "u", "q", "");
    assert_eq!(rig.items("password").len(), 2);
}

#[test]
fn a_run_never_holds_up_the_service_and_never_writes_into_a_vault_that_closed() {
    let rig = rig("slow", Settings::default());
    rig.set_up();
    rig.password("p1", "Example", "u", "p", "");
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    // Another client adds an item, and the server is slow to say so
    {
        let mut fake = rig.fake.lock().unwrap();
        let body = cipher::create_body(&Content { kind: LOGIN, name: "Arrives late".into(), password: Some("x".into()), ..Content::default() }, USER_ID, &fake.user).unwrap();
        fake.store(body);
        fake.hooks.insert("sync", Box::new(|_| std::thread::sleep(Duration::from_millis(800))));
    }
    let socket = rig.socket.clone();
    let runner = std::thread::spawn(move || {
        let request = json!({"v": 1, "op": "sync-now"});
        amethystora_vault_client::exchange(&socket, &request, Duration::from_secs(60)).unwrap()
    });
    std::thread::sleep(Duration::from_millis(300));
    // While it waits, the service answers at once
    let started = Instant::now();
    let status = rig.status();
    assert!(started.elapsed() < Duration::from_millis(400), "{:?}", started.elapsed());
    assert_eq!(status["running"], json!(true));
    // Locked meanwhile: what the run found is not written anywhere
    assert_eq!(rig.ask(json!({"op": "lock"}))["ok"], json!(true));
    let answer = runner.join().unwrap();
    assert_eq!(answer["why"], json!("closed"), "{answer}");
    assert_eq!(rig.ask(json!({"op": "unlock", "method": "password", "secret": b64(VAULT_PASSWORD)}))["ok"], json!(true));
    // Opened again, its own run is slow too: the item the first run found is not in the vault, nor does it come from that run
    assert_eq!(rig.items("password").len(), 1);
    until(|| (rig.items("password").len() == 2).then_some(()));
}

#[test]
fn a_metered_connection_makes_a_timers_run_wait_and_never_a_persons() {
    let folder = Folder::new("metered-stub");
    let stub = folder.0.join("metered");
    let yes = folder.0.join("yes");
    std::fs::write(&stub, format!("#!/bin/sh\n[ \"$1\" = metered ] && [ -e {} ]\n", yes.display())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(&yes, "").unwrap();
    let rig = rig("metered", Settings { metered: Some(stub), ..Settings::default() });
    rig.set_up();
    rig.password("p1", "Example", "u", "p", "");
    // Connecting is a person's act, and goes through; the first run it starts is the person's too
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    let seen = rig.fake.lock().unwrap().seen.len();
    // A change on a metered connection: the run its timer starts waits, and says since when
    rig.password("p2", "Second", "u", "q", "");
    until(|| rig.status()["waiting_since_ms"].is_number().then_some(()));
    assert_eq!(rig.server_ciphers(), 1);
    assert_eq!(rig.fake.lock().unwrap().seen.len(), seen, "the server heard nothing");
    // A person asking is never made to wait
    // Asked again while the run of the first choice is still at it
    until(|| (rig.ask(json!({"op": "sync-now"}))["ok"] == json!(true)).then_some(()));
    assert_eq!(rig.server_ciphers(), 2);
    // Back on a connection that does not count data, the timer's run goes through and the wait is forgotten
    std::fs::remove_file(&yes).unwrap();
    rig.password("p3", "Third", "u", "r", "");
    until(|| (rig.server_ciphers() == 3).then_some(()));
    until(|| (rig.status()["waiting_since_ms"].is_null()).then_some(()));
}

#[test]
fn an_account_the_vault_cannot_read_is_refused_and_nothing_is_written() {
    let rig = rig("newer", Settings::default());
    rig.set_up();
    rig.password("p1", "Example", "u", "p", "");
    // Bitwarden's newer encryption, shown at login
    rig.fake.lock().unwrap().hooks.insert("token", Box::new(|answer| answer["AccountKeys"]["securityState"] = json!({"securityVersion": 2})));
    let answer = rig.connect();
    assert_eq!(answer["ok"], json!(false));
    assert_eq!(answer["problem"], json!("newer-encryption"));
    assert_eq!(rig.status()["connected"], json!(false));
    assert!(rig.fake.lock().unwrap().ciphers.is_empty());
    // A server that lowers its settings after the first login is refused at the next, and nothing is sent
    rig.fake.lock().unwrap().hooks.clear();
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    rig.fake.lock().unwrap().hooks.insert("sync", Box::new(|sync| sync["profile"]["email"] = json!("attacker@evil.test")));
    let answer = rig.ask(json!({"op": "sync-now"}));
    assert_eq!(answer["ok"], json!(false), "{answer}");
    assert_eq!(answer["problem"]["name"], json!("identity-changed"));
    // It waits for a person now: no run starts by itself
    assert_eq!(rig.status()["problem"]["name"], json!("identity-changed"));
}

#[test]
fn a_key_is_sent_only_sealed_and_only_when_chosen() {
    let rig = rig("sealed", Settings::default());
    rig.set_up();
    rig.put("key", "k1", "A key", json!({"key_kind": "ssh", "exportable": true}), json!({"private": "PRIVATE-KEY-MATERIAL"}));
    rig.put("key", "k2", "Locked in", json!({"key_kind": "ssh", "exportable": false}), json!({"private": "NEVER"}));
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    assert_eq!(rig.server_ciphers(), 0, "off until chosen");
    // Chosen, after the person says so
    let before = rig.outside.asked();
    assert_eq!(rig.ask(json!({"op": "sync-choose", "kind": "key", "id": "k1", "choice": "sealed"}))["ok"], json!(true));
    assert_eq!(rig.outside.asked(), before + 1);
    // A key made not to leave is not offered, whatever the choice
    assert_eq!(rig.ask(json!({"op": "sync-choose", "kind": "key", "id": "k2", "choice": "sealed"}))["problem"], json!("not-exportable"));
    assert_eq!(rig.ask(json!({"op": "sync-choose", "kind": "key", "id": "k2", "choice": "on"}))["problem"], json!("not-exportable"));
    assert_eq!(rig.ask(json!({"op": "sync-choose", "kind": "data-key", "id": "x", "choice": "sealed"}))["problem"], json!("not-syncable"));
    until(|| (rig.server_ciphers() == 1).then_some(()));
    let fake = rig.fake.lock().unwrap();
    assert!(fake.seen.iter().all(|seen| !seen.body.contains("PRIVATE-KEY-MATERIAL") && !seen.body.contains("NEVER")));
    let content = cipher::open(&fake.ciphers[0], &fake.user).unwrap().content;
    assert_eq!(content.name, "Amethystora sealed item");
    assert!(!content.notes.unwrap().contains("PRIVATE"));
}

#[test]
fn a_kind_is_one_switch_asked_once_that_covers_what_is_added_later() {
    let rig = rig("kind", Settings::default());
    rig.set_up();
    rig.put("key", "k1", "A key", json!({"key_kind": "ssh", "exportable": true}), json!({"private": "PRIVATE-KEY-MATERIAL"}));
    rig.put("key", "k2", "Locked in", json!({"key_kind": "ssh", "exportable": false}), json!({"private": "NEVER"}));
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    let status = rig.status();
    assert_eq!(status["kinds"]["keys"], json!("off"));
    assert_eq!(status["kinds"]["passwords"], json!("on"));
    // Sealed, for the whole kind, after one question that says what it is
    let before = rig.outside.asked();
    assert_eq!(rig.ask(json!({"op": "sync-kind", "kind": "keys", "choice": "sealed"}))["ok"], json!(true));
    assert_eq!(rig.outside.asked(), before + 1);
    assert_eq!(rig.outside.said.lock().unwrap().asked.last().unwrap().message, say::SYNC_KIND_SEALED);
    until(|| (rig.server_ciphers() == 1).then_some(()));
    assert_eq!(rig.status()["kinds"]["keys"], json!("sealed"));
    // One added later follows it; a key made not to leave never goes
    rig.put("key", "k3", "Another", json!({"key_kind": "ssh", "exportable": true}), json!({"private": "LATER-KEY"}));
    // Asked again while the run of the first choice is still at it
    until(|| (rig.ask(json!({"op": "sync-now"}))["ok"] == json!(true)).then_some(()));
    assert_eq!(rig.server_ciphers(), 2);
    {
        let fake = rig.fake.lock().unwrap();
        assert!(fake.seen.iter().all(|seen| !seen.body.contains("NEVER") && !seen.body.contains("LATER-KEY") && !seen.body.contains("PRIVATE-KEY-MATERIAL")));
    }
    // Off moves the copies to the server's trash, after a question of its own
    let before = rig.outside.asked();
    assert_eq!(rig.ask(json!({"op": "sync-kind", "kind": "keys", "choice": "off"}))["ok"], json!(true));
    assert_eq!(rig.outside.asked(), before + 1);
    assert_eq!(rig.outside.said.lock().unwrap().asked.last().unwrap().message, say::SYNC_KIND_OFF);
    until(|| (rig.fake.lock().unwrap().ciphers.iter().all(|raw| !raw["deletedDate"].is_null())).then_some(()));
    assert_eq!(rig.server_ciphers(), 2, "nothing was destroyed");
    // Words that are not a kind, or a choice, are refused
    assert_eq!(rig.ask(json!({"op": "sync-kind", "kind": "bananas", "choice": "on"}))["problem"], json!("not-syncable"));
    assert_eq!(rig.ask(json!({"op": "sync-kind", "kind": "keys", "choice": "maybe"}))["why"], json!("bad-request"));
    assert_eq!(rig.ask(json!({"op": "sync-kind", "kind": "data-key", "choice": "on"}))["problem"], json!("not-syncable"));
}

// --- The review's findings, through the whole path ----------------------------------------------------------------------------

/// Distinct logins on the server, as another client's import would be
fn flood(fake: &mut Fake, count: usize) {
    for n in 0..count {
        let content = Content { kind: LOGIN, name: format!("Flood {n}"), username: Some("u".into()), password: Some(format!("pw {n} z")), ..Content::default() };
        let body = cipher::create_body(&content, USER_ID, &fake.user).unwrap();
        fake.store(body);
    }
    fake.revision += 1;
}

/// Every file under a folder whose name starts so
fn files_named(folder: &std::path::Path, prefix: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_named(&path, prefix));
        } else if entry.file_name().to_string_lossy().starts_with(prefix) {
            found.push(path);
        }
    }
    found
}

#[test]
fn h1_the_yes_is_for_what_was_asked_and_for_the_connection_that_asked() {
    let rig = rig("h1", Settings::default());
    rig.set_up();
    rig.fake.lock().unwrap().two_factor = true;
    let base = rig.outside.asked();
    let mut first = rig.talk();
    let answer = first.ask(rig.connection_request());
    assert_eq!(answer["problem"], json!("two-factor"), "{answer}");
    // The person was asked, with the address and the account in the sentence
    assert_eq!(rig.outside.asked(), base + 1);
    {
        let said = rig.outside.said.lock().unwrap();
        assert_eq!((said.asked[base].message, said.asked[base].item.as_str()), (say::SYNC_CONNECT, format!("{} ({})", rig.address(), EMAIL).as_str()));
    }
    // A probe is for the connection that began one
    assert_eq!(first.ask(json!({"op": "sync-probe", "address": rig.address()}))["ok"], json!(true));
    assert_eq!(rig.talk().ask(json!({"op": "sync-probe", "address": rig.address()}))["ok"], json!(false));
    let mut with_code = rig.connection_request();
    with_code["two_factor"] = json!({"provider": 0, "code": "123456", "remember": false});
    let refuse = |yes: bool| rig.outside.said.lock().unwrap().refuse = !yes;
    // Another connection of the same account, with the same words, is asked on its own: and here says no
    refuse(false);
    assert_eq!(rig.talk().ask(with_code.clone())["ok"], json!(false));
    assert_eq!(rig.outside.asked(), base + 2);
    // Another account on the first connection is asked about
    let mut other_account = with_code.clone();
    other_account["email"] = json!("someone@else.test");
    assert_eq!(first.ask(other_account)["ok"], json!(false));
    assert_eq!(rig.outside.asked(), base + 3);
    // The code, on the connection that was asked, goes through with no further asking
    refuse(true);
    let answer = first.ask(with_code.clone());
    assert_eq!(answer["ok"], json!(true), "{answer}");
    assert_eq!(rig.outside.asked(), base + 3);
    rig.synced();
    // And the yes is spent: connecting again, from anywhere, is asked about
    let mut again = with_code.clone();
    again["reconnect"] = json!(true);
    let answer = rig.talk().ask(again);
    assert_eq!(answer["ok"], json!(true), "{answer}");
    assert_eq!(rig.outside.asked(), base + 4);
    // A certificate nobody was shown is asked about with all of its fingerprint. (This is last: the service answers no for a minute to a
    // program refused three times in that minute, which is why the refusals are not all before the yes.)
    refuse(false);
    let mut with_certificate = with_code;
    with_certificate["reconnect"] = json!(true);
    with_certificate["ca"] = json!("-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----\n");
    with_certificate["pin"] = json!("ab".repeat(32));
    assert_eq!(rig.talk().ask(with_certificate)["ok"], json!(false));
    let said = rig.outside.said.lock().unwrap();
    assert_eq!(said.asked.len(), base + 5);
    assert_eq!(said.asked[base + 4].message, say::SYNC_CONNECT_CA);
    assert!(said.asked[base + 4].item.contains(&"ab".repeat(32)) && said.asked[base + 4].item.contains(EMAIL) && said.asked[base + 4].item.contains(&rig.address()), "{}", said.asked[base + 4].item);
}

#[test]
fn h6_an_item_that_does_not_read_stops_the_run_and_trashes_nothing() {
    let rig = rig("h6", Settings::default());
    rig.set_up();
    rig.password("p1", "Example", "u", "p", "");
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    let file = files_named(&rig.folder.0.join("state"), "password.p1").pop().expect("the item's file");
    std::fs::write(&file, b"damaged").unwrap();
    let before = rig.fake.lock().unwrap().seen.len();
    let answer = rig.ask(json!({"op": "sync-now"}));
    assert_eq!(answer["problem"]["name"], json!("local-unreadable"), "{answer}");
    let fake = rig.fake.lock().unwrap();
    assert_eq!(fake.seen.len(), before, "the server was asked nothing");
    assert!(fake.ciphers[0]["deletedDate"].is_null());
}

#[test]
fn h6_links_that_do_not_read_stop_the_run_rather_than_start_over() {
    let rig = rig("h6l", Settings::default());
    rig.set_up();
    rig.password("p1", "Example", "u", "p", "");
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    for file in files_named(&rig.folder.0.join("state"), "sync-links.") {
        std::fs::write(file, b"damaged").unwrap();
    }
    let answer = rig.ask(json!({"op": "sync-now"}));
    assert_eq!(answer["problem"]["name"], json!("links-unreadable"), "{answer}");
    assert_eq!(rig.server_ciphers(), 1, "not put there again");
}

#[test]
fn h7_a_lock_ends_a_run_at_once() {
    let rig = rig("h7", Settings::default());
    rig.set_up();
    for n in 0..30 {
        rig.password(&format!("p{n}"), &format!("Item {n}"), "u", "p", "");
    }
    rig.fake.lock().unwrap().hooks.insert("write", Box::new(|_| std::thread::sleep(Duration::from_millis(200))));
    assert_eq!(rig.connect()["ok"], json!(true));
    until(|| (rig.server_ciphers() >= 2).then_some(()));
    assert_eq!(rig.ask(json!({"op": "lock"}))["ok"], json!(true));
    let at_lock = rig.server_ciphers();
    std::thread::sleep(Duration::from_millis(2500));
    // What was in flight may land; nothing is asked after
    assert!(rig.server_ciphers() <= at_lock + 2, "{at_lock} at the lock, {} after", rig.server_ciphers());
}

#[test]
fn h7_a_run_has_a_limit_of_its_own() {
    let rig = rig("h7l", Settings { run_limit: Duration::from_secs(8), ..Settings::default() });
    rig.set_up();
    for n in 0..30 {
        rig.password(&format!("p{n}"), &format!("Item {n}"), "u", "p", "");
    }
    rig.fake.lock().unwrap().hooks.insert("write", Box::new(|_| std::thread::sleep(Duration::from_millis(700))));
    let started = Instant::now();
    assert_eq!(rig.connect()["ok"], json!(true));
    until(|| (rig.status()["problem"]["name"] == json!("timeout")).then_some(()));
    assert!(started.elapsed() < Duration::from_secs(30));
    assert!(rig.server_ciphers() < 30, "the run stopped where the limit found it: {}", rig.server_ciphers());
}

#[test]
fn h8_a_large_pull_does_not_hold_the_service_up_and_all_of_it_lands() {
    let rig = rig("h8", Settings::default());
    rig.set_up();
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    let count: usize = std::env::var("H8_ITEMS").ok().and_then(|n| n.parse().ok()).unwrap_or(300);
    flood(&mut rig.fake.lock().unwrap(), count);
    let socket = rig.socket.clone();
    let runner = std::thread::spawn(move || amethystora_vault_client::exchange(&socket, &json!({"v": 1, "op": "sync-now"}), Duration::from_secs(300)).unwrap());
    let mut worst = Duration::ZERO;
    while !runner.is_finished() {
        let started = Instant::now();
        let _ = amethystora_vault_client::exchange(&rig.socket, &json!({"v": 1, "op": "list", "kind": "nothing"}), Duration::from_secs(300));
        worst = worst.max(started.elapsed());
        std::thread::sleep(Duration::from_millis(10));
    }
    // And through the slices that follow
    let mut polls = 0;
    until(|| {
        let started = Instant::now();
        let listed = rig.ask(json!({"op": "list", "kind": "nothing"}));
        worst = worst.max(started.elapsed());
        polls += 1;
        (listed["ok"] == json!(true) && rig.status()["running"] != json!(true)).then_some(())
    });
    println!("H8 worst answer while {count} items were written: {worst:?} over {polls} polls");
    assert!(worst < Duration::from_millis(1500), "{worst:?}");
    assert_eq!(rig.items("password").len(), count);
    assert_eq!(rig.status()["linked"], json!(count));
    // Nothing is left beside the items
    assert!(files_named(&rig.folder.0.join("state"), ".tmp-").is_empty());
}

#[test]
fn m5_a_write_to_a_hidden_vault_never_reaches_the_server() {
    let rig = rig("m5", Settings::default());
    rig.set_up();
    rig.password("p1", "First vault", "u", "p", "");
    let recovery = rig.ask(json!({"op": "new", "name": "Secret", "hidden": true, "secret": b64("another password")}));
    assert_eq!(recovery["ok"], json!(true));
    let typed = recovery["recovery"].as_str().unwrap().to_string();
    assert_eq!(rig.ask(json!({"op": "confirm", "recovery": b64(&typed)}))["ok"], json!(true));
    assert_eq!(rig.ask(json!({"op": "lock"}))["ok"], json!(true));
    assert_eq!(rig.ask(json!({"op": "unlock", "method": "password", "secret": b64("another password")}))["ok"], json!(true));
    assert_eq!(rig.ask(json!({"op": "unlock", "method": "password", "secret": b64(VAULT_PASSWORD)}))["ok"], json!(true));
    let hidden = rig.ask(json!({"op": "status"}))["vaults"].as_array().unwrap().iter().find(|vault| vault["first"] != json!(true) && vault["open"] == json!(true)).cloned().unwrap();
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    std::thread::sleep(Duration::from_millis(400));
    let before = rig.fake.lock().unwrap().seen.len();
    let value = serde_json::to_value(Password { id: "h1".into(), title: "Hidden one".into(), password: Secret::new("never leaves"), ..Password::default() }).unwrap();
    let answer = rig.ask(json!({"op": "put", "kind": "password", "id": "h1", "name": "Hidden one", "value": value, "vault": hidden["id"]}));
    assert_eq!(answer["ok"], json!(true), "{answer}");
    // A request that is refused counts for nothing either
    let refused = rig.ask(json!({"op": "put", "kind": "password", "id": "../x", "name": "Bad", "value": {}, "vault": hidden["id"]}));
    assert_eq!(refused["ok"], json!(false));
    std::thread::sleep(Duration::from_millis(900));
    assert_eq!(rig.fake.lock().unwrap().seen.len(), before, "the server heard nothing of a write to the hidden vault");
}

#[test]
fn m6_an_argon2_account_connects_through_a_child_of_the_service() {
    let folder = Folder::new("child-stub");
    let marker = folder.0.join("ran");
    let stub = folder.0.join("child");
    std::fs::write(&stub, format!("#!/bin/sh\n: > {}\nexec {} \"$@\"\n", marker.display(), env!("CARGO_BIN_EXE_amethystora-vault-service"))).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let rig = rig("child", Settings { kdf: Some(Kdf { kind: 1, iterations: 2, memory: Some(16), parallelism: Some(1) }), child: Some(stub), ..Settings::default() });
    rig.set_up();
    rig.password("p1", "Example", "u", "p", "");
    let answer = rig.connect();
    assert_eq!(answer["ok"], json!(true), "{answer}");
    assert!(marker.exists(), "Argon2 was made by the child");
    rig.synced();
    assert_eq!(rig.server_ciphers(), 1);
}

#[test]
fn l4_accepting_what_went_and_turning_an_item_off_ask_and_the_service_keeps_what_is_its_own() {
    let rig = rig("l4", Settings::default());
    rig.set_up();
    rig.password("p1", "Example", "u", "p", "");
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    // Accepting asks
    let before = rig.outside.asked();
    assert_eq!(rig.ask(json!({"op": "sync-now", "accept_gone": true}))["ok"], json!(true));
    assert_eq!(rig.outside.asked(), before + 1);
    assert_eq!(rig.outside.said.lock().unwrap().asked.last().unwrap().message, say::SYNC_ACCEPT);
    // Turning an item off asks, and a no leaves its copy on the server
    rig.outside.said.lock().unwrap().refuse = true;
    assert_eq!(rig.ask(json!({"op": "sync-choose", "kind": "password", "id": "p1", "choice": "off"}))["ok"], json!(false));
    std::thread::sleep(Duration::from_millis(600));
    assert!(rig.fake.lock().unwrap().ciphers[0]["deletedDate"].is_null());
    rig.outside.said.lock().unwrap().refuse = false;
    assert_eq!(rig.ask(json!({"op": "sync-choose", "kind": "password", "id": "p1", "choice": "off"}))["ok"], json!(true));
    assert_eq!(rig.outside.said.lock().unwrap().asked.last().unwrap().message, say::SYNC_ITEM_OFF);
    until(|| rig.fake.lock().unwrap().ciphers[0]["deletedDate"].is_string().then_some(()));
    // The connection and the links are not a program's to remove
    let shard = rig.items("sync-links").first().and_then(|link| link["id"].as_str().map(str::to_string)).unwrap_or_else(|| "0".to_string());
    assert_eq!(rig.ask(json!({"op": "remove", "kind": "sync-links", "id": shard}))["ok"], json!(false));
    assert_eq!(rig.ask(json!({"op": "remove", "kind": "sync-account", "id": "account"}))["ok"], json!(false));
    assert_eq!(rig.status()["connected"], json!(true));
}

#[test]
fn an_item_set_to_ask_every_time_is_off_sync_until_it_is_chosen() {
    let rig = rig("ask", Settings::default());
    rig.set_up();
    let value = serde_json::to_value(Password { id: "p1".into(), title: "Ask me".into(), username: "u".into(), password: Secret::new("asked-item-secret"), ..Password::default() }).unwrap();
    let answer = rig.ask(json!({"op": "put", "kind": "password", "id": "p1", "name": "Ask me", "value": value, "approval": "ask"}));
    assert_eq!(answer["ok"], json!(true), "{answer}");
    rig.password("p2", "Plain", "u", "plain-item-secret", "");
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    assert_eq!(rig.server_ciphers(), 1, "only the plain one went");
    // Chosen, after the person is asked, it goes
    let before = rig.outside.asked();
    assert_eq!(rig.ask(json!({"op": "sync-choose", "kind": "password", "id": "p1", "choice": "on"}))["ok"], json!(true));
    assert_eq!(rig.outside.asked(), before + 1);
    until(|| (rig.server_ciphers() == 2).then_some(()));
}

#[test]
fn a_tombstone_past_its_thirty_days_is_removed_by_a_run() {
    let rig = rig("purge", Settings::default());
    rig.set_up();
    let now = amethystora_vault_core::sync::merge::now_ms();
    for (id, age_days) in [("old", 31), ("new", 2)] {
        let value = serde_json::to_value(Password { id: id.into(), title: id.into(), password: Secret::new("a gone one"), deleted_at: Some(now - age_days * 24 * 3600 * 1000), ..Password::default() }).unwrap();
        rig.put("password", id, id, json!({"deletedAt": now}), value);
    }
    assert_eq!(rig.connect()["ok"], json!(true));
    rig.synced();
    let left: Vec<String> = rig.items("password").iter().map(|item| item["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(left, vec!["new".to_string()]);
}
