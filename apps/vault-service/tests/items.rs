//! Items, keys and certificates through the service on a real socket, with the stand-ins and the real job process
//! (`amethystora-vault-work`): every op, each approval's way through, each refusal and hostile fields. plans/vault/work/ops.md
//! is the list. Each thing an account may not do is checked to leave no trace.

mod common;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use amethystora_vault_core::bmc;
use amethystora_vault_core::items::{Account, OtpKind, Secret};
use amethystora_vault_core::otp;
use amethystora_vault_core::protocol::encode;
use amethystora_vault_service::outside::{say, Action, Person};
use base64::Engine;
use common::{b64, files_under, start, why, Running};
use serde_json::{json, Value};

const PASSWORD: &str = "correct horse battery";
const SECRET: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";

fn opened(name: &str) -> Running {
    let running = start(name);
    running.set_up(PASSWORD);
    running
}

fn ok(answer: Value) -> Value {
    assert_eq!(answer["ok"], json!(true), "{answer}");
    answer
}

fn asked(running: &Running) -> Vec<(Action, &'static str, String)> {
    running.outside.said.lock().unwrap().asked.iter().map(|ask| (ask.action, ask.message, ask.item.clone())).collect()
}

fn refuse_prompts(running: &Running, refuse: bool) {
    running.outside.said.lock().unwrap().refuse = refuse;
}

fn decode(text: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD.decode(text.as_str().unwrap()).unwrap()
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

/// Everything on the disk, for looking for what must not be there
fn disk_holds(folder: &Path, text: &[u8]) -> bool {
    files_under(folder).iter().any(|file| std::fs::read(file).is_ok_and(|bytes| bytes.windows(text.len()).any(|window| window == text)))
}

fn add_account(running: &Running, issuer: &str) -> String {
    let answer = ok(running.ask(json!({"op": "auth-add", "account": {"issuer": issuer, "label": "ada", "secret": SECRET}})));
    assert_eq!(answer["added"], json!(true));
    answer["id"].as_str().unwrap().to_string()
}

// --- Every op, closed and never set up -------------------------------------------------------------------------------------

const NEEDS_OPEN: [(&str, &str); 28] = [
    ("password-copy", r#""id":"a""#),
    ("password-reused", ""),
    ("auth-add", r#""link":"otpauth://totp/x?secret=JBSWY3DPEHPK3PXP""#),
    ("auth-list", ""),
    ("auth-code", r#""id":"a""#),
    ("auth-link", r#""id":"a""#),
    ("auth-update", r#""id":"a","order":1"#),
    ("auth-note", r#""id":"a""#),
    ("auth-export", r#""format":"file","password":"cGFzcw==""#),
    ("import-merge", r#""accounts":[]"#),
    ("codes-add", r#""service":"s","codes":["1234"]"#),
    ("codes-list", ""),
    ("codes-next", r#""id":"a""#),
    ("codes-used", r#""id":"a","index":0,"used":true"#),
    ("codes-all", r#""id":"a""#),
    ("codes-update", r#""id":"a","order":1"#),
    ("codes-note", r#""id":"a""#),
    ("codes-coverage", ""),
    ("key-new", r#""name":"k","key_kind":"signing""#),
    ("key-import", r#""name":"k","type":"ed25519","private":"AAAA""#),
    ("key-public", r#""id":"a""#),
    ("key-export", r#""id":"a","plain":true"#),
    ("cert-new", r#""profile":"pdf","title":"t""#),
    ("cert-list", ""),
    ("cert-show", r#""id":"a""#),
    ("cert-chain", r#""id":"a""#),
    ("cert-pfx", r#""id":"a","password":"cGFzcw==""#),
    ("sign-digest", r#""id":"a","purpose":"pdf","digest":"sha256","value":"AAAA""#),
];

fn with_fields(op: &str, fields: &str) -> Value {
    let joined = if fields.is_empty() { format!(r#"{{"op":"{op}"}}"#) } else { format!(r#"{{"op":"{op}",{fields}}}"#) };
    serde_json::from_str(&joined).unwrap()
}

#[test]
fn nothing_works_closed_or_never_set_up() {
    let running = start("closed");
    for (op, fields) in NEEDS_OPEN {
        assert_eq!(why(&running.ask(with_fields(op, fields))), "not-set-up", "{op}");
    }
    running.set_up(PASSWORD);
    ok(running.ask(json!({"op": "lock"})));
    let prompts = running.outside.asked();
    for (op, fields) in NEEDS_OPEN {
        assert_eq!(why(&running.ask(with_fields(op, fields))), "closed", "{op}");
    }
    assert_eq!(running.outside.asked(), prompts, "nobody is asked anything about a closed vault");
    for op in ["cert-key", "cert-renew"] {
        assert_eq!(why(&running.ask(with_fields(op, r#""id":"a","plain":true"#))), "closed", "{op}");
    }
    assert_eq!(why(&running.ask(json!({"op": "ca-cert", "which": "account"}))), "closed");
    // What touches no vault still answers
    ok(running.ask(json!({"op": "password-generate"})));
    ok(running.ask(json!({"op": "cert-profiles"})));
    assert_eq!(ok(running.ask(json!({"op": "machine"})))["root"], json!(false));
}

#[test]
fn a_kind_only_the_service_reads_goes_through_no_generic_op() {
    let running = opened("service-only");
    let id = add_account(&running, "Example");
    for kind in ["authenticator", "backup-codes", "key", "certificate", "account-ca", "data-key"] {
        assert_eq!(why(&running.ask(json!({"op": "put", "kind": kind, "id": "a", "name": "n", "value": {"secret": "x"}}))), "refused", "{kind}");
        assert_eq!(why(&running.ask(json!({"op": "get", "kind": kind, "id": "a"}))), "refused", "{kind}");
    }
    assert_eq!(why(&running.ask(json!({"op": "get", "kind": "authenticator", "id": id}))), "refused");
    // Its setting is still the person's to choose, but for the kinds nothing but the service ever uses
    ok(running.ask(json!({"op": "set-approval", "kind": "authenticator", "id": id, "approval": "ask"})));
    for kind in ["account-ca", "data-key"] {
        assert_eq!(why(&running.ask(json!({"op": "set-approval", "kind": kind, "id": "a", "approval": "ask"}))), "refused", "{kind}");
    }
}

#[test]
fn hostile_fields_are_refused_and_nothing_panics() {
    let running = opened("hostile");
    let long = "x".repeat(5000);
    for (op, fields) in [
        ("password-generate", json!({"length": 7})),
        ("password-generate", json!({"length": 129})),
        ("password-generate", json!({"length": "20"})),
        ("password-generate", json!({"length": -1})),
        ("password-generate", json!({"length": 1e30})),
        ("password-generate", json!({"style": "magic"})),
        ("password-generate", json!({"style": "words", "count": 2})),
        ("password-generate", json!({"style": "words", "count": 21})),
        ("password-generate", json!({"style": "words", "separator": "ab"})),
        ("password-generate", json!({"style": "words", "separator": "\n"})),
        ("password-generate", json!({"lower": false, "upper": false, "digits": false, "symbols": false})),
        ("password-copy", json!({"id": "../../etc/passwd"})),
        ("password-copy", json!({"id": ""})),
        ("password-copy", json!({"id": 5})),
        ("auth-code", json!({"id": "a/b"})),
        ("auth-code", json!({"id": ".."})),
        ("auth-add", json!({})),
        ("auth-add", json!({"link": "x", "account": {}})),
        ("auth-add", json!({"account": {"issuer": 5}})),
        ("auth-add", json!({"account": "text"})),
        ("auth-update", json!({"id": "a/b"})),
        ("codes-used", json!({"id": "a"})),
        ("codes-used", json!({"id": "a", "index": 5000, "used": true})),
        ("codes-used", json!({"id": "a", "index": -1, "used": true})),
        ("codes-add", json!({"service": "s"})),
        ("codes-add", json!({"service": "", "codes": ["1234"]})),
        ("codes-add", json!({"service": "s\nt", "codes": ["1234"]})),
        ("codes-add", json!({"service": "s", "codes": [5]})),
        ("codes-add", json!({"service": "s", "codes": [long]})),
        ("key-new", json!({"name": "k", "key_kind": "magic"})),
        ("key-new", json!({"name": "k\u{0}", "key_kind": "ssh"})),
        ("key-new", json!({"name": "k", "key_kind": "ssh", "vault": "../x"})),
        ("key-new", json!({"name": "k", "key_kind": "ssh", "type": long})),
        ("key-new", json!({"name": "k", "key_kind": "ssh", "approval": "sometimes"})),
        ("key-import", json!({"name": "k", "type": "ed25519", "private": "!!!"})),
        ("key-export", json!({"id": "a"})),
        ("key-export", json!({"id": "a", "plain": true, "passphrase": "cGFzcw=="})),
        ("cert-new", json!({"profile": "pdf"})),
        ("cert-new", json!({"profile": "pdf", "title": "t", "hosts": "x"})),
        ("cert-new", json!({"profile": "pdf", "title": "t", "email": long})),
        ("cert-new", json!({"profile": "pdf", "title": "t", "approval": "never"})),
        ("sign-digest", json!({"id": "a", "purpose": "smime", "digest": "sha256", "value": "AAAA"})),
        ("sign-digest", json!({"id": "a", "purpose": "pdf", "digest": "md5", "value": "AAAA"})),
        ("ca-cert", json!({"which": "other"})),
        ("import-merge", json!({"accounts": "x"})),
        ("auth-export", json!({"format": "zip", "password": "cGFzcw=="})),
    ] {
        let mut request = fields.clone();
        request["op"] = json!(op);
        let answer = running.ask(request);
        assert_ne!(answer["ok"], json!(true), "{op} {fields}: {answer}");
        assert!(matches!(why(&answer), "bad-request" | "refused" | "too-large"), "{op} {fields}: {answer}");
    }
    assert_eq!(running.outside.asked(), 1, "nobody is asked anything for what is malformed: only the setup");
    // An op nobody has
    assert_eq!(why(&running.ask(json!({"op": "no-such-op"}))), "bad-request");
    // And it is still there
    ok(running.ask(json!({"op": "status"})));
}

// --- Passwords -------------------------------------------------------------------------------------------------------------

#[test]
fn passwords_are_made_and_go_only_when_approved() {
    let running = opened("passwords");
    let made = ok(running.ask(json!({"op": "password-generate", "length": 32, "symbols": false})));
    let secret = decode(&made["secret"]);
    assert_eq!(secret.len(), 32);
    assert!(secret.iter().all(u8::is_ascii_alphanumeric));
    assert!(made["bits"].as_f64().unwrap() > 180.0);
    let words = ok(running.ask(json!({"op": "password-generate", "style": "words", "count": 4, "separator": " "})));
    assert_eq!(String::from_utf8(decode(&words["secret"])).unwrap().split(' ').count(), 4);

    let value = json!({"title": "Mail", "username": "ada", "password": "hunter2-hunter2", "history": [{"password": "old-one", "changedAt": 5}]});
    let id = ok(running.ask(json!({"op": "put", "kind": "password", "name": "Mail", "value": value})))["id"].as_str().unwrap().to_string();
    let before = running.outside.asked();
    refuse_prompts(&running, true);
    assert_eq!(why(&running.ask(json!({"op": "password-copy", "id": id}))), "refused");
    assert_eq!(why(&running.ask(json!({"op": "get", "kind": "password", "id": id}))), "refused");
    assert_eq!(running.outside.asked(), before + 2, "while-open has not been approved, so each asks");
    refuse_prompts(&running, false);
    let copied = ok(running.ask(json!({"op": "password-copy", "id": id})));
    assert_eq!(decode(&copied["secret"]), b"hunter2-hunter2");
    assert_eq!(copied["copy"], json!("secret"));
    assert!(!copied.to_string().contains("old-one"), "the current one alone");
    assert_eq!(running.outside.asked(), before + 3);
    // Asked once while open, then not
    let got = ok(running.ask(json!({"op": "get", "kind": "password", "id": id})));
    assert_eq!(got["copy"], json!("secret"));
    assert_eq!(got["value"]["history"][0]["password"], json!("old-one"));
    assert_eq!(running.outside.asked(), before + 3);
    // Ask every time asks every time
    ok(running.ask(json!({"op": "set-approval", "kind": "password", "id": id, "approval": "ask"})));
    let before = running.outside.asked();
    ok(running.ask(json!({"op": "password-copy", "id": id})));
    ok(running.ask(json!({"op": "password-copy", "id": id})));
    assert_eq!(running.outside.asked(), before + 2);
}

#[test]
fn which_items_share_a_password_is_said_with_no_password() {
    let running = opened("reused");
    for (name, password) in [("a", "same-password"), ("b", "same-password"), ("c", "other-password")] {
        ok(running.ask(json!({"op": "put", "kind": "password", "id": name, "name": name, "value": {"password": password}})));
    }
    // Not all of a kind's items need be well made: one that does not read is left out
    ok(running.ask(json!({"op": "put", "kind": "password", "id": "d", "name": "d", "value": 5})));
    // Only items a program could read with no question take part: a, b and c are said yes to, a new guess is not
    for id in ["a", "b", "c"] {
        ok(running.ask(json!({"op": "get", "kind": "password", "id": id})));
    }
    ok(running.ask(json!({"op": "put", "kind": "password", "id": "guess", "name": "guess", "value": {"password": "other-password"}})));
    ok(running.ask(json!({"op": "put", "kind": "password", "id": "guess2", "name": "guess2", "value": {"password": "same-password"}})));
    let before = running.outside.asked();
    let answer = ok(running.ask(json!({"op": "password-reused"})));
    let mut group: Vec<&str> = answer["groups"][0].as_array().unwrap().iter().map(|id| id.as_str().unwrap()).collect();
    group.sort_unstable();
    assert_eq!(group, ["a", "b"]);
    assert_eq!(answer["groups"].as_array().unwrap().len(), 1);
    assert!(!answer.to_string().contains("same-password"));
    assert_eq!(running.outside.asked(), before);
    assert_eq!(why(&running.ask(json!({"op": "password-copy", "id": "d"}))), "refused");
}

// --- Authenticator ---------------------------------------------------------------------------------------------------------

#[test]
fn an_account_goes_in_and_only_codes_come_out() {
    let running = opened("auth");
    let id = add_account(&running, "Example");
    let listed = ok(running.ask(json!({"op": "auth-list"})));
    assert_eq!(listed["items"][0]["meta"]["issuer"], json!("Example"));
    assert_eq!(listed["items"][0]["meta"]["type"], json!("totp"));
    assert_eq!(listed["items"][0]["name"], json!("Example (ada)"));
    assert!(!listed.to_string().contains(SECRET), "{listed}");
    let generic = ok(running.ask(json!({"op": "list", "kind": "authenticator"})));
    assert!(!generic.to_string().contains(SECRET));
    assert!(!disk_holds(&running.state(), SECRET.as_bytes()), "the secret is sealed");

    let reference = Account { secret: Secret::new(SECRET), ..Account::default() };
    let before = now();
    let answer = ok(running.ask(json!({"op": "auth-code", "id": id})));
    let after = now();
    let code = answer["code"].as_str().unwrap();
    assert!(code == otp::code_at(&reference, before, 0).as_str() || code == otp::code_at(&reference, after, 0).as_str());
    assert_eq!(answer["period"], json!(30));
    let left = answer["seconds_left"].as_i64().unwrap();
    assert!((1..=30).contains(&left));
    assert_eq!(answer.get("next").is_some(), left < 5);
    assert!(!answer.to_string().contains(SECRET));
    // While open: asked once
    let asked_now = asked(&running);
    assert_eq!(asked_now.last().unwrap(), &(Action::Use, say::USE_ITEM, "Example (ada)".to_string()));
    let count = running.outside.asked();
    ok(running.ask(json!({"op": "auth-code", "id": id})));
    assert_eq!(running.outside.asked(), count);
    // A step is for HOTP alone
    assert_eq!(why(&running.ask(json!({"op": "auth-code", "id": id, "step": true}))), "bad-request");
    // The same account is the same issuer and secret, however it is written
    let again = ok(running.ask(json!({"op": "auth-add", "account": {"issuer": "Example", "label": "other", "secret": SECRET.to_lowercase()}})));
    assert_eq!(again["added"], json!(false));
    assert_eq!(again["id"], json!(id));
    assert_eq!(ok(running.ask(json!({"op": "auth-list"})))["items"].as_array().unwrap().len(), 1);
}

#[test]
fn a_link_is_parsed_here_and_a_bad_one_refused() {
    let running = opened("link");
    let answer = ok(running.ask(json!({"op": "auth-add", "link": format!("otpauth://totp/Site:ada?secret={SECRET}&issuer=Site&digits=8&algorithm=SHA256&period=60")})));
    let id = answer["id"].as_str().unwrap();
    let item = &ok(running.ask(json!({"op": "auth-list"})))["items"][0];
    assert_eq!(item["meta"]["digits"], json!(8));
    assert_eq!(item["meta"]["algorithm"], json!("SHA256"));
    assert_eq!(item["meta"]["period"], json!(60));
    assert_eq!(ok(running.ask(json!({"op": "auth-code", "id": id})))["code"].as_str().unwrap().len(), 8);
    let steam = ok(running.ask(json!({"op": "auth-add", "link": format!("steam://{SECRET}x")})));
    let _ = steam;
    for bad in ["", "https://example.com", "otpauth://totp/x", "otpauth://totp/x?secret=", "otpauth://totp/x?secret=!!!", "otpauth://bad/x?secret=ABCDEFGH"] {
        let answer = running.ask(json!({"op": "auth-add", "link": bad}));
        assert_eq!(why(&answer), "refused", "{bad}");
    }
    for account in [
        json!({"issuer": "x", "secret": ""}),
        json!({"issuer": "x"}),
        json!({"issuer": "", "label": "", "secret": SECRET}),
        json!({"issuer": "x", "secret": "!!!"}),
        json!({"issuer": "x", "secret": "A"}),
        json!({"issuer": "x", "secret": SECRET, "algorithm": "MD5"}),
        json!({"issuer": "x", "secret": SECRET, "digits": 0}),
        json!({"issuer": "x", "secret": SECRET, "digits": 11}),
        json!({"issuer": "x", "secret": SECRET, "period": 0}),
        json!({"issuer": "x", "secret": SECRET, "period": -30}),
        json!({"issuer": "x", "secret": SECRET, "counter": -1}),
        json!({"issuer": "x".repeat(2000), "secret": SECRET}),
        json!({"issuer": "x", "secret": "A".repeat(2000)}),
        json!({"issuer": "x", "secret": SECRET, "deletedAt": 5}),
    ] {
        let answer = running.ask(json!({"op": "auth-add", "account": account}));
        assert_ne!(answer["ok"], json!(true), "{account}: {answer}");
    }
}

#[test]
fn hotp_moves_on_being_asked_and_never_back() {
    let running = opened("hotp");
    let id = ok(running.ask(json!({"op": "auth-add", "account": {"type": "hotp", "issuer": "Bank", "label": "ada", "secret": SECRET, "counter": 3}})))["id"].as_str().unwrap().to_string();
    let reference = |counter| Account { kind: Some(OtpKind::Hotp), secret: Secret::new(SECRET), counter: Some(counter), ..Account::default() };
    let at = |counter: i64| otp::code_at(&reference(counter), 0, 0).as_str().to_string();
    let same = ok(running.ask(json!({"op": "auth-code", "id": id})));
    assert_eq!(same["code"], json!(at(3)));
    assert_eq!(same["counter"], json!(3));
    assert!(same.get("seconds_left").is_none());
    let stepped = ok(running.ask(json!({"op": "auth-code", "id": id, "step": true})));
    assert_eq!(stepped["code"], json!(at(4)));
    assert_eq!(stepped["counter"], json!(4));
    assert_eq!(ok(running.ask(json!({"op": "auth-list"})))["items"][0]["meta"]["counter"], json!(4));
    // Nothing sets it back: an update has no such field, and another step only goes on
    ok(running.ask(json!({"op": "auth-update", "id": id, "counter": 1, "order": 2})));
    assert_eq!(ok(running.ask(json!({"op": "auth-code", "id": id})))["counter"], json!(4));
    // It stays where it was, through closing and opening
    ok(running.ask(json!({"op": "lock"})));
    ok(running.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)})));
    assert_eq!(ok(running.ask(json!({"op": "auth-code", "id": id, "step": true})))["counter"], json!(5));
    // A person who says no moves nothing
    ok(running.ask(json!({"op": "set-approval", "kind": "authenticator", "id": id, "approval": "ask"})));
    refuse_prompts(&running, true);
    assert_eq!(why(&running.ask(json!({"op": "auth-code", "id": id, "step": true}))), "refused");
    refuse_prompts(&running, false);
    assert_eq!(ok(running.ask(json!({"op": "auth-code", "id": id})))["counter"], json!(5));
    // At the top of what a counter may be, a step is refused rather than wrapping
    let top = ok(running.ask(json!({"op": "auth-add", "account": {"type": "hotp", "issuer": "Top", "secret": SECRET, "counter": (1i64 << 53) - 1}})))["id"].as_str().unwrap().to_string();
    assert_eq!(why(&running.ask(json!({"op": "auth-code", "id": top, "step": true}))), "refused");
}

#[test]
fn the_link_for_a_qr_always_asks() {
    let running = opened("qr");
    let id = add_account(&running, "Example");
    ok(running.ask(json!({"op": "set-approval", "kind": "authenticator", "id": id, "approval": "never"})));
    let before = running.outside.asked();
    refuse_prompts(&running, true);
    assert_eq!(why(&running.ask(json!({"op": "auth-link", "id": id}))), "refused");
    refuse_prompts(&running, false);
    let answer = ok(running.ask(json!({"op": "auth-link", "id": id})));
    assert!(answer["link"].as_str().unwrap().starts_with("otpauth://totp/Example:ada?issuer=Example&secret=JBSW"), "{answer}");
    assert!(answer["key"].as_str().unwrap().starts_with("JBSW Y3DP"));
    let asks = asked(&running);
    assert_eq!(asks.len(), before + 2);
    assert_eq!(asks.last().unwrap(), &(Action::Export, say::SHOW_QR, "Example (ada)".to_string()));
    // A `never` account gives a code with no question at all
    ok(running.ask(json!({"op": "auth-code", "id": id})));
    assert_eq!(running.outside.asked(), before + 2);
}

#[test]
fn a_name_changing_asks_and_an_order_does_not() {
    let running = opened("update");
    let id = add_account(&running, "Example");
    let before = running.outside.asked();
    ok(running.ask(json!({"op": "auth-update", "id": id, "order": 3, "pinned": true, "tags": ["work"], "no_backup_codes": true})));
    assert_eq!(running.outside.asked(), before);
    refuse_prompts(&running, true);
    assert_eq!(why(&running.ask(json!({"op": "auth-update", "id": id, "issuer": "Bank"}))), "refused");
    assert_eq!(why(&running.ask(json!({"op": "auth-update", "id": id, "note": "text"}))), "refused");
    refuse_prompts(&running, false);
    ok(running.ask(json!({"op": "auth-update", "id": id, "issuer": "Bank", "note": "the note"})));
    let item = &ok(running.ask(json!({"op": "auth-list"})))["items"][0];
    assert_eq!(item["name"], json!("Bank (ada)"));
    assert_eq!(item["meta"]["order"], json!(3));
    assert_eq!(item["meta"]["pinned"], json!(true));
    assert_eq!(item["meta"]["tags"][0], json!("work"));
    assert_eq!(item["meta"]["has_note"], json!(true));
    assert!(!item.to_string().contains("the note"), "a note is not listed");
    assert_eq!(ok(running.ask(json!({"op": "auth-note", "id": id})))["note"], json!("the note"));
    ok(running.ask(json!({"op": "auth-update", "id": id, "note": null})));
    assert_eq!(ok(running.ask(json!({"op": "auth-note", "id": id})))["note"], Value::Null);
    // Still makes the same codes
    assert!(ok(running.ask(json!({"op": "auth-code", "id": id})))["code"].is_string());
    for bad in [json!({"issuer": "", "label": ""}), json!({"tags": "x"}), json!({"tags": [5]}), json!({"order": "x"}), json!({"note": 5})] {
        let mut request = bad.clone();
        request["op"] = json!("auth-update");
        request["id"] = json!(id);
        assert_eq!(why(&running.ask(request)), "bad-request", "{bad}");
    }
}

fn account_json(issuer: &str, secret: &str) -> Value {
    json!({"id": "not-a-uuid", "type": "totp", "issuer": issuer, "label": "ada", "secret": secret, "digits": 6, "period": 30, "algorithm": "SHA1", "addedAt": 1})
}

#[test]
fn a_merge_adds_what_is_new_and_says_what_was_held() {
    let running = opened("merge");
    let id = add_account(&running, "Held");
    let service = json!({"id": "x", "serviceName": "Mail", "codes": [{"code": "1111-1111"}]});
    let accounts = [account_json("Held", &SECRET.to_lowercase()), account_json("New", "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"), account_json("New", "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"), json!({"issuer": "Broken"}), json!({"issuer": "Gone", "secret": SECRET, "deletedAt": 5})];
    let before = running.outside.asked();
    let dry = ok(running.ask(json!({"op": "import-merge", "accounts": accounts, "services": [service], "dry": true})));
    assert_eq!(dry["added"], json!(1));
    assert_eq!(dry["same"], json!(2));
    assert_eq!(dry["added_services"], json!(1));
    assert_eq!(dry["skipped"], json!(2));
    assert_eq!(dry["duplicate"], json!([true, false, true]));
    assert_eq!(ok(running.ask(json!({"op": "auth-list"})))["items"].as_array().unwrap().len(), 1, "a dry run keeps nothing");
    let done = ok(running.ask(json!({"op": "import-merge", "accounts": accounts, "services": [service]})));
    assert_eq!(done["added"], json!(1));
    assert_eq!(running.outside.asked(), before, "adding asks nobody, as `put` does not");
    let items = ok(running.ask(json!({"op": "auth-list"})))["items"].as_array().unwrap().clone();
    assert_eq!(items.len(), 2);
    let added = items.iter().find(|item| item["meta"]["issuer"] == json!("New")).unwrap();
    // An id from a file is untrusted, and is not what it is kept under
    assert_ne!(added["id"], json!("not-a-uuid"));
    ok(running.ask(json!({"op": "auth-code", "id": added["id"]})));
    ok(running.ask(json!({"op": "auth-code", "id": id})));
    assert_eq!(ok(running.ask(json!({"op": "codes-list"})))["items"][0]["meta"]["service"], json!("Mail"));
    // The same again is all held
    let again = ok(running.ask(json!({"op": "import-merge", "accounts": accounts, "services": [service]})));
    assert_eq!(again["added"], json!(0));
    assert_eq!(again["same_services"], json!(1));
    // One line carries so many, and no more
    let many: Vec<Value> = (0..257).map(|at| account_json(&format!("n{at}"), SECRET)).collect();
    assert_eq!(why(&running.ask(json!({"op": "import-merge", "accounts": many}))), "too-large");
}

#[test]
fn a_replace_asks_and_takes_away() {
    let running = opened("replace");
    add_account(&running, "Old");
    ok(running.ask(json!({"op": "codes-add", "service": "Keeps", "codes": ["1234-5678"]})));
    let import = [account_json("New", "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ")];
    refuse_prompts(&running, true);
    assert_eq!(why(&running.ask(json!({"op": "import-merge", "accounts": import, "replace": true}))), "refused");
    assert_eq!(ok(running.ask(json!({"op": "auth-list"})))["items"][0]["meta"]["issuer"], json!("Old"));
    assert_eq!(asked(&running).last().unwrap().1, say::REPLACE_ITEMS);
    refuse_prompts(&running, false);
    let dry = ok(running.ask(json!({"op": "import-merge", "accounts": import, "replace": true, "dry": true})));
    assert_eq!(dry["removed"], json!(1));
    let done = ok(running.ask(json!({"op": "import-merge", "accounts": import, "replace": true})));
    assert_eq!(done["removed"], json!(1));
    let items = ok(running.ask(json!({"op": "auth-list"})))["items"].as_array().unwrap().clone();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["meta"]["issuer"], json!("New"));
    assert_eq!(ok(running.ask(json!({"op": "codes-list"})))["items"].as_array().unwrap().len(), 1, "a foreign file holds no backup codes");
    // BackMyCodes' own replaces both
    let done = ok(running.ask(json!({"op": "import-merge", "accounts": [account_json("Own", SECRET)], "source": "backmycodes", "replace": true})));
    assert_eq!(done["removed"], json!(2));
    assert_eq!(ok(running.ask(json!({"op": "codes-list"})))["items"].as_array().unwrap().len(), 0);
    // An empty import replaces nothing
    assert_eq!(why(&running.ask(json!({"op": "import-merge", "replace": true}))), "refused");
    assert_eq!(ok(running.ask(json!({"op": "auth-list"})))["items"].as_array().unwrap().len(), 1);
}

#[test]
fn an_export_always_asks_and_opens_again() {
    let running = opened("export");
    add_account(&running, "Example");
    ok(running.ask(json!({"op": "codes-add", "service": "Mail", "codes": ["1111-1111"]})));
    refuse_prompts(&running, true);
    assert_eq!(why(&running.ask(json!({"op": "auth-export", "format": "file", "password": b64("a password")}))), "refused");
    refuse_prompts(&running, false);
    assert_eq!(asked(&running).last().unwrap().0, Action::Export);
    assert_eq!(asked(&running).last().unwrap().1, say::EXPORT_ALL);
    assert_eq!(running.ask(json!({"op": "auth-export", "format": "file", "password": ""}))["problem"], json!("bad-password"));
    let file = ok(running.ask(json!({"op": "auth-export", "format": "file", "password": b64("a password")})));
    let contents = bmc::read_export(file["file"].as_str().unwrap().as_bytes(), "a password").unwrap();
    assert_eq!(contents.accounts[0].issuer, "Example");
    assert_eq!(contents.accounts[0].secret.as_str(), SECRET);
    assert_eq!(contents.services[0].codes[0].code.as_str(), "1111-1111");
    assert!(bmc::read_export(file["file"].as_str().unwrap().as_bytes(), "the wrong one").is_err());
    let qr = ok(running.ask(json!({"op": "auth-export", "format": "qr", "password": b64("a password")})));
    let mut collector = bmc::Collector::new();
    for chunk in qr["chunks"].as_array().unwrap() {
        collector.add(chunk.as_str().unwrap());
    }
    assert!(collector.is_complete());
    let contents = collector.read("a password", 0).unwrap();
    assert_eq!(contents.accounts[0].issuer, "Example");
}

// --- Backup codes ----------------------------------------------------------------------------------------------------------

/// A new service says what it left out too: the codes it was given twice are counted, not lost quietly
#[test]
fn a_new_service_reports_the_codes_it_left_out() {
    let running = opened("codes-left-out");
    let twice = ok(running.ask(json!({"op": "codes-add", "service": "Twice", "codes": ["7777-7777", "7777-7777"]})));
    assert_eq!((&twice["added"], &twice["repeats"], &twice["over_limit"]), (&json!(1), &json!(1), &json!(0)));
}

#[test]
fn backup_codes_come_one_at_a_time() {
    let running = opened("codes");
    let codes: Vec<String> = (0..5).map(|at| format!("1234-{at}000")).collect();
    let added = ok(running.ask(json!({"op": "codes-add", "service": "Mail", "account": "ada", "codes": codes, "note": "printed"})));
    assert_eq!(added["added"], json!(5));
    let id = added["id"].as_str().unwrap().to_string();
    let listed = ok(running.ask(json!({"op": "codes-list"})));
    let meta = &listed["items"][0]["meta"];
    assert_eq!((&meta["total"], &meta["used"], &meta["left"], &meta["low"]), (&json!(5), &json!(0), &json!(5), &json!(false)));
    assert_eq!(meta["has_note"], json!(true));
    assert_eq!(meta["account"], json!("ada"));
    assert!(!listed.to_string().contains("1234-0000"));
    assert!(!listed.to_string().contains("printed"));
    assert!(!disk_holds(&running.state(), b"1234-0000"));

    let next = ok(running.ask(json!({"op": "codes-next", "id": id})));
    assert_eq!(next["code"], json!("1234-0000"));
    assert_eq!(next["index"], json!(0));
    assert_eq!(next["left"], json!(5));
    assert_eq!(next["copy"], json!("secret"));
    assert!(!next.to_string().contains("1234-1000"));
    // Used, then the next; undone, back again
    let used = ok(running.ask(json!({"op": "codes-used", "id": id, "index": 0, "used": true})));
    assert!(used["used_at"].is_i64());
    assert_eq!(used["left"], json!(4));
    assert_eq!(ok(running.ask(json!({"op": "codes-next", "id": id})))["code"], json!("1234-1000"));
    let undone = ok(running.ask(json!({"op": "codes-used", "id": id, "index": 0, "used": false})));
    assert_eq!(undone["used_at"], Value::Null);
    assert_eq!(ok(running.ask(json!({"op": "codes-next", "id": id})))["code"], json!("1234-0000"));
    // Past the end of the list
    assert_eq!(why(&running.ask(json!({"op": "codes-used", "id": id, "index": 5, "used": true}))), "bad-request");
    // Low at two left, none at the end
    for index in 0..3 {
        ok(running.ask(json!({"op": "codes-used", "id": id, "index": index, "used": true})));
    }
    let meta = ok(running.ask(json!({"op": "codes-list"})))["items"][0]["meta"].clone();
    assert_eq!((&meta["used"], &meta["left"], &meta["low"]), (&json!(3), &json!(2), &json!(true)));
    for index in 3..5 {
        ok(running.ask(json!({"op": "codes-used", "id": id, "index": index, "used": true})));
    }
    let none = ok(running.ask(json!({"op": "codes-next", "id": id})));
    assert_eq!(none["code"], Value::Null);
    assert_eq!(none["left"], json!(0));
    // Adding to it: only what is not there
    let more = ok(running.ask(json!({"op": "codes-add", "id": id, "codes": ["1234-0000", "9999-9999"]})));
    assert_eq!(more["added"], json!(1));
    assert_eq!((&more["repeats"], &more["over_limit"]), (&json!(1), &json!(0)));
    assert_eq!(more["total"], json!(6));
    assert_eq!(ok(running.ask(json!({"op": "codes-note", "id": id})))["note"], json!("printed"));
    // Showing them all always asks, with no question left over from While open
    let before = running.outside.asked();
    refuse_prompts(&running, true);
    assert_eq!(why(&running.ask(json!({"op": "codes-all", "id": id}))), "refused");
    refuse_prompts(&running, false);
    let all = ok(running.ask(json!({"op": "codes-all", "id": id})));
    assert_eq!(all["codes"].as_array().unwrap().len(), 6);
    assert_eq!(all["codes"][0]["used"], json!(true));
    assert_eq!(running.outside.asked(), before + 2);
    assert_eq!(asked(&running).last().unwrap().1, say::SHOW_CODES);
    // A rename asks; an order does not
    let before = running.outside.asked();
    ok(running.ask(json!({"op": "codes-update", "id": id, "order": 2, "tags": ["t"]})));
    assert_eq!(running.outside.asked(), before);
    ok(running.ask(json!({"op": "set-approval", "kind": "backup-codes", "id": id, "approval": "ask"})));
    let before = running.outside.asked();
    ok(running.ask(json!({"op": "codes-update", "id": id, "service": "Post"})));
    assert_eq!(running.outside.asked(), before + 1);
    assert_eq!(ok(running.ask(json!({"op": "codes-list"})))["items"][0]["meta"]["service"], json!("Post"));
}

#[test]
fn coverage_is_worked_out_from_the_list() {
    let running = opened("coverage");
    add_account(&running, "Mail");
    add_account(&running, "Bank");
    let none = ok(running.ask(json!({"op": "auth-add", "account": {"issuer": "NoCodes", "secret": SECRET, "noBackupCodes": true}})));
    assert_eq!(none["added"], json!(true));
    ok(running.ask(json!({"op": "codes-add", "service": "mail", "codes": ["1234-5678"]})));
    let before = running.outside.asked();
    let answer = ok(running.ask(json!({"op": "codes-coverage"})));
    assert_eq!(answer["covered"], json!(1));
    assert_eq!(answer["total"], json!(2));
    assert_eq!(answer["missing"].as_array().unwrap().len(), 1);
    assert_eq!(answer["missing"][0]["service"], json!("Bank"));
    assert_eq!(running.outside.asked(), before);
}

// --- Keys ------------------------------------------------------------------------------------------------------------------

fn openssl(args: &[&str], input: Option<&[u8]>) -> (bool, String) {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new("openssl").args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    if let Some(input) = input {
        child.stdin.take().unwrap().write_all(input).unwrap();
    } else {
        drop(child.stdin.take());
    }
    let out = child.wait_with_output().unwrap();
    (out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

#[test]
fn keys_are_made_and_their_private_part_stays() {
    let running = opened("keys");
    let types = ok(running.ask(json!({"op": "key-types"})));
    assert_eq!(types["ssh"][0], json!("ed25519"));
    assert_eq!(types["symmetric"], json!(["aes-256"]));
    let prompts = running.outside.asked();

    let made = ok(running.ask(json!({"op": "key-new", "name": "My key", "key_kind": "ssh"})));
    let id = made["id"].as_str().unwrap().to_string();
    assert_eq!(made["type"], json!("ed25519"));
    let openssh = made["public"]["openssh"].as_str().unwrap();
    assert!(openssh.starts_with("ssh-ed25519 AAAA"));
    assert!(!made.to_string().contains("PRIVATE"));
    assert!(made.get("key").is_none());
    assert_eq!(running.outside.asked(), prompts, "making a key asks nobody");
    let item = ok(running.ask(json!({"op": "list", "kind": "key"})))["items"][0].clone();
    assert_eq!(item["approval"], json!("while-open"));
    assert_eq!(item["meta"]["openssh"], json!(openssh));
    assert_eq!(item["meta"]["key_kind"], json!("ssh"));
    assert_eq!(ok(running.ask(json!({"op": "key-public", "id": id})))["public"]["openssh"], json!(openssh));
    assert_eq!(running.outside.asked(), prompts, "nor does its public part");

    // The kind says which types go with it
    assert_eq!(running.ask(json!({"op": "key-new", "name": "k", "key_kind": "exchange", "type": "ed25519"}))["problem"], json!("key-not-allowed"));
    assert_eq!(running.ask(json!({"op": "key-new", "name": "k", "key_kind": "ssh", "type": "made-up"}))["problem"], json!("key-not-allowed"));
    assert_eq!(running.ask(json!({"op": "key-new", "name": "k", "key_kind": "signing", "type": "ecdsa-p521"}))["problem"], json!("key-not-allowed"));
    let signing = ok(running.ask(json!({"op": "key-new", "name": "Sign", "key_kind": "signing", "type": "ecdsa-p384"})));
    assert_eq!(ok(running.ask(json!({"op": "list", "kind": "key"})))["items"].as_array().unwrap().iter().find(|item| item["id"] == signing["id"]).unwrap()["approval"], json!("ask"), "any other key asks every time");
    let aes = ok(running.ask(json!({"op": "key-new", "name": "Sym", "key_kind": "symmetric"})));
    assert_eq!(aes["public"], Value::Null);

    // Export: always asks, encrypted unless plainly asked otherwise
    ok(running.ask(json!({"op": "set-approval", "kind": "key", "id": id, "approval": "never"})));
    let prompts = running.outside.asked();
    refuse_prompts(&running, true);
    assert_eq!(why(&running.ask(json!({"op": "key-export", "id": id, "passphrase": b64("a phrase")}))), "refused");
    refuse_prompts(&running, false);
    assert_eq!(asked(&running).last().unwrap(), &(Action::Export, say::EXPORT_ITEM, "My key".to_string()));
    let exported = ok(running.ask(json!({"op": "key-export", "id": id, "passphrase": b64("a phrase")})));
    let pem = decode(&exported["data"]);
    assert!(String::from_utf8_lossy(&pem).contains("ENCRYPTED PRIVATE KEY"));
    let (read_it, said) = openssl(&["pkey", "-passin", "pass:a phrase", "-pubout"], Some(&pem));
    assert!(read_it, "{said}");
    assert!(!openssl(&["pkey", "-passin", "pass:wrong", "-pubout"], Some(&pem)).0);
    assert_eq!(said.trim(), made["public"]["pem"].as_str().unwrap().trim());
    let plain = ok(running.ask(json!({"op": "key-export", "id": id, "plain": true})));
    assert!(String::from_utf8_lossy(&decode(&plain["data"])).contains("BEGIN PRIVATE KEY"));
    assert_eq!(running.outside.asked(), prompts + 3);
    assert_eq!(running.ask(json!({"op": "key-export", "id": id, "passphrase": ""}))["problem"], json!("bad-password"));
    assert!(!disk_holds(&running.state(), b"PRIVATE KEY"));
}

#[test]
fn a_key_made_not_to_leave_never_does() {
    let running = opened("not-exportable");
    let made = ok(running.ask(json!({"op": "key-new", "name": "Stays", "key_kind": "signing", "type": "ed25519", "exportable": false})));
    let id = &made["id"];
    let prompts = running.outside.asked();
    for request in [json!({"op": "key-export", "id": id, "passphrase": b64("a phrase")}), json!({"op": "key-export", "id": id, "plain": true})] {
        let answer = running.ask(request);
        assert_eq!(why(&answer), "refused");
        assert_eq!(answer["problem"], json!("not-exportable"));
    }
    assert_eq!(running.outside.asked(), prompts, "refused before anybody is asked");
    assert_eq!(ok(running.ask(json!({"op": "list", "kind": "key"})))["items"][0]["meta"]["exportable"], json!(false));
    // Lowering how often it asks changes nothing about that, and no generic op reaches it
    ok(running.ask(json!({"op": "set-approval", "kind": "key", "id": id, "approval": "never"})));
    assert_eq!(running.ask(json!({"op": "key-export", "id": id, "plain": true}))["problem"], json!("not-exportable"));
    assert_eq!(why(&running.ask(json!({"op": "get", "kind": "key", "id": id}))), "refused");
    assert_eq!(why(&running.ask(json!({"op": "put", "kind": "key", "id": id, "name": "n", "value": {"exportable": true}}))), "refused");
    // Its public part is still free
    ok(running.ask(json!({"op": "key-public", "id": id})));
}

#[test]
fn an_ssh_key_comes_in_finished_and_a_bad_one_does_not() {
    use amethystora_vault_core::keys;
    let running = opened("key-import");
    let key = keys::generate(keys::KeyType::Ed25519).unwrap();
    let public = key.public().unwrap();
    let answer = ok(running.ask(json!({"op": "key-import", "name": "Imported", "type": "ed25519", "private": encode(key.as_bytes()), "comment": "ada@host"})));
    assert_eq!(answer["public"]["openssh"], json!(public.openssh));
    let item = ok(running.ask(json!({"op": "list", "kind": "key"})))["items"][0].clone();
    assert_eq!(item["meta"]["comment"], json!("ada@host"));
    assert_eq!(item["meta"]["key_kind"], json!("ssh"));
    // A comment that is more than a name is dropped
    let other = keys::generate(keys::KeyType::Ed25519).unwrap();
    let answer = ok(running.ask(json!({"op": "key-import", "name": "Second", "type": "ed25519", "private": encode(other.as_bytes()), "comment": "ada\n@host"})));
    let items = ok(running.ask(json!({"op": "list", "kind": "key"})))["items"].as_array().unwrap().clone();
    assert!(items.iter().find(|item| item["id"] == answer["id"]).unwrap()["meta"].get("comment").is_none());
    // Not that type, not a key at all, and a key no SSH form exists for
    for (key_type, private) in [
        ("rsa-3072", encode(key.as_bytes())),
        ("ed25519", encode(b"not a key")),
        ("ed25519", encode(&[0u8; 32])),
        ("made-up", encode(key.as_bytes())),
    ] {
        let answer = running.ask(json!({"op": "key-import", "name": "k", "type": key_type, "private": private}));
        assert_eq!(why(&answer), "refused", "{key_type}: {answer}");
    }
    // One key's private half beside another's public point, which a PEM put together by hand can hold (the pki review's M3)
    let (one, two) = (keys::generate(keys::KeyType::EcdsaP256).unwrap(), keys::generate(keys::KeyType::EcdsaP256).unwrap());
    let point = |key: &keys::PrivateKey| key.public().unwrap().spki_der[26..].to_vec();
    let (mine, theirs) = (point(&one), point(&two));
    let mut mixed = one.as_bytes().to_vec();
    let at = mixed.windows(mine.len()).position(|window| window == mine.as_slice()).unwrap();
    mixed[at..at + mine.len()].copy_from_slice(&theirs);
    assert_eq!(running.ask(json!({"op": "key-import", "name": "k", "type": "ecdsa-p256", "key_kind": "signing", "private": encode(&mixed)}))["problem"], json!("malformed"));
    let exchange = keys::generate(keys::KeyType::X25519).unwrap();
    assert_eq!(running.ask(json!({"op": "key-import", "name": "k", "type": "x25519", "private": encode(exchange.as_bytes())}))["problem"], json!("key-not-allowed"));
    ok(running.ask(json!({"op": "key-import", "name": "k", "type": "x25519", "key_kind": "exchange", "private": encode(exchange.as_bytes())})));
    assert_eq!(why(&running.ask(json!({"op": "key-import", "name": "k", "type": "ed25519", "private": "A".repeat(200_000)}))), "too-large");
}

#[test]
fn a_slow_key_does_not_stall_the_loop() {
    let running = opened("rsa");
    let socket = running.socket.clone();
    let making = std::thread::spawn(move || amethystora_vault_client::exchange(&socket, &json!({"v": 1, "op": "key-new", "name": "Big", "key_kind": "ssh", "type": "rsa-4096"}), Duration::from_secs(120)).unwrap());
    std::thread::sleep(Duration::from_millis(20));
    let started = std::time::Instant::now();
    ok(running.ask(json!({"op": "status"})));
    let stalled = started.elapsed();
    let made = making.join().unwrap();
    assert_eq!(made["ok"], json!(true), "{made}");
    assert!(stalled < Duration::from_millis(200), "status took {stalled:?} while a key was being made");
}

// --- Certificates ----------------------------------------------------------------------------------------------------------

const ADA: fn() -> Person = || Person { login: "ada".into(), full_name: Some("Ada Lovelace".into()) };

fn as_ada(running: &Running) {
    running.outside.said.lock().unwrap().person = Some(ADA());
}

fn new_cert(running: &Running, fields: Value) -> Value {
    let mut request = fields;
    request["op"] = json!("cert-new");
    if request.get("title").is_none() {
        request["title"] = json!("A certificate");
    }
    running.ask(request)
}

fn chain_of(running: &Running, id: &Value) -> String {
    ok(running.ask(json!({"op": "cert-chain", "id": id})))["pem"].as_str().unwrap().to_string()
}

/// `openssl verify` of a leaf against a root, the rest of its chain as untrusted
fn verifies(dir: &Path, chain: &str, root: &str, purpose: &str) -> (bool, String) {
    let certs: Vec<String> = chain.split_inclusive("-----END CERTIFICATE-----\n").map(str::to_string).collect();
    std::fs::write(dir.join("leaf.pem"), &certs[0]).unwrap();
    std::fs::write(dir.join("ca.pem"), certs[1..certs.len() - 1].concat()).unwrap();
    std::fs::write(dir.join("root.pem"), root).unwrap();
    let path = |name: &str| dir.join(name).to_str().unwrap().to_string();
    openssl(&["verify", "-x509_strict", "-purpose", purpose, "-CAfile", &path("root.pem"), "-untrusted", &path("ca.pem"), &path("leaf.pem")], None)
}

#[test]
fn a_certificate_comes_with_its_chain_and_the_root_only_on_first_need() {
    let running = opened("cert");
    as_ada(&running);
    let machine = running.state().join("machine");
    assert!(!machine.exists(), "a machine nobody signs on holds no key");
    assert_eq!(ok(running.ask(json!({"op": "machine"})))["root"], json!(false));
    assert_eq!(why(&running.ask(json!({"op": "ca-cert", "which": "root"}))), "refused");
    assert_eq!(why(&running.ask(json!({"op": "ca-cert", "which": "account"}))), "refused");

    // A refusal costs no prompt and makes no root
    let prompts = running.outside.asked();
    for (fields, problem, field) in [
        (json!({"profile": "made-up"}), "unknown-profile", None),
        (json!({"profile": "account-ca"}), "unknown-profile", None),
        (json!({"profile": "device-ca"}), "unknown-profile", None),
        (json!({"profile": "local-services-ca"}), "unknown-profile", None),
        (json!({"profile": "root"}), "unknown-profile", None),
        (json!({"profile": "pdf", "name": "j.smith"}), "name-looks-like-host", None),
        (json!({"profile": "pdf", "name": "x\u{202e}y"}), "bad-field", Some("name")),
        (json!({"profile": "pdf", "name": "y".repeat(70)}), "bad-field", Some("name")),
        (json!({"profile": "pdf", "email": "not an address"}), "bad-field", Some("email")),
        (json!({"profile": "pdf", "web": "https://x.example"}), "not-in-profile", Some("web address")),
        (json!({"profile": "pdf", "hosts": ["printer.local"]}), "not-in-profile", Some("host name")),
        (json!({"profile": "pdf", "country": "fr"}), "bad-field", Some("country")),
        (json!({"profile": "pdf", "key_type": "ed25519"}), "key-not-allowed", None),
        (json!({"profile": "smime"}), "missing-field", Some("email")),
        (json!({"profile": "local-tls"}), "missing-field", Some("host name")),
        (json!({"profile": "local-tls", "hosts": ["www.google.com"]}), "outside-limits", None),
        (json!({"profile": "local-tls", "hosts": ["8.8.8.8"]}), "outside-limits", None),
        (json!({"profile": "local-tls", "hosts": ["printer.local", "local.evil.example"]}), "outside-limits", None),
        (json!({"profile": "local-tls", "hosts": ["*.local"]}), "bad-field", Some("host name")),
        (json!({"profile": "local-tls", "name": "Who"}), "not-in-profile", Some("name")),
        (json!({"profile": "service-login"}), "missing-field", Some("name")),
        (json!({"profile": "device-identity", "name": "Who"}), "not-in-profile", Some("name")),
    ] {
        let answer = new_cert(&running, fields.clone());
        assert_eq!(why(&answer), "refused", "{fields}: {answer}");
        assert_eq!(answer["problem"], json!(problem), "{fields}: {answer}");
        assert_eq!(answer["field"].as_str(), field, "{fields}: {answer}");
        assert!(!answer.to_string().contains("j.smith") && !answer.to_string().contains("evil"), "what was typed is not said back: {answer}");
    }
    assert_eq!(running.outside.asked(), prompts);
    assert!(!machine.exists(), "and no root was made for them");

    // Saying no makes nothing either
    refuse_prompts(&running, true);
    assert_eq!(why(&new_cert(&running, json!({"profile": "pdf"}))), "refused");
    refuse_prompts(&running, false);
    assert_eq!(asked(&running).last().unwrap(), &(Action::Use, say::ISSUE_CERT, "A certificate".to_string()));
    assert!(!machine.exists());

    let made = ok(new_cert(&running, json!({"profile": "pdf", "title": "My PDFs", "key_type": "ecdsa-p256"})));
    assert_eq!(made["new_ca"], json!(true));
    let description = &made["description"];
    assert_eq!(description["subject"], json!("CN=Ada Lovelace, OU=Amethystora Vault"));
    assert_eq!(description["issuer"], json!("CN=Ada Lovelace Code Signing CA, OU=Amethystora OS"));
    assert_eq!(description["profile"], json!("pdf"));
    assert_eq!(description["key_type"], json!("ecdsa-p256"));
    assert!(!made.to_string().contains("PRIVATE"));

    // What the machine now has: a root with its key sealed, and the account's CA in the vault, not beside the root
    let root_file = std::fs::read(machine.join("root.json")).unwrap();
    assert!(!root_file.windows(11).any(|window| window == b"PRIVATE KEY"));
    let root_record: Value = serde_json::from_slice(&root_file).unwrap();
    assert_eq!(root_record["key"]["sealed"], json!(true));
    let machine_said = ok(running.ask(json!({"op": "machine"})));
    assert_eq!(machine_said["root"], json!(true));
    let number = machine_said["number"].as_str().unwrap().to_string();
    assert_eq!(number.len(), 8);
    assert_eq!(machine_said["subject"], json!(format!("CN=Amethystora OS Root CA, OU=Machine {number}")));
    assert!(!machine.join("device-ca.json").exists() && !machine.join("local-ca.json").exists());
    assert_eq!(ok(running.ask(json!({"op": "list", "kind": "account-ca"})))["items"].as_array().unwrap().len(), 1);

    // Verified by a program that is not this one, against the root
    let dir = common::Folder::new("verify");
    let root = ok(running.ask(json!({"op": "ca-cert", "which": "root"})))["pem"].as_str().unwrap().to_string();
    let chain = chain_of(&running, &made["id"]);
    assert_eq!(chain.matches("BEGIN CERTIFICATE").count(), 3);
    let (good, said) = verifies(&dir.0, &chain, &root, "smimesign");
    assert!(good, "{said}");
    // The second is under the same CA: no new one, and the same chain above it
    let second = ok(new_cert(&running, json!({"profile": "code", "name": "Ada's Programs", "web": "https://example.org", "organization": "Lab", "country": "GB"})));
    assert_eq!(second["new_ca"], json!(false));
    assert_eq!(second["description"]["subject"], json!("CN=Ada's Programs, OU=Amethystora Vault, O=Lab, C=GB"));
    let chain2 = chain_of(&running, &second["id"]);
    assert_eq!(chain2.split_inclusive("-----END CERTIFICATE-----\n").nth(1), chain.split_inclusive("-----END CERTIFICATE-----\n").nth(1));
    let (good, said) = verifies(&dir.0, &chain2, &root, "codesign");
    assert!(good, "{said}");

    // The CA's certificate is for giving out, and only that
    let ca = ok(running.ask(json!({"op": "ca-cert", "which": "account"})));
    assert!(ca["pem"].as_str().unwrap().contains("BEGIN CERTIFICATE"));
    assert_eq!(ca["subject"], json!("CN=Ada Lovelace Code Signing CA, OU=Amethystora OS"));
    assert_eq!(why(&running.ask(json!({"op": "ca-cert", "which": "device"}))), "refused");

    // Lists and showing
    let listed = ok(running.ask(json!({"op": "cert-list"})));
    assert_eq!(listed["items"].as_array().unwrap().len(), 2);
    assert!(!listed.to_string().contains("\"cert\""));
    let only_pdf = ok(running.ask(json!({"op": "cert-list", "purpose": "pdf", "chain": true})));
    assert_eq!(only_pdf["items"].as_array().unwrap().len(), 1);
    assert_eq!(only_pdf["items"][0]["chain"].as_array().unwrap().len(), 2);
    assert!(openssl(&["x509", "-inform", "DER", "-noout"], Some(&decode(&only_pdf["items"][0]["cert"]))).0);
    assert!(!only_pdf.to_string().contains("\"key\""), "{only_pdf}");
    let shown = ok(running.ask(json!({"op": "cert-show", "id": made["id"]})));
    assert_eq!(shown["name"], json!("My PDFs"));
    assert_eq!(shown["description"]["profile"], json!("pdf"));
    // What is free costs no prompt
    assert_eq!(why(&running.ask(json!({"op": "cert-show", "id": "nothing"}))), "refused");
    assert_eq!(why(&running.ask(json!({"op": "cert-chain", "id": "nothing"}))), "refused");
}

#[test]
fn a_certificate_lives_in_the_first_vault_alone() {
    let running = opened("first-only");
    as_ada(&running);
    let id_in_first = ok(new_cert(&running, json!({"profile": "smime", "email": "ada@example.org"})))["id"].clone();
    // With a hidden vault opened last, a certificate still goes in the first
    running.new_vault("Hidden", true, "another good password");
    let made = ok(new_cert(&running, json!({"profile": "smime", "email": "ada@example.org"})));
    let vaults = ok(running.ask(json!({"op": "status"})));
    let first = vaults["vaults"].as_array().unwrap().iter().find(|vault| vault["first"] == json!(true)).unwrap()["id"].clone();
    assert_eq!(made["vault"], first);
    // And with the first closed, there is none: only a hidden vault open says closed
    ok(running.ask(json!({"op": "lock"})));
    ok(running.ask(json!({"op": "unlock", "method": "password", "secret": b64("another good password")})));
    assert_eq!(why(&new_cert(&running, json!({"profile": "smime", "email": "ada@example.org"}))), "closed");
    assert_eq!(why(&running.ask(json!({"op": "cert-list"}))), "closed");
    assert_eq!(why(&running.ask(json!({"op": "cert-chain", "id": id_in_first}))), "closed");
    // A key in a hidden vault stays there
    let key = ok(running.ask(json!({"op": "key-new", "name": "Hidden key", "key_kind": "ssh"})));
    assert_ne!(key["vault"], first);
    ok(running.ask(json!({"op": "lock"})));
    ok(running.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)})));
    assert_eq!(why(&running.ask(json!({"op": "key-public", "id": key["id"]}))), "refused", "as any other item that is not there");
}

#[test]
fn a_guest_gets_none_and_somebody_unknown_none() {
    let running = opened("guest");
    running.outside.said.lock().unwrap().person = Some(Person { login: "amethystora-guest".into(), full_name: None });
    let prompts = running.outside.asked();
    let answer = new_cert(&running, json!({"profile": "pdf", "name": "Guest"}));
    assert_eq!(why(&answer), "refused");
    assert_eq!(answer["problem"], json!("guest"));
    assert_eq!(running.ask(json!({"op": "root-rotate"}))["problem"], json!("guest"));
    assert_eq!(running.outside.asked(), prompts);
    assert!(!running.state().join("machine").exists());
}

/// polkit's `allow_active`: a session over SSH gets a no, which is all this service ever sees of it. Stand-in for what suite 74
/// does on a machine.
#[test]
fn a_session_over_ssh_gets_no_ca() {
    let running = opened("ssh");
    as_ada(&running);
    refuse_prompts(&running, true);
    assert_eq!(why(&new_cert(&running, json!({"profile": "pdf"}))), "refused");
    assert!(!running.state().join("machine").exists(), "no root");
    assert_eq!(ok(running.ask(json!({"op": "list", "kind": "account-ca"})))["items"].as_array().unwrap().len(), 0, "no CA");
}

#[test]
fn every_profile_is_issued_and_a_machine_ca_made_once() {
    let running = opened("profiles");
    as_ada(&running);
    let machine = running.state().join("machine");
    let dir = common::Folder::new("verify-profiles");

    let tls = ok(new_cert(&running, json!({"profile": "local-tls", "hosts": ["printer.local", "192.168.1.5", "fd00::1"]})));
    assert_eq!(tls["new_ca"], json!(false), "no account's CA for this");
    assert_eq!(tls["description"]["subject"], json!("CN=Local service, OU=Amethystora OS"));
    assert_eq!(asked(&running).last().unwrap().0, Action::Use);
    assert!(machine.join("local-ca.json").exists());
    assert_eq!(ok(running.ask(json!({"op": "list", "kind": "account-ca"})))["items"].as_array().unwrap().len(), 0);
    let root = ok(running.ask(json!({"op": "ca-cert", "which": "root"})))["pem"].as_str().unwrap().to_string();
    let chain = chain_of(&running, &tls["id"]);
    let (good, said) = verifies(&dir.0, &chain, &root, "sslserver");
    assert!(good, "{said}");
    let local_ca = ok(running.ask(json!({"op": "ca-cert", "which": "local"})));
    assert!(local_ca["subject"].as_str().unwrap().starts_with("CN=Local Services CA, OU=Machine "));
    // The second comes from the same CA
    let local_file = std::fs::read(machine.join("local-ca.json")).unwrap();
    ok(new_cert(&running, json!({"profile": "local-tls", "hosts": ["nas.home.arpa"], "key_type": "rsa-3072"})));
    assert_eq!(std::fs::read(machine.join("local-ca.json")).unwrap(), local_file);

    let login = ok(new_cert(&running, json!({"profile": "service-login", "name": "VPN at work"})));
    assert_eq!(login["description"]["subject"].as_str().unwrap().split(", ").next().unwrap(), "CN=VPN at work");
    assert!(login["description"]["subject"].as_str().unwrap().contains("OU=ada"), "{login}");
    assert_eq!(asked(&running).last().unwrap().0, Action::Use);
    assert!(machine.join("device-ca.json").exists());
    let device_file = std::fs::read(machine.join("device-ca.json")).unwrap();
    let (good, said) = verifies(&dir.0, &chain_of(&running, &login["id"]), &root, "sslclient");
    assert!(good, "{said}");

    // The machine's identity is an administrator's word
    refuse_prompts(&running, true);
    assert_eq!(why(&new_cert(&running, json!({"profile": "device-identity"}))), "refused");
    refuse_prompts(&running, false);
    assert_eq!(asked(&running).last().unwrap(), &(Action::Admin, say::DEVICE_IDENTITY, "A certificate".to_string()));
    let device = ok(new_cert(&running, json!({"profile": "device-identity", "title": "This machine"})));
    assert!(device["description"]["subject"].as_str().unwrap().starts_with("CN=Machine "), "{device}");
    assert_eq!(asked(&running).last().unwrap().0, Action::Admin);
    assert_eq!(std::fs::read(machine.join("device-ca.json")).unwrap(), device_file);
    let (good, said) = verifies(&dir.0, &chain_of(&running, &device["id"]), &root, "sslclient");
    assert!(good, "{said}");

    let mail = ok(new_cert(&running, json!({"profile": "smime", "email": "ada@example.org", "key_type": "rsa-3072"})));
    let (good, said) = verifies(&dir.0, &chain_of(&running, &mail["id"]), &root, "smimeencrypt");
    assert!(good, "{said}");
    let code = ok(new_cert(&running, json!({"profile": "code"})));
    assert_eq!(code["description"]["subject"], json!("CN=Ada Lovelace, OU=Amethystora Vault"));
    assert_eq!(code["description"]["key_type"], json!("rsa-3072"), "RSA 3072 by default");
    let (printed, said) = openssl(&["x509", "-noout", "-ext", "extendedKeyUsage"], Some(chain_of(&running, &code["id"]).as_bytes()));
    assert!(printed && said.contains("Code Signing") && !said.contains("E-mail"), "{said}");
    // Each CA's own limits are in the certificate that is handed out
    let ca = ok(running.ask(json!({"op": "ca-cert", "which": "account"})));
    let (_, said) = openssl(&["x509", "-noout", "-ext", "nameConstraints"], Some(ca["pem"].as_str().unwrap().as_bytes()));
    assert!(said.contains("Excluded"), "{said}");
    // Only the key's owner has its folder: what is on disk of the machine holds no key in the clear
    for file in ["root.json", "local-ca.json", "device-ca.json"] {
        let bytes = std::fs::read(machine.join(file)).unwrap();
        let record: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(record["key"]["sealed"], json!(true), "{file}");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(machine.join(file)).unwrap().permissions().mode() & 0o077, 0, "{file}");
    }
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&machine).unwrap().permissions().mode() & 0o077, 0);
}

#[test]
fn a_new_root_makes_new_cas_at_their_next_certificate() {
    let running = opened("rotate");
    as_ada(&running);
    // Nothing to rotate yet, and this is no way to make the first
    assert_eq!(why(&running.ask(json!({"op": "root-rotate"}))), "refused");
    assert!(!running.state().join("machine").exists());
    let first = ok(new_cert(&running, json!({"profile": "pdf"})));
    ok(new_cert(&running, json!({"profile": "local-tls", "hosts": ["printer.local"]})));
    let before = ok(running.ask(json!({"op": "machine"})));
    let old_root = ok(running.ask(json!({"op": "ca-cert", "which": "root"})))["pem"].as_str().unwrap().to_string();

    refuse_prompts(&running, true);
    assert_eq!(why(&running.ask(json!({"op": "root-rotate"}))), "refused");
    refuse_prompts(&running, false);
    assert_eq!(asked(&running).last().unwrap().0, Action::Admin);
    assert_eq!(asked(&running).last().unwrap().1, say::ROTATE_ROOT);
    assert_eq!(ok(running.ask(json!({"op": "machine"})))["sha256"], before["sha256"], "a no changes nothing");

    let rotated = ok(running.ask(json!({"op": "root-rotate"})));
    assert_eq!(asked(&running).last().unwrap().0, Action::Admin);
    assert_ne!(rotated["sha256"], before["sha256"]);
    assert_eq!(why(&running.ask(json!({"op": "ca-cert", "which": "local"}))), "refused", "the machine's CAs went with it");
    let new_root = ok(running.ask(json!({"op": "ca-cert", "which": "root"})))["pem"].as_str().unwrap().to_string();

    // The next certificate has a new CA, under the new root; the old one still verifies for whoever trusted the old
    let next = ok(new_cert(&running, json!({"profile": "pdf"})));
    assert_eq!(next["new_ca"], json!(true));
    let dir = common::Folder::new("verify-rotate");
    let (good, said) = verifies(&dir.0, &chain_of(&running, &next["id"]), &new_root, "smimesign");
    assert!(good, "{said}");
    assert!(!verifies(&dir.0, &chain_of(&running, &next["id"]), &old_root, "smimesign").0);
    let (good, said) = verifies(&dir.0, &chain_of(&running, &first["id"]), &old_root, "smimesign");
    assert!(good, "{said}");
    assert!(!verifies(&dir.0, &chain_of(&running, &first["id"]), &new_root, "smimesign").0);
    let tls = ok(new_cert(&running, json!({"profile": "local-tls", "hosts": ["printer.local"]})));
    let (good, said) = verifies(&dir.0, &chain_of(&running, &tls["id"]), &new_root, "sslserver");
    assert!(good, "{said}");
}

#[test]
fn a_pfx_and_a_key_go_only_when_asked_and_never_quietly() {
    let running = opened("pfx");
    as_ada(&running);
    let made = ok(new_cert(&running, json!({"profile": "code", "key_type": "ecdsa-p256", "title": "Programs"})));
    let id = &made["id"];
    let prompts = running.outside.asked();
    refuse_prompts(&running, true);
    assert_eq!(why(&running.ask(json!({"op": "cert-pfx", "id": id, "password": b64("a password")}))), "refused");
    assert_eq!(why(&running.ask(json!({"op": "cert-key", "id": id, "passphrase": b64("a password")}))), "refused");
    refuse_prompts(&running, false);
    assert_eq!(asked(&running).last().unwrap(), &(Action::Export, say::EXPORT_ITEM, "Programs".to_string()));
    assert_eq!(running.outside.asked(), prompts + 2);
    assert_eq!(running.ask(json!({"op": "cert-pfx", "id": id, "password": ""}))["problem"], json!("bad-password"));
    assert_eq!(running.ask(json!({"op": "cert-pfx", "id": id, "password": b64(&"x".repeat(300))}))["problem"], json!("bad-password"));
    assert_eq!(running.ask(json!({"op": "cert-pfx", "id": id, "password": encode(b"a\0b")}))["problem"], json!("bad-password"));
    assert_eq!(running.outside.asked(), prompts + 2);

    let pfx = ok(running.ask(json!({"op": "cert-pfx", "id": id, "password": b64("a password")})));
    let dir = common::Folder::new("verify-pfx");
    let file = dir.0.join("out.pfx");
    std::fs::write(&file, decode(&pfx["data"])).unwrap();
    let path = file.to_str().unwrap();
    let (read_it, said) = openssl(&["pkcs12", "-in", path, "-passin", "pass:a password", "-nodes"], None);
    assert!(read_it, "{said}");
    assert!(said.contains("PRIVATE KEY") && said.matches("BEGIN CERTIFICATE").count() == 3, "{said}");
    assert!(!openssl(&["pkcs12", "-in", path, "-passin", "pass:wrong", "-nodes"], None).0);

    // Its key with its chain: encrypted, and plain for a local server alone
    let held = ok(running.ask(json!({"op": "cert-key", "id": id, "passphrase": b64("a password")})));
    assert!(String::from_utf8_lossy(&decode(&held["key"])).contains("ENCRYPTED PRIVATE KEY"));
    assert_eq!(held["pem"].as_str().unwrap().matches("BEGIN CERTIFICATE").count(), 3);
    assert_eq!(running.ask(json!({"op": "cert-key", "id": id, "plain": true}))["problem"], json!("wrong-purpose"));
    let tls = ok(new_cert(&running, json!({"profile": "local-tls", "hosts": ["printer.local"]})));
    let held = ok(running.ask(json!({"op": "cert-key", "id": tls["id"], "plain": true})));
    assert!(String::from_utf8_lossy(&decode(&held["key"])).contains("BEGIN PRIVATE KEY"));
}

/// pki cannot see a key certified twice, so the service refuses it: a job process that hands the same key out again (a stand-in that
/// repeats its first answer) gets nothing kept
#[test]
fn a_key_certified_once_is_never_certified_again() {
    use std::os::unix::fs::PermissionsExt;
    let dir = common::Folder::new("replay");
    let cache = dir.0.join("first-issue");
    let script = dir.0.join("worker.sh");
    std::fs::write(&script, format!(
        "#!/bin/sh
REAL={real}
CACHE={cache}
in=$(cat)
case \"$in\" in
*'\"job\":\"issue\"'*)
 if [ -f \"$CACHE\" ]; then cat \"$CACHE\"; exit 0; fi
 printf '%s' \"$in\" | \"$REAL\" > \"$CACHE.tmp\" && cat \"$CACHE.tmp\" && mv \"$CACHE.tmp\" \"$CACHE\";;
*) printf '%s' \"$in\" | \"$REAL\";;
esac
",
        real = env!("CARGO_BIN_EXE_amethystora-vault-work"),
        cache = cache.display()
    )).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let running = common::start_worker("replay-run", 10, Duration::from_secs(600), script);
    running.set_up(PASSWORD);
    as_ada(&running);
    let first = ok(new_cert(&running, json!({"profile": "pdf", "key_type": "ecdsa-p256"})));
    // The same answer again, key and all: refused, and kept nowhere
    let again = new_cert(&running, json!({"profile": "pdf", "key_type": "ecdsa-p256"}));
    assert_eq!(again["ok"], json!(false), "{again}");
    assert_eq!(again["problem"], json!("key-reused"), "{again}");
    // Nor under another profile of the same CA
    let code = new_cert(&running, json!({"profile": "code", "key_type": "ecdsa-p256"}));
    assert_eq!(code["problem"], json!("key-reused"), "{code}");
    let listed = ok(running.ask(json!({"op": "cert-list"})));
    assert_eq!(listed["items"].as_array().unwrap().len(), 1, "{listed}");
    assert_eq!(listed["items"][0]["id"], first["id"]);
}

#[test]
fn renewing_keeps_the_old_one_marked() {
    let running = opened("renew");
    as_ada(&running);
    let made = ok(new_cert(&running, json!({"profile": "smime", "email": "ada@example.org", "title": "Mail", "key_type": "ecdsa-p256"})));
    let renewed = ok(running.ask(json!({"op": "cert-renew", "id": made["id"]})));
    assert_ne!(renewed["id"], made["id"]);
    assert_eq!(asked(&running).last().unwrap(), &(Action::Use, say::ISSUE_CERT, "Mail".to_string()));
    let items = ok(running.ask(json!({"op": "cert-list"})))["items"].as_array().unwrap().clone();
    assert_eq!(items.len(), 2);
    let old = items.iter().find(|item| item["id"] == made["id"]).unwrap();
    let new = items.iter().find(|item| item["id"] == renewed["id"]).unwrap();
    assert_eq!(old["meta"]["renewed_by"], renewed["id"]);
    assert_eq!(new["meta"]["renews"], made["id"]);
    assert_eq!(new["name"], json!("Mail"));
    assert_ne!(old["meta"]["sha256"], new["meta"]["sha256"]);
    assert_eq!(new["meta"]["key_type"], json!("ecdsa-p256"));
    assert_eq!(why(&running.ask(json!({"op": "cert-renew", "id": "nothing"}))), "refused");
}

#[test]
fn a_digest_is_signed_for_the_certificate_s_own_purpose_alone() {
    let running = opened("sign");
    as_ada(&running);
    let dir = common::Folder::new("verify-sign");
    let digest = [7u8; 32];
    let pdf = ok(new_cert(&running, json!({"profile": "pdf", "key_type": "ecdsa-p256", "title": "PDF"})));
    let code = ok(new_cert(&running, json!({"profile": "code", "key_type": "rsa-3072", "title": "Programs"})));
    let mail = ok(new_cert(&running, json!({"profile": "smime", "email": "a@example.org", "key_type": "ecdsa-p256"})));
    let sign = |id: &Value, purpose: &str| running.ask(json!({"op": "sign-digest", "id": id, "purpose": purpose, "digest": "sha256", "value": encode(&digest)}));

    // Its own purpose alone, refused before anyone is asked
    let prompts = running.outside.asked();
    assert_eq!(sign(&pdf["id"], "code")["problem"], json!("wrong-purpose"));
    assert_eq!(sign(&code["id"], "pdf")["problem"], json!("wrong-purpose"));
    assert_eq!(sign(&mail["id"], "pdf")["problem"], json!("wrong-purpose"));
    assert_eq!(why(&sign(&json!("nothing"), "pdf")), "refused");
    assert_eq!(running.ask(json!({"op": "sign-digest", "id": pdf["id"], "purpose": "pdf", "digest": "sha384", "value": encode(&digest)}))["problem"], json!("bad-digest"));
    assert_eq!(running.ask(json!({"op": "sign-digest", "id": pdf["id"], "purpose": "pdf", "digest": "sha256", "value": encode(&[1u8; 31])}))["problem"], json!("bad-digest"));
    assert_eq!(running.outside.asked(), prompts);

    // A no makes no signature, and each signature asks: a private key's default is every time
    refuse_prompts(&running, true);
    assert_eq!(why(&sign(&pdf["id"], "pdf")), "refused");
    refuse_prompts(&running, false);
    assert_eq!(asked(&running).last().unwrap(), &(Action::Use, say::SIGN, "PDF".to_string()));
    for (certificate, purpose) in [(&pdf, "pdf"), (&code, "code")] {
        let answer = ok(sign(&certificate["id"], purpose));
        let chain = chain_of(&running, &certificate["id"]);
        let leaf = chain.split_inclusive("-----END CERTIFICATE-----\n").next().unwrap();
        std::fs::write(dir.0.join("leaf.pem"), leaf).unwrap();
        std::fs::write(dir.0.join("signature"), decode(&answer["signature"])).unwrap();
        let path = |name: &str| dir.0.join(name).to_str().unwrap().to_string();
        let (_, public) = openssl(&["x509", "-pubkey", "-noout", "-in", &path("leaf.pem")], None);
        std::fs::write(dir.0.join("public.pem"), public).unwrap();
        let (good, said) = openssl(&["pkeyutl", "-verify", "-pubin", "-inkey", &path("public.pem"), "-sigfile", &path("signature"), "-pkeyopt", "digest:sha256"], Some(&digest));
        assert!(good, "{purpose}: {said}");
    }
    assert_eq!(running.outside.asked(), prompts + 3);
    // The setting is the person's to lower
    ok(running.ask(json!({"op": "set-approval", "kind": "certificate", "id": pdf["id"], "approval": "while-open"})));
    let prompts = running.outside.asked();
    ok(sign(&pdf["id"], "pdf"));
    ok(sign(&pdf["id"], "pdf"));
    assert_eq!(running.outside.asked(), prompts + 1);
}

fn unsigned_pdf() -> Vec<u8> {
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for text in ["1 0 obj\n<</Type/Catalog/Pages 2 0 R>>\nendobj\n", "2 0 obj\n<</Type/Pages/Kids[3 0 R]/Count 1>>\nendobj\n", "3 0 obj\n<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]>>\nendobj\n"] {
        offsets.push(pdf.len());
        pdf.extend_from_slice(text.as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(b"xref\n0 4\n0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(format!("trailer\n<</Size 4/Root 1 0 R>>\nstartxref\n{xref}\n%%EOF\n").as_bytes());
    pdf
}

/// The module the image ships, with the socket behind it: built by `cargo build -p amethystora-vault-pkcs11`. Not the one built with
/// `test-backend` (the module's own tests rebuild the same file), which asks a folder for its keys and would list none.
fn module() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let deps = exe.parent()?;
    [deps.to_path_buf(), deps.parent()?.to_path_buf()].into_iter().map(|folder| folder.join("libamethystora_vault_pkcs11.so")).find(|path| std::fs::read(path).is_ok_and(|bytes| !bytes.windows(TEST_BACKEND.len()).any(|window| window == TEST_BACKEND)))
}

const TEST_BACKEND: &[u8] = b"AMETHYSTORA_VAULT_PKCS11_TEST";

fn has(program: &str) -> bool {
    Command::new(program).arg("-h").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().is_ok()
}

/// `ame vault pdf sign`: the command line, pdfsig and the module, the key never out of the service, read back by a reader that trusts
/// only the machine's root for documents
#[test]
fn a_pdf_is_signed_through_the_module_with_the_key_staying_here() {
    let (Some(module), true) = (module(), has("pdfsig") && has("certutil") && has("modutil")) else {
        // The build builds the module before its tests (build-apps.sh): a missing one there is a mistake, not a skip
        assert!(std::env::var_os("VAULT_VERIFIERS_REQUIRED").is_none(), "needs the real module built (cargo build -p amethystora-vault-pkcs11), pdfsig and NSS's tools");
        eprintln!("skipped: needs the module built, pdfsig and NSS's tools");
        return;
    };
    let running = opened("pdf");
    as_ada(&running);
    let made = ok(new_cert(&running, json!({"profile": "pdf", "title": "Contracts", "key_type": "rsa-3072"})));
    // A code certificate is never offered to pdfsig
    ok(new_cert(&running, json!({"profile": "code", "key_type": "ecdsa-p256"})));
    let dir = common::Folder::new("pdf-sign");
    let input = dir.0.join("-contract.pdf");
    std::fs::write(&input, unsigned_pdf()).unwrap();
    let prompts = running.outside.asked();
    let cli = env!("CARGO_BIN_EXE_amethystora-vault-cli");
    let home = common::Folder::new("pdf-sign-home");
    let out = Command::new(cli).args(["pdf", "sign"]).arg(&input).arg(format!("--module={}", module.display())).env("AMETHYSTORA_VAULT_SOCKET", &running.socket).env("XDG_RUNTIME_DIR", &dir.0).env("HOME", &home.0).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let signed = std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    assert_eq!(signed, dir.0.join("-contract (signed).pdf"));
    assert_eq!(running.outside.asked(), prompts + 1, "one signature, one question");
    assert_eq!(asked(&running).last().unwrap(), &(Action::Use, say::SIGN, "Contracts".to_string()));
    // The scratch database is gone, and nothing of the key is on the person's side
    let left: Vec<_> = std::fs::read_dir(&dir.0).unwrap().flatten().map(|entry| entry.file_name()).collect();
    assert_eq!(left.len(), 2, "{left:?}");
    // The machine's root went into the account's own database, for signatures only, once
    let trusted = Command::new("certutil").args(["-L", "-d", &format!("sql:{}", home.0.join(".pki/nssdb").display())]).output().unwrap();
    let trusted = String::from_utf8_lossy(&trusted.stdout);
    assert!(trusted.lines().any(|line| line.starts_with("Amethystora OS Root CA") && line.trim_end().ends_with(",C,")), "{trusted}");

    let reader = dir.0.join("reader");
    std::fs::create_dir_all(&reader).unwrap();
    let db = format!("sql:{}", reader.display());
    assert!(Command::new("certutil").args(["-N", "--empty-password", "-d", &db]).status().unwrap().success());
    std::fs::write(dir.0.join("root.pem"), ok(running.ask(json!({"op": "ca-cert", "which": "root"})))["pem"].as_str().unwrap()).unwrap();
    assert!(Command::new("certutil").args(["-A", "-d", &db, "-n", "root", "-t", ",C,", "-a", "-i"]).arg(dir.0.join("root.pem")).status().unwrap().success());
    let said = Command::new("pdfsig").args(["-nssdir", &db, "-no-ocsp"]).arg(&signed).output().unwrap();
    let said = String::from_utf8_lossy(&said.stdout).into_owned();
    assert!(said.contains("Signature is Valid"), "{said}");
    assert!(said.contains("Certificate is Trusted"), "{said}");
    assert!(said.contains("Ada Lovelace"), "{said}");
    let _ = made;

    // A no from the person fails the signing, and leaves no half-made file
    refuse_prompts(&running, true);
    let out = Command::new(cli).args(["pdf", "sign"]).arg(&input).arg(format!("--module={}", module.display())).env("AMETHYSTORA_VAULT_SOCKET", &running.socket).env("XDG_RUNTIME_DIR", &dir.0).output().unwrap();
    assert!(!out.status.success());
    assert!(!dir.0.join("-contract (signed 2).pdf").exists());
    refuse_prompts(&running, false);
    // A closed vault offers pdfsig nothing
    ok(running.ask(json!({"op": "lock"})));
    let out = Command::new(cli).args(["pdf", "sign"]).arg(&input).arg(format!("--module={}", module.display())).env("AMETHYSTORA_VAULT_SOCKET", &running.socket).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn removing_goes_through_the_generic_op_and_always_asks() {
    let running = opened("remove");
    as_ada(&running);
    let id = add_account(&running, "Example");
    let key = ok(running.ask(json!({"op": "key-new", "name": "k", "key_kind": "ssh"})));
    let cert = ok(new_cert(&running, json!({"profile": "smime", "email": "a@example.org", "key_type": "ecdsa-p256"})));
    for (kind, id) in [("authenticator", json!(id)), ("key", key["id"].clone()), ("certificate", cert["id"].clone())] {
        let before = running.outside.asked();
        // Only the first is refused by the person: after three noes in a minute a program is not put to them again (core's limit)
        let said_no = kind == "authenticator";
        if said_no {
            refuse_prompts(&running, true);
            assert_eq!(why(&running.ask(json!({"op": "remove", "kind": kind, "id": id}))), "refused");
            refuse_prompts(&running, false);
        }
        ok(running.ask(json!({"op": "remove", "kind": kind, "id": id})));
        assert_eq!(running.outside.asked(), before + if said_no { 2 } else { 1 });
        assert_eq!(asked(&running).last().unwrap().1, say::REMOVE_ITEM);
    }
    assert_eq!(ok(running.ask(json!({"op": "auth-list"})))["items"].as_array().unwrap().len(), 0);
    assert_eq!(why(&running.ask(json!({"op": "key-public", "id": key["id"]}))), "refused");
    assert_eq!(why(&running.ask(json!({"op": "cert-show", "id": cert["id"]}))), "refused");
}

/// The first vault's name stays what `valid_name` accepts: an account's name from an import may hold anything
#[test]
fn a_hostile_name_from_an_import_is_only_data() {
    let running = opened("names");
    let shown = "x\u{1b}[31mred\ninjected $(program)";
    let answer = ok(running.ask(json!({"op": "import-merge", "accounts": [account_json(shown, SECRET)]})));
    assert_eq!(answer["added"], json!(1));
    let item = ok(running.ask(json!({"op": "auth-list"})))["items"][0].clone();
    let name = item["name"].as_str().unwrap();
    assert!(!name.chars().any(char::is_control), "{name:?}");
    ok(running.ask(json!({"op": "auth-code", "id": item["id"]})));
    assert_eq!(asked(&running).last().unwrap().1, say::USE_ITEM, "the sentence is fixed, whatever it fills in");
}

/// Run by the test below, as another uid, against the socket and items it names
#[test]
#[ignore = "the other account's half of a_second_account_reaches_nothing"]
fn other_account_items() {
    let socket = std::path::PathBuf::from(std::env::var("OTHER_SOCKET").unwrap());
    let requests: Vec<Value> = serde_json::from_str(&std::env::var("OTHER_REQUESTS").unwrap()).unwrap();
    let ask = |mut request: Value| {
        request["v"] = json!(1);
        amethystora_vault_client::exchange(&socket, &request, Duration::from_secs(120)).unwrap()
    };
    for request in requests.clone() {
        let answer = ask(request.clone());
        assert_eq!(why(&answer), "not-set-up", "{request}: {answer}");
    }
    // With a vault of its own, the first account's ids are simply not there
    let recovery = ok(ask(json!({"op": "setup", "method": "password", "secret": b64("the other password")})))["recovery"].as_str().unwrap().to_string();
    ok(ask(json!({"op": "confirm", "recovery": b64(&recovery)})));
    for request in requests.into_iter().filter(|request| request["op"] != json!("auth-list") && request["op"] != json!("ca-cert")) {
        let answer = ask(request.clone());
        assert_eq!(why(&answer), "refused", "{request}: {answer}");
    }
    assert_eq!(ok(ask(json!({"op": "auth-list"})))["items"], json!([]));
    assert_eq!(why(&ask(json!({"op": "ca-cert", "which": "account"}))), "refused");
    // A certificate of its own: another CA, under the same root, which stays the one the first account made
    let made = ok(ask(json!({"op": "cert-new", "profile": "smime", "title": "t", "email": "o@example.org", "key_type": "ecdsa-p256"})));
    assert_eq!(made["new_ca"], json!(true));
    let ca = ok(ask(json!({"op": "ca-cert", "which": "account"})));
    assert_ne!(ca["sha256"].as_str().unwrap(), std::env::var("OTHER_CA").unwrap());
    assert_eq!(ok(ask(json!({"op": "machine"})))["sha256"].as_str().unwrap(), std::env::var("OTHER_ROOT").unwrap());
}

/// Only with root, as socket.rs's own: a second account really is another uid on the socket
#[test]
fn a_second_account_reaches_nothing() {
    // SAFETY: geteuid cannot fail
    if unsafe { libc::geteuid() } != 0 {
        eprintln!("skipped: needs root to be a second account");
        return;
    }
    let running = opened("second");
    as_ada(&running);
    let account = add_account(&running, "Example");
    let key = ok(running.ask(json!({"op": "key-new", "name": "k", "key_kind": "ssh"})));
    let cert = ok(new_cert(&running, json!({"profile": "smime", "email": "a@example.org", "key_type": "ecdsa-p256"})));
    let password = ok(running.ask(json!({"op": "put", "kind": "password", "name": "n", "value": {"password": "p"}})));
    let requests = vec![
        json!({"op": "auth-code", "id": account}),
        json!({"op": "auth-link", "id": account}),
        json!({"op": "auth-list"}),
        json!({"op": "key-public", "id": key["id"]}),
        json!({"op": "key-export", "id": key["id"], "plain": true}),
        json!({"op": "cert-show", "id": cert["id"]}),
        json!({"op": "cert-chain", "id": cert["id"]}),
        json!({"op": "cert-pfx", "id": cert["id"], "password": b64("x")}),
        json!({"op": "sign-digest", "id": cert["id"], "purpose": "pdf", "digest": "sha256", "value": encode(&[0u8; 32])}),
        json!({"op": "password-copy", "id": password["id"]}),
        json!({"op": "ca-cert", "which": "account"}),
    ];
    let ca = ok(running.ask(json!({"op": "ca-cert", "which": "account"})));
    let root = ok(running.ask(json!({"op": "machine"})));
    use std::os::unix::process::CommandExt;
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "other_account_items", "--nocapture"])
        .env("OTHER_SOCKET", &running.socket)
        .env("OTHER_REQUESTS", Value::Array(requests).to_string())
        .env("OTHER_CA", ca["sha256"].as_str().unwrap())
        .env("OTHER_ROOT", root["sha256"].as_str().unwrap())
        .uid(1234)
        .gid(1234)
        .status()
        .unwrap();
    assert!(status.success());
    // The first account's is as it was
    assert!(ok(running.ask(json!({"op": "auth-code", "id": account})))["code"].is_string());
    assert_eq!(ok(running.ask(json!({"op": "ca-cert", "which": "account"})))["sha256"], ca["sha256"]);
}

// --- Budgets, run by hand: `cargo test --release --test items -- --ignored --nocapture budgets` ------------------------------

fn resident_kb(pid: u32) -> u64 {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
    status.lines().find(|line| line.starts_with("VmRSS")).unwrap().split_whitespace().nth(1).unwrap().parse().unwrap()
}

#[test]
#[ignore = "measures in-process, by hand"]
fn budgets_in_process() {
    let running = opened("budgets");
    let many: Vec<Value> = (0..250).map(|at| account_json(&format!("Site {at}"), SECRET)).collect();
    ok(running.ask(json!({"op": "import-merge", "accounts": many})));
    let id = ok(running.ask(json!({"op": "auth-list"})))["items"][0]["id"].clone();
    let request = format!("{}\n", json!({"v": 1, "op": "auth-code", "id": id}));
    use std::io::{BufRead, BufReader, Write};
    let stream = std::os::unix::net::UnixStream::connect(&running.socket).unwrap();
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let mut times = Vec::new();
    for _ in 0..1000 {
        let started = std::time::Instant::now();
        writer.write_all(request.as_bytes()).unwrap();
        line.clear();
        reader.read_line(&mut line).unwrap();
        times.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    println!("auth-code: median {:.3} ms, slowest {:.3} ms, with this process at {} kB", times[500], times[999], resident_kb(std::process::id()));
}

/// `ame vault tls trust browsers on|off`: the Local Services CA, for websites only (`C,,`), in the account's own database, and gone again
#[test]
fn the_local_services_ca_is_trusted_by_browsers_only_when_asked() {
    if !has("certutil") {
        assert!(std::env::var_os("VAULT_VERIFIERS_REQUIRED").is_none(), "certutil is not installed");
        return;
    }
    let running = opened("tls-trust");
    as_ada(&running);
    ok(new_cert(&running, json!({"profile": "local-tls", "hosts": ["printer.local"]})));
    let home = common::Folder::new("tls-trust-home");
    let dir = common::Folder::new("tls-trust-run");
    let cli = env!("CARGO_BIN_EXE_amethystora-vault-cli");
    let run = |state: &str| Command::new(cli).args(["tls", "trust", "browsers", state]).env("AMETHYSTORA_VAULT_SOCKET", &running.socket).env("XDG_RUNTIME_DIR", &dir.0).env("HOME", &home.0).output().unwrap();
    let listed = || {
        let out = Command::new("certutil").args(["-L", "-d", &format!("sql:{}", home.0.join(".pki/nssdb").display())]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let on = run("on");
    assert!(on.status.success(), "{}", String::from_utf8_lossy(&on.stderr));
    assert!(listed().lines().any(|line| line.starts_with("Amethystora Local Services CA") && line.trim_end().ends_with("C,,")), "{}", listed());
    // Twice leaves one, and the root is never put there for websites
    assert!(run("on").status.success());
    assert_eq!(listed().lines().filter(|line| line.starts_with("Amethystora Local Services CA")).count(), 1);
    assert!(!listed().contains("Root CA"));
    assert!(run("off").status.success());
    assert!(!listed().contains("Local Services CA"), "{}", listed());
    assert_eq!(run("sideways").status.code(), Some(64));
}
