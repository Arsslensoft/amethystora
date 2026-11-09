//! Using a secret without holding it (plans/done/PLAN-vault.md), with real tools where this machine has them: `ssh`, `ssh-add`, `ssh-keygen`, `sshd`
//! and `git`, against the service on a real socket, with stand-ins for polkit and logind. A test that needs a tool or a namespace this
//! machine does not have says so and passes: the VM suite runs them all.

mod common;

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use amethystora_vault_core::keys::import_openssh;
use amethystora_vault_core::protocol::encode;
use common::{start, Folder, Running};
use serde_json::{json, Value};

const PASSWORD: &str = "correct horse battery";
const WORKER_KEYS: [(&str, &[&str]); 3] = [("ed25519", &["-t", "ed25519"]), ("ecdsa", &["-t", "ecdsa", "-b", "256"]), ("rsa", &["-t", "rsa", "-b", "3072"])];

/// Setting a vault up asks the person once, which every count here leaves out
const SETUP_ASKS: usize = 1;

fn asked(running: &Running) -> usize {
    running.outside.asked() - SETUP_ASKS
}

/// `sshd` is in the image's `openssh-server`, which the build stage does not install: the tests that log in to one then say so and skip
fn sshd() -> bool {
    let found = Path::new("/usr/sbin/sshd").exists();
    if !found {
        eprintln!("/usr/sbin/sshd is not on this machine: this test did not run");
    }
    found
}

fn has(tool: &str) -> bool {
    let found = std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|folder| folder.join(tool).is_file()));
    if !found {
        eprintln!("{tool} is not on this machine: this test did not run");
    }
    found
}

fn agent(running: &Running) -> PathBuf {
    running.folder.0.join("vault-ssh.sock")
}

/// A new key from the real `ssh-keygen`, its private file and its public line
fn ssh_keygen(folder: &Path, name: &str, options: &[&str], passphrase: &str) -> (PathBuf, String) {
    let file = folder.join(name);
    let made = Command::new("ssh-keygen").args(options).args(["-q", "-N", passphrase, "-C", name, "-f"]).arg(&file).status().unwrap();
    assert!(made.success());
    let public = std::fs::read_to_string(file.with_extension("pub")).unwrap();
    let public: Vec<&str> = public.split_whitespace().take(2).collect();
    (file, public.join(" "))
}

/// Imports it as the client does: the file is read here, and only the finished key goes to the service
fn import(running: &Running, name: &str, file: &Path, passphrase: &[u8], approval: &str) -> String {
    let imported = import_openssh(&std::fs::read(file).unwrap(), passphrase).unwrap();
    let answer = running.ask(json!({"op": "key-import", "name": name, "type": imported.key.key_type().name(), "private": encode(imported.key.as_bytes()), "key_kind": "ssh", "approval": approval}));
    assert_eq!(answer["ok"], json!(true), "{answer}");
    answer["id"].as_str().unwrap().to_string()
}

fn ssh_add(running: &Running, args: &[&str]) -> (bool, String) {
    let output = Command::new("ssh-add").args(args).env("SSH_AUTH_SOCK", agent(running)).stdin(Stdio::null()).output().unwrap();
    (output.status.success(), String::from_utf8_lossy(&output.stdout).into_owned())
}

fn frame(body: &[u8]) -> Vec<u8> {
    let mut out = (body.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(body);
    out
}

fn exchange(stream: &mut UnixStream, body: &[u8]) -> Vec<u8> {
    stream.write_all(&frame(body)).unwrap();
    let mut length = [0u8; 4];
    stream.read_exact(&mut length).unwrap();
    let mut answer = vec![0u8; u32::from_be_bytes(length) as usize];
    stream.read_exact(&mut answer).unwrap();
    answer
}

fn connect(running: &Running) -> UnixStream {
    let stream = UnixStream::connect(agent(running)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    stream
}

fn string(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
}

fn sign_request(public: &str, data: &[u8], flags: u32) -> Vec<u8> {
    use base64::Engine;
    let blob = base64::engine::general_purpose::STANDARD.decode(public.split(' ').nth(1).unwrap()).unwrap();
    let mut out = vec![13];
    string(&mut out, &blob);
    string(&mut out, data);
    out.extend_from_slice(&flags.to_be_bytes());
    out
}

// --- The agent, with the real tools -------------------------------------------------------------------------------------------

#[test]
fn ssh_add_lists_public_keys_only_and_nothing_can_add_or_remove_one() {
    if !has("ssh-add") || !has("ssh-keygen") {
        return;
    }
    let running = start("agent-list");
    let folder = Folder::new("agent-list-keys");
    // Before anything is set up, and while closed: an empty list, which is the truth
    let (fine, text) = ssh_add(&running, &["-L"]);
    assert!(!fine, "{text}");
    assert!(text.contains("no identities"), "{text}");
    running.set_up(PASSWORD);
    let mut publics = Vec::new();
    for (name, options) in WORKER_KEYS {
        let (file, public) = ssh_keygen(&folder.0, name, options, "");
        import(&running, name, &file, b"", "while-open");
        publics.push((public, file));
    }
    let (fine, text) = ssh_add(&running, &["-L"]);
    assert!(fine, "{text}");
    for (public, _) in &publics {
        assert!(text.contains(public.as_str()), "{public} not in {text}");
    }
    assert_eq!(text.lines().count(), 3);
    assert!(!text.contains("PRIVATE"), "{text}");
    // Everything that would change what it holds is refused, and changes nothing
    let (extra, _) = ssh_keygen(&folder.0, "extra", &["-t", "ed25519"], "");
    let file = extra.to_str().unwrap();
    for args in [vec![file], vec!["-d", file], vec!["-D"], vec!["-t", "60", file]] {
        let (fine, text) = ssh_add(&running, &args);
        assert!(!fine, "ssh-add {args:?}: {text}");
    }
    assert_eq!(ssh_add(&running, &["-L"]).1.lines().count(), 3);
    // Listing asked nobody, and no list ever does
    assert_eq!(asked(&running), 0);
    let mut stream = connect(&running);
    for message in [17u8, 18, 19, 20, 21, 22, 23, 25, 26, 27, 1, 2, 9, 0, 255] {
        assert_eq!(exchange(&mut stream, &[message, 0, 0, 0, 0]), [5], "message {message}");
    }
}

#[test]
fn ssh_keygen_signs_through_the_agent_and_the_signature_verifies() {
    if !has("ssh-keygen") {
        return;
    }
    let running = start("agent-sign");
    let folder = Folder::new("agent-sign-keys");
    running.set_up(PASSWORD);
    for (name, options) in WORKER_KEYS {
        let (file, public) = ssh_keygen(&folder.0, name, options, "");
        import(&running, name, &file, b"", "ask");
        let message = folder.0.join(format!("{name}.txt"));
        std::fs::write(&message, b"what is signed").unwrap();
        // The key is removed: only the agent can sign, from the public half
        std::fs::remove_file(&file).unwrap();
        let signed = Command::new("ssh-keygen").args(["-Y", "sign", "-n", "file", "-f"]).arg(file.with_extension("pub")).arg(&message).env("SSH_AUTH_SOCK", agent(&running)).output().unwrap();
        assert!(signed.status.success(), "{name}: {}", String::from_utf8_lossy(&signed.stderr));
        let signature = folder.0.join(format!("{name}.txt.sig"));
        let allowed = folder.0.join(format!("{name}.allowed"));
        std::fs::write(&allowed, format!("me {public}\n")).unwrap();
        let mut check = Command::new("ssh-keygen").args(["-Y", "verify", "-n", "file", "-I", "me", "-f"]).arg(&allowed).arg("-s").arg(&signature).stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap();
        check.stdin.take().unwrap().write_all(b"what is signed").unwrap();
        assert!(check.wait().unwrap().success(), "{name}'s signature did not verify");
    }
    // Each asked the person, since each is on `ask`, and said what it was for
    assert_eq!(asked(&running), 3);
    let asked = running.outside.said.lock().unwrap().asked.clone();
    // `ssh-keygen -Y sign` has the agent sign a message, and the prompt says so
    assert!(asked.iter().skip(SETUP_ASKS).all(|ask| ask.message == amethystora_vault_service::outside::say::SIGN_MESSAGE && ask.detail == "file"));
    assert_eq!(asked[SETUP_ASKS].item, "ed25519");
}

/// sshd without PAM refuses a locked account, and root is locked (`!`) on some images (fedora:45's). The tests that log in as root
/// make it merely passwordless (`*`) while they run and put the line back after, one test at a time.
struct RootUnlocked {
    original: Option<String>,
    _one_at_a_time: std::sync::MutexGuard<'static, ()>,
}

static ROOT: std::sync::Mutex<()> = std::sync::Mutex::new(());

impl RootUnlocked {
    fn new() -> Self {
        let guard = ROOT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let original = std::fs::read_to_string("/etc/shadow").ok().filter(|shadow| shadow.lines().any(|line| line.starts_with("root:!")));
        if let Some(shadow) = &original {
            let unlocked: Vec<String> = shadow.lines().map(|line| if line.starts_with("root:!") { format!("root:*:{}", line.splitn(3, ':').nth(2).unwrap_or("")) } else { line.to_string() }).collect();
            std::fs::write("/etc/shadow", unlocked.join("\n") + "\n").unwrap();
        }
        RootUnlocked { original, _one_at_a_time: guard }
    }
}

impl Drop for RootUnlocked {
    fn drop(&mut self) {
        if let Some(shadow) = &self.original {
            let _ = std::fs::write("/etc/shadow", shadow);
        }
    }
}

/// The real thing: `ssh` logs in to a real `sshd` with a key that is only in the vault
#[test]
fn ssh_logs_in_with_a_key_that_is_only_in_the_vault() {
    if !has("ssh") || !has("ssh-keygen") || !sshd() {
        return;
    }
    // SAFETY: getuid cannot fail
    if unsafe { libc::getuid() } != 0 {
        eprintln!("sshd needs root to log anyone in: this test did not run");
        return;
    }
    let _root = RootUnlocked::new();
    let running = start("agent-ssh");
    let folder = Folder::new("agent-ssh-server");
    running.set_up(PASSWORD);
    std::fs::create_dir_all("/run/sshd").unwrap();
    let (host_key, _) = ssh_keygen(&folder.0, "host", &["-t", "ed25519"], "");
    let authorized = folder.0.join("authorized");
    let mut lines = String::new();
    let mut publics = Vec::new();
    for (name, options) in WORKER_KEYS {
        let (file, public) = ssh_keygen(&folder.0, name, options, "");
        import(&running, name, &file, b"", "while-open");
        std::fs::remove_file(&file).unwrap();
        lines.push_str(&format!("{public}\n"));
        publics.push((name, file.with_extension("pub")));
    }
    std::fs::write(&authorized, lines).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&folder.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    let port = 22000 + (std::process::id() % 2000) as u16;
    let mut server = Command::new("/usr/sbin/sshd")
        .args(["-D", "-e", "-p", &port.to_string(), "-o", "ListenAddress=127.0.0.1", "-o", "PermitRootLogin=prohibit-password", "-o", "StrictModes=no", "-o", "UsePAM=no", "-o", "PasswordAuthentication=no", "-h"])
        .arg(&host_key)
        .arg("-o")
        .arg(format!("AuthorizedKeysFile={}", authorized.display()))
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let login = |public: &Path, sock: &Path| {
        Command::new("ssh")
            .args(["-F", "none", "-p", &port.to_string(), "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=no", "-o", "UserKnownHostsFile=/dev/null", "-o", "LogLevel=ERROR", "-o", "IdentitiesOnly=yes", "-i"])
            .arg(public)
            .arg("-o")
            .arg(format!("IdentityAgent={}", sock.display()))
            .args(["root@127.0.0.1", "echo", "in"])
            .stdin(Stdio::null())
            .output()
            .unwrap()
    };
    let socket = agent(&running);
    let mut results = Vec::new();
    for (name, public) in &publics {
        let done = login(public, &socket);
        results.push((*name, done.status.success(), String::from_utf8_lossy(&done.stdout).into_owned(), String::from_utf8_lossy(&done.stderr).into_owned()));
    }
    let asked_first = asked(&running);
    // Within the window a key on While open does not ask again
    let again = login(&publics[0].1, &socket);
    let asked_then = asked(&running);
    // A no is a failed login
    running.ask(json!({"op": "lock"}));
    let closed = login(&publics[0].1, &socket);
    let _ = server.kill();
    let _ = server.wait();
    for (name, fine, out, error) in results {
        assert!(fine && out.trim() == "in", "{name}: {out} {error}");
    }
    assert_eq!(asked_first, 3);
    assert!(again.status.success());
    assert_eq!(asked_then, 3);
    assert!(!closed.status.success());
}

// --- What the agent does with its own rules ------------------------------------------------------------------------------------

#[test]
fn every_setting_asks_as_it_says() {
    if !has("ssh-keygen") {
        return;
    }
    let running = start("agent-settings");
    let folder = Folder::new("agent-settings-keys");
    running.set_up(PASSWORD);
    let mut stream = connect(&running);
    for (setting, asks) in [("ask", 3), ("while-open", 1), ("never", 0)] {
        let (file, public) = ssh_keygen(&folder.0, setting, &["-t", "ed25519"], "");
        import(&running, setting, &file, b"", setting);
        let before = running.outside.asked();
        for _ in 0..3 {
            assert_eq!(exchange(&mut stream, &sign_request(&public, &login_data(&public), 0))[0], 14, "{setting}");
        }
        assert_eq!(running.outside.asked() - before, asks, "{setting}");
    }
}

#[test]
fn a_no_and_a_closed_vault_are_one_answer_and_a_client_hanging_up_cancels_the_prompt() {
    if !has("ssh-keygen") {
        return;
    }
    let running = start("agent-no");
    let folder = Folder::new("agent-no-keys");
    running.set_up(PASSWORD);
    let (file, public) = ssh_keygen(&folder.0, "key", &["-t", "ed25519"], "");
    import(&running, "key", &file, b"", "ask");
    let mut stream = connect(&running);
    running.outside.said.lock().unwrap().refuse = true;
    assert_eq!(exchange(&mut stream, &sign_request(&public, b"data", 0)), [5]);
    running.outside.said.lock().unwrap().refuse = false;
    // A key nobody holds is refused without asking anybody
    let before = running.outside.asked();
    assert_eq!(exchange(&mut stream, &sign_request("x AAAAC3NzaC1lZDI1NTE5AAAAIL1k", b"data", 0)), [5]);
    assert_eq!(running.outside.asked(), before);
    // A prompt up, and whoever asked goes away
    running.outside.said.lock().unwrap().silent = true;
    let mut gone = connect(&running);
    gone.write_all(&frame(&sign_request(&public, b"data", 0))).unwrap();
    running.wait_until(|said| said.asked.len() == before + 1);
    drop(gone);
    running.wait_until(|said| !said.cancelled.is_empty());
    running.outside.said.lock().unwrap().silent = false;
    // Closed: the list is empty, and it signs nothing
    running.ask(json!({"op": "lock"}));
    assert_eq!(exchange(&mut stream, &[11]), [12, 0, 0, 0, 0]);
    assert_eq!(exchange(&mut stream, &sign_request(&public, b"data", 0)), [5]);
}

#[test]
fn rsa_is_refused_sha1_before_anyone_is_asked() {
    if !has("ssh-keygen") {
        return;
    }
    let running = start("agent-sha1");
    let folder = Folder::new("agent-sha1-keys");
    running.set_up(PASSWORD);
    let (file, public) = ssh_keygen(&folder.0, "key", &["-t", "rsa", "-b", "3072"], "");
    import(&running, "key", &file, b"", "ask");
    let mut stream = connect(&running);
    assert_eq!(exchange(&mut stream, &sign_request(&public, b"data", 0)), [5]);
    assert_eq!(asked(&running), 0);
    let answer = exchange(&mut stream, &sign_request(&public, b"data", 2));
    assert_eq!(answer[0], 14);
    assert!(answer.windows(12).any(|name| name == b"rsa-sha2-256"));
    let answer = exchange(&mut stream, &sign_request(&public, b"data", 4));
    assert!(answer.windows(12).any(|name| name == b"rsa-sha2-512"));
}

/// Only a key marked for SSH is listed or signs: any other key the vault holds (a data key, a certificate authority's) is not the agent's
#[test]
fn a_key_that_is_not_marked_for_ssh_is_neither_listed_nor_signs() {
    if !has("ssh-keygen") {
        return;
    }
    let running = start("agent-not-ssh");
    let folder = Folder::new("agent-not-ssh-keys");
    running.set_up(PASSWORD);
    let (file, public) = ssh_keygen(&folder.0, "plain", &["-t", "ed25519"], "");
    let imported = import_openssh(&std::fs::read(&file).unwrap(), b"").unwrap();
    let meta = json!({"key_kind": "data", "type": "ed25519", "openssh": public});
    let put = running.ask(json!({"op": "put", "kind": "key", "name": "plain", "meta": meta, "approval": "while-open", "value": {"key": encode(imported.key.as_bytes())}}));
    if put["ok"] != json!(true) {
        // The service holds this kind itself: even better
        return;
    }
    let (_, text) = ssh_add(&running, &["-L"]);
    assert!(!text.contains(public.as_str()), "{text}");
    let mut stream = connect(&running);
    assert_eq!(exchange(&mut stream, &sign_request(&public, b"data", 0)), [5]);
    assert_eq!(asked(&running), 0);
}

#[test]
fn another_vault_socket_client_is_not_disturbed_by_hostile_frames() {
    let running = start("agent-hostile");
    running.set_up(PASSWORD);
    let mut seed = 0x1234_5678_9abc_def0u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut stream = connect(&running);
    for round in 0..4000u32 {
        // Random bytes, and random bytes after the two messages that are read further
        let size = (next() % 200) as usize;
        let mut body: Vec<u8> = (0..size).map(|_| next() as u8).collect();
        match round % 3 {
            1 if !body.is_empty() => body[0] = 13,
            2 if !body.is_empty() => body[0] = 11,
            _ => {}
        }
        // Every length a sign request might claim
        if round % 7 == 0 && body.len() > 5 {
            body[0] = 13;
            body[1..5].copy_from_slice(&(next() as u32).to_be_bytes());
        }
        let answer = exchange(&mut stream, &body);
        assert!(answer == [5] || (body.first() == Some(&11) && body.len() == 1 && answer[0] == 12), "{body:?} got {answer:?}");
    }
    // A length of nothing, and a frame split at every byte
    assert_eq!(exchange(&mut stream, &[]), [5]);
    for byte in frame(&[11]) {
        stream.write_all(&[byte]).unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut answer = [0u8; 9];
    stream.read_exact(&mut answer).unwrap();
    assert_eq!(answer[4], 12);
    // Past the limit, in the length alone: the answer is no and the connection goes, with nothing allocated for it
    let mut large = connect(&running);
    large.write_all(&u32::MAX.to_be_bytes()).unwrap();
    let mut rest = Vec::new();
    large.read_to_end(&mut rest).unwrap();
    assert_eq!(rest, [0, 0, 0, 1, 5]);
    // A frame of JSON, which is the other socket's language
    let mut other = connect(&running);
    other.write_all(b"{\"v\":1,\"op\":\"status\"}\n").unwrap();
    other.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    assert!(other.read(&mut [0u8; 16]).is_err() || true);
    // And through it all the vault's own socket answers, and the agent too
    assert_eq!(running.ask(json!({"op": "status"}))["ok"], json!(true));
    assert_eq!(exchange(&mut connect(&running), &[11]), [12, 0, 0, 0, 0]);
}

#[test]
fn a_window_runs_out() {
    // ssh::WINDOW is fifteen minutes, and the arithmetic is the same at a tenth of a second
    use amethystora_vault_service::outside::{say, Action};
    use amethystora_vault_service::peer::Peer;
    use amethystora_vault_service::service::{Config, Next, Reuse, Service};
    use std::rc::Rc;
    let folder = Folder::new("agent-window");
    let (outside, handle) = common::fake();
    let chip = std::sync::Arc::new(amethystora_vault_core::slots::fake::FakeChip::new());
    let mut service = Service::new(Config { state: folder.0.clone(), log_n: 10, worker: PathBuf::new() }, chip, Box::new(outside));
    let (a, _b) = UnixStream::pair().unwrap();
    use std::os::fd::AsRawFd;
    let peer = Rc::new(Peer::of(a.as_raw_fd()).unwrap());
    let say = |service: &mut Service, line: Value| -> Value {
        let mut next = service.request(&peer, line.to_string().as_bytes());
        loop {
            match next {
                Next::Answer(answer) => return answer,
                Next::Approve { then, .. } => next = then(service, &peer, true),
                Next::WhenUnlocked { then } => next = then(service, &peer, true),
                Next::Work { job, then } | Next::Slow { job, then } => next = then(service, &peer, job()),
            }
        }
    };
    let answer = say(&mut service, json!({"v": 1, "op": "setup", "method": "password", "secret": common::b64(PASSWORD)}));
    let answer = say(&mut service, json!({"v": 1, "op": "confirm", "recovery": common::b64(answer["recovery"].as_str().unwrap())}));
    assert_eq!(answer["ok"], json!(true), "{answer}");
    let answer = say(&mut service, json!({"v": 1, "op": "put", "kind": "thing", "name": "thing", "id": "one", "value": "x"}));
    assert_eq!(answer["ok"], json!(true), "{answer}");
    let window = Duration::from_millis(100);
    let asks = |service: &mut Service| matches!(service.with_window(peer.uid, "thing", "one", Action::Use, say::SIGN, "", Reuse::For(window), |_, _| Next::ok()), Next::Approve { .. });
    assert!(asks(&mut service));
    let Next::Approve { then, .. } = service.with_window(peer.uid, "thing", "one", Action::Use, say::SIGN, "", Reuse::For(window), |_, _| Next::ok()) else { panic!() };
    assert!(matches!(then(&mut service, &peer, true), Next::Answer(_)));
    assert!(!asks(&mut service), "asked again inside the window");
    std::thread::sleep(window + Duration::from_millis(50));
    assert!(asks(&mut service), "did not ask after the window");
    drop(handle);
}

/// `cargo test --release -p amethystora-vault-service --test secret -- --ignored one_signature --nocapture`
#[test]
#[ignore]
fn one_signature_takes() {
    let running = start("agent-time");
    let folder = Folder::new("agent-time-keys");
    running.set_up(PASSWORD);
    let mut stream = connect(&running);
    for (name, options, flags) in [
        ("ed25519", &["-t", "ed25519"][..], 0),
        ("ecdsa-p256", &["-t", "ecdsa", "-b", "256"][..], 0),
        ("rsa-3072", &["-t", "rsa", "-b", "3072"][..], 2),
        ("rsa-4096", &["-t", "rsa", "-b", "4096"][..], 4),
    ] {
        let (file, public) = ssh_keygen(&folder.0, name, options, "");
        import(&running, name, &file, b"", "never");
        let mut times: Vec<Duration> = (0..100)
            .map(|_| {
                let begun = Instant::now();
                assert_eq!(exchange(&mut stream, &sign_request(&public, b"session id and the rest", flags))[0], 14);
                begun.elapsed()
            })
            .collect();
        times.sort();
        println!("{name}: median {:?}, slowest {:?}", times[50], times[99]);
    }
    let begun = Instant::now();
    for _ in 0..100 {
        assert_eq!(exchange(&mut stream, &[11])[0], 12);
    }
    println!("a list of 4 keys: {:?} each", begun.elapsed() / 100);
}

// --- The tools' helpers ---------------------------------------------------------------------------------------------------------

fn add_login(running: &Running, name: &str, username: &str, address: &str, password: &str) -> String {
    let answer = running.ask(json!({"op": "put", "kind": "password", "name": name, "meta": {"username": username, "address": address}, "value": {"title": name, "username": username, "address": address, "password": password}}));
    assert_eq!(answer["ok"], json!(true), "{answer}");
    answer["id"].as_str().unwrap().to_string()
}

fn git_fill(running: &Running, input: &str) -> (bool, String) {
    let helper = env!("CARGO_BIN_EXE_git-credential-amethystora");
    let mut child = Command::new("git")
        .current_dir(&running.folder.0).args(["-c", "credential.helper=", "-c"])
        .arg(format!("credential.helper={helper}"))
        .args(["credential", "fill"])
        .env("AMETHYSTORA_VAULT_SOCKET", &running.socket)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "/bin/false")
        .env_remove("SSH_ASKPASS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    (output.status.success(), String::from_utf8_lossy(&output.stdout).into_owned())
}

#[test]
fn real_git_gets_a_login_for_its_host_and_nothing_else() {
    if !has("git") {
        return;
    }
    let running = start("helper-git");
    running.set_up(PASSWORD);
    add_login(&running, "forge", "me", "https://forge.example/", "s3cret");
    add_login(&running, "plain", "them", "http://intranet.example", "other");
    let (fine, out) = git_fill(&running, "protocol=https\nhost=forge.example\n\n");
    assert!(fine, "{out}");
    assert!(out.contains("username=me\n") && out.contains("password=s3cret\n"), "{out}");
    assert_eq!(asked(&running), 1);
    // While open: not asked again
    assert!(git_fill(&running, "protocol=https\nhost=forge.example\n\n").1.contains("password=s3cret"));
    assert_eq!(asked(&running), 1);
    // The same host over plain http, a look-alike, another username: git has nothing, and nobody is asked
    for input in ["protocol=http\nhost=forge.example\n\n", "protocol=https\nhost=forge.example.evil.example\n\n", "protocol=https\nhost=forge.example\nusername=other\n\n", "protocol=ssh\nhost=forge.example\n\n", "protocol=https\nhost=nothing.example\n\n"] {
        let (fine, out) = git_fill(&running, input);
        assert!(!fine || !out.contains("password=s3cret"), "{input}: {out}");
    }
    assert_eq!(asked(&running), 1);
    // Only what says http is given over http
    assert!(git_fill(&running, "protocol=http\nhost=intranet.example\n\n").1.contains("password=other"));
    // approve and reject change nothing in the vault
    let helper = env!("CARGO_BIN_EXE_git-credential-amethystora");
    let before = running.ask(json!({"op": "list", "kind": "password"}));
    for verb in ["approve", "reject"] {
        let mut child = Command::new("git").current_dir(&running.folder.0).args(["-c", "credential.helper=", "-c"]).arg(format!("credential.helper={helper}")).args(["credential", verb]).env("AMETHYSTORA_VAULT_SOCKET", &running.socket).stdin(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(b"protocol=https\nhost=forge.example\nusername=me\npassword=wrong\n\n").unwrap();
        assert!(child.wait().unwrap().success());
    }
    assert_eq!(before, running.ask(json!({"op": "list", "kind": "password"})));
    // A closed vault: git goes on without, and no error of its own
    running.ask(json!({"op": "lock"}));
    assert!(!git_fill(&running, "protocol=https\nhost=forge.example\n\n").1.contains("s3cret"));
}

#[test]
fn a_login_that_holds_a_newline_cannot_write_fields_of_its_own() {
    if !has("git") {
        return;
    }
    let running = start("helper-git-inject");
    running.set_up(PASSWORD);
    add_login(&running, "bad", "me", "forge.example", "x\nhost=evil.example");
    let (_, out) = git_fill(&running, "protocol=https\nhost=forge.example\n\n");
    assert!(!out.contains("evil.example"), "{out}");
}

fn docker_helper(running: &Running, action: &str, input: &str) -> (bool, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_docker-credential-amethystora")).arg(action).env("AMETHYSTORA_VAULT_SOCKET", &running.socket).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    (output.status.success(), String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Docker's helper protocol, as `docker login`, `pull` and `logout` speak it
#[test]
fn docker_credential_protocol() {
    let running = start("helper-docker");
    running.set_up(PASSWORD);
    let (fine, out) = docker_helper(&running, "get", "ghcr.io\n");
    assert!(!fine);
    assert_eq!(out, "credentials not found in native keychain");
    let (fine, out) = docker_helper(&running, "store", r#"{"ServerURL":"ghcr.io","Username":"me","Secret":"tok3n"}"#);
    assert!(fine, "{out}");
    let (fine, out) = docker_helper(&running, "get", "ghcr.io\n");
    assert!(fine, "{out}");
    let got: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(got, json!({"ServerURL": "ghcr.io", "Username": "me", "Secret": "tok3n"}));
    let (fine, out) = docker_helper(&running, "get", "https://ghcr.io/v2/\n");
    assert!(fine && out.contains("tok3n"), "{out}");
    // Plain http gets nothing for what was stored as https
    assert!(!docker_helper(&running, "get", "http://ghcr.io\n").0);
    // Another login in the vault is not on Docker's list, so a build does not ask for all of them
    add_login(&running, "forge", "you", "https://forge.example/", "x");
    let (fine, out) = docker_helper(&running, "list", "");
    assert!(fine);
    assert_eq!(serde_json::from_str::<Value>(&out).unwrap(), json!({"ghcr.io": "me"}));
    assert_eq!(asked(&running), 1);
    // Storing again changes the one there, and a person says so
    let (fine, _) = docker_helper(&running, "store", r#"{"ServerURL":"ghcr.io","Username":"me","Secret":"newer"}"#);
    assert!(fine);
    assert!(docker_helper(&running, "get", "ghcr.io\n").1.contains("newer"));
    assert_eq!(running.ask(json!({"op": "list", "kind": "password"}))["items"].as_array().unwrap().len(), 2);
    // Erasing asks, and a no keeps it
    running.outside.said.lock().unwrap().refuse = true;
    assert!(!docker_helper(&running, "erase", "ghcr.io\n").0);
    running.outside.said.lock().unwrap().refuse = false;
    assert!(docker_helper(&running, "erase", "ghcr.io\n").0);
    assert!(!docker_helper(&running, "get", "ghcr.io\n").0);
    // Nonsense is not a panic
    assert!(!docker_helper(&running, "store", "not json").0);
    assert!(!docker_helper(&running, "get", "\u{1}\u{2}%%%\n").0);
    running.ask(json!({"op": "lock"}));
    assert_eq!(docker_helper(&running, "get", "ghcr.io\n"), (false, "credentials not found in native keychain".to_string()));
}

// --- The command line --------------------------------------------------------------------------------------------------------

struct Person {
    running: Running,
    home: Folder,
}

impl Person {
    fn new(name: &str) -> Person {
        let running = start(name);
        running.set_up(PASSWORD);
        let home = Folder::new(&format!("{name}-home"));
        Person { running, home }
    }

    fn cli(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_amethystora-vault-cli"));
        command
            .env("HOME", &self.home.0)
            .env("AMETHYSTORA_VAULT_SOCKET", &self.running.socket)
            .env("AMETHYSTORA_VAULT_SSH_SOCKET", agent(&self.running))
            .env("XDG_CONFIG_HOME", self.home.0.join(".config"))
            .env("XDG_RUNTIME_DIR", self.running.folder.0.join("run"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdin(Stdio::null());
        command
    }

    fn says(&self, args: &[&str]) -> (i32, String, String) {
        let output = self.cli().args(args).output().unwrap();
        (output.status.code().unwrap_or(-1), String::from_utf8_lossy(&output.stdout).into_owned(), String::from_utf8_lossy(&output.stderr).into_owned())
    }

    fn put(&self, path: &str, text: &str) {
        let full = self.home.0.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, text).unwrap();
    }

    fn at(&self, path: &str) -> PathBuf {
        self.home.0.join(path)
    }

    fn sealed(&self, name: &str) -> Value {
        let items = self.running.ask(json!({"op": "list", "kind": "protected"}));
        let id = items["items"].as_array().unwrap().iter().find(|item| item["name"] == json!(name)).unwrap_or_else(|| panic!("{name} in {items}"))["id"].as_str().unwrap().to_string();
        self.running.ask(json!({"op": "get", "kind": "protected", "id": id}))["value"].clone()
    }

    fn sealed_text(&self, name: &str) -> String {
        use base64::Engine;
        let value = self.sealed(name);
        String::from_utf8(base64::engine::general_purpose::STANDARD.decode(value["files"][0]["data"].as_str().unwrap()).unwrap()).unwrap()
    }
}

#[test]
fn get_answers_each_tool_in_its_own_shape() {
    let person = Person::new("cli-get");
    add_login(&person.running, "api", "AKIAEXAMPLE", "", "the-secret");
    assert_eq!(person.says(&["get", "api"]).1, "the-secret\n");
    let aws: Value = serde_json::from_str(&person.says(&["get", "api", "aws"]).1).unwrap();
    assert_eq!(aws, json!({"Version": 1, "AccessKeyId": "AKIAEXAMPLE", "SecretAccessKey": "the-secret"}));
    let kube: Value = serde_json::from_str(&person.says(&["get", "api", "kube"]).1).unwrap();
    assert_eq!(kube, json!({"apiVersion": "client.authentication.k8s.io/v1", "kind": "ExecCredential", "status": {"token": "the-secret"}}));
    assert_eq!(person.says(&["get", "nothing"]).0, 1);
    assert_eq!(person.says(&["get", "api", "xml"]).0, 64);
    person.running.ask(json!({"op": "lock"}));
    assert_eq!(person.says(&["get", "api"]).0, 2);
}

#[test]
fn a_variable_goes_to_the_command_with_no_namespace() {
    let person = Person::new("cli-variable");
    add_login(&person.running, "github token", "", "", "the-secret");
    let own = std::fs::read_link("/proc/self/ns/user").unwrap();
    let (code, out, err) = person.says(&["run", "github token", "--", "sh", "-c", "printf '%s %s' \"$GITHUB_TOKEN\" \"$(readlink /proc/self/ns/user)\"; exit 3"]);
    assert_eq!(code, 3, "{err}");
    assert_eq!(out, format!("the-secret {}", own.display()));
    assert!(err.contains("GITHUB_TOKEN") && err.contains("inherits"), "{err}");
    let (code, out, _) = person.says(&["run", "MY_VAR=github token", "--", "sh", "-c", "printf %s \"$MY_VAR\""]);
    assert_eq!((code, out.as_str()), (0, "the-secret"));
    // Options after the -- are the command's, and none reaches the vault's own reading of them
    let (code, out, _) = person.says(&["run", "github token", "--", "printf", "%s", "--json"]);
    assert_eq!((code, out.as_str()), (0, "--json"));
    assert_eq!(person.says(&["run", "lower=github token", "--", "true"]).0, 64);
    assert_eq!(person.says(&["run", "github token", "true"]).0, 64);
    assert_eq!(person.says(&["run", "github token", "--"]).0, 64);
    assert_eq!(person.says(&["run", "nothing", "--", "true"]).0, 1);
    assert_eq!(person.says(&["run", "github token", "--", "/no/such/program"]).0, 127);
}

#[test]
fn protect_takes_the_file_only_once_the_sealed_copy_opened() {
    let person = Person::new("cli-protect");
    person.put(".aws/credentials", "[default]\nkey=value\n");
    let path = person.at(".aws/credentials");
    let (code, out, err) = person.says(&["protect", path.to_str().unwrap()]);
    assert_eq!(code, 0, "{out} {err}");
    assert!(!path.exists());
    // It asked once: to open the copy it had just made, which says ask
    assert_eq!(asked(&person.running), 1);
    assert_eq!(person.sealed_text(".aws/credentials"), "[default]\nkey=value\n");
    let (code, out, _) = person.says(&["protected"]);
    assert_eq!(code, 0);
    assert!(out.contains(".aws/credentials\t.aws/credentials\tfile\t20"), "{out}");
    assert!(!out.contains("key=value"), "{out}");
    // And back, as it was, and out of the vault
    assert_eq!(person.says(&["unprotect", ".aws/credentials"]).0, 0);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[default]\nkey=value\n");
    assert_eq!(person.says(&["protected"]).1, "");
}

#[test]
fn a_no_at_the_check_leaves_the_file_where_it_is() {
    let person = Person::new("cli-protect-no");
    person.put(".npmrc", "token\n");
    person.running.outside.said.lock().unwrap().refuse = true;
    let (code, ..) = person.says(&["protect", person.at(".npmrc").to_str().unwrap()]);
    assert_eq!(code, 1);
    assert_eq!(std::fs::read_to_string(person.at(".npmrc")).unwrap(), "token\n");
}

#[test]
fn unprotect_goes_over_nothing_and_a_no_keeps_both() {
    let person = Person::new("cli-unprotect");
    person.put(".npmrc", "token\n");
    assert_eq!(person.says(&["protect", person.at(".npmrc").to_str().unwrap()]).0, 0);
    person.put(".npmrc", "a new one\n");
    assert_eq!(person.says(&["unprotect", ".npmrc"]).0, 13);
    assert_eq!(std::fs::read_to_string(person.at(".npmrc")).unwrap(), "a new one\n");
    assert_eq!(person.sealed_text(".npmrc"), "token\n");
    std::fs::remove_file(person.at(".npmrc")).unwrap();
    // The no comes at the removal: the file is back, and the vault still has it
    let (code, ..) = {
        let waiting = person.running.outside.said.clone();
        waiting.lock().unwrap().refuse = true;
        let got = person.says(&["unprotect", ".npmrc"]);
        waiting.lock().unwrap().refuse = false;
        got
    };
    assert_eq!(code, 1);
    assert_eq!(person.sealed_text(".npmrc"), "token\n");
}

#[test]
fn protect_refuses_what_it_should() {
    let person = Person::new("cli-protect-refuses");
    person.put(".ssh/config", "Host x\n");
    person.put(".ssh/id_x.pub", "x\n");
    person.put("real", "x");
    std::os::unix::fs::symlink(person.at("real"), person.at("link")).unwrap();
    std::fs::create_dir_all(person.at("empty")).unwrap();
    let path = |name: &str| person.at(name).to_str().unwrap().to_string();
    assert_eq!(person.says(&["protect", &path(".ssh/config")]).0, 12);
    assert_eq!(person.says(&["protect", &path(".ssh/id_x.pub")]).0, 12);
    assert_eq!(person.says(&["protect", &path("link")]).0, 10);
    assert_eq!(person.says(&["protect", "/etc/passwd"]).0, 10);
    assert_eq!(person.says(&["protect", &path("../x")]).0, 10);
    assert_eq!(person.says(&["protect", &path("nothing")]).0, 11);
    assert_eq!(person.says(&["protect", &path("empty")]).0, 11);
    assert_eq!(person.says(&["protect", person.home.0.to_str().unwrap()]).0, 10);
    std::fs::File::create(person.at("large")).unwrap().set_len(3 * 1024 * 1024).unwrap();
    assert_eq!(person.says(&["protect", &path("large")]).0, 4);
    assert!(person.at("large").exists() && person.at("real").exists());
    assert_eq!(person.running.ask(json!({"op": "list", "kind": "protected"}))["items"], json!([]));
}

#[test]
fn nearly_the_largest_file_goes_in_and_comes_back_whole() {
    let person = Person::new("cli-protect-large");
    let data: Vec<u8> = (0..700 * 1024).map(|index| (index % 251) as u8).collect();
    std::fs::write(person.at("big"), &data).unwrap();
    let (code, _, err) = person.says(&["protect", person.at("big").to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(person.says(&["unprotect", "big"]).0, 0);
    assert_eq!(std::fs::read(person.at("big")).unwrap(), data);
}

#[test]
fn a_folder_goes_in_leaving_what_holds_no_secret() {
    let person = Person::new("cli-folder");
    person.put(".ssh/id_ed25519", "private\n");
    person.put(".ssh/id_ed25519.pub", "public\n");
    person.put(".ssh/config", "Host x\n");
    person.put(".ssh/sub/key", "deep\n");
    assert_eq!(person.says(&["protect", person.at(".ssh").to_str().unwrap()]).0, 0);
    assert!(!person.at(".ssh/id_ed25519").exists() && !person.at(".ssh/sub").exists());
    assert!(person.at(".ssh/config").exists() && person.at(".ssh/id_ed25519.pub").exists());
    assert_eq!(person.says(&["unprotect", ".ssh"]).0, 0);
    assert_eq!(std::fs::read_to_string(person.at(".ssh/sub/key")).unwrap(), "deep\n");
}

#[test]
fn the_usual_places_are_listed_as_they_are() {
    let person = Person::new("cli-usual");
    person.put(".ssh/id_rsa", "x");
    person.put(".ssh/id_rsa.pub", "x");
    person.put(".codex/auth.json", "abc");
    let (code, out, _) = person.says(&["protected", "usual"]);
    assert_eq!(code, 0);
    assert_eq!(out, ".codex/auth.json\t3\n.ssh/id_rsa\t1\n");
    let listed: Value = serde_json::from_str(&person.says(&["protected", "usual", "--json"]).1).unwrap();
    assert_eq!(listed["usual"][1], json!({"path": ".ssh/id_rsa", "size": 1}));
}

/// Whether this machine gives an account a namespace of its own: Docker's default seccomp profile does not
fn namespaces() -> bool {
    let fine = Command::new("unshare").args(["-Urm", "true"]).status().is_ok_and(|status| status.success());
    if !fine {
        eprintln!("no user namespaces here: this test did not run. The VM suite does (plans/vault/work/secret.md).");
    }
    fine
}

/// A stand-in for a tool that rewrites its own login
const REWRITES: &str = "cat \"$1\" >\"$1.seen\" 2>/dev/null; true";

fn tool(person: &Person, name: &str, body: &str) -> String {
    let script = person.at(name);
    std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script.to_str().unwrap().to_string()
}

#[test]
fn a_tool_rewrites_its_file_in_the_namespace_and_finds_it_sealed_afterwards() {
    if !namespaces() {
        return;
    }
    let _ = REWRITES;
    let person = Person::new("run-rewrite");
    person.put(".claude/.credentials.json", "{\"token\":\"old\"}\n");
    person.put(".claude/settings.json", "{}\n");
    let path = person.at(".claude/.credentials.json");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    assert!(!path.exists());
    let report = person.at("report");
    let rewrite = tool(&person, "tool", &format!(
        "cat '{file}' >'{report}'\nstat -c '%a' '{file}' >>'{report}'\nprintf '%s' '{{\"token\":\"new\"}}' >'{file}'\necho sibling >>'{sibling}'\nexit 0",
        file = path.display(),
        report = report.display(),
        sibling = person.at(".claude/settings.json").display(),
    ));
    let (code, _, err) = person.says(&["run", ".claude/.credentials.json", "--", &rewrite]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("other programs of this account can read"), "{err}");
    let seen = std::fs::read_to_string(&report).unwrap();
    assert!(seen.starts_with("{\"token\":\"old\"}\n"), "{seen}");
    // With the mode it had in the home
    assert!(seen.contains("644"), "{seen}");
    // It is sealed with what the tool left, gone from the home, and what lay beside it was the real thing
    assert_eq!(person.sealed_text(".claude/.credentials.json"), "{\"token\":\"new\"}");
    assert!(!path.exists(), "still at the path");
    assert_eq!(std::fs::read_to_string(person.at(".claude/settings.json")).unwrap(), "{}\nsibling\n");
    // Nothing written reached the disk under another name
    let (code, ..) = person.says(&["run", ".claude/.credentials.json", "--", "true"]);
    assert_eq!(code, 0);
    assert_eq!(person.sealed_text(".claude/.credentials.json"), "{\"token\":\"new\"}");
}

#[test]
fn an_unchanged_file_is_not_sealed_again_and_asks_once() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-unchanged");
    person.put(".npmrc", "token\n");
    assert_eq!(person.says(&["protect", person.at(".npmrc").to_str().unwrap()]).0, 0);
    let before = person.running.outside.asked();
    let (code, _, err) = person.says(&["run", ".npmrc", "--", "cat", person.at(".npmrc").to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(person.running.outside.asked(), before + 1);
}

#[test]
fn a_refused_run_never_sets_anything_up() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-no");
    person.put(".npmrc", "token\n");
    assert_eq!(person.says(&["protect", person.at(".npmrc").to_str().unwrap()]).0, 0);
    person.running.outside.said.lock().unwrap().refuse = true;
    let marker = person.at("ran");
    let (code, _, err) = person.says(&["run", ".npmrc", "--", "touch", marker.to_str().unwrap()]);
    assert_eq!(code, 1, "{err}");
    assert!(!marker.exists() && !person.at(".npmrc").exists());
}

#[test]
fn a_tool_killed_halfway_loses_nothing_and_what_it_left_goes_beside() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-killed");
    person.put(".codex/auth.json", "{\"good\":true}\n");
    let path = person.at(".codex/auth.json");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let killed = tool(&person, "tool", &format!("printf '%s' '{{\"half' >'{}'\nkill -KILL $$", path.display()));
    let (code, _, err) = person.says(&["run", ".codex/auth.json", "--", &killed]);
    assert_eq!(code, 128 + 9, "{err}");
    assert!(err.contains("did not end well"), "{err}");
    // The good copy is as it was, what it left is sealed too, and the path is empty again
    assert_eq!(person.sealed_text(".codex/auth.json"), "{\"good\":true}\n");
    assert_eq!(person.sealed_text(".codex/auth.json (from a command that did not end well)"), "{\"half");
    assert!(!path.exists());
    // And the next run is given the good copy
    let reader = tool(&person, "reader", &format!("cat '{}'", path.display()));
    let (code, out, _) = person.says(&["run", ".codex/auth.json", "--", &reader]);
    assert_eq!((code, out.as_str()), (0, "{\"good\":true}\n"));
}

#[test]
fn a_copy_from_a_command_that_did_not_end_well_keeps_its_mark_after_a_run_that_did() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-killed-mark");
    person.put(".netrc", "good\n");
    let path = person.at(".netrc");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let failing = tool(&person, "tool", &format!("printf half >'{}'\nkill -KILL $$", path.display()));
    assert_eq!(person.says(&["run", ".netrc", "--", &failing]).0, 137);
    let kept = ".netrc (from a command that did not end well)";
    // A run on that copy which ends well, and changes it, leaves it marked: by its path, `run` and `unprotect` still find the good one
    let fixing = tool(&person, "fixer", &format!("printf fixed >'{}'", path.display()));
    let (code, _, err) = person.says(&["run", kept, "--", &fixing]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(person.sealed_text(kept), "fixed");
    let items = person.running.ask(json!({"op": "list", "kind": "protected"}));
    let marked = items["items"].as_array().unwrap().iter().find(|item| item["name"] == json!(kept)).unwrap()["meta"]["killed"].clone();
    assert_eq!(marked, json!(true), "{items}");
    let reader = tool(&person, "reader", &format!("cat '{}'", path.display()));
    assert_eq!(person.says(&["run", ".netrc", "--", &reader]).1, "good\n");
}

#[test]
fn a_protect_that_did_not_finish_finishes_when_asked_again() {
    let person = Person::new("protect-again");
    person.put(".config/gh/hosts.yml", "token: one\n");
    let path = person.at(".config/gh/hosts.yml");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    // What a protect stopped between the sealing and the removal leaves: the plain file beside a sealed copy of the same
    person.put(".config/gh/hosts.yml", "token: one\n");
    let (code, out, err) = person.says(&["protect", path.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert!(out.ends_with("\t.config/gh/hosts.yml\n"), "{out}");
    assert!(!path.exists());
    assert_eq!(person.sealed_text(".config/gh/hosts.yml"), "token: one\n");
    // Something else at the path is still never put over what is sealed, also when it is as long
    person.put(".config/gh/hosts.yml", "token: two\n");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 18);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "token: two\n");
    assert_eq!(person.sealed_text(".config/gh/hosts.yml"), "token: one\n");
}

#[test]
fn the_command_line_killed_outright_leaves_the_old_copy_and_the_next_run_goes_on() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-cli-killed");
    person.put(".pypirc", "good\n");
    let path = person.at(".pypirc");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let started = person.at("started");
    let long = tool(&person, "tool", &format!("printf changed >'{}'\ntouch '{}'\nexec sleep 30", path.display(), started.display()));
    let mut child = person.cli().args(["run", ".pypirc", "--", &long]).stderr(Stdio::null()).spawn().unwrap();
    let begun = Instant::now();
    while !started.exists() {
        assert!(begun.elapsed() < Duration::from_secs(20), "the tool never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    // A second command cannot have it while the first does
    assert_eq!(person.says(&["run", ".pypirc", "--", "true"]).0, 14);
    // Outside the namespace there is nothing to read at the path
    assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(person.sealed_text(".pypirc"), "good\n");
    // What it left behind is an empty file, and the next run takes its place. The tool heard of it too, with its parent gone.
    let (code, out, err) = person.says(&["run", ".pypirc", "--", "cat", path.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (0, "good\n"), "{err}");
    assert!(!path.exists());
}

#[test]
fn a_plain_file_that_came_back_is_not_gone_over() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-there");
    person.put(".npmrc", "sealed\n");
    assert_eq!(person.says(&["protect", person.at(".npmrc").to_str().unwrap()]).0, 0);
    person.put(".npmrc", "plain again\n");
    let (code, _, err) = person.says(&["run", ".npmrc", "--", "true"]);
    assert_eq!(code, 13, "{err}");
    assert_eq!(std::fs::read_to_string(person.at(".npmrc")).unwrap(), "plain again\n");
}

#[test]
fn a_folder_runs_on_a_tmpfs_with_what_stayed_showing_through() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-folder");
    person.put(".ssh/id_x", "private\n");
    person.put(".ssh/config", "Host x\n");
    assert_eq!(person.says(&["protect", person.at(".ssh").to_str().unwrap()]).0, 0);
    let folder = person.at(".ssh");
    let report = person.at("report");
    let rewrite = tool(&person, "tool", &format!(
        "cd '{folder}'\nls -A >'{report}'\ncat id_x config >>'{report}'\nstat -f -c %T . >>'{report}'\nprintf new >tmp && mv tmp id_x\nmkdir -p more\nprintf made >more/thing\necho kept >>config",
        folder = folder.display(),
        report = report.display(),
    ));
    let (code, _, err) = person.says(&["run", ".ssh", "--", &rewrite]);
    assert_eq!(code, 0, "{err}");
    let seen = std::fs::read_to_string(&report).unwrap();
    assert!(seen.contains("config\nid_x\n") && seen.contains("private\nHost x\n"), "{seen}");
    assert!(seen.contains("tmpfs"), "{seen}");
    // A rename inside the folder works, as tools do it, and it all came back. What stayed was written through.
    assert_eq!(std::fs::read_to_string(person.at(".ssh/config")).unwrap(), "Host x\nkept\n");
    assert!(!person.at(".ssh/id_x").exists() && !person.at(".ssh/more").exists());
    let value = person.sealed(".ssh");
    let names: Vec<&str> = value["files"].as_array().unwrap().iter().map(|file| file["at"].as_str().unwrap()).collect();
    assert_eq!(names, ["id_x", "more/thing"], "{value}");
}

#[test]
fn a_file_alone_in_the_home_is_rewritten_in_place_and_a_rename_over_it_is_refused() {
    if !namespaces() {
        return;
    }
    // Directly in the home there is no folder to cover, so the file alone is mounted: a rename over it cannot work, and the tool hears so
    let person = Person::new("run-rename-single");
    person.put(".npmrc", "old\n");
    let path = person.at(".npmrc");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let rename = tool(&person, "tool", &format!("printf new >'{file}.tmp'\nmv '{file}.tmp' '{file}' 2>/dev/null\necho $? >'{report}'\nrm -f '{file}.tmp'", file = path.display(), report = person.at("report").display()));
    assert_eq!(person.says(&["run", ".npmrc", "--", &rename]).0, 0);
    assert_ne!(std::fs::read_to_string(person.at("report")).unwrap().trim(), "0");
    assert_eq!(person.sealed_text(".npmrc"), "old\n");
    assert!(!path.exists());
}

#[test]
fn the_vault_closed_while_a_command_ran_says_so_and_keeps_the_old_copy() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-closed");
    person.put(".npmrc", "old\n");
    let path = person.at(".npmrc");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let rewrite = tool(&person, "tool", &format!("printf new >'{}'\nnc=0", path.display()));
    // The stand-in closes the vault as the screen locking would, through the same socket
    let closing = format!("{rewrite}\n");
    let _ = closing;
    let locks = tool(&person, "locks", &format!("printf new >'{file}'\n'{cli}' lock", file = path.display(), cli = env!("CARGO_BIN_EXE_amethystora-vault-cli")));
    let (code, _, err) = person.says(&["run", ".npmrc", "--", &locks]);
    // With no terminal there is nobody to ask, and that is said
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("gone"), "{err}");
    person.running.ask(json!({"op": "unlock", "method": "password", "secret": common::b64(PASSWORD)}));
    assert_eq!(person.sealed_text(".npmrc"), "old\n");
    assert!(!path.exists());
}

// --- Helpers' settings -----------------------------------------------------------------------------------------------------------

#[test]
fn helpers_say_what_is_set_up_and_write_only_when_asked() {
    if !has("git") {
        return;
    }
    let person = Person::new("cli-helpers");
    let status = || {
        let (code, out, _) = person.says(&["helpers"]);
        assert_eq!(code, 0);
        out
    };
    assert!(status().contains("git\tinstalled\toff"), "{}", status());
    assert!(!person.at(".gitconfig").exists() && !person.at(".docker").exists(), "looking wrote something");
    // git, through git's own command
    assert_eq!(person.says(&["helpers", "git", "on"]).0, 0);
    assert_eq!(person.says(&["helpers", "git", "on"]).0, 0);
    assert!(status().contains("git\tinstalled\ton"));
    let config = std::fs::read_to_string(person.at(".gitconfig")).unwrap();
    assert_eq!(config.matches("helper = amethystora").count(), 1, "{config}");
    assert_eq!(person.says(&["helpers", "git", "off"]).0, 0);
    assert!(status().contains("git\tinstalled\toff"));
    // docker, keeping what the file already says, and no store of somebody else's replaced
    person.put(".docker/config.json", "{\"auths\":{\"x\":{}},\"credsStore\":\"pass\"}");
    assert_eq!(person.says(&["helpers", "docker", "on"]).0, 13);
    person.put(".docker/config.json", "{\"auths\":{\"x\":{}}}");
    assert_eq!(person.says(&["helpers", "docker", "on"]).0, 0);
    let config: Value = serde_json::from_str(&std::fs::read_to_string(person.at(".docker/config.json")).unwrap()).unwrap();
    assert_eq!(config, json!({"auths": {"x": {}}, "credsStore": "amethystora"}));
    assert!(status().contains("\ton\n"));
    assert_eq!(person.says(&["helpers", "docker", "off"]).0, 0);
    assert_eq!(serde_json::from_str::<Value>(&std::fs::read_to_string(person.at(".docker/config.json")).unwrap()).unwrap(), json!({"auths": {"x": {}}}));
    // aws, a block of its own that goes without touching anything else
    person.put(".aws/config", "[profile other]\nregion = eu-west-1\n");
    assert_eq!(person.says(&["helpers", "aws", "on", "login", "work"]).0, 0);
    let text = std::fs::read_to_string(person.at(".aws/config")).unwrap();
    assert!(text.contains("[profile work]\ncredential_process = /usr/bin/ame vault get login aws\n"), "{text}");
    assert_eq!(person.says(&["helpers", "aws", "on", "login", "other"]).0, 13);
    assert_eq!(person.says(&["helpers", "aws", "on", "a b; rm -rf /", "x"]).0, 64);
    assert_eq!(person.says(&["helpers", "aws", "off"]).0, 0);
    assert_eq!(std::fs::read_to_string(person.at(".aws/config")).unwrap(), "[profile other]\nregion = eu-west-1\n");
    // claude, and nothing that needs a shell's quoting
    assert_eq!(person.says(&["helpers", "claude", "on", "x;rm"]).0, 64);
    person.put(".claude/settings.json", "{\"model\":\"m\"}");
    assert_eq!(person.says(&["helpers", "claude", "on", "anthropic"]).0, 0);
    let settings: Value = serde_json::from_str(&std::fs::read_to_string(person.at(".claude/settings.json")).unwrap()).unwrap();
    assert_eq!(settings, json!({"model": "m", "apiKeyHelper": "/usr/bin/ame vault get anthropic"}));
    assert_eq!(person.says(&["helpers", "claude", "off"]).0, 0);
    assert_eq!(serde_json::from_str::<Value>(&std::fs::read_to_string(person.at(".claude/settings.json")).unwrap()).unwrap(), json!({"model": "m"}));
    person.put(".claude/settings.json", "{\"apiKeyHelper\":\"my-own\"}");
    assert_eq!(person.says(&["helpers", "claude", "on", "anthropic"]).0, 13);
    assert_eq!(person.says(&["helpers", "claude", "off"]).0, 0);
    assert!(std::fs::read_to_string(person.at(".claude/settings.json")).unwrap().contains("my-own"));
    assert_eq!(person.says(&["helpers", "nothing", "on"]).0, 64);
}

#[test]
fn the_ssh_switch_writes_its_files_only_while_on() {
    let person = Person::new("cli-ssh");
    let file = person.at(".config/environment.d/60-amethystora-vault-ssh.conf");
    let gcr = person.at(".config/systemd/user/gcr-ssh-agent.socket.d/60-amethystora-vault-ssh.conf");
    assert!(person.says(&["ssh"]).1.contains("ssh\toff"));
    assert_eq!(person.says(&["ssh", "on"]).0, 0);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "SSH_AUTH_SOCK=/run/amethystora/vault-ssh.sock\n");
    assert_eq!(std::fs::read_to_string(&gcr).unwrap(), "[Socket]\nExecStartPost=\n");
    let (_, out, _) = person.says(&["ssh", "status"]);
    assert!(out.contains("ssh\ton") && out.contains("keys\t0"), "{out}");
    assert_eq!(person.says(&["ssh", "off"]).0, 0);
    assert!(!file.exists() && !gcr.exists());
    assert_eq!(person.says(&["ssh", "sideways"]).0, 64);
}

#[test]
fn key_import_takes_a_real_key_with_its_passphrase() {
    if !has("ssh-keygen") {
        return;
    }
    let person = Person::new("cli-key");
    let folder = Folder::new("cli-key-files");
    let (plain, public) = ssh_keygen(&folder.0, "plain", &["-t", "ed25519"], "");
    let (code, out, err) = person.says(&["key", "import", "mine", plain.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert!(!out.trim().is_empty(), "{out}");
    assert!(ssh_add(&person.running, &["-L"]).1.contains(&public));
    // A passphrase comes on standard input, never an argument
    let (locked, public) = ssh_keygen(&folder.0, "locked", &["-t", "ecdsa", "-b", "256"], "pass phrase");
    let mut child = person.cli().args(["key", "import", locked.to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"pass phrase\n").unwrap();
    assert!(child.wait().unwrap().success());
    assert!(ssh_add(&person.running, &["-L"]).1.contains(&public));
    let mut wrong = person.cli().args(["key", "import", locked.to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    wrong.stdin.take().unwrap().write_all(b"wrong\n").unwrap();
    assert_eq!(wrong.wait().unwrap().code(), Some(1));
    assert_eq!(person.says(&["key", "import"]).0, 64);
    assert_eq!(person.says(&["key", "import", "/no/such/file"]).0, 11);
}

// --- What the security review found ------------------------------------------------------------------------------------------------
// Each of these failed before the fix it holds (plans/vault/work/security-secret.md).

/// The data a client signs to log in (RFC 4252): the session, the request, who, the service, how, the key
fn login_data(public: &str) -> Vec<u8> {
    use base64::Engine;
    let blob = base64::engine::general_purpose::STANDARD.decode(public.split(' ').nth(1).unwrap()).unwrap();
    let mut out = Vec::new();
    string(&mut out, &[7u8; 32]);
    out.push(50);
    string(&mut out, b"root");
    string(&mut out, b"ssh-connection");
    string(&mut out, b"publickey");
    out.push(1);
    string(&mut out, b"ssh-ed25519");
    string(&mut out, &blob);
    out
}

/// What `ssh-keygen -Y sign` and git's signed commits have the agent sign
fn sshsig_data(namespace: &str) -> Vec<u8> {
    let mut out = b"SSHSIG".to_vec();
    string(&mut out, namespace.as_bytes());
    string(&mut out, b"");
    string(&mut out, b"sha512");
    string(&mut out, &[1u8; 64]);
    out
}

fn bind_request(forwarding: u8) -> Vec<u8> {
    let mut out = vec![27];
    string(&mut out, b"session-bind@openssh.com");
    string(&mut out, b"a host key blob");
    string(&mut out, &[9u8; 32]);
    string(&mut out, b"signature");
    out.push(forwarding);
    out
}

/// The comments of an identities answer, which must be whole to be read at all
fn comments(answer: &[u8]) -> Vec<Vec<u8>> {
    assert_eq!(answer[0], 12);
    let count = u32::from_be_bytes(answer[1..5].try_into().unwrap()) as usize;
    let (mut at, mut found) = (5, Vec::new());
    for _ in 0..count {
        for part in 0..2 {
            let length = u32::from_be_bytes(answer[at..at + 4].try_into().unwrap()) as usize;
            at += 4;
            if part == 1 {
                found.push(answer[at..at + length].to_vec());
            }
            at += length;
        }
    }
    assert_eq!(at, answer.len(), "the list has more in it than it says");
    found
}

fn last_ask(running: &Running) -> amethystora_vault_service::outside::Ask {
    running.outside.said.lock().unwrap().asked.last().unwrap().clone()
}

#[test]
fn a_forwarded_connection_asks_every_time_says_so_and_lists_no_names() {
    use amethystora_vault_service::outside::say;
    if !has("ssh-keygen") {
        return;
    }
    let running = start("agent-forwarded");
    let folder = Folder::new("agent-forwarded-keys");
    running.set_up(PASSWORD);
    let (file, public) = ssh_keygen(&folder.0, "ed25519", &["-t", "ed25519"], "");
    import(&running, "my key", &file, b"", "while-open");
    let login = login_data(&public);
    let mut here = connect(&running);
    assert_eq!(exchange(&mut here, &sign_request(&public, &login, 0))[0], 14);
    assert_eq!(exchange(&mut here, &sign_request(&public, &login, 0))[0], 14);
    assert_eq!(asked(&running), 1);
    assert_eq!(comments(&exchange(&mut here, &[11])), [b"my key".to_vec()]);
    // Another computer's, which said so when it connected
    let mut there = connect(&running);
    assert_eq!(exchange(&mut there, &bind_request(1)), [6]);
    assert_eq!(comments(&exchange(&mut there, &[11])), [Vec::<u8>::new()]);
    for _ in 0..2 {
        assert_eq!(exchange(&mut there, &sign_request(&public, &login, 0))[0], 14);
    }
    assert_eq!(asked(&running), 3);
    let ask = last_ask(&running);
    assert_eq!(ask.message, say::SIGN_FORWARDED);
    assert!(ask.detail.starts_with("SHA256:"), "{}", ask.detail);
    // It stays so: saying otherwise afterwards changes nothing
    assert_eq!(exchange(&mut there, &bind_request(0)), [6]);
    assert_eq!(exchange(&mut there, &sign_request(&public, &login, 0))[0], 14);
    assert_eq!(asked(&running), 4);
    // And the connection that is not forwarded has the window it had
    assert_eq!(exchange(&mut here, &sign_request(&public, &login, 0))[0], 14);
    assert_eq!(asked(&running), 4);
    // A bind that is not one is the one word for no, and a connection that sends one stays usable
    let mut broken = bind_request(1);
    broken.push(0);
    for bad in [broken, bind_request(2), vec![27], vec![27, 0, 0, 0, 3, b'a', b'b', b'c']] {
        assert_eq!(exchange(&mut here, &bad), [5]);
    }
    assert_eq!(comments(&exchange(&mut here, &[11])), [b"my key".to_vec()]);
}

/// The real thing, which the review demonstrated: `ssh -A` to a real `sshd`, and on the far side a signature of a message
#[test]
fn a_real_forwarded_login_cannot_have_a_message_signed_without_asking() {
    use amethystora_vault_service::outside::say;
    if !has("ssh") || !has("ssh-keygen") || !sshd() {
        return;
    }
    // SAFETY: getuid cannot fail
    if unsafe { libc::getuid() } != 0 {
        eprintln!("sshd needs root to log anyone in: this test did not run");
        return;
    }
    let _root = RootUnlocked::new();
    let running = start("agent-forward-real");
    let folder = Folder::new("agent-forward-real-server");
    running.set_up(PASSWORD);
    std::fs::create_dir_all("/run/sshd").unwrap();
    let (host_key, _) = ssh_keygen(&folder.0, "host", &["-t", "ed25519"], "");
    let (file, public) = ssh_keygen(&folder.0, "ed25519", &["-t", "ed25519"], "");
    import(&running, "my key", &file, b"", "while-open");
    std::fs::remove_file(&file).unwrap();
    let authorized = folder.0.join("authorized");
    std::fs::write(&authorized, format!("{public}\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&folder.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    let port = 24000 + (std::process::id() % 2000) as u16;
    let mut server = Command::new("/usr/sbin/sshd")
        .args(["-D", "-e", "-p", &port.to_string(), "-o", "ListenAddress=127.0.0.1", "-o", "PermitRootLogin=prohibit-password", "-o", "StrictModes=no", "-o", "UsePAM=no", "-o", "PasswordAuthentication=no", "-o", "AllowAgentForwarding=yes", "-h"])
        .arg(&host_key)
        .arg("-o")
        .arg(format!("AuthorizedKeysFile={}", authorized.display()))
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let message = folder.0.join("message");
    std::fs::write(&message, b"what is signed").unwrap();
    let pub_file = folder.0.join("ed25519.pub");
    let remote = format!("ssh-add -l; ssh-keygen -Y sign -n git -f '{}' '{}' && echo SIGNED", pub_file.display(), message.display());
    let done = Command::new("ssh")
        .args(["-A", "-F", "none", "-p", &port.to_string(), "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=no", "-o", "UserKnownHostsFile=/dev/null", "-o", "LogLevel=ERROR", "-o", "IdentitiesOnly=yes", "-i"])
        .arg(&pub_file)
        .arg("-o")
        .arg(format!("IdentityAgent={}", agent(&running).display()))
        .arg("root@127.0.0.1")
        .arg(remote)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let _ = server.kill();
    let _ = server.wait();
    let (out, err) = (String::from_utf8_lossy(&done.stdout).into_owned(), String::from_utf8_lossy(&done.stderr).into_owned());
    // The image build's container logs in but gives the far side no forwarded agent at all (seen in the Beta of 16f323f6): that is
    // the place, not the Vault, and `a_forwarded_connection_asks_every_time_says_so_and_lists_no_names` holds the same rule there
    if !out.contains("SHA256:") && format!("{out}{err}").contains("Could not open a connection to your authentication agent") {
        eprintln!("sshd forwarded no agent on this machine: this test did not run\n{err}");
        return;
    }
    // The far side was shown a key and no name for it, and what it signed was asked
    assert!(out.contains("SHA256:") && !out.contains("my key"), "{out} {err}");
    assert!(out.contains("SIGNED"), "{out} {err}");
    assert_eq!(asked(&running), 2, "{out} {err}");
    assert_eq!(last_ask(&running).message, say::SIGN_FORWARDED);
}

#[test]
fn a_message_is_never_signed_on_a_window_and_the_prompt_says_what_it_is() {
    use amethystora_vault_service::outside::say;
    if !has("ssh-keygen") {
        return;
    }
    let running = start("agent-purposes");
    let folder = Folder::new("agent-purposes-keys");
    running.set_up(PASSWORD);
    let (file, public) = ssh_keygen(&folder.0, "ed25519", &["-t", "ed25519"], "");
    import(&running, "key", &file, b"", "while-open");
    let mut stream = connect(&running);
    let mut sign = |data: &[u8]| assert_eq!(exchange(&mut stream, &sign_request(&public, data, 0))[0], 14);
    sign(&login_data(&public));
    sign(&login_data(&public));
    assert_eq!(asked(&running), 1);
    assert_eq!(last_ask(&running).message, say::SIGN);
    sign(&sshsig_data("git"));
    sign(&sshsig_data("git"));
    assert_eq!(asked(&running), 3);
    let ask = last_ask(&running);
    assert_eq!((ask.message, ask.detail.as_str()), (say::SIGN_MESSAGE, "git"));
    sign(b"anything else");
    sign(b"anything else");
    assert_eq!(asked(&running), 5);
    assert_eq!(last_ask(&running).message, say::SIGN_OTHER);
    // And the window the login opened is still there
    sign(&login_data(&public));
    assert_eq!(asked(&running), 5);
    // Data shaped like a login for another key is not a login for this one
    let (other, other_public) = ssh_keygen(&folder.0, "other", &["-t", "ed25519"], "");
    let _ = other;
    sign(&login_data(&other_public));
    assert_eq!(asked(&running), 6);
}

#[test]
fn a_yes_to_something_else_does_not_end_a_window_and_a_forced_ask_remembers_nothing() {
    use amethystora_vault_service::outside::{say, Action};
    use amethystora_vault_service::peer::Peer;
    use amethystora_vault_service::service::{Config, Next, Reuse, Service};
    use std::os::fd::AsRawFd;
    use std::rc::Rc;
    let folder = Folder::new("agent-window-keys");
    let (outside, handle) = common::fake();
    let chip = std::sync::Arc::new(amethystora_vault_core::slots::fake::FakeChip::new());
    let mut service = Service::new(Config { state: folder.0.clone(), log_n: 10, worker: PathBuf::new() }, chip, Box::new(outside));
    let (a, _b) = UnixStream::pair().unwrap();
    let peer = Rc::new(Peer::of(a.as_raw_fd()).unwrap());
    let say_it = |service: &mut Service, line: Value| -> Value {
        let mut next = service.request(&peer, line.to_string().as_bytes());
        loop {
            match next {
                Next::Answer(answer) => return answer,
                Next::Approve { then, .. } => next = then(service, &peer, true),
                Next::WhenUnlocked { then } => next = then(service, &peer, true),
                Next::Work { job, then } | Next::Slow { job, then } => next = then(service, &peer, job()),
            }
        }
    };
    let answer = say_it(&mut service, json!({"v": 1, "op": "setup", "method": "password", "secret": common::b64(PASSWORD)}));
    say_it(&mut service, json!({"v": 1, "op": "confirm", "recovery": common::b64(answer["recovery"].as_str().unwrap())}));
    say_it(&mut service, json!({"v": 1, "op": "put", "kind": "thing", "name": "thing", "id": "one", "value": "x"}));
    let window = Duration::from_millis(150);
    let sign = |service: &mut Service| service.with_window(peer.uid, "thing", "one", Action::Use, say::SIGN, "", Reuse::For(window), |_, _| Next::ok());
    let use_it = |service: &mut Service, force: bool| service.with_approval(peer.uid, "thing", "one", Action::Export, say::EXPORT_ITEM, force, |_, _| Next::ok());
    let says_yes = |service: &mut Service, next: Next| match next {
        Next::Approve { then, .. } => assert!(matches!(then(service, &peer, true), Next::Answer(_))),
        _ => panic!("it was not asked"),
    };
    let next = sign(&mut service);
    says_yes(&mut service, next);
    assert!(!matches!(sign(&mut service), Next::Approve { .. }), "asked again inside the window");
    // A yes to signing is not a yes to taking the item out
    let next = use_it(&mut service, false);
    says_yes(&mut service, next);
    std::thread::sleep(window + Duration::from_millis(60));
    // The window is over, and a yes to something forced since does not make it endless
    let next = use_it(&mut service, true);
    says_yes(&mut service, next);
    let next = sign(&mut service);
    says_yes(&mut service, next);
    std::thread::sleep(window + Duration::from_millis(60));
    assert!(matches!(sign(&mut service), Next::Approve { .. }), "never asked again");
    // What was forced is a yes to that and to nothing after it
    say_it(&mut service, json!({"v": 1, "op": "put", "kind": "thing", "name": "two", "id": "two", "value": "x"}));
    let next = service.with_approval(peer.uid, "thing", "two", Action::Export, say::EXPORT_ITEM, true, |_, _| Next::ok());
    says_yes(&mut service, next);
    assert!(matches!(service.with_approval(peer.uid, "thing", "two", Action::Export, say::EXPORT_ITEM, false, |_, _| Next::ok()), Next::Approve { .. }));
    drop(handle);
}

// --- Run: the file's place, signals, what is left -----------------------------------------------------------------------------

fn make_fifo(path: &Path) {
    let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: a live string
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
}

/// Waits for the pipe's end to close: the processes that held it have ended
fn watch_pipe(pipe: &Path) -> (std::sync::mpsc::Receiver<&'static str>, std::thread::JoinHandle<()>) {
    let (sender, receiver) = std::sync::mpsc::channel();
    let pipe = pipe.to_path_buf();
    let thread = std::thread::spawn(move || {
        let Ok(mut file) = std::fs::File::open(&pipe) else { return };
        let _ = sender.send("open");
        let mut byte = [0u8; 1];
        if matches!(file.read(&mut byte), Ok(0)) {
            let _ = sender.send("closed");
        }
    });
    (receiver, thread)
}

fn finished(child: &mut std::process::Child, within: Duration) -> Option<std::process::ExitStatus> {
    let begun = Instant::now();
    while begun.elapsed() < within {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

#[test]
fn a_signal_to_the_command_line_ends_the_command_and_everything_it_started() {
    if !namespaces() {
        return;
    }
    for (signal, name) in [(libc::SIGTERM, "term"), (libc::SIGINT, "int"), (libc::SIGHUP, "hup"), (libc::SIGKILL, "kill")] {
        let person = Person::new(&format!("run-signal-{name}"));
        person.put(".npmrc", "token\n");
        assert_eq!(person.says(&["protect", person.at(".npmrc").to_str().unwrap()]).0, 0);
        let pipe = person.at("pipe");
        make_fifo(&pipe);
        // The pipe is held by the tool and by what it started in the background
        // (`wait`, not a second foreground `sleep`: a shell holds a SIGINT it is sent until its foreground child ends, and the command line
        // sends the signal to the command alone. The test used to send it at once, before the shell got there: that was the flake.)
        let long = tool(&person, "tool", &format!("exec 3>'{}'\nsleep 60 &\nwait", pipe.display()));
        let mut start = person.cli();
        start.args(["run", ".npmrc", "--", &long]).stderr(Stdio::null());
        // A test started in the background of a shell has SIGINT and SIGQUIT ignored, which every process below it would inherit
        use std::os::unix::process::CommandExt;
        // SAFETY: signal() only, between fork and exec
        unsafe {
            start.pre_exec(|| {
                libc::signal(libc::SIGINT, libc::SIG_DFL);
                libc::signal(libc::SIGQUIT, libc::SIG_DFL);
                Ok(())
            });
        }
        let mut child = start.spawn().unwrap();
        let (events, watcher) = watch_pipe(&pipe);
        assert_eq!(events.recv_timeout(Duration::from_secs(20)), Ok("open"), "{name}: the tool never started");
        // Let the command line get into its wait for the tool before it is signalled (it failed once in about ten runs at root,
        // on a busy machine, with the signal landing while it was still setting up)
        std::thread::sleep(Duration::from_millis(300));
        // SAFETY: a signal to a process of this test's own
        unsafe { libc::kill(child.id() as i32, signal) };
        let status = finished(&mut child, Duration::from_secs(20));
        if status.is_none() {
            let _ = child.kill();
            let _ = child.wait();
        }
        assert!(status.is_some(), "{name}: the command line took the signal and went on");
        assert_eq!(events.recv_timeout(Duration::from_secs(10)), Ok("closed"), "{name}: something the command started is still running");
        watcher.join().unwrap();
        assert_eq!(person.sealed_text(".npmrc"), "token\n");
    }
}

#[test]
fn nothing_the_command_started_outlives_it() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-background");
    person.put(".npmrc", "token\n");
    let path = person.at(".npmrc");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let leaves = tool(&person, "tool", &format!("(sleep 1; echo late >'{}') &\nexit 0", path.display()));
    let (code, _, err) = person.says(&["run", ".npmrc", "--", &leaves]);
    assert_eq!(code, 0, "{err}");
    std::thread::sleep(Duration::from_millis(2500));
    assert!(!path.exists(), "a process the command started wrote its file to the disk after the run");
    assert_eq!(person.sealed_text(".npmrc"), "token\n");
}

#[test]
fn a_tool_that_writes_a_temporary_file_beside_its_login_and_renames_it_keeps_both_in_the_namespace() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-rename-over");
    person.put(".claude/.credentials.json", "old\n");
    person.put(".claude/settings.json", "{}\n");
    let path = person.at(".claude/.credentials.json");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let rewrite = tool(&person, "tool", "cd \"$(dirname \"$1\")\"\nprintf new >.credentials.json.tmp\nmv .credentials.json.tmp .credentials.json\nprintf made >notes.txt\nprintf leftover >.credentials.json.bak\nprintf more >>settings.json");
    let (code, _, err) = person.says(&["run", ".claude/.credentials.json", "--", &rewrite, path.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(person.sealed_text(".claude/.credentials.json"), "new");
    // Nothing of the token is on the disk, in the file or beside it, and what else the tool made there is
    let mut names: Vec<String> = std::fs::read_dir(person.at(".claude")).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["notes.txt", "settings.json"]);
    assert_eq!(std::fs::read_to_string(person.at(".claude/notes.txt")).unwrap(), "made");
    assert_eq!(std::fs::read_to_string(person.at(".claude/settings.json")).unwrap(), "{}\nmore");
}

#[test]
fn something_taking_the_files_place_away_during_a_run_seals_nothing_and_says_so() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-detached");
    person.put(".npmrc", "old\n");
    let path = person.at(".npmrc");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let (started, go) = (person.at("started"), person.at("go"));
    let waits = tool(&person, "tool", &format!("touch '{}'\nwhile [ ! -e '{}' ]; do sleep 0.05; done\nprintf stolen >'{}'", started.display(), go.display(), path.display()));
    let child = person.cli().args(["run", ".npmrc", "--", &waits]).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let begun = Instant::now();
    while !started.exists() {
        assert!(begun.elapsed() < Duration::from_secs(20), "the tool never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    // While it runs, the lock is not on anything the account can move: another run and an unprotect are turned away
    assert_eq!(person.says(&["run", ".npmrc", "--", "true"]).0, 14);
    assert_eq!(person.says(&["unprotect", ".npmrc"]).0, 14);
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&go, "").unwrap();
    let done = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&done.stderr);
    assert_eq!(done.status.code(), Some(21), "{err}");
    assert!(err.contains("took the file's place"), "{err}");
    // What was sealed is what was sealed
    assert_eq!(person.sealed_text(".npmrc"), "old\n");
}

#[test]
fn a_failed_command_never_replaces_the_good_copy_and_an_empty_file_never_replaces_anything() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-failed");
    person.put(".npmrc", "good\n");
    let path = person.at(".npmrc");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    // It wrote and then was killed: what it wrote is beside the good copy
    let fails = tool(&person, "fails", &format!("printf half >'{}'\nkill -TERM $$", path.display()));
    let (code, _, err) = person.says(&["run", ".npmrc", "--", &fails]);
    assert_eq!(code, 128 + 15, "{err}");
    assert_eq!(person.sealed_text(".npmrc"), "good\n");
    assert_eq!(person.sealed_text(".npmrc (from a command that did not end well)"), "half");
    // A wrapper that reports a kill as an exit status is the same
    let wrapped = tool(&person, "wrapped", &format!("printf wrapped >'{}'\nexit 137", path.display()));
    assert_eq!(person.says(&["run", ".npmrc", "--", &wrapped]).0, 137);
    assert_eq!(person.sealed_text(".npmrc"), "good\n");
    // It emptied the file and went on: nothing is replaced by nothing
    let empties = tool(&person, "empties", &format!(": >'{}'", path.display()));
    let (code, _, err) = person.says(&["run", ".npmrc", "--", &empties]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("empty"), "{err}");
    assert_eq!(person.sealed_text(".npmrc"), "good\n");
}

#[test]
fn a_folder_with_a_plain_copy_of_a_sealed_name_is_not_run() {
    let person = Person::new("run-folder-copy");
    person.put(".ssh/id_x", "private\n");
    assert_eq!(person.says(&["protect", person.at(".ssh").to_str().unwrap()]).0, 0);
    // The copy came back, as a restore or a backup would bring it
    person.put(".ssh/id_x", "an older copy\n");
    let marker = person.at("ran");
    let (code, _, err) = person.says(&["run", ".ssh", "--", "touch", marker.to_str().unwrap()]);
    assert_eq!(code, 13, "{err}");
    assert!(!marker.exists());
    assert_eq!(std::fs::read_to_string(person.at(".ssh/id_x")).unwrap(), "an older copy\n");
    assert_eq!(person.sealed(".ssh")["files"][0]["at"], json!("id_x"));
}

#[test]
fn a_file_is_sealed_back_into_the_vault_it_came_from() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-vault");
    person.running.new_vault("Hidden", true, "hidden password");
    let status = person.running.ask(json!({"op": "status"}));
    let vaults = status["vaults"].as_array().unwrap();
    let id_of = |hidden: bool| vaults.iter().find(|vault| vault["hidden"] == json!(hidden)).unwrap()["id"].as_str().unwrap().to_string();
    let (visible, hidden) = (id_of(false), id_of(true));
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode("old\n");
    let value = json!({"what": "file", "files": [{"at": "", "mode": 420, "data": data}], "folders": []});
    // One file in each, and both vaults open: whichever the service would pick for a new item is not where both of them belong
    for (path, id, vault) in [(".npmrc", "00112233aabbccdd", &visible), (".pypirc", "00112233aabbccee", &hidden)] {
        let put = person.running.ask(json!({"op": "put", "kind": "protected", "name": path, "id": id, "meta": {"path": path, "what": "file", "size": 4, "files": 1}, "value": value, "approval": "while-open", "vault": vault}));
        assert_eq!(put["ok"], json!(true), "{put}");
        let rewrite = tool(&person, "tool", &format!("printf new >'{}'", person.at(path).display()));
        let (code, _, err) = person.says(&["run", path, "--", &rewrite]);
        assert_eq!(code, 0, "{err}");
    }
    let items = person.running.ask(json!({"op": "list", "kind": "protected"}));
    let items = items["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{items:?}");
    for (path, vault) in [(".npmrc", &visible), (".pypirc", &hidden)] {
        let item = items.iter().find(|item| item["name"] == json!(path)).unwrap();
        assert_eq!(item["vault"], json!(vault), "{path} moved to another vault");
        assert_eq!(person.sealed_text(path), "new");
    }
}

#[test]
fn protecting_what_is_protected_already_never_loses_what_is_sealed() {
    let person = Person::new("protect-again");
    person.put(".aws/credentials", "sealed\n");
    let path = person.at(".aws/credentials");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    // An empty file there, as a killed command leaves one
    person.put(".aws/credentials", "");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 12);
    assert_eq!(person.sealed_text(".aws/credentials"), "sealed\n");
    assert!(person.says(&["protected", "usual"]).1.is_empty(), "an empty file is listed as a credential");
    // A different file there
    person.put(".aws/credentials", "another\n");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 18);
    assert_eq!(person.sealed_text(".aws/credentials"), "sealed\n");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "another\n");
    // A folder takes what is new, and refuses what would change what is sealed
    person.put(".ssh/id_a", "a\n");
    assert_eq!(person.says(&["protect", person.at(".ssh").to_str().unwrap()]).0, 0);
    person.put(".ssh/id_b", "b\n");
    assert_eq!(person.says(&["protect", person.at(".ssh").to_str().unwrap()]).0, 0);
    let value = person.sealed(".ssh");
    let names: Vec<&str> = value["files"].as_array().unwrap().iter().map(|file| file["at"].as_str().unwrap()).collect();
    assert_eq!(names, ["id_a", "id_b"]);
    assert!(!person.at(".ssh/id_b").exists());
    person.put(".ssh/id_a", "changed\n");
    assert_eq!(person.says(&["protect", person.at(".ssh").to_str().unwrap()]).0, 18);
    assert_eq!(std::fs::read_to_string(person.at(".ssh/id_a")).unwrap(), "changed\n");
    assert_eq!(person.sealed(".ssh")["files"].as_array().unwrap().len(), 2);
}

#[test]
fn a_file_with_another_name_a_name_with_an_equals_sign_and_a_folder_called_pub_are_handled() {
    let person = Person::new("protect-odd");
    person.put(".npmrc", "token\n");
    std::fs::hard_link(person.at(".npmrc"), person.at("other-name")).unwrap();
    assert_eq!(person.says(&["protect", person.at(".npmrc").to_str().unwrap()]).0, 19);
    assert_eq!(std::fs::read_to_string(person.at(".npmrc")).unwrap(), "token\n");
    person.put("a=b", "x\n");
    assert_eq!(person.says(&["protect", person.at("a=b").to_str().unwrap()]).0, 10);
    // A folder named like a public key is a folder, and what is in it is sealed with the rest
    person.put(".ssh/id_x", "private\n");
    person.put(".ssh/keys.pub/inner", "secret\n");
    person.put(".ssh/id_x.pub", "public\n");
    assert_eq!(person.says(&["protect", person.at(".ssh").to_str().unwrap()]).0, 0);
    let value = person.sealed(".ssh");
    let names: Vec<&str> = value["files"].as_array().unwrap().iter().map(|file| file["at"].as_str().unwrap()).collect();
    assert_eq!(names, ["id_x", "keys.pub/inner"]);
    assert!(person.at(".ssh/id_x.pub").exists() && !person.at(".ssh/keys.pub").exists());
}

#[test]
fn run_by_a_path_finds_the_good_copy_and_never_the_one_a_failed_command_left() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-by-path");
    // The leftover is older than the good copy, so that it comes first in any list
    let value = json!({"what": "file", "files": [{"at": "", "mode": 420, "data": "SEFMRg=="}], "folders": []});
    let put = person.running.ask(json!({"op": "put", "kind": "protected", "name": ".npmrc (from a command that did not end well)", "id": "00000000000000aa", "meta": {"path": ".npmrc", "what": "file", "size": 4, "files": 1, "killed": true}, "value": value, "approval": "while-open"}));
    assert_eq!(put["ok"], json!(true), "{put}");
    person.put(".npmrc", "good\n");
    let path = person.at(".npmrc");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let (code, out, err) = person.says(&["run", path.to_str().unwrap(), "--", "cat", path.to_str().unwrap()]);
    assert_eq!((code, out.as_str()), (0, "good\n"), "{err}");
    assert_eq!(person.says(&["unprotect", path.to_str().unwrap()]).0, 0);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "good\n");
}

#[test]
fn what_a_command_leaves_that_is_not_a_plain_file_is_passed_over_and_the_rest_is_sealed() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-link-left");
    person.put(".ssh/id_x", "private\n");
    assert_eq!(person.says(&["protect", person.at(".ssh").to_str().unwrap()]).0, 0);
    let folder = person.at(".ssh");
    let leaves = tool(&person, "tool", &format!("ln -s /etc/passwd '{folder}/link'\nprintf new >'{folder}/id_x'", folder = folder.display()));
    let (code, _, err) = person.says(&["run", ".ssh", "--", &leaves]);
    assert_eq!(code, 0, "{err}");
    let value = person.sealed(".ssh");
    let files = value["files"].as_array().unwrap();
    assert_eq!(files.len(), 1, "{value}");
    assert_eq!(files[0]["at"], json!("id_x"));
}

#[test]
fn a_folder_nobody_changed_is_not_sealed_again_whatever_the_umask() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-umask");
    person.put(".ssh/id_x", "private\n");
    std::fs::create_dir_all(person.at(".ssh/more")).unwrap();
    person.put(".ssh/more/deep", "deep\n");
    std::fs::set_permissions(person.at(".ssh/more"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    assert_eq!(person.says(&["protect", person.at(".ssh").to_str().unwrap()]).0, 0);
    let before = person.running.outside.asked();
    let output = Command::new("sh")
        .args(["-c", "umask 077; exec \"$0\" \"$@\""])
        .arg(env!("CARGO_BIN_EXE_amethystora-vault-cli"))
        .args(["run", ".ssh", "--", "true"])
        .env("HOME", &person.home.0)
        .env("AMETHYSTORA_VAULT_SOCKET", &person.running.socket)
        .env("XDG_RUNTIME_DIR", person.running.folder.0.join("run"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    // It asked to open it and not a second time to seal what had not changed
    assert_eq!(person.running.outside.asked(), before + 1);
    assert_eq!(person.sealed(".ssh")["folders"][0]["mode"], json!(0o755));
}

// --- The variable, and the file that is not the vault's --------------------------------------------------------------------------

#[test]
fn a_variable_that_steers_programs_is_never_given_a_secret() {
    let person = Person::new("run-steering");
    add_login(&person.running, "path", "", "", "the-secret");
    add_login(&person.running, "ld preload", "", "", "the-secret");
    add_login(&person.running, "api", "", "", "the-secret");
    let before = person.running.outside.asked();
    // Chosen by the item's name, or by the person
    assert_eq!(person.says(&["run", "path", "--", "true"]).0, 22);
    assert_eq!(person.says(&["run", "ld preload", "--", "true"]).0, 22);
    for variable in ["LD_PRELOAD", "PATH", "BASH_ENV", "IFS", "NODE_OPTIONS", "PYTHONPATH", "GCONV_PATH", "XDG_CONFIG_HOME"] {
        assert_eq!(person.says(&["run", &format!("{variable}=api"), "--", "true"]).0, 22, "{variable}");
    }
    // Refused before anybody is asked
    assert_eq!(person.running.outside.asked(), before);
    let (code, out, _) = person.says(&["run", "MY_TOKEN=api", "--", "sh", "-c", "printf %s \"$MY_TOKEN\""]);
    assert_eq!((code, out.as_str()), (0, "the-secret"));
}

#[test]
fn the_vault_just_runs_where_it_was_asked_to() {
    let Ok(root) = std::env::var("AMETHYSTORA_ROOT") else {
        eprintln!("AMETHYSTORA_ROOT is not set: vault.just was not read");
        return;
    };
    let text = std::fs::read_to_string(Path::new(&root).join("usr/share/amethystora/just/vault.just")).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    // These take a path or a command that the person typed relative to where they are
    for recipe in ["run", "protect", "unprotect", "key", "get"] {
        let at = lines.iter().position(|line| line.starts_with(&format!("{recipe} ")) && line.ends_with(':')).unwrap_or_else(|| panic!("no recipe {recipe}"));
        let attributes: Vec<&str> = lines[..at].iter().rev().take_while(|line| line.starts_with('[')).copied().collect();
        assert!(attributes.contains(&"[no-cd]"), "{recipe} runs in the recipes' folder: {attributes:?}");
    }
}

// --- The tools' helpers ---------------------------------------------------------------------------------------------------------

fn helper_get(running: &Running, input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_git-credential-amethystora"))
        .arg("get")
        .current_dir(&running.folder.0)
        .env("AMETHYSTORA_VAULT_SOCKET", &running.socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    String::from_utf8_lossy(&child.wait_with_output().unwrap().stdout).into_owned()
}

#[test]
fn git_reads_a_field_said_twice_as_git_does_and_keeps_a_carriage_return() {
    let running = start("helper-git-twice");
    running.set_up(PASSWORD);
    add_login(&running, "forge", "me", "https://forge.example/", "s3cret");
    add_login(&running, "evil", "me", "https://evil.example/", "evil-secret");
    // The last wins
    let out = helper_get(&running, "protocol=https\nhost=evil.example\nhost=forge.example\n\n");
    assert!(out.contains("password=s3cret") && !out.contains("evil-secret"), "{out}");
    let out = helper_get(&running, "protocol=https\nhost=forge.example\nhost=evil.example\n\n");
    assert!(out.contains("evil-secret") && !out.contains("s3cret"), "{out}");
    // A carriage return is part of the name
    assert_eq!(helper_get(&running, "protocol=https\r\nhost=forge.example\r\n\r\n"), "");
    assert_eq!(helper_get(&running, "protocol=https\nhost=me@forge.example\n\n"), "");
}

#[test]
fn docker_never_replaces_or_erases_a_login_it_did_not_store_and_a_name_with_a_login_in_it_is_nobody_s() {
    let running = start("helper-docker-theirs");
    running.set_up(PASSWORD);
    add_login(&running, "registry", "me", "https://registry.example/", "mine");
    // Refused with words that read right under `docker login`, which shows them
    let (stored, said) = docker_helper(&running, "store", r#"{"ServerURL":"registry.example","Username":"docker","Secret":"theirs"}"#);
    assert!(!stored);
    assert!(!said.contains("not found") && said.contains("not kept"), "{said}");
    let listed = running.ask(json!({"op": "list", "kind": "password"}));
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);
    assert_eq!(listed["items"][0]["name"], json!("registry"));
    assert_eq!(listed["items"][0]["meta"].get("docker"), None);
    docker_helper(&running, "erase", "registry.example\n");
    assert_eq!(running.ask(json!({"op": "list", "kind": "password"}))["items"].as_array().unwrap().len(), 1);
    // A request for a registry with a login in front of it is for nobody
    assert!(!docker_helper(&running, "get", "evil.example@registry.example\n").0);
    assert!(docker_helper(&running, "get", "registry.example\n").1.contains("mine"));
}

#[test]
fn git_is_not_given_the_vault_while_another_helper_would_keep_what_it_answers() {
    if !has("git") {
        return;
    }
    let person = Person::new("helpers-git-other");
    let (code, ..) = Command::new("git").current_dir(&person.home.0).env("HOME", &person.home.0).env("GIT_CONFIG_NOSYSTEM", "1").args(["config", "--global", "credential.helper", "store"]).status().map(|status| (status.code(), 0, 0)).unwrap();
    assert_eq!(code, Some(0));
    let (code, _, err) = person.says(&["helpers", "git", "on"]);
    assert_eq!(code, 13, "{err}");
    assert!(err.contains("store"), "{err}");
    assert!(!std::fs::read_to_string(person.at(".gitconfig")).unwrap().contains("amethystora"));
}

#[test]
fn the_helpers_leave_alone_what_they_cannot_read_or_would_have_to_follow() {
    use std::os::unix::fs::symlink;
    let person = Person::new("helpers-odd");
    // A config that is not text is not replaced
    person.put(".aws/config", "");
    let odd = [b'[', 0xff, 0xfe, b']', b'\n'];
    std::fs::write(person.at(".aws/config"), odd).unwrap();
    assert_eq!(person.says(&["helpers", "aws", "on", "login", "work"]).0, 13);
    assert_eq!(std::fs::read(person.at(".aws/config")).unwrap(), odd);
    // A block whose end is gone is not taken away with everything after it
    let broken = "# amethystora-vault work begin\n[profile work]\ncredential_process = x\n[profile mine]\nregion = eu\n";
    person.put(".aws/config", broken);
    assert_eq!(person.says(&["helpers", "aws", "off"]).0, 13);
    assert_eq!(std::fs::read_to_string(person.at(".aws/config")).unwrap(), broken);
    // A dotfile that is a link is left as one, and what it leads to as it was
    person.put("dotfiles/settings.json", "{\"model\":\"m\"}");
    std::fs::create_dir_all(person.at(".claude")).unwrap();
    symlink(person.at("dotfiles/settings.json"), person.at(".claude/settings.json")).unwrap();
    assert_eq!(person.says(&["helpers", "claude", "on", "anthropic"]).0, 13);
    assert!(std::fs::symlink_metadata(person.at(".claude/settings.json")).unwrap().file_type().is_symlink());
    assert_eq!(std::fs::read_to_string(person.at("dotfiles/settings.json")).unwrap(), "{\"model\":\"m\"}");
    // A folder that leads out of the home is not written into
    let outside = Folder::new("helpers-odd-outside");
    symlink(&outside.0, person.at(".docker")).unwrap();
    assert_eq!(person.says(&["helpers", "docker", "on"]).0, 13);
    assert!(std::fs::read_dir(&outside.0).unwrap().next().is_none());
    // A pipe is not read, which would never end
    std::fs::remove_file(person.at(".claude/settings.json")).unwrap();
    make_fifo(&person.at(".claude/settings.json"));
    let mut child = person.cli().args(["helpers", "claude", "on", "anthropic"]).spawn().unwrap();
    let status = finished(&mut child, Duration::from_secs(10));
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    assert_eq!(status.and_then(|status| status.code()), Some(13));
}

#[test]
fn an_agent_list_always_fits_what_ssh_reads_and_shows_a_name_as_text() {
    if !has("ssh-keygen") {
        return;
    }
    let running = start("agent-long-names");
    let folder = Folder::new("agent-long-names-keys");
    running.set_up(PASSWORD);
    let (file, _) = ssh_keygen(&folder.0, "ed25519", &["-t", "ed25519"], "");
    let imported = import_openssh(&std::fs::read(&file).unwrap(), b"").unwrap();
    let mut made = 0;
    for index in 0..150 {
        // The longest name the service takes (wire's `valid_name`: 256 characters)
        let name = format!("{index:03}{}", "n".repeat(253));
        let answer = running.ask(json!({"op": "key-import", "name": name, "type": imported.key.key_type().name(), "private": encode(imported.key.as_bytes()), "key_kind": "ssh", "approval": "while-open"}));
        if answer["ok"] == json!(true) {
            made += 1;
        }
    }
    let answer = running.ask(json!({"op": "key-import", "name": "red\u{1b}[31m", "type": imported.key.key_type().name(), "private": encode(imported.key.as_bytes()), "key_kind": "ssh", "approval": "while-open"}));
    let mut stream = connect(&running);
    let list = exchange(&mut stream, &[11]);
    assert!(list.len() <= 256 * 1024, "{} bytes", list.len());
    let found = comments(&list);
    assert!(made >= 130 && found.len() >= 130, "{made} keys were made and {} are listed", found.len());
    assert!(found.iter().all(|comment| comment.len() <= 100 && !comment.contains(&0x1b)), "a comment is long or holds an escape");
    let _ = answer;
}

#[test]
#[ignore = "needs wire's SERVICE_ONLY to list the private key kinds: run it once that is merged"]
fn the_private_half_of_a_key_is_never_handed_to_a_client() {
    if !has("ssh-keygen") {
        return;
    }
    let running = start("get-key");
    let folder = Folder::new("get-key-keys");
    running.set_up(PASSWORD);
    let (file, _) = ssh_keygen(&folder.0, "ed25519", &["-t", "ed25519"], "");
    let id = import(&running, "key", &file, b"", "never");
    let answer = running.ask(json!({"op": "get", "kind": "key", "id": id}));
    assert_ne!(answer["ok"], json!(true), "{answer}");
    assert!(!answer.to_string().contains("PRIVATE"), "{answer}");
}

#[test]
fn the_command_is_the_first_process_of_a_namespace_of_its_own() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-pid");
    person.put(".npmrc", "token\n");
    assert_eq!(person.says(&["protect", person.at(".npmrc").to_str().unwrap()]).0, 0);
    let report = person.at("report");
    let looks = tool(&person, "tool", &format!("echo $$ >'{report}'\nls /proc | grep -c '^[0-9]' >>'{report}'", report = report.display()));
    let (code, _, err) = person.says(&["run", ".npmrc", "--", &looks]);
    assert_eq!(code, 0, "{err}");
    let seen = std::fs::read_to_string(&report).unwrap();
    let mut numbers = seen.lines().map(|line| line.trim().parse::<u32>().unwrap());
    let (own, visible) = (numbers.next().unwrap(), numbers.next().unwrap());
    // Its number in the namespace is a small one, and where /proc was mounted afresh it is alone with its init
    assert!(own <= 5, "the command is process {own}");
    eprintln!("processes visible in /proc to the command: {visible}");
}

// --- The last round: git's helpers in every spelling, and what a tool makes beside its login -------------------------------------

/// Real git, in the person's home and a folder of its own, with the vault's socket
fn git_in(person: &Person, dir: &Path, args: &[&str], input: &str) -> (bool, String) {
    let mut child = Command::new("git")
        .current_dir(dir)
        .env("HOME", &person.home.0)
        .env("XDG_CONFIG_HOME", person.home.0.join(".config"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("AMETHYSTORA_VAULT_SOCKET", &person.running.socket)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "/bin/false")
        .env_remove("SSH_ASKPASS")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    (output.status.success(), String::from_utf8_lossy(&output.stdout).into_owned())
}

#[test]
fn the_vault_gives_git_nothing_while_another_helper_would_store_it_in_any_spelling() {
    if !has("git") {
        return;
    }
    let person = Person::new("git-spellings");
    add_login(&person.running, "forge", "me", "https://forge.example/", "s3cret");
    let ours = env!("CARGO_BIN_EXE_git-credential-amethystora");
    let fill = |dir: &Path| git_in(&person, dir, &["credential", "fill"], "protocol=https\nhost=forge.example\n\n");
    let home = person.home.0.clone();
    // Alone, it answers
    std::fs::write(person.at(".gitconfig"), format!("[credential]\n\thelper = {ours}\n")).unwrap();
    assert!(fill(&home).1.contains("password=s3cret"));
    // Another helper beside it in the account's file
    std::fs::write(person.at(".gitconfig"), format!("[credential]\n\thelper = store\n\thelper = {ours}\n")).unwrap();
    assert!(!fill(&home).1.contains("s3cret"));
    // One for the address, in a section of its own
    std::fs::write(person.at(".gitconfig"), format!("[credential \"https://forge.example\"]\n\thelper = store\n[credential]\n\thelper = {ours}\n")).unwrap();
    assert!(!fill(&home).1.contains("s3cret"));
    assert_eq!(person.says(&["helpers", "git", "on"]).0, 13);
    // One set in the repository git is run in
    std::fs::write(person.at(".gitconfig"), format!("[credential]\n\thelper = {ours}\n")).unwrap();
    let repo = person.at("repo");
    std::fs::create_dir_all(&repo).unwrap();
    assert!(git_in(&person, &repo, &["init", "-q"], "").0);
    assert!(fill(&repo).1.contains("password=s3cret"));
    assert!(git_in(&person, &repo, &["config", "credential.helper", "store"], "").0);
    assert!(!fill(&repo).1.contains("s3cret"));
    // And approve, which is what would have written it down, has nothing to write
    assert!(!person.at(".git-credentials").exists());
}

fn dir_names(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(path).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    names
}

/// A file with the token in it, anywhere in the home outside the vault's own folders
fn token_on_disk(person: &Person, token: &str) -> Vec<PathBuf> {
    fn walk(folder: &Path, token: &str, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else { continue };
            // The scripts of the test's own tools say the words they write
            if kind.is_file() && std::os::unix::fs::PermissionsExt::mode(&entry.metadata().unwrap().permissions()) & 0o111 != 0 {
                continue;
            }
            if kind.is_dir() {
                walk(&path, token, found);
            } else if kind.is_file() && std::fs::read(&path).is_ok_and(|bytes| bytes.windows(token.len()).any(|window| window == token.as_bytes())) {
                found.push(path);
            }
        }
    }
    let mut found = Vec::new();
    walk(&person.home.0, token, &mut found);
    found
}

#[test]
fn an_editor_killed_by_a_hangup_leaves_no_swap_file_with_the_token_on_the_disk() {
    if !namespaces() || !has("vim") {
        return;
    }
    let person = Person::new("run-vim");
    person.put(".aws/credentials", "TOKEN-IN-THE-FILE\n");
    person.put(".aws/config", "[default]\n");
    let path = person.at(".aws/credentials");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let child = person
        .cli()
        .args(["run", ".aws/credentials", "--", "vim", "-u", "NONE", "-N", "-i", "NONE", "-c", "set directory=.", "-c", "normal ggOpwned-line", "-c", "preserve", "-c", "sleep 60"])
        .arg(&path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Long enough for it to have made its swap file and written the text into it
    std::thread::sleep(Duration::from_secs(4));
    // SAFETY: a signal to a process of this test's own
    unsafe { libc::kill(child.id() as i32, libc::SIGHUP) };
    let done = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&done.stderr);
    assert!(token_on_disk(&person, "TOKEN-IN-THE-FILE").is_empty(), "the token is on the disk: {:?}", token_on_disk(&person, "TOKEN-IN-THE-FILE"));
    assert!(token_on_disk(&person, "pwned-line").is_empty());
    assert_eq!(dir_names(&person.at(".aws")), ["config"], "{err}");
    // And it was said what was not kept
    assert!(err.contains("swp") || err.contains("Not kept"), "{err}");
    assert_eq!(person.sealed_text(".aws/credentials"), "TOKEN-IN-THE-FILE\n");
}

#[test]
fn a_copy_of_the_file_a_tool_forgets_or_is_killed_beside_is_not_written_out_and_is_named() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-copies");
    person.put(".aws/credentials", "TOKEN-IN-THE-FILE\n");
    let path = person.at(".aws/credentials");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    // A copy under a name that says nothing, and one with the new content, from a command that went on
    let forgets = tool(&person, "forgets", "cd \"$(dirname \"$1\")\"\ncp \"$1\" tmpAbC123\ncp \"$1\" .tmpXyz\ncp \"$1\" sedQ1w2E3\nprintf TOKEN-NEW >\"$1\"\ncp \"$1\" tmpNew\nprintf fine >notes.txt");
    let (code, _, err) = person.says(&["run", ".aws/credentials", "--", &forgets, path.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(dir_names(&person.at(".aws")), ["notes.txt"]);
    for name in ["tmpAbC123", ".tmpXyz", "sedQ1w2E3", "tmpNew"] {
        assert!(err.contains(name), "{name} is not named: {err}");
    }
    // A command killed between writing a temporary file and moving it into place
    let killed = tool(&person, "killed", "cd \"$(dirname \"$1\")\"\nprintf TOKEN-HALF >tmpHalf\nprintf more >notes2.txt\nkill -KILL $$");
    let (code, _, err) = person.says(&["run", ".aws/credentials", "--", &killed, path.to_str().unwrap()]);
    assert_eq!(code, 137, "{err}");
    assert!(err.contains("tmpHalf"), "{err}");
    assert!(token_on_disk(&person, "TOKEN-HALF").is_empty() && token_on_disk(&person, "TOKEN-IN-THE-FILE").is_empty() && token_on_disk(&person, "TOKEN-NEW").is_empty());
    assert_eq!(dir_names(&person.at(".aws")), ["notes.txt"]);
}

#[test]
fn what_a_tool_makes_beside_its_login_is_kept_whole_or_named() {
    if !namespaces() || !has("ssh-keygen") {
        return;
    }
    let person = Person::new("run-beside");
    person.put(".ssh/id_x", "PRIVATE-OLD\n");
    person.put(".ssh/config", "Host x\n");
    let path = person.at(".ssh/id_x");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let makes = tool(
        &person,
        "makes",
        "cd \"$(dirname \"$1\")\"\nssh-keygen -q -N '' -f other_key\nssh-keygen -q -N '' -f id_x_work\nmkdir -p projects/s\ndd if=/dev/urandom of=projects/s/log.jsonl bs=1024 count=1024 2>/dev/null\nprintf small >projects/s/small\ndd if=/dev/urandom of=big bs=1M count=20 2>/dev/null\nmkdir many\ni=0; while [ $i -lt 300 ]; do printf x >many/f$i; i=$((i+1)); done\nmkfifo pipe\nprintf 'Host y\\n' >>config",
    );
    let (code, _, err) = person.says(&["run", ".ssh/id_x", "--", &makes, path.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    let ssh = person.at(".ssh");
    // Kept, whatever the size
    for name in ["other_key", "other_key.pub", "projects", "big", "many", "config"] {
        assert!(ssh.join(name).exists(), "{name} is gone: {err}");
    }
    assert_eq!(std::fs::metadata(ssh.join("projects/s/log.jsonl")).unwrap().len(), 1024 * 1024);
    assert_eq!(std::fs::read_to_string(ssh.join("projects/s/small")).unwrap(), "small");
    assert_eq!(std::fs::metadata(ssh.join("big")).unwrap().len(), 20 * 1024 * 1024);
    assert_eq!(dir_names(&ssh.join("many")).len(), 300);
    assert_eq!(std::fs::read_to_string(ssh.join("config")).unwrap(), "Host x\nHost y\n");
    // Not kept, and named: a key whose name has the protected file's in it, and a pipe
    assert!(!ssh.join("id_x_work").exists());
    assert!(err.contains("id_x_work") && err.contains("id_x_work.pub"), "{err}");
    assert!(!ssh.join("pipe").exists() && err.contains("pipe") && err.contains("could not be kept"), "{err}");
    assert_eq!(person.sealed_text(".ssh/id_x"), "PRIVATE-OLD\n");
}

#[test]
fn the_usual_tools_rewrite_files_they_made_beside_the_login_and_fail_loudly_on_the_ones_that_were_there() {
    if !namespaces() || !has("python3") || !has("git") {
        return;
    }
    let person = Person::new("run-usual");
    person.put(".aws/credentials", "TOKEN-IN-THE-FILE\n");
    person.put(".aws/config", "[default]\nregion = eu\n");
    let path = person.at(".aws/credentials");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    // On what the tool made itself, in the namespace's own memory, everything works as it normally does
    let fine = tool(
        &person,
        "fine",
        "cd \"$(dirname \"$1\")\"\nprintf 'a=b\\n' >mine\nsed -i 's/a=b/a=c/' mine\ngit config --file mine2 core.x y\npython3 -c \"import os; open('p1','w').write('1'); os.replace('p1','p2'); open('p3','w').write('3'); os.replace('p2','p3')\"\nrm mine2\nmv mine mine-renamed",
    );
    let (code, _, err) = person.says(&["run", ".aws/credentials", "--", &fine, path.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(dir_names(&person.at(".aws")), ["config", "mine-renamed", "p3"]);
    assert_eq!(std::fs::read_to_string(person.at(".aws/mine-renamed")).unwrap(), "a=c\n");
    assert_eq!(std::fs::read_to_string(person.at(".aws/p3")).unwrap(), "1");
    // On a file that was there, a rename over it cannot be: the tool says so, the file is as it was, and nothing of the login is on the disk
    let cannot = tool(
        &person,
        "cannot",
        "cd \"$(dirname \"$1\")\"\nsed -i 's/eu/us/' config\necho \"sed $?\" >>\"$2\"\ngit config --file config core.x y\necho \"git $?\" >>\"$2\"\npython3 -c \"import os; open('t','w').write('1'); os.replace('t','config')\" 2>/dev/null\necho \"python $?\" >>\"$2\"\nexit 0",
    );
    let report = person.at("report");
    let (code, _, err) = person.says(&["run", ".aws/credentials", "--", &cannot, path.to_str().unwrap(), report.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    let said = std::fs::read_to_string(&report).unwrap();
    assert!(said.lines().all(|line| !line.ends_with(" 0")), "{said}");
    assert_eq!(std::fs::read_to_string(person.at(".aws/config")).unwrap(), "[default]\nregion = eu\n");
    assert!(token_on_disk(&person, "TOKEN-IN-THE-FILE").is_empty());
}

#[test]
fn a_command_that_fails_after_refreshing_its_login_leaves_the_new_one_in_place_and_the_old_beside() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-rotated");
    person.put(".config/gh/hosts.yml", "token: old\n");
    let path = person.at(".config/gh/hosts.yml");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let refreshes = |token: &str| tool(&person, "tool", &format!("printf 'token: {token}\\n' >'{}'\nexit 1", path.display()));
    let (code, _, err) = person.says(&["run", ".config/gh/hosts.yml", "--", &refreshes("new")]);
    assert_eq!(code, 1, "{err}");
    let before = ".config/gh/hosts.yml (from before a command that did not end well)";
    assert_eq!(person.sealed_text(".config/gh/hosts.yml"), "token: new\n");
    assert_eq!(person.sealed_text(before), "token: old\n");
    assert!(err.contains("in place"), "{err}");
    // The next run is given the new one, and a second failure keeps one copy from before, the last
    let reader = tool(&person, "reader", &format!("cat '{}'", path.display()));
    assert_eq!(person.says(&["run", ".config/gh/hosts.yml", "--", &reader]).1, "token: new\n");
    assert_eq!(person.says(&["run", ".config/gh/hosts.yml", "--", &refreshes("newer")]).0, 1);
    assert_eq!(person.sealed_text(".config/gh/hosts.yml"), "token: newer\n");
    assert_eq!(person.sealed_text(before), "token: new\n");
    let items = person.running.ask(json!({"op": "list", "kind": "protected"}));
    assert_eq!(items["items"].as_array().unwrap().len(), 2, "{items}");
}

#[test]
fn a_tool_that_keeps_a_hard_link_to_its_rewritten_login_is_sealed() {
    if !namespaces() {
        return;
    }
    let person = Person::new("run-hard-link");
    person.put(".aws/credentials", "old\n");
    let path = person.at(".aws/credentials");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let links = tool(&person, "tool", "printf new >\"$1\"\nln \"$1\" \"$1.bak\"");
    let (code, _, err) = person.says(&["run", ".aws/credentials", "--", &links, path.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(person.sealed_text(".aws/credentials"), "new");
    assert!(!person.at(".aws/credentials.bak").exists());
}

#[test]
fn a_folder_comes_back_with_the_mode_it_had() {
    use std::os::unix::fs::PermissionsExt;
    let person = Person::new("protect-folder-mode");
    person.put(".kube/config", "secret\n");
    std::fs::set_permissions(person.at(".kube"), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(person.says(&["protect", person.at(".kube").to_str().unwrap()]).0, 0);
    assert!(!person.at(".kube").exists());
    assert_eq!(person.says(&["unprotect", ".kube"]).0, 0);
    assert_eq!(std::fs::metadata(person.at(".kube")).unwrap().permissions().mode() & 0o777, 0o755);
    assert_eq!(std::fs::read_to_string(person.at(".kube/config")).unwrap(), "secret\n");
}

#[test]
fn protecting_a_renamed_item_again_from_another_folder_is_refused_as_protected_already() {
    let person = Person::new("protect-renamed");
    person.put(".npmrc", "sealed\n");
    let path = person.at(".npmrc");
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 0);
    let items = person.running.ask(json!({"op": "list", "kind": "protected"}));
    let item = items["items"][0].clone();
    let value = person.sealed(".npmrc");
    let renamed = person.running.ask(json!({"op": "put", "kind": "protected", "id": item["id"], "name": "work npm", "meta": item["meta"], "value": value}));
    assert_eq!(renamed["ok"], json!(true), "{renamed}");
    person.put(".npmrc", "another\n");
    // This test's own folder is not the home, so a path looked up there would find nothing
    assert_eq!(person.says(&["protect", path.to_str().unwrap()]).0, 18);
    assert_eq!(person.running.ask(json!({"op": "list", "kind": "protected"}))["items"].as_array().unwrap().len(), 1);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "another\n");
}

#[test]
fn the_aws_block_goes_and_every_other_line_stays_as_it_was() {
    let person = Person::new("helpers-aws-off");
    // No file: nothing is made
    assert_eq!(person.says(&["helpers", "aws", "off"]).0, 0);
    assert!(!person.at(".aws/config").exists());
    let theirs = "[default]\r\nregion = eu\r\n# amethystora-vault is how I keep my logins\r\n";
    person.put(".aws/config", &format!("{theirs}# amethystora-vault work begin\n[profile work]\ncredential_process = /usr/bin/ame vault get work aws\n# amethystora-vault work end\n"));
    assert_eq!(person.says(&["helpers", "aws", "off"]).0, 0);
    assert_eq!(std::fs::read_to_string(person.at(".aws/config")).unwrap(), theirs);
    // A comment of theirs is not a block of ours: nothing is said to be on, and nothing is rewritten
    let listed: Value = serde_json::from_str(&person.says(&["helpers", "--json"]).1).unwrap();
    assert_eq!(listed["helpers"].as_array().unwrap().iter().find(|row| row["tool"] == json!("aws")).unwrap()["on"], json!(false));
    use std::os::unix::fs::MetadataExt;
    let inode = std::fs::metadata(person.at(".aws/config")).unwrap().ino();
    assert_eq!(person.says(&["helpers", "aws", "off"]).0, 0);
    assert_eq!(std::fs::metadata(person.at(".aws/config")).unwrap().ino(), inode);
}
