//! What the review of 2026-11-02 found (plans/vault/work/review-items.md), each held by a test that failed before its fix.

mod items_support;
use items_support::*;

use amethystora_vault_core::bmc::{self, Added, Collector, read_export};
use amethystora_vault_core::codes::{parse_codes, parse_document_codes};
use amethystora_vault_core::generate::words;
use amethystora_vault_core::import::{ImportError, read_file, read_foreign};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use std::time::Instant;

#[global_allocator]
static COUNTING: Counting = Counting;

const KEY: &str = "ORSXG5DTMVRXEZLU";
const MIB: usize = 1024 * 1024;
const PASSWORD: &str = "a long enough password";

fn plain_export(accounts: Value, services: Value) -> Vec<u8> {
    json!({"app": "BackMyCodes", "scheme": "plaintext", "accounts": accounts, "backupServices": services}).to_string().into_bytes()
}

fn transfer(payload: &str) -> Collector {
    let mut collector = Collector::new();
    assert_eq!(collector.add(&format!("BMCP:01/01:{}", URL_SAFE_NO_PAD.encode(payload))), Added::New);
    collector
}

// Finding 1

#[test]
fn many_groups_on_one_entry_cost_little() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let count = 40_000;
    let groups: Vec<Value> = (0..count).map(|i| json!({"uuid": format!("g{i}"), "name": format!("group {i}")})).collect();
    let ids: Vec<String> = (0..count).map(|i| format!("g{i}")).collect();
    let file = json!({"header": {}, "db": {"groups": groups, "entries": [{"type": "totp", "groups": ids, "info": {"secret": KEY}}]}}).to_string();
    let start = Instant::now();
    let read = read_foreign(file.as_bytes(), "").unwrap();
    assert!(start.elapsed().as_secs_f64() < 2.0, "{:?}", start.elapsed());
    // Past the bound on tags: refused and counted, never cut
    assert_eq!((read.accounts.len(), read.skipped), (0, 1));
}

// Finding 3

#[test]
fn a_file_counter_or_digits_out_of_range_is_refused() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    for (account, why) in [
        (json!({"type": "hotp", "secret": KEY, "counter": -5}), "a negative counter"),
        (json!({"type": "hotp", "secret": KEY, "counter": 1u64 << 60}), "a counter past 2^53 - 1"),
        (json!({"secret": KEY, "digits": 0}), "no digits"),
        (json!({"secret": KEY, "digits": -3}), "negative digits"),
        (json!({"secret": KEY, "digits": 100}), "too many digits"),
        (json!({"type": "hotp", "secret": KEY, "digits": 11}), "too many digits on HOTP"),
    ] {
        let read = read_export(&plain_export(json!([account]), json!([])), "").unwrap();
        assert_eq!((read.accounts.len(), read.skipped), (0, 1), "{why}");
    }
    // Steam's digits mean nothing, and what is in range stays
    let read = read_export(&plain_export(json!([{"type": "steam", "secret": KEY, "digits": 0}, {"secret": KEY, "digits": 10}, {"type": "hotp", "secret": KEY, "counter": 9007199254740991u64}]), json!([])), "").unwrap();
    assert_eq!((read.accounts.len(), read.skipped), (3, 0));
}

#[test]
fn a_transfer_counter_out_of_range_is_refused_not_repaired() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let read = transfer(&format!(r#"{{"a":[{{"s":"{KEY}","t":"hotp","c":-5}}]}}"#)).read("", 1).unwrap();
    assert_eq!((read.accounts.len(), read.skipped), (0, 1));
    let read = transfer(&format!(r#"{{"a":[{{"s":"{KEY}","d":0}}]}}"#)).read("", 1).unwrap();
    assert_eq!((read.accounts.len(), read.skipped), (0, 1));
}

#[test]
fn digits_out_of_range_show_dashes() {
    use amethystora_vault_core::items::{Account, Secret};
    use amethystora_vault_core::otp::code_at;
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    for digits in [0, -3, 11, 100] {
        let account = Account { secret: Secret::new(KEY), digits, ..Account::default() };
        assert_eq!(code_at(&account, 59, 0).as_str(), "------", "{digits}");
    }
}

// Finding 4

#[test]
fn no_field_or_entry_is_past_what_one_item_carries() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    // One byte past each bound, so that the bound itself is what refuses, not the size of the whole entry
    for account in [
        json!({"secret": KEY, "algorithm": "x".repeat(33)}),
        json!({"secret": KEY, "spaceId": "x".repeat(257)}),
        json!({"secret": KEY, "id": "x".repeat(257)}),
        json!({"secret": KEY, "issuer": "x".repeat(1025)}),
        // Only 1024 characters make a code
        json!({"secret": "A".repeat(1025)}),
    ] {
        let read = read_export(&plain_export(json!([account]), json!([])), "").unwrap();
        assert_eq!((read.accounts.len(), read.skipped), (0, 1), "{account}");
    }
    for service in [json!({"serviceName": "s", "spaceId": "x".repeat(257)}), json!({"serviceName": "s", "id": "x".repeat(257)}), json!({"serviceName": "s", "account": "x".repeat(1025)})] {
        let read = read_export(&plain_export(json!([]), json!([service])), "").unwrap();
        assert_eq!((read.services.len(), read.skipped), (0, 1), "{service}");
    }
    // And at each bound they stay
    let read = read_export(&plain_export(json!([{"secret": "A".repeat(1024), "algorithm": "x".repeat(32), "spaceId": "x".repeat(256), "id": "x".repeat(256)}]), json!([{"serviceName": "s", "spaceId": "x".repeat(256), "id": "x".repeat(256), "account": "x".repeat(1024)}])), "").unwrap();
    assert_eq!((read.accounts.len(), read.services.len(), read.skipped), (1, 1, 0));
    // Two MiB of anything is past the whole item too
    let long = "x".repeat(2 * MIB);
    let read = read_export(&plain_export(json!([{"secret": KEY, "algorithm": long}]), json!([])), "").unwrap();
    assert_eq!((read.accounts.len(), read.skipped), (0, 1));
    // Every field in its bound and the whole past it: refused too, and counted in the preview
    let code = |i: usize| json!({"code": format!("{i:0>256}"), "used": false});
    let huge: Vec<Value> = (0..4096).map(code).collect();
    let read = read_export(&plain_export(json!([]), json!([{"serviceName": "s", "codes": huge}])), "").unwrap();
    assert_eq!((read.services.len(), read.skipped), (0, 1));
}

// Finding 5

#[test]
fn colons_and_lines_take_no_memory_before_their_bound() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let file = json!({"services": [], "schemaVersion": 4, "servicesEncrypted": ":".repeat(15 * MIB)}).to_string();
    let (read, peak, _) = measured(|| read_foreign(file.as_bytes(), "pw"));
    assert_eq!(read.unwrap_err(), ImportError::Unsupported);
    assert!(peak < 64 * MIB, "{peak}");
    let lines = "a\n".repeat(8 * MIB - 1);
    let (read, peak, _) = measured(|| read_foreign(lines.as_bytes(), ""));
    assert!(read.is_err());
    assert!(peak < 32 * MIB, "{peak}");
}

// Finding 6

#[test]
fn slots_share_one_budget_of_time() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let slot = json!({"type": 1, "n": 1u64 << 18, "r": 8, "p": 1, "salt": "00", "key": "00", "key_params": {"nonce": "00", "tag": "00"}});
    let file = json!({"header": {"slots": vec![slot; 16], "params": {"nonce": "00", "tag": "00"}}, "db": "AAAA"}).to_string();
    assert!(file.len() < 2048);
    let start = Instant::now();
    assert_eq!(read_foreign(file.as_bytes(), "pw").unwrap_err(), ImportError::TooMuchMemory);
    // One scrypt at the cap takes about a second: sixteen of them took 18
    assert!(start.elapsed().as_secs_f64() < 6.0, "{:?}", start.elapsed());
}

// Finding 7

#[test]
fn a_marker_with_no_space_after_it_goes_too() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    fn text(parsed: amethystora_vault_core::codes::ParsedCodes) -> Vec<String> {
        parsed.codes.iter().map(|c| c.as_str().to_string()).collect()
    }
    for (pasted, code) in [("•abcd-1234", "abcd-1234"), ("1.abcd-1234", "abcd-1234"), ("1)abcd-1234", "abcd-1234"), ("12)5678-abcd", "5678-abcd"), ("*abcd-1234", "abcd-1234"), ("–abcd-1234", "abcd-1234")] {
        assert_eq!(text(parse_codes(pasted)), [code], "{pasted}");
        assert_eq!(text(parse_document_codes(pasted)), [code], "{pasted}");
    }
    // These keep what they start with, each by its own rule
    assert_eq!(text(parse_codes("-abc\n1.5abc\n1234.abc\n")), ["-abc", "1.5abc", "1234.abc"]);
}

// Finding 8

#[test]
fn a_code_past_its_bound_refuses_the_service() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let long = "c".repeat(300);
    let read = read_export(&plain_export(json!([]), json!([{"serviceName": "s", "codes": [{"code": "ok", "used": false}, {"code": long, "used": false}]}])), "").unwrap();
    assert_eq!((read.services.len(), read.skipped), (0, 1));
    let read = transfer(&format!(r#"{{"b":[{{"n":"s","c":[{{"code":"ok","used":false}},{{"code":"{long}","used":false}}]}}]}}"#)).read("", 1).unwrap();
    assert_eq!((read.services.len(), read.skipped), (0, 1));
}

// Finding 9

#[test]
fn a_later_version_says_so() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let error = read_file(br#"{"app":"BackMyCodes","version":3}"#, PASSWORD).unwrap_err();
    assert_eq!(format!("{error:?}"), "UnsupportedVersion");
}

// Finding 10

#[test]
fn reading_a_file_holds_one_tree() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let accounts: Vec<Value> = (0..30_000).map(|i| json!({"id": format!("{i}"), "issuer": "x", "secret": KEY})).collect();
    let file = plain_export(json!(accounts), json!([]));
    let (read, alone, _) = measured(|| read_export(&file, "").unwrap());
    drop(read);
    let (read, through_import, _) = measured(|| read_file(&file, "").unwrap());
    assert_eq!(read.accounts.len(), 30_000);
    // The file's text once more is all it may add
    assert!(through_import < alone + file.len() + MIB, "{through_import} against {alone}, the file being {}", file.len());
}

// Not confirmed by the review

#[test]
fn an_opened_payload_that_cannot_be_read_is_not_a_wrong_password() {
    use amethystora_vault_core::bmc::BmcError;
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let not_json: &[&str] = &[r#"{"accounts":[{"note":"\ud800"}]}"#, r#"{"accounts":[{"digits":1e400}]}"#, "not json at all"];
    for payload in not_json {
        // A BMC1, sealed with the right password
        let (salt, iv) = ([7u8; 16], [9u8; 12]);
        let mut key = [0u8; 32];
        openssl::pkcs5::pbkdf2_hmac(PASSWORD.as_bytes(), &salt, 200_000, openssl::hash::MessageDigest::sha256(), &mut key).unwrap();
        use std::io::Write;
        let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::new(6));
        encoder.write_all(payload.as_bytes()).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut tag = [0u8; 16];
        let ct = openssl::symm::encrypt_aead(openssl::symm::Cipher::aes_256_gcm(), &key, Some(&iv), &[], &compressed, &mut tag).unwrap();
        let blob = [&salt[..], &iv[..], &ct[..], &tag[..]].concat();
        let mut collector = Collector::new();
        assert_eq!(collector.add(&format!("BMC1:01/01:{}", URL_SAFE_NO_PAD.encode(blob))), Added::New);
        assert_ne!(collector.read(PASSWORD, 1).unwrap_err(), BmcError::BadPassword, "{payload}");

        // An encrypted export, sealed the same way: a throwaway key, wrapped under the real scrypt
        let text = sealed_export(payload.as_bytes());
        assert_ne!(read_export(text.as_bytes(), PASSWORD).unwrap_err(), BmcError::BadPassword, "{payload}");
        assert_ne!(read_file(text.as_bytes(), PASSWORD).unwrap_err(), ImportError::BadPassword, "{payload}");
    }
}

/// An export whose payload is whatever a test says, from a real one's keys: it opens an export written by this crate for
/// its config, then seals the payload to its public key by the format's own steps.
fn sealed_export(payload: &[u8]) -> String {
    let real = bmc::write_export(&[], &[], PASSWORD, 1).unwrap();
    let mut file: Value = serde_json::from_str(&real).unwrap();
    let public = STANDARD.decode(file["config"]["publicKey"].as_str().unwrap()).unwrap();
    let (kem_ct, shared) = encapsulate(&public);
    let nonce = [5u8; 12];
    let mut tag = [0u8; 16];
    let mut ct = openssl::symm::encrypt_aead(openssl::symm::Cipher::aes_256_gcm(), &shared, Some(&nonce), &[], payload, &mut tag).unwrap();
    ct.extend_from_slice(&tag);
    file["vault"] = json!({"kemCT": STANDARD.encode(kem_ct), "nonce": STANDARD.encode(nonce), "ct": STANDARD.encode(ct)});
    file.to_string()
}

fn encapsulate(public: &[u8]) -> (Vec<u8>, [u8; 32]) {
    use foreign_types::ForeignTypeRef;
    use openssl::pkey::{KeyType, PKey};
    let key = PKey::public_key_from_raw_bytes_ex(None, KeyType::ML_KEM_1024, None, public).unwrap();
    let ctx = openssl::pkey_ctx::PkeyCtx::new(&key).unwrap();
    let (mut ct, mut shared) = (vec![0u8; 1568], [0u8; 32]);
    let (mut ct_len, mut shared_len) = (ct.len(), shared.len());
    // Safety: the context lives past both calls, and both buffers are as long as the lengths given with them
    unsafe {
        assert!(openssl_sys::EVP_PKEY_encapsulate_init(ctx.as_ptr(), std::ptr::null()) > 0);
        assert!(openssl_sys::EVP_PKEY_encapsulate(ctx.as_ptr(), ct.as_mut_ptr(), &mut ct_len, shared.as_mut_ptr(), &mut shared_len) > 0);
    }
    (ct, shared)
}

#[test]
fn a_list_of_accounts_finds_its_services_cheaply() {
    use amethystora_vault_core::codes::{ServiceIndex, service_for};
    use amethystora_vault_core::items::{Account, BackupService};
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let services: Vec<BackupService> = (0..1000).map(|i| BackupService { service_name: format!("Service number {i}, Inc."), ..BackupService::default() }).collect();
    let accounts: Vec<Account> = (0..1000).map(|i| Account { issuer: format!("service number {}", i * 7 % 1500), ..Account::default() }).collect();
    let start = Instant::now();
    let index = ServiceIndex::new(&services);
    let found = accounts.iter().filter(|a| service_for(a, &index).is_some()).count();
    assert_eq!(found, accounts.iter().filter(|a| a.issuer.trim_start_matches("service number ").parse::<usize>().unwrap() < 1000).count());
    // Built once, then a lookup per row: a debug build's own time, where building it per row took many seconds
    assert!(start.elapsed().as_secs_f64() < 1.5, "{:?}", start.elapsed());
}

#[test]
fn words_reserve_what_they_need() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    // A growing buffer would leave a copy behind: with a separator of four bytes, the old guess always grew
    for _ in 0..200 {
        let (made, _, allocations) = measured(|| words(20, '😀').unwrap());
        assert_eq!(made.password.as_str().matches('😀').count(), 19);
        assert_eq!(allocations, 1, "one buffer, which becomes the secret and is never grown");
    }
}
