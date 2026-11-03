//! The job process on its own (`amethystora-vault-work`), fed what the service feeds it: the one place a signature is made, so the
//! one place that must refuse a key that is not a leaf's own. A CA's or the root's key is a type of its own in `pki` and is not even
//! readable as a leaf's key here; these hold that, and the dates and the purpose, from outside the service's checks.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

use amethystora_vault_core::keys::{self, KeyType, PrivateKey};
use amethystora_vault_core::pki::{self, Authority, CaKey, Issuer, Request};
use amethystora_vault_core::protocol::encode;
use serde_json::{json, Value};

const WORKER: &str = env!("CARGO_BIN_EXE_amethystora-vault-work");

fn run(request: &Value) -> Value {
    let mut child = Command::new(WORKER).env_clear().stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(request.to_string().as_bytes()).unwrap();
    let mut out = String::new();
    child.stdout.take().unwrap().read_to_string(&mut out).unwrap();
    assert!(child.wait().unwrap().success());
    serde_json::from_str(&out).unwrap()
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

struct Machine {
    root: CaKey,
    root_cert: Vec<u8>,
    ca: CaKey,
    ca_cert: Vec<u8>,
}

fn machine() -> Machine {
    let at = now();
    let root = CaKey::generate(Authority::Root).unwrap();
    let root_cert = pki::make_root(&root, at).unwrap();
    let ca = CaKey::generate(Authority::AccountCa).unwrap();
    let spki = ca.public().unwrap().spki_der;
    let request = Request { profile: "account-ca", public_key: &spki, name: Some("Ada"), now: at, ..Request::default() };
    let ca_cert = pki::issue(&request, &Issuer { authority: Authority::Root, cert: &root_cert, key: &root, chain: &[] }).unwrap().cert;
    Machine { root, root_cert, ca, ca_cert }
}

fn leaf(machine: &Machine, profile: &str) -> (PrivateKey, Vec<u8>) {
    let key = keys::generate(KeyType::EcdsaP256).unwrap();
    let spki = key.public().unwrap().spki_der;
    let chain = [machine.root_cert.clone()];
    let request = Request { profile, public_key: &spki, name: Some("Ada"), now: now(), ..Request::default() };
    let issued = pki::issue(&request, &Issuer { authority: Authority::AccountCa, cert: &machine.ca_cert, key: &machine.ca, chain: &chain }).unwrap();
    (key, issued.cert)
}

fn sign(key: &[u8], cert: &[u8], purpose: &str, at: i64) -> Value {
    run(&json!({"job": "sign", "type": "ecdsa-p256", "key": encode(key), "cert": encode(cert), "purpose": purpose, "now": at, "digest": "sha256", "value": encode(&[7u8; 32])}))
}

fn problem(answer: &Value) -> &str {
    assert_eq!(answer["ok"], json!(false), "{answer}");
    answer["problem"].as_str().unwrap_or("")
}

#[test]
fn only_a_leafs_own_key_signs_and_only_for_its_own_purpose_within_its_dates() {
    let machine = machine();
    let (pdf_key, pdf_cert) = leaf(&machine, "pdf");
    let (code_key, code_cert) = leaf(&machine, "code");
    let at = now();

    let signed = sign(pdf_key.as_bytes(), &pdf_cert, "pdf", at);
    assert_eq!(signed["ok"], json!(true), "{signed}");
    assert!(!signed["signature"].as_str().unwrap().is_empty());
    assert_eq!(sign(code_key.as_bytes(), &code_cert, "code", at)["ok"], json!(true));

    // Another purpose than the certificate's own, and one that is none
    assert_eq!(problem(&sign(pdf_key.as_bytes(), &pdf_cert, "code", at)), "wrong-purpose");
    assert_eq!(problem(&sign(code_key.as_bytes(), &code_cert, "pdf", at)), "wrong-purpose");
    assert_eq!(problem(&sign(pdf_key.as_bytes(), &pdf_cert, "smime", at)), "wrong-purpose");

    // The CA's key, and the root's, as the service stores them: not a leaf's key by type, so not read as one at all
    for (stored, cert) in [(machine.ca.to_stored(), &machine.ca_cert), (machine.root.to_stored(), &machine.root_cert)] {
        let answer = sign(&stored, cert, "pdf", at);
        assert_eq!(answer["ok"], json!(false), "{answer}");
        assert!(answer.get("signature").is_none());
    }
    // A CA's certificate with a leaf's key, a leaf's certificate with another's key: the key has to be the certificate's own
    assert_eq!(sign(pdf_key.as_bytes(), &machine.ca_cert, "pdf", at)["ok"], json!(false));
    assert_eq!(sign(code_key.as_bytes(), &pdf_cert, "pdf", at)["ok"], json!(false));

    // Before it starts and after it ends
    assert_eq!(problem(&sign(pdf_key.as_bytes(), &pdf_cert, "pdf", at - 3 * 86400)), "expired");
    assert_eq!(problem(&sign(pdf_key.as_bytes(), &pdf_cert, "pdf", at + 20 * 365 * 86400)), "expired");

    // A digest of the wrong length, and a hash that is not allowed
    let short = run(&json!({"job": "sign", "type": "ecdsa-p256", "key": encode(pdf_key.as_bytes()), "cert": encode(&pdf_cert), "purpose": "pdf", "now": at, "digest": "sha256", "value": encode(&[7u8; 31])}));
    assert_eq!(problem(&short), "bad-digest");
    let md5 = run(&json!({"job": "sign", "type": "ecdsa-p256", "key": encode(pdf_key.as_bytes()), "cert": encode(&pdf_cert), "purpose": "pdf", "now": at, "digest": "md5", "value": encode(&[7u8; 16])}));
    assert_eq!(problem(&md5), "digest-not-allowed");
}

#[test]
fn nothing_odd_panics_the_worker_or_leaves_it_waiting() {
    for request in [json!({}), json!({"job": 3}), json!({"job": "sign"}), json!({"job": "no-such"}), json!({"job": "sign", "key": 5, "cert": [], "purpose": null, "now": "x"}), json!({"job": "issue"}), json!({"job": "make-ca", "root": 7})] {
        let answer = run(&request);
        assert_eq!(answer["ok"], json!(false), "{request}");
    }
    // Not even JSON
    let mut child = Command::new(WORKER).env_clear().stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"\xff\xfe not json").unwrap();
    let mut out = Vec::new();
    child.stdout.take().unwrap().read_to_end(&mut out).unwrap();
    assert!(!child.wait().unwrap().success());
    assert!(out.is_empty());
}
