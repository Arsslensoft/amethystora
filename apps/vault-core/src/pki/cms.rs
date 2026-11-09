//! Signing a digest, the key in memory only for the call: as a bare signature, which the PKCS#11 module forwards, and as
//! the CMS a PDF's signature field holds. OpenSSL's CMS API wants the data rather than its digest, and the service never
//! sees a PDF, so the SignedData (RFC 5652) is written here, signed attributes and all.

use openssl::md::Md;
use openssl::pkey_ctx::PkeyCtx;
use openssl::rsa::Padding;
use openssl::sha::{sha256, sha384};
use openssl::x509::X509;

use super::facts::{facts, CERT_SIGN, CRL_SIGN, DIGITAL_SIGNATURE};
use super::{check_now, der, valid_at, Digest, PkiError, Profile};
use crate::keys::{KeyKind, KeyType, PrivateKey};

const SIGNED_DATA: &str = "1.2.840.113549.1.7.2";
const DATA: &str = "1.2.840.113549.1.7.1";
const SHA256: &str = "2.16.840.1.101.3.4.2.1";
const SHA384: &str = "2.16.840.1.101.3.4.2.2";
const CONTENT_TYPE: &str = "1.2.840.113549.1.9.3";
const MESSAGE_DIGEST: &str = "1.2.840.113549.1.9.4";
const SIGNING_TIME: &str = "1.2.840.113549.1.9.5";
const SIGNING_CERTIFICATE_V2: &str = "1.2.840.113549.1.9.16.2.47";

/// Whether the key is RSA rather than ECDSA, and refused if it is neither
fn is_rsa(key: &PrivateKey) -> Result<bool, PkiError> {
    match key.key_type() {
        KeyType::Rsa3072 | KeyType::Rsa4096 | KeyType::RsaOther => Ok(true),
        KeyType::EcdsaP256 | KeyType::EcdsaP384 | KeyType::EcdsaP521 => Ok(false),
        _ => Err(PkiError::KeyNotAllowed),
    }
}

fn check_length(digest: Digest, value: &[u8]) -> Result<(), PkiError> {
    if value.len() == digest.size() {
        Ok(())
    } else {
        Err(PkiError::BadDigest)
    }
}

/// What a signature is for. Each is one profile's certificate, and a certificate of another profile cannot sign it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Pdf,
    Code,
}

impl Purpose {
    fn profile(self) -> Profile {
        match self {
            Purpose::Pdf => Profile::Pdf,
            Purpose::Code => Profile::Code,
        }
    }
}

enum Use {
    /// A leaf certificate's key: its DER, and what it is for
    Leaf(Vec<u8>, Purpose),
    /// A key the person marked for SSH, which has no certificate
    Ssh,
}

/// A key that may sign a digest for a person, and what for. The only way to sign: the root's and the CAs' keys are a type
/// that cannot be given here, and any other key is checked against its certificate, which has to be a leaf of the profile
/// the purpose names and within its dates, so a blind digest cannot be turned into a certificate or into another purpose's
/// signature. An SSH key is signed with when it is marked for SSH, and has no certificate to check.
pub struct Signer<'a> {
    key: &'a PrivateKey,
    usage: Use,
}

impl<'a> Signer<'a> {
    /// The key of `cert`, for `purpose`, at `now`. Refused unless `cert` is the key's own, CA:FALSE, a signing certificate
    /// and no CA's, with exactly the purposes that profile's certificates carry, and valid now.
    pub fn leaf(key: &'a PrivateKey, cert: &[u8], purpose: Purpose, now: i64) -> Result<Signer<'a>, PkiError> {
        check_now(now)?;
        let parsed = X509::from_der(cert).map_err(|_| PkiError::Malformed)?;
        if !key.pkey()?.public_eq(&*parsed.public_key()?) {
            return Err(PkiError::IssuerKeyMismatch);
        }
        let found = facts(&parsed)?;
        let usage = found.usage.unwrap_or(0);
        let profile = purpose.profile();
        if found.ca != Some(false)
            || usage & DIGITAL_SIGNATURE == 0
            || usage & (CERT_SIGN | CRL_SIGN) != 0
            || !found.purposes_are(super::profile_purposes(profile))
        {
            return Err(PkiError::WrongCertificate);
        }
        if !valid_at(&parsed, now)? {
            return Err(PkiError::CertificateNotValid);
        }
        Ok(Signer { key, usage: Use::Leaf(cert.to_vec(), purpose) })
    }

    /// A key marked for SSH. Anything else is refused: only the person's SSH keys sign without a certificate.
    pub fn ssh(key: &'a PrivateKey, kind: KeyKind) -> Result<Signer<'a>, PkiError> {
        if kind != KeyKind::Ssh {
            return Err(PkiError::KeyNotAllowed);
        }
        Ok(Signer { key, usage: Use::Ssh })
    }
}

/// A digest signed with an RSA or ECDSA key: PKCS#1 v1.5 for RSA, the DER `ECDSA-Sig-Value` for ECDSA. Any other key
/// is refused, and with it anything that is not a digest's length. A leaf's certificate is held to its dates again at `now`: a
/// signer may have been kept since it was made.
pub fn sign_digest(signer: &Signer<'_>, digest: Digest, value: &[u8], now: i64) -> Result<Vec<u8>, PkiError> {
    if let Use::Leaf(cert, _) = &signer.usage {
        check_now(now)?;
        let parsed = X509::from_der(cert).map_err(|_| PkiError::Malformed)?;
        if !valid_at(&parsed, now)? {
            return Err(PkiError::CertificateNotValid);
        }
    }
    sign_checked(signer, digest, value)
}

/// `sign_digest` once the dates have been seen to
fn sign_checked(signer: &Signer<'_>, digest: Digest, value: &[u8]) -> Result<Vec<u8>, PkiError> {
    let key = signer.key;
    check_length(digest, value)?;
    let rsa = is_rsa(key)?;
    let pkey = key.pkey()?;
    let mut context = PkeyCtx::new(&pkey)?;
    context.sign_init()?;
    if rsa {
        context.set_rsa_padding(Padding::PKCS1)?;
    }
    context.set_signature_md(match digest {
        Digest::Sha256 => Md::sha256(),
        Digest::Sha384 => Md::sha384(),
    })?;
    let mut signature = Vec::new();
    context.sign_to_vec(value, &mut signature)?;
    Ok(signature)
}

fn attribute(oid: &str, value: &[u8]) -> Vec<u8> {
    der::seq(&[&der::oid(oid), &der::set_of(vec![value.to_vec()])])
}

/// A detached CMS SignedData over a PDF's digest: the digest in, the bytes of the signature field out. It carries the
/// certificate and its chain, and the time from the service's clock, which a reader shows and does not trust.
pub fn sign_pdf_digest(signer: &Signer<'_>, chain: &[Vec<u8>], digest: Digest, value: &[u8], now: i64) -> Result<Vec<u8>, PkiError> {
    let Use::Leaf(cert, Purpose::Pdf) = &signer.usage else {
        return Err(PkiError::WrongCertificate);
    };
    let key = signer.key;
    check_now(now)?;
    check_length(digest, value)?;
    let parsed = X509::from_der(cert).map_err(|_| PkiError::Malformed)?;
    // The signer was checked when it was made, which may be a while ago
    if !valid_at(&parsed, now)? {
        return Err(PkiError::CertificateNotValid);
    }
    let digest_oid = if digest == Digest::Sha256 { SHA256 } else { SHA384 };
    let digest_algorithm = der::seq(&[&der::oid(digest_oid), &der::null()]);
    let signature_algorithm = if is_rsa(key)? {
        let oid = if digest == Digest::Sha256 { "1.2.840.113549.1.1.11" } else { "1.2.840.113549.1.1.12" };
        der::seq(&[&der::oid(oid), &der::null()])
    } else {
        der::seq(&[&der::oid(if digest == Digest::Sha256 { "1.2.840.10045.4.3.2" } else { "1.2.840.10045.4.3.3" })])
    };

    // ESSCertIDv2 with SHA-256, its default and so left out, which ties the signature to this certificate (RFC 5035)
    let certificate_id = der::seq(&[&der::seq(&[&der::seq(&[&der::octets(&sha256(cert))])])]);
    let attributes = der::set_of(vec![
        attribute(CONTENT_TYPE, &der::oid(DATA)),
        attribute(MESSAGE_DIGEST, &der::octets(value)),
        attribute(SIGNING_TIME, &der::time(now)),
        attribute(SIGNING_CERTIFICATE_V2, &certificate_id),
    ]);
    // Signed as a SET, stored under [0]: the two encodings differ in that one byte
    let attributes_digest: Vec<u8> = match digest {
        Digest::Sha256 => sha256(&attributes).to_vec(),
        Digest::Sha384 => sha384(&attributes).to_vec(),
    };
    let signature = sign_checked(signer, digest, &attributes_digest)?;
    let mut signed_attributes = attributes;
    signed_attributes[0] = der::CONTEXT_0;

    let serial = parsed.serial_number().to_bn()?.to_vec();
    let signer_id = der::seq(&[&parsed.issuer_name().to_der()?, &der::uint(&serial)]);
    let signer_info = der::seq(&[&der::uint(&[1]), &signer_id, &digest_algorithm, &signed_attributes, &signature_algorithm, &der::octets(&signature)]);

    let mut certificates: Vec<&[u8]> = vec![cert.as_slice()];
    certificates.extend(chain.iter().map(Vec::as_slice));
    let signed_data = der::seq(&[
        &der::uint(&[1]),
        &der::set_of(vec![digest_algorithm.clone()]),
        &der::seq(&[&der::oid(DATA)]),
        &der::tlv(der::CONTEXT_0, &certificates),
        &der::set_of(vec![signer_info]),
    ]);
    Ok(der::seq(&[&der::oid(SIGNED_DATA), &der::tlv(der::CONTEXT_0, &[&signed_data])]))
}
