//! Every link and code case of BackMyCodes' test data, when this checkout has the local copy (plans/vault-bmc/vectors.json,
//! never in the repository): without it, each test says that it skipped. A case may differ only for one of the reasons
//! below, each printed with its count: section 16 or the Coordinator's decisions; `digits: 0` showing dashes, decided by
//! the coordinator on 2026-11-02; and a lone surrogate, which no Rust string can hold.

mod items_support;
use items_support::*;

use amethystora_vault_core::items::{Account, OtpKind, Secret};
use amethystora_vault_core::otp::{self, code_at};
use amethystora_vault_core::otpauth::{read_link, write_link};
use serde_json::Value;
use std::collections::BTreeMap;

#[global_allocator]
static COUNTING: Counting = Counting;

fn stored_account(v: &Value) -> Account {
    let kind = match v.get("type").and_then(Value::as_str) {
        Some("hotp") => Some(OtpKind::Hotp),
        Some("steam") => Some(OtpKind::Steam),
        Some("totp") => Some(OtpKind::Totp),
        _ => None,
    };
    Account {
        kind,
        issuer: v.get("issuer").and_then(Value::as_str).unwrap_or("").to_string(),
        label: v.get("label").and_then(Value::as_str).unwrap_or("").to_string(),
        secret: Secret::new(v["secret"].as_str().unwrap()),
        algorithm: v["algorithm"].as_str().unwrap().to_string(),
        digits: v["digits"].as_i64().unwrap(),
        period: v["period"].as_i64().unwrap(),
        counter: v.get("counter").and_then(Value::as_i64),
        ..Account::default()
    }
}

fn report(what: &str, differences: BTreeMap<&str, Vec<String>>) {
    for (why, cases) in &differences {
        eprintln!("{what}: {} differ on purpose: {why}", cases.len());
        for case in cases {
            eprintln!("    {case}");
        }
    }
}

/// Why a link that BackMyCodes refuses is read here: SHA-224 and SHA-384 (decision 2)
fn names_224_or_384(uri: &str) -> bool {
    let lower = uri.to_ascii_lowercase();
    lower.contains("algorithm=sha224") || lower.contains("algorithm=sha384")
}

#[test]
fn reading_links() {
    let Some(data) = vectors() else { return };
    let mut differences: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut unexplained = Vec::new();
    for case in data["links"]["parse"].as_array().unwrap() {
        let uri = case["uri"].as_str().unwrap();
        let want = &case["account"];
        let got = read_link(uri).map(|a| account_json(&a)).unwrap_or(Value::Null);
        if got == *want {
            continue;
        }
        let why = if want.is_null() && names_224_or_384(uri) {
            "decision 2: SHA-224 and SHA-384 are accepted"
        } else if !want.is_null() && got.is_null() && want["secret"] == "" {
            "section 16.18: a secret of no bytes is refused"
        } else if !want.is_null() && got.is_null() && want["counter"].as_i64().is_some_and(|c| c < 0) {
            "section 16.19: a counter below 0 is refused"
        } else if !want.is_null() && got.is_null() && want["counter"].as_i64().is_some_and(|c| c > (1 << 53) - 1) {
            "section 16.19: a counter past 2^53 - 1 is refused"
        } else if !want.is_null() && !got.is_null() && {
            let mut with_colon = got.clone();
            with_colon["label"] = Value::String(format!("{}:", got["issuer"].as_str().unwrap_or("")));
            got["label"] == "" && with_colon == *want
        } {
            "decision 5: a label `ISSUER:` for its own issuer reads as empty"
        } else {
            unexplained.push(format!("{uri}\n  want {want}\n  got  {got}"));
            continue;
        };
        differences.entry(why).or_default().push(uri.to_string());
    }
    report("reading links", differences);
    assert!(unexplained.is_empty(), "{} unexplained:\n{}", unexplained.len(), unexplained.join("\n"));
}

#[test]
fn writing_links() {
    let Some(data) = vectors() else { return };
    let mut differences: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut unexplained = Vec::new();
    for case in data["links"]["build"].as_array().unwrap() {
        let account = stored_account(&case["account"]);
        let written = write_link(&account).map(|l| l.as_str().to_string());
        let want = case["uri"].as_str();
        if want.is_none() && written.is_some() && format!("{}{}", account.issuer, account.label).contains('\u{fffd}') {
            differences.entry("a lone surrogate, which JavaScript refuses to encode, cannot be in a Rust string").or_default().push(case["account"].to_string());
            continue;
        }
        if written.as_deref() != want {
            unexplained.push(format!("wrote {written:?}, want {want:?}"));
            continue;
        }
        let Some(uri) = want else { continue };
        let got = read_link(uri).map(|a| account_json(&a)).unwrap_or(Value::Null);
        let parsed = &case["parsed"];
        if got == *parsed {
            continue;
        }
        let why = if parsed.is_null() && names_224_or_384(uri) {
            "decision 2: SHA-224 and SHA-384 are accepted (reading back)"
        } else if !parsed.is_null() && !got.is_null() && got["label"] == "" && parsed["label"] == format!("{}:", got["issuer"].as_str().unwrap_or("")) {
            "decision 5: a label `ISSUER:` for its own issuer reads as empty (reading back)"
        } else if !parsed.is_null() && got.is_null() && parsed["secret"] == "" {
            "section 16.18: a secret of no bytes is refused (reading back)"
        } else if !parsed.is_null() && got.is_null() && parsed["counter"].as_i64().is_some_and(|c| c < 0) {
            "section 16.19: a counter below 0 is refused (reading back)"
        } else {
            unexplained.push(format!("{uri}\n  want {parsed}\n  got  {got}"));
            continue;
        };
        differences.entry(why).or_default().push(uri.to_string());
    }
    report("writing links", differences);
    assert!(unexplained.is_empty(), "{} unexplained:\n{}", unexplained.len(), unexplained.join("\n"));
}

#[test]
fn base32() {
    let Some(data) = vectors() else { return };
    for case in data["codes"]["base32"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let got = otp::base32_decode(input).map(|b| hex(&b));
        assert_eq!(got.as_deref(), case["bytes"].as_str(), "{input:?}");
        if let Some(canonical) = case.get("base32").and_then(Value::as_str) {
            assert_eq!(otp::canonical_secret(input).as_ref().map(Secret::as_str), Some(canonical), "{input:?}");
        }
    }
    for case in data["codes"]["encode"].as_array().unwrap() {
        assert_eq!(otp::base32_encode(&unhex(case["bytes"].as_str().unwrap())).as_str(), case["base32"].as_str().unwrap());
    }
}

#[test]
fn secrets_and_grouping() {
    let Some(data) = vectors() else { return };
    for case in data["codes"]["secrets"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        assert_eq!(otp::normalize_typed(input).as_str(), case["normalized"].as_str().unwrap(), "{input:?}");
        // The fixture's flag is on the raw text, as section 17 says
        assert_eq!(otp::typed_secret_is_valid(input), case["valid"].as_bool().unwrap(), "{input:?}");
        assert_eq!(otp::group_secret(input).as_str(), case["grouped"].as_str().unwrap(), "{input:?}");
    }
    for case in data["codes"]["format"].as_array().unwrap() {
        assert_eq!(otp::group_code(case["code"].as_str().unwrap()), case["formatted"].as_str().unwrap());
    }
}

#[test]
fn codes() {
    let Some(data) = vectors() else { return };
    let mut differences: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut unexplained = Vec::new();
    for case in data["codes"]["codes"].as_array().unwrap() {
        let account = stored_account(&case["account"]);
        let (now, offset) = (case["nowSec"].as_i64().unwrap(), case["offset"].as_i64().unwrap());
        let got = code_at(&account, now, offset);
        let want = case["code"].as_str().unwrap();
        if got.as_str() == want {
            continue;
        }
        let dashes = got.as_str().chars().all(|c| c == '-');
        let kind = account.otp_kind();
        let why = if account.digits == 0 && kind != OtpKind::Steam {
            "decided: `digits: 0` shows dashes, where BackMyCodes' arithmetic made `0`"
        } else if dashes && otp::holds_no_bytes(account.secret.as_str()) {
            "section 16.18: a secret of no bytes makes no code"
        } else if dashes && kind != OtpKind::Hotp && account.period <= 0 {
            "section 16.20: a period of 0 or below makes no code"
        } else {
            unexplained.push(format!("{case}: got {}", got.as_str()));
            continue;
        };
        differences.entry(why).or_default().push(case.to_string());
    }
    report("codes", differences);
    assert!(unexplained.is_empty(), "{} unexplained:\n{}", unexplained.len(), unexplained.join("\n"));
}
