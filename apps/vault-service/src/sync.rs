//! Sync with a Vaultwarden or Bitwarden server, the service's side (plans/done/PLAN-vault.md, "Sync with Vaultwarden and
//! Bitwarden"; the protocol is `apps/vault-core/src/sync`, written from plans/vault-sync/spec-protocol.md).
//!
//! This service has no network. It reaches `amethystora-vault-net` (its own user, the network, no key) by a socket only this
//! service can open, and that one carries ciphertext, a token and at a password login the master password's hash, for one
//! request at a time. A run is a thread of its own, since a slow server must never hold up anyone's scrypt: it reads what the
//! vault holds on the loop, goes to the server on the thread, seals what it decided into files beside the items' places on the
//! thread too, and puts them in place on the loop a slice at a time, each group only if the items it was decided from are still as
//! they were. A lock stops it at once: nothing more is asked of the server, and the socket to the network service is shut.
//!
//! Sync is of the first vault only, which a hidden vault can never be. It runs only while that vault is open: on opening, a
//! little after a change, and every 15 minutes, never at all while a vault is closed. Its connection, with the account's
//! decrypted key, and one link for each of the server's items are items of the vault, service-only like a data key.

use std::any::Any;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use amethystora_vault_core::protocol::{Approval, Refusal, Request};
use amethystora_vault_core::store::{BatchItem, BatchWriter, Store, Trouble};
use amethystora_vault_core::sync::address::Server;
use amethystora_vault_core::sync::api::{self, Extra, Http, NetFailure, Problem, Request as HttpRequest, Response, MOST_BODY};
use amethystora_vault_core::sync::convert::{CertFacts, Convert, KeyRead, SshForms};
use amethystora_vault_core::sync::crypto::{self, Kdf, KdfProblem};
use amethystora_vault_core::sync::kinds;
use amethystora_vault_core::sync::map::{self, Local};
use amethystora_vault_core::sync::merge::{self, Access, Basis, Connection, Group, Input, Leftover, Link, Outcome, CONNECTION_ID, KIND_CONNECTION, KIND_LINK};
use amethystora_vault_core::vaults::Account;

use crate::outside::{say, Action};
use crate::peer::Peer;
use crate::service::{Next, Service};

pub type Done = Box<dyn Any + Send>;

/// The network service's socket
pub const NET_SOCKET: &str = "/run/amethystora/vault-net.sock";
const METERED: &str = "/usr/libexec/amethystora-metered";
/// How often a connected, open vault looks at the server
const INTERVAL: Duration = Duration::from_secs(15 * 60);
/// A change is sent a little after it, so that a burst of them is one run
const DEBOUNCE: Duration = Duration::from_secs(20);
/// A run its timer started waits for a connection that does not count data, but never longer than a week
const WEEK_MS: i64 = 7 * 24 * 3600 * 1000;
/// A request waits this long for the network service, which itself gives up on a server after a minute
const PATIENCE: Duration = Duration::from_secs(130);
/// A run, from its first request to its last, takes this long at most: a slow server holds the vault's key and every secret it
/// gathered for as long as it likes otherwise (the review's H7)
const RUN_LIMIT: Duration = Duration::from_secs(10 * 60);
/// The longest answer the network service sends: the largest body, as base64, and a little JSON
const MOST_ANSWER: usize = MOST_BODY / 3 * 4 + 4096;
/// A run's results are put in place this many groups, or this long, at a time before the loop is given back
const SLICE_GROUPS: usize = 128;
const SLICE_TIME: Duration = Duration::from_millis(5);
/// What one yes to a connection covers: the steps of a login (a code, a confirmation, a trusted certificate), and no more
const MOST_STEPS: u32 = 20;

/// What a request of the person's changes, and so wants a run soon: the plugs that write items. The generic writes call `changed`
/// themselves once they have written to the first vault.
const CHANGING_PLUGS: &[&str] = &["auth-add", "auth-update", "codes-add", "codes-used", "codes-update", "import-merge", "key-new", "key-import", "cert-new", "cert-renew"];

/// Whether a plug of this request writes items that sync, and when nothing but the first vault is open to write them to. With a
/// hidden vault open as well the plug may be writing there, and the run would be a timing the plan forbids; the next interval
/// takes what it wrote.
pub fn plug_changes(service: &mut Service, uid: u32, request: &Request) -> bool {
    matches!(request, Request::Plug { op, .. } if CHANGING_PLUGS.contains(&op.as_str())) && service.open_count(uid) == 1 && service.first_open(uid).is_some()
}

/// The kinds sync keeps for itself, which a program may neither read nor set nor remove: `off` is how a connection ends, and says so to the
/// person in its own words
pub fn owns(kind: &str) -> bool {
    kind == KIND_CONNECTION || kind == KIND_LINK
}

/// Whether this vault is the one a server is connected to
pub fn is_connected(store: &Store) -> bool {
    store.meta.first && store.entry(KIND_CONNECTION, CONNECTION_ID).is_some()
}

// --- Stopping a run -----------------------------------------------------------------------------------------------------

/// What stops a run from outside: the vault closed. Checked before each request, and the socket to the network service is shut, so
/// that a request in flight ends at once.
pub struct Cancel {
    stopped: AtomicBool,
    socket: Mutex<Option<UnixStream>>,
}

impl Cancel {
    pub fn new() -> Arc<Cancel> {
        Arc::new(Cancel { stopped: AtomicBool::new(false), socket: Mutex::new(None) })
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Ok(mut socket) = self.socket.lock()
            && let Some(stream) = socket.take()
        {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }

    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    /// The thread's side of the socket, to be shut by `stop`: at once, if that came first
    fn hold(&self, stream: UnixStream) {
        if let Ok(mut socket) = self.socket.lock() {
            if self.stopped() {
                let _ = stream.shutdown(Shutdown::Both);
            } else {
                *socket = Some(stream);
            }
        }
    }
}


// --- What needs libcrypto --------------------------------------------------------------------------------------------------------

/// What a run asks of `amethystora-vault-work`, one short-lived process for each question: this service never loads libcrypto, and a key,
/// a certificate or a protected file read back from a server's text is a parser's work. A job that fails is an answer of none.
/// An SSH key's three strings, as the worker made them
type SshStrings = (Zeroizing<String>, String, String);

struct Worker {
    program: PathBuf,
    /// An SSH key's strings, asked for once a run: a run compares each cipher that arrives with each item that has none, and a process for
    /// each pair would be thousands
    ssh: std::cell::RefCell<HashMap<String, Option<SshStrings>>>,
}

impl Worker {
    fn ask(&self, request: Value) -> Option<Value> {
        let answer = crate::plugs::work::run(&self.program, &request);
        (answer["ok"] == json!(true) && answer["none"] != json!(true)).then_some(answer)
    }
}

fn encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn decode(text: &Value) -> Option<Zeroizing<Vec<u8>>> {
    base64::engine::general_purpose::STANDARD.decode(text.as_str()?).ok().map(Zeroizing::new)
}

impl Convert for Worker {
    fn ssh_forms(&self, key_type: &str, key: &[u8]) -> Option<SshForms> {
        use sha2::Digest;
        let asked = format!("{key_type}:{}", amethystora_vault_core::seal::hex(&sha2::Sha256::digest(key)));
        let found = match self.ssh.borrow().get(&asked) {
            Some(found) => found.clone(),
            None => {
                let answer = self.ask(json!({"job": "sync-ssh", "type": key_type, "key": encode(key)}));
                let made = answer.and_then(|answer| Some((Zeroizing::new(answer["private"].as_str()?.to_string()), answer["public"].as_str()?.to_string(), answer["fingerprint"].as_str()?.to_string())));
                self.ssh.borrow_mut().insert(asked, made.clone());
                made
            }
        };
        found.map(|(private, public, fingerprint)| SshForms { private, public, fingerprint })
    }

    fn key_from_text(&self, text: &str) -> Option<KeyRead> {
        let answer = self.ask(json!({"job": "sync-key", "text": text}))?;
        Some(KeyRead { key_type: answer["type"].as_str()?.to_string(), key: decode(&answer["key"])?, public: answer["public"].clone(), agree: answer["agree"] == json!(true) })
    }

    fn cert_facts(&self, certs: &[Vec<u8>], key: Option<(&str, &[u8])>) -> Option<CertFacts> {
        let certs: Vec<String> = certs.iter().map(|der| encode(der)).collect();
        let mut request = json!({"job": "sync-certs", "certs": certs});
        if let Some((key_type, bytes)) = key {
            request["type"] = json!(key_type);
            request["key"] = json!(encode(bytes));
        }
        let answer = self.ask(request)?;
        Some(CertFacts { leaf_sha256: answer["leaf_sha256"].as_str()?.to_string(), spki_sha256: answer["spki_sha256"].as_str()?.to_string(), key_matches: answer["key_matches"] == json!(true), chain_ok: answer["chain_ok"] == json!(true) })
    }

    /// A protected file's own parser: the same `Tree::from_value` the command line holds an item to
    fn accepts(&self, kind: &str, value: &Value) -> bool {
        kind != crate::protect::KIND || crate::protect::Tree::from_value(value).is_ok()
    }
}

// --- The network service, as a server ----------------------------------------------------------------------------------

/// The network service's socket, spoken to one request at a time: what `Http` is here
pub struct NetHttp {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    cancel: Arc<Cancel>,
    until: Instant,
}

impl NetHttp {
    fn connect(path: &Path, cancel: Arc<Cancel>, until: Instant) -> Result<NetHttp, NetFailure> {
        if cancel.stopped() {
            return Err(NetFailure::Cancelled);
        }
        let stream = UnixStream::connect(path).map_err(|_| NetFailure::Unreachable)?;
        stream.set_read_timeout(Some(PATIENCE)).and_then(|()| stream.set_write_timeout(Some(PATIENCE))).map_err(|_| NetFailure::Unreachable)?;
        let writer = stream.try_clone().map_err(|_| NetFailure::Unreachable)?;
        cancel.hold(stream.try_clone().map_err(|_| NetFailure::Unreachable)?);
        Ok(NetHttp { reader: BufReader::new(stream), writer, cancel, until })
    }

    /// A session for the connected server alone, with its private CA if the person trusted one
    pub fn open(path: &Path, server: &Server, ca: Option<&str>, pin: Option<&str>, cancel: Arc<Cancel>, until: Instant) -> Result<NetHttp, NetFailure> {
        let mut net = NetHttp::connect(path, cancel, until)?;
        let mut line = json!({"v": 1, "op": "session", "origins": server.origins()});
        if let (Some(ca), Some(pin)) = (ca, pin) {
            line["ca"] = json!(ca);
            line["pin"] = json!(pin);
        }
        net.exchange(&line)?;
        Ok(net)
    }

    fn exchange(&mut self, request: &Value) -> Result<Value, NetFailure> {
        if self.cancel.stopped() {
            return Err(NetFailure::Cancelled);
        }
        let left = self.until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(NetFailure::Timeout);
        }
        // No wait goes past the run's own limit
        self.reader.get_ref().set_read_timeout(Some(left.min(PATIENCE))).map_err(|_| NetFailure::Unreachable)?;
        let text = Zeroizing::new(request.to_string());
        let ended = |net: &NetHttp, failure: NetFailure| if net.cancel.stopped() { NetFailure::Cancelled } else { failure };
        self.writer.write_all(text.as_bytes()).and_then(|()| self.writer.write_all(b"\n")).and_then(|()| self.writer.flush()).map_err(|_| ended(self, NetFailure::Unreachable))?;
        let mut line = Zeroizing::new(Vec::new());
        match (&mut self.reader).take(MOST_ANSWER as u64 + 1).read_until(b'\n', &mut line) {
            Ok(0) => return Err(ended(self, NetFailure::Unreachable)),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock || error.kind() == std::io::ErrorKind::TimedOut => return Err(ended(self, NetFailure::Timeout)),
            Err(_) => return Err(ended(self, NetFailure::Unreachable)),
        }
        if line.len() > MOST_ANSWER || line.last() != Some(&b'\n') {
            return Err(NetFailure::TooLarge);
        }
        let answer: Value = serde_json::from_slice(&line).map_err(|_| NetFailure::Unreachable)?;
        if answer.get("ok") == Some(&Value::Bool(true)) {
            Ok(answer)
        } else {
            Err(NetFailure::from_name(answer.get("why").and_then(Value::as_str).unwrap_or("")))
        }
    }
}

impl Http for NetHttp {
    fn call(&mut self, request: HttpRequest) -> Result<Response, NetFailure> {
        let headers: Vec<Value> = request.headers.iter().map(|(name, value)| json!([name, value])).collect();
        let body = Zeroizing::new(base64::engine::general_purpose::STANDARD.encode(&request.body[..]));
        let answer = self.exchange(&json!({"v": 1, "op": "request", "method": request.method, "url": request.url, "headers": headers, "body": &*body}))?;
        let status = answer.get("status").and_then(Value::as_u64).and_then(|status| u16::try_from(status).ok()).ok_or(NetFailure::Unreachable)?;
        let body = base64::engine::general_purpose::STANDARD.decode(answer.get("body").and_then(Value::as_str).unwrap_or("")).map_err(|_| NetFailure::Unreachable)?;
        Ok(Response { status, body: Zeroizing::new(body) })
    }
}

/// What a TLS server showed, asked of the network service without trusting it (for the person to compare a fingerprint with)
fn probe(path: &Path, origin: &str, cancel: Arc<Cancel>) -> Result<Value, NetFailure> {
    NetHttp::connect(path, cancel, Instant::now() + Duration::from_secs(60))?.exchange(&json!({"v": 1, "op": "probe", "origin": origin}))
}

// --- Argon2, in a child -------------------------------------------------------------------------------------------------

/// The master key. PBKDF2 is made here; Argon2, which takes up to a gigabyte, is made by a short-lived child of this very program,
/// so that the service that holds every account's keys never grows by it, and what it took goes back to the system with the child.
/// The password goes to the child on its standard input, never as an argument or in the environment.
fn derive_in(exe: &Path, password: &[u8], email: &str, kdf: &Kdf) -> Result<Zeroizing<[u8; 32]>, KdfProblem> {
    if kdf.kind != 1 {
        return crypto::master_key(password, email, kdf);
    }
    kdf.check()?;
    let request = Zeroizing::new(json!({"password": base64::engine::general_purpose::STANDARD.encode(password), "email": email, "kdf": kdf}).to_string());
    let mut child = Command::new(exe).arg(KDF_CHILD).env_clear().stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_| KdfProblem::Unknown)?;
    let sent = child.stdin.take().is_some_and(|mut stdin| stdin.write_all(request.as_bytes()).is_ok());
    let mut answer = Zeroizing::new(Vec::new());
    let read = child.stdout.take().is_some_and(|stdout| stdout.take(256).read_to_end(&mut answer).is_ok());
    let status = child.wait().map_err(|_| KdfProblem::Unknown)?;
    if !(sent && read && status.success()) {
        return Err(KdfProblem::Unknown);
    }
    let bytes = Zeroizing::new(amethystora_vault_core::seal::unhex(std::str::from_utf8(&answer).map_err(|_| KdfProblem::Unknown)?.trim()).ok_or(KdfProblem::Unknown)?);
    let key: [u8; 32] = bytes.as_slice().try_into().map_err(|_| KdfProblem::Unknown)?;
    Ok(Zeroizing::new(key))
}

/// The argument the service's own binary takes to be that child
pub const KDF_CHILD: &str = "--sync-kdf";

/// The child: one request on its standard input, the master key in hex on its standard output
pub fn kdf_child() -> ExitCode {
    let mut text = Zeroizing::new(Vec::new());
    if std::io::stdin().take(4096).read_to_end(&mut text).is_err() {
        return ExitCode::FAILURE;
    }
    let Ok(request) = serde_json::from_slice::<Value>(&text) else { return ExitCode::FAILURE };
    let password = request.get("password").and_then(Value::as_str).and_then(|text| base64::engine::general_purpose::STANDARD.decode(text).ok()).map(Zeroizing::new);
    let (Some(password), Some(email), Some(kdf)) = (password, request.get("email").and_then(Value::as_str), request.get("kdf").and_then(|kdf| serde_json::from_value::<Kdf>(kdf.clone()).ok())) else { return ExitCode::FAILURE };
    match crypto::master_key(&password, email, &kdf) {
        Ok(key) => {
            let hex = Zeroizing::new(amethystora_vault_core::seal::hex(&key[..]));
            if std::io::stdout().write_all(hex.as_bytes()).is_ok() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(_) => ExitCode::FAILURE,
    }
}

// --- When, and which runs ---------------------------------------------------------------------------------------------------

/// One account's sync can do one slow thing at a time, a run or a connection step. Held by whatever is doing it, on its
/// thread and in what follows it on the loop, so that a lock and an opening again cannot start a second run before the first
/// has ended.
pub struct Busy(Arc<AtomicBool>);

impl Drop for Busy {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// What a person said yes to, for the connection that asked and for nothing else: a program on another connection, or the same
/// one asking for another address, account or certificate, is asked about anew. It goes with the connection.
struct Approved {
    by: Weak<Peer>,
    address: String,
    email: String,
    pin: Option<String>,
    steps: u32,
}

#[derive(Default)]
struct User {
    /// When the next run is due, if one is
    due: Option<Instant>,
    /// The first run after an opening reads `/sync` whatever the revision date says
    full: bool,
    /// The next run is a person's doing (they connected): it waits for nothing
    manual: bool,
    access: Option<Access>,
    /// A problem that waits for a person: no run starts by itself until one is made
    stopped: Option<Value>,
    approved: Option<Approved>,
}

/// What follows a run the service started itself, on the loop
pub type Follow = Box<dyn FnOnce(&mut Service, Done)>;

pub struct Job {
    pub job: Box<dyn FnOnce() -> Done + Send>,
    pub then: Follow,
}

pub struct Hub {
    pub net: PathBuf,
    pub metered: PathBuf,
    pub interval: Duration,
    pub debounce: Duration,
    /// The program that makes Argon2 keys: this service's own binary
    pub kdf_child: PathBuf,
    /// How long a run may take, from its first request
    pub run_limit: Duration,
    users: HashMap<u32, User>,
    /// Not forgotten when a vault closes: a thread still running is still that account's
    busy: HashMap<u32, Arc<AtomicBool>>,
    /// The run or the connection step each account has going, to be stopped by a lock
    running: HashMap<u32, Arc<Cancel>>,
    jobs: Vec<Job>,
    /// What runs have decided, waiting to be put in place a slice at a time
    applying: VecDeque<Applying>,
}

impl Default for Hub {
    fn default() -> Hub {
        Hub {
            net: PathBuf::from(NET_SOCKET),
            metered: PathBuf::from(METERED),
            interval: INTERVAL,
            debounce: DEBOUNCE,
            kdf_child: PathBuf::from("/proc/self/exe"),
            run_limit: RUN_LIMIT,
            users: HashMap::new(),
            busy: HashMap::new(),
            running: HashMap::new(),
            jobs: Vec::new(),
            applying: VecDeque::new(),
        }
    }
}

impl Hub {
    /// When something is next due: the one deadline sync adds, and none while nothing is connected and open. A result being put in
    /// place is due at once, a slice at a time between the loop's other work.
    pub fn deadline(&self) -> Option<Instant> {
        if !self.applying.is_empty() {
            return Some(Instant::now());
        }
        self.users.values().filter_map(|user| user.due).min()
    }

    /// The first vault of this account opened and has a server: look at it as soon as the loop comes round
    pub fn opened(&mut self, uid: u32) {
        let user = self.users.entry(uid).or_default();
        user.due = Some(Instant::now());
        user.full = true;
    }

    /// The person changed something that syncs: a run a little later, which takes a burst of changes at once. Called once an item of
    /// the first vault has been written, and by nothing else: a request about another vault must not show on the network (the
    /// review's M5).
    pub fn changed(&mut self, uid: u32) {
        if let Some(user) = self.users.get_mut(&uid) {
            let soon = Instant::now() + self.debounce;
            if user.stopped.is_none() {
                user.due = Some(user.due.map_or(soon, |due| due.min(soon)));
            }
        }
    }

    fn stop_running(&mut self, uid: u32) {
        if let Some(cancel) = self.running.remove(&uid) {
            cancel.stop();
        }
        self.applying.retain(|applying| applying.uid != uid);
    }

    /// The vault closed: what its run was doing ends at once, and what it had decided is not put anywhere
    pub fn closed(&mut self, uid: u32) {
        self.stop_running(uid);
        self.users.remove(&uid);
    }

    pub fn closed_all(&mut self) {
        let uids: Vec<u32> = self.running.keys().copied().chain(self.applying.iter().map(|applying| applying.uid)).collect();
        for uid in uids {
            self.stop_running(uid);
        }
        self.users.clear();
    }

    pub fn take_jobs(&mut self) -> Vec<Job> {
        std::mem::take(&mut self.jobs)
    }

    fn take_busy(&mut self, uid: u32) -> Option<Arc<Busy>> {
        let flag = self.busy.entry(uid).or_default();
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).ok()?;
        Some(Arc::new(Busy(flag.clone())))
    }

    /// A cancel for the thing an account is about to do, known to the hub so that a lock can stop it
    fn start_running(&mut self, uid: u32) -> Arc<Cancel> {
        let cancel = Cancel::new();
        self.running.insert(uid, cancel.clone());
        cancel
    }

    /// The person said yes to this: for the connection that asked, and nothing else
    fn approve(&mut self, peer: &Rc<Peer>, address: &str, email: &str, pin: Option<&str>) {
        let user = self.users.entry(peer.uid).or_default();
        user.approved = Some(Approved { by: Rc::downgrade(peer), address: address.to_string(), email: email.to_string(), pin: pin.map(str::to_string), steps: 0 });
    }

    /// Whether this connection was said yes to for exactly this address, account and certificate: the steps of one login take it
    /// again without asking, a few times
    fn approved(&mut self, peer: &Rc<Peer>, address: &str, email: &str, pin: Option<&str>) -> bool {
        let Some(approved) = self.users.get_mut(&peer.uid).and_then(|user| user.approved.as_mut()) else { return false };
        let same = approved.by.upgrade().is_some_and(|by| Rc::ptr_eq(&by, peer)) && approved.address == address && approved.email == email && approved.pin.as_deref() == pin;
        if same && approved.steps < MOST_STEPS {
            approved.steps += 1;
            return true;
        }
        false
    }

    /// For a probe: the connection that was said yes to for this address, whatever the account and certificate
    fn approved_address(&self, peer: &Rc<Peer>, address: &str) -> bool {
        self.users.get(&peer.uid).and_then(|user| user.approved.as_ref()).is_some_and(|approved| approved.by.upgrade().is_some_and(|by| Rc::ptr_eq(&by, peer)) && approved.address == address)
    }
}

/// Starts every run that is due, and puts a slice of what a run decided in place. Nothing here touches `last_use`: a run of
/// sync's own never keeps a vault open.
pub fn tick(service: &mut Service, now: Instant) {
    apply_slice(service);
    let due: Vec<u32> = service.sync.users.iter().filter(|(_, user)| user.due.is_some_and(|due| due <= now)).map(|(uid, _)| *uid).collect();
    for uid in due {
        let manual = service.sync.users.get_mut(&uid).is_some_and(|user| {
            user.due = None;
            std::mem::take(&mut user.manual)
        });
        match prepare_run(service, uid, manual, false) {
            Ok(run) => {
                let closes = run.closes;
                let busy = run.busy.clone();
                service.sync.jobs.push(Job {
                    job: run.job,
                    then: Box::new(move |service, done| {
                        finish_run(service, uid, closes, done, busy);
                    }),
                });
            }
            // Another thread of this account's is still at it: a short wait, not a quarter of an hour
            Err(Refusal::Failed) => {
                if let Some(user) = service.sync.users.get_mut(&uid) {
                    user.due = Some(now + Duration::from_secs(5));
                    user.manual |= manual;
                }
            }
            Err(_) => {}
        }
    }
}

// --- The connection and the vault's items -----------------------------------------------------------------------------------

fn load_connection(store: &mut Store) -> Option<Connection> {
    let value = store.get(KIND_CONNECTION, CONNECTION_ID).ok()?;
    serde_json::from_value(value).ok()
}

fn save_connection(store: &mut Store, connection: &Connection) -> Result<(), Trouble> {
    let value = serde_json::to_value(connection).map_err(|_| Trouble::Disk)?;
    store.put(KIND_CONNECTION, CONNECTION_ID, "account", &Value::Null, Approval::Never, &value).map(|_| ())
}

fn local_of(store: &mut Store, kind: &str, id: &str) -> Option<Local> {
    let entry = store.entry(kind, id)?.clone();
    let value = store.get(kind, id).ok()?;
    Some(Local { kind: entry.kind, id: entry.id, name: entry.name, meta: entry.meta, approval: entry.approval, value })
}

/// What the vault holds that a run looks at
struct Gathered {
    links: Vec<Link>,
    locals: Vec<Local>,
    /// Items in the index that did not read, and whether a shard of links did not: a run asks nothing of the server with either
    unreadable: usize,
    links_unreadable: bool,
}

/// The links, and every vault item that may sync: the passwords, authenticator accounts and backup codes, and a key or a
/// certificate only where the person chose it. An item that is listed and does not read is counted, not left out: leaving it out
/// would read as the person having removed it, and its copy on the server would go to the trash (the review's H6).
fn gather(store: &mut Store, connection: &Connection) -> Gathered {
    let wanted: Vec<(String, String)> = store
        .entries()
        .filter(|entry| kinds::may_sync(&entry.kind) && connection.choice_of(&entry.kind, &entry.id, &entry.meta) != "off")
        .map(|entry| (entry.kind.clone(), entry.id.clone()))
        .collect();
    let mut locals = Vec::with_capacity(wanted.len());
    let mut unreadable = 0;
    for (kind, id) in wanted {
        match local_of(store, &kind, &id) {
            Some(local) => locals.push(local),
            None => unreadable += 1,
        }
    }
    let mut links = Vec::new();
    let mut links_unreadable = false;
    for shard in merge::SHARDS.chars() {
        match read_shard(store, shard) {
            Ok(mut found) => links.append(&mut found),
            Err(()) => links_unreadable = true,
        }
    }
    Gathered { links, locals, unreadable, links_unreadable }
}

/// The links of one shard: none if there is no such item, and an error if there is one that does not read
fn read_shard(store: &mut Store, shard: char) -> Result<Vec<Link>, ()> {
    let id = shard.to_string();
    if store.entry(KIND_LINK, &id).is_none() {
        return Ok(Vec::new());
    }
    store.get(KIND_LINK, &id).ok().and_then(|value| serde_json::from_value(value).ok()).ok_or(())
}

// --- A run ------------------------------------------------------------------------------------------------------------------

struct Prepared {
    job: Box<dyn FnOnce() -> Done + Send>,
    closes: u64,
    busy: Arc<Busy>,
}

/// What a run decided, and the files that put it in place
struct Finished {
    outcome: Outcome,
    files: Vec<Vec<Option<BatchItem>>>,
}

/// What the thread brings back
enum Ran {
    /// A timer's run, on a connection that counts data: it waits, for a week at most
    Waiting,
    Net(NetFailure),
    Done(Box<Finished>),
}

fn is_metered(helper: &Path) -> bool {
    Command::new(helper).arg("metered").env_clear().env("PATH", "/usr/bin:/usr/sbin").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|status| status.success())
}

/// Each write of each group sealed into a file beside its place, and the filesystem flushed once: all of it on the thread, so that
/// the loop only has to rename
fn write_files(writer: &BatchWriter, groups: &[Group]) -> Vec<Vec<Option<BatchItem>>> {
    let mut files: Vec<Vec<Option<BatchItem>>> = groups.iter().map(|group| group.writes.iter().map(|write| writer.write(&write.kind, &write.id, &write.name, &write.meta, write.approval, &write.value).ok()).collect()).collect();
    if writer.flush().is_err() {
        for item in files.iter_mut().flatten().filter_map(Option::take) {
            item.discard();
        }
    }
    files
}

/// Reads what the run needs on the loop and makes the thread's work. `Failed` means a slow thing of this account's is still
/// going.
fn prepare_run(service: &mut Service, uid: u32, manual: bool, accept: bool) -> Result<Prepared, Refusal> {
    let net = service.sync.net.clone();
    let helper = service.sync.metered.clone();
    let worker = Worker { program: service.config.worker.clone(), ssh: Default::default() };
    let closes = service.closes_so_far(uid);
    let full = service.sync.users.get(&uid).is_some_and(|user| user.full) || manual;
    let access = service.sync.users.get_mut(&uid).and_then(|user| user.access.take());
    let Some(store) = service.first_open(uid) else { return Err(Refusal::Closed) };
    let Some(connection) = load_connection(store) else { return Err(Refusal::Refused) };
    let Gathered { links, locals, unreadable, links_unreadable } = gather(store, &connection);
    let sealing = Some((store.key().copy(), store.id().to_string()));
    let writer = store.batch_writer();
    let Some(busy) = service.sync.take_busy(uid) else {
        // Put the token back: this run did not start
        if let Some(user) = service.sync.users.get_mut(&uid) {
            user.access = access;
        }
        return Err(Refusal::Failed);
    };
    if let Some(user) = service.sync.users.get_mut(&uid) {
        user.full = false;
    }
    let cancel = service.sync.start_running(uid);
    let limit = service.sync.run_limit;
    let held = busy.clone();
    let job = move || -> Done {
        let _held = held;
        let now_ms = merge::now_ms();
        let waited = connection.waiting_since_ms.is_some_and(|since| now_ms - since >= WEEK_MS);
        if !manual && !waited && is_metered(&helper) {
            return Box::new(Ran::Waiting);
        }
        let server = connection.server();
        let mut http = match NetHttp::open(&net, &server, connection.ca.as_deref(), connection.pin.as_deref(), cancel, Instant::now() + limit) {
            Ok(http) => http,
            Err(failure) => return Box::new(Ran::Net(failure)),
        };
        let input = Input { connection, access, links, locals, sealing, accept, full, now_ms, local_unreadable: unreadable, links_unreadable, retry_files: manual };
        let outcome = merge::run(&mut http, &worker, input);
        let files = write_files(&writer, &outcome.groups);
        Box::new(Ran::Done(Box::new(Finished { outcome, files })))
    };
    Ok(Prepared { job: Box::new(job), closes, busy })
}

/// What finishes a run once its groups are in place
struct Finish {
    problem: Option<Problem>,
    report: Value,
    refresh: Option<Zeroizing<String>>,
    revision: Option<u64>,
    access: Option<Access>,
    churn: u32,
    accepted: bool,
    purge: Vec<Basis>,
    /// What files came to on this server: why none is tried (a word), and whether one came back whole
    no_files: Option<Option<String>>,
    files_read: Option<bool>,
    /// What the run left on the server to be cleaned up, as it stands
    pending: Option<Vec<Leftover>>,
}

/// A run's groups waiting for the loop: put in place a slice at a time, each only if the items it was decided from are still as
/// they were. Files of groups never put in place are taken away with it.
struct Applying {
    uid: u32,
    closes: u64,
    groups: VecDeque<(Group, Vec<Option<BatchItem>>)>,
    skipped: HashSet<String>,
    applied: usize,
    skipped_groups: usize,
    full: bool,
    finish: Option<Finish>,
    _busy: Arc<Busy>,
}

impl Drop for Applying {
    fn drop(&mut self) {
        for (_, files) in self.groups.drain(..) {
            for item in files.into_iter().flatten() {
                item.discard();
            }
        }
    }
}

fn discard(files: Vec<Option<BatchItem>>) {
    for item in files.into_iter().flatten() {
        item.discard();
    }
}

/// Writes what the run decided, if the vault is still open and the items are still as they were, and says how it went. What does not
/// fit in one slice is put in place by `tick`, and the answer says so.
fn finish_run(service: &mut Service, uid: u32, closes: u64, done: Done, busy: Arc<Busy>) -> Value {
    service.sync.running.remove(&uid);
    let Ok(ran) = done.downcast::<Ran>() else { return json!({"ok": false, "why": Refusal::Failed.name()}) };
    // Nothing is written into a vault that closed, or closed and opened again, while the thread was at the server
    if service.closes_so_far(uid) != closes || !service.is_open(uid) {
        // An opened-again vault still takes the login's news: a server that rotates a refresh token has retired the old one
        if let Ran::Done(finished) = *ran
            && service.is_open(uid)
        {
            keep_news(service, uid, finished.outcome.refresh.as_ref(), &finished.outcome.pending);
        }
        return json!({"ok": false, "why": Refusal::Closed.name()});
    }
    // Nor into a vault whose connection was turned off while the thread was at the server
    if service.first_open(uid).is_none_or(|store| !is_connected(store)) {
        return refused("not-connected");
    }
    let mut finish = Finish { problem: None, report: json!({}), refresh: None, revision: None, access: None, churn: 0, accepted: false, purge: Vec::new(), no_files: None, files_read: None, pending: None };
    let mut groups = VecDeque::new();
    match *ran {
        Ran::Waiting => {
            if let Some(store) = service.first_open(uid)
                && let Some(mut connection) = load_connection(store)
                && connection.waiting_since_ms.is_none()
            {
                connection.waiting_since_ms = Some(merge::now_ms());
                let _ = save_connection(store, &connection);
            }
            finish.report = json!({"waiting": true});
        }
        Ran::Net(failure) => finish.problem = Some(Problem::Net(failure)),
        Ran::Done(finished) => {
            let Finished { mut outcome, files } = *finished;
            finish.problem = outcome.problem.take();
            finish.report = serde_json::to_value(&outcome.report).unwrap_or_default();
            finish.refresh = outcome.refresh.take();
            finish.revision = outcome.revision;
            finish.access = outcome.access.take();
            finish.churn = outcome.churn;
            finish.accepted = outcome.accepted;
            finish.purge = std::mem::take(&mut outcome.purge);
            // A run that stopped before it asked about files hands back what the connection had
            finish.no_files = Some(outcome.no_files.take());
            finish.files_read = Some(outcome.files_read);
            finish.pending = Some(std::mem::take(&mut outcome.pending));
            // By shard, so that a slice touches few of the sixteen items the links are in; the order within a cipher is kept
            let mut together: Vec<(Group, Vec<Option<BatchItem>>)> = outcome.groups.drain(..).zip(files).collect();
            together.sort_by_key(|(group, _)| merge::shard_of(&group.cipher));
            groups = together.into();
        }
    }
    let mut applying = Applying { uid, closes, groups, skipped: HashSet::new(), applied: 0, skipped_groups: 0, full: false, finish: Some(finish), _busy: busy };
    // The first slice at once: a run of a few items is done before the answer
    let finished = put_in_place(service, &mut applying);
    if finished {
        return conclude(service, &mut applying);
    }
    let mut answer = describe(applying.finish.as_ref());
    answer["report"]["applying"] = json!(true);
    service.sync.applying.push_back(applying);
    answer
}

/// The refresh token a server rotated, kept in a vault that was closed and opened again while the run was at the server
/// What the run learned that must outlive it: the refresh token a server rotated, and the ciphers and slots it made and could not clean up
fn keep_news(service: &mut Service, uid: u32, refresh: Option<&Zeroizing<String>>, pending: &[Leftover]) {
    if let Some(store) = service.first_open(uid)
        && let Some(mut connection) = load_connection(store)
    {
        let before = (connection.refresh.clone(), connection.pending.clone());
        if let (Some(refresh), None) = (refresh, &connection.api_key) {
            connection.refresh = Some(refresh.to_string());
        }
        connection.pending = pending.to_vec();
        if (connection.refresh.clone(), connection.pending.clone()) != before {
            let _ = save_connection(store, &connection);
        }
    }
}

fn summary_of(problem: &Option<Problem>) -> Value {
    match problem {
        Some(problem) => json!({"name": problem.name(), "detail": problem.detail()}),
        None => Value::Null,
    }
}

fn describe(finish: Option<&Finish>) -> Value {
    let Some(finish) = finish else { return json!({"ok": false}) };
    json!({"ok": finish.problem.is_none(), "problem": summary_of(&finish.problem), "report": finish.report})
}

/// A slice of the groups, put in place; true when none are left (or the vault went). Each group is skipped whole if something it
/// was decided from changed meanwhile, and so is every later group of the same cipher.
fn put_in_place(service: &mut Service, applying: &mut Applying) -> bool {
    if service.closes_so_far(applying.uid) != applying.closes || !service.is_open(applying.uid) {
        for (_, files) in applying.groups.drain(..) {
            discard(files);
        }
        return true;
    }
    let started = Instant::now();
    let Some(store) = service.first_open(applying.uid) else { return true };
    let mut items: Vec<BatchItem> = Vec::new();
    let mut linked: Vec<(String, Option<Link>)> = Vec::new();
    let (mut entries_left, mut bytes_left) = store.room();
    let mut taken = 0;
    while let Some((group, files)) = applying.groups.pop_front() {
        let holds = group.basis.iter().all(|basis| local_of(store, &basis.kind, &basis.id).map(|local| map::hash(&local)) == basis.hash);
        if applying.skipped.contains(&group.cipher) || !holds {
            applying.skipped.insert(group.cipher.clone());
            applying.skipped_groups += 1;
            discard(files);
            continue;
        }
        let new: Vec<usize> = group.writes.iter().filter(|write| store.entry(&write.kind, &write.id).is_none()).map(|write| write.name.len() + write.meta.to_string().len()).collect();
        if files.iter().any(Option::is_none) || new.len() > entries_left || new.iter().sum::<usize>() > bytes_left {
            applying.full = true;
            applying.skipped.insert(group.cipher.clone());
            discard(files);
            continue;
        }
        entries_left -= new.len();
        bytes_left -= new.iter().sum::<usize>();
        taken += group.writes.len().max(1);
        items.extend(files.into_iter().flatten());
        linked.push((group.cipher, group.link));
        applying.applied += 1;
        if taken >= SLICE_GROUPS || started.elapsed() >= SLICE_TIME {
            break;
        }
    }
    if !items.is_empty() && store.commit_batch(items).is_err() {
        applying.full = true;
    }
    // The links that changed, written with the items they go with: sixteen items at most, whatever the number of ciphers
    let changed: BTreeSet<char> = linked.iter().map(|(cipher, _)| merge::shard_of(cipher)).collect();
    for shard in changed {
        // A shard that does not read is not written over: the next run stops on it, and nothing is lost that way
        let Ok(mut links) = read_shard(store, shard) else {
            applying.full = true;
            continue;
        };
        for (cipher, link) in linked.iter_mut().filter(|(cipher, _)| merge::shard_of(cipher) == shard) {
            links.retain(|old| old.cipher != *cipher);
            links.extend(link.take());
        }
        let id = shard.to_string();
        if links.is_empty() {
            let _ = store.remove(KIND_LINK, &id);
        } else if let Ok(value) = serde_json::to_value(&links) {
            let _ = store.put(KIND_LINK, &id, "links", &Value::Null, Approval::Never, &value);
        }
    }
    applying.groups.is_empty()
}

/// The slice of the run at the front of the queue, if there is one
fn apply_slice(service: &mut Service) {
    let Some(mut applying) = service.sync.applying.pop_front() else { return };
    if put_in_place(service, &mut applying) {
        conclude(service, &mut applying);
    } else {
        service.sync.applying.push_back(applying);
    }
}

/// Every group is in place: the tombstones past their days go, the connection hears how it went, and the next run is set
fn conclude(service: &mut Service, applying: &mut Applying) -> Value {
    let uid = applying.uid;
    let Some(finish) = applying.finish.take() else { return json!({"ok": false}) };
    let interval = service.sync.interval;
    let now = Instant::now();
    let Finish { problem, mut report, refresh, revision, access, churn, accepted, purge, no_files, files_read, pending } = finish;
    report["applied"] = json!(applying.applied);
    report["skipped"] = json!(applying.skipped_groups);
    if applying.full {
        report["full"] = json!(true);
    }
    if let Some(store) = service.first_open(uid) {
        for basis in &purge {
            if let Some(local) = local_of(store, &basis.kind, &basis.id)
                && map::deleted(&local)
                && Some(map::hash(&local)) == basis.hash
            {
                let _ = store.remove(&basis.kind, &basis.id);
            }
        }
    }
    let passes = problem.as_ref().is_none_or(Problem::passes);
    let summary = summary_of(&problem);
    if let Some(user) = service.sync.users.get_mut(&uid) {
        user.access = access;
        user.stopped = if passes { None } else { Some(summary.clone()) };
        // A problem that waits for a person stops the timer, until a run is asked for or the vault is connected again
        user.due = if passes { Some(now + interval) } else { None };
    }
    if let Some(store) = service.first_open(uid)
        && let Some(mut connection) = load_connection(store)
    {
        let before = serde_json::to_value(&connection).unwrap_or_default();
        if problem.is_none() {
            connection.last_revision = revision.or(connection.last_revision);
        }
        if let (Some(refresh), None) = (&refresh, &connection.api_key) {
            connection.refresh = Some(refresh.to_string());
        }
        connection.record(churn, accepted, merge::now_ms());
        if let Some(no_files) = no_files {
            connection.no_files = no_files;
        }
        if let Some(files_read) = files_read {
            connection.files_read = files_read;
        }
        if let Some(pending) = pending {
            connection.pending = pending;
        }
        connection.last = Some(json!({"at_ms": merge::now_ms(), "problem": summary, "report": report}));
        if problem.is_none() && report.get("waiting").is_none() {
            connection.last_success_ms = Some(merge::now_ms());
            connection.waiting_since_ms = None;
        }
        if serde_json::to_value(&connection).unwrap_or_default() != before {
            let _ = save_connection(store, &connection);
        }
    }
    // A run of a thousand items has had its thread's memory and the loop's: give back what is free, so that a vault that is open and idle is
    // as small as it was
    // SAFETY: glibc's own, with no argument that points anywhere
    unsafe { libc::malloc_trim(0) };
    json!({"ok": problem.is_none(), "problem": summary, "report": report})
}

// --- The operations ---------------------------------------------------------------------------------------------------------

fn refused(problem: &str) -> Value {
    json!({"ok": false, "why": Refusal::Refused.name(), "problem": problem})
}

fn problem_answer(problem: &Problem) -> Value {
    let mut answer = problem.detail();
    answer["ok"] = json!(false);
    answer["why"] = json!(if problem.passes() { Refusal::Failed.name() } else { Refusal::Refused.name() });
    answer["problem"] = json!(problem.name());
    answer
}

fn text<'a>(body: &'a Value, name: &str, most: usize) -> Option<&'a str> {
    body.get(name).and_then(Value::as_str).filter(|text| !text.is_empty() && text.len() <= most && !text.chars().any(char::is_control))
}

/// The first vault, open: or why not
fn need_first(service: &mut Service, uid: u32) -> Result<(), Refusal> {
    if service.first_open(uid).is_some() {
        return Ok(());
    }
    match Account::load(&service.config.state, uid) {
        Ok(Some(_)) => Err(Refusal::Closed),
        Ok(None) => Err(Refusal::NotSetUp),
        Err(_) => Err(Refusal::Failed),
    }
}

fn answer_or(result: Result<Next, Refusal>) -> Next {
    result.unwrap_or_else(Next::refuse)
}

fn probe_op_next(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Next {
    answer_or((|| {
        let uid = peer.uid;
        need_first(service, uid)?;
        let Ok(server) = Server::from_address(text(body, "address", 300).ok_or(Refusal::BadRequest)?) else { return Ok(Next::Answer(refused("not-an-address"))) };
        // Only on the connection that was said yes to for this address, and so after a connection to it began there: no program can
        // have the service reach out to a server of its choosing
        if !service.sync.approved_address(peer, &server.address) {
            return Ok(Next::refuse(Refusal::Refused));
        }
        let busy = service.sync.take_busy(uid).ok_or(Refusal::Failed)?;
        let net = service.sync.net.clone();
        let cancel = service.sync.start_running(uid);
        let origin = server.origins().into_iter().next().ok_or(Refusal::BadRequest)?;
        Ok(Next::slow(
            move || {
                let _busy = busy;
                probe(&net, &origin, cancel)
            },
            |service, peer, seen: Result<Value, NetFailure>| {
                service.sync.running.remove(&peer.uid);
                match seen {
                    Ok(seen) => {
                        let chain: Vec<Value> = seen["chain"].as_array().into_iter().flatten().map(shown).collect();
                        Next::Answer(json!({"ok": true, "trusted": seen["trusted"] == json!(true), "chain": chain}))
                    }
                    Err(failure) => problem_answer(&Problem::Net(failure)).into(),
                }
            },
        ))
    })())
}

impl From<Value> for Next {
    fn from(answer: Value) -> Next {
        Next::Answer(answer)
    }
}

/// A certificate as the person is shown it: text a server chose, cleaned, and the PEM only to send back
fn shown(cert: &Value) -> Value {
    let field = |name: &str| api::clean(cert.get(name).and_then(Value::as_str).unwrap_or(""));
    json!({"subject": field("subject"), "issuer": field("issuer"), "sha256": field("sha256"), "pem": cert.get("pem").and_then(Value::as_str).unwrap_or(""), "self_signed": cert["self_signed"] == json!(true)})
}

/// What the person typed for this connection, checked for its shape
struct Attempt {
    server: Server,
    email: String,
    password: Zeroizing<Vec<u8>>,
    api_key: Option<merge::ApiKey>,
    confirmed: Option<u32>,
    two_factor: Option<(u8, Zeroizing<String>, bool)>,
    new_device: Option<Zeroizing<String>>,
    email_code: bool,
    resend_device: bool,
    ca: Option<String>,
    pin: Option<String>,
    reconnect: bool,
}

fn attempt(body: &Value) -> Result<Attempt, Value> {
    let bad = || refused("bad-request");
    let server = Server::from_address(text(body, "address", 300).ok_or_else(bad)?).map_err(|why| refused(why.name()))?;
    let email = text(body, "email", 254).filter(|email| email.contains('@')).ok_or_else(bad)?.to_string();
    let password = body.get("secret").and_then(Value::as_str).and_then(|secret| base64::engine::general_purpose::STANDARD.decode(secret).ok()).filter(|secret| !secret.is_empty() && secret.len() <= 1024).ok_or_else(bad)?;
    let api_key = match body.get("api_key") {
        None | Some(Value::Null) => None,
        Some(key) => {
            let (id, secret) = (text(key, "id", 128).ok_or_else(bad)?, text(key, "secret", 128).ok_or_else(bad)?);
            Some(merge::ApiKey { id: id.to_string(), secret: secret.to_string() })
        }
    };
    let confirmed = body.get("confirm_kdf").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok());
    let two_factor = match body.get("two_factor") {
        None | Some(Value::Null) => None,
        Some(factor) => {
            let provider = factor.get("provider").and_then(Value::as_u64).and_then(|n| u8::try_from(n).ok()).filter(|n| matches!(n, 0 | 1 | 3)).ok_or_else(bad)?;
            Some((provider, Zeroizing::new(text(factor, "code", 64).ok_or_else(bad)?.to_string()), factor.get("remember") == Some(&json!(true))))
        }
    };
    let new_device = match body.get("new_device") {
        None | Some(Value::Null) => None,
        Some(_) => Some(Zeroizing::new(text(body, "new_device", 32).ok_or_else(bad)?.to_string())),
    };
    let ca = match body.get("ca") {
        None | Some(Value::Null) => None,
        Some(_) => Some(text_with_lines(body, "ca", 16 * 1024).filter(|pem| pem.starts_with("-----BEGIN CERTIFICATE-----")).ok_or_else(bad)?.to_string()),
    };
    let pin = match body.get("pin") {
        None | Some(Value::Null) => None,
        Some(_) => Some(text(body, "pin", 64).filter(|pin| pin.len() == 64 && pin.bytes().all(|b| b.is_ascii_hexdigit())).ok_or_else(bad)?.to_ascii_lowercase()),
    };
    if ca.is_some() != pin.is_some() {
        return Err(bad());
    }
    Ok(Attempt {
        server,
        email: crypto::normalize_email(&email),
        password: Zeroizing::new(password),
        api_key,
        confirmed,
        two_factor,
        new_device,
        email_code: body.get("send_email_code") == Some(&json!(true)),
        resend_device: body.get("resend_device_code") == Some(&json!(true)),
        ca,
        pin,
        reconnect: body.get("reconnect") == Some(&json!(true)),
    })
}

/// A text field that may hold line ends (a certificate)
fn text_with_lines<'a>(body: &'a Value, name: &str, most: usize) -> Option<&'a str> {
    body.get(name).and_then(Value::as_str).filter(|text| !text.is_empty() && text.len() <= most && !text.chars().any(|c| c.is_control() && c != '\n'))
}

/// What a connection step brings back
enum Step {
    /// The email code was asked for, or a new device's code asked for again
    Sent,
    Connected(Box<(Connection, Access)>),
    Problem(Problem),
}

/// Connects. The person is asked once, on the connection that asked, naming the address, the account's email and the certificate
/// that would be trusted (its whole fingerprint): what was said yes to is what is done, and a program that asks for anything else, or
/// on another connection, is asked about anew (the review's H1). The steps of one login (a code, a confirmation) come back on that
/// connection with the same words and are not asked again.
fn connect_next(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Next {
    answer_or((|| {
        let uid = peer.uid;
        need_first(service, uid)?;
        let attempt = match attempt(body) {
            Ok(attempt) => attempt,
            Err(answer) => return Ok(Next::Answer(answer)),
        };
        if service.first_open(uid).and_then(load_connection).is_some() && !attempt.reconnect {
            return Ok(Next::Answer(refused("already-connected")));
        }
        let (address, email, pin) = (attempt.server.address.clone(), attempt.email.clone(), attempt.pin.clone());
        if service.sync.approved(peer, &address, &email, pin.as_deref()) {
            return Ok(start_connect(service, peer, attempt));
        }
        let (message, item) = match &pin {
            Some(pin) => (say::SYNC_CONNECT_CA, format!("{} ({}), SHA-256 {}", api::clean(&address), api::clean(&email), pin)),
            None => (say::SYNC_CONNECT, format!("{} ({})", api::clean(&address), api::clean(&email))),
        };
        Ok(Next::approve(Action::Open, message, &item, move |service, peer| {
            service.sync.approve(peer, &address, &email, pin.as_deref());
            start_connect(service, peer, attempt)
        }))
    })())
}

/// The step itself, once it is said yes to
fn start_connect(service: &mut Service, peer: &Rc<Peer>, attempt: Attempt) -> Next {
    answer_or((|| {
        let uid = peer.uid;
        need_first(service, uid)?;
        let old = service.first_open(uid).and_then(load_connection);
        if old.is_some() && !attempt.reconnect {
            return Ok(Next::Answer(refused("already-connected")));
        }
        // A reconnection keeps what the last login pinned, and the device the server knows; another account needs `off` first
        let pinned = old.as_ref().map(|old| old.kdf);
        let device = old.as_ref().map_or_else(merge::new_device_id, |old| old.device.clone());
        let remembered = old.as_ref().and_then(|old| old.remember.clone());
        let closes = service.closes_so_far(uid);
        let busy = service.sync.take_busy(uid).ok_or(Refusal::Failed)?;
        let net = service.sync.net.clone();
        let exe = service.sync.kdf_child.clone();
        let limit = service.sync.run_limit;
        let cancel = service.sync.start_running(uid);
        let (ca, pin) = (attempt.ca.clone().or_else(|| old.as_ref().and_then(|old| old.ca.clone())), attempt.pin.clone().or_else(|| old.as_ref().and_then(|old| old.pin.clone())));
        let (keep_ca, keep_pin) = (ca.clone(), pin.clone());
        let server = attempt.server.clone();
        let job = move || -> Step {
            let _busy = busy;
            let mut http = match NetHttp::open(&net, &server, keep_ca.as_deref(), keep_pin.as_deref(), cancel, Instant::now() + limit) {
                Ok(http) => http,
                Err(failure) => return Step::Problem(Problem::Net(failure)),
            };
            let Attempt { server, email, password, api_key, confirmed, two_factor, new_device, email_code, resend_device, .. } = attempt;
            let derive = |password: &[u8], email: &str, kdf: &Kdf| derive_in(&exe, password, email, kdf);
            let pending = match merge::prepare(&mut http, &derive, server.clone(), &email, &password, confirmed, pinned.as_ref(), api_key, device) {
                Ok(pending) => pending,
                Err(problem) => return Step::Problem(problem),
            };
            if email_code || resend_device {
                let hash = pending.hash.as_ref().map(|hash| hash.as_str()).unwrap_or("");
                let sent = if email_code { api::send_email_code(&mut http, &server, &email, hash, &pending.device) } else { api::resend_device_code(&mut http, &server, &email, hash) };
                return match sent {
                    Ok(()) => Step::Sent,
                    Err(problem) => Step::Problem(problem),
                };
            }
            let extra = Extra {
                two_factor: two_factor.as_ref().map(|(provider, code, remember)| (*provider, code.as_str(), *remember)),
                remembered: remembered.as_deref(),
                new_device: new_device.as_ref().map(|code| code.as_str()),
            };
            match merge::finish(&mut http, &pending, &extra) {
                Ok(connected) => Step::Connected(Box::new(connected)),
                Err(problem) => Step::Problem(problem),
            }
        };
        Ok(Next::slow(job, move |service, peer, step: Step| {
            let uid = peer.uid;
            service.sync.running.remove(&uid);
            if service.closes_so_far(uid) != closes {
                return Next::refuse(Refusal::Closed);
            }
            match step {
                Step::Sent => Next::ok(),
                Step::Problem(problem) => Next::Answer(problem_answer(&problem)),
                Step::Connected(connected) => {
                    let (mut connection, access) = *connected;
                    let Some(store) = service.first_open(uid) else { return Next::refuse(Refusal::Closed) };
                    if let Some(old) = load_connection(store) {
                        if old.user_id != connection.user_id {
                            return Next::Answer(refused("other-account"));
                        }
                        connection.choices = old.choices.clone();
                    }
                    connection.ca = ca;
                    connection.pin = pin;
                    if let Err(trouble) = save_connection(store, &connection) {
                        return Next::trouble(trouble);
                    }
                    let (address, email) = (connection.address.clone(), connection.email.clone());
                    let user = service.sync.users.entry(uid).or_default();
                    user.access = Some(access);
                    user.stopped = None;
                    user.approved = None;
                    user.full = true;
                    user.manual = true;
                    user.due = Some(Instant::now());
                    Next::Answer(json!({"ok": true, "connected": true, "address": address, "email": email}))
                }
            }
        }))
    })())
}

fn status_next(service: &mut Service, peer: &Rc<Peer>, _body: &Value) -> Next {
    answer_or((|| {
        let uid = peer.uid;
        need_first(service, uid)?;
        let (stopped, due, running) = match service.sync.users.get(&uid) {
            Some(user) => (user.stopped.clone(), user.due, service.sync.busy.get(&uid).is_some_and(|flag| flag.load(Ordering::SeqCst))),
            None => (None, None, false),
        };
        let Some(store) = service.first_open(uid) else { return Err(Refusal::Closed) };
        let Some(connection) = load_connection(store) else { return Ok(Next::Answer(json!({"ok": true, "connected": false}))) };
        let linked: usize = merge::SHARDS.chars().map(|shard| read_shard(store, shard).map_or(0, |links| links.len())).sum();
        let choices: HashMap<&String, &String> = connection.choices.iter().collect();
        let next_in = due.map(|due| due.saturating_duration_since(Instant::now()).as_secs());
        Ok(Next::Answer(json!({
            "ok": true,
            "connected": true,
            "address": connection.address,
            "email": connection.email,
            "linked": linked,
            "running": running,
            "last_success_ms": connection.last_success_ms,
            "waiting_since_ms": connection.waiting_since_ms,
            "problem": stopped.or_else(|| connection.last.as_ref().and_then(|last| last.get("problem")).filter(|problem| !problem.is_null()).cloned()),
            "last": connection.last,
            "next_in_seconds": next_in,
            "choices": choices,
            "kinds": kinds::ALL.iter().map(|(word, kind)| (word.to_string(), connection.kind_choice(kind).to_string())).collect::<std::collections::BTreeMap<_, _>>(),
            "kinds_said": kinds::ALL.iter().filter(|(_, kind)| connection.kinds.contains_key(*kind)).map(|(word, _)| *word).collect::<Vec<_>>(),
            "no_files": connection.no_files,
            "files_read": connection.files_read,
            "private_ca": connection.ca.is_some(),
        })))
    })())
}

/// A run now, because the person asked: whatever the connection counts, and `accept_gone` if they said that what went from the server,
/// or came from it, may be taken here. Saying so asks, as turning an item off does: it is what a server that empties the account
/// would have a program say.
fn now_next(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Next {
    answer_or((|| {
        let uid = peer.uid;
        need_first(service, uid)?;
        let accept = body.get("accept_gone") == Some(&json!(true));
        if service.first_open(uid).and_then(load_connection).is_none() {
            return Ok(Next::Answer(refused("not-connected")));
        }
        if accept {
            return Ok(Next::approve(Action::Use, say::SYNC_ACCEPT, "", |service, peer| answer_or(start_now(service, peer.uid, true))));
        }
        start_now(service, uid, false)
    })())
}

fn start_now(service: &mut Service, uid: u32, accept: bool) -> Result<Next, Refusal> {
    let run = prepare_run(service, uid, true, accept)?;
    let (closes, busy) = (run.closes, run.busy.clone());
    Ok(Next::slow(run.job, move |service, peer, done: Done| Next::Answer(finish_run(service, peer.uid, closes, done, busy))))
}

/// Stops syncing: the connection and the links go, and nothing on either side. The server keeps what it has.
fn off_next(service: &mut Service, peer: &Rc<Peer>, _body: &Value) -> Next {
    answer_or((|| {
        let uid = peer.uid;
        need_first(service, uid)?;
        Ok(Next::approve(Action::Use, say::SYNC_OFF, "", move |service, peer| {
            let uid = peer.uid;
            service.sync.stop_running(uid);
            let Some(store) = service.first_open(uid) else { return Next::refuse(Refusal::Closed) };
            for shard in merge::SHARDS.chars() {
                let _ = store.remove(KIND_LINK, &shard.to_string());
            }
            match store.remove(KIND_CONNECTION, CONNECTION_ID) {
                Ok(()) | Err(Trouble::NotFound) => {}
                Err(trouble) => return Next::trouble(trouble),
            }
            service.sync.users.remove(&uid);
            Next::ok()
        }))
    })())
}

/// What an item does in sync: `on` (its kind's default for the kinds that sync), `off`, or `sealed` (a secure note only this
/// vault opens). Sending an item is a person's approved choice, and so is taking it off the server: the second moves its copy
/// there to the trash.
fn choose_next(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Next {
    answer_or((|| {
        let uid = peer.uid;
        need_first(service, uid)?;
        let kind = text(body, "kind", 32).ok_or(Refusal::BadRequest)?.to_string();
        let id = text(body, "id", 64).ok_or(Refusal::BadRequest)?.to_string();
        let choice = text(body, "choice", 8).filter(|choice| matches!(*choice, "on" | "off" | "sealed")).ok_or(Refusal::BadRequest)?.to_string();
        if !kinds::may_sync(&kind) || !amethystora_vault_core::protocol::valid_id(&id) {
            return Ok(Next::Answer(refused("not-syncable")));
        }
        let Some(store) = service.first_open(uid) else { return Err(Refusal::Closed) };
        let Some(entry) = store.entry(&kind, &id).cloned() else { return Err(Refusal::Refused) };
        if choice != "off" && entry.meta.get("exportable") == Some(&Value::Bool(false)) {
            return Ok(Next::Answer(refused("not-exportable")));
        }
        let off = choice == "off";
        let asks = entry.approval == Approval::Ask;
        let by_item = kinds::by_item_only(&kind, &entry.meta);
        let apply = move |service: &mut Service, peer: &Rc<Peer>| {
            let Some(store) = service.first_open(peer.uid) else { return Next::refuse(Refusal::Closed) };
            let Some(mut connection) = load_connection(store) else { return Next::Answer(refused("not-connected")) };
            let key = format!("{kind}.{id}");
            // What the item does with no word of its own: its kind's, but an item set to ask every time is off until its kind was said
            // for. Saying the same takes the item's own away; anything else is kept, and wins.
            let inherited = if by_item || (asks && !connection.kinds.contains_key(&kind)) { "off" } else { connection.kind_choice(&kind) };
            if choice == inherited {
                connection.choices.remove(&key);
            } else {
                connection.choices.insert(key, choice.clone());
            }
            match save_connection(store, &connection) {
                Ok(()) => {
                    service.sync.changed(peer.uid);
                    Next::ok()
                }
                Err(trouble) => Next::trouble(trouble),
            }
        };
        Ok(Next::approve(if off { Action::Use } else { Action::Export }, if off { say::SYNC_ITEM_OFF } else { say::SYNC_ITEM }, &entry.name, apply))
    })())
}

/// What a whole kind does in sync: `off`, `on` (plain: Bitwarden's own encryption alone, which every app of the account opens) or
/// `sealed`. It covers every item of the kind, those set to ask every time and those added later included, and one question is asked for
/// all of them: to send, an export of them; to stop, a use. An item's own choice still wins over it.
fn kind_next(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Next {
    answer_or((|| {
        let uid = peer.uid;
        need_first(service, uid)?;
        let word = text(body, "kind", 32).ok_or(Refusal::BadRequest)?;
        let choice = text(body, "choice", 8).filter(|choice| matches!(*choice, "on" | "off" | "sealed")).ok_or(Refusal::BadRequest)?.to_string();
        let Some((_, kind)) = kinds::ALL.iter().find(|(name, kind)| *name == word || *kind == word).copied() else { return Ok(Next::Answer(refused("not-syncable"))) };
        let Some(store) = service.first_open(uid) else { return Err(Refusal::Closed) };
        if load_connection(store).is_none() {
            return Ok(Next::Answer(refused("not-connected")));
        }
        let message = match choice.as_str() {
            "off" => say::SYNC_KIND_OFF,
            "on" => say::SYNC_KIND_ON,
            _ => say::SYNC_KIND_SEALED,
        };
        let off = choice == "off";
        let apply = move |service: &mut Service, peer: &Rc<Peer>| {
            let Some(store) = service.first_open(peer.uid) else { return Next::refuse(Refusal::Closed) };
            let Some(mut connection) = load_connection(store) else { return Next::Answer(refused("not-connected")) };
            connection.kinds.insert(kind.to_string(), choice.clone());
            match save_connection(store, &connection) {
                Ok(()) => {
                    service.sync.changed(peer.uid);
                    Next::ok()
                }
                Err(trouble) => Next::trouble(trouble),
            }
        };
        Ok(Next::approve(if off { Action::Use } else { Action::Export }, message, kinds::plural(kind), apply))
    })())
}

/// The ops answer with a `Next`; the plug table wants a `Result` (a refusal is a `Next` here, with its word)
macro_rules! op {
    ($name:ident, $inner:ident) => {
        fn $name(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
            Ok($inner(service, peer, body))
        }
    };
}
op!(probe_op, probe_op_next);
op!(connect, connect_next);
op!(status, status_next);
op!(now, now_next);
op!(off, off_next);
op!(choose, choose_next);
op!(kind, kind_next);

pub const OPS: &[(&str, crate::plugs::Plug)] = &[
    ("sync-probe", probe_op),
    ("sync-connect", connect),
    ("sync-status", status),
    ("sync-now", now),
    ("sync-off", off),
    ("sync-choose", choose),
    ("sync-kind", kind),
];
