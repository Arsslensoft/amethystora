//! BackMyCodes' transfer and export formats, read and written byte for byte (plans/vault-bmc/spec-formats.md, sections
//! 1, 2, 7, 8 and 9, and the Coordinator's decisions at its end). Written from that file and the RFCs alone.
//!
//! Everything read here is untrusted: nothing in the outer JSON of a file or the text of a chunk is authenticated. Every
//! count has a bound, and a refusal is one of a few fixed names, never anything read.

use std::collections::BTreeMap;
use std::io::{Read, Write};

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use foreign_types::ForeignTypeRef;
use openssl::hash::MessageDigest;
use openssl::pkey::{KeyType, PKey, Private, Public};
use openssl::pkey_ctx::PkeyCtx;
use openssl::symm::{Cipher, Crypter, Mode};
use serde::Serialize;
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::items::{
    Account, BackupCode, BackupService, JsonRefusal, MAX_ALGORITHM, MAX_CODE, MAX_CODES, MAX_ID, MAX_ITEM, MAX_NAME, MAX_NOTE,
    MAX_SECRET_CHARS, MAX_TAGS, OtpKind, Secret, Unknown, WipedValue, WipingWriter, is_js_space, json_size, new_id, parse_json,
    to_json_wiping,
};
use crate::otpauth::MAX_COUNTER;
use crate::otp;

/// A file read, and what any one QR transfer may name
pub const MAX_FILE: usize = 16 * 1024 * 1024;
pub const MAX_CHUNKS: usize = 256;
/// Nothing writes more into one code: it is what a version 40 QR holds at the lowest level
pub const MAX_CHUNK_TEXT: usize = 2953;
/// What a QR transfer may inflate to, stopping as it reads
pub const MAX_INFLATED: usize = 16 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 65_536;

const SLICE: usize = 2048;
const PBKDF2_ROUNDS: usize = 200_000;
const SCRYPT_LOG_N: u8 = 17;
const KEM_PUBLIC: usize = 1568;
const KEM_SECRET: usize = 3168;
const KEM_CIPHERTEXT: usize = 1568;
/// What a person must type to write a transfer or an export
pub const MIN_EXPORT_PASSWORD: usize = 8;
const SCHEME: &str = "ML-KEM-1024 + AES-256-GCM + scrypt(N=131072,r=8,p=1)";

/// An export or a transfer, read. `skipped` counts what a door refused; `plain` is what was not encrypted and takes a warning.
#[derive(Debug, Default)]
pub struct Contents {
    pub accounts: Vec<Account>,
    pub services: Vec<BackupService>,
    pub skipped: usize,
    pub plain: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmcError {
    /// Not valid JSON
    NotJson,
    /// JSON, but not BackMyCodes'
    NotAnExport,
    /// A version this does not read, 3 or later among them
    UnsupportedVersion,
    /// A wrong password or a changed file: nothing tells them apart
    BadPassword,
    /// Past a bound
    TooLarge,
    /// A transfer with no chunk at all
    NoChunks,
    /// Chunks are missing
    Incomplete,
    /// A written transfer or export needs a password of at least [`MIN_EXPORT_PASSWORD`] characters
    WeakPassword,
    /// It opened with the password, and what it holds cannot be read: never said as a wrong password
    Unreadable,
    /// The machine's cryptography failed
    Crypto,
}

impl From<getrandom::Error> for BmcError {
    fn from(_: getrandom::Error) -> BmcError {
        BmcError::Crypto
    }
}

impl From<openssl::error::ErrorStack> for BmcError {
    fn from(_: openssl::error::ErrorStack) -> BmcError {
        BmcError::Crypto
    }
}

impl From<serde_json::Error> for BmcError {
    fn from(_: serde_json::Error) -> BmcError {
        BmcError::Crypto
    }
}

// Base64: section 1's readers

fn sextet(c: u8) -> Option<u32> {
    match c {
        b'A'..=b'Z' => Some((c - b'A') as u32),
        b'a'..=b'z' => Some((c - b'a') as u32 + 26),
        b'0'..=b'9' => Some((c - b'0') as u32 + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// The standard alphabet with no padding left. Bits past the last whole byte are dropped, or refused when `strict`.
fn decode_sextets(text: &[u8], strict: bool) -> Option<Zeroizing<Vec<u8>>> {
    if text.len() % 4 == 1 {
        return None;
    }
    let mut out = Zeroizing::new(Vec::with_capacity(text.len() / 4 * 3 + 2));
    let (mut bits, mut have) = (0u32, 0u32);
    for &c in text {
        bits = (bits << 6 | sextet(c)?) & 0xffff;
        have += 6;
        if have >= 8 {
            have -= 8;
            out.push((bits >> have) as u8);
        }
    }
    if strict && bits & ((1 << have) - 1) != 0 {
        return None;
    }
    Some(out)
}

/// What exports hold: ASCII whitespace goes, padding may be left off
pub fn unb64(text: &str) -> Option<Zeroizing<Vec<u8>>> {
    let mut clean = Zeroizing::new(Vec::with_capacity(text.len()));
    clean.extend(text.bytes().filter(|b| !matches!(b, b'\t' | b'\n' | 0x0c | b'\r' | b' ')));
    if clean.len() & 3 == 0 && clean.last() == Some(&b'=') {
        clean.pop();
        if clean.last() == Some(&b'=') {
            clean.pop();
        }
    }
    decode_sextets(&clean, true)
}

/// What a chunk holds: both alphabets, up to two `=`
pub fn unb64url(text: &str) -> Option<Zeroizing<Vec<u8>>> {
    let mut clean = Zeroizing::new(Vec::with_capacity(text.len()));
    clean.extend(text.bytes().filter(|b| !matches!(b, b'\t' | b'\n' | 0x0c | b'\r' | b' ')).map(|b| match b {
        b'-' => b'+',
        b'_' => b'/',
        b => b,
    }));
    for _ in 0..2 {
        if clean.last() == Some(&b'=') {
            clean.pop();
        }
    }
    decode_sextets(&clean, false)
}

/// What 2FAS, Aegis's `db` and Google's `data` hold, read as a browser's `atob` after forgiving what apps write
pub fn forgiving_b64(text: &str) -> Option<Zeroizing<Vec<u8>>> {
    let mut clean = Zeroizing::new(Vec::with_capacity(text.len() + 3));
    for c in text.chars().filter(|c| !is_js_space(*c)) {
        match c {
            '-' => clean.push(b'+'),
            '_' => clean.push(b'/'),
            c if c.is_ascii() => clean.push(c as u8),
            _ => return None,
        }
    }
    while clean.len() & 3 != 0 {
        clean.push(b'=');
    }
    for _ in 0..2 {
        if clean.last() == Some(&b'=') {
            clean.pop();
        }
    }
    decode_sextets(&clean, false)
}

// AES-GCM. `ct` is always ciphertext then tag, there is never additional data, and only a tag that checks gives anything.

/// Open with a key of 16, 24 or 32 bytes and a nonce of any length but none
pub fn gcm_open(key: &[u8], nonce: &[u8], ct: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    let cipher = match key.len() {
        16 => Cipher::aes_128_gcm(),
        24 => Cipher::aes_192_gcm(),
        32 => Cipher::aes_256_gcm(),
        _ => return None,
    };
    if nonce.is_empty() || ct.len() < 16 {
        return None;
    }
    let (body, tag) = ct.split_at(ct.len() - 16);
    let mut crypter = Crypter::new(cipher, Mode::Decrypt, key, Some(nonce)).ok()?;
    crypter.set_tag(tag).ok()?;
    let mut out = Zeroizing::new(vec![0u8; body.len() + cipher.block_size()]);
    let count = crypter.update(body, &mut out).ok()?;
    let rest = crypter.finalize(&mut out[count..]).ok()?;
    out.truncate(count + rest);
    Some(out)
}

fn gcm_seal(key: &[u8; 32], nonce: &[u8; 12], plain: &[u8]) -> Result<Vec<u8>, BmcError> {
    let cipher = Cipher::aes_256_gcm();
    let mut crypter = Crypter::new(cipher, Mode::Encrypt, key, Some(nonce))?;
    let mut out = vec![0u8; plain.len() + cipher.block_size() + 16];
    let count = crypter.update(plain, &mut out)?;
    let rest = crypter.finalize(&mut out[count..])?;
    let mut tag = [0u8; 16];
    crypter.get_tag(&mut tag)?;
    out.truncate(count + rest);
    out.extend_from_slice(&tag);
    Ok(out)
}

pub(crate) fn pbkdf2_sha256(password: &str, salt: &[u8], rounds: usize) -> Result<Zeroizing<[u8; 32]>, BmcError> {
    let mut key = Zeroizing::new([0u8; 32]);
    openssl::pkcs5::pbkdf2_hmac(password.as_bytes(), salt, rounds, MessageDigest::sha256(), &mut key[..])?;
    Ok(key)
}

/// One scrypt of the export's own fixed cost: the file's `config.scrypt` is never read, so a file cannot ask for more
fn export_kek(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, BmcError> {
    let params = scrypt::Params::new(SCRYPT_LOG_N, 8, 1).map_err(|_| BmcError::Crypto)?;
    let mut kek = Zeroizing::new([0u8; 32]);
    scrypt::scrypt(password.as_bytes(), salt, &params, &mut kek[..]).map_err(|_| BmcError::Crypto)?;
    Ok(kek)
}

// ML-KEM-1024, FIPS 203, from libcrypto (3.5 and later). A secret key is the expanded 3168 bytes. The openssl crate has
// the keys but not these two calls yet, so they are its own -sys declarations.

fn kem_generate() -> Result<PKey<Private>, BmcError> {
    let mut seed = Zeroizing::new([0u8; 64]);
    getrandom::fill(&mut seed[..])?;
    Ok(PKey::private_key_from_seed(None, KeyType::ML_KEM_1024, None, &seed[..])?)
}

fn kem_encapsulate(public: &PKey<Public>) -> Result<(Vec<u8>, Zeroizing<[u8; 32]>), BmcError> {
    let ctx = PkeyCtx::new(public)?;
    let mut ct = vec![0u8; KEM_CIPHERTEXT];
    let mut shared = Zeroizing::new([0u8; 32]);
    let (mut ct_len, mut shared_len) = (ct.len(), shared.len());
    // Safety: the context lives past both calls, and both buffers are as long as the lengths given with them
    let done = unsafe {
        openssl_sys::EVP_PKEY_encapsulate_init(ctx.as_ptr(), std::ptr::null()) > 0
            && openssl_sys::EVP_PKEY_encapsulate(ctx.as_ptr(), ct.as_mut_ptr(), &mut ct_len, shared.as_mut_ptr(), &mut shared_len) > 0
    };
    if !done || ct_len != KEM_CIPHERTEXT || shared_len != 32 {
        let _ = openssl::error::ErrorStack::get();
        return Err(BmcError::Crypto);
    }
    Ok((ct, shared))
}

/// A wrong key gives an unrelated secret, by design: the failure shows as the AES-GCM tag not checking
fn kem_decapsulate(secret: &[u8], ct: &[u8]) -> Option<Zeroizing<[u8; 32]>> {
    if secret.len() != KEM_SECRET || ct.len() != KEM_CIPHERTEXT {
        return None;
    }
    let key = PKey::private_key_from_raw_bytes_ex(None, KeyType::ML_KEM_1024, None, secret).ok()?;
    let ctx = PkeyCtx::new(&key).ok()?;
    let mut shared = Zeroizing::new([0u8; 32]);
    let mut shared_len = shared.len();
    // Safety: as in kem_encapsulate
    let done = unsafe {
        openssl_sys::EVP_PKEY_decapsulate_init(ctx.as_ptr(), std::ptr::null()) > 0
            && openssl_sys::EVP_PKEY_decapsulate(ctx.as_ptr(), shared.as_mut_ptr(), &mut shared_len, ct.as_ptr(), ct.len()) > 0
    };
    if !done || shared_len != 32 {
        let _ = openssl::error::ErrorStack::get();
        return None;
    }
    Some(shared)
}

// Reading an entry. Loose as section 1 says: a field of the wrong type falls back to its default rather than refusing
// the entry. But a secret of no bytes, a period of 0 or below, and anything past a bound are refused (section 16).

/// A number out of the range of i64, which no default may stand in for: the entry that holds one is refused
struct OutOfRange;

/// `Ok(None)` is a field that is absent or not a number, which falls back to its default; a number too large or too
/// small to be kept is `Err`, since a default would be a wrong value (section 16, point 19: refused, never repaired)
fn loose_number(value: Option<&Value>) -> Result<Option<i64>, OutOfRange> {
    let Some(value) = value else { return Ok(None) };
    if let Some(n) = value.as_i64() {
        return Ok(Some(n));
    }
    let Some(n) = value.as_f64() else { return Ok(None) };
    let n = n.round();
    if n.abs() < 9.2e18 { Ok(Some(n as i64)) } else { Err(OutOfRange) }
}

/// [`loose_number`] of one key, where `None` is an entry to refuse
fn number(object: &Map<String, Value>, key: &str) -> Option<Option<i64>> {
    loose_number(object.get(key)).ok()
}

fn loose_text(object: &Map<String, Value>, key: &str) -> String {
    object.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn loose_optional_text(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(Value::as_str).map(str::to_string)
}

fn loose_bool(object: &Map<String, Value>, key: &str) -> Option<bool> {
    object.get(key).and_then(Value::as_bool)
}

fn loose_tags(object: &Map<String, Value>) -> Option<Vec<String>> {
    let list = object.get("tags")?.as_array()?;
    Some(list.iter().filter_map(Value::as_str).map(str::to_string).collect())
}

fn loose_kind(value: Option<&Value>) -> Option<OtpKind> {
    match value?.as_str()? {
        "totp" => Some(OtpKind::Totp),
        "hotp" => Some(OtpKind::Hotp),
        "steam" => Some(OtpKind::Steam),
        _ => None,
    }
}

fn unknown_keys(object: &Map<String, Value>, known: &[&str]) -> Unknown {
    // deletedAt is the Vault's own tombstone: a file does not get to set one
    let keep = |k: &str| !known.contains(&k) && k != "deletedAt";
    Unknown(object.iter().filter(|(k, _)| keep(k)).map(|(k, v)| (k.clone(), v.clone())).collect())
}

fn tags_fit(tags: &Option<Vec<String>>) -> bool {
    tags.as_ref().is_none_or(|tags| tags.len() <= MAX_TAGS && tags.iter().all(|t| t.len() <= MAX_NAME))
}

/// A door's checks that every kind of entry shares. Out of range is refused, never repaired: a HOTP counter below 0 or
/// past 2^53 - 1 (16.19), and digits that make no code. Then the whole entry must go in one item.
pub(crate) fn account_fits(account: &Account) -> bool {
    account.secret.as_str().len() <= MAX_SECRET_CHARS
        && !otp::holds_no_bytes(account.secret.as_str())
        && account.period > 0
        && (account.otp_kind() != OtpKind::Hotp || account.counter.is_none_or(|c| (0..=MAX_COUNTER).contains(&c)))
        && (account.otp_kind() == OtpKind::Steam || (1..=10).contains(&account.digits))
        && account.id.len() <= MAX_ID
        && account.space_id.as_ref().is_none_or(|s| s.len() <= MAX_ID)
        && account.algorithm.len() <= MAX_ALGORITHM
        && account.issuer.len() <= MAX_NAME
        && account.label.len() <= MAX_NAME
        && account.note.as_ref().is_none_or(|n| n.as_str().len() <= MAX_NOTE)
        && tags_fit(&account.tags)
        && account.unknown.fits()
        && json_size(account) <= MAX_ITEM
}

const ACCOUNT_KEYS: [&str; 17] = [
    "id", "type", "issuer", "label", "secret", "algorithm", "digits", "period", "counter", "pinned", "tags", "order", "note",
    "noBackupCodes", "spaceId", "addedAt", "updatedAt",
];

fn account_from(value: &Value) -> Option<Account> {
    let o = value.as_object()?;
    let account = Account {
        id: loose_text(o, "id"),
        kind: loose_kind(o.get("type")),
        issuer: loose_text(o, "issuer"),
        label: loose_text(o, "label"),
        secret: Secret::new(loose_text(o, "secret")),
        algorithm: o.get("algorithm").and_then(Value::as_str).unwrap_or("SHA1").to_string(),
        digits: number(o, "digits")?.unwrap_or(6),
        period: number(o, "period")?.unwrap_or(30),
        counter: number(o, "counter")?,
        pinned: loose_bool(o, "pinned"),
        tags: loose_tags(o),
        order: number(o, "order")?,
        note: loose_optional_text(o, "note").map(Secret::new),
        no_backup_codes: loose_bool(o, "noBackupCodes"),
        space_id: loose_optional_text(o, "spaceId"),
        added_at: number(o, "addedAt")?.unwrap_or(0),
        updated_at: number(o, "updatedAt")?,
        deleted_at: None,
        unknown: unknown_keys(o, &ACCOUNT_KEYS),
    };
    account_fits(&account).then_some(account)
}

fn code_from(value: &Value) -> Option<BackupCode> {
    let o = value.as_object()?;
    let code = BackupCode {
        code: Secret::new(loose_text(o, "code")),
        used: o.get("used") == Some(&Value::Bool(true)),
        used_at: number(o, "usedAt")?,
        unknown: unknown_keys(o, &["code", "used", "usedAt"]),
    };
    (code.code.as_str().len() <= MAX_CODE && code.unknown.fits()).then_some(code)
}

/// A code that is not an object is dropped, as BackMyCodes does; one past its bounds refuses the whole service, so that
/// it is counted rather than lost
fn codes_from(value: Option<&Value>) -> Option<Vec<BackupCode>> {
    let Some(list) = value.and_then(Value::as_array) else { return Some(Vec::new()) };
    if list.len() > MAX_CODES {
        return None;
    }
    list.iter().filter(|c| c.is_object()).map(code_from).collect()
}

fn service_fits(service: &BackupService) -> bool {
    service.service_name.len() <= MAX_NAME
        && service.id.len() <= MAX_ID
        && service.space_id.as_ref().is_none_or(|s| s.len() <= MAX_ID)
        && service.account.as_ref().is_none_or(|a| a.len() <= MAX_NAME)
        && service.note.as_ref().is_none_or(|n| n.as_str().len() <= MAX_NOTE)
        && tags_fit(&service.tags)
        && service.unknown.fits()
        && service.size_with_room() <= MAX_ITEM
}

const SERVICE_KEYS: [&str; 11] = [
    "id", "serviceName", "codes", "note", "spaceId", "addedAt", "updatedAt", "account", "tags", "order", "deletedAt",
];

fn service_from(value: &Value) -> Option<BackupService> {
    let o = value.as_object()?;
    let service = BackupService {
        id: loose_text(o, "id"),
        service_name: loose_text(o, "serviceName"),
        codes: codes_from(o.get("codes"))?,
        note: loose_optional_text(o, "note").map(Secret::new),
        space_id: loose_optional_text(o, "spaceId"),
        added_at: number(o, "addedAt")?.unwrap_or(0),
        updated_at: number(o, "updatedAt")?,
        account: loose_optional_text(o, "account"),
        tags: loose_tags(o),
        order: number(o, "order")?,
        deleted_at: None,
        unknown: unknown_keys(o, &SERVICE_KEYS),
    };
    service_fits(&service).then_some(service)
}

/// Entries that are not objects are dropped silently, as BackMyCodes does; the ones a door refuses are counted
fn read_list<T>(list: Option<&Value>, read: impl Fn(&Value) -> Option<T>, skipped: &mut usize) -> Result<Vec<T>, BmcError> {
    let Some(list) = list.and_then(Value::as_array) else { return Ok(Vec::new()) };
    if list.len() > MAX_ENTRIES {
        return Err(BmcError::TooLarge);
    }
    let mut out = Vec::with_capacity(list.len());
    for entry in list.iter().filter(|e| e.is_object()) {
        match read(entry) {
            Some(item) => out.push(item),
            None => *skipped += 1,
        }
    }
    Ok(out)
}

fn contents_from(payload: &Value) -> Result<Contents, BmcError> {
    let mut contents = Contents::default();
    if payload.is_array() {
        contents.accounts = read_list(Some(payload), account_from, &mut contents.skipped)?;
    } else {
        let mut skipped = 0;
        contents.accounts = read_list(payload.get("accounts"), account_from, &mut skipped)?;
        contents.services = read_list(payload.get("backupServices"), service_from, &mut skipped)?;
        contents.skipped = skipped;
    }
    Ok(contents)
}

// The file export

/// Whether a parsed file is BackMyCodes' own: the first thing import looks for
pub(crate) fn is_export(value: &Value) -> bool {
    value.get("app").and_then(Value::as_str) == Some("BackMyCodes")
}

/// Whether a file is an encrypted export, so needs a password; `None` when it is not an export at all
pub fn export_is_encrypted(text: &[u8]) -> Option<bool> {
    if text.len() > MAX_FILE {
        return None;
    }
    let file = parse_json(text).ok()?;
    is_export(&file.0).then(|| file.0.get("scheme").and_then(Value::as_str) != Some("plaintext"))
}

/// Read an export. The Recovery Kit's wrap is not looked at: the file opens with its password as any other.
pub fn read_export(text: &[u8], password: &str) -> Result<Contents, BmcError> {
    if text.len() > MAX_FILE {
        return Err(BmcError::TooLarge);
    }
    let file = parse_json(text).map_err(|why| match why {
        JsonRefusal::NotJson => BmcError::NotJson,
        JsonRefusal::TooLarge => BmcError::TooLarge,
    })?;
    if !is_export(&file.0) {
        return Err(BmcError::NotAnExport);
    }
    if file.0.get("scheme").and_then(Value::as_str) == Some("plaintext") {
        let mut contents = contents_from(&file.0)?;
        contents.plain = true;
        return Ok(contents);
    }
    let version = file.0.get("version").and_then(Value::as_f64).ok_or(BmcError::NotAnExport)?;
    if version != 1.0 && version != 2.0 {
        return Err(BmcError::UnsupportedVersion);
    }
    if password.is_empty() {
        return Err(BmcError::BadPassword);
    }
    let field = |outer: &str, key: &str| file.0.get(outer).and_then(|o| o.get(key)).and_then(Value::as_str).and_then(unb64);
    let bad = || BmcError::BadPassword;
    let salt = field("config", "salt").ok_or_else(bad)?;
    let sk_nonce = field("config", "skNonce").ok_or_else(bad)?;
    let sk_enc = field("config", "skEnc").ok_or_else(bad)?;
    let kem_ct = field("vault", "kemCT").ok_or_else(bad)?;
    let nonce = field("vault", "nonce").ok_or_else(bad)?;
    let ct = field("vault", "ct").ok_or_else(bad)?;
    let kek = export_kek(password, &salt)?;
    let secret = gcm_open(&kek[..], &sk_nonce, &sk_enc).ok_or_else(bad)?;
    let shared = kem_decapsulate(&secret, &kem_ct).ok_or_else(bad)?;
    let payload = gcm_open(&shared[..], &nonce, &ct).ok_or_else(bad)?;
    let payload = parse_json(&payload).map_err(|why| match why {
        JsonRefusal::NotJson => BmcError::Unreadable,
        JsonRefusal::TooLarge => BmcError::TooLarge,
    })?;
    contents_from(&payload.0)
}

#[derive(Serialize)]
struct ExportFile {
    version: u32,
    app: &'static str,
    #[serde(rename = "exportedAt")]
    exported_at: String,
    scheme: &'static str,
    config: ExportConfig,
    vault: ExportVault,
}

#[derive(Serialize)]
struct ExportConfig {
    salt: String,
    #[serde(rename = "skNonce")]
    sk_nonce: String,
    #[serde(rename = "skEnc")]
    sk_enc: String,
    #[serde(rename = "publicKey")]
    public_key: String,
    scrypt: ExportScrypt,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct ExportScrypt {
    N: u32,
    r: u32,
    p: u32,
}

#[derive(Serialize)]
struct ExportVault {
    #[serde(rename = "kemCT")]
    kem_ct: String,
    nonce: String,
    ct: String,
}

#[derive(Serialize)]
struct ExportPayload<'a> {
    accounts: Vec<&'a Account>,
    #[serde(rename = "backupServices")]
    backup_services: Vec<&'a BackupService>,
}

fn live_accounts(accounts: &[Account]) -> Vec<&Account> {
    accounts.iter().filter(|a| a.deleted_at.is_none()).collect()
}

fn live_services(services: &[BackupService]) -> Vec<&BackupService> {
    services.iter().filter(|s| s.deleted_at.is_none()).collect()
}

fn check_password(password: &str) -> Result<(), BmcError> {
    if password.chars().count() < MIN_EXPORT_PASSWORD {
        return Err(BmcError::WeakPassword);
    }
    Ok(())
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ`, as JavaScript's `toISOString`
pub fn iso_time(unix_ms: i64) -> String {
    let (days, ms) = (unix_ms.div_euclid(86_400_000), unix_ms.rem_euclid(86_400_000));
    // Civil from days: Howard Hinnant's published arithmetic
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let (h, m, s, millis) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}.{millis:03}Z")
}

/// `backmycodes-export-2026-10-16T09-00-00.json`, the name BackMyCodes gives its files
pub fn export_file_name(unix_ms: i64) -> String {
    let time = iso_time(unix_ms).replace([':', '.'], "-");
    format!("backmycodes-export-{}.json", &time[..19])
}

fn write_encrypted(version: u32, payload: &[u8], password: &str, now_ms: i64) -> Result<String, BmcError> {
    check_password(password)?;
    let (mut salt, mut sk_nonce, mut nonce) = ([0u8; 32], [0u8; 12], [0u8; 12]);
    getrandom::fill(&mut salt)?;
    getrandom::fill(&mut sk_nonce)?;
    getrandom::fill(&mut nonce)?;
    let kek = export_kek(password, &salt)?;
    let key = kem_generate()?;
    let secret = Zeroizing::new(key.raw_private_key()?);
    let public = key.raw_public_key()?;
    if secret.len() != KEM_SECRET || public.len() != KEM_PUBLIC {
        return Err(BmcError::Crypto);
    }
    let sk_enc = gcm_seal(&kek, &sk_nonce, &secret)?;
    let public_key = PKey::public_key_from_raw_bytes_ex(None, KeyType::ML_KEM_1024, None, &public)?;
    let (kem_ct, shared) = kem_encapsulate(&public_key)?;
    let ct = gcm_seal(&shared, &nonce, payload)?;
    let file = ExportFile {
        version,
        app: "BackMyCodes",
        exported_at: iso_time(now_ms),
        scheme: SCHEME,
        config: ExportConfig {
            salt: STANDARD.encode(salt),
            sk_nonce: STANDARD.encode(sk_nonce),
            sk_enc: STANDARD.encode(sk_enc),
            public_key: STANDARD.encode(public),
            scrypt: ExportScrypt { N: 1 << SCRYPT_LOG_N, r: 8, p: 1 },
        },
        vault: ExportVault { kem_ct: STANDARD.encode(kem_ct), nonce: STANDARD.encode(nonce), ct: STANDARD.encode(ct) },
    };
    // Two spaces, one key a line, no newline at the end: serde_json's pretty form is exactly section 1's
    Ok(serde_json::to_string_pretty(&file)?)
}

/// An encrypted export of version 2, the one the Vault writes. Nothing removed is in it, and no Recovery Kit's wrap ever.
pub fn write_export(accounts: &[Account], services: &[BackupService], password: &str, now_ms: i64) -> Result<String, BmcError> {
    let payload = to_json_wiping(&ExportPayload { accounts: live_accounts(accounts), backup_services: live_services(services) })?;
    write_encrypted(2, &payload, password, now_ms)
}

/// An encrypted export of version 1: its payload is the bare list of accounts, with no backup codes
pub fn write_export_v1(accounts: &[Account], password: &str, now_ms: i64) -> Result<String, BmcError> {
    let payload = to_json_wiping(&live_accounts(accounts))?;
    write_encrypted(1, &payload, password, now_ms)
}

// The QR transfer

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Magic {
    /// Encrypted
    Bmc1,
    /// Plain, which is read with a warning and which the window never offers to write
    Bmcp,
}

impl Magic {
    fn text(self) -> &'static str {
        match self {
            Magic::Bmc1 => "BMC1",
            Magic::Bmcp => "BMCP",
        }
    }
}

#[derive(Debug)]
pub struct Chunk {
    pub index: usize,
    pub total: usize,
    pub bytes: Zeroizing<Vec<u8>>,
}

/// JavaScript's `parseInt(text, 10)`: leading space, a sign, digits, and anything after them ignored. `None` for no
/// digits, and it saturates, since anything so large is past every bound anyway.
fn parse_int(text: &str) -> Option<i64> {
    let text = text.trim_start_matches(is_js_space);
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let count = digits.bytes().take_while(u8::is_ascii_digit).count();
    if count == 0 {
        return None;
    }
    let value = digits.bytes().take(count).fold(0i64, |n, d| n.saturating_mul(10).saturating_add((d - b'0') as i64));
    Some(if negative { -value } else { value })
}

/// One chunk's text, for a given magic: `MAGIC:INDEX/TOTAL:PAYLOAD`
pub fn parse_chunk(text: &str, magic: Magic) -> Option<Chunk> {
    if text.len() > MAX_CHUNK_TEXT {
        return None;
    }
    let rest = text.strip_prefix(magic.text())?.strip_prefix(':')?;
    let slash = rest.find('/')?;
    let colon = rest.find(':')?;
    if colon < slash {
        return None;
    }
    let index = parse_int(&rest[..slash])?;
    let total = parse_int(&rest[slash + 1..colon])?;
    if total < 1 || total > MAX_CHUNKS as i64 || index < 1 || index > total {
        return None;
    }
    Some(Chunk { index: index as usize, total: total as usize, bytes: unb64url(&rest[colon + 1..])? })
}

/// What a scanned code did to the collection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Added {
    New,
    /// This one was already in
    Again,
    NotAChunk,
    /// From another transfer than the first code's: say so, and offer to start again (section 16, point 25)
    OtherTransfer,
}

/// The chunks collected so far, in any order. There is no transfer id in the format, so another transfer's chunk shows
/// only by its magic, its total, or different bytes at an index already held.
#[derive(Default)]
pub struct Collector {
    magic: Option<Magic>,
    total: usize,
    chunks: BTreeMap<usize, Zeroizing<Vec<u8>>>,
}

impl Collector {
    pub fn new() -> Collector {
        Collector::default()
    }

    pub fn add(&mut self, text: &str) -> Added {
        let found = parse_chunk(text, Magic::Bmc1).map(|c| (Magic::Bmc1, c)).or_else(|| parse_chunk(text, Magic::Bmcp).map(|c| (Magic::Bmcp, c)));
        let Some((magic, chunk)) = found else { return Added::NotAChunk };
        if self.magic.is_some_and(|m| m != magic || self.total != chunk.total) {
            return Added::OtherTransfer;
        }
        self.magic = Some(magic);
        self.total = chunk.total;
        match self.chunks.get(&chunk.index) {
            Some(held) if **held == *chunk.bytes => Added::Again,
            Some(_) => Added::OtherTransfer,
            None => {
                self.chunks.insert(chunk.index, chunk.bytes);
                Added::New
            }
        }
    }

    pub fn total(&self) -> usize {
        self.total
    }

    pub fn have(&self) -> usize {
        self.chunks.len()
    }

    pub fn has(&self, index: usize) -> bool {
        self.chunks.contains_key(&index)
    }

    /// The numbers still to come, for `Missing QR code(s)`
    pub fn missing(&self) -> Vec<usize> {
        (1..=self.total).filter(|i| !self.chunks.contains_key(i)).collect()
    }

    pub fn is_complete(&self) -> bool {
        self.total > 0 && self.chunks.len() == self.total
    }

    /// Whether it needs a password, so that a plain transfer can open at once and with its warning
    pub fn is_plain(&self) -> bool {
        self.magic == Some(Magic::Bmcp)
    }

    fn blob(&self) -> Zeroizing<Vec<u8>> {
        let mut blob = Zeroizing::new(Vec::with_capacity(self.chunks.values().map(|c| c.len()).sum()));
        for chunk in self.chunks.values() {
            blob.extend_from_slice(chunk);
        }
        blob
    }

    /// Read the whole transfer. A wrong password says so for both its causes: every chunk from one transfer is not
    /// provable here. Every entry gets a new id, and `added_at` is now.
    pub fn read(&self, password: &str, now_ms: i64) -> Result<Contents, BmcError> {
        let magic = self.magic.ok_or(BmcError::NoChunks)?;
        if !self.is_complete() {
            return Err(BmcError::Incomplete);
        }
        let blob = self.blob();
        let json = match magic {
            Magic::Bmcp => blob,
            Magic::Bmc1 => open_bmc1(&blob, password)?,
        };
        let payload = parse_json(&json).map_err(|why| match (why, magic) {
            (JsonRefusal::TooLarge, _) => BmcError::TooLarge,
            (_, Magic::Bmc1) => BmcError::Unreadable,
            (_, Magic::Bmcp) => BmcError::NotJson,
        })?;
        let mut contents = transfer_contents(&payload, now_ms)?;
        contents.plain = magic == Magic::Bmcp;
        Ok(contents)
    }
}

fn open_bmc1(blob: &[u8], password: &str) -> Result<Zeroizing<Vec<u8>>, BmcError> {
    if blob.len() < 16 + 12 + 16 {
        return Err(BmcError::BadPassword);
    }
    let (salt, rest) = blob.split_at(16);
    let (iv, ct) = rest.split_at(12);
    let key = pbkdf2_sha256(password, salt, PBKDF2_ROUNDS)?;
    let compressed = gcm_open(&key[..], iv, ct).ok_or(BmcError::BadPassword)?;
    // Inflated through a buffer of ours that wipes itself, not std::io::copy's, stopping at the bound as it reads.
    // Past the tag, a stream that does not inflate is a file that opened and cannot be read.
    let mut inflated = WipingWriter::default();
    let mut reader = DeflateDecoder::new(&compressed[..]);
    let mut buffer = Zeroizing::new([0u8; 8192]);
    loop {
        let count = reader.read(&mut buffer[..]).map_err(|_| BmcError::Unreadable)?;
        if count == 0 {
            return Ok(inflated.0);
        }
        if inflated.0.len() + count > MAX_INFLATED {
            return Err(BmcError::TooLarge);
        }
        inflated.write_all(&buffer[..count]).map_err(|_| BmcError::Crypto)?;
    }
}

fn compact_account(value: &Value, now_ms: i64) -> Result<Option<Account>, BmcError> {
    let Some(o) = value.as_object() else { return Ok(None) };
    let (Some(digits), Some(period), Some(counter)) = (number(o, "d"), number(o, "p"), number(o, "c")) else { return Ok(None) };
    let account = Account {
        id: new_id()?,
        kind: loose_kind(o.get("t")),
        issuer: loose_text(o, "i"),
        label: loose_text(o, "l"),
        secret: Secret::new(loose_text(o, "s")),
        algorithm: o.get("a").and_then(Value::as_str).unwrap_or("SHA1").to_string(),
        digits: digits.unwrap_or(6),
        period: period.unwrap_or(30),
        counter,
        added_at: now_ms,
        ..Account::default()
    };
    Ok(account_fits(&account).then_some(account))
}

fn compact_service(value: &Value, now_ms: i64) -> Result<Option<BackupService>, BmcError> {
    let Some(o) = value.as_object() else { return Ok(None) };
    let name = loose_text(o, "n");
    let Some(codes) = codes_from(o.get("c")) else { return Ok(None) };
    let service = BackupService { id: new_id()?, service_name: name, codes, added_at: now_ms, ..BackupService::default() };
    Ok(service_fits(&service).then_some(service))
}

fn transfer_contents(payload: &WipedValue, now_ms: i64) -> Result<Contents, BmcError> {
    let (accounts, services) = match &payload.0 {
        Value::Array(_) => (Some(&payload.0), None),
        _ => (payload.0.get("a"), payload.0.get("b")),
    };
    let accounts = accounts.and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    let services = services.and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    if accounts.len() > MAX_ENTRIES || services.len() > MAX_ENTRIES {
        return Err(BmcError::TooLarge);
    }
    let mut contents = Contents::default();
    for entry in accounts {
        match compact_account(entry, now_ms)? {
            Some(account) => contents.accounts.push(account),
            None => contents.skipped += 1,
        }
    }
    for entry in services {
        match compact_service(entry, now_ms)? {
            Some(service) => contents.services.push(service),
            None => contents.skipped += 1,
        }
    }
    Ok(contents)
}

#[derive(Serialize)]
struct CompactAccount<'a> {
    i: &'a str,
    l: &'a str,
    s: &'a Secret,
    a: &'a str,
    d: i64,
    p: i64,
    /// Decision 1: the kind and the counter, which BackMyCodes ignores, after its six
    #[serde(skip_serializing_if = "Option::is_none")]
    t: Option<OtpKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    c: Option<i64>,
}

#[derive(Serialize)]
struct CompactService<'a> {
    n: &'a str,
    c: &'a [BackupCode],
}

#[derive(Serialize)]
struct CompactPayload<'a> {
    a: Vec<CompactAccount<'a>>,
    b: Vec<CompactService<'a>>,
}

fn compact_payload(accounts: &[Account], services: &[BackupService]) -> Result<Zeroizing<Vec<u8>>, BmcError> {
    let a = live_accounts(accounts)
        .into_iter()
        .map(|account| {
            let kind = account.otp_kind();
            CompactAccount {
                i: &account.issuer,
                l: &account.label,
                s: &account.secret,
                a: &account.algorithm,
                d: account.digits,
                p: account.period,
                t: (kind != OtpKind::Totp).then_some(kind),
                c: (kind == OtpKind::Hotp).then_some(account.counter.unwrap_or(0)),
            }
        })
        .collect();
    let b = live_services(services).into_iter().map(|s| CompactService { n: &s.service_name, c: &s.codes }).collect();
    Ok(to_json_wiping(&CompactPayload { a, b })?)
}

/// The accounts BackMyCodes would show wrong codes for if it read this transfer, since it ignores `t` and `c`. The
/// window names them before writing (decision 1).
pub fn wrong_in_backmycodes(accounts: &[Account]) -> Vec<&Account> {
    accounts.iter().filter(|a| a.deleted_at.is_none() && a.otp_kind() != OtpKind::Totp).collect()
}

fn into_chunks(magic: Magic, blob: &[u8]) -> Result<Vec<Secret>, BmcError> {
    let total = blob.len().div_ceil(SLICE).max(1);
    if total > MAX_CHUNKS {
        return Err(BmcError::TooLarge);
    }
    let mut chunks = Vec::with_capacity(total);
    for index in 1..=total {
        let start = (index - 1) * SLICE;
        let slice = &blob[start..blob.len().min(start + SLICE)];
        // The base64 goes straight into the chunk's own buffer, sized first: it is the secret, encoded
        let head = format!("{}:{index:02}/{total:02}:", magic.text());
        let mut chunk = Zeroizing::new(String::with_capacity(head.len() + slice.len().div_ceil(3) * 4));
        chunk.push_str(&head);
        URL_SAFE_NO_PAD.encode_string(slice, &mut chunk);
        chunks.push(Secret::from(chunk));
    }
    Ok(chunks)
}

/// A BMC1 transfer: raw deflate, then AES-256-GCM under PBKDF2-SHA256, in QR-sized chunks
pub fn write_bmc1(accounts: &[Account], services: &[BackupService], password: &str) -> Result<Vec<Secret>, BmcError> {
    check_password(password)?;
    let (mut salt, mut iv) = ([0u8; 16], [0u8; 12]);
    getrandom::fill(&mut salt)?;
    getrandom::fill(&mut iv)?;
    let payload = compact_payload(accounts, services)?;
    let mut encoder = DeflateEncoder::new(WipingWriter::default(), Compression::new(6));
    encoder.write_all(&payload).map_err(|_| BmcError::Crypto)?;
    let compressed = encoder.finish().map_err(|_| BmcError::Crypto)?;
    let key = pbkdf2_sha256(password, &salt, PBKDF2_ROUNDS)?;
    let ct = gcm_seal(&key, &iv, &compressed.0)?;
    let mut blob = Vec::with_capacity(salt.len() + iv.len() + ct.len());
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&iv);
    blob.extend_from_slice(&ct);
    into_chunks(Magic::Bmc1, &blob)
}

/// A BMCP transfer: the JSON itself, unencrypted. It exists for BackMyCodes' sake and for tests: the plan has the window
/// read this with a warning and never offer to write it.
pub fn write_bmcp(accounts: &[Account], services: &[BackupService]) -> Result<Vec<Secret>, BmcError> {
    into_chunks(Magic::Bmcp, &compact_payload(accounts, services)?)
}
