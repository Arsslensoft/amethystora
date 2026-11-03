//! Every import file and Google link of BackMyCodes' test data, when this checkout has the local copy (never in the
//! repository): without it, each test says that it skipped. A case may differ only for one of the reasons below, each
//! printed with its count: section 16 or the Coordinator's decisions, among them a file holding a number past a double
//! (`1e400`) being refused whole, decided by the coordinator on 2026-11-02.

mod items_support;
use items_support::*;

use amethystora_vault_core::import::{self, Detected, ImportError, Source, read_foreign, read_migration};
use serde_json::Value;
use std::collections::BTreeMap;

#[global_allocator]
static COUNTING: Counting = Counting;

fn source_name(source: Source) -> &'static str {
    match source {
        Source::Aegis => "aegis",
        Source::TwoFas => "2fas",
        Source::Bitwarden => "bitwarden",
        Source::Google => "google",
        Source::Links => "uris",
        Source::BackMyCodes => "backmycodes",
    }
}

fn detected_json(detected: Option<Detected>) -> Value {
    match detected {
        Some(d) if d.source != Source::BackMyCodes => serde_json::json!({"source": source_name(d.source), "encrypted": d.encrypted}),
        _ => Value::Null,
    }
}

fn error_name(error: ImportError) -> &'static str {
    match error {
        ImportError::Unsupported => "unsupported",
        ImportError::Empty => "empty",
        ImportError::PasswordRequired => "password-required",
        ImportError::BadPassword => "bad-password",
        ImportError::BitwardenEncrypted => "bitwarden-encrypted",
        ImportError::TooMuchMemory => "too-much-memory",
        ImportError::UnsupportedVersion => "unsupported-version",
        ImportError::TooLarge => "too-large",
        ImportError::Crypto => "crypto",
    }
}

/// What differs between two lists of accounts, for the reasons given, and how: the entries only one side holds
fn differing<'a>(want: &'a [Value], got: &'a [Value]) -> (Vec<&'a Value>, Vec<&'a Value>) {
    let only_want = want.iter().filter(|a| !got.contains(a)).collect();
    let only_got = got.iter().filter(|a| !want.contains(a)).collect();
    (only_want, only_got)
}

fn explain_accounts(want: &[Value], got: &[Value], want_skipped: i64, got_skipped: i64) -> Option<&'static str> {
    let (only_want, only_got) = differing(want, got);
    let names_224_384 = |a: &&Value| matches!(a["algorithm"].as_str(), Some("SHA224" | "SHA384"));
    // An account BackMyCodes skips for its hash, and this reads
    if only_want.is_empty() && !only_got.is_empty() && only_got.iter().all(names_224_384) && want_skipped - got_skipped == only_got.len() as i64 {
        return Some("decision 2: SHA-224 and SHA-384 are accepted, so these are read rather than skipped");
    }
    // An account that holds an empty label from a link `ISSUER:`
    if !only_want.is_empty()
        && only_want.len() == only_got.len()
        && only_want.iter().all(|w| {
            only_got.iter().any(|g| {
                let mut with_colon = (*g).clone();
                with_colon["label"] = Value::String(format!("{}:", g["issuer"].as_str().unwrap_or("")));
                g["label"] == "" && with_colon == **w
            })
        })
    {
        return Some("decision 5: a label `ISSUER:` for its own issuer reads as empty");
    }
    // Refused at the door, where BackMyCodes keeps it
    if only_got.is_empty() && got_skipped - want_skipped == only_want.len() as i64 {
        let no_bytes = |a: &&Value| a["secret"] == "";
        if only_want.iter().all(no_bytes) {
            return Some("section 16.18: a secret of no bytes is refused");
        }
    }
    None
}

#[test]
fn files() {
    let Some(data) = vectors() else { return };
    let mut differences: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut unexplained = Vec::new();
    for case in data["importCases"]["files"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut text = case["text"].as_str().unwrap().to_string();
        let password = case.get("password").and_then(Value::as_str).unwrap_or("");
        // A number past a double is no JSON to serde_json, where JavaScript makes it Infinity: that whole file is refused.
        // The rest of such a file is still checked, with the largest double in its place, which every rule treats alike.
        if text.contains("1e400") || text.contains("1e999") {
            assert_eq!(read_foreign(text.as_bytes(), password).unwrap_err(), ImportError::Unsupported, "{name}");
            differences.entry("decided: a number past a double, such as 1e400, refuses the whole file rather than one entry").or_default().push(name.to_string());
            text = text.replace("1e400", "1.7976931348623157e308").replace("1e999", "1.7976931348623157e308");
        }
        let text = text.as_str();
        let detected = detected_json(import::detect(text.as_bytes()));
        if detected != case["detected"] {
            unexplained.push(format!("{name}: detected {detected}, want {}", case["detected"]));
        }
        match (read_foreign(text.as_bytes(), password), case.get("error").and_then(Value::as_str)) {
            (Ok(read), None) => {
                let got: Vec<Value> = read.accounts.iter().map(account_json).collect();
                let want = case["result"]["accounts"].as_array().unwrap();
                let (want_skipped, got_skipped) = (case["result"]["skipped"].as_i64().unwrap(), read.skipped as i64);
                if got != *want || got_skipped != want_skipped {
                    match explain_accounts(want, &got, want_skipped, got_skipped) {
                        Some(why) => differences.entry(why).or_default().push(name.to_string()),
                        None => unexplained.push(format!("{name}:\n  want {want:?} skipped {want_skipped}\n  got  {got:?} skipped {got_skipped}")),
                    }
                }
                assert_eq!(source_name(read.source), case["result"]["source"].as_str().unwrap(), "{name}");
            }
            (Err(got), Some(want)) if error_name(got) == want => {}
            (Err(ImportError::TooMuchMemory), Some("bad-password")) => {
                differences.entry("decision 4: scrypt past 256 MiB is refused in its own words, not as a wrong password").or_default().push(name.to_string());
            }
            (got, want) => unexplained.push(format!("{name}: got {:?}, want {want:?} {}", got.map(|r| r.accounts.iter().map(account_json).collect::<Vec<_>>()), case["result"])),
        }
    }
    for (why, cases) in &differences {
        eprintln!("import files: {} differ on purpose: {why}", cases.len());
        cases.iter().for_each(|c| eprintln!("    {c}"));
    }
    assert!(unexplained.is_empty(), "{} unexplained:\n{}", unexplained.len(), unexplained.join("\n"));
}

#[test]
fn google_links() {
    let Some(data) = vectors() else { return };
    let mut differences: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut unexplained = Vec::new();
    for case in data["importCases"]["migrations"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let uri = case["uri"].as_str().unwrap();
        let got = read_migration(uri);
        let want = &case["result"];
        let same = match (&got, want) {
            (None, Value::Null) => true,
            (Some((accounts, skipped)), Value::Object(_)) => {
                let list: Vec<Value> = accounts.iter().map(account_json).collect();
                list == *want["accounts"].as_array().unwrap() && *skipped as i64 == want["skipped"].as_i64().unwrap()
            }
            _ => false,
        };
        if same {
            continue;
        }
        let why = if name.starts_with("a URL like") && got.is_none() {
            "section 10.2: only otpauth-migration://offline? is taken, and BackMyCodes' URL parser let this one through"
        } else if name.starts_with("a key past 2^53") || name == "a field number past 2^32" {
            "section 10.2: a varint is read as a u64, not added up in doubles"
        } else {
            unexplained.push(format!("{name}: {uri}\n  want {want}\n  got  {:?}", got.map(|(a, s)| (a.iter().map(account_json).collect::<Vec<_>>(), s))));
            continue;
        };
        differences.entry(why).or_default().push(name.to_string());
    }
    for (why, cases) in &differences {
        eprintln!("Google links: {} differ on purpose: {why}", cases.len());
        cases.iter().for_each(|c| eprintln!("    {c}"));
    }
    assert!(unexplained.is_empty(), "{} unexplained:\n{}", unexplained.len(), unexplained.join("\n"));
}

#[test]
fn the_whole_encrypted_files() {
    let Some(data) = vectors() else { return };
    let aegis = data["importEncrypted"]["aegis"].as_str().unwrap();
    for password in ["correct horse", "backup battery"] {
        let read = read_foreign(aegis.as_bytes(), password).unwrap();
        assert_eq!((read.accounts.len(), read.skipped), (4, 5));
        assert_eq!(read.accounts[0].issuer, "GitHub");
        assert_eq!(read.accounts[0].tags.as_deref().unwrap(), ["Work".to_string(), "Dev".to_string()]);
        assert_eq!(read.accounts[2].kind, Some(amethystora_vault_core::items::OtpKind::Steam));
    }
    assert_eq!(read_foreign(aegis.as_bytes(), "wrong").unwrap_err(), ImportError::BadPassword);
    let two_fas = data["importEncrypted"]["twoFas"].as_str().unwrap();
    let read = read_foreign(two_fas.as_bytes(), "staple").unwrap();
    assert_eq!((read.accounts.len(), read.skipped), (4, 1));
    assert_eq!(read.accounts[0].issuer, "My bank");
    assert_eq!(read.accounts[1].tags.as_deref().unwrap(), ["Work".to_string()]);
}
