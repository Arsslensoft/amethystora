//! `pdfsig` signing through the module, the key never in NSS or in `pdfsig`'s reach but for each signature, which here a
//! test backend makes from a folder. Run with `--features test-backend`, with the workspace naming this crate.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use amethystora_vault_core::keys::{generate, KeyType};
use amethystora_vault_core::pki::*;

fn run(command: &mut Command) -> String {
    let out = command.output().unwrap();
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{command:?}\n{said}");
    said
}

fn attempt(command: &mut Command) -> (bool, String) {
    let out = command.output().unwrap();
    (out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

fn module() -> PathBuf {
    // deps/pdfsig-HASH -> the target folder that has the cdylib
    let exe = std::env::current_exe().unwrap();
    let deps = exe.parent().unwrap();
    for folder in [deps.to_path_buf(), deps.parent().unwrap().to_path_buf()] {
        let path = folder.join("libamethystora_vault_pkcs11.so");
        if path.exists() {
            return path;
        }
    }
    panic!("the module is not built");
}

fn unsigned_pdf() -> Vec<u8> {
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for text in [
        "1 0 obj\n<</Type/Catalog/Pages 2 0 R>>\nendobj\n",
        "2 0 obj\n<</Type/Pages/Kids[3 0 R]/Count 1>>\nendobj\n",
        "3 0 obj\n<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]>>\nendobj\n",
    ] {
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

fn database(dir: &Path, name: &str) -> String {
    let path = dir.join(name);
    std::fs::create_dir_all(&path).unwrap();
    let db = format!("sql:{}", path.display());
    run(Command::new("certutil").args(["-N", "--empty-password", "-d", &db]));
    db
}

fn pem_file(dir: &Path, name: &str, cert: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, pem_chain(cert, &[]).unwrap()).unwrap();
    path
}

struct Fixture {
    root: Vec<u8>,
    account: Issued,
    account_key: CaKey,
}

fn fixture(now: i64) -> Fixture {
    let (root_key, root) = rotate_root(now).unwrap();
    let account_key = CaKey::generate(Authority::AccountCa).unwrap();
    let spki = account_key.public().unwrap().spki_der;
    let account = issue(
        &Request { profile: "account-ca", public_key: &spki, now, name: Some("Ada Lovelace"), ..Request::default() },
        &Issuer { authority: Authority::Root, cert: &root, key: &root_key, chain: &[] },
    )
    .unwrap();
    Fixture { root, account, account_key }
}

#[test]
fn pdfsig_signs_through_the_module() {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let fixture = fixture(now);
    let scratch = std::env::temp_dir().join(format!("amethystora-vault-pkcs11-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let input = scratch.join("in.pdf");
    std::fs::write(&input, unsigned_pdf()).unwrap();

    for key_type in [KeyType::Rsa3072, KeyType::Rsa4096, KeyType::EcdsaP256, KeyType::EcdsaP384] {
        let dir = scratch.join(key_type.name());
        std::fs::create_dir_all(&dir).unwrap();
        let key = generate(key_type).unwrap();
        let spki = key.public().unwrap().spki_der;
        let issued = issue(
            &Request { profile: "pdf", public_key: &spki, now, name: Some("Ada Lovelace"), ..Request::default() },
            &Issuer { authority: Authority::AccountCa, cert: &fixture.account.cert, key: &fixture.account_key, chain: &fixture.account.chain },
        )
        .unwrap();
        std::fs::write(dir.join("cert.der"), &issued.cert).unwrap();
        std::fs::write(dir.join("key.der"), key.as_bytes()).unwrap();
        std::fs::write(dir.join("key.type"), key_type.name()).unwrap();

        std::fs::write(dir.join("chain.der"), &fixture.account.cert).unwrap();
        std::fs::write(dir.join("root.der"), &fixture.root).unwrap();

        // A database with nothing in it but the module: the chain comes from the module too
        let db = database(&dir, "signer");
        run(Command::new("modutil").args(["-force", "-dbdir", &db, "-add", "amethystora-vault", "-libfile"]).arg(module()));

        let test_env = ("AMETHYSTORA_VAULT_PKCS11_TEST", dir.as_os_str());
        let listed = run(Command::new("pdfsig").env(test_env.0, test_env.1).args(["-nssdir", &db, "-list-nicks"]));
        assert!(listed.contains("Ada Lovelace"), "{key_type:?}: {listed}");
        assert!(!listed.contains("Amethystora Vault CA"), "{key_type:?}: {listed}");

        let out = dir.join("signed.pdf");
        let nick = listed.lines().find(|line| line.contains("Ada Lovelace")).unwrap().trim().to_string();
        let (ok, said) = attempt(Command::new("pdfsig").env(test_env.0, test_env.1).args(["-nssdir", &db, "-add-signature", "-nick", &nick, "-new-signature-field-name", "S1"]).arg(&input).arg(&out));
        assert!(ok, "{key_type:?} with nick {nick}: {said}");
        assert_eq!(std::fs::read_to_string(dir.join("signed")).unwrap().len(), 1, "{key_type:?}: the backend signed once");

        // Read by a database that knows nothing of the module and trusts the root for email
        let reader = database(&dir, "reader");
        run(Command::new("certutil").args(["-A", "-d", &reader, "-n", "root", "-t", ",C,", "-a", "-i"]).arg(pem_file(&dir, "root.pem", &fixture.root)));
        let said = run(Command::new("pdfsig").args(["-nssdir", &reader, "-no-ocsp"]).arg(&out));
        assert!(said.contains("Signature is Valid"), "{key_type:?}: {said}");
        assert!(said.contains("Certificate is Trusted"), "{key_type:?}: {said}");
        assert!(said.contains("Total document signed"), "{key_type:?}: {said}");

        // The certificate by its PKCS#11 URI, as pdfsig's man page has it
        let by_uri = dir.join("uri.pdf");
        run(Command::new("pdfsig").env(test_env.0, test_env.1).args(["-nssdir", &db, "-add-signature", "-nick", "pkcs11:token=Amethystora%20Vault;object=Ada%20Lovelace;type=cert", "-new-signature-field-name", "S1"]).arg(&input).arg(&by_uri));
        let said = run(Command::new("pdfsig").args(["-nssdir", &reader, "-no-ocsp"]).arg(&by_uri));
        assert!(said.contains("Certificate is Trusted"), "{key_type:?} by URI: {said}");
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn a_refusal_fails_the_signing() {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let fixture = fixture(now);
    let scratch = std::env::temp_dir().join(format!("amethystora-vault-pkcs11-refused-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let input = scratch.join("in.pdf");
    std::fs::write(&input, unsigned_pdf()).unwrap();
    let key = generate(KeyType::EcdsaP256).unwrap();
    let spki = key.public().unwrap().spki_der;
    let issued = issue(
        &Request { profile: "pdf", public_key: &spki, now, name: Some("Ada Lovelace"), ..Request::default() },
        &Issuer { authority: Authority::AccountCa, cert: &fixture.account.cert, key: &fixture.account_key, chain: &fixture.account.chain },
    )
    .unwrap();
    std::fs::write(scratch.join("cert.der"), &issued.cert).unwrap();
    // No key file: the backend says no, as the service would for a closed vault or a person's No
    let db = database(&scratch, "signer");
    run(Command::new("modutil").args(["-force", "-dbdir", &db, "-add", "amethystora-vault", "-libfile"]).arg(module()));
    let listed = run(Command::new("pdfsig").env("AMETHYSTORA_VAULT_PKCS11_TEST", &scratch).args(["-nssdir", &db, "-list-nicks"]));
    let nick = listed.lines().find(|line| line.contains("Ada Lovelace")).unwrap().trim().to_string();
    let out = scratch.join("signed.pdf");
    let (ok, said) = attempt(Command::new("pdfsig").env("AMETHYSTORA_VAULT_PKCS11_TEST", &scratch).args(["-nssdir", &db, "-add-signature", "-nick", &nick, "-new-signature-field-name", "S1"]).arg(&input).arg(&out));
    assert!(!ok || !out.exists(), "{said}");
    let _ = std::fs::remove_dir_all(&scratch);
}
