//! The importers against files of this file's own, encrypted ones sealed here with the primitives their formats name:
//! each line holds a rule of section 10 or 11.

use amethystora_vault_core::bmc::write_export;
use amethystora_vault_core::import::{
    Detected, ImportError, Imported, MAX_AEGIS_SCRYPT, Source, detect, plan_merge, plan_replace, plan_scan, read_file, read_foreign, read_migration, same_account,
};
use amethystora_vault_core::items::{Account, BackupService, OtpKind, Secret, is_id};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

const KEY: &str = "ORSXG5DTMVRXEZLU";
const NOW: i64 = 1_800_000_000_000;

fn seal(key: &[u8], nonce: &[u8], plain: &[u8]) -> (Vec<u8>, [u8; 16]) {
    let cipher = match key.len() {
        16 => openssl::symm::Cipher::aes_128_gcm(),
        _ => openssl::symm::Cipher::aes_256_gcm(),
    };
    let mut tag = [0u8; 16];
    let ct = openssl::symm::encrypt_aead(cipher, key, Some(nonce), &[], plain, &mut tag).unwrap();
    (ct, tag)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// An Aegis file, sealed as Aegis seals one, with one slot
fn aegis_file(db: &Value, password: &str, log_n: u8, r: u32, p: u32, master_len: usize) -> Value {
    let master = vec![0x5a; master_len];
    let salt = [3u8; 32];
    let mut derived = [0u8; 32];
    let params = scrypt::Params::new(log_n, r, p).unwrap();
    scrypt::scrypt(password.as_bytes(), &salt, &params, &mut derived).unwrap();
    let key_nonce = [1u8; 12];
    let (key, key_tag) = seal(&derived, &key_nonce, &master);
    let db_nonce = [2u8; 12];
    let (ct, db_tag) = seal(&master, &db_nonce, db.to_string().as_bytes());
    json!({
        "version": 1,
        "header": {
            "slots": [{"type": 1, "uuid": "x", "key": hex(&key), "key_params": {"nonce": hex(&key_nonce), "tag": hex(&key_tag)}, "n": 1u64 << log_n, "r": r, "p": p, "salt": hex(&salt)}],
            "params": {"nonce": hex(&db_nonce), "tag": hex(&db_tag)}
        },
        "db": STANDARD.encode(ct)
    })
}

fn two_fas_blob(services: &Value, password: &str, iv: &[u8]) -> String {
    let salt = [4u8; 256];
    let mut key = [0u8; 32];
    openssl::pkcs5::pbkdf2_hmac(password.as_bytes(), &salt, 10_000, openssl::hash::MessageDigest::sha256(), &mut key).unwrap();
    let (mut ct, tag) = seal(&key, iv, services.to_string().as_bytes());
    ct.extend_from_slice(&tag);
    format!("{}:{}:{}", STANDARD.encode(ct), STANDARD.encode(salt), STANDARD.encode(iv))
}

fn sample_db() -> Value {
    json!({
        "version": 3,
        "groups": [{"uuid": "g1", "name": " Work "}, {"uuid": "g2", "name": "Dev"}, {"uuid": "g1", "name": "Office"}, {"uuid": "", "name": "no id"}],
        "entries": [
            {"type": "totp", "name": " me ", "issuer": " Co ", "note": " the note ", "favorite": true, "groups": ["g1", "g2", "gone"], "group": "Dev",
             "info": {"secret": "orsx g5dt-mvrx ezlu", "algo": "sha-256", "digits": "8", "period": 60}},
            {"type": "HOTP", "name": "h", "issuer": "Bank", "info": {"secret": KEY, "algo": "SHA512", "digits": 6, "counter": 7}},
            {"type": "steam", "name": "player", "issuer": "Steam", "info": {"secret": KEY, "digits": 5, "algo": "MD5"}},
            {"type": "totp", "name": "new hash", "issuer": "H", "info": {"secret": KEY, "algo": "SHA224"}},
            {"type": "yandex", "info": {"secret": KEY}},
            {"type": "motp", "info": {"secret": KEY}},
            {"type": "totp", "info": {"secret": KEY, "algo": "MD5"}},
            {"type": "totp", "info": {"secret": KEY, "digits": 7}},
            {"type": "totp", "info": {"secret": "not base32!"}},
            {"type": "totp", "info": {"secret": "M"}},
            {"type": "totp", "info": {"secret": KEY, "period": 0}},
            {"type": "hotp", "info": {"secret": KEY, "counter": -1}},
            "not an object",
        ]
    })
}

fn check_sample(read: &Imported) {
    assert_eq!(read.source, Source::Aegis);
    assert_eq!((read.accounts.len(), read.skipped), (4, 9));
    let a = &read.accounts[0];
    assert_eq!(a.kind, Some(OtpKind::Totp));
    assert_eq!((a.issuer.as_str(), a.label.as_str(), a.secret.as_str(), a.algorithm.as_str(), a.digits, a.period), ("Co", "me", KEY, "SHA256", 8, 60));
    assert_eq!(a.note.as_ref().unwrap().as_str(), "the note");
    assert_eq!(a.pinned, Some(true));
    // Named by id, a later duplicate id winning, then the old single name, without repeats
    assert_eq!(a.tags.as_deref().unwrap(), ["Office".to_string(), "Dev".to_string()]);
    assert_eq!((a.id.as_str(), a.added_at), ("", 0));
    let h = &read.accounts[1];
    assert_eq!((h.kind, h.counter, h.period, h.digits, h.algorithm.as_str()), (Some(OtpKind::Hotp), Some(7), 30, 6, "SHA512"));
    assert_eq!((h.pinned, h.note.is_none(), h.tags.is_none()), (None, true, true));
    let s = &read.accounts[2];
    assert_eq!((s.kind, s.algorithm.as_str(), s.digits, s.period, s.counter), (Some(OtpKind::Steam), "SHA1", 6, 30, None));
    assert_eq!(read.accounts[3].algorithm, "SHA224");
}

#[test]
fn plain_aegis() {
    let file = json!({"version": 1, "header": {"slots": null, "params": null}, "db": sample_db()}).to_string();
    assert_eq!(detect(file.as_bytes()), Some(Detected { source: Source::Aegis, encrypted: false }));
    check_sample(&read_foreign(file.as_bytes(), "").unwrap());
}

#[test]
fn encrypted_aegis() {
    let file = aegis_file(&sample_db(), "pw", 10, 8, 1, 32).to_string();
    assert_eq!(detect(file.as_bytes()), Some(Detected { source: Source::Aegis, encrypted: true }));
    check_sample(&read_foreign(file.as_bytes(), "pw").unwrap());
    assert_eq!(read_foreign(file.as_bytes(), "wrong").unwrap_err(), ImportError::BadPassword);
    assert_eq!(read_foreign(file.as_bytes(), "").unwrap_err(), ImportError::PasswordRequired);
    // AES-128 as its master key, which Aegis's format allows
    let small = aegis_file(&sample_db(), "pw", 10, 8, 1, 16).to_string();
    check_sample(&read_foreign(small.as_bytes(), "pw").unwrap());
    // URL-safe base64 with line breaks, for the db
    let mut wrapped = aegis_file(&sample_db(), "pw", 10, 8, 1, 32);
    let db = wrapped["db"].as_str().unwrap().replace('+', "-").replace('/', "_");
    wrapped["db"] = json!(format!("{}\n{}", &db[..10], db[10..].trim_end_matches('=')));
    check_sample(&read_foreign(wrapped.to_string().as_bytes(), "pw").unwrap());
    // The second slot opens, after one of a biometric type and one that fails
    let mut slots = aegis_file(&sample_db(), "pw", 10, 8, 1, 32);
    let good = slots["header"]["slots"][0].clone();
    slots["header"]["slots"] = json!([{"type": 2}, {"type": "1"}, 7, {"type": 1, "n": 1024, "r": 8, "p": 1, "salt": "zz"}, good]);
    check_sample(&read_foreign(slots.to_string().as_bytes(), "pw").unwrap());
    // A vault that is not an object
    let list = aegis_file(&json!([1]), "pw", 10, 8, 1, 32).to_string();
    assert_eq!(read_foreign(list.as_bytes(), "pw").unwrap_err(), ImportError::Unsupported);
}

#[test]
fn aegis_scrypt_is_capped() {
    let base = aegis_file(&sample_db(), "pw", 10, 8, 1, 32);
    // Aegis's own default fits: n=2^15, r=8, p=1 is 32 MiB. The cap is 256 MiB, 2^18 at r=8.
    assert_eq!(MAX_AEGIS_SCRYPT, 256 * 1024 * 1024);
    for (n, r, p, why) in [
        (1u64 << 19, 8, 1, "memory"),
        (1 << 20, 16, 1, "memory"),
        (1 << 30, 16, 16, "memory"),
        (1 << 18, 8, 2, "time"),
        (1 << 40, 1, 1, "memory"),
    ] {
        let mut file = base.clone();
        file["header"]["slots"][0]["n"] = json!(n);
        file["header"]["slots"][0]["r"] = json!(r);
        file["header"]["slots"][0]["p"] = json!(p);
        assert_eq!(read_foreign(file.to_string().as_bytes(), "pw").unwrap_err(), ImportError::TooMuchMemory, "{why} {n} {r} {p}");
    }
    // A slot over the cap does not stop one that opens after it
    let mut both = base.clone();
    let good = both["header"]["slots"][0].clone();
    let mut over = good.clone();
    over["n"] = json!(1u64 << 19);
    both["header"]["slots"] = json!([over, good]);
    check_sample(&read_foreign(both.to_string().as_bytes(), "pw").unwrap());
    // Costs that make no sense read as a slot that fails
    for (n, r, p) in [(1000u64, 8, 1), (1024, 0, 1), (1024, 8, 0), (0, 8, 1), (1, 8, 1)] {
        let mut file = base.clone();
        file["header"]["slots"][0]["n"] = json!(n);
        file["header"]["slots"][0]["r"] = json!(r);
        file["header"]["slots"][0]["p"] = json!(p);
        assert_eq!(read_foreign(file.to_string().as_bytes(), "pw").unwrap_err(), ImportError::BadPassword, "{n} {r} {p}");
    }
    // Only so many slots are tried, whatever a file holds
    let mut many = base.clone();
    let good = many["header"]["slots"][0].clone();
    let mut failing = good.clone();
    failing["salt"] = json!("zz");
    let mut list = vec![failing; 16];
    list.push(good);
    many["header"]["slots"] = json!(list);
    assert_eq!(read_foreign(many.to_string().as_bytes(), "pw").unwrap_err(), ImportError::BadPassword);
}

fn two_fas_services() -> Value {
    json!([
        {"name": "First", "secret": KEY, "otp": {"account": "a", "tokenType": "totp", "digits": 6, "period": 30, "algorithm": "SHA1"}, "order": {"position": 1}, "groupId": "g"},
        {"name": "Second", "secret": "orsx g5dt mvrx ezlu", "otp": {"tokenType": "HOTP", "digits": 8, "algorithm": "SHA256"}, "order": {"position": 0}},
        {"name": "Third", "secret": KEY, "otp": {"tokenType": "STEAM", "account": "player", "digits": 5}, "order": {"position": 2}},
        {"name": "Fourth", "secret": KEY, "otp": {"issuer": "Own", "label": "Own:dave"}, "order": {"position": 3}, "groupId": "unknown"},
        {"name": "Fifth", "secret": KEY, "otp": {"algorithm": "MD5"}, "order": {"position": 4}},
        {"name": "Sixth", "secret": KEY, "otp": {"label": "x"}},
        3
    ])
}

fn check_two_fas(read: &Imported) {
    assert_eq!(read.source, Source::TwoFas);
    assert_eq!((read.accounts.len(), read.skipped), (5, 2));
    let names: Vec<&str> = read.accounts.iter().map(|a| a.issuer.as_str()).collect();
    assert_eq!(names, ["Second", "First", "Third", "Own", "Sixth"]);
    assert_eq!((read.accounts[0].kind, read.accounts[0].counter, read.accounts[0].secret.as_str(), read.accounts[0].digits), (Some(OtpKind::Hotp), Some(0), KEY, 8));
    assert_eq!((read.accounts[1].label.as_str(), read.accounts[1].tags.as_deref()), ("a", Some(&["Work".to_string()][..])));
    assert_eq!(read.accounts[2].kind, Some(OtpKind::Steam));
    assert_eq!((read.accounts[3].label.as_str(), read.accounts[3].tags.is_none()), ("dave", true));
    assert_eq!(read.accounts[4].label, "x");
}

#[test]
fn two_fas() {
    let plain = json!({"services": two_fas_services(), "groups": [{"id": "g", "name": "Work"}], "schemaVersion": 4}).to_string();
    assert_eq!(detect(plain.as_bytes()), Some(Detected { source: Source::TwoFas, encrypted: false }));
    check_two_fas(&read_foreign(plain.as_bytes(), "").unwrap());

    let sealed = json!({"services": [], "groups": [{"id": "g", "name": "Work"}], "schemaVersion": 4, "servicesEncrypted": two_fas_blob(&two_fas_services(), "pässwörd", &[9; 12])}).to_string();
    assert_eq!(detect(sealed.as_bytes()), Some(Detected { source: Source::TwoFas, encrypted: true }));
    check_two_fas(&read_foreign(sealed.as_bytes(), "pässwörd").unwrap());
    assert_eq!(read_foreign(sealed.as_bytes(), "wrong").unwrap_err(), ImportError::BadPassword);
    assert_eq!(read_foreign(sealed.as_bytes(), "").unwrap_err(), ImportError::PasswordRequired);
    // A 16-byte iv opens too
    let sealed = json!({"services": [], "groups": [{"id": "g", "name": "Work"}], "schemaVersion": 4, "servicesEncrypted": two_fas_blob(&two_fas_services(), "pw", &[9; 16])}).to_string();
    check_two_fas(&read_foreign(sealed.as_bytes(), "pw").unwrap());
    // Any other number of parts, and a content of the wrong shape
    let parts = json!({"services": [], "schemaVersion": 4, "servicesEncrypted": "a:b"}).to_string();
    assert_eq!(read_foreign(parts.as_bytes(), "pw").unwrap_err(), ImportError::Unsupported);
    let object = json!({"services": [], "schemaVersion": 4, "servicesEncrypted": two_fas_blob(&json!({"a": 1}), "pw", &[9; 12])}).to_string();
    assert_eq!(read_foreign(object.as_bytes(), "pw").unwrap_err(), ImportError::Unsupported);
    // Not 2FAS without its marker
    assert_eq!(detect(json!({"services": []}).to_string().as_bytes()), None);
}

#[test]
fn bitwarden() {
    let file = json!({
        "encrypted": false,
        "folders": [{"id": "f", "name": "Personal"}],
        "items": [
            {"name": "By name", "folderId": "f", "login": {"username": "u", "totp": format!("otpauth://totp/?secret={KEY}")}},
            {"name": "A link", "login": {"username": "u", "totp": format!("otpauth://totp/Co:me?secret={KEY}&issuer=Co")}},
            {"name": "Named", "folderId": "f", "login": {"username": "u", "totp": format!("otpauth://totp/Co:?secret={KEY}")}},
            {"name": "A steam", "login": {"username": "user", "totp": format!("STEAM://{KEY}")}},
            {"name": "A bare", "login": {"username": "user", "totp": "orsx-g5dt-mvrx-ezlu===="}},
            {"name": "Too short", "login": {"totp": "ORSXG5DT"}},
            {"name": "Not base32", "login": {"totp": "ORSXG5DTMVRXEZL1"}},
            {"name": "Nothing here", "login": {"username": "u"}},
            {"name": "A note", "type": 2},
            "not an object"
        ]
    })
    .to_string();
    assert_eq!(detect(file.as_bytes()), Some(Detected { source: Source::Bitwarden, encrypted: false }));
    let read = read_foreign(file.as_bytes(), "").unwrap();
    assert_eq!((read.accounts.len(), read.skipped), (4, 3));
    // A link with an empty path is refused, so it is one of the skipped
    assert_eq!((read.accounts[0].issuer.as_str(), read.accounts[0].label.as_str()), ("Co", "me"));
    // No issuer in the link: the item's name, and its colon stays since the issuer was none of its own
    assert_eq!((read.accounts[1].issuer.as_str(), read.accounts[1].label.as_str()), ("Named", "Co:"));
    assert_eq!(read.accounts[1].tags.as_deref(), Some(&["Personal".to_string()][..]));
    assert_eq!((read.accounts[2].kind, read.accounts[2].issuer.as_str(), read.accounts[2].label.as_str()), (Some(OtpKind::Steam), "Steam", "user"));
    assert_eq!((read.accounts[3].kind, read.accounts[3].secret.as_str(), read.accounts[3].issuer.as_str()), (Some(OtpKind::Totp), KEY, "A bare"));

    let encrypted = json!({"encrypted": true, "encKeyValidation_DO_NOT_EDIT": "x"}).to_string();
    assert_eq!(detect(encrypted.as_bytes()), Some(Detected { source: Source::Bitwarden, encrypted: true }));
    assert_eq!(read_foreign(encrypted.as_bytes(), "any").unwrap_err(), ImportError::BitwardenEncrypted);
    assert_eq!(read_foreign(json!({"encrypted": false, "items": []}).to_string().as_bytes(), "").unwrap_err(), ImportError::Empty);
    assert_eq!(detect(json!({"items": []}).to_string().as_bytes()), None);
}

/// A protobuf by hand: field 1 of accounts, each with this field's numbers
fn varint(mut n: u64, out: &mut Vec<u8>) {
    while n >= 0x80 {
        out.push(n as u8 | 0x80);
        n >>= 7;
    }
    out.push(n as u8);
}

fn bytes_field(number: u64, body: &[u8], out: &mut Vec<u8>) {
    varint(number << 3 | 2, out);
    varint(body.len() as u64, out);
    out.extend_from_slice(body);
}

fn number_field(number: u64, value: u64, out: &mut Vec<u8>) {
    varint(number << 3, out);
    varint(value, out);
}

fn migration(accounts: &[Vec<u8>]) -> String {
    let mut payload = Vec::new();
    for account in accounts {
        bytes_field(1, account, &mut payload);
    }
    number_field(2, 1, &mut payload);
    format!("otpauth-migration://offline?data={}", STANDARD.encode(payload))
}

fn otp_parameters(secret: &[u8], name: &str, issuer: &str, algorithm: u64, digits: u64, kind: u64, counter: u64) -> Vec<u8> {
    let mut out = Vec::new();
    bytes_field(1, secret, &mut out);
    bytes_field(2, name.as_bytes(), &mut out);
    bytes_field(3, issuer.as_bytes(), &mut out);
    number_field(4, algorithm, &mut out);
    number_field(5, digits, &mut out);
    number_field(6, kind, &mut out);
    number_field(7, counter, &mut out);
    out
}

#[test]
fn google_migration() {
    let link = migration(&[
        otp_parameters(b"12345678901234567890", "Co:me", "Co", 1, 1, 2, 0),
        otp_parameters(b"12345678901234567890", "you", "Co", 2, 2, 1, 7),
        otp_parameters(b"12345678901234567890", "Mine: me ", "", 3, 2, 0, 0),
        otp_parameters(b"", "no secret", "", 1, 1, 2, 0),
        otp_parameters(b"x", "md5", "", 4, 1, 2, 0),
        otp_parameters(b"x", "seven", "", 1, 3, 2, 0),
        otp_parameters(b"x", "kind", "", 1, 1, 3, 0),
        otp_parameters(b"x", "too far", "", 1, 1, 1, 1 << 53),
    ]);
    let (accounts, skipped) = read_migration(&link).unwrap();
    assert_eq!((accounts.len(), skipped), (3, 5));
    let a = &accounts[0];
    assert_eq!((a.kind, a.issuer.as_str(), a.label.as_str(), a.secret.as_str(), a.algorithm.as_str(), a.digits, a.period), (Some(OtpKind::Totp), "Co", "me", "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", "SHA1", 6, 30));
    let b = &accounts[1];
    assert_eq!((b.kind, b.counter, b.algorithm.as_str(), b.digits), (Some(OtpKind::Hotp), Some(7), "SHA256", 8));
    assert_eq!((accounts[2].issuer.as_str(), accounts[2].label.as_str(), accounts[2].algorithm.as_str()), ("Mine", "me", "SHA512"));

    let data = link.split_once("data=").unwrap().1.to_string();
    // Scheme in any case, the URL alphabet, spaces and line breaks in it, no padding: the same
    for spelled in [
        link.replace("otpauth-migration", "OTPAUTH-MIGRATION"),
        format!("  {link}\n"),
        format!("otpauth-migration://offline?data={}", data.replace('+', "-").replace('/', "_").trim_end_matches('=')),
        format!("otpauth-migration://offline?other=1&data={data}&data=AAAA"),
        format!("{link}#fragment"),
        format!("otpauth-migration://offline?da%74a={data}"),
        format!("otpauth-migration://offline?data={}", data.replace('=', "%3D")),
    ] {
        assert_eq!(read_migration(&spelled).map(|r| (r.0.len(), r.1)), Some((3, 5)), "{spelled}");
    }
    // What is refused
    for bad in [
        "otpauth-migration://offline?data=".to_string(),
        "otpauth-migration://offline?data".to_string(),
        "otpauth-migration://offline".to_string(),
        format!("otpauth-migration://offline?DATA={data}"),
        format!("otpauth-migration://other?data={data}"),
        format!("otpauth-migration://offline/path?data={data}"),
        format!("otpauth-migration://offline#?data={data}"),
        "otpauth-migration://offline?data=AA==AAAA".to_string(),
        "otpauth-migration://offline?data=AAAAA".to_string(),
        "otpauth-migration://offline?data====".to_string(),
        // Wire types 3 and 7, a key past 64 bits, a truncated length, a field 1 that is no message
        format!("otpauth-migration://offline?data={}", STANDARD.encode([0x0b])),
        format!("otpauth-migration://offline?data={}", STANDARD.encode([0x0f])),
        format!("otpauth-migration://offline?data={}", STANDARD.encode([0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01])),
        format!("otpauth-migration://offline?data={}", STANDARD.encode([0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f])),
        format!("otpauth-migration://offline?data={}", STANDARD.encode([0x0a, 0x05, 0x01])),
        format!("otpauth-migration://offline?data={}", STANDARD.encode([0x0a, 0x01, 0x0b])),
        format!("otpauth-migration://offline?data={}", "A".repeat(70_000)),
    ] {
        assert!(read_migration(&bad).is_none(), "{bad}");
    }
    // A field 1 of the wrong wire type counts as one skipped; no accounts is a valid payload
    assert_eq!(read_migration(&format!("otpauth-migration://offline?data={}", STANDARD.encode([0x08, 0x01, 0x10, 0x01]))).map(|r| (r.0.len(), r.1)), Some((0, 1)));
    assert_eq!(read_migration("otpauth-migration://offline?data=EAE%3D").map(|r| (r.0.len(), r.1)), Some((0, 0)));
    // The last value of a repeated field wins, and a varint where bytes belong reads as its default
    let mut twice = Vec::new();
    bytes_field(1, b"first", &mut twice);
    bytes_field(1, b"f", &mut twice);
    bytes_field(2, b"x", &mut twice);
    number_field(2, 5, &mut twice);
    let (accounts, _) = read_migration(&migration(&[twice])).unwrap();
    assert_eq!((accounts[0].secret.as_str(), accounts[0].label.as_str()), ("MY", ""));
}

#[test]
fn a_file_of_links() {
    let text = format!(
        "\u{feff}\u{feff}\u{a0}otpauth://totp/a?secret={KEY}\u{2028}\n\u{3000}steam://{KEY}\u{feff}\r\n\nnot a link\n\u{85}otpauth://totp/b?secret={KEY}\n{}\notpauth-migration://offline?data=\n",
        migration(&[otp_parameters(b"12345678901234567890", "g", "G", 1, 1, 2, 0)])
    );
    assert_eq!(detect(text.as_bytes()), Some(Detected { source: Source::Links, encrypted: false }));
    let read = read_foreign(text.as_bytes(), "").unwrap();
    assert_eq!(read.source, Source::Links);
    assert_eq!((read.accounts.len(), read.skipped), (3, 3));
    assert_eq!(read.accounts[1].kind, Some(OtpKind::Steam));
    assert_eq!(read.accounts[2].issuer, "G");
    // Two on one line, joined by a lone CR, are one bad line
    let joined = format!("otpauth://totp/a?secret={KEY}\rotpauth://totp/b?secret={KEY}");
    let read = read_foreign(joined.as_bytes(), "").unwrap();
    assert_eq!((read.accounts.len(), read.skipped), (0, 1));
    // Every line a migration: Google's
    let google = migration(&[otp_parameters(b"12345678901234567890", "g", "G", 1, 1, 2, 0)]);
    assert_eq!(detect(google.as_bytes()), Some(Detected { source: Source::Google, encrypted: false }));
    assert_eq!(read_foreign(google.as_bytes(), "").unwrap().source, Source::Google);
    assert_eq!(read_foreign(b"otpauth-migration://offline?data=EAE%3D", "").unwrap_err(), ImportError::Empty);
}

#[test]
fn what_is_not_known() {
    for text in ["", "  \n\t\r\n", "null", "123", "[]", "\"otpauth://totp/a?secret=MZXW6\"", "ordinary text", "otpauth:/totp/a", r#"{"header":[],"db":{}}"#, r#"{"header":{},"db":[]}"#, r#"{"app":"Other"}"#, r#"{"encrypted":"false","items":[]}"#] {
        assert_eq!(detect(text.as_bytes()), None, "{text}");
        assert_eq!(read_foreign(text.as_bytes(), "pw").unwrap_err(), ImportError::Unsupported, "{text}");
    }
    assert_eq!(read_foreign(&vec![b' '; 17 * 1024 * 1024], "").unwrap_err(), ImportError::TooLarge);
    let file = aegis_file(&json!({"entries": []}), "pw", 10, 8, 1, 32).to_string();
    assert_eq!(read_foreign(file.as_bytes(), "pw").unwrap_err(), ImportError::Empty);
    // A __proto__ key is a key like any other, and the last of a repeated key wins
    let repeated = format!(r#"{{"__proto__":{{"x":1}},"header":{{}},"db":"x","db":{{"entries":[{{"type":"totp","info":{{"secret":"{KEY}"}}}}]}}}}"#);
    assert_eq!(read_foreign(repeated.as_bytes(), "").unwrap().accounts.len(), 1);
}

#[test]
fn loose_values_read_as_javascript_would() {
    let digits = |v: Value| {
        let file = json!({"header": {}, "db": {"entries": [{"type": "totp", "info": {"secret": KEY, "digits": v}}]}}).to_string();
        read_foreign(file.as_bytes(), "").unwrap().accounts.first().map(|a| a.digits)
    };
    for (value, want) in [
        (json!(null), Some(6)),
        (json!(""), Some(6)),
        (json!(8), Some(8)),
        (json!(8.0), Some(8)),
        (json!("8"), Some(8)),
        (json!(" 8 "), Some(8)),
        (json!("0x8"), Some(8)),
        (json!("0b110"), Some(6)),
        (json!("0o10"), Some(8)),
        (json!(".8e1"), Some(8)),
        (json!("+8"), Some(8)),
        (json!([8]), Some(8)),
        (json!([[6]]), Some(6)),
        (json!(5), None),
        (json!(true), None),
        (json!({"a": 8}), None),
        (json!("eight"), None),
        (json!("Infinity"), None),
        (json!("inf"), None),
        (json!("nan"), None),
        (json!("8e"), None),
        (json!("-0x8"), None),
        (json!([6, 8]), None),
    ] {
        assert_eq!(digits(value.clone()), want, "{value}");
    }
    let algorithm = |v: Value| {
        let file = json!({"header": {}, "db": {"entries": [{"type": "totp", "info": {"secret": KEY, "algo": v}}]}}).to_string();
        read_foreign(file.as_bytes(), "").unwrap().accounts.first().map(|a| a.algorithm.clone())
    };
    for (value, want) in [
        (json!("sha-256"), Some("SHA256")),
        (json!("Sha512"), Some("SHA512")),
        (json!(["sha512"]), Some("SHA512")),
        (json!("ſha1"), Some("SHA1")),
        (json!("SHA-384"), Some("SHA384")),
        (json!("SHA_256"), None),
        (json!("SHA--256"), None),
        (json!("MD5"), None),
        (json!(256), None),
    ] {
        assert_eq!(algorithm(value.clone()).as_deref(), want, "{value}");
    }
}

// Merge and replace

fn account(issuer: &str, secret: &str) -> Account {
    Account { issuer: issuer.to_string(), secret: Secret::new(secret), ..Account::default() }
}

fn imported(source: Source, accounts: Vec<Account>, services: Vec<BackupService>) -> Imported {
    Imported { source, accounts, services, skipped: 0, plain: false }
}

#[test]
fn the_same_account_goes_by_its_bytes() {
    assert!(same_account(&account("Co", KEY), &account("Co", "orsx g5dt mvrx ezlu====")));
    assert!(!same_account(&account("Co", KEY), &account("co", KEY)));
    assert!(!same_account(&account("Co", KEY), &account("Co", "MZXW6YTBOI")));
    // Label does not count, and a secret that does not decode goes by its text
    let mut labelled = account("Co", KEY);
    labelled.label = "other".to_string();
    assert!(same_account(&account("Co", KEY), &labelled));
    assert!(same_account(&account("Co", "bad!"), &account("Co", "bad!")));
    assert!(!same_account(&account("Co", "bad!"), &account("Co", "BAD!")));
}

#[test]
fn merging() {
    let held_id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let free = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    let held = vec![Account { id: held_id.to_string(), ..account("Co", KEY) }, Account { id: "gone".to_string(), deleted_at: Some(1), ..account("Trash", KEY) }];
    let services = vec![BackupService { id: "s".to_string(), service_name: "Svc".to_string(), ..BackupService::default() }];
    let import = imported(
        Source::BackMyCodes,
        vec![
            Account { id: free.to_string(), added_at: 5, ..account("New", KEY) },
            // An id already held, an id of the wrong shape: each gets its own
            Account { id: held_id.to_string(), ..account("Other", KEY) },
            Account { id: "../../x".to_string(), ..account("Third", KEY) },
            // A padded copy of what is held, and a repeat inside the import
            account("Co", "orsxg5dtmvrxezlu=="),
            account("New", "ORSX G5DT MVRX EZLU"),
            // In the trash is not held
            account("Trash", KEY),
        ],
        vec![
            BackupService { service_name: "Svc".to_string(), ..BackupService::default() },
            BackupService { id: free.to_string(), service_name: "Another".to_string(), ..BackupService::default() },
            BackupService { service_name: "Another".to_string(), ..BackupService::default() },
        ],
    );
    let plan = plan_merge(&held, &services, import, NOW).unwrap();
    assert!(plan.has_anything_new());
    assert_eq!(plan.duplicate, [false, false, false, true, true, false]);
    assert_eq!(plan.duplicate_accounts, 2);
    let names: Vec<&str> = plan.accounts.iter().map(|a| a.issuer.as_str()).collect();
    assert_eq!(names, ["New", "Other", "Third", "Trash"]);
    assert_eq!(plan.accounts[0].id, free);
    assert!(plan.accounts.iter().all(|a| is_id(&a.id) && a.id != held_id && a.added_at == NOW && a.updated_at == Some(NOW)));
    assert_eq!(plan.services.len(), 1);
    assert_eq!(plan.duplicate_services, 2);
    // A file's id free among accounts may not reuse another new entry's
    assert_ne!(plan.services[0].id, free);
    assert!(is_id(&plan.services[0].id));

    // Nothing new at all: the button goes grey. Only new services still merges.
    let nothing = plan_merge(&held, &services, imported(Source::Links, vec![account("Co", KEY)], vec![]), NOW).unwrap();
    assert!(!nothing.has_anything_new());
    let only_services = plan_merge(&held, &services, imported(Source::BackMyCodes, vec![], vec![BackupService { service_name: "S2".to_string(), ..BackupService::default() }]), NOW).unwrap();
    assert!(only_services.has_anything_new());
    // A scan, by the same rule
    let scan = plan_scan(&held, vec![account("Co", KEY), account("X", KEY), account("X", KEY)], NOW).unwrap();
    assert_eq!((scan.accounts.len(), scan.duplicate_accounts), (1, 2));
}

#[test]
fn replacing() {
    let held = vec![Account { id: "a1".to_string(), ..account("Co", KEY) }, Account { id: "a2".to_string(), deleted_at: Some(1), ..account("T", KEY) }];
    let services = vec![BackupService { id: "s1".to_string(), service_name: "Svc".to_string(), ..BackupService::default() }];
    // A foreign file keeps the services
    let plan = plan_replace(&held, &services, imported(Source::Aegis, vec![account("New", KEY), account("New", KEY)], vec![]), NOW).unwrap();
    assert_eq!(plan.remove_accounts, ["a1"]);
    assert!(plan.remove_services.is_empty());
    assert_eq!(plan.add_accounts.len(), 1);
    assert_eq!(plan.add_accounts[0].added_at, NOW);
    // BackMyCodes' own replaces both, and an id being replaced is not free
    let same = BackupService { id: "s1".to_string(), service_name: "Svc".to_string(), ..BackupService::default() };
    let plan = plan_replace(&held, &services, imported(Source::BackMyCodes, vec![], vec![same]), NOW).unwrap();
    assert_eq!(plan.remove_services, ["s1"]);
    assert_eq!(plan.add_services.len(), 1);
    assert!(is_id(&plan.add_services[0].id));
    // An empty import is refused, by every path
    assert_eq!(plan_replace(&held, &services, imported(Source::BackMyCodes, vec![], vec![]), NOW).unwrap_err(), ImportError::Empty);
}

#[test]
fn an_export_is_the_first_format_tried() {
    let accounts = vec![Account { id: "11111111-1111-4111-8111-111111111111".to_string(), ..account("Co", KEY) }];
    let text = write_export(&accounts, &[], "a long enough password", NOW).unwrap();
    assert_eq!(detect(text.as_bytes()), Some(Detected { source: Source::BackMyCodes, encrypted: true }));
    let read = read_file(text.as_bytes(), "a long enough password").unwrap();
    assert_eq!(read.source, Source::BackMyCodes);
    assert_eq!(read.accounts[0].id, accounts[0].id);
    assert_eq!(read_file(text.as_bytes(), "wrong").unwrap_err(), ImportError::BadPassword);
    assert_eq!(read_foreign(text.as_bytes(), "a long enough password").unwrap_err(), ImportError::Unsupported);
    assert_eq!(read_file(br#"{"app":"BackMyCodes","scheme":"plaintext"}"#, "").unwrap_err(), ImportError::Empty);
    let plain = read_file(format!(r#"{{"app":"BackMyCodes","scheme":"plaintext","accounts":[{{"secret":"{KEY}"}}]}}"#).as_bytes(), "").unwrap();
    assert!(plain.plain);
    assert_eq!(plain.accounts.len(), 1);
    // Foreign files go on past it
    assert_eq!(read_file(format!("otpauth://totp/a?secret={KEY}").as_bytes(), "").unwrap().source, Source::Links);
}
