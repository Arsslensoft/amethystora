//! What sync needs of libcrypto, which the service never loads: an SSH key item's strings, a key read back from a note's text,
//! and a certificate chain checked. `Convert` is the door: the service sends each to `amethystora-vault-work` (which calls the
//! functions here), and a test calls them directly (`Direct`). Nothing here touches a file or a socket.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use openssl::sha::sha256;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::keys::{self, KeyError, KeyType, PrivateKey};
use crate::pki;

/// The three strings of an SSH key item (spec 5.3): the key in OpenSSH's unencrypted form, its one-line public key and its fingerprint
pub struct SshForms {
    pub private: Zeroizing<String>,
    pub public: String,
    pub fingerprint: String,
}

/// A key read back from text, as the vault holds it
pub struct KeyRead {
    pub key_type: String,
    /// PKCS#8, or an AES key's 32 bytes
    pub key: Zeroizing<Vec<u8>>,
    /// The item's `public` object (`type`, `pem`, `openssh`, `sha256`, `ssh_sha256`), null for an AES key
    pub public: Value,
    /// The two halves of the key agree: a message signed with the private half verifies under the public
    pub agree: bool,
}

/// What a chain of certificates says, checked
pub struct CertFacts {
    pub leaf_sha256: String,
    pub spki_sha256: String,
    /// There was a key to hold against the leaf, and it is the leaf's own
    pub key_matches: bool,
    /// Each certificate is signed by the next one's key: a chain, and not any certificates that parse
    pub chain_ok: bool,
}

pub trait Convert {
    /// An SSH item's strings, for the six types Bitwarden's tools make (Ed25519, RSA 3072 and 4096, ECDSA on P-256, P-384 and P-521)
    fn ssh_forms(&self, key_type: &str, key: &[u8]) -> Option<SshForms>;
    /// A private key from the text of a note's hidden field or an SSH item: PEM, or OpenSSH's form, or an AES key's base64 line
    fn key_from_text(&self, text: &str) -> Option<KeyRead>;
    /// Every certificate parses, and the key (type and bytes), if one is given, is the leaf's
    fn cert_facts(&self, certs: &[Vec<u8>], key: Option<(&str, &[u8])>) -> Option<CertFacts>;
    /// The same check the vault's own code makes of a value of this kind before it keeps it (a protected file's tree)
    fn accepts(&self, kind: &str, value: &Value) -> bool;
}

fn ssh_string(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
}

fn ssh_mpint(out: &mut Vec<u8>, number: &[u8]) {
    let start = number.iter().position(|byte| *byte != 0).unwrap_or(number.len());
    let number = &number[start..];
    if number.first().is_some_and(|byte| byte & 0x80 != 0) {
        out.extend_from_slice(&(number.len() as u32 + 1).to_be_bytes());
        out.push(0);
        out.extend_from_slice(number);
    } else {
        ssh_string(out, number);
    }
}

/// OpenSSH's `openssh-key-v1` file with no cipher and no comment. Its two check numbers, which `ssh-keygen` makes random, are the
/// first bytes of the public key's digest: the same key is always the same text, so that a note is not rewritten for nothing.
fn openssh_private(key: &PrivateKey) -> Result<Zeroizing<String>, KeyError> {
    let pkey = key.pkey()?;
    let (name, blob) = keys::openssh_blob(&pkey, key.key_type())?.ok_or(KeyError::Unsupported)?;
    let check = sha256(&blob)[..4].to_vec();
    let mut inner = Zeroizing::new(Vec::new());
    inner.extend_from_slice(&check);
    inner.extend_from_slice(&check);
    ssh_string(&mut inner, name.as_bytes());
    match key.key_type() {
        KeyType::Ed25519 => {
            let public = pkey.raw_public_key()?;
            let mut pair = Zeroizing::new(pkey.raw_private_key()?);
            pair.extend_from_slice(&public);
            ssh_string(&mut inner, &public);
            ssh_string(&mut inner, &pair);
        }
        KeyType::Rsa3072 | KeyType::Rsa4096 | KeyType::RsaOther => {
            let rsa = pkey.rsa()?;
            let (Some(p), Some(q), Some(iqmp)) = (rsa.p(), rsa.q(), rsa.iqmp()) else { return Err(KeyError::Malformed) };
            for number in [rsa.n().to_vec(), rsa.e().to_vec(), rsa.d().to_vec(), iqmp.to_vec(), p.to_vec(), q.to_vec()] {
                ssh_mpint(&mut inner, &number);
            }
        }
        KeyType::EcdsaP256 | KeyType::EcdsaP384 | KeyType::EcdsaP521 => {
            let ec = pkey.ec_key()?;
            let mut context = openssl::bn::BigNumContext::new()?;
            let point = ec.public_key().to_bytes(ec.group(), openssl::ec::PointConversionForm::UNCOMPRESSED, &mut context)?;
            let curve = match key.key_type() {
                KeyType::EcdsaP256 => "nistp256",
                KeyType::EcdsaP384 => "nistp384",
                _ => "nistp521",
            };
            ssh_string(&mut inner, curve.as_bytes());
            ssh_string(&mut inner, &point);
            ssh_mpint(&mut inner, &ec.private_key().to_vec());
        }
        _ => return Err(KeyError::Unsupported),
    }
    ssh_string(&mut inner, b"");
    let mut pad = 1u8;
    while inner.len() % 8 != 0 {
        inner.push(pad);
        pad += 1;
    }
    let mut file = Zeroizing::new(b"openssh-key-v1\0".to_vec());
    ssh_string(&mut file, b"none");
    ssh_string(&mut file, b"none");
    ssh_string(&mut file, b"");
    file.extend_from_slice(&1u32.to_be_bytes());
    ssh_string(&mut file, &blob);
    ssh_string(&mut file, &inner);
    // 70 columns, as OpenSSH writes it
    let body = Zeroizing::new(STANDARD.encode(&*file));
    let mut text = Zeroizing::new(String::from("-----BEGIN OPENSSH PRIVATE KEY-----\n"));
    for line in body.as_bytes().chunks(70) {
        text.push_str(std::str::from_utf8(line).map_err(|_| KeyError::Failed)?);
        text.push('\n');
    }
    text.push_str("-----END OPENSSH PRIVATE KEY-----\n");
    Ok(text)
}

pub fn ssh_forms(key_type: &str, bytes: &[u8]) -> Option<SshForms> {
    let kind = KeyType::parse(key_type)?;
    if !matches!(kind, KeyType::Ed25519 | KeyType::EcdsaP256 | KeyType::EcdsaP384 | KeyType::EcdsaP521 | KeyType::Rsa3072 | KeyType::Rsa4096) {
        return None;
    }
    let key = PrivateKey::from_bytes(kind, bytes).ok()?;
    let public = key.public().ok()?;
    Some(SshForms { private: openssh_private(&key).ok()?, public: public.openssh?, fingerprint: public.ssh_sha256? })
}

/// A private key and its public forms, with the check that its halves are one key
fn read_of(key: PrivateKey) -> Option<KeyRead> {
    let (public, agree) = if key.key_type() == KeyType::Aes256 {
        (Value::Null, true)
    } else {
        let forms = key.public().ok()?;
        let agree = halves_agree(&key, &forms.spki_der);
        (json!({"type": forms.key_type.name(), "pem": forms.spki_pem, "openssh": forms.openssh, "sha256": forms.sha256, "ssh_sha256": forms.ssh_sha256}), agree)
    };
    Some(KeyRead { key_type: key.key_type().name().to_string(), key: Zeroizing::new(key.as_bytes().to_vec()), public, agree })
}

pub fn key_from_text(text: &str) -> Option<KeyRead> {
    let text = text.trim();
    let key = if text.contains("BEGIN OPENSSH PRIVATE KEY") {
        keys::import_openssh(text.as_bytes(), b"").ok()?.key
    } else if text.contains("PRIVATE KEY-----") {
        keys::from_pem(text).ok()?
    } else {
        // An AES key's line: its 32 bytes in base64 and nothing else
        let bytes = Zeroizing::new(STANDARD.decode(text.as_bytes()).ok()?);
        PrivateKey::from_bytes(KeyType::Aes256, &bytes).ok()?
    };
    read_of(key)
}

/// A private key whose public half is not the one it says, which a PEM put together by hand can be: a test message signed with the
/// private half has to verify under the public. A key that cannot sign (X25519, ML-KEM) has nothing to prove here.
pub fn halves_agree(key: &PrivateKey, spki: &[u8]) -> bool {
    let digest = match key.key_type() {
        KeyType::Ed25519 | KeyType::MlDsa65 => "none",
        KeyType::EcdsaP256 | KeyType::EcdsaP384 | KeyType::EcdsaP521 | KeyType::Rsa3072 | KeyType::Rsa4096 | KeyType::RsaOther => "sha256",
        _ => return true,
    };
    let message = b"amethystora-vault: the halves of one key";
    let (Ok(signature), Ok(public)) = (keys::sign_message(key, digest, message), openssl::pkey::PKey::public_key_from_der(spki)) else { return false };
    let verifier = if digest == "none" { openssl::sign::Verifier::new_without_digest(&public) } else { openssl::sign::Verifier::new(openssl::hash::MessageDigest::sha256(), &public) };
    verifier.and_then(|mut verifier| verifier.verify_oneshot(&signature, message)).unwrap_or(false)
}

pub fn cert_facts(certs: &[Vec<u8>], key: Option<(&str, &[u8])>) -> Option<CertFacts> {
    let leaf = certs.first()?;
    let mut facts = None;
    let mut chain_ok = true;
    let mut parsed = Vec::with_capacity(certs.len());
    for (at, der) in certs.iter().enumerate() {
        let told = pki::describe(der).ok()?;
        if at == 0 {
            facts = Some((told.sha256, told.key.sha256, told.key.spki_der));
        }
        parsed.push(openssl::x509::X509::from_der(der).ok()?);
    }
    for pair in parsed.windows(2) {
        chain_ok &= pair[1].public_key().ok().and_then(|key| pair[0].verify(&key).ok()).unwrap_or(false);
    }
    let (leaf_sha256, spki_sha256, spki) = facts?;
    debug_assert!(!leaf.is_empty());
    let key_matches = match key {
        None => false,
        Some((key_type, bytes)) => KeyType::parse(key_type).and_then(|kind| PrivateKey::from_bytes(kind, bytes).ok()).and_then(|key| key.public().ok()).is_some_and(|forms| forms.spki_der == spki),
    };
    Some(CertFacts { leaf_sha256, spki_sha256, key_matches, chain_ok })
}

/// Libcrypto in this process: for tests, which have it. The service has no such thing, and asks `amethystora-vault-work`.
#[cfg(any(test, feature = "fake-server"))]
pub struct Direct;

#[cfg(any(test, feature = "fake-server"))]
impl Convert for Direct {
    fn ssh_forms(&self, key_type: &str, key: &[u8]) -> Option<SshForms> {
        ssh_forms(key_type, key)
    }

    fn key_from_text(&self, text: &str) -> Option<KeyRead> {
        key_from_text(text)
    }

    fn cert_facts(&self, certs: &[Vec<u8>], key: Option<(&str, &[u8])>) -> Option<CertFacts> {
        cert_facts(certs, key)
    }

    /// A stand-in for the service's own parser of a protected file: its shape, and names that stay where they are
    fn accepts(&self, kind: &str, value: &Value) -> bool {
        if kind != "protected" {
            return true;
        }
        let plain = |at: &str| !at.is_empty() && at.split('/').all(|name| !name.is_empty() && name != "." && name != ".." && !name.chars().any(char::is_control));
        let what = value["what"].as_str();
        let files = value["files"].as_array();
        matches!(what, Some("file" | "folder"))
            && files.is_some_and(|files| files.iter().all(|file| (what == Some("file") || file["at"].as_str().is_some_and(plain)) && file["data"].as_str().is_some()))
            && value["folders"].as_array().is_some_and(|folders| folders.iter().all(|folder| folder["at"].as_str().is_some_and(plain)))
    }
}
