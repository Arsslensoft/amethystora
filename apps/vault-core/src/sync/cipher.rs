//! One cipher of `/sync` (spec 5.3 to 5.6, 6): whether this client may read it at all, what it holds once opened, and the
//! body that changes it. An update starts from the server's last raw JSON and replaces only the EncStrings of what changed, so
//! that what this client does not read (passkeys, other URIs, fields, keys it does not know) goes back as it came.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use zeroize::Zeroizing;

use super::crypto::{self, EncString, SymKey, Unreadable};
use crate::seal;
use super::json;
use super::lengths;

/// The custom field that says a secure note is one of ours, and which: plain to anyone who opens the item, and honest
pub const MARKER: &str = "amethystora-vault";
pub const MARKER_CODES: &str = "backup-codes";
pub const MARKER_SEALED: &str = "sealed";
/// What a plain key, certificate or protected file is, in the marker's place (the item's kind says which, in words)
pub const MARKER_KEY: &str = "key";
pub const MARKER_CERT: &str = "certificate";
pub const MARKER_FILE: &str = "file";

/// The other fields of ours a plain note carries: its kind and expiry as text a person reads, and a private key as a hidden field
pub const FIELD_KIND: &str = "kind";
pub const FIELD_EXPIRES: &str = "expires";
pub const FIELD_PRIVATE: &str = "private key";
const OUR_FIELDS: &[&str] = &[FIELD_KIND, FIELD_EXPIRES, FIELD_PRIVATE];

pub const LOGIN: u8 = 1;
pub const NOTE: u8 = 2;
pub const SSH: u8 = 5;

/// A field of ours on a plain note: text, or hidden (type 1)
#[derive(Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub value: String,
    pub hidden: bool,
}

/// An SSH key item's three strings (spec 5.3), the private one in OpenSSH's unencrypted form
#[derive(Clone, PartialEq, Eq)]
pub struct Ssh {
    pub private: String,
    pub public: String,
    pub fingerprint: String,
}

/// One attachment of a cipher as the server lists it. The size is the encrypted file's, as the server says: a hint, never a bound.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Att {
    pub id: String,
    pub size: u64,
}

/// An attachment id that may go into a URL (spec-attachments 1): letters and digits, as a server makes them
pub fn attachment_id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// The attachments a cipher lists, those with an id that may go into a URL, by id. Nothing else of them is read here.
pub fn attachments(raw: &Value) -> Vec<Att> {
    let mut found: Vec<Att> = json::get(raw, "attachments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|one| {
            let id = json::text(one, "id").filter(|id| attachment_id_ok(id))?.to_string();
            let size = match json::get(one, "size") {
                Some(Value::String(text)) => text.parse().unwrap_or(0),
                Some(other) => other.as_u64().unwrap_or(0),
                None => 0,
            };
            Some(Att { id, size })
        })
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

/// The attachment record of an id, raw, for its wrapped key
pub fn attachment_of<'a>(raw: &'a Value, id: &str) -> Option<&'a Value> {
    json::get(raw, "attachments").and_then(Value::as_array)?.iter().find(|one| json::text(one, "id") == Some(id))
}

/// Why a cipher is left alone: read for nothing, written never, deleted never (spec 5.6)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Foreign {
    Organization,
    /// A type this client does not know, or a card, identity or SSH key, which the vault has no kind for
    OtherType,
    Sealed,
    Withheld,
    Damaged,
    /// A key or field that is not type 2
    OldFormat,
    /// More than any cipher of a person's is, or nested deeper: one that would fill the links, or the stack, of a vault
    TooBig,
}

/// The most a cipher's JSON may be, counting its strings and what holds them, and how deep it may nest. An honest cipher is a few KB
/// (ten thousand characters of notes are about fourteen); the links of a sixteenth of five thousand of these are in one file.
const MOST_RAW: usize = 32 * 1024;
const MOST_DEPTH: usize = 16;

fn too_big(raw: &Value) -> bool {
    let mut size = 0usize;
    let mut stack: Vec<(&Value, usize)> = vec![(raw, 1)];
    while let Some((value, depth)) = stack.pop() {
        if depth > MOST_DEPTH {
            return true;
        }
        match value {
            Value::String(text) => size += text.len() + 2,
            Value::Array(items) => {
                size += 2 + items.len();
                stack.extend(items.iter().map(|item| (item, depth + 1)));
            }
            Value::Object(object) => {
                for (name, item) in object {
                    size += name.len() + 4;
                    stack.push((item, depth + 1));
                }
            }
            _ => size += 8,
        }
        if size > MOST_RAW {
            return true;
        }
    }
    false
}

/// The cipher types, ours and the rest: a card, an identity and an SSH key are known and left alone
pub fn sort(raw: &Value) -> Result<u8, Foreign> {
    if json::some(raw, "organizationId").is_some() {
        return Err(Foreign::Organization);
    }
    if json::some(raw, "partialData").is_some() {
        return Err(Foreign::Withheld);
    }
    if too_big(raw) {
        return Err(Foreign::TooBig);
    }
    if blob(raw) {
        return Err(Foreign::Sealed);
    }
    let kind = json::number(raw, "type").and_then(|n| u8::try_from(n).ok()).ok_or(Foreign::Damaged)?;
    if !(1..=5).contains(&kind) {
        return Err(Foreign::OtherType);
    }
    if json::text(raw, "id").is_none_or(|id| !super::api::is_uuid(id)) || json::text(raw, "name").is_none() {
        return Err(Foreign::Damaged);
    }
    if !every_string_is_type2(raw) {
        return Err(Foreign::OldFormat);
    }
    // Cards and identities are the person's own; an SSH key item may be a key of ours or one made in another app
    if kind == 3 || kind == 4 {
        return Err(Foreign::OtherType);
    }
    Ok(kind)
}

/// A `data` that is a sealed blob of the newer format (spec 4.5)
fn blob(raw: &Value) -> bool {
    let Some(data) = json::some(raw, "data") else { return false };
    let parsed = match data {
        Value::String(text) => serde_json::from_str::<Value>(text).ok(),
        other => Some(other.clone()),
    };
    parsed.is_some_and(|data| json::get(&data, "format_version").is_some())
}

/// The EncStrings this client reads: each must be type 2, or the whole cipher is left alone
fn encrypted_strings(raw: &Value) -> Vec<&str> {
    let mut found = Vec::new();
    fn take<'a>(found: &mut Vec<&'a str>, value: Option<&'a Value>) {
        if let Some(Value::String(text)) = value {
            found.push(text.as_str());
        }
    }
    let mut take = |value| take(&mut found, value);
    for name in ["key", "name", "notes"] {
        take(json::get(raw, name));
    }
    if let Some(login) = json::some(raw, "login") {
        for name in ["username", "password", "totp"] {
            take(json::get(login, name));
        }
        for uri in json::get(login, "uris").and_then(Value::as_array).into_iter().flatten() {
            take(json::get(uri, "uri"));
            take(json::get(uri, "uriChecksum"));
        }
    }
    if let Some(ssh) = json::some(raw, "sshKey") {
        for name in ["privateKey", "publicKey", "keyFingerprint"] {
            take(json::get(ssh, name));
        }
    }
    for field in json::get(raw, "fields").and_then(Value::as_array).into_iter().flatten() {
        take(json::get(field, "name"));
        take(json::get(field, "value"));
    }
    for old in json::get(raw, "passwordHistory").and_then(Value::as_array).into_iter().flatten() {
        take(json::get(old, "password"));
    }
    found
}

fn every_string_is_type2(raw: &Value) -> bool {
    encrypted_strings(raw).iter().all(|text| text.starts_with("2."))
}

/// A sign of Bitwarden's newer encryption on a personal cipher (spec 4.5): a blob, or a type 7 key or field
pub fn newer_encryption(raw: &Value) -> bool {
    if json::some(raw, "organizationId").is_some() {
        return false;
    }
    blob(raw) || encrypted_strings(raw).iter().any(|text| text.starts_with("7."))
}

/// A time as the servers write it, `2026-11-02T10:11:12.1234567Z`, in nanoseconds since 1970. Parsed, never compared as text.
pub fn parse_time(text: &str) -> Option<i128> {
    let text = text.strip_suffix('Z').or_else(|| text.strip_suffix("+00:00")).unwrap_or(text);
    let (date, time) = text.split_once('T')?;
    let mut date = date.split('-');
    let (year, month, day): (i64, i64, i64) = (date.next()?.parse().ok()?, date.next()?.parse().ok()?, date.next()?.parse().ok()?);
    let (clock, fraction) = time.split_once('.').unwrap_or((time, ""));
    let mut clock = clock.split(':');
    let (hour, minute, second): (i64, i64, i64) = (clock.next()?.parse().ok()?, clock.next()?.parse().ok()?, clock.next()?.parse().ok()?);
    if date.next().is_some() || clock.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    if fraction.len() > 9 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let nanos: i128 = if fraction.is_empty() { 0 } else { fraction.parse::<i128>().ok()? * 10i128.pow(9 - fraction.len() as u32) };
    // Days from the civil date (Howard Hinnant's algorithm)
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second;
    Some(i128::from(seconds) * 1_000_000_000 + nanos)
}

/// Now, as the servers write a time
pub fn now_text() -> String {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    time_text(since.as_secs() as i64, Some(since.subsec_millis()))
}

/// A time in seconds since 1970 as the servers write one, with milliseconds or without
pub fn time_text(seconds: i64, millis: Option<u32>) -> String {
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // The civil date from days (the same algorithm, backwards)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    match millis {
        Some(millis) => format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z", rest / 3600, rest % 3600 / 60, rest % 60),
        None => format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rest / 3600, rest % 3600 / 60, rest % 60),
    }
}

/// What a cipher holds that this client maps, opened. The service and the network service wipe every block they give back
/// (their allocator), which is where this is ever held.
#[derive(Clone, Default, PartialEq)]
pub struct Content {
    pub kind: u8,
    pub name: String,
    pub notes: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub totp: Option<String>,
    /// The first URI, which the vault's one address is
    pub uri: Option<String>,
    /// MARKER's value: which of ours a secure note is
    pub marker: Option<String>,
    /// The other fields of ours, by name
    pub fields: Vec<Field>,
    /// An SSH key item's strings
    pub ssh: Option<Ssh>,
}

impl std::fmt::Debug for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Content(type {})", self.kind)
    }
}

/// A cipher of ours, opened
pub struct Opened {
    pub id: String,
    pub content: Content,
    pub revision: String,
    pub deleted: bool,
    pub has_key: bool,
    /// The first URI had no checksum to prove it, on an item without its own key: display text, never a fill match
    pub uri_unproven: bool,
    /// What the server lists, by id
    pub attachments: Vec<Att>,
}

/// The key a cipher's fields are under: its own, exactly 64 bytes, or the user key on an item without one
pub fn key_of(raw: &Value, user: &SymKey) -> Result<SymKey, Unreadable> {
    match json::some(raw, "key") {
        Some(Value::String(wrapped)) => crypto::open_key(wrapped, user),
        Some(_) => Err(Unreadable),
        None => Ok(user.clone()),
    }
}

fn open_optional(raw: &Value, name: &str, key: &SymKey) -> Result<Option<String>, Unreadable> {
    match json::some(raw, name) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(crypto::open_text(text, key)?.to_string())),
        Some(_) => Err(Unreadable),
    }
}

/// Whether two versions of a cipher are under the same item key: neither has one, or both have one that opens to the same bytes. A
/// server cannot make a wrapped key, but it can put another cipher's in this one's place (spec 10.2).
pub fn same_key(a: &Value, b: &Value, user: &SymKey) -> bool {
    match (json::some(a, "key"), json::some(b, "key")) {
        (None, None) => true,
        (Some(Value::String(x)), Some(Value::String(y))) => {
            x == y
                || match (crypto::open_key(x, user), crypto::open_key(y, user)) {
                    (Ok(x), Ok(y)) => seal::same(x.bytes(), y.bytes()),
                    _ => false,
                }
        }
        _ => false,
    }
}

/// Opens what is mapped. A field that does not open makes the whole item unreadable: never a blank that would sync back
/// (spec 5.4). A URI whose checksum is wrong is tampering; on an item with its own key a missing one is too.
pub fn open(raw: &Value, user: &SymKey) -> Result<Opened, Unreadable> {
    let kind = sort(raw).map_err(|_| Unreadable)?;
    let key = key_of(raw, user)?;
    let has_key = json::some(raw, "key").is_some();
    let mut content = Content { kind, name: open_optional(raw, "name", &key)?.ok_or(Unreadable)?, notes: open_optional(raw, "notes", &key)?, ..Content::default() };
    let mut uri_unproven = false;
    if kind == LOGIN {
        let login = json::some(raw, "login").ok_or(Unreadable)?;
        content.username = open_optional(login, "username", &key)?;
        content.password = open_optional(login, "password", &key)?;
        content.totp = open_optional(login, "totp", &key)?;
        for (at, uri) in json::get(login, "uris").and_then(Value::as_array).into_iter().flatten().enumerate() {
            let Some(text) = open_optional(uri, "uri", &key)? else { continue };
            match open_optional(uri, "uriChecksum", &key)? {
                Some(sum) if crypto::uri_checksum(&text).as_bytes() == sum.as_bytes() => {}
                Some(_) => return Err(Unreadable),
                None if has_key => return Err(Unreadable),
                None if at == 0 => uri_unproven = true,
                None => {}
            }
            if at == 0 {
                content.uri = Some(text);
            }
        }
    }
    if kind == SSH {
        let ssh = json::some(raw, "sshKey").ok_or(Unreadable)?;
        let (private, public, fingerprint) = (open_optional(ssh, "privateKey", &key)?, open_optional(ssh, "publicKey", &key)?, open_optional(ssh, "keyFingerprint", &key)?);
        match (private, public, fingerprint) {
            (Some(private), Some(public), Some(fingerprint)) if !private.is_empty() && !public.is_empty() && !fingerprint.is_empty() => content.ssh = Some(Ssh { private, public, fingerprint }),
            _ => return Err(Unreadable),
        }
    }
    for field in json::get(raw, "fields").and_then(Value::as_array).into_iter().flatten() {
        let name = open_optional(field, "name", &key)?;
        let value = open_optional(field, "value", &key)?;
        match name.as_deref() {
            Some(MARKER) => content.marker = value,
            Some(ours) if OUR_FIELDS.contains(&ours) => {
                content.fields.push(Field { name: ours.to_string(), value: value.unwrap_or_default(), hidden: json::number(field, "type") == Some(1) });
            }
            _ => {}
        }
    }
    content.fields.sort_by(|a, b| a.name.cmp(&b.name));
    for old in json::get(raw, "passwordHistory").and_then(Value::as_array).into_iter().flatten() {
        open_optional(old, "password", &key)?;
    }
    Ok(Opened {
        attachments: attachments(raw),
        id: json::text(raw, "id").ok_or(Unreadable)?.to_string(),
        content,
        revision: json::text(raw, "revisionDate").filter(|text| parse_time(text).is_some()).ok_or(Unreadable)?.to_string(),
        deleted: json::some(raw, "deletedDate").is_some(),
        has_key,
        uri_unproven,
    })
}

/// A field too long for Bitwarden's server, whichever server this is (spec 7)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooLong;

fn fits(text: &Option<String>, most: usize) -> Result<(), TooLong> {
    match text {
        Some(text) if lengths::encstring(text.len()) > most => Err(TooLong),
        _ => Ok(()),
    }
}

impl Content {
    pub fn check_lengths(&self) -> Result<(), TooLong> {
        fits(&Some(self.name.clone()), lengths::NAME)?;
        fits(&self.notes, lengths::NOTES)?;
        fits(&self.username, lengths::NAME)?;
        fits(&self.password, lengths::PASSWORD)?;
        fits(&self.totp, lengths::NAME)?;
        fits(&self.uri, lengths::URI)?;
        fits(&self.marker, lengths::PASSWORD)?;
        for field in &self.fields {
            fits(&Some(field.name.clone()), lengths::NAME)?;
            fits(&Some(field.value.clone()), lengths::PASSWORD)?;
        }
        if let Some(ssh) = &self.ssh {
            fits(&Some(ssh.private.clone()), lengths::PASSWORD)?;
            fits(&Some(ssh.public.clone()), lengths::PASSWORD)?;
            fits(&Some(ssh.fingerprint.clone()), lengths::NAME)?;
        }
        Ok(())
    }
}

fn sealed(key: &SymKey, text: &Option<String>) -> Value {
    text.as_ref().map_or(Value::Null, |text| json!(crypto::seal_text(key, text)))
}

fn uri_entry(key: &SymKey, uri: &str) -> Value {
    json!({"uri": crypto::seal_text(key, uri), "uriChecksum": crypto::seal_text(key, &crypto::uri_checksum(uri)), "match": null})
}

fn marker_field(key: &SymKey, marker: &str) -> Value {
    json!({"type": 0, "name": crypto::seal_text(key, MARKER), "value": crypto::seal_text(key, marker), "linkedId": null})
}

fn field_of(key: &SymKey, field: &Field) -> Value {
    json!({"type": u8::from(field.hidden), "name": crypto::seal_text(key, &field.name), "value": crypto::seal_text(key, &field.value), "linkedId": null})
}

/// The fields a content carries as ours, the marker first
fn our_fields(key: &SymKey, content: &Content) -> Vec<Value> {
    content.marker.iter().map(|marker| marker_field(key, marker)).chain(content.fields.iter().map(|field| field_of(key, field))).collect()
}

fn ssh_object(key: &SymKey, ssh: &Ssh) -> Value {
    json!({"privateKey": crypto::seal_text(key, &ssh.private), "publicKey": crypto::seal_text(key, &ssh.public), "keyFingerprint": crypto::seal_text(key, &ssh.fingerprint)})
}

/// A new personal item: always a fresh key of its own, whatever the server's version says (spec 6.3, BW10)
pub fn create_body(content: &Content, user_id: &str, user: &SymKey) -> Result<Value, TooLong> {
    content.check_lengths()?;
    let key = SymKey::random();
    let mut body = json!({
        "encryptedFor": user_id,
        "type": content.kind,
        "organizationId": null,
        "folderId": null,
        "key": EncString::seal(user, key.bytes()).text(),
        "name": crypto::seal_text(&key, &content.name),
        "notes": sealed(&key, &content.notes),
        "favorite": false,
        "reprompt": 0,
        "fields": Some(our_fields(&key, content)).filter(|fields| !fields.is_empty()).map_or(Value::Null, Value::Array),
        "passwordHistory": null,
        "lastKnownRevisionDate": null,
        "archivedDate": null,
    });
    if content.kind == LOGIN {
        body["login"] = json!({
            "username": sealed(&key, &content.username),
            "password": sealed(&key, &content.password),
            "passwordRevisionDate": null,
            "totp": sealed(&key, &content.totp),
            "uris": content.uri.as_ref().map_or(Value::Null, |uri| json!([uri_entry(&key, uri)])),
            "autofillOnPageLoad": null,
            "fido2Credentials": null,
        });
    } else if content.kind == SSH {
        body["sshKey"] = ssh_object(&key, content.ssh.as_ref().ok_or(TooLong)?);
    } else {
        body["secureNote"] = json!({"type": 0});
    }
    Ok(body)
}

/// The read-only things a body never carries (spec 6.4)
const NEVER_SENT: &[&str] = &["object", "id", "data", "partialData", "creationDate", "revisionDate", "deletedDate", "edit", "viewPassword", "permissions", "organizationUseTotp", "collectionIds", "attachments", "attachments2"];
const TYPED: &[&str] = &["login", "secureNote", "card", "identity", "sshKey", "bankAccount", "driversLicense", "passport"];
/// The spec's spelling of the keys a body is known to carry, for a server that wrote them in another case
const KNOWN: &[&str] = &["type", "organizationId", "folderId", "key", "name", "notes", "favorite", "reprompt", "fields", "passwordHistory", "archivedDate", "login", "secureNote", "card", "identity", "sshKey"];

/// An update: the server's last raw JSON, less what is read-only, with only the changed fields' EncStrings replaced.
/// Everything else, unknown keys and passkeys included, goes back as it came; `key` is kept; `data` never goes.
pub fn update_body(raw: &Value, old: &Content, new: &Content, user_id: &str, user: &SymKey) -> Result<Value, UpdateRefused> {
    new.check_lengths().map_err(|_| UpdateRefused::TooLong)?;
    let key = key_of(raw, user).map_err(|_| UpdateRefused::Unreadable)?;
    let mut body: Map<String, Value> = raw.as_object().cloned().ok_or(UpdateRefused::Unreadable)?;
    let revision = json::text(raw, "revisionDate").ok_or(UpdateRefused::Unreadable)?.to_string();
    for name in NEVER_SENT {
        json::remove(&mut body, name);
    }
    for name in KNOWN {
        if let Some(spelled) = json::key_of(&body, name)
            && spelled != *name
            && let Some(value) = body.remove(&spelled)
        {
            body.insert(name.to_string(), value);
        }
    }
    let kind = new.kind;
    let typed = match kind {
        LOGIN => "login",
        SSH => "sshKey",
        _ => "secureNote",
    };
    // The typed objects of other types go, when null; one that is not null on the wrong type is the server's business
    for name in TYPED {
        if *name != typed && body.get(*name).is_some_and(Value::is_null) {
            body.remove(*name);
        }
    }
    if new.name != old.name {
        body.insert("name".into(), json!(crypto::seal_text(&key, &new.name)));
    }
    if new.notes != old.notes {
        body.insert("notes".into(), sealed(&key, &new.notes));
    }
    if new.marker != old.marker || new.fields != old.fields {
        let fields = body.entry("fields").or_insert(Value::Null);
        let mut list = fields.as_array().cloned().unwrap_or_default();
        // Only our own fields are taken out; every other field stays as it came, one that does not open included
        list.retain(|field| {
            !json::get(field, "name").and_then(Value::as_str).is_some_and(|name| crypto::open_text(name, &key).is_ok_and(|name| name.as_str() == MARKER || OUR_FIELDS.contains(&name.as_str())))
        });
        list.extend(our_fields(&key, new));
        *fields = if list.is_empty() { Value::Null } else { Value::Array(list) };
    }
    if kind == SSH && new.ssh != old.ssh {
        let mut object: Map<String, Value> = body.get("sshKey").and_then(Value::as_object).cloned().unwrap_or_default();
        if let Some(ssh) = &new.ssh {
            for (name, value) in [("privateKey", &ssh.private), ("publicKey", &ssh.public), ("keyFingerprint", &ssh.fingerprint)] {
                json::set(&mut object, name, json!(crypto::seal_text(&key, value)));
            }
        }
        body.insert("sshKey".into(), Value::Object(object));
    }
    if kind == LOGIN {
        let mut login: Map<String, Value> = body.get("login").and_then(Value::as_object).cloned().unwrap_or_default();
        json::remove(&mut login, "uri");
        let mut set = |name: &str, value: Value| json::set(&mut login, name, value);
        if new.username != old.username {
            set("username", sealed(&key, &new.username));
        }
        if new.totp != old.totp {
            set("totp", sealed(&key, &new.totp));
        }
        if new.password != old.password {
            // The old password goes into the history as the ciphertext that came, newest first, and nothing in it is lost
            let old_password = json::get(raw, "login").and_then(|login| json::get(login, "password")).cloned().unwrap_or(Value::Null);
            set("password", sealed(&key, &new.password));
            set("passwordRevisionDate", json!(now_text()));
            if !old_password.is_null() {
                let mut history = json::get(raw, "passwordHistory").and_then(Value::as_array).cloned().unwrap_or_default();
                history.insert(0, json!({"password": old_password, "lastUsedDate": now_text()}));
                body.insert("passwordHistory".into(), Value::Array(history));
            }
        }
        if new.uri != old.uri {
            let mut uris = json::get(&Value::Object(login.clone()), "uris").and_then(Value::as_array).cloned().unwrap_or_default();
            match (&new.uri, uris.is_empty()) {
                (Some(uri), true) => uris.push(uri_entry(&key, uri)),
                (Some(uri), false) => {
                    // The first URI's text and checksum, keeping its match and anything else it carries
                    if let Some(first) = uris[0].as_object_mut() {
                        json::set(first, "uri", json!(crypto::seal_text(&key, uri)));
                        json::set(first, "uriChecksum", json!(crypto::seal_text(&key, &crypto::uri_checksum(uri))));
                    }
                }
                (None, false) => {
                    uris.remove(0);
                }
                (None, true) => {}
            }
            json::set(&mut login, "uris", if uris.is_empty() { Value::Null } else { Value::Array(uris) });
        }
        body.insert("login".into(), Value::Object(login));
    }
    body.insert("encryptedFor".into(), json!(user_id));
    body.insert("lastKnownRevisionDate".into(), json!(revision));
    Ok(Value::Object(body))
}

/// Every type 2 EncString in a value, opened under one key and sealed again under another. False if one does not open.
fn reseal(value: &mut Value, from: &SymKey, to: &SymKey) -> bool {
    match value {
        Value::String(text) if crypto::is_type2(text) => match crypto::open_text(text, from) {
            Ok(plain) => {
                *text = crypto::seal_text(to, &plain);
                true
            }
            Err(_) => false,
        },
        Value::Array(items) => items.iter_mut().all(|item| reseal(item, from, to)),
        Value::Object(object) => object.values_mut().all(|item| reseal(item, from, to)),
        _ => true,
    }
}

/// An update that changes nothing but the key: a cipher that has none is moved to a key of its own (spec 6.3), every EncString it
/// holds opened under the user key and sealed again under the new one, the passkeys and fields this client does not read included.
/// Refused for a cipher with attachments, whose files hang off the key that opened them, and for any string that does not open.
pub fn rekeyed_body(raw: &Value, user_id: &str, user: &SymKey) -> Result<Value, UpdateRefused> {
    let attached = json::some(raw, "attachments").is_some_and(|attachments| attachments.as_array().is_none_or(|list| !list.is_empty()));
    if json::some(raw, "key").is_some() || attached {
        return Err(UpdateRefused::Unreadable);
    }
    let opened = open(raw, user).map_err(|_| UpdateRefused::Unreadable)?;
    let mut body = update_body(raw, &opened.content, &opened.content, user_id, user)?;
    // An address with its own key has to carry its checksum (5.4), which an old item's lacks: made here, under the key it is about to leave
    if let Some(login) = body.get_mut("login").and_then(Value::as_object_mut)
        && let Some(name) = json::key_of(login, "uris")
        && let Some(Value::Array(uris)) = login.get_mut(&name)
    {
        for entry in uris.iter_mut().filter_map(Value::as_object_mut) {
            let Some(uri_name) = json::key_of(entry, "uri") else { continue };
            let Some(plain) = entry.get(&uri_name).and_then(Value::as_str).and_then(|text| crypto::open_text(text, user).ok()) else { continue };
            if json::key_of(entry, "uriChecksum").is_none_or(|sum| entry.get(&sum).is_none_or(Value::is_null)) {
                json::set(entry, "uriChecksum", json!(crypto::seal_text(user, &crypto::uri_checksum(&plain))));
            }
        }
    }
    let key = SymKey::random();
    if !reseal(&mut body, user, &key) {
        return Err(UpdateRefused::Unreadable);
    }
    body["key"] = json!(EncString::seal(user, key.bytes()).text());
    Ok(body)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateRefused {
    TooLong,
    Unreadable,
}

/// A secret that wipes itself, for what a cipher body carries on its way out
pub type Body = Zeroizing<Vec<u8>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> SymKey {
        SymKey::from_slice(&(0u8..64).collect::<Vec<_>>()).unwrap()
    }

    const ID: &str = "3f2504e0-4f89-11d3-9a0c-0305e82c3301";
    const USER: &str = "11111111-2222-3333-4444-555555555555";

    fn login() -> Content {
        Content {
            kind: LOGIN,
            name: "Example".into(),
            username: Some("person".into()),
            password: Some("hunter2".into()),
            uri: Some("https://example.test/login".into()),
            ..Content::default()
        }
    }

    /// What a server sends back for a created body: the body, with what the server adds
    fn as_served(mut body: Value) -> Value {
        body["id"] = json!(ID);
        body["object"] = json!("cipherDetails");
        body["revisionDate"] = json!("2026-11-02T10:00:00.123456Z");
        body["creationDate"] = json!("2026-11-02T10:00:00.123456Z");
        body["deletedDate"] = Value::Null;
        body["edit"] = json!(true);
        body["collectionIds"] = json!([]);
        body["attachments"] = Value::Null;
        body.as_object_mut().unwrap().remove("encryptedFor");
        body.as_object_mut().unwrap().remove("lastKnownRevisionDate");
        body
    }

    #[test]
    fn a_created_item_opens_and_has_its_own_key() {
        let body = create_body(&login(), USER, &user()).unwrap();
        assert_eq!(body["encryptedFor"], json!(USER));
        assert!(body["key"].as_str().unwrap().starts_with("2."));
        assert!(body.get("data").is_none());
        let opened = open(&as_served(body), &user()).unwrap();
        assert!(opened.has_key);
        assert_eq!(opened.content, login());
        assert!(!opened.uri_unproven);
    }

    #[test]
    fn a_put_carries_everything_it_was_given() {
        let mut raw = as_served(create_body(&login(), USER, &user()).unwrap());
        // What this client does not read, and keys it does not know, in the cipher and its typed object
        raw["login"]["fido2Credentials"] = json!([{"credentialId": "2.a|b|c", "keyValue": "2.d|e|f", "creationDate": "2026-01-01T00:00:00Z"}]);
        raw["login"]["autofillOnPageLoad"] = json!(true);
        raw["login"]["uri"] = json!("2.copy|of|first");
        raw["login"]["somethingNew"] = json!({"x": 1});
        raw["futureField"] = json!([1, 2, 3]);
        raw["folderId"] = json!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        raw["favorite"] = json!(true);
        raw["reprompt"] = json!(1);
        raw["archivedDate"] = json!("2026-10-01T00:00:00Z");
        raw["data"] = json!("{\"name\":\"stale\"}");
        raw["card"] = Value::Null;
        let item = key_of(&raw, &user()).unwrap();
        let second = json!({"uri": crypto::seal_text(&item, "https://second.test"), "uriChecksum": crypto::seal_text(&item, &crypto::uri_checksum("https://second.test")), "match": 3});
        raw["login"]["uris"].as_array_mut().unwrap().push(second);
        let fields_before = raw["fields"].clone();
        let mut new = login();
        new.password = Some("new password".into());
        let body = update_body(&raw, &login(), &new, USER, &user()).unwrap();
        // Kept as they came
        for name in ["key", "name", "notes", "folderId", "favorite", "reprompt", "archivedDate", "futureField", "type"] {
            assert_eq!(body[name], raw[name], "{name}");
        }
        assert_eq!(body["fields"], fields_before);
        for name in ["fido2Credentials", "autofillOnPageLoad", "somethingNew", "username", "uris", "totp"] {
            assert_eq!(body["login"][name], raw["login"][name], "{name}");
        }
        // Never sent
        for name in NEVER_SENT {
            assert!(body.get(*name).is_none(), "{name}");
        }
        assert!(body["login"].get("uri").is_none());
        assert!(body.get("card").is_none());
        assert_eq!(body["lastKnownRevisionDate"], json!("2026-11-02T10:00:00.123456Z"));
        assert_eq!(body["encryptedFor"], json!(USER));
        // The new password, and the old one's ciphertext first in the history
        assert_ne!(body["login"]["password"], raw["login"]["password"]);
        assert_eq!(body["passwordHistory"][0]["password"], raw["login"]["password"]);
        assert!(body["login"]["passwordRevisionDate"].as_str().unwrap().ends_with('Z'));
        let mut served = body.clone();
        served["id"] = json!(ID);
        served["revisionDate"] = json!("2026-11-02T10:00:01Z");
        assert_eq!(open(&served, &user()).unwrap().content.password.as_deref(), Some("new password"));
    }

    #[test]
    fn a_changed_address_changes_only_the_first_uri() {
        let raw = as_served(create_body(&login(), USER, &user()).unwrap());
        let mut new = login();
        new.uri = Some("https://other.test".into());
        let body = update_body(&raw, &login(), &new, USER, &user()).unwrap();
        let mut served = body.clone();
        served["id"] = json!(ID);
        served["revisionDate"] = json!("2026-11-02T10:00:01Z");
        assert_eq!(open(&served, &user()).unwrap().content.uri.as_deref(), Some("https://other.test"));
        assert_eq!(body["login"]["uris"][0]["match"], Value::Null);
    }

    #[test]
    fn foreign_items_are_left_alone() {
        let ours = as_served(create_body(&login(), USER, &user()).unwrap());
        assert_eq!(sort(&ours), Ok(LOGIN));
        let with = |name: &str, value: Value| {
            let mut raw = ours.clone();
            raw[name] = value;
            sort(&raw)
        };
        assert_eq!(with("organizationId", json!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")), Err(Foreign::Organization));
        assert_eq!(with("type", json!(9)), Err(Foreign::OtherType));
        assert_eq!(with("type", json!(3)), Err(Foreign::OtherType));
        assert_eq!(with("data", json!("{\"format_version\":1}")), Err(Foreign::Sealed));
        assert_eq!(with("partialData", json!({})), Err(Foreign::Withheld));
        assert_eq!(with("name", Value::Null), Err(Foreign::Damaged));
        assert_eq!(with("key", json!("0.AAAA|BBBB")), Err(Foreign::OldFormat));
        assert_eq!(with("notes", json!("4.AAAA")), Err(Foreign::OldFormat));
        assert_eq!(with("id", json!("../accounts")), Err(Foreign::Damaged));
        assert!(newer_encryption(&{
            let mut raw = ours.clone();
            raw["notes"] = json!("7.AAAA");
            raw
        }));
        assert!(!newer_encryption(&ours));
    }

    #[test]
    fn a_tampered_uri_or_field_makes_the_item_unreadable() {
        let raw = as_served(create_body(&login(), USER, &user()).unwrap());
        let key = key_of(&raw, &user()).unwrap();
        // A URI swapped for another with the old checksum (BW06's way in)
        let mut swapped = raw.clone();
        swapped["login"]["uris"][0]["uri"] = json!(crypto::seal_text(&key, "https://attacker.test"));
        assert!(open(&swapped, &user()).is_err());
        let mut no_sum = raw.clone();
        no_sum["login"]["uris"][0]["uriChecksum"] = Value::Null;
        assert!(open(&no_sum, &user()).is_err());
        // A password's ciphertext moved into the username: it opens, it is a change the person sees, never a blank
        let mut moved = raw.clone();
        moved["login"]["username"] = raw["login"]["password"].clone();
        assert_eq!(open(&moved, &user()).unwrap().content.username.as_deref(), Some("hunter2"));
        // A field under another key
        let mut foreign = raw.clone();
        foreign["notes"] = json!(crypto::seal_text(&SymKey::random(), "x"));
        assert!(open(&foreign, &user()).is_err());
        // A 32-byte item key under a good MAC
        let mut short = raw.clone();
        short["key"] = json!(EncString::seal(&user(), &[1u8; 32]).text());
        assert!(open(&short, &user()).is_err());
    }

    #[test]
    fn times_are_parsed_never_compared_as_text() {
        let seven = parse_time("2026-11-02T10:11:12.1234567Z").unwrap();
        let six = parse_time("2026-11-02T10:11:12.123456Z").unwrap();
        assert_eq!(seven - six, 700);
        assert_eq!(parse_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_time("2000-03-01T00:00:00Z"), Some(951_868_800 * 1_000_000_000));
        assert!(parse_time("2026-13-01T00:00:00Z").is_none());
        assert!(parse_time("yesterday").is_none());
        assert!(parse_time(&now_text()).is_some());
    }

    #[test]
    fn too_long_is_refused_before_anything_goes() {
        let mut content = login();
        content.name = "x".repeat(688);
        assert_eq!(create_body(&content, USER, &user()).err(), Some(TooLong));
        content.name = "x".repeat(687);
        assert!(create_body(&content, USER, &user()).is_ok());
        content.notes = Some("x".repeat(7440));
        assert_eq!(create_body(&content, USER, &user()).err(), Some(TooLong));
    }
}
