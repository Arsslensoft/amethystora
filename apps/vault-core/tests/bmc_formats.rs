//! BMC1, BMCP and the file export, read and written, with values of this file's own and the one the spec worked out from
//! fixed randomness. Section 7 and 8's rules hold every line.

mod items_support;
use items_support::need;

use amethystora_vault_core::bmc::{
    Added, BmcError, Collector, Magic, export_file_name, export_is_encrypted, forgiving_b64, gcm_open, iso_time, parse_chunk, read_export,
    unb64, unb64url, write_bmc1, write_bmcp, write_export, write_export_v1, wrong_in_backmycodes,
};
use amethystora_vault_core::items::{Account, BackupCode, BackupService, OtpKind, Secret, is_id};
use amethystora_vault_core::otp::code_at;
use base64::Engine;
use serde_json::{Value, json};

const NOW: i64 = 1_800_000_000_000;
const PASSWORD: &str = "a long enough password";

fn sample_accounts() -> Vec<Account> {
    vec![
        Account {
            id: "11111111-1111-4111-8111-111111111111".to_string(),
            kind: Some(OtpKind::Totp),
            issuer: "Café ☕".to_string(),
            label: "me@example.org".to_string(),
            secret: Secret::new("ORSXG5DTMVRXEZLU"),
            algorithm: "SHA256".to_string(),
            digits: 8,
            period: 60,
            pinned: Some(true),
            tags: Some(vec!["Work".to_string()]),
            order: Some(3),
            note: Some(Secret::new("the blue one")),
            no_backup_codes: Some(true),
            added_at: 1_700_000_000_000,
            updated_at: Some(1_700_000_000_500),
            ..Account::default()
        },
        Account {
            id: "22222222-2222-4222-8222-222222222222".to_string(),
            kind: Some(OtpKind::Hotp),
            issuer: "Bank".to_string(),
            label: "me".to_string(),
            secret: Secret::new("MZXW6YTBOI"),
            counter: Some(41),
            added_at: 1_700_000_001_000,
            ..Account::default()
        },
        Account {
            id: "33333333-3333-4333-8333-333333333333".to_string(),
            kind: Some(OtpKind::Steam),
            issuer: "Steam".to_string(),
            label: "player".to_string(),
            secret: Secret::new("MZXW6YTBOI"),
            added_at: 1_700_000_002_000,
            ..Account::default()
        },
    ]
}

fn sample_services() -> Vec<BackupService> {
    let mut used = BackupCode::new("bbbb-2222");
    used.mark_used(1_700_000_003_000);
    vec![BackupService {
        id: "44444444-4444-4444-8444-444444444444".to_string(),
        service_name: "Example".to_string(),
        codes: vec![BackupCode::new("aaaa-1111"), used],
        note: Some(Secret::new("from the old laptop")),
        added_at: 1_700_000_004_000,
        ..BackupService::default()
    }]
}

// Base64's readers

#[test]
fn base64_readers() {
    let bytes = |v: Option<zeroize::Zeroizing<Vec<u8>>>| v.map(|b| b.to_vec());
    assert_eq!(bytes(unb64("Zm9v")), Some(b"foo".to_vec()));
    assert_eq!(bytes(unb64(" Zm9v\nYg==\t")), Some(b"foob".to_vec()));
    assert_eq!(bytes(unb64("Zm9vYg")), Some(b"foob".to_vec()));
    assert_eq!(bytes(unb64("Zm9vY")), None);
    assert_eq!(bytes(unb64("Zm9_")), None);
    // Exports are canonical: bits past the last byte are refused
    assert_eq!(bytes(unb64("Zm9vYh==")), None);

    assert_eq!(bytes(unb64url("AAEC_w")), Some(vec![0, 1, 2, 255]));
    assert_eq!(bytes(unb64url("AAEC/w==")), Some(vec![0, 1, 2, 255]));
    assert_eq!(bytes(unb64url("AAEC*w")), None);

    // The forgiving reader: both alphabets, any whitespace, missing padding, unused bits ignored
    assert_eq!(bytes(forgiving_b64("Zm9v\u{a0}Yg")), Some(b"foob".to_vec()));
    assert_eq!(bytes(forgiving_b64("AAEC_w")), Some(vec![0, 1, 2, 255]));
    assert_eq!(bytes(forgiving_b64("Zm9vYh==")), Some(b"foob".to_vec()));
    assert_eq!(bytes(forgiving_b64("===")), None);
    assert_eq!(bytes(forgiving_b64("Zg==Zg==")), None);
    assert_eq!(bytes(forgiving_b64("Zm9vZ")), None);
}

#[test]
fn aes_gcm_refuses_what_does_not_check() {
    // NIST's GCM test case 13, AES-256 with an all-zero key and nonce, and no plaintext
    let tag = [0x53, 0x0f, 0x8a, 0xfb, 0xc7, 0x45, 0x36, 0xb9, 0xa9, 0x63, 0xb4, 0xf1, 0xc4, 0xcb, 0x73, 0x8b];
    assert_eq!(gcm_open(&[0; 32], &[0; 12], &tag).unwrap().len(), 0);
    let mut bad = tag;
    bad[0] ^= 1;
    assert!(gcm_open(&[0; 32], &[0; 12], &bad).is_none());
    assert!(gcm_open(&[0; 32], &[0; 12], &tag[..15]).is_none());
    assert!(gcm_open(&[0; 32], &[], &tag).is_none());
    assert!(gcm_open(&[0; 20], &[0; 12], &tag).is_none());
}

// Chunks

#[test]
fn chunk_syntax() {
    let c = parse_chunk("BMC1:02/03:AAEC_w", Magic::Bmc1).unwrap();
    assert_eq!((c.index, c.total, c.bytes.to_vec()), (2, 3, vec![0, 1, 2, 255]));
    // parseInt's looseness
    let c = parse_chunk("BMC1:1x/2y:AAEC", Magic::Bmc1).unwrap();
    assert_eq!((c.index, c.total), (1, 2));
    let c = parse_chunk("BMC1: +1/ 2:AAEC", Magic::Bmc1).unwrap();
    assert_eq!((c.index, c.total), (1, 2));
    for bad in [
        "BMCP:01/01:AAEC",
        "BMC1:0101:AAEC",
        "BMC1:01/01AAEC",
        "BMC1:00/01:AAEC",
        "BMC1:02/01:AAEC",
        "BMC1:01/00:AAEC",
        "BMC1:xx/01:AAEC",
        "BMC1:01/xx:AAEC",
        "BMC1:-1/01:AAEC",
        "bmc1:01/01:AAEC",
        "BMC1:01:01/02:AAEC",
        "BMC1:01/01:AAE*",
        // Past the bound, however it is spelled
        "BMC1:01/257:AAEC",
        "BMC1:01/99999999999999999999999999:AAEC",
    ] {
        assert!(parse_chunk(bad, Magic::Bmc1).is_none(), "{bad}");
    }
    assert!(parse_chunk("BMCP:01/01:AAEC", Magic::Bmcp).is_some());
    let long = format!("BMC1:01/01:{}", "A".repeat(3000));
    assert!(parse_chunk(&long, Magic::Bmc1).is_none());
}

#[test]
fn collecting_in_any_order() {
    let mut many = Vec::new();
    for i in 0..1500 {
        many.push(Account { issuer: format!("issuer number {i}"), secret: Secret::new("ORSXG5DTMVRXEZLU"), ..Account::default() });
    }
    let chunks = write_bmcp(&many, &[]).unwrap();
    assert!(chunks.len() > 2);
    let total = chunks.len();
    let mut collector = Collector::new();
    assert_eq!(collector.add("nothing like a chunk"), Added::NotAChunk);
    assert_eq!(collector.read("", NOW).unwrap_err(), BmcError::NoChunks);
    for (i, chunk) in chunks.iter().enumerate().rev() {
        assert_eq!(collector.add(chunk.as_str()), Added::New);
        assert_eq!(collector.add(chunk.as_str()), Added::Again);
        assert!(collector.has(i + 1));
        if i > 0 {
            assert_eq!(collector.read("", NOW).unwrap_err(), BmcError::Incomplete);
            assert_eq!(collector.missing(), (1..=i).collect::<Vec<_>>());
        }
        assert_eq!(collector.total(), total);
    }
    assert!(collector.is_plain());
    let read = collector.read("", NOW).unwrap();
    assert!(read.plain);
    assert_eq!(read.accounts.len(), 1500);
    assert_eq!(read.accounts[1499].issuer, "issuer number 1499");
    // The text of a chunk: its full length is section 7's arithmetic
    let first = chunks[0].as_str();
    assert!(first.starts_with(&format!("BMCP:01/{total:02}:")));
    assert_eq!(first.len(), 11 + 2731);
}

#[test]
fn another_transfer_is_said_so() {
    let mut collector = Collector::new();
    assert_eq!(collector.add("BMCP:01/02:AAEC"), Added::New);
    assert_eq!(collector.add("BMC1:02/02:AAEC"), Added::OtherTransfer);
    assert_eq!(collector.add("BMCP:02/03:AAEC"), Added::OtherTransfer);
    assert_eq!(collector.add("BMCP:01/02:AAED"), Added::OtherTransfer);
    assert_eq!(collector.have(), 1);
    assert_eq!(collector.add("BMCP:02/02:AAEC"), Added::New);
    assert!(collector.is_complete());
}

#[test]
fn reading_the_worked_example() {
    // Section 7's own, from fixed randomness and zlib's deflate
    let chunk = "BMC1:01/01:AAECAwQFBgcICQoLDA0OD6ChoqOkpaanqKmqqwCUdCudy0bnZ4j_b6gi-Nlk9uPdhvEa4ar2FtbtFHnXk1zVObAzT_fNYmux1WHRfdryUkzEgSVMuaqKb3CtFPPwq964ucEdVN2dLtjkNnqKcg1ldLnF10viq6fwdYwcyIwEJaggiezXWF8B1PoQFQLi3lTpmL64vSuVyZNhoBi-lK7xtn3-bGI0F4fVhK53Ey_j4R7Cmvw7uiAt6HGOHsf8soE";
    let mut collector = Collector::new();
    assert_eq!(collector.add(chunk), Added::New);
    assert!(!collector.is_plain());
    let read = collector.read("correct horse battery", NOW).unwrap();
    assert!(!read.plain);
    assert_eq!(read.accounts.len(), 1);
    let account = &read.accounts[0];
    assert_eq!((account.issuer.as_str(), account.label.as_str(), account.secret.as_str()), ("ACME", "alice", "JBSWY3DPEHPK3PXP"));
    assert_eq!((account.kind, account.added_at), (None, NOW));
    assert!(is_id(&account.id));
    assert_eq!(read.services[0].service_name, "GitHub");
    assert_eq!(read.services[0].codes[1].used_at, Some(1_700_000_003_000));
    assert_eq!(collector.read("not it", NOW).unwrap_err(), BmcError::BadPassword);
    // BMCP of the same payload, both shapes
    let mut collector = Collector::new();
    collector.add("BMCP:01/01:eyJhIjpbeyJpIjoiQUNNRSIsImwiOiJhbGljZSIsInMiOiJKQlNXWTNEUEVIUEszUFhQIiwiYSI6IlNIQTEiLCJkIjo2LCJwIjozMH1dLCJiIjpbeyJuIjoiR2l0SHViIiwiYyI6W3siY29kZSI6ImFhYWEtMTExMSIsInVzZWQiOmZhbHNlfSx7ImNvZGUiOiJiYmJiLTIyMjIiLCJ1c2VkIjp0cnVlLCJ1c2VkQXQiOjE3MDAwMDAwMDMwMDB9XX1dfQ");
    assert_eq!(collector.read("", NOW).unwrap().services[0].codes.len(), 2);
    let mut collector = Collector::new();
    collector.add("BMCP:01/01:W3siaSI6IkFDTUUiLCJsIjoiYWxpY2UiLCJzIjoiSkJTV1kzRFBFSFBLM1BYUCIsImEiOiJTSEExIiwiZCI6NiwicCI6MzB9XQ");
    let read = collector.read("", NOW).unwrap();
    assert_eq!((read.accounts.len(), read.services.len()), (1, 0));
}

#[test]
fn transfers_round_trip_with_kind_and_counter() {
    let (accounts, services) = (sample_accounts(), sample_services());
    let chunks = write_bmc1(&accounts, &services, PASSWORD).unwrap();
    let mut collector = Collector::new();
    for chunk in &chunks {
        assert!(chunk.as_str().starts_with("BMC1:"));
        assert_eq!(collector.add(chunk.as_str()), Added::New);
    }
    let read = collector.read(PASSWORD, NOW).unwrap();
    assert_eq!(read.accounts.len(), 3);
    // The kinds and the counter survive (decision 1), the rest of an account does not, as in any QR
    assert_eq!(read.accounts[0].kind, None);
    assert_eq!((read.accounts[1].kind, read.accounts[1].counter), (Some(OtpKind::Hotp), Some(41)));
    assert_eq!(read.accounts[2].kind, Some(OtpKind::Steam));
    assert!(read.accounts[0].note.is_none());
    assert_eq!(read.accounts[0].added_at, NOW);
    for (a, b) in read.accounts.iter().zip(&accounts) {
        assert_eq!(code_at(a, 1_700_000_000, 0).as_str(), code_at(b, 1_700_000_000, 0).as_str());
        assert_ne!(a.id, b.id, "a transfer gives new ids");
    }
    assert!(read.services[0].codes[1].used);
    assert_eq!(read.services[0].codes[1].code.as_str(), "bbbb-2222");
    assert_eq!(wrong_in_backmycodes(&accounts).len(), 2);

    // The JSON itself: BackMyCodes' six fields, in order, then the two it ignores
    let chunk = write_bmcp(&accounts[1..2], &[]).unwrap();
    let body = chunk[0].as_str().rsplit(':').next().unwrap();
    let json = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(body).unwrap();
    assert_eq!(
        String::from_utf8(json).unwrap(),
        r#"{"a":[{"i":"Bank","l":"me","s":"MZXW6YTBOI","a":"SHA1","d":6,"p":30,"t":"hotp","c":41}],"b":[]}"#
    );
    // A short password, and what was removed, never get written
    assert_eq!(write_bmc1(&accounts, &services, "short").unwrap_err(), BmcError::WeakPassword);
    let mut removed = sample_accounts();
    removed[1].deleted_at = Some(NOW);
    let chunk = write_bmcp(&removed[1..2], &[]).unwrap();
    assert_eq!(base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(chunk[0].as_str().rsplit(':').next().unwrap()).unwrap(), br#"{"a":[],"b":[]}"#);
}

#[test]
fn an_inflation_bomb_stops_at_its_bound() {
    // 32 MiB of spaces deflates to some 32 KiB, sealed and sent as a transfer of a few codes
    use std::io::Write;
    let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(9));
    encoder.write_all(&vec![b' '; 32 * 1024 * 1024]).unwrap();
    let compressed = encoder.finish().unwrap();
    let (salt, iv) = ([7u8; 16], [9u8; 12]);
    let mut key = [0u8; 32];
    openssl::pkcs5::pbkdf2_hmac(PASSWORD.as_bytes(), &salt, 200_000, openssl::hash::MessageDigest::sha256(), &mut key).unwrap();
    let mut tag = [0u8; 16];
    let ct = openssl::symm::encrypt_aead(openssl::symm::Cipher::aes_256_gcm(), &key, Some(&iv), &[], &compressed, &mut tag).unwrap();
    let blob = [&salt[..], &iv[..], &ct[..], &tag[..]].concat();
    let mut collector = Collector::new();
    let slices: Vec<&[u8]> = blob.chunks(2048).collect();
    for (i, slice) in slices.iter().enumerate() {
        let text = format!("BMC1:{:02}/{:02}:{}", i + 1, slices.len(), base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(slice));
        assert_eq!(collector.add(&text), Added::New);
    }
    assert_eq!(collector.read(PASSWORD, NOW).unwrap_err(), BmcError::TooLarge);
}

// The file export

#[test]
fn an_export_round_trips_and_reads_in_its_exact_shape() {
    let (accounts, services) = (sample_accounts(), sample_services());
    let text = write_export(&accounts, &services, PASSWORD, 1_792_000_000_123).unwrap();
    assert_eq!(export_is_encrypted(text.as_bytes()), Some(true));

    // Exactly the key order and the two-space form of section 8
    let keys: Vec<&str> = text.lines().map(|l| l.trim_start().split('"').nth(1).unwrap_or("")).filter(|k| !k.is_empty()).collect();
    assert_eq!(
        keys,
        ["version", "app", "exportedAt", "scheme", "config", "salt", "skNonce", "skEnc", "publicKey", "scrypt", "N", "r", "p", "vault", "kemCT", "nonce", "ct"]
    );
    assert!(text.starts_with("{\n  \"version\": 2,\n  \"app\": \"BackMyCodes\",\n  \"exportedAt\": \"2026-10-14T"));
    assert!(text.contains("\"scheme\": \"ML-KEM-1024 + AES-256-GCM + scrypt(N=131072,r=8,p=1)\",\n  \"config\": {\n    \"salt\": \""));
    assert!(text.contains("\"scrypt\": {\n      \"N\": 131072,\n      \"r\": 8,\n      \"p\": 1\n    }\n  },\n  \"vault\": {\n    \"kemCT\": \""));
    assert!(text.ends_with("\"\n  }\n}"));
    assert!(!text.contains("recovery"));
    let file: Value = serde_json::from_str(&text).unwrap();
    let length = |outer: &str, key: &str| base64::engine::general_purpose::STANDARD.decode(file[outer][key].as_str().unwrap()).unwrap().len();
    assert_eq!(
        (length("config", "salt"), length("config", "skNonce"), length("config", "skEnc"), length("config", "publicKey"), length("vault", "kemCT"), length("vault", "nonce")),
        (32, 12, 3184, 1568, 1568, 12)
    );

    let read = read_export(text.as_bytes(), PASSWORD).unwrap();
    assert!(!read.plain);
    assert_eq!(serde_json::to_value(&read.accounts).unwrap(), serde_json::to_value(&accounts).unwrap());
    assert_eq!(serde_json::to_value(&read.services).unwrap(), serde_json::to_value(&services).unwrap());
    assert_eq!(read_export(text.as_bytes(), "not the password").unwrap_err(), BmcError::BadPassword);
    assert_eq!(read_export(text.as_bytes(), "").unwrap_err(), BmcError::BadPassword);
    assert_eq!(write_export(&accounts, &services, "short", NOW).unwrap_err(), BmcError::WeakPassword);
}

#[test]
fn version_1_round_trips() {
    let accounts = sample_accounts();
    let text = write_export_v1(&accounts, PASSWORD, NOW).unwrap();
    assert!(text.starts_with("{\n  \"version\": 1,"));
    let read = read_export(text.as_bytes(), PASSWORD).unwrap();
    assert_eq!(read.accounts.len(), 3);
    assert!(read.services.is_empty());
}

#[test]
fn the_recovery_kit_is_ignored_and_the_file_numbers_are_not_read() {
    let accounts = sample_accounts();
    let mut file: Value = serde_json::from_str(&write_export(&accounts, &[], PASSWORD, NOW).unwrap()).unwrap();
    file["recovery"] = json!({"kitId": "x", "threshold": 2, "shares": 3, "nonce": "AAAA", "ct": "AAAA"});
    // A file's own numbers can never ask for more memory, or change anything: they are not read
    file["config"]["scrypt"] = json!({"N": 1u64 << 40, "r": 64, "p": 64});
    file["scheme"] = json!("anything but the word for plain");
    file["config"]["publicKey"] = json!("");
    let read = read_export(file.to_string().as_bytes(), PASSWORD).unwrap();
    assert_eq!(read.accounts.len(), 3);
}

#[test]
fn what_an_export_refuses() {
    assert_eq!(read_export(b"not json", PASSWORD).unwrap_err(), BmcError::NotJson);
    assert_eq!(read_export(br#"{"app":"Other"}"#, PASSWORD).unwrap_err(), BmcError::NotAnExport);
    assert_eq!(read_export(br#"[1]"#, PASSWORD).unwrap_err(), BmcError::NotAnExport);
    assert_eq!(read_export(br#"{"app":"BackMyCodes"}"#, PASSWORD).unwrap_err(), BmcError::NotAnExport);
    assert_eq!(read_export(br#"{"app":"BackMyCodes","version":"2"}"#, PASSWORD).unwrap_err(), BmcError::NotAnExport);
    for version in ["3", "0", "1.5", "-1"] {
        let text = format!(r#"{{"app":"BackMyCodes","version":{version}}}"#);
        assert_eq!(read_export(text.as_bytes(), PASSWORD).unwrap_err(), BmcError::UnsupportedVersion, "{version}");
    }
    // Missing pieces and the wrong sizes read as a changed file
    assert_eq!(read_export(br#"{"app":"BackMyCodes","version":2}"#, PASSWORD).unwrap_err(), BmcError::BadPassword);
    assert_eq!(read_export(&vec![b' '; 17 * 1024 * 1024], PASSWORD).unwrap_err(), BmcError::TooLarge);
}

#[test]
fn plain_files_are_read_with_their_warning() {
    assert_eq!(export_is_encrypted(br#"{"app":"BackMyCodes","scheme":"plaintext"}"#), Some(false));
    assert_eq!(export_is_encrypted(br#"{"app":"Other"}"#), None);
    let read = read_export(br#"{"app":"BackMyCodes","scheme":"plaintext"}"#, "").unwrap();
    assert!(read.plain);
    assert_eq!((read.accounts.len(), read.services.len()), (0, 0));
    let text = r#"{"app":"BackMyCodes","scheme":"plaintext","version":9,
        "accounts":[
            {"id":"x","type":"steam","issuer":"S","label":"l","secret":"MZXW6","algorithm":"SHA1","digits":30.0,"period":"odd","addedAt":5,"favourite":{"a":[1]},"deletedAt":6},
            7,
            {"id":"y","secret":"","addedAt":1},
            {"id":"z","secret":"MZXW6","period":0},
            {"id":"w","type":"other","issuer":5,"secret":"MZXW6","tags":["a",1,"b"],"pinned":"yes"}
        ],
        "backupServices":[
            {"id":"s","serviceName":"N","codes":[{"code":"c","used":1,"usedAt":9,"k":true},{"code":"d","used":true}],"addedAt":2},
            "no",
            {"id":"t"}
        ]}"#;
    let read = read_export(text.as_bytes(), "").unwrap();
    assert_eq!(read.accounts.len(), 2);
    // A secret of no bytes and a period of 0 are refused at the door; what is not an object is dropped without a count
    assert_eq!(read.skipped, 2);
    let a = &read.accounts[0];
    assert_eq!((a.kind, a.digits, a.period, a.deleted_at), (Some(OtpKind::Steam), 30, 30, None));
    assert_eq!(a.unknown.0.keys().collect::<Vec<_>>(), ["favourite"]);
    let b = &read.accounts[1];
    assert_eq!((b.kind, b.issuer.as_str(), b.pinned), (None, "", None));
    assert_eq!(b.tags.as_deref().unwrap(), ["a".to_string(), "b".to_string()]);
    let s = &read.services[0];
    assert_eq!((s.codes[0].used, s.codes[1].used), (false, true));
    assert_eq!(s.codes[0].unknown.0.len(), 1);
    assert_eq!(read.services[1].codes.len(), 0);
}

#[test]
fn times_and_names() {
    assert_eq!(iso_time(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(iso_time(1_700_000_000_000), "2023-11-14T22:13:20.000Z");
    assert_eq!(iso_time(951_782_400_123), "2000-02-29T00:00:00.123Z");
    assert_eq!(iso_time(-1), "1969-12-31T23:59:59.999Z");
    assert_eq!(export_file_name(1_700_000_000_000), "backmycodes-export-2023-11-14T22-13-20.json");
}

#[test]
fn a_chunk_fits_a_version_40_code_at_level_l() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let draws = |text: &str| {
        let Ok(mut child) = Command::new("qrencode").args(["-l", "L", "-8", "-o", "/dev/null"]).stdin(Stdio::piped()).stderr(Stdio::null()).spawn() else {
            return None;
        };
        child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
        Some(child.wait().unwrap().success())
    };
    let full = format!("BMC1:01/02:{}", "A".repeat(2731));
    assert_eq!(full.len(), 2742);
    if need("qrencode") && let Some(drawn) = draws(&full) {
        assert!(drawn);
        assert_eq!(draws(&"A".repeat(2954)), Some(false));
    }
}
