//! Every parser of what comes from outside, on truncated, changed and random bytes: none may panic, and none may hold
//! more than its bound allows. A panic fails the test, and `panic = "abort"` would end a real program.

mod items_support;
use items_support::*;

use amethystora_vault_core::bmc::{Collector, Magic, export_is_encrypted, forgiving_b64, parse_chunk, read_export, unb64, unb64url};
use amethystora_vault_core::codes::{Directory, coverage, parse_codes, parse_document_codes, same_service, service_keys};
use amethystora_vault_core::import::{ImportError, detect, read_file, read_foreign, read_migration};
use amethystora_vault_core::items::{Account, BackupService, Item, KIND_ACCOUNT, KIND_BACKUP_CODES, KIND_PASSWORD, Secret};
use amethystora_vault_core::otp::{self, code_at};
use amethystora_vault_core::otpauth::{read_link, write_link};
use base64::Engine;
use serde_json::json;

#[global_allocator]
static COUNTING: Counting = Counting;

const MIB: usize = 1024 * 1024;

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Truncated at every byte, changed many ways, and random
fn inputs(samples: &[Vec<u8>], rounds: usize, mut each: impl FnMut(&[u8])) {
    let mut random = Random(0x9E37_79B9_7F4A_7C15);
    for sample in samples {
        for cut in 0..sample.len() {
            each(&sample[..cut]);
        }
        for _ in 0..rounds {
            each(&random.mutated(sample));
        }
    }
    for _ in 0..rounds {
        let len = random.below(600);
        each(&random.bytes(len));
    }
}

fn foreign_samples() -> Vec<Vec<u8>> {
    let key = "ORSXG5DTMVRXEZLU";
    let aegis = json!({"version": 1, "header": {"slots": null, "params": null}, "db": {"groups": [{"uuid": "g", "name": "G"}], "entries": [
        {"type": "totp", "name": "n", "issuer": "i", "note": "x", "favorite": true, "groups": ["g"], "info": {"secret": key, "algo": "SHA1", "digits": 6, "period": 30}},
        {"type": "hotp", "info": {"secret": key, "counter": 1}},
        {"type": "steam", "info": {"secret": key}}
    ]}});
    let two_fas = json!({"schemaVersion": 4, "groups": [{"id": "g", "name": "G"}], "services": [
        {"name": "n", "secret": key, "otp": {"tokenType": "TOTP", "account": "a", "digits": 6, "period": 30, "algorithm": "SHA1"}, "order": {"position": 0}, "groupId": "g"}
    ]});
    let two_fas_sealed = json!({"schemaVersion": 4, "services": [], "servicesEncrypted": "AAAA:AAAA:AAAA"});
    let bitwarden = json!({"encrypted": false, "folders": [{"id": "f", "name": "F"}], "items": [
        {"name": "n", "folderId": "f", "login": {"username": "u", "totp": format!("otpauth://totp/a?secret={key}")}},
        {"name": "m", "login": {"totp": key}}
    ]});
    let links = format!("otpauth://totp/a:b?secret={key}&issuer=a\nsteam://{key}\notpauth-migration://offline?data=CikKFDEyMzQ1Njc4OTAxMjM0NTY3ODkwEgVhbGljZRoEQUNNRSABKAEwAhAB\n");
    vec![aegis.to_string().into_bytes(), two_fas.to_string().into_bytes(), two_fas_sealed.to_string().into_bytes(), bitwarden.to_string().into_bytes(), links.into_bytes()]
}

#[test]
fn foreign_files() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    inputs(&foreign_samples(), 3000, |bytes| {
        let ((), peak, _) = measured(|| {
            let _ = detect(bytes);
            let _ = read_foreign(bytes, "pw");
            let _ = read_file(bytes, "pw");
        });
        assert!(peak < 8 * MIB, "{peak} bytes for an input of {}", bytes.len());
    });
}

fn seal(key: &[u8], nonce: &[u8], plain: &[u8]) -> (Vec<u8>, [u8; 16]) {
    let cipher = if key.len() == 16 { openssl::symm::Cipher::aes_128_gcm() } else { openssl::symm::Cipher::aes_256_gcm() };
    let mut tag = [0u8; 16];
    let ct = openssl::symm::encrypt_aead(cipher, key, Some(nonce), &[], plain, &mut tag).unwrap();
    (ct, tag)
}

/// An encrypted Aegis file whose one slot opens with "pw" at a cost of next to nothing (n=2), its database whatever
/// bytes a test gives
fn aegis_sealing(db: &[u8]) -> Vec<u8> {
    let (master, salt, key_nonce, db_nonce) = ([0x5au8; 32], [3u8; 32], [1u8; 12], [2u8; 12]);
    let mut derived = [0u8; 32];
    scrypt::scrypt(b"pw", &salt, &scrypt::Params::new(1, 1, 1).unwrap(), &mut derived).unwrap();
    let (key, key_tag) = seal(&derived, &key_nonce, &master);
    let (ct, db_tag) = seal(&master, &db_nonce, db);
    json!({"version": 1, "header": {
        "slots": [{"type": 1, "key": hex(&key), "key_params": {"nonce": hex(&key_nonce), "tag": hex(&key_tag)}, "n": 2, "r": 1, "p": 1, "salt": hex(&salt)}],
        "params": {"nonce": hex(&db_nonce), "tag": hex(&db_tag)}}, "db": base64::engine::general_purpose::STANDARD.encode(ct)})
    .to_string()
    .into_bytes()
}

/// An encrypted 2FAS file that opens with "pw", its services whatever bytes a test gives
fn two_fas_sealing(services: &[u8], key: &[u8; 32]) -> Vec<u8> {
    let (salt, iv) = ([4u8; 32], [9u8; 12]);
    let (mut ct, tag) = seal(key, &iv, services);
    ct.extend_from_slice(&tag);
    let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    json!({"schemaVersion": 4, "services": [], "servicesEncrypted": format!("{}:{}:{}", b64(&ct), b64(&salt), b64(&iv))}).to_string().into_bytes()
}

/// Past the password: a slot that opens (try_slot's own path), the database it opens (open_aegis's), and what each holds
/// changed every way, so that what was decrypted is no more trusted than what was not
#[test]
fn encrypted_files_past_their_password() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let db = json!({"groups": [{"uuid": "g", "name": "G"}], "entries": [
        {"type": "totp", "name": "n", "issuer": "i", "groups": ["g"], "info": {"secret": "ORSXG5DTMVRXEZLU", "algo": "SHA1", "digits": 6, "period": 30}},
        {"type": "hotp", "info": {"secret": "ORSXG5DTMVRXEZLU", "counter": 1}}
    ]})
    .to_string();
    let whole = aegis_sealing(db.as_bytes());
    assert_eq!(read_foreign(&whole, "pw").unwrap().accounts.len(), 2, "the sample opens");
    // The file around it, changed: slots, costs, hex and base64
    inputs(&[whole], 1500, |bytes| {
        let ((), peak, _) = measured(|| {
            let _ = read_foreign(bytes, "pw");
        });
        assert!(peak < 8 * MIB, "{peak}");
    });
    // What it decrypts to, changed: every one of these opens and is then parsed
    inputs(&[db.clone().into_bytes()], 1500, |bytes| {
        let file = aegis_sealing(bytes);
        let ((), peak, _) = measured(|| {
            let _ = read_foreign(&file, "pw");
        });
        assert!(peak < 8 * MIB, "{peak}");
    });

    let services = json!([{"name": "n", "secret": "ORSXG5DTMVRXEZLU", "otp": {"account": "a", "tokenType": "HOTP", "counter": 2}, "order": {"position": 0}}]).to_string();
    let mut key = [0u8; 32];
    openssl::pkcs5::pbkdf2_hmac(b"pw", &[4u8; 32], 10_000, openssl::hash::MessageDigest::sha256(), &mut key).unwrap();
    assert_eq!(read_foreign(&two_fas_sealing(services.as_bytes(), &key), "pw").unwrap().accounts.len(), 1, "the sample opens");
    let mut random = Random(5);
    for _ in 0..300 {
        let changed = random.mutated(services.as_bytes());
        let _ = read_foreign(&two_fas_sealing(&changed, &key), "pw");
    }
}

/// ML-KEM's own inputs, hostile: a secret key and a ciphertext that the password's own wrap hands over, of every wrong
/// shape. Each costs one scrypt of the export's, so there are few.
#[test]
fn export_keys_that_are_hostile() {
    use amethystora_vault_core::bmc::{BmcError, write_export};
    // Its scrypt runs while other tests measure their peaks
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let password = "a long enough password";
    let file: serde_json::Value = serde_json::from_str(&write_export(&[], &[], password, 1).unwrap()).unwrap();
    let b64 = base64::engine::general_purpose::STANDARD;
    let salt = b64.decode(file["config"]["salt"].as_str().unwrap()).unwrap();
    let mut kek = [0u8; 32];
    scrypt::scrypt(password.as_bytes(), &salt, &scrypt::Params::new(17, 8, 1).unwrap(), &mut kek).unwrap();
    let mut random = Random(99);
    let secret_keys: Vec<Vec<u8>> = vec![random.bytes(3168), vec![0; 3168], vec![0xff; 3168], vec![], random.bytes(3167), random.bytes(3169), random.bytes(64)];
    let ciphertexts: Vec<Vec<u8>> = vec![random.bytes(1568), vec![], random.bytes(1), random.bytes(1567), random.bytes(1569), vec![0xff; 100_000]];
    let real_ct = b64.decode(file["vault"]["kemCT"].as_str().unwrap()).unwrap();
    let cases = secret_keys.iter().map(|sk| (Some(sk), &real_ct)).chain(ciphertexts.iter().map(|ct| (None, ct)));
    for (secret_key, kem_ct) in cases {
        let mut hostile = file.clone();
        if let Some(sk) = secret_key {
            let nonce = [6u8; 12];
            let (mut wrapped, tag) = seal(&kek, &nonce, sk);
            wrapped.extend_from_slice(&tag);
            hostile["config"]["skNonce"] = json!(b64.encode(nonce));
            hostile["config"]["skEnc"] = json!(b64.encode(wrapped));
        }
        hostile["vault"]["kemCT"] = json!(b64.encode(kem_ct));
        assert_eq!(read_export(hostile.to_string().as_bytes(), password).unwrap_err(), BmcError::BadPassword);
    }
}

#[test]
fn links_and_secrets() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let samples: Vec<Vec<u8>> = [
        "otpauth://totp/Co%20Op:me%40x?secret=ORSXG5DTMVRXEZLU&issuer=Co%20Op&algorithm=SHA256&digits=8&period=60",
        "otpauth://hotp/x?secret=ORSXG5DTMVRXEZLU&counter=3&encoder=steam",
        "otpauth://steam/Steam:gaben?secret=ORSXG5DTMVRXEZLU&issuer=Steam&digits=5",
        "steam://ORSXG5DTMVRXEZLU==",
        "ſtraße ﬀ  \u{212a} = é",
    ]
    .iter()
    .map(|s| s.as_bytes().to_vec())
    .collect();
    inputs(&samples, 5000, |bytes| {
        let text = lossy(bytes);
        let ((), peak, _) = measured(|| {
            if let Some(account) = read_link(&text) {
                let _ = write_link(&account);
                let _ = code_at(&account, 1_700_000_000, 0);
            }
            let _ = otp::base32_decode(&text);
            let _ = otp::canonical_secret(&text);
            let _ = otp::structured_secret(&text);
            let _ = otp::typed_secret_is_valid(&otp::normalize_typed(&text));
            let _ = otp::group_secret(&text);
            let _ = otp::group_code(&text);
            let odd = Account { secret: Secret::new(text.as_str()), algorithm: text.clone(), digits: bytes.len() as i64 - 3, period: bytes.len() as i64 - 3, ..Account::default() };
            let _ = code_at(&odd, i64::MIN + 5, i64::MAX);
            let _ = code_at(&odd, i64::MAX, i64::MIN);
            let _ = write_link(&odd);
        });
        assert!(peak < MIB, "{peak}");
    });
}

#[test]
fn google_links() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let good = "CikKFDEyMzQ1Njc4OTAxMjM0NTY3ODkwEgVhbGljZRoEQUNNRSABKAEwAhAB";
    let samples = vec![format!("otpauth-migration://offline?data={good}").into_bytes()];
    inputs(&samples, 5000, |bytes| {
        let ((), peak, _) = measured(|| {
            let _ = read_migration(&lossy(bytes));
        });
        assert!(peak < MIB, "{peak}");
    });
    // Random bytes as the protobuf itself, which reach further than random text does
    let mut random = Random(77);
    for _ in 0..20_000 {
        let len = random.below(200);
        let link = format!("otpauth-migration://offline?data={}", base64::engine::general_purpose::STANDARD.encode(random.bytes(len)));
        let _ = read_migration(&link);
    }
    // A huge length in a varint must not reach an allocation
    let huge = base64::engine::general_purpose::STANDARD.encode([0x0a, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]);
    let (read, peak, _) = measured(|| read_migration(&format!("otpauth-migration://offline?data={huge}")));
    assert!(read.is_none());
    assert!(peak < MIB, "{peak}");
}

#[test]
fn transfers() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let plain = format!("BMCP:01/01:{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"a":[{"i":"x","l":"y","s":"MZXW6","a":"SHA1","d":6,"p":30,"t":"hotp","c":3}],"b":[{"n":"s","c":[{"code":"c","used":true,"usedAt":1}]}]}"#));
    let samples = vec![plain.clone().into_bytes(), b"BMC1:02/03:AAEC_w".to_vec(), br#"{"a":[{"i":1,"p":"x"}],"b":[{"n":[],"c":5}]}"#.to_vec()];
    inputs(&samples, 5000, |bytes| {
        let text = lossy(bytes);
        let ((), peak, _) = measured(|| {
            let _ = parse_chunk(&text, Magic::Bmc1);
            let _ = parse_chunk(&text, Magic::Bmcp);
            let _ = unb64(&text);
            let _ = unb64url(&text);
            let _ = forgiving_b64(&text);
            let mut collector = Collector::new();
            collector.add(&text);
            collector.add(&plain);
            if collector.is_plain() {
                let _ = collector.read("", 1);
            }
            // A plain transfer's JSON, whatever it holds
            let mut mine = Collector::new();
            mine.add(&format!("BMCP:01/01:{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes[..bytes.len().min(2000)])));
            let _ = mine.read("", 1);
        });
        assert!(peak < 8 * MIB, "{peak}");
    });
}

#[test]
fn exports_without_their_scrypt() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    // A plain file reaches every field; an encrypted one with no password stops before its scrypt
    let plain = br#"{"version":2,"app":"BackMyCodes","exportedAt":"x","scheme":"plaintext","accounts":[{"id":"a","type":"totp","issuer":"i","label":"l","secret":"MZXW6","algorithm":"SHA1","digits":6,"period":30,"tags":["t"],"note":"n","addedAt":1,"more":{"k":[1]}}],"backupServices":[{"id":"s","serviceName":"n","codes":[{"code":"c","used":false}],"addedAt":2}]}"#.to_vec();
    let sealed = br#"{"version":2,"app":"BackMyCodes","scheme":"x","config":{"salt":"AAAA","skNonce":"AAAA","skEnc":"AAAA","publicKey":"AAAA","scrypt":{"N":1,"r":1,"p":1}},"vault":{"kemCT":"AAAA","nonce":"AAAA","ct":"AAAA"}}"#.to_vec();
    inputs(&[plain, sealed], 5000, |bytes| {
        let ((), peak, _) = measured(|| {
            let _ = export_is_encrypted(bytes);
            let _ = read_export(bytes, "");
        });
        assert!(peak < 8 * MIB, "{peak}");
    });
}

#[test]
fn items_from_the_store() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let samples = vec![
        br#"{"id":"a","title":"t","password":"p","history":[{"password":"q","changedAt":1}],"tags":["x"],"addedAt":1}"#.to_vec(),
        br#"{"id":"a","type":"hotp","secret":"MZXW6","digits":6,"counter":1,"note":"n","more":[1]}"#.to_vec(),
        br#"{"id":"a","serviceName":"s","codes":[{"code":"c","used":true}]}"#.to_vec(),
    ];
    inputs(&samples, 3000, |bytes| {
        for kind in [KIND_PASSWORD, KIND_ACCOUNT, KIND_BACKUP_CODES, "x"] {
            if let Some(item) = Item::from_json(kind, bytes) {
                let _ = item.to_json();
            }
        }
    });
}

#[test]
fn codes_and_names() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let samples: Vec<Vec<u8>> = [
        "1. 1234 5678    2. 2345 6789\n• abc-123, def-456\t\tghi\r\n-\n1.\n1)x\n",
        "GitHub, Inc. (work) https://www.example.co.uk/ path\nŸ Я 😀 İ Σ",
        "http://a.b/c\nd",
        r#"{"entries":[["n","d.com","https://x",0,["e.org"]],["m",1,2,3],5]}"#,
    ]
    .iter()
    .map(|s| s.as_bytes().to_vec())
    .collect();
    inputs(&samples, 5000, |bytes| {
        let text = lossy(bytes);
        let ((), peak, _) = measured(|| {
            let _ = parse_codes(&text);
            let _ = parse_document_codes(&text);
            let _ = service_keys(&text);
            let _ = same_service(&text, "GitHub");
            let _ = amethystora_vault_core::codes::hue(&text);
            let _ = coverage(
                &[Account { issuer: text.clone(), ..Account::default() }, Account { label: text.clone(), ..Account::default() }],
                &[BackupService { service_name: text.clone(), ..BackupService::default() }],
            );
            if let Some(directory) = Directory::parse(bytes) {
                let _ = directory.find(&text);
            }
        });
        assert!(peak < 8 * MIB, "{peak}");
    });
}

/// The biggest things a file may be, held to what is bounded: a file's own size, parsed JSON, and no scrypt past its cap
#[test]
fn the_largest_inputs_stay_within_their_bounds() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    const FILE: usize = 16 * MIB;
    // Past the file bound: refused before anything is read
    let over = vec![b'['; FILE + 1];
    let (read, peak, _) = measured(|| read_foreign(&over, "pw"));
    assert_eq!(read.unwrap_err(), ImportError::TooLarge);
    assert!(peak < MIB, "{peak}");

    // At the file bound, as nodes: too many of them, refused without building them
    let mut nodes = vec![b'['];
    nodes.extend(std::iter::repeat_n(&b"[],"[..], (FILE - 2) / 3).flatten());
    nodes.push(b']');
    let (read, peak, _) = measured(|| read_foreign(&nodes, "pw"));
    assert_eq!(read.unwrap_err(), ImportError::TooLarge);
    assert!(peak < 2 * FILE + MIB, "{peak}");

    // Just under the node bound, which may be built: its cost is the bound's. Different keys are the dearest form.
    let mut most = String::from("{");
    for i in 0..499_990 {
        most.push_str(&format!("\"{i}\":0,"));
    }
    most.push_str("\"last\":0}");
    let (read, peak, _) = measured(|| read_foreign(most.as_bytes(), "pw"));
    assert_eq!(read.unwrap_err(), ImportError::Unsupported);
    assert!(peak < 128 * MIB, "{peak}");

    // As deep as the node bound lets it go: serde_json stops at 128 levels, so no stack goes with it
    let deep = vec![b'['; 999_000];
    assert_eq!(read_foreign(&deep, "pw").unwrap_err(), ImportError::Unsupported);
    let deep_objects = "{\"a\":".repeat(499_000);
    assert_eq!(read_foreign(deep_objects.as_bytes(), "pw").unwrap_err(), ImportError::Unsupported);

    // Too many entries of each kind are refused whole
    let many = |make: &dyn Fn(String) -> String| make(vec![r#"{"type":"totp"}"#; 65_537].join(","));
    let aegis = many(&|list| format!(r#"{{"header":{{}},"db":{{"entries":[{list}]}}}}"#));
    assert_eq!(read_foreign(aegis.as_bytes(), "").unwrap_err(), ImportError::TooLarge);
    let two_fas = many(&|list| format!(r#"{{"schemaVersion":4,"services":[{list}]}}"#));
    assert_eq!(read_foreign(two_fas.as_bytes(), "").unwrap_err(), ImportError::TooLarge);
    let bitwarden = many(&|list| format!(r#"{{"encrypted":false,"items":[{list}]}}"#));
    assert_eq!(read_foreign(bitwarden.as_bytes(), "").unwrap_err(), ImportError::TooLarge);
    let export = many(&|list| format!(r#"{{"app":"BackMyCodes","scheme":"plaintext","accounts":[{list}]}}"#));
    assert_eq!(read_file(export.as_bytes(), "").unwrap_err(), ImportError::TooLarge);
    let lines = "otpauth://totp/a?secret=MZXW6\n".repeat(65_537);
    assert_eq!(read_foreign(lines.as_bytes(), "").unwrap_err(), ImportError::TooLarge);

    // A scrypt past the cap allocates nothing on its way to being refused, a thousand slots of it
    let slot = json!({"type": 1, "n": 1u64 << 20, "r": 16, "p": 16, "salt": "00", "key": "00", "key_params": {"nonce": "00", "tag": "00"}});
    let file = json!({"header": {"slots": vec![slot; 1000], "params": {"nonce": "00", "tag": "00"}}, "db": "AAAA"}).to_string();
    let (read, peak, _) = measured(|| read_foreign(file.as_bytes(), "pw"));
    assert_eq!(read.unwrap_err(), ImportError::TooMuchMemory);
    assert!(peak < 8 * MIB, "{peak}");

    // A scrypt at the cap is what its bound says, and no more
    let slot = json!({"type": 1, "n": 1u64 << 18, "r": 8, "p": 1, "salt": "00", "key": "00", "key_params": {"nonce": "00", "tag": "00"}});
    let file = json!({"header": {"slots": [slot], "params": {"nonce": "00", "tag": "00"}}, "db": "AAAA"}).to_string();
    let (read, peak, _) = measured(|| read_foreign(file.as_bytes(), "pw"));
    assert_eq!(read.unwrap_err(), ImportError::BadPassword);
    assert!(peak < 256 * MIB + 8 * MIB, "{peak}");
    assert!(peak > 256 * MIB, "{peak}: the cap was to be reached");

    // An entry past its bounds is one skipped, however long
    let long = "x".repeat(2 * MIB);
    let file = json!({"header": {}, "db": {"entries": [{"type": "totp", "issuer": long, "info": {"secret": "MZXW6"}}, {"type": "totp", "info": {"secret": long}}, {"type": "totp", "info": {"secret": "MZXW6"}}]}}).to_string();
    let read = read_foreign(file.as_bytes(), "").unwrap();
    assert_eq!((read.accounts.len(), read.skipped), (1, 2));
    // And text past its own bound gives no codes
    let refused = parse_codes(&"a ".repeat(MIB));
    assert!(refused.codes.is_empty() && refused.text_too_large);
    assert!(service_keys(&"a".repeat(MIB)).is_empty());
}
