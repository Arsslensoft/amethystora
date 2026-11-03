//! A certificate's way out: its chain as PEM, which needs no approval, and PFX with a typed password, which `osslsigncode`,
//! `signtool` and `jarsigner` take.

use openssl::nid::Nid;
use openssl::pkcs12::Pkcs12;
use openssl::stack::Stack;
use openssl::x509::X509;
use zeroize::Zeroizing;

use super::{Digest, PkiError};
use crate::keys::{PrivateKey, PBKDF2_ROUNDS};

const MAX_PASSWORD: usize = 256;

/// The certificate, then the chain above it
pub fn pem_chain(cert: &[u8], chain: &[Vec<u8>]) -> Result<String, PkiError> {
    let mut out = String::new();
    for der in std::iter::once(cert).chain(chain.iter().map(Vec::as_slice)) {
        X509::from_der(der).map_err(|_| PkiError::Malformed)?;
        out.push_str(&crate::keys::pem("CERTIFICATE", der));
    }
    Ok(out)
}

/// PKCS#12 with its key and certificates under AES-256-CBC and its MAC HMAC-SHA-256, both stretched as far as the PKCS#8
/// export is, so that neither is the cheap way in. The crate's builder copies the password into a string it does not
/// wipe; there is no other door to OpenSSL's `PKCS12_create`.
pub fn pfx(key: &PrivateKey, cert: &[u8], chain: &[Vec<u8>], name: &str, password: &str) -> Result<Zeroizing<Vec<u8>>, PkiError> {
    // The crate's builder unwraps a CString of each: a NUL here would end the service
    if password.is_empty() || password.chars().count() > MAX_PASSWORD || password.contains('\0') {
        return Err(PkiError::BadPassword);
    }
    if !super::names::check_text(name, super::MAX_NAME) {
        return Err(PkiError::BadField("name"));
    }
    let pkey = key.pkey()?;
    let cert = X509::from_der(cert).map_err(|_| PkiError::Malformed)?;
    if !pkey.public_eq(&*cert.public_key()?) {
        return Err(PkiError::IssuerKeyMismatch);
    }
    let mut stack = Stack::new()?;
    for der in chain {
        stack.push(X509::from_der(der).map_err(|_| PkiError::Malformed)?)?;
    }
    let mut builder = Pkcs12::builder();
    builder
        .name(name)
        .pkey(&pkey)
        .cert(&cert)
        .ca(stack)
        .key_algorithm(Nid::AES_256_CBC)
        .cert_algorithm(Nid::AES_256_CBC)
        .key_iter(PBKDF2_ROUNDS)
        .mac_iter(PBKDF2_ROUNDS)
        .mac_md(Digest::Sha256.md());
    Ok(Zeroizing::new(builder.build2(password)?.to_der()?))
}
