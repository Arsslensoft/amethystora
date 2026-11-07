//! What each vault item becomes on the server, and back (plans/done/PLAN-vault.md's table): a password is a Login, an
//! authenticator account the `totp` of its Login or a Login of its own, backup codes a secure note, and a key or certificate,
//! when the person chooses it, a sealed secure note that only this vault opens. Nothing else ever goes.
//!
//! A cipher's fields belong to the vault items it carries. A field no item of the vault owns stays as the server has it, so
//! that a Login's `totp` this vault cannot read, or its notes, are never wiped by an update.

use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::items::{Account, BackupCode, BackupService, OtpKind, Password, Secret, KIND_ACCOUNT, KIND_BACKUP_CODES, KIND_PASSWORD};
use crate::otpauth;
use crate::protocol::Approval;
use crate::seal::{self, Key};

use super::cipher::{Content, LOGIN, MARKER_CODES, MARKER_SEALED, NOTE};

/// A vault item as a run sees it
#[derive(Clone)]
pub struct Local {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub meta: Value,
    pub approval: Approval,
    pub value: Value,
}

impl std::fmt::Debug for Local {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Local({}.{})", self.kind, self.id)
    }
}

/// Overwrites what a copy of an item holds, where it stands: a run's copies are not left for the allocator to free
pub fn wipe(local: &mut Local) {
    use zeroize::Zeroize;
    fn value(at: &mut Value) {
        match at {
            Value::String(text) => text.zeroize(),
            Value::Array(items) => items.iter_mut().for_each(value),
            Value::Object(object) => object.values_mut().for_each(value),
            _ => {}
        }
    }
    local.name.zeroize();
    value(&mut local.value);
    value(&mut local.meta);
}

/// Whether an item is in the vault's trash: kept, marked, and not synced as alive
pub fn deleted(local: &Local) -> bool {
    local.value.get("deletedAt").is_some_and(|at| !at.is_null())
}

/// What a part of a cipher's content was at the last sync, to tell a change on this side
pub fn hash(local: &Local) -> String {
    let projection = match local.kind.as_str() {
        KIND_PASSWORD => password(local).map(|p| json!([p.title, p.username, p.address, p.password.as_str(), p.note.as_ref().map(Secret::as_str)])),
        KIND_ACCOUNT => account(local).and_then(|a| totp_text(&a)).map(|totp| json!([totp.as_str()])),
        KIND_BACKUP_CODES => codes(local).map(|c| json!([c.service_name, codes_text(&c).as_str()])),
        _ => Some(json!([local.name, local.meta, local.value])),
    };
    let text = Zeroizing::new(serde_json::to_vec(&projection.unwrap_or(Value::Null)).unwrap_or_default());
    seal::hex(&Sha256::digest(&text))
}

pub fn password(local: &Local) -> Option<Password> {
    serde_json::from_value(local.value.clone()).ok()
}

pub fn account(local: &Local) -> Option<Account> {
    serde_json::from_value(local.value.clone()).ok()
}

pub fn codes(local: &Local) -> Option<BackupService> {
    serde_json::from_value(local.value.clone()).ok()
}

/// What goes in a Login's `totp`: the `otpauth://` form all of Bitwarden's apps read, and Steam's own form (spec 5.3)
pub fn totp_text(account: &Account) -> Option<Secret> {
    match account.otp_kind() {
        OtpKind::Steam => Some(Secret::new(format!("steam://{}", account.secret.as_str()))),
        _ => otpauth::write_link(account),
    }
}

/// An authenticator account from a Login's `totp`: a link, Steam's form, or a bare base32 secret (spec 5.3)
pub fn read_totp(text: &str, name: &str) -> Option<Account> {
    if let Some(mut account) = otpauth::read_link(text) {
        if account.issuer.is_empty() && account.label.is_empty() {
            account.issuer = name.to_string();
        }
        return Some(account);
    }
    let secret = crate::otp::canonical_secret(&crate::otp::normalize_typed(text))?;
    if secret.is_empty() || crate::otp::base32_decode(secret.as_str()).is_none() {
        return None;
    }
    Some(Account { kind: Some(OtpKind::Totp), issuer: name.to_string(), secret, ..Account::default() })
}

/// Backup codes as a note's text: one code a line, `[x] ` before a used one, then a blank line and the note
pub fn codes_text(service: &BackupService) -> Zeroizing<String> {
    let mut text = Zeroizing::new(String::new());
    for code in &service.codes {
        text.push_str(if code.used { "[x] " } else { "[ ] " });
        text.push_str(code.code.as_str());
        text.push('\n');
    }
    if let Some(note) = &service.note {
        text.push('\n');
        text.push_str(note.as_str());
    }
    let length = text.trim_end_matches('\n').len();
    text.truncate(length);
    text
}

/// The codes and the note back from that text. A line without a mark is an unused code: somebody edited it elsewhere.
pub fn read_codes_text(text: &str) -> (Vec<BackupCode>, Option<String>) {
    let (list, note) = match text.split_once("\n\n") {
        Some((list, note)) => (list, Some(note.to_string()).filter(|note| !note.trim().is_empty())),
        None => (text, None),
    };
    let mut codes = Vec::new();
    for line in list.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let (used, code) = if let Some(code) = line.strip_prefix("[x] ").or_else(|| line.strip_prefix("[X] ")) {
            (true, code)
        } else {
            (false, line.strip_prefix("[ ] ").unwrap_or(line))
        };
        let mut code = BackupCode::new(code.trim());
        code.used = used;
        codes.push(code);
    }
    (codes, note)
}

/// Which vault items one cipher carries
#[derive(Default)]
pub struct Parts<'a> {
    pub password: Option<&'a Local>,
    pub account: Option<&'a Local>,
    pub codes: Option<&'a Local>,
    pub sealed: Option<&'a Local>,
    /// Kinds the cipher carried and no longer does: their fields are cleared. A field no item ever owned is left alone.
    pub gone: Vec<&'a str>,
}

/// The name of every sealed note: it must not carry the item's own (spec 7)
pub const SEALED_NAME: &str = "Amethystora sealed item";
const SEALED_HEADER: &str = "amethystora-vault-sealed-1\n";
/// The purpose of the vault's own key that sealed notes are under: another vault's never open here
pub const SEALED_PURPOSE: &str = "sync-sealed";
const SEALED_RECORD: &str = "item";
/// The most a note's text may be: 10,000 characters of EncString (spec 7)
pub const SEALED_MOST: usize = 7439;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unmapped {
    /// Larger than a file of ours may be (the sealed blob of an item is at most what a note holds, or a file)
    TooLargeToSeal,
    /// A file more than this client sends
    FileTooLarge,
    /// An authenticator account no link can say: an algorithm Bitwarden's apps do not read
    NoLink,
    Unreadable,
}

/// What a sealed note says when its blob is the note's attachment: a blob of more than a note holds (spec 7) goes as a file
pub const SEALED_ATTACHED: &str = "amethystora-vault-sealed-1\nattached";

/// An item as the bytes a sealed note holds, under the vault's own sealing: its kind, id, name and value, so that a broken server
/// learns its length and nothing more. `counter` is how many times this item has been sent: inside the seal, where a server cannot
/// move it, so that an older copy of the note, which a server can keep, is told from a newer.
pub fn sealed_bytes(local: &Local, key: &Key, vault: &str, counter: u64) -> Result<Vec<u8>, Unmapped> {
    let inside = Zeroizing::new(serde_json::to_vec(&json!({"kind": local.kind, "id": local.id, "name": local.name, "meta": local.meta, "approval": local.approval, "value": local.value, "c": counter})).map_err(|_| Unmapped::Unreadable)?);
    Ok(seal::seal_data(key, vault, SEALED_PURPOSE, SEALED_RECORD, &inside))
}

/// The sealed note's text for an item, when it fits one note
pub fn sealed_text(local: &Local, key: &Key, vault: &str, counter: u64) -> Result<Zeroizing<String>, Unmapped> {
    let text = Zeroizing::new(format!("{SEALED_HEADER}{}", base64::engine::general_purpose::STANDARD.encode(sealed_bytes(local, key, vault, counter)?)));
    if text.len() > SEALED_MOST { Err(Unmapped::TooLargeToSeal) } else { Ok(text) }
}

/// A sealed note opened, if it is this vault's: anyone else's, or a changed one, does not open. And its counter, which is 0 for one
/// that has none.
pub fn open_sealed(text: &str, key: &Key, vault: &str) -> Option<(Local, u64)> {
    let body = text.strip_prefix(SEALED_HEADER)?;
    open_sealed_bytes(&base64::engine::general_purpose::STANDARD.decode(body.trim()).ok()?, key, vault)
}

/// The same from the blob itself, which a note's attachment is
pub fn open_sealed_bytes(bytes: &[u8], key: &Key, vault: &str) -> Option<(Local, u64)> {
    let plain = seal::open_data(key, vault, SEALED_PURPOSE, SEALED_RECORD, bytes).ok()?;
    let inside: Value = serde_json::from_slice(&plain).ok()?;
    let kind = inside.get("kind")?.as_str()?.to_string();
    let id = inside.get("id")?.as_str()?.to_string();
    if !super::kinds::may_sync(&kind) || !crate::protocol::valid_id(&id) {
        return None;
    }
    let counter = inside.get("c").and_then(Value::as_u64).unwrap_or(0);
    let local = Local {
        kind,
        id,
        name: inside.get("name")?.as_str()?.to_string(),
        meta: inside.get("meta").cloned().unwrap_or(Value::Null),
        approval: serde_json::from_value(inside.get("approval")?.clone()).ok()?,
        value: inside.get("value")?.clone(),
    };
    Some((local, counter))
}

/// Whether a sealed note holds its blob as its attachment
pub fn is_attached_seal(content: &Content) -> bool {
    content.notes.as_deref() == Some(SEALED_ATTACHED)
}

pub fn is_sealed_note(content: &Content) -> bool {
    content.kind == NOTE && (content.marker.as_deref() == Some(MARKER_SEALED) || content.notes.as_deref().is_some_and(|notes| notes.starts_with(SEALED_HEADER)))
}

pub fn is_codes_note(content: &Content) -> bool {
    content.kind == NOTE && content.marker.as_deref() == Some(MARKER_CODES)
}

/// The content a cipher should have: what the server has, with the fields its vault items own replaced. A part that is gone
/// clears its own fields, and leaves the rest.
pub fn compose(old: Option<&Content>, parts: &Parts<'_>, sealed: Option<&str>) -> Result<Content, Unmapped> {
    let mut content = old.cloned().unwrap_or_default();
    if let Some(sealed) = sealed {
        content.kind = NOTE;
        content.name = SEALED_NAME.to_string();
        content.notes = Some(sealed.to_string());
        content.marker = Some(MARKER_SEALED.to_string());
        return Ok(content);
    }
    if let Some(local) = parts.codes {
        let service = codes(local).ok_or(Unmapped::Unreadable)?;
        content.kind = NOTE;
        content.name = service.service_name.clone();
        content.notes = Some(codes_text(&service).to_string());
        content.marker = Some(MARKER_CODES.to_string());
        return Ok(content);
    }
    content.kind = LOGIN;
    let mut named = false;
    match parts.password {
        Some(local) => {
            let password = password(local).ok_or(Unmapped::Unreadable)?;
            content.name = password.title.clone();
            content.username = Some(password.username.clone()).filter(|text| !text.is_empty());
            content.password = Some(password.password.as_str().to_string()).filter(|text| !text.is_empty());
            content.uri = Some(password.address.clone()).filter(|text| !text.is_empty());
            content.notes = password.note.as_ref().map(|note| note.as_str().to_string());
            named = true;
        }
        None if parts.gone.contains(&KIND_PASSWORD) => {
            content.username = None;
            content.password = None;
            content.uri = None;
            // The note was the password item's too
            content.notes = None;
        }
        None => {}
    }
    match parts.account {
        Some(local) => {
            let account = account(local).ok_or(Unmapped::Unreadable)?;
            content.totp = Some(totp_text(&account).ok_or(Unmapped::NoLink)?.as_str().to_string());
            if !named {
                content.name = if account.issuer.is_empty() { account.label.clone() } else { account.issuer.clone() };
                if content.username.is_none() && !account.label.is_empty() && !account.issuer.is_empty() {
                    content.username = Some(account.label.clone());
                }
            }
        }
        None if parts.gone.contains(&KIND_ACCOUNT) => content.totp = None,
        None => {}
    }
    if content.name.is_empty() {
        content.name = "-".to_string();
    }
    Ok(content)
}

/// Whether the server's version of a Login has the vault's password in another field, or another field's text as the password.
/// With an item key a server cannot make a value, but it can move the ciphertexts of one item around (spec 10.2, BW05), and a
/// password in the username, the address or the title is shown, or looked up, where a password never is.
pub fn password_moved(old: &Password, old_totp: Option<&str>, content: &Content) -> bool {
    let theirs = [content.name.as_str(), content.username.as_deref().unwrap_or(""), content.uri.as_deref().unwrap_or(""), content.notes.as_deref().unwrap_or(""), content.totp.as_deref().unwrap_or("")];
    let mine = [old.title.as_str(), old.username.as_str(), old.address.as_str(), old.note.as_ref().map_or("", Secret::as_str), old_totp.unwrap_or("")];
    let secret = old.password.as_str();
    let new_secret = content.password.as_deref().unwrap_or("");
    // A password the item had a run ago, blanked there and put somewhere that shows it in the next, is the same move in two steps
    let secrets = std::iter::once(secret).chain(old.history.iter().map(|old| old.password.as_str())).filter(|secret| !secret.is_empty());
    // The old secret is now somewhere that shows it, having left the secret's place
    let exposed = secrets.into_iter().any(|secret| theirs.iter().zip(&mine).any(|(now, was)| *now == secret && now != was));
    // Or a field's text is now the secret
    let replaced = !new_secret.is_empty() && new_secret != secret && mine.iter().any(|was| !was.is_empty() && *was == new_secret);
    exposed || replaced
}

/// Whether a field the list shows, or a key's text, is the same as the password of another item of the vault, or one it had. A server
/// can copy a ciphertext from one cipher into another, which opens (it was never opened under another key), and the secret then
/// stands in a name that `list` shows to anyone. `was` is the cipher's content before, where there is one: a field that did not
/// change is the person's own and is not looked at.
pub fn exposes(content: &Content, was: Option<&Content>, secrets: &std::collections::HashSet<String>) -> bool {
    let shown = |content: &Content| -> [String; 3] { [content.name.clone(), content.username.clone().unwrap_or_default(), content.uri.clone().unwrap_or_default()] };
    let before = was.map(shown);
    shown(content).iter().enumerate().any(|(at, field)| !field.is_empty() && secrets.contains(field) && before.as_ref().is_none_or(|before| before[at] != *field))
}

/// A password item from a Login, keeping what the server does not hold (tags, order, its history, its own keys)
pub fn pull_password(content: &Content, existing: Option<&Password>, now_ms: i64, id: &str, uri_unproven: bool) -> Password {
    let mut password = existing.cloned().unwrap_or_else(|| Password { id: id.to_string(), added_at: now_ms, ..Password::default() });
    password.title = content.name.clone();
    password.username = content.username.clone().unwrap_or_default();
    password.address = content.uri.clone().unwrap_or_default();
    password.note = content.notes.clone().map(Secret::new);
    let new = Secret::new(content.password.clone().unwrap_or_default());
    if new.as_str() != password.password.as_str() {
        password.set_password(new, now_ms);
    }
    password.updated_at = Some(now_ms);
    password.deleted_at = None;
    // An address no checksum proves is for showing, never for filling, until the person confirms it (spec 5.4)
    if uri_unproven && !password.address.is_empty() {
        password.unknown.0.insert("addressUnproven".into(), Value::Bool(true));
    } else {
        password.unknown.0.remove("addressUnproven");
    }
    password
}

/// An authenticator account from a Login's `totp`. A counter never goes back: the higher of the two is kept.
pub fn pull_account(content: &Content, existing: Option<&Account>, now_ms: i64, id: &str) -> Option<Account> {
    let read = read_totp(content.totp.as_deref()?, &content.name)?;
    let mut account = existing.cloned().unwrap_or_else(|| Account { id: id.to_string(), added_at: now_ms, ..Account::default() });
    let counter = match (existing.and_then(|a| a.counter), read.counter) {
        (Some(mine), Some(theirs)) => Some(mine.max(theirs)),
        (mine, theirs) => theirs.or(mine),
    };
    account.kind = read.kind;
    account.issuer = read.issuer;
    account.label = read.label;
    account.secret = read.secret;
    account.algorithm = read.algorithm;
    account.digits = read.digits;
    account.period = read.period;
    account.counter = if account.otp_kind() == OtpKind::Hotp { counter } else { None };
    account.updated_at = Some(now_ms);
    account.deleted_at = None;
    Some(account)
}

/// Backup codes from a note: the server's list, a code used on either side staying used
pub fn pull_codes(content: &Content, existing: Option<&BackupService>, now_ms: i64, id: &str) -> BackupService {
    let mut service = existing.cloned().unwrap_or_else(|| BackupService { id: id.to_string(), added_at: now_ms, ..BackupService::default() });
    let (mut codes, note) = read_codes_text(content.notes.as_deref().unwrap_or(""));
    for code in &mut codes {
        if let Some(mine) = service.codes.iter().find(|mine| mine.code.as_str() == code.code.as_str() && mine.used) {
            code.used = true;
            code.used_at = mine.used_at;
        } else if code.used {
            code.used_at = Some(now_ms);
        }
    }
    service.service_name = content.name.clone();
    service.codes = codes;
    service.note = note.map(Secret::new);
    service.updated_at = Some(now_ms);
    service.deleted_at = None;
    service
}

/// What the list shows of an item sync wrote: the fields `ops.md` gives each kind, and no secret
pub fn meta_of(kind: &str, value: &Value) -> Value {
    let mut meta = meta_without_trash(kind, value);
    // A tombstone says so in what `list` shows: the window does not have to read the value to tell it from an item
    if let (Some(at), Some(object)) = (value.get("deletedAt").filter(|at| !at.is_null()), meta.as_object_mut()) {
        object.insert("deletedAt".into(), at.clone());
    }
    meta
}

fn meta_without_trash(kind: &str, value: &Value) -> Value {
    match kind {
        KIND_PASSWORD => {
            let password: Option<Password> = serde_json::from_value(value.clone()).ok();
            password.map_or(Value::Null, |p| json!({"username": p.username, "address": p.address}))
        }
        KIND_ACCOUNT => {
            let Some(a) = serde_json::from_value::<Account>(value.clone()).ok() else { return Value::Null };
            let mut meta = json!({"type": a.otp_kind(), "issuer": a.issuer, "label": a.label, "algorithm": a.algorithm, "digits": a.digits, "period": a.period, "has_note": a.note.is_some(), "added_at": a.added_at});
            if let Some(counter) = a.counter {
                meta["counter"] = json!(counter);
            }
            for (name, value) in [("pinned", json!(a.pinned)), ("tags", json!(a.tags)), ("order", json!(a.order)), ("no_backup_codes", json!(a.no_backup_codes))] {
                if !value.is_null() {
                    meta[name] = value;
                }
            }
            meta
        }
        KIND_BACKUP_CODES => {
            let Some(c) = serde_json::from_value::<BackupService>(value.clone()).ok() else { return Value::Null };
            let used = c.codes.iter().filter(|code| code.used).count();
            let mut meta = json!({"service": c.service_name, "total": c.codes.len(), "used": used, "left": c.codes.len() - used, "low": crate::codes::is_low(&c), "has_note": c.note.is_some(), "added_at": c.added_at});
            for (name, value) in [("account", json!(c.account)), ("tags", json!(c.tags)), ("order", json!(c.order))] {
                if !value.is_null() {
                    meta[name] = value;
                }
            }
            meta
        }
        _ => Value::Null,
    }
}

/// The name the list shows
pub fn name_of(kind: &str, value: &Value) -> String {
    let name = match kind {
        KIND_PASSWORD => value.get("title").and_then(Value::as_str).unwrap_or("").to_string(),
        KIND_ACCOUNT => {
            let issuer = value.get("issuer").and_then(Value::as_str).unwrap_or("");
            let label = value.get("label").and_then(Value::as_str).unwrap_or("");
            if issuer.is_empty() { label.to_string() } else { issuer.to_string() }
        }
        KIND_BACKUP_CODES => value.get("serviceName").and_then(Value::as_str).unwrap_or("").to_string(),
        _ => String::new(),
    };
    let name: String = name.chars().filter(|c| !c.is_control()).take(256).collect();
    if name.trim().is_empty() { "-".to_string() } else { name }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(kind: &str, value: Value) -> Local {
        Local { kind: kind.into(), id: "a1".into(), name: "x".into(), meta: Value::Null, approval: Approval::WhileOpen, value }
    }

    #[test]
    fn backup_codes_go_and_come_back() {
        let mut service = BackupService { service_name: "Example".into(), note: Some(Secret::new("printed in 2026")), ..BackupService::default() };
        service.codes = vec![BackupCode::new("1111-2222"), BackupCode::new("3333-4444")];
        service.codes[1].mark_used(5);
        let text = codes_text(&service);
        assert_eq!(text.as_str(), "[ ] 1111-2222\n[x] 3333-4444\n\nprinted in 2026");
        let (codes, note) = read_codes_text(&text);
        assert_eq!(codes.len(), 2);
        assert!(!codes[0].used && codes[1].used);
        assert_eq!(note.as_deref(), Some("printed in 2026"));
        let (edited, _) = read_codes_text("5555\n[ ] 6666");
        assert_eq!(edited.iter().map(|c| c.code.as_str()).collect::<Vec<_>>(), ["5555", "6666"]);
    }

    #[test]
    fn a_login_field_no_item_owns_is_left_as_it_is() {
        let old = Content { kind: LOGIN, name: "Site".into(), password: Some("p".into()), totp: Some("not a link at all !".into()), notes: Some("n".into()), ..Content::default() };
        let pw = local(KIND_PASSWORD, json!({"title": "Site", "password": "p2", "note": "n"}));
        let new = compose(Some(&old), &Parts { password: Some(&pw), ..Parts::default() }, None).unwrap();
        // The password changed, the totp nobody here reads is untouched
        assert_eq!(new.password.as_deref(), Some("p2"));
        assert_eq!(new.totp.as_deref(), Some("not a link at all !"));
        // Once the vault removes the authenticator account the Login carried, its totp goes too
        let gone = compose(Some(&old), &Parts { password: Some(&pw), gone: vec![KIND_ACCOUNT], ..Parts::default() }, None).unwrap();
        assert_eq!(gone.totp, None);
    }

    #[test]
    fn an_authenticator_rides_in_its_login_and_a_counter_never_goes_back() {
        let account = Account { kind: Some(OtpKind::Hotp), issuer: "Example".into(), label: "me".into(), secret: Secret::new("JBSWY3DPEHPK3PXP"), counter: Some(7), ..Account::default() };
        let auth = local(KIND_ACCOUNT, serde_json::to_value(&account).unwrap());
        let content = compose(None, &Parts { account: Some(&auth), ..Parts::default() }, None).unwrap();
        assert_eq!(content.name, "Example");
        assert!(content.totp.as_deref().unwrap().starts_with("otpauth://hotp/"));
        let mut behind = content.clone();
        behind.totp = Some(behind.totp.clone().unwrap().replace("counter=7", "counter=3"));
        assert_eq!(pull_account(&behind, Some(&account), 1, "a1").unwrap().counter, Some(7));
        let mut ahead = content.clone();
        ahead.totp = Some(ahead.totp.clone().unwrap().replace("counter=7", "counter=9"));
        assert_eq!(pull_account(&ahead, Some(&account), 1, "a1").unwrap().counter, Some(9));
        let steam = Account { kind: Some(OtpKind::Steam), issuer: "Steam".into(), secret: Secret::new("JBSWY3DPEHPK3PXP"), ..Account::default() };
        assert_eq!(totp_text(&steam).unwrap().as_str(), "steam://JBSWY3DPEHPK3PXP");
        let bare = read_totp("jbsw y3dp ehpk 3pxp", "Site").unwrap();
        assert_eq!((bare.issuer.as_str(), bare.secret.as_str()), ("Site", "JBSWY3DPEHPK3PXP"));
    }

    #[test]
    fn a_sealed_note_opens_only_in_its_own_vault() {
        let key = Key::random();
        let item = local("key", json!({"private": "AAAA"}));
        let text = sealed_text(&item, &key, "v1", 3).unwrap();
        assert!(!text.contains("AAAA"));
        let (back, counter) = open_sealed(&text, &key, "v1").unwrap();
        assert_eq!((back.kind.as_str(), back.value.clone(), counter), ("key", json!({"private": "AAAA"}), 3));
        assert!(open_sealed(&text, &key, "v2").is_none());
        assert!(open_sealed(&text, &Key::random(), "v1").is_none());
        let big = local("certificate", json!({"pem": "x".repeat(6000)}));
        assert_eq!(sealed_text(&big, &key, "v1", 1).err(), Some(Unmapped::TooLargeToSeal));
        // Never a kind outside the sealed ones, whatever a sealed note says
        let sneaky = local("data-key", json!("00"));
        assert!(open_sealed(&sealed_text(&sneaky, &key, "v1", 1).unwrap(), &key, "v1").is_none());
    }
}
