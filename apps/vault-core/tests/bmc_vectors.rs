//! BackMyCodes' own export, QR code and crypto values, when this checkout has the local copy (never in the repository):
//! without it, each test prints SKIPPED where it is seen (`vectors()` in items_support), as the image build's checkout
//! has none. The export's payload is compared as bytes, key order included.

mod items_support;
use items_support::*;

use amethystora_vault_core::bmc::{Collector, gcm_open, read_export};
use serde_json::Value;

#[global_allocator]
static COUNTING: Counting = Counting;

fn check_expected(data: &Value, accounts: &[amethystora_vault_core::items::Account], services: &[amethystora_vault_core::items::BackupService], keep_ids: bool) {
    let expected = &data["legacy"]["expected"];
    let want_accounts = expected["accounts"].as_array().unwrap();
    assert_eq!(accounts.len(), want_accounts.len());
    for (got, want) in accounts.iter().zip(want_accounts) {
        assert_eq!(got.issuer, want["issuer"].as_str().unwrap());
        assert_eq!(got.label, want["label"].as_str().unwrap());
        assert_eq!(got.secret.as_str(), want["secret"].as_str().unwrap());
        assert_eq!(got.algorithm, want["algorithm"].as_str().unwrap());
        assert_eq!(got.digits, want["digits"].as_i64().unwrap());
        assert_eq!(got.period, want["period"].as_i64().unwrap());
        if keep_ids {
            assert_eq!(got.id, want["id"].as_str().unwrap());
            assert_eq!(got.added_at, want["addedAt"].as_i64().unwrap());
        }
    }
    let want_services = expected["backupServices"].as_array().unwrap();
    assert_eq!(services.len(), want_services.len());
    for (got, want) in services.iter().zip(want_services) {
        assert_eq!(got.service_name, want["serviceName"].as_str().unwrap());
        let want_codes = want["codes"].as_array().unwrap();
        assert_eq!(got.codes.len(), want_codes.len());
        for (code, want_code) in got.codes.iter().zip(want_codes) {
            assert_eq!(code.code.as_str(), want_code["code"].as_str().unwrap());
            assert_eq!(code.used, want_code["used"].as_bool().unwrap());
        }
    }
}

#[test]
fn the_export() {
    let Some(data) = vectors() else { return };
    let text = data["legacy"]["fileExport"].as_str().unwrap();
    let password = data["legacy"]["passwords"]["fileExport"].as_str().unwrap();
    let read = read_export(text.as_bytes(), password).unwrap();
    assert!(!read.plain);
    assert_eq!(read.skipped, 0);
    check_expected(&data, &read.accounts, &read.services, true);
    // Its payload writes back to the same bytes, the JSON of section 2, key order and all
    let written = format!(
        r#"{{"accounts":{},"backupServices":{}}}"#,
        serde_json::to_string(&read.accounts).unwrap(),
        serde_json::to_string(&read.services).unwrap()
    );
    assert_eq!(written, vector_bytes("expected").unwrap());
    assert_eq!(read_export(text.as_bytes(), "wrong").unwrap_err(), amethystora_vault_core::bmc::BmcError::BadPassword);
}

#[test]
fn the_qr_code() {
    let Some(data) = vectors() else { return };
    let password = data["legacy"]["passwords"]["qrExport"].as_str().unwrap();
    let mut collector = Collector::new();
    for chunk in data["legacy"]["qrExport"].as_array().unwrap() {
        collector.add(chunk.as_str().unwrap());
    }
    let read = collector.read(password, 1_800_000_000_000).unwrap();
    check_expected(&data, &read.accounts, &read.services, false);
    assert!(read.accounts.iter().all(|a| a.added_at == 1_800_000_000_000 && amethystora_vault_core::items::is_id(&a.id)));
}

#[test]
fn crypto() {
    let Some(data) = vectors() else { return };
    for case in data["crypto"]["aes"].as_array().unwrap() {
        let key = unhex(case["key"].as_str().unwrap());
        let nonce = unhex(case["nonce"].as_str().unwrap());
        let ct = unhex(case["ct"].as_str().unwrap());
        let plain = gcm_open(&key, &nonce, &ct).unwrap();
        assert_eq!(hex(&plain), case["plaintext"].as_str().unwrap());
    }
    for case in data["crypto"]["pbkdf2"].as_array().unwrap() {
        let mut out = vec![0u8; case["length"].as_u64().unwrap() as usize];
        openssl::pkcs5::pbkdf2_hmac(
            case["password"].as_str().unwrap().as_bytes(),
            &unhex(case["salt"].as_str().unwrap()),
            case["iterations"].as_u64().unwrap() as usize,
            openssl::hash::MessageDigest::sha256(),
            &mut out,
        )
        .unwrap();
        assert_eq!(hex(&out), case["output"].as_str().unwrap());
    }
    for case in data["crypto"]["scrypt"].as_array().unwrap() {
        let n = case["N"].as_u64().unwrap();
        let params = scrypt::Params::new(n.trailing_zeros() as u8, case["r"].as_u64().unwrap() as u32, case["p"].as_u64().unwrap() as u32).unwrap();
        let mut out = vec![0u8; case["dkLen"].as_u64().unwrap() as usize];
        scrypt::scrypt(case["password"].as_str().unwrap().as_bytes(), &unhex(case["salt"].as_str().unwrap()), &params, &mut out).unwrap();
        assert_eq!(hex(&out), case["output"].as_str().unwrap());
    }
}
