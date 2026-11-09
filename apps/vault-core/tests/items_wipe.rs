//! Nothing of ours frees a buffer that still holds a secret. An allocator looks at every block as it is freed, and at
//! every block a growing buffer leaves behind, for the secret's text.
//!
//! The known limit, which these tests do not reach, is the libraries' own memory: flate2's window and dictionary (so
//! write_bmc1 and reading a BMC1 are not here, only BMCP) and libcrypto's own buffers. See plans/vault/work/items.md.

mod items_support;

use amethystora_vault_core::bmc::{Collector, read_export, write_bmcp, write_export};
use amethystora_vault_core::codes::{add_codes, parse_codes, parse_document_codes};
use amethystora_vault_core::generate::estimate_bits;
use amethystora_vault_core::import::{plan_merge, read_file, read_foreign, read_migration};
use amethystora_vault_core::items::{Account, BackupService, Item, Secret};
use amethystora_vault_core::otp::{self, Algorithm};
use amethystora_vault_core::otpauth::{ManualEntry, manual_account, read_link, write_link};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

const KEY: &str = "ORSXG5DTMVRXEZLU";

static LOOKING: AtomicBool = AtomicBool::new(false);
static NEEDLE: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());
static NEEDLE_LEN: AtomicUsize = AtomicUsize::new(0);
static FOUND: AtomicUsize = AtomicUsize::new(0);
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

struct Scanning;

// GlobalAlloc's own realloc allocates, copies and frees through these two, so a buffer that grows is looked at too.
// A new block is zeroed, so that what an earlier owner left in memory never reads as this one's.
unsafe impl GlobalAlloc for Scanning {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if LOOKING.load(Ordering::Relaxed) {
            let len = NEEDLE_LEN.load(Ordering::Relaxed);
            // Safety: the needle is 'static, and the block is ours until the call below
            let needle = unsafe { std::slice::from_raw_parts(NEEDLE.load(Ordering::Relaxed), len) };
            let block = unsafe { std::slice::from_raw_parts(ptr, layout.size()) };
            if block.len() >= len && block.windows(len).any(|window| window == needle) {
                FOUND.fetch_add(1, Ordering::Relaxed);
            }
        }
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static SCANNING: Scanning = Scanning;

/// How many freed blocks held the text while this ran, its result included
fn freed_holding<T>(needle: &'static [u8], run: impl FnOnce() -> T) -> usize {
    NEEDLE.store(needle.as_ptr() as *mut u8, Ordering::Relaxed);
    NEEDLE_LEN.store(needle.len(), Ordering::Relaxed);
    FOUND.store(0, Ordering::Relaxed);
    LOOKING.store(true, Ordering::Relaxed);
    drop(run());
    LOOKING.store(false, Ordering::Relaxed);
    FOUND.load(Ordering::Relaxed)
}

fn clean<T>(what: &str, needle: &'static [u8], run: impl FnOnce() -> T) {
    assert_eq!(freed_holding(needle, run), 0, "{what} freed a buffer holding the secret");
}

fn leak(text: String) -> &'static [u8] {
    Box::leak(text.into_boxed_str()).as_bytes()
}

fn seal(key: &[u8], nonce: &[u8], plain: &[u8]) -> (Vec<u8>, [u8; 16]) {
    let mut tag = [0u8; 16];
    let ct = openssl::symm::encrypt_aead(openssl::symm::Cipher::aes_256_gcm(), key, Some(nonce), &[], plain, &mut tag).unwrap();
    (ct, tag)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The JSON with a byte that is not UTF-8 inside its first string named `n`
fn with_bad_byte(json: &str) -> Vec<u8> {
    let mut bytes = json.replacen("\"n\"", "\"nX\"", 1).into_bytes();
    let at = bytes.windows(3).position(|w| w == b"nX\"").unwrap() + 1;
    bytes[at] = 0xff;
    bytes
}

fn account() -> Account {
    Account { issuer: "Co".to_string(), label: "me".to_string(), secret: Secret::new(KEY), ..Account::default() }
}

#[test]
fn the_scanner_sees_what_it_is_looking_for() {
    let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let text = KEY.to_string();
    assert_eq!(freed_holding(KEY.as_bytes(), move || text), 1, "a plain String");
    let secret = Secret::new(KEY);
    assert_eq!(freed_holding(KEY.as_bytes(), move || secret), 0, "a Secret");
    let mut small = KEY.to_string();
    small.shrink_to_fit();
    assert_eq!(freed_holding(KEY.as_bytes(), move || {
        small.reserve(10_000);
        std::mem::forget(small);
    }), 1, "a String that grew leaves its old buffer");
}

#[test]
fn links() {
    let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    for link in [
        format!("otpauth://totp/Co:me?secret={KEY}&issuer=Co"),
        format!("otpauth://totp/x?secret={KEY}&encoder=steam"),
        format!("otpauth://hotp/x?secret={KEY}&counter=1"),
        format!("otpauth://totp/x?secret={KEY}&period=%zz"),
        format!("otpauth://totp/x?secret={KEY}%C3"),
        format!("steam://{KEY}"),
        format!("otpauth://totp/x?secret={}&digits=6", KEY.to_lowercase()),
        format!("otpauth://totp/x?secret={}%3D&digits=6", KEY),
    ] {
        clean(&link, KEY.as_bytes(), || read_link(&link));
    }
    // A secret too long to keep is refused before anything is made of it: and the upper-cased copy of one that is kept wipes
    let long = "abcdefghijklmnop".repeat(70);
    clean("steam:// too long", b"ABCDEFGHIJKLMNOP", || read_link(&format!("steam://{long}")));
    clean("steam:// lower case", b"ABCDEFGHIJKLMNOP", || read_link("steam://abcdefghijklmnop"));
    let held = account();
    clean("write_link", KEY.as_bytes(), || write_link(&held));
    let long = Account { issuer: "é".repeat(200), label: "é".repeat(200), ..account() };
    clean("write_link with long names", KEY.as_bytes(), || write_link(&long));
}

#[test]
fn secrets() {
    let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    // Long enough that a buffer that grew would leave a freed block with the first 16 characters whole in it
    let typed = "orsx g5dt-mvrx\u{a0}ezlu mfrg gzdf mzts ewda xs4n b2gk 3tnn";
    clean("normalize_typed", KEY.as_bytes(), || otp::normalize_typed(typed));
    clean("structured_secret", KEY.as_bytes(), || otp::structured_secret(typed));
    clean("canonical_secret", KEY.as_bytes(), || otp::canonical_secret(typed));
    clean("base32_decode", b"testsecret", || otp::base32_decode(typed));
    clean("base32_encode", KEY.as_bytes(), || otp::base32_encode(b"testsecret"));
    clean("group_secret", b"ORSX G5DT", || otp::group_secret(KEY));
    let entry = ManualEntry { kind: amethystora_vault_core::items::OtpKind::Totp, issuer: "Co", label: "me", key: typed, algorithm: Algorithm::Sha1, digits: 6, period: 30, counter: 0 };
    clean("manual_account", KEY.as_bytes(), || manual_account(&entry));
    clean("estimate_bits", b"tr7#kq9!mz2@vx4$", || estimate_bits("Tr7#kQ9!mZ2@vX4$pL6%"));
}

#[test]
fn backup_codes() {
    let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    clean("parse_codes", b"qwert-11111", || parse_codes("1. qwert-11111\nasdfg-22222\nqwert-11111"));
    clean("parse_codes, two groups", b"9876 5432", || parse_codes("9876 5432\n8765 4321"));
    clean("parse_document_codes", b"qwert-11111", || parse_document_codes("Your codes\nqwert-11111\n"));
    // Decoded to wide characters by a filter, it would still be the code: so nothing keeps it as that either
    let wide: Vec<u8> = "qwert-11111".chars().filter(|c| *c != '-').flat_map(|c| (c as u32).to_ne_bytes()).collect();
    clean("parse_document_codes, as wide characters", Box::leak(wide.into_boxed_slice()), || parse_document_codes("qwert-11111\n"));
    let codes = parse_codes("qwert-11111\nasdfg-22222");
    let mut service = BackupService::default();
    clean("add_codes", b"qwert-11111", || add_codes(&mut service, codes.codes, 1));
    clean("add_codes, again", b"qwert-11111", || add_codes(&mut service, parse_codes("qwert-11111\nzxcvb-33333").codes, 1));
}

#[test]
fn encrypted_aegis_and_two_fas() {
    let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let db = json!({"entries": [{"type": "totp", "name": "n", "issuer": "i", "note": "the note", "info": {"secret": KEY}}]}).to_string();
    let (master, salt, nonce_a, nonce_b) = ([0x5au8; 32], [3u8; 32], [1u8; 12], [2u8; 12]);
    let params = scrypt::Params::new(10, 8, 1).unwrap();
    let mut derived = [0u8; 32];
    scrypt::scrypt(b"pw", &salt, &params, &mut derived).unwrap();
    let (key, key_tag) = seal(&derived, &nonce_a, &master);
    // A bad byte in a string makes the text a copy: that copy is the point
    for body in [db.clone().into_bytes(), with_bad_byte(&db)] {
        let (ct, tag) = seal(&master, &nonce_b, &body);
        let file = json!({"header": {
            "slots": [{"type": 1, "key": hex(&key), "key_params": {"nonce": hex(&nonce_a), "tag": hex(&key_tag)}, "n": 1024, "r": 8, "p": 1, "salt": hex(&salt)}],
            "params": {"nonce": hex(&nonce_b), "tag": hex(&tag)}}, "db": STANDARD.encode(ct)}).to_string();
        clean("encrypted Aegis", KEY.as_bytes(), || read_foreign(file.as_bytes(), "pw").unwrap());
        clean("encrypted Aegis, its note", b"the note", || read_foreign(file.as_bytes(), "pw").unwrap());
    }

    let services = json!([{"name": "n", "secret": KEY, "otp": {"account": "a"}}]).to_string();
    let (salt, iv) = ([4u8; 32], [9u8; 12]);
    let mut key = [0u8; 32];
    openssl::pkcs5::pbkdf2_hmac(b"pw", &salt, 10_000, openssl::hash::MessageDigest::sha256(), &mut key).unwrap();
    for body in [services.clone().into_bytes(), with_bad_byte(&services)] {
        let (mut ct, tag) = seal(&key, &iv, &body);
        ct.extend_from_slice(&tag);
        let blob = format!("{}:{}:{}", STANDARD.encode(ct), STANDARD.encode(salt), STANDARD.encode(iv));
        let file = json!({"services": [], "schemaVersion": 4, "servicesEncrypted": blob}).to_string();
        clean("encrypted 2FAS", KEY.as_bytes(), || read_foreign(file.as_bytes(), "pw").unwrap());
    }
}

#[test]
fn other_files() {
    let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    for file in [
        json!({"encrypted": false, "items": [{"name": "n", "login": {"username": "u", "totp": format!("otpauth://totp/a?secret={KEY}")}}]}).to_string(),
        json!({"encrypted": false, "items": [{"name": "n", "login": {"username": "u", "totp": KEY.to_lowercase()}}]}).to_string(),
        json!({"header": {}, "db": {"entries": [{"type": "totp", "info": {"secret": KEY}}]}}).to_string(),
        json!({"schemaVersion": 4, "services": [{"name": "n", "secret": KEY.to_lowercase()}]}).to_string(),
        format!("otpauth://totp/a?secret={KEY}\nsteam://{KEY}\n"),
        format!("\u{feff}otpauth://totp/a?secret={KEY}\n\u{fffe}"),
    ] {
        clean(&file, KEY.as_bytes(), || read_foreign(file.as_bytes(), "").unwrap());
        clean(&file, KEY.as_bytes(), || read_file(file.as_bytes(), "").unwrap());
    }
    // A file that fails part of the way through its parse: what was parsed before the failure goes too
    for broken in [
        format!(r#"{{"header":{{}},"db":{{"entries":[{{"type":"totp","info":{{"secret":"{KEY}"}}}}]}},"x":1e400}}"#),
        format!(r#"{{"header":{{}},"db":{{"entries":[{{"type":"totp","info":{{"secret":"{KEY}"}}}}]"#),
        format!(r#"{{"secret":"{KEY}","secret":"another"}}"#),
    ] {
        clean(&broken, KEY.as_bytes(), || read_foreign(broken.as_bytes(), ""));
    }
    // Google's: the raw bytes, and their Base32 after
    let mut payload = vec![0x0a, 2 + 10];
    payload.extend_from_slice(&[0x0a, 10]);
    payload.extend_from_slice(b"testsecret");
    let link = format!("otpauth-migration://offline?data={}", STANDARD.encode(&payload).replace('+', "%2B"));
    clean("read_migration", KEY.as_bytes(), || read_migration(&link));
    clean("read_migration, the raw key", b"testsecret", || read_migration(&link));
    // The link's own base64, in any buffer it passes through on the way
    let text = STANDARD.encode(&payload);
    clean("read_migration, the base64", leak(text), || read_migration(&link));
}

#[test]
fn exports_and_transfers() {
    let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let password = "a long enough password";
    let mut accounts = vec![account()];
    accounts[0].note = Some(Secret::new("the note"));
    let services = vec![BackupService { service_name: "s".to_string(), codes: vec![amethystora_vault_core::items::BackupCode::new("qwert-11111")], ..BackupService::default() }];
    clean("write_export", KEY.as_bytes(), || write_export(&accounts, &services, password, 1).unwrap());
    let file = write_export(&accounts, &services, password, 1).unwrap();
    clean("read_export", KEY.as_bytes(), || read_export(file.as_bytes(), password).unwrap());
    clean("read_export, a code", b"qwert-11111", || read_export(file.as_bytes(), password).unwrap());

    let chunks = write_bmcp(&accounts, &services).unwrap();
    let body = chunks[0].as_str().rsplit(':').next().unwrap().to_string();
    let fragment = leak(body[..24].to_string());
    clean("write_bmcp", fragment, || write_bmcp(&accounts, &services).unwrap());
    clean("write_bmcp", KEY.as_bytes(), || write_bmcp(&accounts, &services).unwrap());
    clean("reading a BMCP", KEY.as_bytes(), || {
        let mut collector = Collector::new();
        collector.add(chunks[0].as_str());
        collector.read("", 1).unwrap()
    });
    clean("reading a BMCP, its base64", fragment, || {
        let mut collector = Collector::new();
        collector.add(chunks[0].as_str());
        collector.read("", 1).unwrap()
    });

    // A plain file's unknown keys are a stranger's to name, and may hold anything
    let plain = json!({"app": "BackMyCodes", "scheme": "plaintext", "accounts": [{"secret": KEY, "later": "the hidden value"}]}).to_string();
    clean("unknown keys", b"the hidden value", || read_export(plain.as_bytes(), "").unwrap());
    assert!(!URL_SAFE_NO_PAD.encode("x").is_empty());
}

/// A string with an escape in it (a line break, a quote, a character written as `\uXXXX`, a surrogate pair) is decoded into
/// memory that wipes, in an item with unknown keys beside it and in the files an import reads
#[test]
fn escaped_strings() {
    let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let note = "the first line\nthe second, \"quoted\", caf\u{e9} \u{1F600}";
    let written = r#"the first line\nthe second, \"quoted\", caf\u00e9 \ud83d\ude00"#;
    let account = format!(r#"{{"secret":"ORSXG5DT\u004dVRXEZLU","note":"{written}","later":"x","issuer":"Co"}}"#);
    clean("Item::from_json, the secret", KEY.as_bytes(), || Item::from_json("authenticator", account.as_bytes()).unwrap());
    clean("Item::from_json, the note", note.as_bytes(), || Item::from_json("authenticator", account.as_bytes()).unwrap());
    let decoded = Item::from_json("authenticator", account.as_bytes()).unwrap();
    let Item::Account(decoded) = decoded else { panic!() };
    assert_eq!((decoded.secret.as_str(), decoded.note.as_ref().map(Secret::as_str)), (KEY, Some(note)));
    let password = format!(r#"{{"password":"{written}","history":[{{"password":"{written}","changedAt":1}}],"later":"x"}}"#);
    clean("Item::from_json, a password", note.as_bytes(), || Item::from_json("password", password.as_bytes()).unwrap());
    let codes = format!(r#"{{"serviceName":"s","codes":[{{"code":"{written}","later":"x"}}],"note":"{written}"}}"#);
    clean("Item::from_json, a backup code", note.as_bytes(), || Item::from_json("backup-codes", codes.as_bytes()).unwrap());
    // What is not JSON's is still refused
    for odd in [r#"\ud83d"#, r#"\ude00"#, r#"\ud83dx"#, r#"\x"#, "\u{1}", r#"\u12"#] {
        let refused = format!(r#"{{"secret":"{odd}"}}"#);
        assert!(Item::from_json("authenticator", refused.as_bytes()).is_none(), "{odd}");
    }

    let plain = format!(r#"{{"app": "BackMyCodes", "scheme": "plaintext", "accounts": [{{"secret": "{KEY}", "note": "{written}"}}]}}"#);
    clean("read_export, a note", note.as_bytes(), || read_export(plain.as_bytes(), "").unwrap());
    let aegis = format!(r#"{{"header": {{}}, "db": {{"entries": [{{"type": "totp", "name": "n", "note": "{written}", "info": {{"secret": "ORSXG5DT\u004dVRXEZLU"}}}}]}}}}"#);
    clean("an Aegis file, a note", note.as_bytes(), || read_foreign(aegis.as_bytes(), "").unwrap());
    clean("an Aegis file, the secret", KEY.as_bytes(), || read_foreign(aegis.as_bytes(), "").unwrap());
}

#[test]
fn items_and_merging() {
    let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let item = Item::Account(account());
    clean("Item::to_json", KEY.as_bytes(), || item.to_json().unwrap());
    let json = item.to_json().unwrap();
    clean("Item::from_json", KEY.as_bytes(), || Item::from_json("authenticator", &json).unwrap());
    let imported = read_foreign(format!("otpauth://totp/a?secret={KEY}\n").as_bytes(), "").unwrap();
    let held = [account()];
    clean("plan_merge", KEY.as_bytes(), || plan_merge(&held, &[], imported, 1).unwrap());
}
