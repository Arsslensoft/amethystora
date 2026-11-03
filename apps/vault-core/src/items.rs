//! The items a vault holds, as they are sealed. A name here is stored forever, so a field is never renamed. An account,
//! a backup-code service and a code are BackMyCodes' JSON (plans/vault-bmc/spec-formats.md, section 2), keys in that
//! order, with the Vault's own keys after them, which BackMyCodes keeps as unknown keys and writes back.

use std::fmt;
use std::io::{self, Write};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use zeroize::{Zeroize, Zeroizing};

/// What an authenticated `amethystora-vault:<kind>:<id>` calls each item
pub const KIND_PASSWORD: &str = "password";
pub const KIND_ACCOUNT: &str = "authenticator";
pub const KIND_BACKUP_CODES: &str = "backup-codes";

/// A password keeps this many before it
pub const PASSWORD_HISTORY: usize = 5;

/// Each bound on an entry read from elsewhere: past it the entry is refused, never cut
pub const MAX_NAME: usize = 1024;
pub const MAX_NOTE: usize = 64 * 1024;
pub const MAX_TAGS: usize = 64;
pub const MAX_SECRET_CHARS: usize = 1024;
pub const MAX_CODE: usize = 256;
pub const MAX_CODES: usize = 4096;
/// An entry's unknown keys, written as JSON
pub const MAX_UNKNOWN: usize = 64 * 1024;
/// An id and a BackMyCodes space's id, which are a UUID or six bytes of hex where they come from
pub const MAX_ID: usize = 256;
/// An algorithm's name, `SSL3-SHA1` the longest known
pub const MAX_ALGORITHM: usize = 32;
/// One item as JSON: what one request carries, less room for the request around it
pub const MAX_ITEM: usize = amethystora_vault_client::MAX_DATA - 4096;
/// What marking a code used writes into its JSON, at most: `,"usedAt":` and a number of up to 20 characters. A service
/// is taken, and grown, only while every one of its codes still has this room.
pub const USED_AT_ROOM: usize = 32;

/// Text that wipes itself, and prints nothing
#[derive(Clone, Default)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    pub fn new(text: impl Into<String>) -> Secret {
        Secret(Zeroizing::new(text.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Zeroizing<String>> for Secret {
    fn from(text: Zeroizing<String>) -> Secret {
        Secret(text)
    }
}

impl AsRef<str> for Secret {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[secret]")
    }
}

impl Serialize for Secret {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Secret, D::Error> {
        String::deserialize(deserializer).map(Secret::new)
    }
}

/// Keys this version does not know, kept and written back. They may hold anything, so they print nothing either.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Unknown(pub Map<String, Value>);

impl fmt::Debug for Unknown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} unknown keys]", self.0.len())
    }
}

impl Unknown {
    pub fn fits(&self) -> bool {
        self.0.is_empty() || json_size(self) <= MAX_UNKNOWN
    }
}

impl Drop for Unknown {
    fn drop(&mut self) {
        self.0.values_mut().for_each(wipe);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OtpKind {
    Totp,
    Hotp,
    Steam,
}

fn sha1_name() -> String {
    "SHA1".to_string()
}
fn six() -> i64 {
    6
}
fn thirty() -> i64 {
    30
}

/// An authenticator account. `kind` left out means TOTP, as in accounts from before HOTP and Steam.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    #[serde(default)]
    pub id: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<OtpKind>,
    #[serde(default)]
    pub issuer: String,
    #[serde(default)]
    pub label: String,
    pub secret: Secret,
    #[serde(default = "sha1_name")]
    pub algorithm: String,
    #[serde(default = "six")]
    pub digits: i64,
    #[serde(default = "thirty")]
    pub period: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<Secret>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_backup_codes: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,
    #[serde(default)]
    pub added_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
    /// The Vault's own: removed, and kept for sync's trash
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
    #[serde(flatten)]
    pub unknown: Unknown,
}

impl Default for Account {
    fn default() -> Account {
        Account {
            id: String::new(),
            kind: None,
            issuer: String::new(),
            label: String::new(),
            secret: Secret::default(),
            algorithm: sha1_name(),
            digits: 6,
            period: 30,
            counter: None,
            pinned: None,
            tags: None,
            order: None,
            note: None,
            no_backup_codes: None,
            space_id: None,
            added_at: 0,
            updated_at: None,
            deleted_at: None,
            unknown: Unknown::default(),
        }
    }
}

impl Account {
    pub fn otp_kind(&self) -> OtpKind {
        self.kind.unwrap_or(OtpKind::Totp)
    }
}

/// A service's backup codes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupService {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub service_name: String,
    #[serde(default)]
    pub codes: Vec<BackupCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<Secret>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,
    #[serde(default)]
    pub added_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
    /// The rest are the Vault's own
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
    #[serde(flatten)]
    pub unknown: Unknown,
}

impl BackupService {
    /// Its JSON as it is, and the room each code needs to be marked used: never past [`MAX_ITEM`]
    pub fn size_with_room(&self) -> usize {
        json_size(self) + self.codes.len() * USED_AT_ROOM
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCode {
    pub code: Secret,
    #[serde(default)]
    pub used: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_at: Option<i64>,
    #[serde(flatten)]
    pub unknown: Unknown,
}

impl BackupCode {
    pub fn new(code: impl Into<String>) -> BackupCode {
        BackupCode { code: Secret::new(code), ..BackupCode::default() }
    }

    pub fn mark_used(&mut self, now_ms: i64) {
        self.used = true;
        self.used_at = Some(now_ms);
    }

    pub fn mark_unused(&mut self) {
        self.used = false;
        self.used_at = None;
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OldPassword {
    pub password: Secret,
    pub changed_at: i64,
}

/// A login: the last five passwords stay beside the current one, with when each stopped being it
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Password {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub address: String,
    pub password: Secret,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<Secret>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<OldPassword>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
    #[serde(default)]
    pub added_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
    #[serde(flatten)]
    pub unknown: Unknown,
}

impl Password {
    /// The new password in place, the old one first in the history unless it was empty or did not change
    pub fn set_password(&mut self, new: Secret, now_ms: i64) {
        if !self.password.is_empty() && self.password.as_str() != new.as_str() {
            let old = std::mem::replace(&mut self.password, new);
            self.history.insert(0, OldPassword { password: old, changed_at: now_ms });
            self.history.truncate(PASSWORD_HISTORY);
        } else {
            self.password = new;
        }
        self.updated_at = Some(now_ms);
    }
}

/// One item of any kind, for the store and for sync, which go by its id, its change time and its tombstone
#[derive(Clone, Debug)]
pub enum Item {
    Password(Password),
    Account(Account),
    BackupCodes(BackupService),
}

impl Item {
    pub fn kind(&self) -> &'static str {
        match self {
            Item::Password(_) => KIND_PASSWORD,
            Item::Account(_) => KIND_ACCOUNT,
            Item::BackupCodes(_) => KIND_BACKUP_CODES,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Item::Password(i) => &i.id,
            Item::Account(i) => &i.id,
            Item::BackupCodes(i) => &i.id,
        }
    }

    /// When it last changed: the change time, or when it was added if it never has
    pub fn changed_at(&self) -> i64 {
        match self {
            Item::Password(i) => i.updated_at.unwrap_or(i.added_at),
            Item::Account(i) => i.updated_at.unwrap_or(i.added_at),
            Item::BackupCodes(i) => i.updated_at.unwrap_or(i.added_at),
        }
    }

    pub fn deleted_at(&self) -> Option<i64> {
        match self {
            Item::Password(i) => i.deleted_at,
            Item::Account(i) => i.deleted_at,
            Item::BackupCodes(i) => i.deleted_at,
        }
    }

    /// Removed, as sync needs it: the item stays as a tombstone until its trash is emptied
    pub fn mark_deleted(&mut self, now_ms: i64) {
        match self {
            Item::Password(i) => (i.deleted_at, i.updated_at) = (Some(now_ms), Some(now_ms)),
            Item::Account(i) => (i.deleted_at, i.updated_at) = (Some(now_ms), Some(now_ms)),
            Item::BackupCodes(i) => (i.deleted_at, i.updated_at) = (Some(now_ms), Some(now_ms)),
        }
    }

    /// The JSON the store seals
    pub fn to_json(&self) -> Result<Zeroizing<Vec<u8>>, serde_json::Error> {
        match self {
            Item::Password(i) => to_json_wiping(i),
            Item::Account(i) => to_json_wiping(i),
            Item::BackupCodes(i) => to_json_wiping(i),
        }
    }

    /// What the store opened, read by the kind its authenticated name says. `None` for a kind that is not an item's.
    pub fn from_json(kind: &str, json: &[u8]) -> Option<Item> {
        match kind {
            KIND_PASSWORD => serde_json::from_slice(json).ok().map(Item::Password),
            KIND_ACCOUNT => serde_json::from_slice(json).ok().map(Item::Account),
            KIND_BACKUP_CODES => serde_json::from_slice(json).ok().map(Item::BackupCodes),
            _ => None,
        }
    }
}

/// A random UUID, the shape an item's id always has
pub fn new_id() -> Result<String, getrandom::Error> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut b = [0u8; 16];
    getrandom::fill(&mut b)?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let mut id = String::with_capacity(36);
    for (i, byte) in b.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            id.push('-');
        }
        id.push(HEX[(byte >> 4) as usize] as char);
        id.push(HEX[(byte & 15) as usize] as char);
    }
    Ok(id)
}

/// Whether an id has the shape of [`new_id`]. An id from a file is untrusted, and it reaches paths.
pub fn is_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

/// What a JavaScript `\s` matches: not `str::trim`, which takes U+0085 and leaves U+FEFF
pub fn is_js_space(c: char) -> bool {
    matches!(c,
        '\u{9}'..='\u{D}' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{2028}' | '\u{2029}'
        | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}')
}

pub fn js_trim(text: &str) -> &str {
    text.trim_matches(is_js_space)
}

/// A growing buffer that wipes each buffer it leaves behind, for JSON with secrets in it
#[derive(Default)]
pub(crate) struct WipingWriter(pub(crate) Zeroizing<Vec<u8>>);

impl Write for WipingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let needed = self.0.len() + buf.len();
        if needed > self.0.capacity() {
            let mut bigger = Zeroizing::new(Vec::with_capacity(needed.max(self.0.capacity() * 2).max(256)));
            bigger.extend_from_slice(&self.0);
            self.0 = bigger;
        }
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Compact JSON in a buffer that wipes itself, copies left behind by growing included
pub fn to_json_wiping<T: Serialize>(value: &T) -> Result<Zeroizing<Vec<u8>>, serde_json::Error> {
    let mut writer = WipingWriter::default();
    serde_json::to_writer(&mut writer, value)?;
    Ok(writer.0)
}

/// Counts what would be written, and keeps none of it
struct Counter(usize);

impl Write for Counter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// How long the compact JSON of a value is, without writing it anywhere
pub fn json_size<T: Serialize>(value: &T) -> usize {
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, value).map_or(usize::MAX, |()| counter.0)
}

/// UTF-8 with each bad sequence replaced by U+FFFD, as `String::from_utf8_lossy`, in a buffer of the right size that
/// wipes itself: what a decrypted file holds is never copied into a plain String
pub(crate) fn lossy(bytes: &[u8]) -> Zeroizing<String> {
    let length = bytes.utf8_chunks().map(|c| c.valid().len() + if c.invalid().is_empty() { 0 } else { 3 }).sum();
    let mut text = Zeroizing::new(String::with_capacity(length));
    for chunk in bytes.utf8_chunks() {
        text.push_str(chunk.valid());
        if !chunk.invalid().is_empty() {
            text.push('\u{FFFD}');
        }
    }
    text
}

/// A file's text: [`lossy`], and up to two byte order marks gone, moved in place
pub(crate) fn file_text(bytes: &[u8]) -> Zeroizing<String> {
    let mut text = lossy(bytes);
    let marks = text.chars().take(2).take_while(|c| *c == '\u{FEFF}').count();
    text.drain(..marks * '\u{FEFF}'.len_utf8());
    text
}

/// Parsed JSON from a file, whose strings are wiped when it goes
pub(crate) struct WipedValue(pub(crate) Value);

impl Drop for WipedValue {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// Built as serde_json's own Value is, but every part held so that a parse that stops part way, or a key given twice,
/// wipes what it had already read
impl<'de> Deserialize<'de> for WipedValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<WipedValue, D::Error> {
        deserializer.deserialize_any(Building)
    }
}

struct Building;

impl<'de> serde::de::Visitor<'de> for Building {
    type Value = WipedValue;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON")
    }

    fn visit_bool<E>(self, v: bool) -> Result<WipedValue, E> {
        Ok(WipedValue(Value::Bool(v)))
    }

    fn visit_i64<E>(self, v: i64) -> Result<WipedValue, E> {
        Ok(WipedValue(Value::Number(v.into())))
    }

    fn visit_u64<E>(self, v: u64) -> Result<WipedValue, E> {
        Ok(WipedValue(Value::Number(v.into())))
    }

    fn visit_f64<E>(self, v: f64) -> Result<WipedValue, E> {
        Ok(WipedValue(serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)))
    }

    fn visit_str<E>(self, v: &str) -> Result<WipedValue, E> {
        Ok(WipedValue(Value::String(v.to_owned())))
    }

    fn visit_string<E>(self, v: String) -> Result<WipedValue, E> {
        Ok(WipedValue(Value::String(v)))
    }

    fn visit_unit<E>(self) -> Result<WipedValue, E> {
        Ok(WipedValue(Value::Null))
    }

    fn visit_none<E>(self) -> Result<WipedValue, E> {
        Ok(WipedValue(Value::Null))
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<WipedValue, D::Error> {
        WipedValue::deserialize(deserializer)
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<WipedValue, A::Error> {
        let mut out = WipedValue(Value::Array(Vec::new()));
        while let Some(mut item) = seq.next_element::<WipedValue>()? {
            if let Value::Array(list) = &mut out.0 {
                list.push(std::mem::take(&mut item.0));
            }
        }
        Ok(out)
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<WipedValue, A::Error> {
        let mut out = WipedValue(Value::Object(Map::new()));
        while let Some(key) = map.next_key::<String>()? {
            let mut item = map.next_value::<WipedValue>()?;
            if let Value::Object(object) = &mut out.0
                && let Some(mut before) = object.insert(key, std::mem::take(&mut item.0))
            {
                wipe(&mut before);
            }
        }
        Ok(out)
    }
}

pub(crate) fn wipe(value: &mut Value) {
    match value {
        Value::String(s) => s.zeroize(),
        Value::Array(list) => list.iter_mut().for_each(wipe),
        Value::Object(map) => map.values_mut().for_each(wipe),
        _ => {}
    }
}

/// How much parsed JSON a file may make: each node costs some 100 bytes, against the few it may take to write
const MAX_JSON_NODES: usize = 1_000_000;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum JsonRefusal {
    NotJson,
    TooLarge,
}

/// Parse JSON from elsewhere, counting its nodes first. serde_json itself stops at 128 levels of nesting.
pub(crate) fn parse_json(text: &[u8]) -> Result<WipedValue, JsonRefusal> {
    let (mut nodes, mut in_string, mut escaped) = (0usize, false, false);
    for &b in text {
        if in_string {
            match b {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
        } else {
            match b {
                b'"' => in_string = true,
                b',' | b':' | b'[' | b'{' => nodes += 1,
                _ => {}
            }
        }
    }
    if nodes > MAX_JSON_NODES {
        return Err(JsonRefusal::TooLarge);
    }
    let mut reader = serde_json::Deserializer::from_slice(text);
    let value = WipedValue::deserialize(&mut reader).map_err(|_| JsonRefusal::NotJson)?;
    reader.end().map_err(|_| JsonRefusal::NotJson)?;
    Ok(value)
}
