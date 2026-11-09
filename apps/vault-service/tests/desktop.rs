//! The real service binary, started as systemd starts it, on a private bus with stand-ins for logind and polkit that speak the
//! real protocol, and the real command line. Skipped where there is no dbus-daemon. `budgets` measures what
//! plans/vault/IMPLEMENTERS.md asks: run it with `cargo test --release --test desktop -- --ignored budgets --nocapture`.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use amethystora_vault_client::{exchange, open_at, seal_at, Refusal};
use amethystora_vault_service::bus::{Bus, Message, Writer, METHOD_CALL};
use common::{b64, Folder};
use serde_json::{json, Value};

const SERVICE: &str = env!("CARGO_BIN_EXE_amethystora-vault-service");
const CLI: &str = env!("CARGO_BIN_EXE_amethystora-vault-cli");
const PASSWORD: &str = "correct horse battery";
/// What logind keeps in /run/systemd/users for an account that has a session
const LOGGED_IN: &str = "NAME=me\nSESSIONS=2\n";
/// And the session it names
fn session_file(uid: u32) -> String {
    format!("UID={uid}\nCLASS=user\nSTATE=active\n")
}

/// What the stand-ins saw, and what they answer
#[derive(Default)]
struct Seen {
    checks: Vec<String>,
    /// The subject's keys, and whether a descriptor came with the call
    subject_had_pidfd: bool,
    cancelled: usize,
    /// The accounts ListSessions gives
    sessions: Vec<u32>,
    refuse: bool,
    /// What each session's LockedHint says
    locked: bool,
    /// What class each session is
    class: &'static str,
    /// Hold polkit's answer: the person is thinking
    silent: bool,
    /// The read end of the inhibitor it handed out: readable once the service has let go
    inhibitor: Option<OwnedFd>,
    inhibits: usize,
    /// The service's own name on the bus, from the calls it made
    service: String,
}

type ToSend = Arc<Mutex<Vec<Box<dyn FnOnce(&mut Bus) + Send>>>>;

struct Lab {
    folder: Folder,
    bus_address: PathBuf,
    socket: PathBuf,
    daemon: Child,
    seen: Arc<Mutex<Seen>>,
    /// Signals for the stand-in to send
    send: ToSend,
    stop: Arc<AtomicBool>,
    desktop: Option<std::thread::JoinHandle<()>>,
    listener: UnixListener,
    service: Option<Child>,
}

fn has_dbus() -> bool {
    Command::new("dbus-daemon").arg("--version").stdout(Stdio::null()).status().is_ok_and(|status| status.success())
}

/// Whether the service, which seals an approval slot through systemd-creds whenever sysfs shows a chip, can: in a toolbox on a
/// machine with a TPM, sysfs shows the host's chip and systemd-creds cannot reach the host's credential service. A build stage
/// shows no chip, and the slot is made without one.
fn chip_usable() -> bool {
    let chip = std::fs::read_dir("/sys/class/tpmrm").is_ok_and(|mut chips| chips.next().is_some());
    if !chip {
        return true;
    }
    let child = Command::new("systemd-creds").args(["--user", "encrypt", "--name=vault-test", "-", "-"]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    child.is_ok_and(|mut child| {
        drop(child.stdin.take());
        child.wait().is_ok_and(|status| status.success())
    })
}

fn my_uid() -> u32 {
    // SAFETY: getuid cannot fail
    unsafe { libc::getuid() }
}

fn standins(mut bus: Bus, seen: Arc<Mutex<Seen>>, send: ToSend, stop: Arc<AtomicBool>) {
    for name in ["org.freedesktop.login1", "org.freedesktop.PolicyKit1"] {
        let mut body = Writer::default();
        body.string(name);
        body.u32(0);
        bus.call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName", "su", &body.bytes);
    }
    while !stop.load(Ordering::Relaxed) {
        let _ = bus.flush();
        if bus.read().is_err() {
            return;
        }
        for action in send.lock().unwrap().drain(..) {
            action(&mut bus);
        }
        while let Ok(Some(message)) = bus.next_message() {
            if message.kind == METHOD_CALL {
                answer_call(&mut bus, &message, &seen);
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn answer_call(bus: &mut Bus, message: &Message, seen: &Mutex<Seen>) {
    let mut seen = seen.lock().unwrap();
    if message.member == "CheckAuthorization" || message.member == "Inhibit" {
        seen.service = message.sender.clone();
    }
    match message.member.as_str() {
        "CheckAuthorization" => {
            seen.checks.push(message.body.iter().map(|byte| if byte.is_ascii_graphic() || *byte == b' ' { *byte as char } else { '.' }).collect());
            seen.subject_had_pidfd = !message.fds.is_empty();
            if seen.silent {
                return;
            }
            let mut body = Writer::default();
            body.align(8);
            body.bool(!seen.refuse);
            body.bool(false);
            body.array(8, |_| {});
            bus.reply(message, "(bba{ss})", &body.bytes, None);
        }
        "CancelCheckAuthorization" => {
            seen.cancelled += 1;
            bus.reply(message, "", &[], None);
        }
        "ListSessions" => {
            let sessions = seen.sessions.clone();
            let mut body = Writer::default();
            body.array(8, |writer| {
                for uid in sessions {
                    writer.align(8);
                    writer.string("2");
                    writer.u32(uid);
                    writer.string("me");
                    writer.string("seat0");
                    writer.string("/org/freedesktop/login1/session/_32");
                }
            });
            bus.reply(message, "a(susso)", &body.bytes, None);
        }
        "Get" if message.body.windows(10).any(|window| window == b"LockedHint") => {
            let mut body = Writer::default();
            body.signature("b");
            body.bool(seen.locked);
            bus.reply(message, "v", &body.bytes, None);
        }
        "Get" if message.body.windows(5).any(|window| window == b"Class") => {
            let mut body = Writer::default();
            body.signature("s");
            body.string(seen.class);
            bus.reply(message, "v", &body.bytes, None);
        }
        "Get" => {
            let mut body = Writer::default();
            body.signature("(uo)");
            body.align(8);
            body.u32(my_uid());
            body.string("/org/freedesktop/login1/user/_1000");
            bus.reply(message, "v", &body.bytes, None);
        }
        "Inhibit" => {
            let mut ends = [0; 2];
            // SAFETY: pipe2 writes two descriptors, which are then owned
            let (read, write) = unsafe {
                assert_eq!(libc::pipe2(ends.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK), 0);
                use std::os::fd::FromRawFd;
                (OwnedFd::from_raw_fd(ends[0]), OwnedFd::from_raw_fd(ends[1]))
            };
            seen.inhibitor = Some(read);
            seen.inhibits += 1;
            let mut body = Writer::default();
            body.u32(0);
            bus.reply(message, "h", &body.bytes, Some(write));
        }
        _ => {}
    }
}

fn start_lab(name: &str) -> Lab {
    let folder = Folder::new(name);
    let bus_address = folder.0.join("bus");
    let daemon = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--nopidfile", &format!("--address=unix:path={}", bus_address.display())])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut bus = None;
    for _ in 0..200 {
        if let Ok(connected) = Bus::connect(&bus_address) {
            bus = Some(connected);
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let bus = bus.expect("the private bus");
    let seen = Arc::new(Mutex::new(Seen { sessions: vec![my_uid()], class: "user", ..Seen::default() }));
    let send = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let desktop = {
        let (seen, send, stop) = (seen.clone(), send.clone(), stop.clone());
        std::thread::spawn(move || standins(bus, seen, send, stop))
    };
    let socket = folder.0.join("vault.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    std::fs::create_dir_all(folder.0.join("state")).unwrap();
    // What logind keeps for an account that is logged in
    std::fs::create_dir_all(folder.0.join("users")).unwrap();
    std::fs::write(folder.0.join("users").join(my_uid().to_string()), LOGGED_IN).unwrap();
    std::fs::create_dir_all(folder.0.join("sessions")).unwrap();
    std::fs::write(folder.0.join("sessions").join("2"), session_file(my_uid())).unwrap();
    // The stand-ins' names are theirs before anyone asks for them
    std::thread::sleep(Duration::from_millis(100));
    Lab { folder, bus_address, socket, daemon, seen, send, stop, desktop: Some(desktop), listener, service: None }
}

/// As systemd starts it: the listening socket as descriptor 3, and LISTEN_PID its own
fn spawn_service(lab: &Lab, stderr: Stdio, wrapper: &[&str]) -> Child {
    let fd = lab.listener.as_raw_fd();
    let mut command = Command::new("sh");
    command.arg("-c").arg(r#"export LISTEN_PID=$$; exec "$@""#).arg("sh");
    command.args(wrapper).arg(SERVICE);
    command
        .env("LISTEN_FDS", "1")
        .env("STATE_DIRECTORY", lab.folder.0.join("state"))
        .env("AMETHYSTORA_VAULT_LOGIND_USERS", lab.folder.0.join("users"))
        .env("AMETHYSTORA_VAULT_LOGIND_SESSIONS", lab.folder.0.join("sessions"))
        .env("DBUS_SYSTEM_BUS_ADDRESS", format!("unix:path={}", lab.bus_address.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr);
    // SAFETY: dup2 is async-signal-safe, and this runs between fork and exec
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(fd, 3) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.spawn().unwrap()
}

impl Lab {
    fn ask(&self, mut request: Value) -> Value {
        request["v"] = json!(1);
        exchange(&self.socket, &request, Duration::from_secs(30)).unwrap()
    }

    fn signal(&self, action: impl FnOnce(&mut Bus) + Send + 'static) {
        self.send.lock().unwrap().push(Box::new(action));
    }

    fn lock_session(&self) {
        self.signal(|bus| {
            let mut body = Writer::default();
            body.string("org.freedesktop.login1.Session");
            body.array(8, |writer| {
                writer.align(8);
                writer.string("LockedHint");
                writer.signature("b");
                writer.bool(true);
            });
            body.array(4, |_| {});
            bus.signal("/org/freedesktop/login1/session/_32", "org.freedesktop.DBus.Properties", "PropertiesChanged", "sa{sv}as", &body.bytes);
        });
    }

    fn lock_signal(&self) {
        self.signal(|bus| {
            bus.signal("/org/freedesktop/login1/session/_32", "org.freedesktop.login1.Session", "Lock", "", &[]);
        });
    }

    fn sleep_machine(&self) {
        self.signal(|bus| {
            let mut body = Writer::default();
            body.bool(true);
            bus.signal("/org/freedesktop/login1", "org.freedesktop.login1.Manager", "PrepareForSleep", "b", &body.bytes);
        });
    }

    fn log_in(&self) {
        std::fs::write(self.folder.0.join("users").join(my_uid().to_string()), LOGGED_IN).unwrap();
        std::fs::write(self.folder.0.join("sessions").join("2"), session_file(my_uid())).unwrap();
        self.seen.lock().unwrap().sessions = vec![my_uid()];
    }

    fn end_session(&self) {
        self.seen.lock().unwrap().sessions.clear();
        self.session_removed();
    }

    fn session_removed(&self) {
        self.signal(|bus| {
            let mut body = Writer::default();
            body.string("2");
            body.string("/org/freedesktop/login1/session/_32");
            bus.signal("/org/freedesktop/login1", "org.freedesktop.login1.Manager", "SessionRemoved", "so", &body.bytes);
        });
    }

    fn is_open(&self) -> bool {
        self.ask(json!({"op": "status"}))["vaults"].as_array().unwrap().iter().any(|vault| vault["open"] == json!(true))
    }

    fn until_closed(&self) {
        for _ in 0..500 {
            if !self.is_open() {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("it stayed open");
    }

    /// Setup through the real command line, as a script would drive it
    fn cli_set_up(&self, password: &str) {
        let mut child = self.cli(&["setup"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{password}").unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut lines = Vec::new();
        for _ in 0..2 {
            let mut line = String::new();
            stdout.read_line(&mut line).unwrap();
            lines.push(line);
        }
        writeln!(stdin, "{}", lines[1].trim()).unwrap();
        drop(stdin);
        assert!(child.wait().unwrap().success());
    }

    fn cli(&self, args: &[&str]) -> Command {
        let mut command = Command::new(CLI);
        command.args(args).env("AMETHYSTORA_VAULT_SOCKET", &self.socket);
        command
    }

    fn cli_with_input(&self, args: &[&str], input: &str) -> (i32, String) {
        let mut child = self.cli(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        let output = child.wait_with_output().unwrap();
        (output.status.code().unwrap_or(-1), String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

impl Drop for Lab {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(mut service) = self.service.take() {
            let _ = service.kill();
            let _ = service.wait();
        }
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
        if let Some(desktop) = self.desktop.take() {
            let _ = desktop.join();
        }
    }
}

fn readable(fd: &OwnedFd) -> bool {
    let mut poll = libc::pollfd { fd: fd.as_raw_fd(), events: libc::POLLIN, revents: 0 };
    // SAFETY: one pollfd, not waiting
    unsafe { libc::poll(&mut poll, 1, 0) > 0 }
}

#[test]
fn the_real_thing_opens_closes_and_says_nothing() {
    if !has_dbus() {
        eprintln!("skipped: no dbus-daemon here");
        return;
    }
    let mut lab = start_lab("real");
    let stderr_file = lab.folder.0.join("stderr");
    let stderr = std::fs::File::create(&stderr_file).unwrap();
    lab.service = Some(spawn_service(&lab, Stdio::from(stderr), &[]));
    // Never set up
    assert_eq!(seal_at(&lab.socket, "resume", "r", b"x"), Err(Refusal::NotSetUp));
    let (code, _) = lab.cli_with_input(&["open"], "");
    assert_eq!(code, 3);

    // polkit, through the real wire: the process that asks by its pidfd, the sentence, and the action's own name
    lab.seen.lock().unwrap().refuse = true;
    let (code, _) = lab.cli_with_input(&["setup"], &format!("{PASSWORD}\n"));
    assert_eq!(code, 1);
    lab.seen.lock().unwrap().refuse = false;
    {
        let seen = lab.seen.lock().unwrap();
        assert_eq!(seen.checks.len(), 1);
        assert!(seen.checks[0].contains("unix-process"), "{}", seen.checks[0]);
        assert!(seen.checks[0].contains("org.amethystora.vault.open"), "{}", seen.checks[0]);
        assert!(seen.checks[0].contains("polkit.message"), "{}", seen.checks[0]);
        assert!(seen.checks[0].contains("A program calling itself $(program)"), "{}", seen.checks[0]);
        assert!(seen.checks[0].contains("amethystora-vaul"), "the program's own word for itself: {}", seen.checks[0]);
        assert!(seen.subject_had_pidfd);
    }

    lab.cli_set_up(PASSWORD);
    assert!(lab.is_open());
    let sealed = seal_at(&lab.socket, "resume", "r", b"plain").unwrap();
    assert_eq!(open_at(&lab.socket, "resume", "r", &sealed).unwrap(), b"plain");
    // It took the inhibitor while open
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(lab.seen.lock().unwrap().inhibits, 1);
    assert!(!readable(lab.seen.lock().unwrap().inhibitor.as_ref().unwrap()));

    // The screen locks
    lab.lock_session();
    lab.until_closed();
    assert_eq!(seal_at(&lab.socket, "resume", "r", b"x"), Err(Refusal::Closed));

    // logind's Lock, which comes before LockedHint does
    assert_eq!(lab.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))["ok"], json!(true));
    std::thread::sleep(Duration::from_millis(50));
    lab.lock_signal();
    lab.until_closed();

    // The command line reads the password from standard input, and says which way it went by its code
    let (code, _) = lab.cli_with_input(&["open", "password"], "not the password\n");
    assert_eq!(code, 1);
    let (code, printed) = lab.cli_with_input(&["open", "password"], &format!("{PASSWORD}\n"));
    assert_eq!(code, 0);
    assert!(printed.contains("Vault"), "{printed}");
    assert!(lab.is_open());

    // The machine sleeps, and the inhibitor goes with the key
    std::thread::sleep(Duration::from_millis(50));
    lab.sleep_machine();
    lab.until_closed();
    std::thread::sleep(Duration::from_millis(50));
    assert!(readable(lab.seen.lock().unwrap().inhibitor.as_ref().unwrap()), "the sleep is let go");

    // Another session of the account ends and a person's is left: it stays open
    assert_eq!(lab.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))["ok"], json!(true));
    std::thread::sleep(Duration::from_millis(50));
    lab.session_removed();
    std::thread::sleep(Duration::from_millis(100));
    assert!(lab.is_open());

    // Only the account's manager session is left, which never ends with lingering: that is a logout
    lab.seen.lock().unwrap().class = "manager";
    lab.session_removed();
    lab.until_closed();
    lab.seen.lock().unwrap().class = "user";

    // The session ends
    assert_eq!(lab.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))["ok"], json!(true));
    std::thread::sleep(Duration::from_millis(50));
    lab.end_session();
    lab.until_closed();
    lab.log_in();

    // A hidden vault leaves nothing behind it, through every way of saying
    assert_eq!(lab.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))["ok"], json!(true));
    let (code, _) = {
        let mut child = lab.cli(&["new"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        // The name and the word hidden are typed, never part of the command
        write!(stdin, "Hidden name\nhidden\nthe hidden password\n").unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut lines = Vec::new();
        for _ in 0..2 {
            let mut line = String::new();
            stdout.read_line(&mut line).unwrap();
            lines.push(line);
        }
        writeln!(stdin, "{}", lines[1].trim()).unwrap();
        drop(stdin);
        (child.wait().unwrap().code().unwrap_or(-1), ())
    };
    assert_eq!(code, 0);
    let output = lab.cli(&["status"]).output().unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("Hidden name"), "named while it is open");
    assert_eq!(lab.ask(json!({"op": "lock"}))["ok"], json!(true));
    let output = lab.cli(&["status"]).output().unwrap();
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Hidden name"));
    assert_eq!(String::from_utf8_lossy(&output.stdout).lines().filter(|line| line.starts_with("vault\t")).count(), 1);

    // The bus goes: everything closes, and nothing opens until it is back
    assert_eq!(lab.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))["ok"], json!(true));
    let _ = lab.daemon.kill();
    let _ = lab.daemon.wait();
    lab.until_closed();
    assert_eq!(lab.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))["why"], json!("failed"));

    // Nor does anything wait on a person nobody can ask: the answer is no, at once
    let started = Instant::now();
    assert_eq!(lab.ask(json!({"op": "forget-everything"}))["why"], json!("refused"));
    assert!(started.elapsed() < Duration::from_secs(3));

    // Stopping is one more closing
    let mut service = lab.service.take().unwrap();
    // SAFETY: a signal to the child this test owns
    unsafe { libc::kill(service.id() as i32, libc::SIGTERM) };
    let status = service.wait().unwrap();
    assert_eq!(status.code(), Some(0));

    // And nothing, at any point, reached its log
    let logged = std::fs::read_to_string(&stderr_file).unwrap();
    assert!(logged.is_empty(), "{logged}");
}

#[test]
fn nothing_is_put_to_a_person_over_the_lock_screen() {
    if !has_dbus() {
        eprintln!("skipped: no dbus-daemon here");
        return;
    }
    let mut lab = start_lab("over-lock");
    lab.service = Some(spawn_service(&lab, Stdio::null(), &[]));
    lab.cli_set_up(PASSWORD);
    assert_eq!(lab.ask(json!({"op": "put", "kind": "k", "id": "a", "name": "A secret name", "value": 1, "approval": "ask"}))["ok"], json!(true));
    let asked = lab.seen.lock().unwrap().checks.len();
    // Locked: closed, and polkit never hears of it
    lab.seen.lock().unwrap().locked = true;
    assert_eq!(lab.ask(json!({"op": "get", "kind": "k", "id": "a"}))["why"], json!("closed"));
    assert_eq!(lab.seen.lock().unwrap().checks.len(), asked);
    lab.seen.lock().unwrap().locked = false;
    // Not locked: asked
    assert_eq!(lab.ask(json!({"op": "get", "kind": "k", "id": "a"}))["ok"], json!(true));
    assert_eq!(lab.seen.lock().unwrap().checks.len(), asked + 1);
    assert!(lab.seen.lock().unwrap().checks[asked].contains("A secret name"));
    // The prompt is up and the screen locks: it is taken back, and whoever asked hears closed
    lab.seen.lock().unwrap().silent = true;
    let socket = lab.socket.clone();
    let waiting = std::thread::spawn(move || exchange(&socket, &json!({"v": 1, "op": "get", "kind": "k", "id": "a"}), Duration::from_secs(20)).unwrap());
    for _ in 0..200 {
        if lab.seen.lock().unwrap().checks.len() == asked + 2 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(lab.seen.lock().unwrap().checks.len(), asked + 2);
    lab.lock_session();
    assert_eq!(waiting.join().unwrap()["why"], json!("closed"));
    for _ in 0..200 {
        if lab.seen.lock().unwrap().cancelled == 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(lab.seen.lock().unwrap().cancelled, 1);
    assert!(!lab.is_open());

    // The same with nothing open, asking to open it: the bus is there only for the prompt, and the prompt still goes
    if !chip_usable() {
        eprintln!("the rest skipped: sysfs shows a chip that systemd-creds cannot reach here");
        return;
    }
    lab.seen.lock().unwrap().silent = false;
    assert_eq!(lab.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))["ok"], json!(true));
    assert_eq!(lab.ask(json!({"op": "add-slot", "kind": "approval"}))["ok"], json!(true));
    assert_eq!(lab.ask(json!({"op": "lock"}))["ok"], json!(true));
    lab.seen.lock().unwrap().silent = true;
    let asked = lab.seen.lock().unwrap().checks.len();
    let socket = lab.socket.clone();
    let waiting = std::thread::spawn(move || exchange(&socket, &json!({"v": 1, "op": "unlock", "method": "approval"}), Duration::from_secs(20)).unwrap());
    for _ in 0..200 {
        if lab.seen.lock().unwrap().checks.len() == asked + 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    lab.sleep_machine();
    assert_eq!(waiting.join().unwrap()["why"], json!("closed"));
    assert!(!lab.is_open());
}

/// A signal of logind's, as a stranger on the bus sends it straight to the service: little-endian or not
fn signal_to(stranger: &mut Bus, service: &str, interface: &str, member: &str, signature: &str, body: &[u8]) {
    stranger.signal_to(service, "/org/freedesktop/login1", interface, member, signature, body);
    let _ = stranger.flush();
}

/// A big-endian signal, which logind never sends and a stranger can
fn big_endian_signal(service: &str) -> Vec<u8> {
    let mut fields = Vec::new();
    let mut add = |code: u8, signature: u8, value: &str| {
        while fields.len() % 8 != 0 {
            fields.push(0);
        }
        fields.extend([code, 1, signature, 0]);
        while fields.len() % 4 != 0 {
            fields.push(0);
        }
        fields.extend((value.len() as u32).to_be_bytes());
        fields.extend(value.bytes());
        fields.push(0);
    };
    add(1, b'o', "/org/freedesktop/login1");
    add(2, b's', "org.freedesktop.login1.Manager");
    add(3, b's', "PrepareForSleep");
    add(6, b's', service);
    let mut message = vec![b'B', 4, 0, 1];
    message.extend(0u32.to_be_bytes());
    message.extend(1u32.to_be_bytes());
    message.extend((fields.len() as u32).to_be_bytes());
    message.extend(fields);
    while message.len() % 8 != 0 {
        message.push(0);
    }
    message
}

#[test]
fn a_stranger_on_the_bus_cannot_close_anything_or_say_yes() {
    if !has_dbus() {
        eprintln!("skipped: no dbus-daemon here");
        return;
    }
    let mut lab = start_lab("stranger");
    lab.service = Some(spawn_service(&lab, Stdio::null(), &[]));
    lab.cli_set_up(PASSWORD);
    assert_eq!(lab.ask(json!({"op": "put", "kind": "k", "id": "a", "name": "n", "value": "secret", "approval": "ask"}))["ok"], json!(true));
    std::thread::sleep(Duration::from_millis(100));
    let service = lab.seen.lock().unwrap().service.clone();
    assert!(!service.is_empty());
    // Another program on the same bus, with a name of its own
    let mut stranger = Bus::connect(&lab.bus_address).unwrap();
    let mut sleeping = Writer::default();
    sleeping.bool(true);
    signal_to(&mut stranger, &service, "org.freedesktop.login1.Manager", "PrepareForSleep", "b", &sleeping.bytes);
    signal_to(&mut stranger, &service, "org.freedesktop.login1.Session", "Lock", "", &[]);
    let mut changed = Writer::default();
    changed.string("org.freedesktop.login1.Session");
    changed.array(8, |writer| {
        writer.align(8);
        writer.string("LockedHint");
        writer.signature("b");
        writer.bool(true);
    });
    changed.array(4, |_| {});
    signal_to(&mut stranger, &service, "org.freedesktop.DBus.Properties", "PropertiesChanged", "sa{sv}as", &changed.bytes);
    let mut removed = Writer::default();
    removed.string("2");
    removed.string("/org/freedesktop/login1/session/_32");
    signal_to(&mut stranger, &service, "org.freedesktop.login1.Manager", "SessionRemoved", "so", &removed.bytes);
    stranger.send_raw(&big_endian_signal(&service));
    let _ = stranger.flush();
    std::thread::sleep(Duration::from_millis(500));
    assert!(lab.is_open(), "a stranger's signals closed it");
    // Still answering, and the real thing still closes it
    assert_eq!(lab.ask(json!({"op": "status"}))["ok"], json!(true));
    lab.lock_session();
    lab.until_closed();
}

#[test]
fn the_service_refuses_to_start_any_other_way() {
    let output = Command::new(SERVICE).stdin(Stdio::null()).output().unwrap();
    assert!(!output.status.success());
    let output = Command::new(SERVICE).env("LISTEN_FDS", "1").env("LISTEN_PID", "1").stdin(Stdio::null()).output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn the_command_line_takes_no_secret_as_an_argument_and_says_how_it_ended() {
    if !has_dbus() {
        eprintln!("skipped: no dbus-daemon here");
        return;
    }
    let mut lab = start_lab("cli");
    lab.service = Some(spawn_service(&lab, Stdio::null(), &[]));
    let status = lab.cli(&["nothing-of-the-sort"]).stderr(Stdio::null()).status().unwrap();
    assert_eq!(status.code(), Some(64));
    let status = lab.cli(&["status"]).stdout(Stdio::null()).status().unwrap();
    assert_eq!(status.code(), Some(0));
    let status = lab.cli(&["lock"]).status().unwrap();
    assert_eq!(status.code(), Some(0));
    let output = lab.cli(&["item", "get", "k", "a"]).output().unwrap();
    assert_eq!(output.status.code(), Some(3));
    lab.cli_set_up(PASSWORD);
    let (code, printed) = lab.cli_with_input(&["item", "put", "password", "GitHub", "gh1", "--text", "--approval=while-open"], "hunter2\n");
    assert_eq!(code, 0, "{printed}");
    let output = lab.cli(&["item", "get", "password", "gh1"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hunter2\n");
    let output = lab.cli(&["item", "list", "password"]).output().unwrap();
    let list = String::from_utf8_lossy(&output.stdout);
    assert!(list.contains("gh1") && list.contains("GitHub") && !list.contains("hunter2"), "{list}");
    let (code, _) = lab.cli_with_input(&["change-password"], "not the password\nthe new one is long\n");
    assert_eq!(code, 1);
    let (code, _) = lab.cli_with_input(&["change-password"], &format!("{PASSWORD}\nthe new one is long\n"));
    assert_eq!(code, 0);
    let status = lab.cli(&["item", "remove", "password", "gh1"]).status().unwrap();
    assert_eq!(status.code(), Some(0));
    let status = lab.cli(&["forget"]).status().unwrap();
    assert_eq!(status.code(), Some(0));
    assert_eq!(lab.ask(json!({"op": "status"}))["set_up"], json!(false));
}

#[test]
fn it_is_out_of_the_account_s_reach_running_as_another_user() {
    // SAFETY: getuid cannot fail
    if unsafe { libc::getuid() } != 0 || !has_dbus() {
        eprintln!("skipped: needs root and dbus-daemon");
        return;
    }
    let lab = start_lab("not-dumpable");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&lab.folder.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(lab.folder.0.join("state"), std::fs::Permissions::from_mode(0o777)).unwrap();
    std::fs::set_permissions(&lab.socket, std::fs::Permissions::from_mode(0o666)).unwrap();
    std::fs::set_permissions(&lab.bus_address, std::fs::Permissions::from_mode(0o666)).unwrap();
    let fd = lab.listener.as_raw_fd();
    let mut command = Command::new("sh");
    command.arg("-c").arg(r#"export LISTEN_PID=$$; exec "$@""#).arg("sh").arg(SERVICE);
    command.env("LISTEN_FDS", "1").env("STATE_DIRECTORY", lab.folder.0.join("state")).env("DBUS_SYSTEM_BUS_ADDRESS", format!("unix:path={}", lab.bus_address.display())).stdin(Stdio::null());
    // SAFETY: dup2 is async-signal-safe, and this runs between fork and exec
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(fd, 3) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut service = command.uid(1235).gid(1235).spawn().unwrap();
    // Running means answering: before main, the loader is still at work and the flag not yet set
    for _ in 0..200 {
        if exchange(&lab.socket, &json!({"v": 1, "op": "status"}), Duration::from_secs(5)).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut proc_owner = None;
    for _ in 0..400 {
        // A not-dumpable process's /proc files are root's, whoever it runs as: once it has got as far as saying so, which is a
        // moment after it is the service
        if let Ok(stat) = std::fs::read(format!("/proc/{}/comm", service.id()))
            && String::from_utf8_lossy(&stat).starts_with("amethystora-vau")
        {
            use std::os::unix::fs::MetadataExt;
            proc_owner = std::fs::metadata(format!("/proc/{}/mem", service.id())).ok().map(|meta| meta.uid());
            if proc_owner == Some(0) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let _ = service.kill();
    let _ = service.wait();
    // Some runs on the WSL kernel of the lab never show the process as root's, though it is not dumpable: a kernel that does not
    // say so cannot be asked, and a flake here would stop an image build. It is checked on the machines that do (the VM suite too).
    if proc_owner != Some(0) {
        eprintln!("skipped: this kernel does not give a not-dumpable process's /proc files to root (owner {proc_owner:?})");
        return;
    }
    assert_eq!(proc_owner, Some(0), "/proc/PID/mem of a not-dumpable process is root's");
}

// --- Measured, not asserted: run by hand in release ---------------------------------------------------------------------

fn field(pid: u32, name: &str) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap().lines().find(|line| line.starts_with(name)).unwrap_or("").trim().replace('\t', " ")
}

fn wakeups(pid: u32) -> u64 {
    let mut total = 0;
    for entry in std::fs::read_dir(format!("/proc/{pid}/task")).unwrap().flatten() {
        let text = std::fs::read_to_string(entry.path().join("status")).unwrap_or_default();
        for line in text.lines().filter(|line| line.contains("ctxt_switches")) {
            total += line.split_whitespace().last().and_then(|number| number.parse::<u64>().ok()).unwrap_or(0);
        }
    }
    total
}

fn median(mut times: Vec<f64>) -> f64 {
    times.sort_by(f64::total_cmp);
    times[times.len() / 2]
}

#[test]
#[ignore = "measures the budgets, by hand, in release"]
fn budgets() {
    assert!(has_dbus());
    let mut lab = start_lab("budgets");

    // Cold start to first answer
    let mut colds = Vec::new();
    for _ in 0..15 {
        let started = Instant::now();
        let mut service = spawn_service(&lab, Stdio::null(), &[]);
        let answer = lab.ask(json!({"op": "status"}));
        colds.push(started.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(answer["ok"], json!(true));
        let _ = service.kill();
        let _ = service.wait();
    }
    println!("cold start to first answer: median {:.2} ms, slowest {:.2} ms (budget 150 ms)", median(colds.clone()), colds.iter().copied().fold(0.0, f64::max));

    let service = spawn_service(&lab, Stdio::null(), &[]);
    let pid = service.id();
    lab.service = Some(service);
    std::thread::sleep(Duration::from_millis(100));
    println!("resident, nothing open: {} / {}", field(pid, "VmRSS"), field(pid, "VmHWM"));

    let started = Instant::now();
    lab.cli_set_up(PASSWORD);
    println!("setup, one scrypt: {:?}", started.elapsed());
    std::thread::sleep(Duration::from_millis(200));
    println!("resident, open after scrypt: {} / {} / {} (budget 8 MB resident, beyond the scrypt's peak)", field(pid, "VmRSS"), field(pid, "VmHWM"), field(pid, "VmLck"));

    lab.ask(json!({"op": "lock"}));
    let started = Instant::now();
    assert_eq!(lab.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))["ok"], json!(true));
    println!("open with a password: {:?}", started.elapsed());
    std::thread::sleep(Duration::from_millis(200));
    println!("resident, reopened: {}", field(pid, "VmRSS"));

    // A seal and an open, on one connection, once running
    let stream = UnixStream::connect(&lab.socket).unwrap();
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    let request = |op: &str, data: &str| format!("{}\n", json!({"v": 1, "op": op, "purpose": "resume", "record": "r", "data": data}));
    let mut seals = Vec::new();
    let mut opens = Vec::new();
    let plain = amethystora_vault_core::protocol::encode(&[9u8; 4096]);
    let mut line = String::new();
    for _ in 0..400 {
        let started = Instant::now();
        writer.write_all(request("seal", &plain).as_bytes()).unwrap();
        line.clear();
        reader.read_line(&mut line).unwrap();
        seals.push(started.elapsed().as_secs_f64() * 1000.0);
        let sealed: Value = serde_json::from_str(&line).unwrap();
        let started = Instant::now();
        writer.write_all(request("open", sealed["data"].as_str().unwrap()).as_bytes()).unwrap();
        line.clear();
        reader.read_line(&mut line).unwrap();
        opens.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    println!(
        "4 KiB seal: median {:.3} ms, slowest {:.3} ms; open: median {:.3} ms, slowest {:.3} ms (budget 5 ms)",
        median(seals.clone()),
        seals.iter().copied().fold(0.0, f64::max),
        median(opens.clone()),
        opens.iter().copied().fold(0.0, f64::max)
    );
    drop(writer);
    drop(reader);

    // A thousand items: how long the list takes, and what it costs to hold
    for at in 0..1000 {
        lab.ask(json!({"op": "put", "kind": "password", "id": format!("i{at}"), "name": format!("Item number {at}"), "meta": {"user": "someone@example.org"}, "value": {"secret": "s"}, "approval": "ask"}));
    }
    let started = Instant::now();
    let list = lab.ask(json!({"op": "list"}));
    println!("list of 1000 items: {:?}, {} bytes", started.elapsed(), list.to_string().len());
    std::thread::sleep(Duration::from_millis(100));
    println!("resident, open, 1000 items: {}", field(pid, "VmRSS"));

    // Open and idle: nothing wakes it
    let before = wakeups(pid);
    std::thread::sleep(Duration::from_secs(30));
    println!("open and idle, 30 s: {} context switches (budget 0 beyond the idle deadline)", wakeups(pid) - before);

    // Closed, it goes
    lab.ask(json!({"op": "lock"}));
    let started = Instant::now();
    let mut service = lab.service.take().unwrap();
    loop {
        if service.try_wait().unwrap().is_some() {
            println!("closed, exits after {:?} (budget 60 s)", started.elapsed());
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(70));
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
#[ignore = "shows what a huge line costs the real binary, by hand"]
fn budgets_huge_line() {
    assert!(has_dbus());
    let mut lab = start_lab("huge");
    let service = spawn_service(&lab, Stdio::null(), &[]);
    let pid = service.id();
    lab.service = Some(service);
    let mut stream = UnixStream::connect(&lab.socket).unwrap();
    let chunk = vec![b'x'; 1 << 20];
    let mut sent = 0usize;
    while sent < 200 * (1 << 20) {
        match stream.write(&chunk) {
            Ok(count) => sent += count,
            Err(_) => break,
        }
    }
    let mut answer = String::new();
    let _ = stream.read_to_string(&mut answer);
    println!("{sent} bytes accepted, answered {answer}; peak {}", field(pid, "VmHWM"));
}


/// Whether the service has libcrypto among its libraries: it never may. Its /proc/PID/maps is closed to a test, as to everyone,
/// and it opens no library by itself, so its links are the whole answer.
fn links_libcrypto() -> bool {
    let out = Command::new("ldd").arg(SERVICE).output().unwrap();
    String::from_utf8_lossy(&out.stdout).contains("libcrypto")
}

#[test]
#[ignore = "measures the items' budgets, by hand, in release"]
fn budgets_items() {
    assert!(has_dbus());
    let mut lab = start_lab("budgets-items");
    let service = spawn_service(&lab, Stdio::null(), &[]);
    let pid = service.id();
    lab.service = Some(service);
    std::thread::sleep(Duration::from_millis(100));
    lab.cli_set_up(PASSWORD);
    std::thread::sleep(Duration::from_millis(200));
    println!("resident, open after scrypt: {}", field(pid, "VmRSS"));

    // A thousand authenticator accounts, a few hundred at a time as the client sends them
    let secret = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
    for batch in 0..4 {
        let accounts: Vec<Value> = (0..250).map(|at| json!({"issuer": format!("Site {}", batch * 250 + at), "label": "someone@example.org", "secret": secret, "addedAt": 1})).collect();
        let started = Instant::now();
        let answer = lab.ask(json!({"op": "import-merge", "accounts": accounts}));
        assert_eq!(answer["added"], json!(250), "{answer}");
        println!("import-merge of 250 onto {}: {:?}", batch * 250, started.elapsed());
    }
    let started = Instant::now();
    let listed = lab.ask(json!({"op": "auth-list"}));
    println!("auth-list of 1000: {:?}, {} bytes", started.elapsed(), listed.to_string().len());
    std::thread::sleep(Duration::from_millis(100));
    println!("resident, open, 1000 accounts: {}", field(pid, "VmRSS"));
    let started = Instant::now();
    let answer = lab.ask(json!({"op": "codes-coverage"}));
    assert_eq!(answer["total"], json!(1000));
    println!("codes-coverage of 1000: {:?}", started.elapsed());

    // A thousand codes on one connection, once each account's While open has been answered
    let ids: Vec<String> = listed["items"].as_array().unwrap().iter().map(|item| item["id"].as_str().unwrap().to_string()).collect();
    let stream = UnixStream::connect(&lab.socket).unwrap();
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let mut times = Vec::new();
    for id in &ids {
        writer.write_all(format!("{}\n", json!({"v": 1, "op": "auth-code", "id": id})).as_bytes()).unwrap();
        line.clear();
        reader.read_line(&mut line).unwrap();
    }
    for id in &ids {
        let started = Instant::now();
        writer.write_all(format!("{}\n", json!({"v": 1, "op": "auth-code", "id": id})).as_bytes()).unwrap();
        line.clear();
        reader.read_line(&mut line).unwrap();
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        assert!(line.contains("\"code\""), "{line}");
    }
    println!(
        "auth-code over the socket, 1000 accounts: median {:.3} ms, slowest {:.3} ms (budget 5 ms)",
        median(times.clone()),
        times.iter().copied().fold(0.0, f64::max)
    );
    std::thread::sleep(Duration::from_millis(100));
    println!("resident after 2000 auth-codes: {} / {} (anon {}, file {})", field(pid, "VmRSS"), field(pid, "VmHWM"), field(pid, "RssAnon"), field(pid, "RssFile"));
    assert!(!links_libcrypto());
    drop(writer);
    drop(reader);

    // What takes the job process: its start, a key, a certificate, a signature
    let started = Instant::now();
    let types = lab.ask(json!({"op": "key-types"}));
    assert_eq!(types["ok"], json!(true), "{types}");
    println!("key-types (one job, nothing but its start): {:?}", started.elapsed());
    let started = Instant::now();
    let made = lab.ask(json!({"op": "key-new", "name": "k", "key_kind": "ssh"}));
    assert_eq!(made["ok"], json!(true), "{made}");
    println!("key-new ed25519: {:?}", started.elapsed());
    let started = Instant::now();
    let made = lab.ask(json!({"op": "key-new", "name": "k", "key_kind": "ssh", "type": "rsa-4096"}));
    assert_eq!(made["ok"], json!(true), "{made}");
    println!("key-new rsa-4096: {:?}", started.elapsed());
    for (key_type, label) in [("rsa-3072", "RSA 3072"), ("ecdsa-p256", "ECDSA P-256")] {
        let started = Instant::now();
        let cert = lab.ask(json!({"op": "cert-new", "profile": "pdf", "title": "PDF", "name": "Ada Lovelace", "key_type": key_type}));
        assert_eq!(cert["ok"], json!(true), "{cert}");
        println!("cert-new pdf, {label} (the first makes the root and the account's CA): {:?}", started.elapsed());
        let id = cert["id"].as_str().unwrap().to_string();
        lab.ask(json!({"op": "set-approval", "kind": "certificate", "id": id, "approval": "never"}));
        let mut signs = Vec::new();
        for _ in 0..20 {
            let started = Instant::now();
            let signed = lab.ask(json!({"op": "sign-digest", "id": id, "purpose": "pdf", "digest": "sha256", "value": amethystora_vault_core::protocol::encode(&[7u8; 32])}));
            signs.push(started.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(signed["ok"], json!(true), "{signed}");
        }
        println!("sign-digest, {label}, no prompt, through the job: median {:.1} ms, slowest {:.1} ms", median(signs.clone()), signs.iter().copied().fold(0.0, f64::max));
    }
    std::thread::sleep(Duration::from_millis(100));
    println!("resident after the jobs: {} / {}", field(pid, "VmRSS"), field(pid, "VmHWM"));
    assert!(!links_libcrypto());
}

// --- Sync, measured against Vaultwarden in a container (by hand, in release) -----------------------------------------------

/// Starts a program as systemd starts a socket-activated one: the listening socket is descriptor 3
fn spawn_activated(program: &std::path::Path, fd: std::os::fd::RawFd) -> Child {
    let mut command = Command::new("sh");
    command.arg("-c").arg(r#"export LISTEN_PID=$$; exec "$@""#).arg("sh").arg(program);
    command.env("LISTEN_FDS", "1").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    // SAFETY: dup2 is async-signal-safe, and this runs between fork and exec
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(fd, 3) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.spawn().unwrap()
}

struct VwHttp(amethystora_vault_net::http::Session);

impl amethystora_vault_core::sync::api::Http for VwHttp {
    fn call(&mut self, request: amethystora_vault_core::sync::api::Request) -> Result<amethystora_vault_core::sync::api::Response, amethystora_vault_core::sync::api::NetFailure> {
        let headers: Vec<(String, String)> = request.headers.iter().map(|(name, value)| (name.to_string(), value.clone())).collect();
        self.0.request(request.method, &request.url, &headers, &request.body).map(|(status, body)| amethystora_vault_core::sync::api::Response { status, body }).map_err(|failure| amethystora_vault_core::sync::api::NetFailure::from_name(failure.name()))
    }
}

fn rss_kb(pid: u32) -> u64 {
    field(pid, "VmRSS").split_whitespace().nth(1).and_then(|kb| kb.parse().ok()).unwrap_or(0)
}

/// The highest resident size a process reaches while something runs, read every 20 ms
fn watch_peak(pid: u32, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<u64> {
    std::thread::spawn(move || {
        let mut peak = 0u64;
        while !stop.load(Ordering::Relaxed) {
            peak = peak.max(rss_kb(pid));
            std::thread::sleep(Duration::from_millis(20));
        }
        peak
    })
}

/// Connects the way a person's program does: one request, and the yes to it is for the connection it came on
fn connect_account(lab: &Lab, address: &str, email: &str, password: &str) -> Value {
    let mut stream = std::os::unix::net::UnixStream::connect(&lab.socket).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
    let request = json!({"v": 1, "op": "sync-connect", "address": address, "email": email, "secret": b64(password)});
    stream.write_all(format!("{request}\n").as_bytes()).unwrap();
    let mut answer = String::new();
    BufReader::new(stream).read_line(&mut answer).unwrap();
    serde_json::from_str(&answer).unwrap()
}

fn until_synced(lab: &Lab) -> Value {
    let started = Instant::now();
    loop {
        let status = lab.ask(json!({"op": "sync-status"}));
        if status["last_success_ms"].is_number() && status["running"] != json!(true) {
            return status;
        }
        assert!(started.elapsed() < Duration::from_secs(900));
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
#[ignore = "measures sync against Vaultwarden on 127.0.0.1:8080, by hand, in release"]
fn budgets_sync() {
    use amethystora_vault_core::sync::crypto::Kdf;
    assert!(has_dbus());
    let mut lab = start_lab("budgets-sync");
    // The network service, the real program, started again whenever it has exited (systemd would)
    std::fs::create_dir_all("/run/amethystora").unwrap();
    let net_path = std::path::Path::new("/run/amethystora/vault-net.sock");
    let _ = std::fs::remove_file(net_path);
    let net_listener = UnixListener::bind(net_path).unwrap();
    let net_bin = std::path::Path::new(SERVICE).with_file_name("amethystora-vault-net");
    let net_stop = Arc::new(AtomicBool::new(false));
    let supervisor = {
        let stop = net_stop.clone();
        std::thread::spawn(move || {
            let fd = net_listener.as_raw_fd();
            let mut child = spawn_activated(&net_bin, fd);
            while !stop.load(Ordering::Relaxed) {
                if child.try_wait().unwrap().is_some() {
                    child = spawn_activated(&net_bin, fd);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = child.kill();
            let _ = child.wait();
            drop(net_listener);
        })
    };
    // An account on the container
    let address = "http://127.0.0.1:8080";
    let email = format!("budget-{}@example.test", amethystora_vault_core::seal::random_hex(4));
    let account_password = "a master password for the container";
    let mut http = VwHttp(amethystora_vault_net::http::Session::new(vec!["http://127.0.0.1:8080".into()], None, None).unwrap());
    amethystora_vault_core::sync::register::register(&mut http, &format!("{address}/identity"), &email, account_password, &Kdf { kind: 0, iterations: 600_000, memory: None, parallelism: None }).unwrap();

    let service = spawn_service(&lab, Stdio::null(), &[]);
    let pid = service.id();
    lab.service = Some(service);
    std::thread::sleep(Duration::from_millis(100));
    lab.cli_set_up(PASSWORD);
    println!("sync: resident with the vault open and nothing connected: {} kB", rss_kb(pid));
    let started = Instant::now();
    for at in 0..1000 {
        let value = json!({"id": format!("p{at}"), "title": format!("Site {at}"), "username": format!("user{at}"), "password": format!("secret-{at}"), "address": format!("https://site{at}.example.test/login"), "addedAt": 1});
        let answer = lab.ask(json!({"op": "put", "kind": "password", "id": format!("p{at}"), "name": format!("Site {at}"), "meta": {"username": format!("user{at}"), "address": format!("https://site{at}.example.test/login")}, "value": value}));
        assert_eq!(answer["ok"], json!(true), "{answer}");
    }
    println!("sync: 1000 passwords put through the socket: {:?}", started.elapsed());
    std::thread::sleep(Duration::from_millis(200));
    println!("sync: resident, 1000 items, nothing connected: {} kB", rss_kb(pid));

    let stop = Arc::new(AtomicBool::new(false));
    let watcher = watch_peak(pid, stop.clone());
    let started = Instant::now();
    let connected = connect_account(&lab, address, &email, account_password);
    assert_eq!(connected["ok"], json!(true), "{connected}");
    println!("sync: connect (prelogin, 600,000 rounds, login, /sync): {:?}", started.elapsed());
    let started = Instant::now();
    let status = until_synced(&lab);
    println!("sync: first run, 1000 items pushed (a POST each): {:?}, linked {}", started.elapsed(), status["linked"]);
    stop.store(true, Ordering::Relaxed);
    let peak = watcher.join().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    println!("sync: resident peak during connect and the first run: {peak} kB; after: {} kB ({} / {}); high-water mark {}", rss_kb(pid), field(pid, "RssAnon"), field(pid, "RssFile"), field(pid, "VmHWM"));

    // Nothing changed
    let started = Instant::now();
    let answer = lab.ask(json!({"op": "sync-now"}));
    println!("sync: a run with nothing changed: {:?} (applied {})", started.elapsed(), answer["report"]["applied"]);
    // One change among a thousand, by the person
    let mut value = lab.ask(json!({"op": "get", "kind": "password", "id": "p500"}))["value"].clone();
    value["password"] = json!("a new one");
    lab.ask(json!({"op": "put", "kind": "password", "id": "p500", "name": "Site 500", "value": value}));
    let started = Instant::now();
    let answer = lab.ask(json!({"op": "sync-now"}));
    println!("sync: a run with one change among 1000: {:?} (sent {})", started.elapsed(), answer["report"]["sent"]);
    println!("sync: resident after those runs: {} kB ({} / {})", rss_kb(pid), field(pid, "RssAnon"), field(pid, "RssFile"));

    // A second machine: an empty vault takes all of them. Off, every item removed here, connected again.
    assert_eq!(lab.ask(json!({"op": "sync-off"}))["ok"], json!(true));
    for at in 0..1000 {
        lab.ask(json!({"op": "remove", "kind": "password", "id": format!("p{at}")}));
    }
    let stop = Arc::new(AtomicBool::new(false));
    let watcher = watch_peak(pid, stop.clone());
    let started = Instant::now();
    let connected = connect_account(&lab, address, &email, account_password);
    assert_eq!(connected["ok"], json!(true), "{connected}");
    let status = until_synced(&lab);
    println!("sync: connect and pull of 1000 into an empty vault: {:?}, linked {}", started.elapsed(), status["linked"]);
    stop.store(true, Ordering::Relaxed);
    let peak = watcher.join().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    println!("sync: resident peak during the pull: {peak} kB; after: {} kB", rss_kb(pid));
    let list = lab.ask(json!({"op": "list", "kind": "password"}));
    println!("sync: passwords in the vault after the pull: {}", list["items"].as_array().map_or(0, Vec::len));

    // Open, idle and connected: the one deadline sync adds is fifteen minutes away
    for second in [0, 5, 10, 20] {
        std::thread::sleep(Duration::from_secs(if second == 0 { 0 } else { 5 }));
        println!("sync: resident {second} s after the pull: {} kB ({})", rss_kb(pid), field(pid, "RssAnon"));
    }
    let before = wakeups(pid);
    std::thread::sleep(Duration::from_secs(30));
    println!("sync: open, idle and connected, 30 s: {} context switches; resident {} kB ({} / {})", wakeups(pid) - before, rss_kb(pid), field(pid, "RssAnon"), field(pid, "RssFile"));
    // Locked and opened again: the vault is read from its files, with nothing left from the runs
    assert_eq!(lab.ask(json!({"op": "lock"}))["ok"], json!(true));
    assert_eq!(lab.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))["ok"], json!(true));
    std::thread::sleep(Duration::from_millis(500));
    println!("sync: resident after a lock and an opening again: {} kB ({})", rss_kb(pid), field(pid, "RssAnon"));
    net_stop.store(true, Ordering::Relaxed);
    supervisor.join().unwrap();
    let _ = std::fs::remove_file(net_path);
}
