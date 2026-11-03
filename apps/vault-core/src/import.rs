//! Other authenticator apps' exports, read: Google's otpauth-migration, Aegis and 2FAS (plain or encrypted), Bitwarden's
//! unencrypted JSON and a text of links (plans/vault-bmc/spec-formats.md, sections 10 and 11, and the Coordinator's
//! decisions at its end). These parse untrusted files, so they run in the client, never in the service, which receives
//! only finished items (decision 7). Every count has a bound, and a refusal is a fixed name.

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::bmc::{self, BmcError, Contents, MAX_ENTRIES, MAX_FILE, gcm_open, pbkdf2_sha256};
use crate::items::{
    Account, BackupService, JsonRefusal, MAX_NAME, MAX_NOTE, MAX_TAGS, OtpKind, Secret, WipedValue, file_text, is_id, js_trim,
    lossy, new_id, parse_json,
};
use crate::otp;
use crate::otpauth::{self, MAX_COUNTER, form_decode};

/// Aegis writes n=2^15, r=8, p=1, about 32 MiB. A file from anywhere gets no more than this in all: one budget of
/// 128 * r * n * p, its memory and with p its time, summed over the slots tried (decision 4), so that sixteen slots at
/// the cap cost no more than one.
pub const MAX_AEGIS_SCRYPT: u64 = 256 * 1024 * 1024;
/// Password slots tried in a file, in turn
pub const MAX_SLOTS: usize = 16;
/// A migration link, which is one QR code in real life
const MAX_MIGRATION_LINK: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    BackMyCodes,
    Aegis,
    TwoFas,
    Bitwarden,
    Google,
    Links,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Detected {
    pub source: Source,
    pub encrypted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// Not a format this reads, or decrypted to the wrong shape
    Unsupported,
    /// Parsed, with no accounts and nothing skipped
    Empty,
    /// Encrypted, and no password given. Reads as a wrong password.
    PasswordRequired,
    BadPassword,
    /// Bitwarden with `encrypted: true`, whatever the password: export it again unencrypted
    BitwardenEncrypted,
    /// An Aegis file whose password slots each ask for more memory than [`MAX_AEGIS_SCRYPT`], said in a plain sentence
    TooMuchMemory,
    /// BackMyCodes' own export of a version this does not read: `Please update the app`
    UnsupportedVersion,
    /// Past another bound
    TooLarge,
    /// A failure of the machine's own
    Crypto,
}

/// What was read, for the window to show before anything is saved
#[derive(Debug)]
pub struct Imported {
    pub source: Source,
    pub accounts: Vec<Account>,
    pub services: Vec<BackupService>,
    /// Entries that could not be represented, which the window counts
    pub skipped: usize,
    /// BackMyCodes' own, come unencrypted: a plain file, which takes a warning
    pub plain: bool,
}

fn too_large(why: JsonRefusal) -> ImportError {
    match why {
        JsonRefusal::TooLarge => ImportError::TooLarge,
        JsonRefusal::NotJson => ImportError::Unsupported,
    }
}

fn detect_value(value: &Value) -> Option<Detected> {
    let o = value.as_object()?;
    if bmc::is_export(value) {
        let encrypted = o.get("scheme").and_then(Value::as_str) != Some("plaintext");
        return Some(Detected { source: Source::BackMyCodes, encrypted });
    }
    if o.get("header").is_some_and(Value::is_object) {
        match o.get("db") {
            Some(Value::Object(_)) => return Some(Detected { source: Source::Aegis, encrypted: false }),
            Some(Value::String(_)) => return Some(Detected { source: Source::Aegis, encrypted: true }),
            _ => {}
        }
    }
    if o.get("services").is_some_and(Value::is_array) && (o.contains_key("schemaVersion") || o.contains_key("servicesEncrypted")) {
        let encrypted = o.get("servicesEncrypted").and_then(Value::as_str).is_some_and(|s| !s.is_empty());
        return Some(Detected { source: Source::TwoFas, encrypted });
    }
    let bitwarden = o.get("items").is_some_and(Value::is_array)
        || o.get("passwordProtected") == Some(&Value::Bool(true))
        || o.contains_key("encKeyValidation_DO_NOT_EDIT");
    if let (true, Some(encrypted)) = (bitwarden, o.get("encrypted").and_then(Value::as_bool)) {
        return Some(Detected { source: Source::Bitwarden, encrypted });
    }
    None
}

fn has_scheme(line: &str, scheme: &str) -> bool {
    line.get(..scheme.len()).is_some_and(|head| head.eq_ignore_ascii_case(scheme))
}

fn is_migration(line: &str) -> bool {
    has_scheme(line, "otpauth-migration://")
}

fn is_link_line(line: &str) -> bool {
    has_scheme(line, "otpauth://") || is_migration(line) || has_scheme(line, "steam://")
}

/// Lines split on LF or CRLF, a lone CR being no break: each trimmed, and the empty ones gone. One past the bound is
/// enough to refuse, so no more are kept.
fn lines_of(text: &str) -> Vec<&str> {
    text.split('\n').map(js_trim).filter(|line| !line.is_empty()).take(MAX_ENTRIES + 1).collect()
}

fn detect_lines(lines: &[&str]) -> Option<Detected> {
    if !lines.iter().any(|line| is_link_line(line)) {
        return None;
    }
    let source = if lines.iter().all(|line| is_migration(line)) { Source::Google } else { Source::Links };
    Some(Detected { source, encrypted: false })
}

/// What a file is, by section 10.1, with nothing opened. `None` is not a known format.
pub fn detect(bytes: &[u8]) -> Option<Detected> {
    if bytes.len() > MAX_FILE {
        return None;
    }
    let text = file_text(bytes);
    match parse_json(text.as_bytes()) {
        Ok(value) => detect_value(&value.0),
        Err(JsonRefusal::TooLarge) => None,
        Err(JsonRefusal::NotJson) => Some(lines_of(&text)).filter(|lines| lines.len() <= MAX_ENTRIES).and_then(|lines| detect_lines(&lines)),
    }
}

/// A file of any format this reads. BackMyCodes' own export comes first, then the others.
pub fn read_file(bytes: &[u8], password: &str) -> Result<Imported, ImportError> {
    if bytes.len() > MAX_FILE {
        return Err(ImportError::TooLarge);
    }
    let text = file_text(bytes);
    let parsed = parse_json(text.as_bytes());
    if parsed.as_ref().is_ok_and(|value| bmc::is_export(&value.0)) {
        // The export reads its own text again: one tree at a time
        drop(parsed);
        return read_export(text.as_bytes(), password);
    }
    read_foreign_parsed(&text, parsed, password)
}

fn read_export(text: &[u8], password: &str) -> Result<Imported, ImportError> {
    let contents: Contents = bmc::read_export(text, password).map_err(|why| match why {
        BmcError::BadPassword => ImportError::BadPassword,
        BmcError::UnsupportedVersion => ImportError::UnsupportedVersion,
        BmcError::TooLarge => ImportError::TooLarge,
        BmcError::Crypto => ImportError::Crypto,
        _ => ImportError::Unsupported,
    })?;
    if contents.accounts.is_empty() && contents.services.is_empty() && contents.skipped == 0 {
        return Err(ImportError::Empty);
    }
    Ok(Imported {
        source: Source::BackMyCodes,
        accounts: contents.accounts,
        services: contents.services,
        skipped: contents.skipped,
        plain: contents.plain,
    })
}

/// A foreign file, BackMyCodes' own being refused here as not foreign. An export's text is left to [`read_file`].
pub fn read_foreign(bytes: &[u8], password: &str) -> Result<Imported, ImportError> {
    if bytes.len() > MAX_FILE {
        return Err(ImportError::TooLarge);
    }
    let text = file_text(bytes);
    read_foreign_parsed(&text, parse_json(text.as_bytes()), password)
}

fn read_foreign_parsed(text: &str, parsed: Result<WipedValue, JsonRefusal>, password: &str) -> Result<Imported, ImportError> {
    let read = match parsed {
        Ok(value) => {
            let detected = detect_value(&value.0).ok_or(ImportError::Unsupported)?;
            match detected.source {
                Source::BackMyCodes => return Err(ImportError::Unsupported),
                Source::Aegis => read_aegis(&value.0, detected.encrypted, password)?,
                Source::TwoFas => read_two_fas(&value.0, detected.encrypted, password)?,
                _ => read_bitwarden(&value.0, detected.encrypted)?,
            }
        }
        Err(JsonRefusal::TooLarge) => return Err(ImportError::TooLarge),
        Err(JsonRefusal::NotJson) => {
            let lines = lines_of(text);
            if lines.len() > MAX_ENTRIES {
                return Err(ImportError::TooLarge);
            }
            let detected = detect_lines(&lines).ok_or(ImportError::Unsupported)?;
            read_lines(&lines, detected.source)?
        }
    };
    if read.accounts.is_empty() && read.skipped == 0 {
        return Err(ImportError::Empty);
    }
    Ok(read)
}

fn imported(source: Source, accounts: Vec<Account>, skipped: usize) -> Imported {
    Imported { source, accounts, services: Vec::new(), skipped, plain: false }
}

fn read_lines(lines: &[&str], source: Source) -> Result<Imported, ImportError> {
    let (mut accounts, mut skipped) = (Vec::new(), 0);
    for line in lines {
        if is_migration(line) {
            match read_migration(line) {
                Some(mut read) => {
                    accounts.append(&mut read.0);
                    skipped += read.1;
                }
                None => skipped += 1,
            }
        } else {
            match otpauth::read_link(line) {
                Some(account) => accounts.push(account),
                None => skipped += 1,
            }
        }
        if accounts.len() > MAX_ENTRIES {
            return Err(ImportError::TooLarge);
        }
    }
    Ok(imported(source, accounts, skipped))
}

// Google Authenticator: otpauth-migration://offline?data=...

/// One QR of Google's Transfer accounts: the accounts and how many were skipped. `None` when it could not be read, and
/// the window says only that. Only `otpauth-migration://offline?` is taken, in any letter case: nothing else is real.
pub fn read_migration(link: &str) -> Option<(Vec<Account>, usize)> {
    if link.len() > MAX_MIGRATION_LINK {
        return None;
    }
    let mut kept = Zeroizing::new(String::with_capacity(link.len()));
    kept.extend(js_trim(link).chars().filter(|c| !matches!(c, '\t' | '\n' | '\r')));
    let clean = kept.trim_matches(|c: char| c <= ' ');
    let rest = clean.get("otpauth-migration://".len()..).filter(|_| is_migration(clean))?;
    let query = rest.get(.."offline?".len()).filter(|head| head.eq_ignore_ascii_case("offline?")).map(|head| &rest[head.len()..])?;
    let query = query.split('#').next().unwrap_or("");
    let value = query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        (form_decode(name).as_slice() == b"data").then(|| form_decode(value))
    })?;
    if value.is_empty() {
        return None;
    }
    // A space here was a `+` before the form decoding made it one
    let text = lossy(&value);
    let mut data = Zeroizing::new(String::with_capacity(text.len()));
    data.extend(text.chars().map(|c| if c == ' ' { '+' } else { c }));
    let payload = bmc::forgiving_b64(&data)?;
    read_migration_payload(&payload)
}

/// A protobuf reader for what section 10.2 lays out, by hand, with no generated code
struct Fields<'a> {
    bytes: &'a [u8],
}

enum Field<'a> {
    Number(u64),
    Bytes(&'a [u8]),
    Fixed,
}

impl<'a> Fields<'a> {
    /// At most ten bytes, and one past 64 bits is malformed
    fn varint(&mut self) -> Option<u64> {
        let mut value = 0u64;
        for i in 0..10 {
            let (&byte, rest) = self.bytes.split_first()?;
            self.bytes = rest;
            if i == 9 && byte > 1 {
                return None;
            }
            value |= u64::from(byte & 0x7f) << (7 * i);
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }

    /// The next field, `Ok(None)` at the end, `Err` when malformed
    fn next(&mut self) -> Result<Option<(u64, Field<'a>)>, ()> {
        if self.bytes.is_empty() {
            return Ok(None);
        }
        let key = self.varint().ok_or(())?;
        let field = match key & 7 {
            0 => Field::Number(self.varint().ok_or(())?),
            1 | 5 => {
                let size = if key & 7 == 1 { 8 } else { 4 };
                self.bytes = self.bytes.get(size..).ok_or(())?;
                Field::Fixed
            }
            2 => {
                let len = usize::try_from(self.varint().ok_or(())?).map_err(|_| ())?;
                if len > self.bytes.len() {
                    return Err(());
                }
                let (body, rest) = self.bytes.split_at(len);
                self.bytes = rest;
                Field::Bytes(body)
            }
            _ => return Err(()),
        };
        Ok(Some((key >> 3, field)))
    }
}

fn read_migration_payload(payload: &[u8]) -> Option<(Vec<Account>, usize)> {
    let mut top = Fields { bytes: payload };
    let (mut accounts, mut skipped) = (Vec::new(), 0);
    while let Some((number, field)) = top.next().ok()? {
        if number != 1 {
            continue;
        }
        let Field::Bytes(body) = field else {
            skipped += 1;
            continue;
        };
        match migration_account(body).ok()? {
            Some(account) => accounts.push(account),
            None => skipped += 1,
        }
        if accounts.len() > MAX_ENTRIES {
            return None;
        }
    }
    Some((accounts, skipped))
}

fn migration_account(body: &[u8]) -> Result<Option<Account>, ()> {
    let (mut secret, mut name, mut issuer): (&[u8], &[u8], &[u8]) = (&[], &[], &[]);
    let (mut algorithm, mut digits, mut kind, mut counter) = (0, 0, 0, 0);
    let mut fields = Fields { bytes: body };
    while let Some((number, field)) = fields.next()? {
        let bytes = if let Field::Bytes(b) = field { b } else { &[] };
        let value = if let Field::Number(n) = field { n } else { 0 };
        match number {
            1 => secret = bytes,
            2 => name = bytes,
            3 => issuer = bytes,
            4 => algorithm = value,
            5 => digits = value,
            6 => kind = value,
            7 => counter = value,
            _ => {}
        }
    }
    let algorithm = match algorithm {
        0 | 1 => "SHA1",
        2 => "SHA256",
        3 => "SHA512",
        _ => return Ok(None),
    };
    let digits = match digits {
        0 | 1 => 6,
        2 => 8,
        _ => return Ok(None),
    };
    let kind = match kind {
        0 | 2 => OtpKind::Totp,
        1 => OtpKind::Hotp,
        _ => return Ok(None),
    };
    if secret.is_empty() || secret.len() * 8 / 5 > crate::items::MAX_SECRET_CHARS {
        return Ok(None);
    }
    let name = String::from_utf8_lossy(name);
    let name = js_trim(&name);
    let issuer = String::from_utf8_lossy(issuer);
    let issuer = js_trim(&issuer);
    let (issuer, label) = if !issuer.is_empty() {
        let label = name.strip_prefix(issuer).and_then(|rest| rest.strip_prefix(':')).map_or(name, js_trim);
        (issuer, label)
    } else if let Some((head, tail)) = name.split_once(':') {
        (js_trim(head), js_trim(tail))
    } else {
        ("", name)
    };
    if issuer.len() > MAX_NAME || label.len() > MAX_NAME {
        return Ok(None);
    }
    let mut account = Account {
        kind: Some(kind),
        issuer: issuer.to_string(),
        label: label.to_string(),
        secret: otp::base32_encode(secret),
        algorithm: algorithm.to_string(),
        digits,
        ..Account::default()
    };
    if kind == OtpKind::Hotp {
        if counter > MAX_COUNTER as u64 {
            return Ok(None);
        }
        account.counter = Some(counter as i64);
    }
    Ok(Some(account))
}

// Loose values, shared by Aegis and 2FAS: what JavaScript's Number and String make of a JSON value

fn js_number_of_text(text: &str) -> f64 {
    let t = js_trim(text);
    if t.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(digits) = t.strip_prefix(prefix) {
            if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
                return f64::NAN;
            }
            return digits.chars().fold(0.0, |n, c| n * f64::from(radix) + f64::from(c.to_digit(radix).unwrap_or(0)));
        }
    }
    let unsigned = t.strip_prefix(['+', '-']).unwrap_or(t);
    if unsigned == "Infinity" {
        return if t.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY };
    }
    // Rust's parse also takes `inf` and `nan`: only a decimal gets this far
    let mut seen_digit = false;
    let mut stage = 0;
    for c in unsigned.chars() {
        match (stage, c) {
            (0 | 1 | 3, '0'..='9') => seen_digit = true,
            (0, '.') => stage = 1,
            (0 | 1, 'e' | 'E') if seen_digit => stage = 2,
            (2, '+' | '-') => stage = 3,
            (2, '0'..='9') => stage = 3,
            _ => return f64::NAN,
        }
    }
    if !seen_digit || stage == 2 {
        return f64::NAN;
    }
    t.parse().unwrap_or(f64::NAN)
}

fn js_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(list) => {
            let parts: Vec<String> = list.iter().map(|v| if v.is_null() { String::new() } else { js_string(v) }).collect();
            parts.join(",")
        }
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn js_number(value: Option<&Value>) -> f64 {
    match value {
        None => f64::NAN,
        Some(Value::Null) => 0.0,
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::Object(_)) => f64::NAN,
        Some(v) => js_number_of_text(&js_string(v)),
    }
}

fn is_missing(value: Option<&Value>) -> bool {
    matches!(value, None | Some(Value::Null)) || value.and_then(Value::as_str) == Some("")
}

fn trimmed(value: Option<&Value>) -> String {
    value.and_then(Value::as_str).map_or_else(String::new, |s| js_trim(s).to_string())
}

fn loose_digits(value: Option<&Value>) -> Option<i64> {
    if is_missing(value) {
        return Some(6);
    }
    let n = js_number(value);
    if n == 6.0 {
        Some(6)
    } else if n == 8.0 {
        Some(8)
    } else {
        None
    }
}

fn loose_period(value: Option<&Value>) -> Option<i64> {
    if is_missing(value) {
        return Some(30);
    }
    let n = js_number(value);
    (n.is_finite() && n > 0.0 && n.fract() == 0.0 && n < 9.2e18).then_some(n as i64)
}

fn loose_counter(value: Option<&Value>) -> Option<i64> {
    if is_missing(value) {
        return Some(0);
    }
    let n = js_number(value);
    (n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n <= MAX_COUNTER as f64).then_some(n as i64)
}

/// The names BackMyCodes takes, and SHA-224 and SHA-384 beside them (decision 2)
fn loose_algorithm(value: Option<&Value>) -> Option<&'static str> {
    if is_missing(value) {
        return Some("SHA1");
    }
    let name = js_string(value?).to_uppercase().replacen('-', "", 1);
    ["SHA1", "SHA224", "SHA256", "SHA384", "SHA512"].into_iter().find(|n| *n == name)
}

/// One entry of Aegis or 2FAS, in the order of section 10: `None` is skipped
struct Otp<'a> {
    kind: &'a str,
    secret: Option<&'a Value>,
    algorithm: Option<&'a Value>,
    digits: Option<&'a Value>,
    period: Option<&'a Value>,
    counter: Option<&'a Value>,
}

fn entry_account(otp: Otp<'_>) -> Option<Account> {
    let secret = otp::structured_secret(otp.secret?.as_str()?)?;
    let mut account = Account { secret, ..Account::default() };
    match otp.kind {
        "steam" => {
            account.kind = Some(OtpKind::Steam);
            account.period = loose_period(otp.period)?;
        }
        "hotp" => {
            account.algorithm = loose_algorithm(otp.algorithm)?.to_string();
            account.digits = loose_digits(otp.digits)?;
            account.kind = Some(OtpKind::Hotp);
            account.counter = Some(loose_counter(otp.counter)?);
        }
        "totp" => {
            account.algorithm = loose_algorithm(otp.algorithm)?.to_string();
            account.digits = loose_digits(otp.digits)?;
            account.kind = Some(OtpKind::Totp);
            account.period = loose_period(otp.period)?;
        }
        _ => return None,
    }
    Some(account)
}

fn names_fit(account: &Account) -> bool {
    account.issuer.len() <= MAX_NAME
        && account.label.len() <= MAX_NAME
        && account.note.as_ref().is_none_or(|n| n.as_str().len() <= MAX_NOTE)
        && account.tags.as_ref().is_none_or(|t| t.len() <= MAX_TAGS && t.iter().all(|t| t.len() <= MAX_NAME))
}

/// What a group's name is to an entry that names the group
#[derive(Clone, Copy)]
enum GroupName {
    Empty,
    /// Past [`MAX_NAME`]: never copied, and an entry that names it is refused
    TooLong,
    /// Its place in [`Groups::names`], where each distinct name is held once
    Is(usize),
}

/// A file's groups (Aegis's, 2FAS's, Bitwarden's folders) by id. A file may name one group from every entry, and one
/// entry may name it many thousand times, so a name is checked for its length once, held once, and handed out as a
/// number: no entry copies or compares one until it has passed its bounds.
struct Groups {
    by_id: HashMap<String, GroupName>,
    names: Vec<String>,
}

impl Groups {
    fn new(list: Option<&Value>, id_key: &str) -> Groups {
        let (mut by_id, mut names, mut places) = (HashMap::new(), Vec::new(), HashMap::<String, usize>::new());
        for group in list.and_then(Value::as_array).map_or(&[][..], Vec::as_slice) {
            let id = group.get(id_key).and_then(Value::as_str).map_or("", js_trim);
            if id.is_empty() {
                continue;
            }
            let name = group.get("name").and_then(Value::as_str).map_or("", js_trim);
            let name = if name.is_empty() {
                GroupName::Empty
            } else if name.len() > MAX_NAME {
                GroupName::TooLong
            } else if let Some(&place) = places.get(name) {
                GroupName::Is(place)
            } else {
                names.push(name.to_string());
                places.insert(name.to_string(), names.len() - 1);
                GroupName::Is(names.len() - 1)
            };
            by_id.insert(id.to_string(), name);
        }
        Groups { by_id, names }
    }

    fn get(&self, id: &str) -> Option<GroupName> {
        self.by_id.get(id).copied()
    }

    /// The one tag an entry of 2FAS or Bitwarden names its group by: `Err` is an entry to refuse
    fn tags_of(&self, id: &str) -> Result<Option<Vec<String>>, ()> {
        match self.get(id) {
            Some(GroupName::TooLong) => Err(()),
            Some(GroupName::Is(place)) => Ok(Some(vec![self.names[place].clone()])),
            Some(GroupName::Empty) | None => Ok(None),
        }
    }
}

fn checked_entries(list: Option<&Value>) -> Result<&[Value], ImportError> {
    let list = list.and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    if list.len() > MAX_ENTRIES {
        return Err(ImportError::TooLarge);
    }
    Ok(list)
}

// Aegis

/// Lowercase or uppercase hex pairs. A value that is not a string reads as empty; an odd length or a bad digit fails.
fn hex_field(object: &Value, key: &str) -> Option<Zeroizing<Vec<u8>>> {
    let text = object.get(key).and_then(Value::as_str).unwrap_or("");
    if text.len() & 1 != 0 {
        return None;
    }
    let mut out = Zeroizing::new(Vec::with_capacity(text.len() / 2));
    for pair in text.as_bytes().chunks(2) {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        out.push((high * 16 + low) as u8);
    }
    Some(out)
}

enum SlotOutcome {
    Opened(Zeroizing<Vec<u8>>),
    TooMuchMemory,
    Failed,
}

/// One slot, which takes its cost from what is left of the file's one budget, or is not tried
fn try_slot(slot: &Value, password: &str, budget: &mut f64) -> SlotOutcome {
    let (n, r, p) = (js_number(slot.get("n")), js_number(slot.get("r")), js_number(slot.get("p")));
    let whole = |v: f64| v.is_finite() && v.fract() == 0.0;
    if !whole(n) || !whole(r) || !whole(p) || n < 2.0 || r < 1.0 || p < 1.0 {
        return SlotOutcome::Failed;
    }
    // What scrypt allocates, and with p what it takes in time, however a file divides its asking
    let cost = 128.0 * r * n * p;
    if cost > *budget {
        return SlotOutcome::TooMuchMemory;
    }
    // BackMyCodes' own limits on each, which every file Aegis writes is within
    let (n, r, p) = (n as u64, r as u64, p as u64);
    if !n.is_power_of_two() || n > 1 << 20 || r > 16 || p > 16 {
        return SlotOutcome::Failed;
    }
    let opened = (|| {
        let salt = hex_field(slot, "salt")?;
        let key = hex_field(slot, "key")?;
        let key_params = slot.get("key_params").filter(|v| v.is_object())?;
        let nonce = hex_field(key_params, "nonce")?;
        let tag = hex_field(key_params, "tag")?;
        let params = scrypt::Params::new(n.trailing_zeros() as u8, r as u32, p as u32).ok()?;
        *budget -= cost;
        let mut derived = Zeroizing::new([0u8; 32]);
        scrypt::scrypt(password.as_bytes(), &salt, &params, &mut derived[..]).ok()?;
        let mut ct = Zeroizing::new(Vec::with_capacity(key.len() + tag.len()));
        ct.extend_from_slice(&key);
        ct.extend_from_slice(&tag);
        gcm_open(&derived[..], &nonce, &ct)
    })();
    opened.map_or(SlotOutcome::Failed, SlotOutcome::Opened)
}

fn open_aegis(file: &Value, password: &str) -> Result<WipedValue, ImportError> {
    if password.is_empty() {
        return Err(ImportError::PasswordRequired);
    }
    let header = file.get("header");
    let slots = header.and_then(|h| h.get("slots")).and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    let mut master = None;
    let mut over = false;
    let mut budget = MAX_AEGIS_SCRYPT as f64;
    let is_password_slot = |slot: &&Value| slot.is_object() && slot.get("type").and_then(Value::as_f64) == Some(1.0);
    for slot in slots.iter().filter(is_password_slot).take(MAX_SLOTS) {
        match try_slot(slot, password, &mut budget) {
            SlotOutcome::Opened(key) => {
                master = Some(key);
                break;
            }
            SlotOutcome::TooMuchMemory => over = true,
            SlotOutcome::Failed => {}
        }
    }
    let Some(master) = master else {
        return Err(if over { ImportError::TooMuchMemory } else { ImportError::BadPassword });
    };
    let params = header.and_then(|h| h.get("params")).filter(|p| p.is_object()).ok_or(ImportError::BadPassword)?;
    let nonce = hex_field(params, "nonce").ok_or(ImportError::BadPassword)?;
    let tag = hex_field(params, "tag").ok_or(ImportError::BadPassword)?;
    let db = file.get("db").and_then(Value::as_str).unwrap_or("");
    let mut ct = bmc::forgiving_b64(db).ok_or(ImportError::BadPassword)?;
    ct.extend_from_slice(&tag);
    let plain = gcm_open(&master, &nonce, &ct).ok_or(ImportError::BadPassword)?;
    parse_json(file_text(&plain).as_bytes()).map_err(too_large)
}

fn read_aegis(file: &Value, encrypted: bool, password: &str) -> Result<Imported, ImportError> {
    let opened;
    let db = if encrypted {
        opened = open_aegis(file, password)?;
        if !opened.0.is_object() {
            return Err(ImportError::Unsupported);
        }
        &opened.0
    } else {
        file.get("db").ok_or(ImportError::Unsupported)?
    };
    let groups = Groups::new(db.get("groups"), "uuid");
    let entries = checked_entries(db.get("entries"))?;
    let (mut accounts, mut skipped) = (Vec::with_capacity(entries.len()), 0);
    for entry in entries {
        match aegis_entry(entry, &groups) {
            Some(account) => accounts.push(account),
            None => skipped += 1,
        }
    }
    Ok(imported(Source::Aegis, accounts, skipped))
}

fn aegis_entry(entry: &Value, groups: &Groups) -> Option<Account> {
    entry.as_object()?;
    let info = entry.get("info").filter(|v| v.is_object());
    let field = |key: &str| info.and_then(|i| i.get(key));
    let kind = trimmed(entry.get("type")).to_lowercase();
    let mut account = entry_account(Otp {
        kind: &kind,
        secret: field("secret"),
        algorithm: field("algo"),
        digits: field("digits"),
        period: field("period"),
        counter: field("counter"),
    })?;
    account.issuer = trimmed(entry.get("issuer"));
    account.label = trimmed(entry.get("name"));
    let note = trimmed(entry.get("note"));
    account.note = (!note.is_empty()).then(|| Secret::new(note));
    account.pinned = (entry.get("favorite") == Some(&Value::Bool(true))).then_some(true);
    // Past the bound on tags the entry is refused at once: a file may name a group many thousand times
    let mut tags: Vec<String> = Vec::new();
    // A group is held once and met by its place, so a repeat costs one comparison of numbers, whatever its name
    let mut held: Vec<usize> = Vec::new();
    for id in entry.get("groups").and_then(Value::as_array).map_or(&[][..], Vec::as_slice) {
        match groups.get(id.as_str().map_or("", js_trim)) {
            Some(GroupName::TooLong) => return None,
            Some(GroupName::Is(place)) if !held.contains(&place) => {
                if tags.len() == MAX_TAGS {
                    return None;
                }
                held.push(place);
                tags.push(groups.names[place].clone());
            }
            _ => {}
        }
    }
    let group = entry.get("group").and_then(Value::as_str).map_or("", js_trim);
    if !group.is_empty() && !tags.iter().any(|t| t == group) {
        if group.len() > MAX_NAME || tags.len() == MAX_TAGS {
            return None;
        }
        tags.push(group.to_string());
    }
    account.tags = (!tags.is_empty()).then_some(tags);
    names_fit(&account).then_some(account)
}

// 2FAS

fn open_two_fas(blob: &str, password: &str) -> Result<WipedValue, ImportError> {
    if password.is_empty() {
        return Err(ImportError::PasswordRequired);
    }
    // Four parts at most, however many colons the text holds
    let mut parts = blob.splitn(4, ':');
    let (Some(data), Some(salt), Some(iv), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(ImportError::Unsupported);
    };
    let bad = || ImportError::BadPassword;
    let data = bmc::forgiving_b64(data).ok_or_else(bad)?;
    let salt = bmc::forgiving_b64(salt).ok_or_else(bad)?;
    let iv = bmc::forgiving_b64(iv).ok_or_else(bad)?;
    let key = pbkdf2_sha256(password, &salt, 10_000).map_err(|_| ImportError::Crypto)?;
    let plain = gcm_open(&key[..], &iv, &data).ok_or_else(bad)?;
    parse_json(file_text(&plain).as_bytes()).map_err(too_large)
}

fn read_two_fas(file: &Value, encrypted: bool, password: &str) -> Result<Imported, ImportError> {
    let opened;
    let services = if encrypted {
        let blob = file.get("servicesEncrypted").and_then(Value::as_str).unwrap_or("");
        opened = open_two_fas(blob, password)?;
        if !opened.0.is_array() {
            return Err(ImportError::Unsupported);
        }
        &opened.0
    } else {
        file.get("services").ok_or(ImportError::Unsupported)?
    };
    let groups = Groups::new(file.get("groups"), "id");
    let list = checked_entries(Some(services))?;
    let position = |s: &Value| s.get("order").and_then(|o| o.get("position")).and_then(Value::as_f64).unwrap_or(MAX_COUNTER as f64);
    let mut objects: Vec<&Value> = list.iter().filter(|s| s.is_object()).collect();
    let mut skipped = list.len() - objects.len();
    // Not total_cmp, which puts -0 before 0: a stable sort keeps those two in file order, as a JavaScript one does
    objects.sort_by(|a, b| position(a).partial_cmp(&position(b)).unwrap_or(std::cmp::Ordering::Equal));
    let mut accounts = Vec::with_capacity(objects.len());
    for service in objects {
        match two_fas_service(service, &groups) {
            Some(account) => accounts.push(account),
            None => skipped += 1,
        }
    }
    Ok(imported(Source::TwoFas, accounts, skipped))
}

fn two_fas_service(service: &Value, groups: &Groups) -> Option<Account> {
    let otp_object = service.get("otp").filter(|v| v.is_object());
    let field = |key: &str| otp_object.and_then(|o| o.get(key));
    let kind = trimmed(field("tokenType")).to_lowercase();
    let mut account = entry_account(Otp {
        kind: if kind.is_empty() { "totp" } else { &kind },
        secret: service.get("secret"),
        algorithm: field("algorithm"),
        digits: field("digits"),
        period: field("period"),
        counter: field("counter"),
    })?;
    let issuer = trimmed(field("issuer"));
    account.issuer = if issuer.is_empty() { trimmed(service.get("name")) } else { issuer };
    let label = trimmed(field("account"));
    account.label = if label.is_empty() {
        let label = trimmed(field("label"));
        let stripped = label.strip_prefix(account.issuer.as_str()).and_then(|rest| rest.strip_prefix(':'));
        stripped.map_or(label.as_str(), js_trim).to_string()
    } else {
        label
    };
    account.tags = groups.tags_of(js_trim(service.get("groupId").and_then(Value::as_str).unwrap_or(""))).ok()?;
    names_fit(&account).then_some(account)
}

// Bitwarden, unencrypted only

fn read_bitwarden(file: &Value, encrypted: bool) -> Result<Imported, ImportError> {
    if encrypted {
        return Err(ImportError::BitwardenEncrypted);
    }
    let folders = Groups::new(file.get("folders"), "id");
    let items = checked_entries(file.get("items"))?;
    let (mut accounts, mut skipped) = (Vec::new(), 0);
    for item in items.iter().filter(|i| i.is_object()) {
        let login = item.get("login");
        let totp = login.and_then(|l| l.get("totp")).and_then(Value::as_str).map_or("", js_trim);
        if totp.is_empty() {
            continue;
        }
        match bitwarden_item(item, login, totp, &folders) {
            Some(account) => accounts.push(account),
            None => skipped += 1,
        }
    }
    Ok(imported(Source::Bitwarden, accounts, skipped))
}

fn bitwarden_item(item: &Value, login: Option<&Value>, totp: &str, folders: &Groups) -> Option<Account> {
    let name = trimmed(item.get("name"));
    let username = trimmed(login.and_then(|l| l.get("username")));
    let mut account = if has_scheme(totp, "otpauth://") || has_scheme(totp, "steam://") {
        let mut account = otpauth::read_link(totp)?;
        if account.issuer.is_empty() {
            account.issuer = name;
        }
        if account.label.is_empty() {
            account.label = username;
        }
        account
    } else {
        let mut secret = otp::normalize_typed(totp);
        if !otp::typed_secret_is_valid(&secret) {
            return None;
        }
        let end = secret.trim_end_matches('=').len();
        secret.truncate(end);
        Account { kind: Some(OtpKind::Totp), issuer: name, label: username, secret: Secret::from(secret), ..Account::default() }
    };
    account.tags = folders.tags_of(js_trim(item.get("folderId").and_then(Value::as_str).unwrap_or(""))).ok()?;
    names_fit(&account).then_some(account)
}

// Merge and replace

/// What makes two accounts the same: the issuer exactly, and the secret by its bytes, so that a spaced, padded or
/// lower-case copy of one secret is one account (section 16, point 14). A digest, so that a long list costs no pairs of
/// comparisons and no secret lies about in a set. A secret that does not decode goes by its text.
fn account_key(account: &Account) -> [u8; 32] {
    // Not libcrypto's: a merge runs in the service, which never loads it
    let mut hash = Sha256::new();
    hash.update((account.issuer.len() as u64).to_be_bytes());
    hash.update(account.issuer.as_bytes());
    match otp::base32_decode(account.secret.as_str()) {
        Some(bytes) => {
            hash.update(b"d");
            hash.update(&*bytes);
        }
        None => {
            hash.update(b"t");
            hash.update(account.secret.as_str().as_bytes());
        }
    }
    hash.finalize().into()
}

pub fn same_account(a: &Account, b: &Account) -> bool {
    account_key(a) == account_key(b)
}

/// What a merge does, for the preview and then for saving. Nothing held changes.
#[derive(Debug, Default)]
pub struct MergePlan {
    /// To add, each with a fresh id unless its own is good and free, and `added_at` now
    pub accounts: Vec<Account>,
    pub services: Vec<BackupService>,
    /// Whether each account of the import, in its order, is one already held or an earlier repeat
    pub duplicate: Vec<bool>,
    pub duplicate_accounts: usize,
    pub duplicate_services: usize,
}

impl MergePlan {
    /// The button goes grey only when there is nothing new of either kind
    pub fn has_anything_new(&self) -> bool {
        !self.accounts.is_empty() || !self.services.is_empty()
    }
}

/// An id from a file is untrusted: it has to look like ours and be free (section 16, point 15)
fn free_id(id: &str, taken: &mut HashSet<String>) -> Result<String, getrandom::Error> {
    let id = if is_id(id) && !taken.contains(id) { id.to_string() } else { new_id()? };
    taken.insert(id.clone());
    Ok(id)
}

/// Merge: what is held stays, and what is new is added after it. Repeats inside the import count too (section 16, point 13).
pub fn plan_merge(held_accounts: &[Account], held_services: &[BackupService], import: Imported, now_ms: i64) -> Result<MergePlan, ImportError> {
    let live_keys = held_accounts.iter().filter(|a| a.deleted_at.is_none()).map(account_key).collect();
    let live_names = held_services.iter().filter(|s| s.deleted_at.is_none()).map(|s| s.service_name.clone()).collect();
    add_new(held_accounts, held_services, live_keys, live_names, import, now_ms)
}

/// Every held id stays taken, a tombstone's and one being replaced too: sync must not see an id come back as another item
fn add_new(
    held_accounts: &[Account],
    held_services: &[BackupService],
    mut seen: HashSet<[u8; 32]>,
    mut names: HashSet<String>,
    import: Imported,
    now_ms: i64,
) -> Result<MergePlan, ImportError> {
    let mut plan = MergePlan::default();
    let mut taken: HashSet<String> = held_accounts.iter().map(|a| a.id.clone()).chain(held_services.iter().map(|s| s.id.clone())).collect();
    for mut account in import.accounts {
        let new = seen.insert(account_key(&account));
        plan.duplicate.push(!new);
        if new {
            account.id = free_id(&account.id, &mut taken).map_err(|_| ImportError::Crypto)?;
            account.added_at = now_ms;
            account.updated_at = Some(now_ms);
            plan.accounts.push(account);
        } else {
            plan.duplicate_accounts += 1;
        }
    }
    for mut service in import.services {
        if names.insert(service.service_name.clone()) {
            service.id = free_id(&service.id, &mut taken).map_err(|_| ImportError::Crypto)?;
            service.added_at = now_ms;
            service.updated_at = Some(now_ms);
            plan.services.push(service);
        } else {
            plan.duplicate_services += 1;
        }
    }
    Ok(plan)
}

/// What a replace does. The window keeps what it removes for its Undo.
#[derive(Debug, Default)]
pub struct ReplacePlan {
    pub remove_accounts: Vec<String>,
    pub remove_services: Vec<String>,
    pub add_accounts: Vec<Account>,
    pub add_services: Vec<BackupService>,
}

/// Replace: the import in place of what is held. A foreign file holds no backup codes, so it keeps the held services,
/// and BackMyCodes' own replaces both. An empty import is refused here, for every path (section 16, point 16).
pub fn plan_replace(held_accounts: &[Account], held_services: &[BackupService], import: Imported, now_ms: i64) -> Result<ReplacePlan, ImportError> {
    if import.accounts.is_empty() && import.services.is_empty() {
        return Err(ImportError::Empty);
    }
    let remove_accounts = held_accounts.iter().filter(|a| a.deleted_at.is_none()).map(|a| a.id.clone()).collect();
    let remove_services = match import.source {
        Source::BackMyCodes => held_services.iter().filter(|s| s.deleted_at.is_none()).map(|s| s.id.clone()).collect(),
        _ => Vec::new(),
    };
    let plan = add_new(held_accounts, held_services, HashSet::new(), HashSet::new(), import, now_ms)?;
    Ok(ReplacePlan { remove_accounts, remove_services, add_accounts: plan.accounts, add_services: plan.services })
}

/// Adding from one scan: the same rule as a merge, and how many of each
pub fn plan_scan(held: &[Account], accounts: Vec<Account>, now_ms: i64) -> Result<MergePlan, ImportError> {
    plan_merge(held, &[], Imported { source: Source::Links, accounts, services: Vec::new(), skipped: 0, plain: false }, now_ms)
}
