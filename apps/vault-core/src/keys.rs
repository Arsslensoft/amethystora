//! Keys: making them, their public forms, export, and importing an existing SSH key (plans/PLAN-vault.md, Cryptographic
//! keys). No socket and no files: bytes in and out, private ones in memory that wipes itself.
//!
//! A private key is kept as its PKCS#8 DER, an AES key as its 32 bytes. It only becomes an OpenSSL key for the length of
//! one call. Making an RSA 4096 key takes a second or more, and stretching a passphrase a good part of one: the service
//! runs both off its main loop.

use std::fmt;

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use base64::Engine;
use openssl::asn1::Asn1Object;
use openssl::bn::{BigNum, BigNumContext};
use openssl::ec::{EcGroup, EcKey, EcPoint, PointConversionForm};
use openssl::error::ErrorStack;
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{HasPublic, Id, PKey, PKeyRef, Private};
use openssl::pkey_ctx::PkeyCtx;
use openssl::rsa::Rsa;
use openssl::sha::sha256;
use openssl::symm::{Cipher, Crypter, Mode};
use zeroize::Zeroizing;

use crate::pki::der;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyType {
    Ed25519,
    EcdsaP256,
    EcdsaP384,
    Rsa3072,
    Rsa4096,
    MlDsa65,
    X25519,
    MlKem768,
    Aes256,
    /// Only ever imported: an existing SSH key of a curve or a size that is never offered
    EcdsaP521,
    RsaOther,
}

impl KeyType {
    pub fn name(self) -> &'static str {
        match self {
            KeyType::Ed25519 => "ed25519",
            KeyType::EcdsaP256 => "ecdsa-p256",
            KeyType::EcdsaP384 => "ecdsa-p384",
            KeyType::Rsa3072 => "rsa-3072",
            KeyType::Rsa4096 => "rsa-4096",
            KeyType::MlDsa65 => "ml-dsa-65",
            KeyType::X25519 => "x25519",
            KeyType::MlKem768 => "ml-kem-768",
            KeyType::Aes256 => "aes-256",
            KeyType::EcdsaP521 => "ecdsa-p521",
            KeyType::RsaOther => "rsa",
        }
    }

    /// The type a stored item or a request names. The imported-only ones parse too, as stored items may be of them.
    pub fn parse(name: &str) -> Option<KeyType> {
        const ALL: [KeyType; 11] = [
            KeyType::Ed25519,
            KeyType::EcdsaP256,
            KeyType::EcdsaP384,
            KeyType::Rsa3072,
            KeyType::Rsa4096,
            KeyType::MlDsa65,
            KeyType::X25519,
            KeyType::MlKem768,
            KeyType::Aes256,
            KeyType::EcdsaP521,
            KeyType::RsaOther,
        ];
        ALL.into_iter().find(|key_type| key_type.name() == name)
    }

    /// ML-DSA and ML-KEM only where this libcrypto has them (OpenSSL 3.5 and later), asked each time rather than assumed
    pub fn available(self) -> bool {
        match self {
            KeyType::MlDsa65 | KeyType::MlKem768 => pq_context(self).is_ok(),
            _ => true,
        }
    }
}

/// What a key is for, in the plan's table
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyKind {
    Ssh,
    Signing,
    Exchange,
    Symmetric,
}

impl KeyKind {
    /// The types offered here on this machine, the default first
    pub fn types(self) -> Vec<KeyType> {
        let all: &[KeyType] = match self {
            KeyKind::Ssh => &[KeyType::Ed25519, KeyType::EcdsaP256, KeyType::Rsa3072, KeyType::Rsa4096],
            KeyKind::Signing => {
                &[KeyType::Ed25519, KeyType::EcdsaP256, KeyType::EcdsaP384, KeyType::Rsa3072, KeyType::Rsa4096, KeyType::MlDsa65]
            }
            KeyKind::Exchange => &[KeyType::X25519, KeyType::MlKem768],
            KeyKind::Symmetric => &[KeyType::Aes256],
        };
        all.iter().copied().filter(|key_type| key_type.available()).collect()
    }
}

/// Why a key could not be made, read or written. None of them carries anything of the key or the passphrase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyError {
    /// This libcrypto does not have the algorithm
    Unavailable,
    /// What was given is not a key of a form this reads
    Malformed,
    WrongPassphrase,
    /// A passphrase that is empty, or too long to be one
    BadPassphrase,
    /// A real key, of a type or a cipher that is not taken
    Unsupported,
    /// This key type cannot do what was asked: an AES key has no public form, an imported type cannot be generated
    NotForThisType,
    /// A file that asks for more work than is allowed to open it: rounds, iterations or a number past its bound
    TooCostly,
    Failed,
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            KeyError::Unavailable => "this system's OpenSSL does not have that algorithm",
            KeyError::Malformed => "not a key this can read",
            KeyError::WrongPassphrase => "wrong passphrase",
            KeyError::BadPassphrase => "a passphrase must be 1 to 1024 bytes",
            KeyError::Unsupported => "that kind of key is not supported",
            KeyError::NotForThisType => "not possible with this type of key",
            KeyError::TooCostly => "the file asks for more work to open than is allowed (256 rounds, or 10 million iterations); save it again with fewer, as ssh-keygen does with 16",
            KeyError::Failed => "the operation failed",
        })
    }
}

impl std::error::Error for KeyError {}

impl From<ErrorStack> for KeyError {
    fn from(_: ErrorStack) -> KeyError {
        KeyError::Failed
    }
}

const MAX_PASSPHRASE: usize = 1024;
/// More than any SSH key file, so that a huge one is refused before it is read
const MAX_IMPORT: usize = 64 * 1024;
/// ssh-keygen's default is 16, and 256 takes a few seconds. A file asking for more could hold the import for minutes.
const MAX_BCRYPT_ROUNDS: u32 = 256;
/// A PKCS#8 file sets its own PBKDF2 count. OWASP's figure is 600,000, which takes under a second; this is sixteen times that.
const MAX_PBKDF2_ROUNDS: u32 = 10_000_000;
const MAX_RSA_BITS: u32 = 16384;
/// PBKDF2 at OWASP's figure for HMAC-SHA-256, and not scrypt: every PKCS#8 reader has it, and it stays in the 8 MB
pub(crate) const PBKDF2_ROUNDS: u32 = 600_000;

/// A private key, or an AES key. Prints only its type, and wipes itself when dropped.
pub struct PrivateKey {
    key_type: KeyType,
    bytes: Zeroizing<Vec<u8>>,
}

impl fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PrivateKey({})", self.key_type.name())
    }
}

/// A key's public forms, with no approval needed: the window shows these and holds nothing else
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicForms {
    pub key_type: KeyType,
    pub spki_der: Vec<u8>,
    pub spki_pem: String,
    /// `ssh-ed25519 AAAA...` for what SSH has a form for, without a comment
    pub openssh: Option<String>,
    /// SHA-256 of the SPKI, in lowercase hex
    pub sha256: String,
    /// `SHA256:...`, what `ssh-keygen -l` prints
    pub ssh_sha256: Option<String>,
}

fn pq_nid(key_type: KeyType) -> Result<Nid, KeyError> {
    let name = if key_type == KeyType::MlDsa65 { "ML-DSA-65" } else { "ML-KEM-768" };
    Ok(Asn1Object::from_str(name).map_err(|_| KeyError::Unavailable)?.nid())
}

fn pq_context(key_type: KeyType) -> Result<PkeyCtx<()>, KeyError> {
    let mut context = PkeyCtx::new_id(Id::from_raw(pq_nid(key_type)?.as_raw())).map_err(|_| KeyError::Unavailable)?;
    context.keygen_init().map_err(|_| KeyError::Unavailable)?;
    Ok(context)
}

fn curve(key_type: KeyType) -> Option<(Nid, &'static str)> {
    match key_type {
        KeyType::EcdsaP256 => Some((Nid::X9_62_PRIME256V1, "nistp256")),
        KeyType::EcdsaP384 => Some((Nid::SECP384R1, "nistp384")),
        KeyType::EcdsaP521 => Some((Nid::SECP521R1, "nistp521")),
        _ => None,
    }
}

fn from_pkey(pkey: &PKeyRef<Private>) -> Result<PrivateKey, KeyError> {
    let key_type = classify(pkey).ok_or(KeyError::Unsupported)?;
    Ok(PrivateKey { key_type, bytes: Zeroizing::new(pkey.private_key_to_pkcs8()?) })
}

/// Makes a new key, from libcrypto's generator, which the kernel seeds
pub fn generate(key_type: KeyType) -> Result<PrivateKey, KeyError> {
    let pkey = match key_type {
        KeyType::Ed25519 => PKey::generate_ed25519()?,
        KeyType::X25519 => PKey::generate_x25519()?,
        KeyType::EcdsaP256 | KeyType::EcdsaP384 => {
            let (nid, _) = curve(key_type).ok_or(KeyError::Failed)?;
            PKey::from_ec_key(EcKey::generate(&*EcGroup::from_curve_name(nid)?)?)?
        }
        KeyType::Rsa3072 => PKey::from_rsa(Rsa::generate(3072)?)?,
        KeyType::Rsa4096 => PKey::from_rsa(Rsa::generate(4096)?)?,
        KeyType::MlDsa65 | KeyType::MlKem768 => pq_context(key_type)?.keygen()?,
        KeyType::Aes256 => {
            let mut bytes = Zeroizing::new(vec![0u8; 32]);
            openssl::rand::rand_bytes(&mut bytes)?;
            return Ok(PrivateKey { key_type, bytes });
        }
        KeyType::EcdsaP521 | KeyType::RsaOther => return Err(KeyError::NotForThisType),
    };
    from_pkey(&pkey)
}

/// A whole message signed, as SSH asks: Ed25519 with no digest of its own (`none`), RSA PKCS#1 v1.5 or a DER ECDSA over the
/// digest named. Only the worker calls it, for a key the person marked for SSH (`Signer::ssh` is the type that says so for a digest).
pub fn sign_message(key: &PrivateKey, digest: &str, data: &[u8]) -> Result<Vec<u8>, KeyError> {
    let pkey = key.pkey()?;
    let mut signer = match digest {
        "none" => openssl::sign::Signer::new_without_digest(&pkey)?,
        "sha256" => openssl::sign::Signer::new(MessageDigest::sha256(), &pkey)?,
        "sha384" => openssl::sign::Signer::new(MessageDigest::sha384(), &pkey)?,
        "sha512" => openssl::sign::Signer::new(MessageDigest::sha512(), &pkey)?,
        _ => return Err(KeyError::NotForThisType),
    };
    Ok(signer.sign_oneshot_to_vec(data)?)
}

/// What a key is, from its SubjectPublicKeyInfo. A key read back from DER has no type number when only a provider knows
/// it (`PKey::id` says -1 for ML-DSA, and the C function that asks by name has no binding), so this reads the algorithm's
/// identifiers, whichever OpenSSL it is.
pub(crate) fn classify<T: HasPublic>(pkey: &PKeyRef<T>) -> Option<KeyType> {
    let spki = pkey.public_key_to_der().ok()?;
    let (_, info, _) = der::read(&spki)?;
    let (_, algorithm, _) = der::read(info)?;
    let (_, oid, parameters) = der::read(algorithm)?;
    let is = |dotted: &str| der::oid(dotted)[2..] == *oid;
    if is("1.2.840.113549.1.1.1") {
        Some(match pkey.bits() {
            3072 => KeyType::Rsa3072,
            4096 => KeyType::Rsa4096,
            _ => KeyType::RsaOther,
        })
    } else if is("1.2.840.10045.2.1") {
        let (_, curve, _) = der::read(parameters)?;
        let named = |dotted: &str| der::oid(dotted)[2..] == *curve;
        if named("1.2.840.10045.3.1.7") {
            Some(KeyType::EcdsaP256)
        } else if named("1.3.132.0.34") {
            Some(KeyType::EcdsaP384)
        } else if named("1.3.132.0.35") {
            Some(KeyType::EcdsaP521)
        } else {
            None
        }
    } else if is("1.3.101.112") {
        Some(KeyType::Ed25519)
    } else if is("1.3.101.110") {
        Some(KeyType::X25519)
    } else if is("2.16.840.1.101.3.4.3.18") {
        Some(KeyType::MlDsa65)
    } else if is("2.16.840.1.101.3.4.4.2") {
        Some(KeyType::MlKem768)
    } else {
        None
    }
}

impl PrivateKey {
    /// A key as `as_bytes` gave it, read back from an item. Refused unless it is that type of key.
    pub fn from_bytes(key_type: KeyType, bytes: &[u8]) -> Result<PrivateKey, KeyError> {
        if key_type == KeyType::Aes256 {
            if bytes.len() != 32 {
                return Err(KeyError::Malformed);
            }
        } else {
            let pkey = PKey::private_key_from_pkcs8(bytes).map_err(|_| KeyError::Malformed)?;
            if classify(&pkey) != Some(key_type) {
                return Err(KeyError::Malformed);
            }
        }
        Ok(PrivateKey { key_type, bytes: Zeroizing::new(bytes.to_vec()) })
    }

    pub fn key_type(&self) -> KeyType {
        self.key_type
    }

    /// PKCS#8 DER, or an AES key's 32 bytes: what an item stores, sealed
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// As an OpenSSL key, for the length of one call
    pub(crate) fn pkey(&self) -> Result<PKey<Private>, KeyError> {
        if self.key_type == KeyType::Aes256 {
            return Err(KeyError::NotForThisType);
        }
        PKey::private_key_from_pkcs8(&self.bytes).map_err(|_| KeyError::Malformed)
    }

    /// The public forms of everything but an AES key, which has none
    pub fn public(&self) -> Result<PublicForms, KeyError> {
        PublicForms::from_spki(&self.pkey()?.public_key_to_der()?)
    }

    /// Unencrypted, for a server that cannot ask a person: PKCS#8 PEM, or an AES key's base64 line
    pub fn export_plain(&self) -> Result<Zeroizing<Vec<u8>>, KeyError> {
        if self.key_type == KeyType::Aes256 {
            // Encoded straight into the buffer that is returned, of its final size: a string that grows is moved, and the
            // block it leaves is freed with the key's base64 in it
            let mut line = Zeroizing::new(vec![b'\n'; 45]);
            STANDARD.encode_slice(&*self.bytes, &mut line[..44]).map_err(|_| KeyError::Failed)?;
            return Ok(line);
        }
        Ok(Zeroizing::new(self.pkey()?.private_key_to_pem_pkcs8()?))
    }

    /// PKCS#8 PEM under a passphrase, PBES2 with PBKDF2-HMAC-SHA-256 and AES-256-CBC. OpenSSL's own writer stretches the
    /// passphrase only 2048 times, so this one is written here.
    ///
    /// An AES key has no PKCS#8 form anything reads, so it comes as what `openssl enc -d -aes-256-cbc -pbkdf2 -iter 600000 -a`
    /// opens: base64, with its salt.
    pub fn export_encrypted(&self, passphrase: &[u8]) -> Result<Vec<u8>, KeyError> {
        if passphrase.is_empty() || passphrase.len() > MAX_PASSPHRASE {
            return Err(KeyError::BadPassphrase);
        }
        if self.key_type == KeyType::Aes256 {
            return export_aes(&self.bytes, passphrase);
        }
        let mut salt = [0u8; 16];
        let mut iv = [0u8; 16];
        openssl::rand::rand_bytes(&mut salt)?;
        openssl::rand::rand_bytes(&mut iv)?;
        let mut key = Zeroizing::new([0u8; 32]);
        openssl::pkcs5::pbkdf2_hmac(passphrase, &salt, PBKDF2_ROUNDS as usize, MessageDigest::sha256(), &mut *key)?;
        let sealed = openssl::symm::encrypt(Cipher::aes_256_cbc(), &*key, Some(&iv[..]), &self.bytes)?;
        let kdf = der::seq(&[
            &der::oid("1.2.840.113549.1.5.12"),
            &der::seq(&[
                &der::octets(&salt),
                &der::uint(&PBKDF2_ROUNDS.to_be_bytes()),
                &der::seq(&[&der::oid("1.2.840.113549.2.9"), &der::null()]),
            ]),
        ]);
        let cipher = der::seq(&[&der::oid("2.16.840.1.101.3.4.1.42"), &der::octets(&iv)]);
        let algorithm = der::seq(&[&der::oid("1.2.840.113549.1.5.13"), &der::seq(&[&kdf, &cipher])]);
        Ok(pem("ENCRYPTED PRIVATE KEY", &der::seq(&[&algorithm, &der::octets(&sealed)])).into_bytes())
    }
}

fn export_aes(key: &[u8], passphrase: &[u8]) -> Result<Vec<u8>, KeyError> {
    let mut salt = [0u8; 8];
    openssl::rand::rand_bytes(&mut salt)?;
    let mut stretched = Zeroizing::new([0u8; 48]);
    openssl::pkcs5::pbkdf2_hmac(passphrase, &salt, PBKDF2_ROUNDS as usize, MessageDigest::sha256(), &mut *stretched)?;
    let sealed = openssl::symm::encrypt(Cipher::aes_256_cbc(), &stretched[..32], Some(&stretched[32..]), key)?;
    let mut out = b"Salted__".to_vec();
    out.extend_from_slice(&salt);
    out.extend_from_slice(&sealed);
    Ok(format!("{}\n", STANDARD.encode(&out)).into_bytes())
}

pub fn pem(label: &str, der: &[u8]) -> String {
    let body = STANDARD.encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    for line in body.as_bytes().chunks(64) {
        out.push_str(&String::from_utf8_lossy(line));
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl PublicForms {
    /// From a SubjectPublicKeyInfo, which is all the window ever holds
    pub fn from_spki(spki_der: &[u8]) -> Result<PublicForms, KeyError> {
        let pkey = PKey::public_key_from_der(spki_der).map_err(|_| KeyError::Malformed)?;
        // Anything past the key's own encoding is refused: its fingerprints would be of the extra bytes
        if pkey.public_key_to_der()? != spki_der {
            return Err(KeyError::Malformed);
        }
        let key_type = classify(&pkey).ok_or(KeyError::Unsupported)?;
        let blob = openssh_blob(&pkey, key_type)?;
        Ok(PublicForms {
            key_type,
            spki_der: spki_der.to_vec(),
            spki_pem: pem("PUBLIC KEY", spki_der),
            openssh: blob.as_ref().map(|(name, blob)| format!("{name} {}", STANDARD.encode(blob))),
            sha256: hex(&sha256(spki_der)),
            ssh_sha256: blob.as_ref().map(|(_, blob)| format!("SHA256:{}", STANDARD_NO_PAD.encode(sha256(blob)))),
        })
    }
}

fn ssh_string(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
}

fn ssh_mpint(out: &mut Vec<u8>, number: &[u8]) {
    if number.first().is_some_and(|byte| byte & 0x80 != 0) {
        out.extend_from_slice(&(number.len() as u32 + 1).to_be_bytes());
        out.push(0);
        out.extend_from_slice(number);
    } else {
        ssh_string(out, number);
    }
}

/// The key's name in SSH and its wire form, for the types SSH has one for
pub(crate) fn openssh_blob<T: HasPublic>(pkey: &PKeyRef<T>, key_type: KeyType) -> Result<Option<(&'static str, Vec<u8>)>, KeyError> {
    let mut blob = Vec::new();
    let name = match key_type {
        KeyType::Ed25519 => {
            ssh_string(&mut blob, b"ssh-ed25519");
            ssh_string(&mut blob, &pkey.raw_public_key()?);
            "ssh-ed25519"
        }
        KeyType::Rsa3072 | KeyType::Rsa4096 | KeyType::RsaOther => {
            let rsa = pkey.rsa()?;
            ssh_string(&mut blob, b"ssh-rsa");
            ssh_mpint(&mut blob, &rsa.e().to_vec());
            ssh_mpint(&mut blob, &rsa.n().to_vec());
            "ssh-rsa"
        }
        KeyType::EcdsaP256 | KeyType::EcdsaP384 | KeyType::EcdsaP521 => {
            let (_, short) = curve(key_type).ok_or(KeyError::Failed)?;
            let name = match key_type {
                KeyType::EcdsaP256 => "ecdsa-sha2-nistp256",
                KeyType::EcdsaP384 => "ecdsa-sha2-nistp384",
                _ => "ecdsa-sha2-nistp521",
            };
            let ec = pkey.ec_key()?;
            let mut context = BigNumContext::new()?;
            let point = ec.public_key().to_bytes(ec.group(), PointConversionForm::UNCOMPRESSED, &mut context)?;
            ssh_string(&mut blob, name.as_bytes());
            ssh_string(&mut blob, short.as_bytes());
            ssh_string(&mut blob, &point);
            name
        }
        _ => return Ok(None),
    };
    Ok(Some((name, blob)))
}

/// An SSH key brought in from a file, and the comment the file had, which is text somebody wrote
#[derive(Debug)]
pub struct Imported {
    pub key: PrivateKey,
    pub comment: String,
}

/// Reads an existing private key with its passphrase: OpenSSH's own format, or the PEM forms ssh-keygen also reads
/// (`RSA PRIVATE KEY`, `EC PRIVATE KEY`, PKCS#8). Refused rather than repaired.
pub fn import_openssh(file: &[u8], passphrase: &[u8]) -> Result<Imported, KeyError> {
    if file.len() > MAX_IMPORT || passphrase.len() > MAX_PASSPHRASE {
        return Err(KeyError::Malformed);
    }
    let text = std::str::from_utf8(file).map_err(|_| KeyError::Malformed)?;
    if text.contains("BEGIN OPENSSH PRIVATE KEY") {
        return import_openssh_v1(text, passphrase);
    }
    if !text.contains("PRIVATE KEY-----") {
        return Err(KeyError::Malformed);
    }
    let encrypted = text.contains("ENCRYPTED");
    if text.contains("BEGIN ENCRYPTED PRIVATE KEY") {
        check_pkcs8_cost(text)?;
    }
    let pkey = PKey::private_key_from_pem_passphrase(file, passphrase)
        .map_err(|_| if encrypted { KeyError::WrongPassphrase } else { KeyError::Malformed })?;
    check_consistent(&pkey)?;
    let key = from_pkey(&pkey)?;
    if openssh_blob(&pkey, key.key_type)?.is_none() {
        return Err(KeyError::Unsupported);
    }
    Ok(Imported { key, comment: String::new() })
}

/// An unencrypted private key PEM of any type this reads, with an SSH form or none (sync reads a key back from a note's hidden
/// field): refused unless its numbers agree with each other, as an import is
pub fn from_pem(text: &str) -> Result<PrivateKey, KeyError> {
    if text.len() > MAX_IMPORT || !text.contains("PRIVATE KEY-----") || text.contains("ENCRYPTED") {
        return Err(KeyError::Malformed);
    }
    let pkey = PKey::private_key_from_pem(text.as_bytes()).map_err(|_| KeyError::Malformed)?;
    check_consistent(&pkey)?;
    from_pkey(&pkey)
}

/// The scheme, key derivation and count of an encrypted PKCS#8 file, which it sets itself: read, and bounded, before OpenSSL
/// is asked to do the work. Only PBES2 with PBKDF2 is taken; scrypt's cost is another set of numbers and PBES1's is a count
/// in another place, so neither is read. A file with more than one key in it is refused: which one OpenSSL reads is its choice.
fn check_pkcs8_cost(text: &str) -> Result<(), KeyError> {
    if text.matches("-----BEGIN").count() != 1 {
        return Err(KeyError::Malformed);
    }
    let mut body = String::with_capacity(text.len());
    for line in text.lines().map(str::trim).skip_while(|line| !line.starts_with("-----BEGIN ENCRYPTED PRIVATE KEY")).skip(1) {
        if line.starts_with("-----END") {
            break;
        }
        body.push_str(line);
    }
    let binary = STANDARD.decode(body.as_bytes()).map_err(|_| KeyError::Malformed)?;
    let malformed = || KeyError::Malformed;
    let identifier = |bytes: &[u8]| -> Result<(Vec<u8>, Vec<u8>), KeyError> {
        // An AlgorithmIdentifier: the object identifier, then whatever follows it
        let (_, contents, _) = der::read(bytes).ok_or_else(malformed)?;
        let (_, oid, parameters) = der::read(contents).ok_or_else(malformed)?;
        Ok((oid.to_vec(), parameters.to_vec()))
    };
    let (_, info, _) = der::read(&binary).ok_or_else(malformed)?;
    let (scheme, parameters) = identifier(info)?;
    if scheme != der::oid("1.2.840.113549.1.5.13")[2..] {
        return Err(KeyError::Unsupported);
    }
    let (_, pbes2, _) = der::read(&parameters).ok_or_else(malformed)?;
    let (derivation, parameters) = identifier(pbes2)?;
    if derivation != der::oid("1.2.840.113549.1.5.12")[2..] {
        return Err(KeyError::Unsupported);
    }
    let (_, pbkdf2, _) = der::read(&parameters).ok_or_else(malformed)?;
    let (_, _salt, rest) = der::read(pbkdf2).ok_or_else(malformed)?;
    let (tag, count, _) = der::read(rest).ok_or_else(malformed)?;
    if tag != der::INTEGER || count.first().is_none_or(|byte| byte & 0x80 != 0) {
        return Err(KeyError::Malformed);
    }
    let count = &count[count.iter().position(|byte| *byte != 0).unwrap_or(count.len())..];
    if count.is_empty() {
        return Err(KeyError::Malformed);
    }
    if count.len() > 4 || count.iter().fold(0u32, |all, byte| (all << 8) | u32::from(*byte)) > MAX_PBKDF2_ROUNDS {
        return Err(KeyError::TooCostly);
    }
    Ok(())
}

fn check_rsa_size(bits: u32) -> Result<(), KeyError> {
    if !(2048..=MAX_RSA_BITS).contains(&bits) {
        return Err(KeyError::Unsupported);
    }
    Ok(())
}

/// The size of each number of an RSA key, in bits, against its modulus's, before any of them is used in arithmetic: a prime
/// of a million bits holds a primality test for hours. A key's numbers cannot be larger than these.
fn check_rsa_numbers(n: u32, e: u32, d: u32, iqmp: u32, p: u32, q: u32) -> Result<(), KeyError> {
    check_rsa_size(n)?;
    let half = n / 2 + 8;
    if e > 64 || d > n || iqmp > half || p > half || q > half {
        return Err(KeyError::Malformed);
    }
    Ok(())
}

/// A key from a PEM file is taken only when its numbers agree with each other, so that the public key it shows is the one
/// that signs: OpenSSL reads the numbers and checks none of it.
fn check_consistent(pkey: &PKeyRef<Private>) -> Result<(), KeyError> {
    match pkey.id() {
        Id::RSA => {
            let rsa = pkey.rsa()?;
            let bits = |number: Option<&openssl::bn::BigNumRef>| number.map(|number| number.num_bits() as u32);
            let (Some(p), Some(q), Some(iqmp), Some(dmp1), Some(dmq1)) = (bits(rsa.p()), bits(rsa.q()), bits(rsa.iqmp()), bits(rsa.dmp1()), bits(rsa.dmq1()))
            else {
                return Err(KeyError::Malformed);
            };
            check_rsa_numbers(rsa.n().num_bits() as u32, rsa.e().num_bits() as u32, rsa.d().num_bits() as u32, iqmp, p, q)?;
            if dmp1.max(dmq1) > p.max(q) || !rsa.check_key().unwrap_or(false) {
                return Err(KeyError::Malformed);
            }
        }
        Id::EC => pkey.ec_key()?.check_key().map_err(|_| KeyError::Malformed)?,
        _ => {}
    }
    Ok(())
}

/// A walk through SSH's wire format, every length checked
struct Wire<'a>(&'a [u8]);

impl<'a> Wire<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], KeyError> {
        if count > self.0.len() {
            return Err(KeyError::Malformed);
        }
        let (head, tail) = self.0.split_at(count);
        self.0 = tail;
        Ok(head)
    }

    fn u32(&mut self) -> Result<u32, KeyError> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn string(&mut self) -> Result<&'a [u8], KeyError> {
        let count = self.u32()? as usize;
        self.take(count)
    }
}

/// How many bits a big-endian number takes, leading zeros not counted
fn bit_length(bytes: &[u8]) -> u32 {
    match bytes.iter().position(|byte| *byte != 0) {
        None => 0,
        Some(first) => ((bytes.len() - first) * 8) as u32 - bytes[first].leading_zeros(),
    }
}

/// A secret number, in OpenSSL's secure kind, which it wipes when it frees it: `BigNum::from_slice`'s is freed as it was.
/// Built a byte at a time, since a number of this kind cannot be made from bytes in one call. The length is bounded already.
fn secret_number(bytes: &[u8]) -> Result<BigNum, KeyError> {
    let mut number = BigNum::new_secure()?;
    for byte in bytes {
        number.mul_word(256)?;
        number.add_word(u32::from(*byte))?;
    }
    Ok(number)
}

/// A cipher's key length, IV length and whether it is AES-GCM. chacha20-poly1305 is left out: `ssh-keygen` only writes it
/// when told to with `-Z`.
fn ssh_cipher(name: &[u8]) -> Option<(Cipher, usize, usize, bool)> {
    Some(match name {
        b"aes128-ctr" => (Cipher::aes_128_ctr(), 16, 16, false),
        b"aes192-ctr" => (Cipher::aes_192_ctr(), 24, 16, false),
        b"aes256-ctr" => (Cipher::aes_256_ctr(), 32, 16, false),
        b"aes128-cbc" => (Cipher::aes_128_cbc(), 16, 16, false),
        b"aes192-cbc" => (Cipher::aes_192_cbc(), 24, 16, false),
        b"aes256-cbc" => (Cipher::aes_256_cbc(), 32, 16, false),
        b"aes128-gcm@openssh.com" => (Cipher::aes_128_gcm(), 16, 12, true),
        b"aes256-gcm@openssh.com" => (Cipher::aes_256_gcm(), 32, 12, true),
        _ => return None,
    })
}

fn import_openssh_v1(text: &str, passphrase: &[u8]) -> Result<Imported, KeyError> {
    // Of its final size, so that it never grows into a new block and leaves the old one behind
    let mut body = Zeroizing::new(String::with_capacity(text.len()));
    let mut ended = false;
    for line in text.lines().map(str::trim).skip_while(|line| !line.starts_with("-----BEGIN OPENSSH PRIVATE KEY")).skip(1) {
        if line.starts_with("-----END OPENSSH PRIVATE KEY") {
            ended = true;
            break;
        }
        body.push_str(line);
    }
    if !ended {
        return Err(KeyError::Malformed);
    }
    let binary = Zeroizing::new(STANDARD.decode(body.as_bytes()).map_err(|_| KeyError::Malformed)?);
    let mut wire = Wire(binary.as_slice());
    if wire.take(15)? != b"openssh-key-v1\0" {
        return Err(KeyError::Malformed);
    }
    let cipher_name = wire.string()?;
    let kdf_name = wire.string()?;
    let kdf_options = wire.string()?;
    if wire.u32()? != 1 {
        return Err(KeyError::Unsupported);
    }
    let public_blob = wire.string()?;
    let sealed = wire.string()?;

    let private = if cipher_name == b"none" {
        if kdf_name != b"none" || !wire.0.is_empty() {
            return Err(KeyError::Malformed);
        }
        Zeroizing::new(sealed.to_vec())
    } else {
        let (cipher, key_len, iv_len, gcm) = ssh_cipher(cipher_name).ok_or(KeyError::Unsupported)?;
        if kdf_name != b"bcrypt" {
            return Err(KeyError::Unsupported);
        }
        let mut options = Wire(kdf_options);
        let salt = options.string()?;
        let rounds = options.u32()?;
        if rounds > MAX_BCRYPT_ROUNDS {
            return Err(KeyError::TooCostly);
        }
        if salt.is_empty() || salt.len() > 64 || rounds == 0 {
            return Err(KeyError::Malformed);
        }
        if passphrase.is_empty() {
            return Err(KeyError::WrongPassphrase);
        }
        let mut stretched = Zeroizing::new(vec![0u8; key_len + iv_len]);
        bcrypt_pbkdf::bcrypt_pbkdf(passphrase, salt, rounds, &mut stretched).map_err(|_| KeyError::Malformed)?;
        let (key, iv) = stretched.split_at(key_len);
        let tag = if gcm { Some(wire.take(16)?) } else { None };
        // Nothing after the sealed part, or its tag
        if !wire.0.is_empty() {
            return Err(KeyError::Malformed);
        }
        decrypt(cipher, key, iv, sealed, tag)?
    };

    let mut wire = Wire(private.as_slice());
    if wire.u32()? != wire.u32()? {
        return Err(KeyError::WrongPassphrase);
    }
    let (pkey, comment) = read_private(&mut wire)?;
    // The padding is 1, 2, 3... to the cipher's block
    if wire.0.iter().enumerate().any(|(index, byte)| usize::from(*byte) != index + 1) {
        return Err(KeyError::Malformed);
    }
    let key = from_pkey(&pkey)?;
    let (_, blob) = openssh_blob(&pkey, key.key_type)?.ok_or(KeyError::Unsupported)?;
    if blob != public_blob {
        return Err(KeyError::Malformed);
    }
    Ok(Imported { key, comment })
}

fn decrypt(cipher: Cipher, key: &[u8], iv: &[u8], sealed: &[u8], tag: Option<&[u8]>) -> Result<Zeroizing<Vec<u8>>, KeyError> {
    let mut crypter = Crypter::new(cipher, Mode::Decrypt, key, Some(iv))?;
    crypter.pad(false);
    if let Some(tag) = tag {
        crypter.set_tag(tag)?;
    }
    let mut out = Zeroizing::new(vec![0u8; sealed.len() + cipher.block_size()]);
    let mut count = crypter.update(sealed, &mut out).map_err(|_| KeyError::Malformed)?;
    // A wrong passphrase shows here under AES-GCM, and at the check numbers under the rest
    count += crypter.finalize(&mut out[count..]).map_err(|_| KeyError::WrongPassphrase)?;
    out.truncate(count);
    Ok(out)
}

fn read_private(wire: &mut Wire<'_>) -> Result<(PKey<Private>, String), KeyError> {
    let name = wire.string()?;
    let pkey = match name {
        b"ssh-ed25519" => {
            let public = wire.string()?;
            let private = wire.string()?;
            if private.len() != 64 || private[32..] != *public {
                return Err(KeyError::Malformed);
            }
            PKey::private_key_from_raw_bytes(&private[..32], Id::ED25519).map_err(|_| KeyError::Malformed)?
        }
        b"ssh-rsa" => {
            let (n, e, d, iqmp, p, q) = (wire.string()?, wire.string()?, wire.string()?, wire.string()?, wire.string()?, wire.string()?);
            check_rsa_numbers(bit_length(n), bit_length(e), bit_length(d), bit_length(iqmp), bit_length(p), bit_length(q))?;
            let (n, e, d, iqmp, p, q) = (secret_number(n)?, secret_number(e)?, secret_number(d)?, secret_number(iqmp)?, secret_number(p)?, secret_number(q)?);
            // p - 1 and q - 1 give the factors away, so they are secret numbers too
            let mut context = BigNumContext::new_secure()?;
            let one = BigNum::from_u32(1)?;
            let (mut p1, mut q1) = (BigNum::new_secure()?, BigNum::new_secure()?);
            p1.checked_sub(&p, &one)?;
            q1.checked_sub(&q, &one)?;
            let (mut dmp1, mut dmq1) = (BigNum::new_secure()?, BigNum::new_secure()?);
            dmp1.nnmod(&d, &p1, &mut context)?;
            dmq1.nnmod(&d, &q1, &mut context)?;
            let rsa = Rsa::from_private_components(n, e, d, p, q, dmp1, dmq1, iqmp).map_err(|_| KeyError::Malformed)?;
            if !rsa.check_key().unwrap_or(false) {
                return Err(KeyError::Malformed);
            }
            PKey::from_rsa(rsa)?
        }
        b"ecdsa-sha2-nistp256" | b"ecdsa-sha2-nistp384" | b"ecdsa-sha2-nistp521" => {
            let key_type = match name {
                b"ecdsa-sha2-nistp256" => KeyType::EcdsaP256,
                b"ecdsa-sha2-nistp384" => KeyType::EcdsaP384,
                _ => KeyType::EcdsaP521,
            };
            let (nid, short) = curve(key_type).ok_or(KeyError::Failed)?;
            if wire.string()? != short.as_bytes() {
                return Err(KeyError::Malformed);
            }
            let group = EcGroup::from_curve_name(nid)?;
            let mut context = BigNumContext::new()?;
            let point = EcPoint::from_bytes(&group, wire.string()?, &mut context).map_err(|_| KeyError::Malformed)?;
            let scalar = wire.string()?;
            if scalar.len() > 80 {
                return Err(KeyError::Malformed);
            }
            let private = secret_number(scalar)?;
            let ec = EcKey::from_private_components(&group, &private, &point).map_err(|_| KeyError::Malformed)?;
            ec.check_key().map_err(|_| KeyError::Malformed)?;
            PKey::from_ec_key(ec)?
        }
        _ => return Err(KeyError::Unsupported),
    };
    let comment = String::from_utf8_lossy(wire.string()?).into_owned();
    Ok((pkey, comment))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The crate against OpenBSD's own vector. The tests in `tests/keys_forms.rs` hold the import to files `ssh-keygen` wrote.
    #[test]
    fn bcrypt_pbkdf_matches_openbsd() {
        let mut out = [0u8; 32];
        bcrypt_pbkdf::bcrypt_pbkdf(b"password", b"salt", 4, &mut out).unwrap();
        assert_eq!(hex(&out), "5bbf0cc293587f1c3635555c27796598d47e579071bf427e9d8fbe842aba34d9");
    }
}
