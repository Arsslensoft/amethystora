//! Keys against programs that are not this crate: `openssl` and `ssh-keygen`, as separate programs (plans/done/PLAN-vault.md,
//! Tests). See `common` for what happens when one is not installed.

mod common;

use amethystora_vault_core::keys::{generate, import_openssh, KeyError, KeyKind, KeyType, PrivateKey, PublicForms};
use common::{need, output, run, run_in, Scratch};
use std::process::Command;

const OFFERED: [KeyType; 9] = [
    KeyType::Ed25519,
    KeyType::EcdsaP256,
    KeyType::EcdsaP384,
    KeyType::Rsa3072,
    KeyType::Rsa4096,
    KeyType::MlDsa65,
    KeyType::X25519,
    KeyType::MlKem768,
    KeyType::Aes256,
];

#[test]
fn every_type_makes_a_key_that_reads_back() {
    for key_type in OFFERED {
        if !key_type.available() {
            eprintln!("{} is not in this OpenSSL", key_type.name());
            assert!(matches!(key_type, KeyType::MlDsa65 | KeyType::MlKem768));
            continue;
        }
        let key = generate(key_type).unwrap();
        assert_eq!(key.key_type(), key_type);
        let again = PrivateKey::from_bytes(key_type, key.as_bytes()).unwrap();
        assert_eq!(again.as_bytes(), key.as_bytes());
        assert!(PrivateKey::from_bytes(if key_type == KeyType::Ed25519 { KeyType::X25519 } else { KeyType::Ed25519 }, key.as_bytes()).is_err());
        assert!(!format!("{key:?}").contains(&format!("{:?}", &key.as_bytes()[..4])));
    }
    assert!(generate(KeyType::RsaOther).is_err());
    assert!(generate(KeyType::EcdsaP521).is_err());
}

#[test]
fn the_offers_follow_the_table() {
    assert_eq!(KeyKind::Ssh.types()[0], KeyType::Ed25519);
    assert_eq!(KeyKind::Signing.types()[0], KeyType::Ed25519);
    assert_eq!(KeyKind::Exchange.types()[0], KeyType::X25519);
    assert_eq!(KeyKind::Symmetric.types(), [KeyType::Aes256]);
    assert!(!KeyKind::Ssh.types().contains(&KeyType::MlDsa65));
}

#[test]
fn public_forms_agree_with_openssl_and_ssh_keygen() {
    if !need("openssl") || !need("ssh-keygen") {
        return;
    }
    let scratch = Scratch::new("public");
    for key_type in OFFERED {
        if !key_type.available() || key_type == KeyType::Aes256 {
            continue;
        }
        let key = generate(key_type).unwrap();
        let forms = key.public().unwrap();
        assert_eq!(PublicForms::from_spki(&forms.spki_der).unwrap(), forms);
        let pem = scratch.write("public.pem", forms.spki_pem.as_bytes());
        let text = output(Command::new("openssl").args(["pkey", "-pubin", "-noout", "-text", "-in"]).arg(&pem));
        assert!(!text.is_empty(), "{}", key_type.name());
        let der = scratch.write("public.der", &forms.spki_der);
        let sum = output(Command::new("sha256sum").arg(&der));
        assert_eq!(sum.split_whitespace().next().unwrap(), forms.sha256);
        // An OpenSSH line only where SSH has one, and ssh-keygen reads it as the same key
        match key_type {
            KeyType::MlDsa65 | KeyType::MlKem768 | KeyType::X25519 => assert!(forms.openssh.is_none()),
            _ => {
                let line = scratch.write("id.pub", format!("{} test\n", forms.openssh.as_ref().unwrap()).as_bytes());
                let listed = output(Command::new("ssh-keygen").arg("-lf").arg(&line));
                assert!(listed.contains(forms.ssh_sha256.as_ref().unwrap()), "{listed}");
                // ssh-keygen has no PKCS#8 form of an Ed25519 key
                if key_type != KeyType::Ed25519 {
                    let converted = output(Command::new("ssh-keygen").args(["-e", "-m", "PKCS8", "-f"]).arg(&line));
                    assert_eq!(converted, forms.spki_pem);
                }
            }
        }
    }
    assert_eq!(generate(KeyType::Aes256).unwrap().public().unwrap_err(), KeyError::NotForThisType);
}

#[test]
fn private_export_opens_in_openssl_and_only_with_the_passphrase() {
    if !need("openssl") {
        return;
    }
    let scratch = Scratch::new("export");
    let pass = scratch.write("pass", b"correct horse");
    let wrong = scratch.write("wrong", b"incorrect horse");
    for key_type in [KeyType::Ed25519, KeyType::EcdsaP384, KeyType::Rsa3072, KeyType::MlDsa65, KeyType::X25519, KeyType::MlKem768] {
        if !key_type.available() {
            continue;
        }
        let key = generate(key_type).unwrap();
        let sealed = key.export_encrypted(b"correct horse").unwrap();
        assert!(std::str::from_utf8(&sealed).unwrap().starts_with("-----BEGIN ENCRYPTED PRIVATE KEY-----"));
        let path = scratch.write("sealed.pem", &sealed);
        // Nothing of the key in the clear, and PBKDF2 at the count this says
        let structure = output(Command::new("openssl").arg("asn1parse").arg("-in").arg(&path));
        assert!(structure.contains("PBES2") && structure.contains("PBKDF2") && structure.contains("aes-256-cbc") && structure.contains("hmacWithSHA256"), "{structure}");
        assert!(structure.contains("927C0"), "600000 rounds: {structure}");
        let mut opened = Command::new("openssl");
        opened.args(["pkey", "-pubout", "-in"]).arg(&path).arg("-passin").arg(format!("file:{}", pass.display()));
        assert_eq!(output(&mut opened), key.public().unwrap().spki_pem, "{}", key_type.name());
        let refused = Command::new("openssl")
            .args(["pkey", "-pubout", "-in"])
            .arg(&path)
            .arg("-passin")
            .arg(format!("file:{}", wrong.display()))
            .output()
            .unwrap();
        assert!(!refused.status.success());
        // The plain form, for a server
        let plain = scratch.write("plain.pem", &key.export_plain().unwrap());
        assert_eq!(output(Command::new("openssl").args(["pkey", "-pubout", "-in"]).arg(&plain)), key.public().unwrap().spki_pem);
    }
    assert_eq!(generate(KeyType::Ed25519).unwrap().export_encrypted(b"").unwrap_err(), KeyError::BadPassphrase);
}

#[test]
fn an_aes_key_opens_with_openssl_enc() {
    if !need("openssl") {
        return;
    }
    let scratch = Scratch::new("aes");
    let key = generate(KeyType::Aes256).unwrap();
    assert_eq!(key.as_bytes().len(), 32);
    let pass = scratch.write("pass", b"a passphrase");
    let sealed = scratch.write("key.enc", &key.export_encrypted(b"a passphrase").unwrap());
    let raw = scratch.path("key.raw");
    run(Command::new("openssl")
        .args(["enc", "-d", "-aes-256-cbc", "-pbkdf2", "-iter", "600000", "-a", "-md", "sha256", "-in"])
        .arg(&sealed)
        .arg("-out")
        .arg(&raw)
        .arg("-pass")
        .arg(format!("file:{}", pass.display())));
    assert_eq!(std::fs::read(&raw).unwrap(), key.as_bytes());
}

/// A key `ssh-keygen` wrote, in the format and under the cipher given, opens here as the same key
fn check_import(scratch: &Scratch, kind: &[&str], format: &[&str], cipher: Option<&str>) -> PrivateKey {
    let file = scratch.path("id");
    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_file(file.with_extension("pub"));
    let mut command = Command::new("ssh-keygen");
    command.args(kind).args(format).args(["-q", "-C", "someone@example", "-a", "4", "-N", "open sesame", "-f"]).arg(&file);
    if let Some(cipher) = cipher {
        command.args(["-Z", cipher]);
    }
    run(&mut command);
    let bytes = std::fs::read(&file).unwrap();
    let imported = import_openssh(&bytes, b"open sesame").unwrap_or_else(|error| panic!("{kind:?} {cipher:?}: {error}"));
    let public = std::fs::read_to_string(format!("{}.pub", file.display())).unwrap();
    let expected = public.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
    assert_eq!(imported.key.public().unwrap().openssh.unwrap(), expected, "{kind:?} {cipher:?}");
    assert!(matches!(import_openssh(&bytes, b"not it"), Err(KeyError::WrongPassphrase)), "{kind:?} {cipher:?}");
    assert!(import_openssh(&bytes, b"").is_err());
    if format.is_empty() {
        assert_eq!(imported.comment, "someone@example");
    }
    imported.key
}

#[test]
fn openssh_keys_import_with_their_passphrase() {
    if !need("ssh-keygen") {
        return;
    }
    let scratch = Scratch::new("import");
    let ed25519: &[&str] = &["-t", "ed25519"];
    assert_eq!(check_import(&scratch, ed25519, &[], None).key_type(), KeyType::Ed25519);
    for cipher in ["aes128-ctr", "aes192-ctr", "aes256-ctr", "aes128-cbc", "aes256-cbc", "aes128-gcm@openssh.com", "aes256-gcm@openssh.com"] {
        check_import(&scratch, ed25519, &[], Some(cipher));
    }
    assert_eq!(check_import(&scratch, &["-t", "ecdsa", "-b", "256"], &[], None).key_type(), KeyType::EcdsaP256);
    assert_eq!(check_import(&scratch, &["-t", "ecdsa", "-b", "384"], &[], None).key_type(), KeyType::EcdsaP384);
    assert_eq!(check_import(&scratch, &["-t", "ecdsa", "-b", "521"], &[], None).key_type(), KeyType::EcdsaP521);
    assert_eq!(check_import(&scratch, &["-t", "rsa", "-b", "3072"], &[], None).key_type(), KeyType::Rsa3072);
    assert_eq!(check_import(&scratch, &["-t", "rsa", "-b", "2048"], &[], None).key_type(), KeyType::RsaOther);
    // The PEM forms ssh-keygen still writes when told to
    assert_eq!(check_import(&scratch, &["-t", "rsa", "-b", "3072"], &["-m", "PEM"], None).key_type(), KeyType::Rsa3072);
    assert_eq!(check_import(&scratch, &["-t", "ecdsa", "-b", "256"], &["-m", "PKCS8"], None).key_type(), KeyType::EcdsaP256);
}

#[test]
fn unencrypted_and_refused_imports() {
    if !need("ssh-keygen") {
        return;
    }
    let scratch = Scratch::new("import2");
    let file = scratch.path("id");
    run_in(Command::new("ssh-keygen").args(["-t", "ed25519", "-q", "-N", "", "-C", "", "-f"]).arg(&file), scratch.dir());
    let bytes = std::fs::read(&file).unwrap();
    assert_eq!(import_openssh(&bytes, b"").unwrap().key.key_type(), KeyType::Ed25519);
    // A bit changed, cut short, huge, and nothing like a key are refused, never repaired and never a panic
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(import_openssh(text.replace("OPENSSH PRIVATE KEY-----\n", "OPENSSH PRIVATE KEY-----\nAAAA").as_bytes(), b"").is_err());
    assert!(import_openssh(&bytes[..bytes.len() / 2], b"").is_err());
    assert!(import_openssh(&vec![b'a'; 70_000], b"").is_err());
    assert!(import_openssh(b"hello", b"").is_err());
    assert!(import_openssh(b"-----BEGIN OPENSSH PRIVATE KEY-----\nAAAA\n-----END OPENSSH PRIVATE KEY-----\n", b"").is_err());
    // Every truncation of the binary inside, which walks each length check
    let body: String = text.lines().filter(|line| !line.starts_with("-----")).collect();
    use base64::Engine;
    let binary = base64::engine::general_purpose::STANDARD.decode(&body).unwrap();
    for cut in 0..binary.len() {
        let short = base64::engine::general_purpose::STANDARD.encode(&binary[..cut]);
        let file = format!("-----BEGIN OPENSSH PRIVATE KEY-----\n{short}\n-----END OPENSSH PRIVATE KEY-----\n");
        assert!(import_openssh(file.as_bytes(), b"").is_err(), "cut at {cut}");
    }
}
