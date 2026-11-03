//! One message's way from the browser to the service and back. Each message says the page's origin and the top-level page's, which
//! the background script took from the browser's own `sender`, and one without them is refused. Nothing lives past the message: the
//! lists, the suffix list and the connection to the service are all made for it and go with it.
//!
//! What the extension says, each with `"v":1` and `origin` and `top`:
//!
//! ```text
//! state                        -> vault: open|closed|not-set-up, site, logins: [{id,name,user,exact}], codes and others: [{id,name,account,type}]
//! login {id}                   -> user, password: only an item that fits the page
//! code {id, picked?}           -> code: only an entry that fits the page, or that the person picked
//! generate {user, length?}     -> password, kept as a new login for the page first
//! save {user, password, name?} -> id: a new login for the page
//! open                         -> starts the Vault's window
//! ```
//!
//! Every message may carry `actions`: the origins the page's form or button posts to, which must all be this site's, or login, code and generate refuse them and state says `form`.
//!
//! Refusals are the service's names, with `problem` where the extension has something more to say: `no-suffix-list`,
//! `cross-site`, `no-match`, `exists`, `other-site-form`.

use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use amethystora_vault_client::{Refusal, MAX_LINE};
use base64::Engine;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::frame::{self, Frame, MAX_MESSAGE};
use crate::site::{self, Directory, Fit, Origin, Scheme, Suffixes};

/// As long as a person might take over a prompt. Past this the prompt's own client is gone, and the service cancels it.
const PATIENCE: Duration = Duration::from_secs(300);
const MAX_OFFERED: usize = 64;
const MAX_OTHERS: usize = 256;
const MAX_ACTIONS: usize = 8;
const MAX_FIELD_CHARS: usize = 1024;
const MAX_NAME_CHARS: usize = 256;

/// Where things are: a test's are its own
pub struct Places<'a> {
    pub socket: &'a Path,
    pub suffixes: &'a Path,
    pub directory: &'a Path,
    pub window: &'a Path,
}

impl Places<'static> {
    pub fn machine() -> Places<'static> {
        Places {
            socket: Path::new(amethystora_vault_client::SOCKET),
            suffixes: Path::new(site::SUFFIX_LIST),
            directory: Path::new(site::DIRECTORY),
            window: Path::new("/usr/bin/amethystora-vault"),
        }
    }
}

/// Why an answer is a refusal
struct No {
    why: Refusal,
    problem: Option<&'static str>,
}

impl From<Refusal> for No {
    fn from(why: Refusal) -> No {
        No { why, problem: None }
    }
}

fn problem(why: Refusal, problem: &'static str) -> No {
    No { why, problem: Some(problem) }
}

/// The browser went away while the host was waiting, so there is nobody left to answer
struct Gone;

enum Failed {
    No(No),
    Gone,
}

impl<T: Into<No>> From<T> for Failed {
    fn from(no: T) -> Failed {
        Failed::No(no.into())
    }
}

impl From<Gone> for Failed {
    fn from(_: Gone) -> Failed {
        Failed::Gone
    }
}

/// Reads and answers until the browser closes. Blocked on the pipe or the socket, with no timer, whenever nothing is happening.
pub fn serve<I: Read + AsFd>(input: &mut I, output: &mut impl Write, places: &Places) -> io::Result<()> {
    loop {
        let message = match frame::read(input)? {
            Frame::Message(message) => message,
            Frame::End => return Ok(()),
            Frame::Broken => {
                let _ = frame::write(output, refusal(&Refusal::TooLarge.into()).to_string().as_bytes());
                return Ok(());
            }
        };
        let answer = match handle(&message, places, input.as_fd()) {
            Ok(answer) => answer,
            Err(Failed::No(no)) => refusal(&no),
            Err(Failed::Gone) => return Ok(()),
        };
        drop(message);
        let bytes = Zeroizing::new(answer.to_string().into_bytes());
        // Nothing here comes near the browser's limit but a list of enormous names: a refusal says so rather than breaking the port
        let bytes = if bytes.len() > MAX_MESSAGE { Zeroizing::new(refusal(&Refusal::TooLarge.into()).to_string().into_bytes()) } else { bytes };
        frame::write(output, &bytes)?;
    }
}

fn refusal(no: &No) -> Value {
    json!({"ok": false, "why": no.why.name(), "problem": no.problem})
}

fn origin_of(body: &Value, name: &str) -> Result<Origin, No> {
    body.get(name).and_then(Value::as_str).and_then(Origin::parse).ok_or(Refusal::BadRequest.into())
}

fn handle(message: &[u8], places: &Places, browser: BorrowedFd) -> Result<Value, Failed> {
    let body: Value = serde_json::from_slice(message).map_err(|_| Refusal::BadRequest)?;
    if !body.is_object() || body.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(Refusal::BadRequest.into());
    }
    let op = body.get("op").and_then(Value::as_str).ok_or(Refusal::BadRequest)?;
    let page = origin_of(&body, "origin")?;
    let top = origin_of(&body, "top")?;
    if !matches!(op, "state" | "login" | "code" | "generate" | "save" | "open") {
        return Err(Refusal::BadRequest.into());
    }
    let list = Suffixes::load(places.suffixes).ok_or_else(|| problem(Refusal::Failed, "no-suffix-list"))?;
    if !list.same_site(&page.host, &top.host) || page.scheme != top.scheme {
        return Err(problem(Refusal::Refused, "cross-site").into());
    }
    if op == "open" {
        return open_window(places.window);
    }
    let ours = form_is_ours(&list, &page, &body)?;
    if !ours && matches!(op, "login" | "code" | "generate") {
        return Err(problem(Refusal::Refused, "other-site-form").into());
    }
    let mut service = Service::connect(places.socket, browser)?;
    match op {
        "state" => state(&list, &page, ours, places.directory, &mut service),
        "login" => login(&list, &page, &body, &mut service),
        "code" => code(&list, &page, places.directory, &body, &mut service),
        "generate" => generate(&list, &page, &top, &body, &mut service),
        _ => save(&list, &page, &top, &body, &mut service),
    }
}

/// Whether every place the page's form or button posts to (`actions`, as the browser resolved them) is this site, and no weaker
/// a scheme: a form that sends the password elsewhere is how a page with an injected form reads it. No `actions` is the page's own.
fn form_is_ours(list: &Suffixes, page: &Origin, body: &Value) -> Result<bool, No> {
    match body.get("actions") {
        None | Some(Value::Null) => Ok(true),
        Some(Value::Array(actions)) if actions.len() <= MAX_ACTIONS => Ok(actions.iter().all(|action| {
            action.as_str().and_then(Origin::parse).is_some_and(|to| list.same_site(&to.host, &page.host) && (to.scheme == page.scheme || to.scheme == Scheme::Https))
        })),
        Some(_) => Err(Refusal::BadRequest.into()),
    }
}

/// The socket, one line a message. A person may take minutes over a prompt, so the wait watches the browser too: if it closes,
/// the host goes at once, and the service cancels the prompt of a client that hung up.
struct Service<'a> {
    stream: UnixStream,
    browser: BorrowedFd<'a>,
}

impl<'a> Service<'a> {
    fn connect(socket: &Path, browser: BorrowedFd<'a>) -> Result<Service<'a>, No> {
        let stream = UnixStream::connect(socket).map_err(|_| Refusal::Failed)?;
        stream.set_write_timeout(Some(Duration::from_secs(10))).map_err(|_| Refusal::Failed)?;
        Ok(Service { stream, browser })
    }

    fn ask(&mut self, request: &Value) -> Result<Value, Failed> {
        let mut line = Zeroizing::new(request.to_string().into_bytes());
        line.push(b'\n');
        self.stream.write_all(&line).map_err(|_| Refusal::Failed)?;
        let answer = self.read_line()?;
        Ok(serde_json::from_slice(&answer).map_err(|_| Refusal::Failed)?)
    }

    /// What the service answered, or the refusal it said
    fn ok(&mut self, request: &Value) -> Result<Value, Failed> {
        let answer = self.ask(request)?;
        if answer.get("ok") == Some(&Value::Bool(true)) {
            return Ok(answer);
        }
        Err(Refusal::from_name(answer.get("why").and_then(Value::as_str).unwrap_or("failed")).into())
    }

    fn read_line(&mut self) -> Result<Zeroizing<Vec<u8>>, Failed> {
        let deadline = Instant::now() + PATIENCE;
        let mut line = Zeroizing::new(Vec::with_capacity(4096));
        let mut chunk = Zeroizing::new(vec![0u8; 64 * 1024]);
        loop {
            self.wait(deadline)?;
            let count = match self.stream.read(&mut chunk) {
                Ok(0) => return Err(Refusal::Failed.into()),
                Ok(count) => count,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => return Err(Refusal::Failed.into()),
            };
            if line.len() + count > MAX_LINE {
                return Err(Refusal::TooLarge.into());
            }
            line.extend_from_slice(&chunk[..count]);
            if let Some(end) = line.iter().position(|byte| *byte == b'\n') {
                line.truncate(end);
                return Ok(line);
            }
        }
    }

    /// Until the service has something to say. Nothing is asked of the browser's pipe but that it is still there.
    fn wait(&self, deadline: Instant) -> Result<(), Failed> {
        let mut fds = [
            libc::pollfd { fd: self.stream.as_raw_fd(), events: libc::POLLIN, revents: 0 },
            libc::pollfd { fd: self.browser.as_raw_fd(), events: 0, revents: 0 },
        ];
        loop {
            let left = deadline.saturating_duration_since(Instant::now()).as_millis().min(i32::MAX as u128) as i32;
            if left == 0 {
                return Err(Refusal::Failed.into());
            }
            // SAFETY: two pollfds of an array that lives through the call
            let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, left) };
            if ready < 0 {
                if io::Error::last_os_error().kind() == ErrorKind::Interrupted {
                    continue;
                }
                return Err(Refusal::Failed.into());
            }
            if fds[1].revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
                return Err(Gone.into());
            }
            if fds[0].revents != 0 {
                return Ok(());
            }
        }
    }

    fn items(&mut self, kind: &str) -> Result<Vec<Value>, Failed> {
        let mut answer = self.ok(&json!({"v": 1, "op": "list", "kind": kind}))?;
        let items = answer.get_mut("items").and_then(Value::as_array_mut).map(std::mem::take).ok_or(Refusal::Failed)?;
        Ok(items.into_iter().filter(|item| item["meta"].get("deleted_at").is_none_or(Value::is_null)).collect())
    }
}

/// A field of an item's `meta`, or nothing
fn meta<'v>(item: &'v Value, name: &str) -> &'v str {
    item["meta"].get(name).and_then(Value::as_str).unwrap_or("")
}

fn id_of(item: &Value) -> &str {
    item.get("id").and_then(Value::as_str).unwrap_or("")
}

fn name_of(item: &Value) -> &str {
    item.get("name").and_then(Value::as_str).unwrap_or("")
}

fn wanted_id(body: &Value) -> Result<&str, No> {
    body.get("id").and_then(Value::as_str).filter(|id| (1..=64).contains(&id.len())).ok_or(Refusal::BadRequest.into())
}

fn code_row(item: &Value) -> Value {
    json!({"id": id_of(item), "name": name_of(item), "account": meta(item, "label"), "type": meta(item, "type")})
}

fn state(list: &Suffixes, page: &Origin, ours: bool, directory: &Path, service: &mut Service) -> Result<Value, Failed> {
    let site = list.registrable(&page.host).unwrap_or(&page.host);
    let passwords = match service.items("password") {
        Ok(items) => items,
        Err(Failed::No(No { why: Refusal::Closed, .. })) => return Ok(json!({"ok": true, "vault": "closed", "site": site})),
        Err(Failed::No(No { why: Refusal::NotSetUp, .. })) => return Ok(json!({"ok": true, "vault": "not-set-up", "site": site})),
        Err(other) => return Err(other),
    };
    let fitting: Vec<(&Value, Fit)> = passwords.iter().map(|item| (item, site::fit(list, page, meta(item, "address")))).filter(|(_, fit)| *fit != Fit::No).collect();
    let mut logins: Vec<Value> = fitting.iter().map(|(item, fit)| json!({"id": id_of(item), "name": name_of(item), "user": meta(item, "username"), "exact": *fit == Fit::Exact})).collect();
    // The page's own host first
    logins.sort_by_key(|row| row["exact"] != Value::Bool(true));
    logins.truncate(MAX_OFFERED);
    let names: Vec<&str> = fitting.iter().map(|(item, _)| name_of(item)).collect();
    let entries = service.items("authenticator")?;
    let table = if entries.is_empty() { None } else { Directory::load(directory) };
    let (matching, others): (Vec<&Value>, Vec<&Value>) = entries.iter().partition(|item| site::code_fits(list, table.as_ref(), page, meta(item, "issuer"), &names));
    Ok(json!({
        "ok": true,
        "vault": "open",
        "site": site,
        "form": if ours { "ours" } else { "other-site" },
        "logins": logins,
        "codes": matching.iter().take(MAX_OFFERED).map(|item| code_row(item)).collect::<Vec<_>>(),
        "others": others.iter().take(MAX_OTHERS).map(|item| code_row(item)).collect::<Vec<_>>(),
    }))
}

/// Base64 from the service as text, or a refusal: what is not text cannot go into a field
fn secret_text(answer: &Value) -> Result<Zeroizing<String>, No> {
    let encoded = answer.get("secret").and_then(Value::as_str).ok_or(Refusal::Failed)?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|_| Refusal::Failed)?;
    String::from_utf8(bytes).map(Zeroizing::new).map_err(|_| Refusal::Failed.into())
}

fn login(list: &Suffixes, page: &Origin, body: &Value, service: &mut Service) -> Result<Value, Failed> {
    let id = wanted_id(body)?;
    let items = service.items("password")?;
    let item = items.iter().find(|item| id_of(item) == id).ok_or_else(|| problem(Refusal::Refused, "no-match"))?;
    if site::fit(list, page, meta(item, "address")) == Fit::No {
        return Err(problem(Refusal::Refused, "no-match").into());
    }
    let answer = service.ok(&json!({"v": 1, "op": "password-copy", "id": id}))?;
    let password = secret_text(&answer)?;
    Ok(json!({"ok": true, "user": meta(item, "username"), "password": password.as_str()}))
}

fn code(list: &Suffixes, page: &Origin, directory: &Path, body: &Value, service: &mut Service) -> Result<Value, Failed> {
    let id = wanted_id(body)?;
    let entries = service.items("authenticator")?;
    let entry = entries.iter().find(|item| id_of(item) == id).ok_or_else(|| problem(Refusal::Refused, "no-match"))?;
    if body.get("picked") != Some(&Value::Bool(true)) {
        let passwords = service.items("password")?;
        let names: Vec<&str> = passwords.iter().filter(|item| site::fit(list, page, meta(item, "address")) != Fit::No).map(name_of).collect();
        let table = Directory::load(directory);
        if !site::code_fits(list, table.as_ref(), page, meta(entry, "issuer"), &names) {
            return Err(problem(Refusal::Refused, "no-match").into());
        }
    }
    // HOTP's counter moves only on being asked, and a fill is being asked
    let mut request = json!({"v": 1, "op": "auth-code", "id": id});
    if meta(entry, "type") == "hotp" {
        request["step"] = Value::Bool(true);
    }
    let answer = service.ok(&request)?;
    let code = answer.get("code").and_then(Value::as_str).filter(|code| (1..=16).contains(&code.len()) && code.chars().all(char::is_alphanumeric)).ok_or(Refusal::Failed)?;
    Ok(json!({"ok": true, "code": code}))
}

/// A new password, kept in the Vault before it is handed over: a page that submits first, or a popup that closes, must never leave an
/// account with a password nobody has. It is a new login for the top-level page, as Save makes one, so `user` is needed.
fn generate(list: &Suffixes, page: &Origin, top: &Origin, body: &Value, service: &mut Service) -> Result<Value, Failed> {
    if page != top {
        return Err(problem(Refusal::Refused, "cross-site").into());
    }
    let mut request = json!({"v": 1, "op": "password-generate"});
    if let Some(length) = body.get("length") {
        // A field's own limit, which the page chose
        let length = length.as_u64().filter(|length| (8..=128).contains(length)).ok_or(Refusal::BadRequest)?;
        request["length"] = json!(length);
    }
    let user = plain_field(body, "user", MAX_FIELD_CHARS)?.ok_or(Refusal::BadRequest)?;
    ensure_new(list, page, &user, service)?;
    let answer = service.ok(&request)?;
    let password = secret_text(&answer)?;
    store(body, &user, &password, default_name(list, page), service)?;
    Ok(json!({"ok": true, "password": password.as_str()}))
}

fn plain_field(body: &Value, name: &str, most: usize) -> Result<Option<String>, No> {
    match body.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text.chars().count() <= most && !text.chars().any(char::is_control) => Ok(Some(text.clone())),
        Some(_) => Err(Refusal::BadRequest.into()),
    }
}

fn new_id() -> Result<String, No> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Refusal::Failed)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..]))
}

fn default_name(list: &Suffixes, page: &Origin) -> String {
    list.registrable(&page.host).unwrap_or(&page.host).to_string()
}

/// A login for this user on this site is the window's to change, with its history and its approval. Over http an https item of
/// the same address counts too: it is the same account, and a second item would then fit both.
fn ensure_new(list: &Suffixes, page: &Origin, user: &str, service: &mut Service) -> Result<(), Failed> {
    let secure = Origin { scheme: Scheme::Https, ..page.clone() };
    let items = service.items("password")?;
    let there = |item: &Value| meta(item, "username") == user && (site::fit(list, page, meta(item, "address")) != Fit::No || site::fit(list, &secure, meta(item, "address")) != Fit::No);
    if items.iter().any(there) {
        return Err(problem(Refusal::Refused, "exists").into());
    }
    Ok(())
}

/// The browser's word for the page is the address, never anything the page or the extension typed
fn store(body: &Value, user: &str, password: &str, name: String, service: &mut Service) -> Result<String, Failed> {
    let address = body.get("origin").and_then(Value::as_str).ok_or(Refusal::BadRequest)?;
    let id = new_id()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as i64);
    let value = json!({"id": id, "title": name, "username": user, "address": address, "password": password, "added_at": now});
    let answer = service.ok(&json!({"v": 1, "op": "put", "kind": "password", "id": id, "name": name, "meta": {"username": user, "address": address}, "value": value}))?;
    Ok(answer.get("id").and_then(Value::as_str).unwrap_or(&id).to_string())
}

/// A new login for the top-level page, on the person's click. `name` is the person's, or the page's site.
fn save(list: &Suffixes, page: &Origin, top: &Origin, body: &Value, service: &mut Service) -> Result<Value, Failed> {
    if page != top {
        return Err(problem(Refusal::Refused, "cross-site").into());
    }
    let user = plain_field(body, "user", MAX_FIELD_CHARS)?.unwrap_or_default();
    let password = Zeroizing::new(plain_field(body, "password", MAX_FIELD_CHARS)?.filter(|text| !text.is_empty()).ok_or(Refusal::BadRequest)?);
    let name = plain_field(body, "name", MAX_NAME_CHARS)?.filter(|name| !name.trim().is_empty()).unwrap_or_else(|| default_name(list, page));
    ensure_new(list, page, &user, service)?;
    let id = store(body, &user, &password, name, service)?;
    Ok(json!({"ok": true, "id": id}))
}

/// Starts the window on its own, so that it outlives this host and the browser
fn open_window(window: &Path) -> Result<Value, Failed> {
    Command::new(window).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn().map_err(|_| Refusal::Failed)?;
    Ok(json!({"ok": true}))
}
