//! Where the `items`, `keys` and `pki` modules of `apps/vault-core` plug in: a table from an operation's name on the socket to
//! a function here that calls their pure functions. The module knows nothing of sockets or approvals; the function here reads
//! its request's fields, asks what has to be asked through `Service::with_approval`, and turns the result into an answer.
//! plans/vault/work/ops.md lists every operation.
//!
//! Imports are not parsed here and never will be: a foreign file is parsed in the client, and the service only receives
//! finished items. Nothing here calls libcrypto: what needs it goes through `work`, a short-lived process, off the loop.

mod authenticator;
mod backup;
mod certs;
pub mod keys;
mod machine;
mod moving;
mod passwords;
pub mod work;

use std::rc::Rc;

use base64::Engine;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use amethystora_vault_core::protocol::{valid_hex_id, valid_id, valid_name, Approval, Refusal, MAX_DATA};
use amethystora_vault_core::store::MAX_META;

use crate::peer::Peer;
use crate::service::{Next, Service};

pub type Plug = fn(&mut Service, &Rc<Peer>, &Value) -> Result<Next, Refusal>;

const TABLE: &[(&str, Plug)] = &[
    ("password-generate", passwords::generate),
    ("password-copy", passwords::copy),
    ("password-reused", passwords::reused),
    ("auth-add", authenticator::add),
    ("auth-list", authenticator::list),
    ("auth-code", authenticator::code),
    ("auth-link", authenticator::link),
    ("auth-update", authenticator::update),
    ("auth-note", authenticator::note),
    ("auth-export", authenticator::export),
    ("import-merge", authenticator::merge),
    ("codes-add", backup::add),
    ("codes-list", backup::list),
    ("codes-next", backup::next),
    ("codes-used", backup::used),
    ("codes-all", backup::all),
    ("codes-update", backup::update),
    ("codes-note", backup::note),
    ("codes-coverage", backup::coverage),
    ("key-types", keys::types),
    ("key-new", keys::new),
    ("key-import", keys::import),
    ("key-public", keys::public),
    ("key-export", keys::export),
    ("cert-profiles", certs::profiles),
    ("cert-new", certs::new),
    ("cert-list", certs::list),
    ("cert-show", certs::show),
    ("cert-chain", certs::chain),
    ("cert-pfx", certs::pfx),
    ("cert-key", certs::key),
    ("cert-renew", certs::renew),
    ("sign-digest", certs::sign_digest),
    ("ca-cert", certs::ca_cert),
    ("machine", certs::machine),
    ("root-rotate", certs::rotate),
    ("vault-backup", moving::backup),
    ("vault-restore-put", moving::put),
    ("vault-restore", moving::restore),
];

pub fn find(op: &str) -> Option<Plug> {
    TABLE.iter().chain(crate::sync::OPS).find(|(name, _)| *name == op).map(|(_, plug)| *plug)
}

// --- A request's fields: each checked for its shape, and every number bounded, before anything is done with it --------------

fn text<'a>(body: &'a Value, name: &str) -> Result<&'a str, Refusal> {
    body.get(name).and_then(Value::as_str).ok_or(Refusal::BadRequest)
}

fn optional_text<'a>(body: &'a Value, name: &str) -> Result<Option<&'a str>, Refusal> {
    match body.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(Refusal::BadRequest),
    }
}

fn flag(body: &Value, name: &str) -> Result<Option<bool>, Refusal> {
    match body.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(Refusal::BadRequest),
    }
}

fn number(body: &Value, name: &str, range: std::ops::RangeInclusive<u64>) -> Result<Option<u64>, Refusal> {
    match body.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().filter(|value| range.contains(value)).map(Some).ok_or(Refusal::BadRequest),
    }
}

fn item_id(body: &Value) -> Result<String, Refusal> {
    let id = text(body, "id")?;
    if valid_id(id) { Ok(id.to_string()) } else { Err(Refusal::BadRequest) }
}

fn vault_of(body: &Value) -> Result<Option<String>, Refusal> {
    match optional_text(body, "vault")? {
        Some(id) if valid_hex_id(id) => Ok(Some(id.to_string())),
        Some(_) => Err(Refusal::BadRequest),
        None => Ok(None),
    }
}

fn name_of(body: &Value, field: &str) -> Result<String, Refusal> {
    let name = text(body, field)?;
    if valid_name(name) { Ok(name.to_string()) } else { Err(Refusal::BadRequest) }
}

fn approval_of(body: &Value) -> Result<Option<Approval>, Refusal> {
    match optional_text(body, "approval")? {
        Some(name) => Approval::from_name(name).map(Some).ok_or(Refusal::BadRequest),
        None => Ok(None),
    }
}

/// Base64, of at most `limit` bytes, its length checked before it is decoded
fn base64_field(body: &Value, name: &str, limit: usize) -> Result<Zeroizing<Vec<u8>>, Refusal> {
    let encoded = text(body, name)?;
    if encoded.len() > limit.div_ceil(3) * 4 {
        return Err(Refusal::TooLarge);
    }
    let decoded = base64::engine::general_purpose::STANDARD.decode(encoded).map(Zeroizing::new).map_err(|_| Refusal::BadRequest)?;
    if decoded.len() > limit { Err(Refusal::TooLarge) } else { Ok(decoded) }
}

/// A password or passphrase in base64: text, 1 to 1024 bytes, no NUL. A bad one has a word of its own for the window.
fn passphrase(body: &Value, name: &str) -> Result<Result<Zeroizing<String>, Next>, Refusal> {
    let bytes = base64_field(body, name, MAX_DATA)?;
    let bad = || refuse_with("bad-password", None);
    if bytes.is_empty() || bytes.len() > 1024 || bytes.contains(&0) {
        return Ok(Err(bad()));
    }
    Ok(std::str::from_utf8(&bytes).map(|text| Zeroizing::new(text.to_string())).map_err(|_| bad()))
}

/// Closed or never set up, as each op says it: an account's own business, so no more than `status` says
fn must_be_open(service: &Service, uid: u32) -> Result<(), Refusal> {
    if service.is_open(uid) {
        return Ok(());
    }
    match service.account(uid) {
        Ok(_) => Err(Refusal::Closed),
        Err(why) => Err(why),
    }
}

/// A refusal with a stable word for the window, and which field if one is wrong. Never what was typed.
fn refuse_with(problem: &str, field: Option<&str>) -> Next {
    let mut answer = json!({"ok": false, "why": Refusal::Refused.name(), "problem": problem});
    if let Some(field) = field {
        answer["field"] = json!(field);
    }
    Next::Answer(answer)
}

/// An item as its type, or the refusal to give: the same for one that is not there as for one that does not read
fn read<T: DeserializeOwned>(service: &mut Service, uid: u32, vault: &str, kind: &str, id: &str) -> Result<T, Next> {
    let store = service.store_of(uid, vault).map_err(Next::refuse)?;
    let value = store.get(kind, id).map_err(Next::trouble)?;
    let item = serde_json::from_value(value).map_err(|_| Next::refuse(Refusal::Refused))?;
    // What was read is the person's use of the vault, as `get` is: only a success counts
    service.touch(uid);
    Ok(item)
}

/// About what a new item takes on the disk, for the account's quota (`Service::write_in`)
fn extra_of(name: &str, meta: &Value, value: &Value) -> usize {
    value.to_string().len() + name.len() + meta.to_string().len()
}

fn flat(result: Result<Next, Next>) -> Next {
    match result {
        Ok(next) | Err(next) => next,
    }
}

/// Hands the heap a big request used back to the system: glibc keeps what was freed below the top for the next time, and the
/// service is held to its resident budget for hours with an import done once
fn give_back() {
    // SAFETY: malloc_trim only releases free memory, and is safe to call at any time
    unsafe { libc::malloc_trim(0) };
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|since| since.as_millis() as i64).unwrap_or(0)
}

/// What is listed has to fit the index's limit for one item
fn meta_fits(meta: &Value) -> bool {
    meta.to_string().len() <= MAX_META
}

/// What a person calls an account or a service, from text an import may have written: it is shown in prompts
fn clean_name(text: &str, otherwise: &str) -> String {
    let clean: String = text.chars().filter(|c| !c.is_control()).take(256).collect();
    let clean = clean.trim();
    if clean.is_empty() { otherwise.to_string() } else { clean.to_string() }
}

/// Tags, each short: what `auth-update` and `codes-update` accept
fn tags_of(body: &Value) -> Result<Option<Vec<String>>, Refusal> {
    let Some(value) = body.get("tags").filter(|value| !value.is_null()) else { return Ok(None) };
    let list = value.as_array().ok_or(Refusal::BadRequest)?;
    if list.len() > amethystora_vault_core::items::MAX_TAGS {
        return Err(Refusal::BadRequest);
    }
    list.iter()
        .map(|tag| tag.as_str().filter(|tag| valid_name(tag)).map(str::to_string).ok_or(Refusal::BadRequest))
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// A note given or taken away: `null` removes it
fn note_of(body: &Value) -> Result<Option<Option<amethystora_vault_core::items::Secret>>, Refusal> {
    match body.get("note") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(note)) if note.len() <= amethystora_vault_core::items::MAX_NOTE => Ok(Some(Some(amethystora_vault_core::items::Secret::new(note.clone())))),
        Some(_) => Err(Refusal::BadRequest),
    }
}
