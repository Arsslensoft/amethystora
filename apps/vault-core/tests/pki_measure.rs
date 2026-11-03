//! Time and resident memory of each operation, for the service's budgets (8 MB while open). Each runs in a process of its
//! own, so that its peak is its own: `cargo test --release -p amethystora-vault-core --test pki_measure -- --ignored --nocapture`.

use std::process::Command;
use std::time::Instant;

use amethystora_vault_core::keys::{generate, import_openssh, KeyType, PrivateKey};
use amethystora_vault_core::pki::*;

const NOW: i64 = 1793577600;

fn memory(field: &str) -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let line = status.lines().find(|line| line.starts_with(field)).unwrap();
    line.split_whitespace().nth(1).unwrap().parse().unwrap()
}

fn pdf_leaf(key: &PrivateKey, ca_key: &CaKey, ca: &Issued) -> Issued {
    let spki = key.public().unwrap().spki_der;
    issue(
        &Request { profile: "pdf", public_key: &spki, now: NOW, name: Some("Ada"), ..Request::default() },
        &Issuer { authority: Authority::AccountCa, cert: &ca.cert, key: ca_key, chain: &ca.chain },
    )
    .unwrap()
}

/// What one operation does, its setup done before the clock starts
fn prepare(name: &str) -> Box<dyn FnMut()> {
    let key_of = |key_type| generate(key_type).unwrap();
    if let Some(type_name) = name.strip_prefix("generate ") {
        let key_type = KeyType::parse(type_name).unwrap();
        return Box::new(move || {
            std::hint::black_box(generate(key_type).unwrap());
        });
    }
    match name {
        "public forms rsa-4096" => {
            let key = key_of(KeyType::Rsa4096);
            return Box::new(move || {
                std::hint::black_box(key.public().unwrap());
            });
        }
        "export encrypted pkcs8" => {
            let key = key_of(KeyType::Ed25519);
            return Box::new(move || {
                std::hint::black_box(key.export_encrypted(b"a passphrase").unwrap());
            });
        }
        "import openssh, 16 rounds" | "import openssh, 24 rounds" => {
            let rounds = if name.contains("16") { "16" } else { "24" };
            let dir = std::env::temp_dir().join(format!("amethystora-vault-measure-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let file = dir.join("id");
            Command::new("ssh-keygen").args(["-t", "ed25519", "-q", "-a", rounds, "-N", "open sesame", "-f"]).arg(&file).status().unwrap();
            let bytes = std::fs::read(&file).unwrap();
            std::fs::remove_dir_all(&dir).unwrap();
            return Box::new(move || {
                std::hint::black_box(import_openssh(&bytes, b"open sesame").unwrap());
            });
        }
        "make root, key given" => {
            let key = CaKey::generate(Authority::Root).unwrap();
            return Box::new(move || {
                std::hint::black_box(make_root(&key, NOW).unwrap());
            });
        }
        _ => {}
    }
    let root_key = CaKey::generate(Authority::Root).unwrap();
    let root = make_root(&root_key, NOW).unwrap();
    let ca_key = CaKey::generate(Authority::AccountCa).unwrap();
    let ca_spki = ca_key.public().unwrap().spki_der;
    let ca = issue(
        &Request { profile: "account-ca", public_key: &ca_spki, now: NOW, name: Some("Ada"), ..Request::default() },
        &Issuer { authority: Authority::Root, cert: &root, key: &root_key, chain: &[] },
    )
    .unwrap();
    let ec_key = key_of(KeyType::EcdsaP256);
    let rsa_key = key_of(KeyType::Rsa3072);
    let rsa_leaf = pdf_leaf(&rsa_key, &ca_key, &ca);
    let ec_leaf = pdf_leaf(&ec_key, &ca_key, &ca);
    let digest = [7u8; 32];
    let name = name.to_string();
    Box::new(move || match name.as_str() {
        "issue account ca" => {
            let spki = rsa_key.public().unwrap().spki_der;
            let issuer = Issuer { authority: Authority::Root, cert: &root, key: &root_key, chain: &[] };
            std::hint::black_box(issue(&Request { profile: "account-ca", public_key: &spki, now: NOW, name: Some("Ada"), ..Request::default() }, &issuer).unwrap());
        }
        "issue pdf" => {
            std::hint::black_box(pdf_leaf(&ec_key, &ca_key, &ca));
        }
        "export pfx" => {
            std::hint::black_box(pfx(&rsa_key, &rsa_leaf.cert, &rsa_leaf.chain, "Ada", "a password").unwrap());
        }
        "sign pdf digest rsa" => {
            let by = Signer::leaf(&rsa_key, &rsa_leaf.cert, Purpose::Pdf, NOW).unwrap();
            std::hint::black_box(sign_pdf_digest(&by, &rsa_leaf.chain, Digest::Sha256, &digest, NOW).unwrap());
        }
        "sign pdf digest ecdsa" => {
            let by = Signer::leaf(&ec_key, &ec_leaf.cert, Purpose::Pdf, NOW).unwrap();
            std::hint::black_box(sign_pdf_digest(&by, &ec_leaf.chain, Digest::Sha256, &digest, NOW).unwrap());
        }
        "sign digest rsa" => {
            let by = Signer::leaf(&rsa_key, &rsa_leaf.cert, Purpose::Pdf, NOW).unwrap();
            std::hint::black_box(sign_digest(&by, Digest::Sha256, &digest).unwrap());
        }
        "sign digest ecdsa" => {
            let by = Signer::leaf(&ec_key, &ec_leaf.cert, Purpose::Pdf, NOW).unwrap();
            std::hint::black_box(sign_digest(&by, Digest::Sha256, &digest).unwrap());
        }
        other => panic!("{other}"),
    })
}

#[test]
#[ignore]
fn measure() {
    if let Ok(name) = std::env::var("VAULT_MEASURE") {
        let mut operation = prepare(&name);
        let before = memory("VmRSS");
        let mut times = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            operation();
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        println!("| {name} | {:.2} | {:.2} | {:.2} | {before} | {} | {} |", times[0], times[2], times[4], memory("VmRSS"), memory("VmHWM"));
        return;
    }
    println!("| Operation | min ms | median ms | max ms | RSS before, kB | RSS after, kB | peak RSS, kB |");
    for name in [
        "generate ed25519",
        "generate ecdsa-p256",
        "generate ecdsa-p384",
        "generate x25519",
        "generate aes-256",
        "generate ml-dsa-65",
        "generate ml-kem-768",
        "generate rsa-3072",
        "generate rsa-4096",
        "public forms rsa-4096",
        "export encrypted pkcs8",
        "import openssh, 16 rounds",
        "import openssh, 24 rounds",
        "make root, key given",
        "issue account ca",
        "issue pdf",
        "export pfx",
        "sign digest rsa",
        "sign digest ecdsa",
        "sign pdf digest rsa",
        "sign pdf digest ecdsa",
    ] {
        let out = Command::new(std::env::current_exe().unwrap()).args(["--ignored", "--exact", "measure", "--nocapture"]).env("VAULT_MEASURE", name).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let lines: Vec<&str> = text.lines().filter(|line| line.starts_with('|')).collect();
        println!("{}", if lines.is_empty() { format!("{name} failed: {text}") } else { lines.join("\n") });
    }
}
