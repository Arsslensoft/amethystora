//! What a certificate says about what it may do, read from its DER: whether it is a CA, its key usage and its extended key
//! usages. Issuing and signing both decide on these, so they are read in one place, from the certificate's own bytes.

use openssl::x509::X509Ref;

use super::{der, PkiError};

const OID_KEY_USAGE: &str = "2.5.29.15";
const OID_BASIC_CONSTRAINTS: &str = "2.5.29.19";
const OID_EXTENDED_KEY_USAGE: &str = "2.5.29.37";

const BOOLEAN: u8 = 0x01;
const BIT_STRING: u8 = 0x03;
/// `[3]`, the extensions of a TBSCertificate
const EXTENSIONS: u8 = 0xa3;

/// keyCertSign and cRLSign, bits 5 and 6 of the first byte
pub(super) const CERT_SIGN: u8 = 0x04;
pub(super) const CRL_SIGN: u8 = 0x02;
pub(super) const DIGITAL_SIGNATURE: u8 = 0x80;

#[derive(Default)]
pub(super) struct Facts {
    /// `Some(is_ca)` when the certificate has a Basic Constraints extension
    pub ca: Option<bool>,
    /// The first byte of the Key Usage bits, when it has the extension
    pub usage: Option<u8>,
    /// Each extended key usage's identifier, as the bytes inside its DER, when it has the extension
    pub purposes: Option<Vec<Vec<u8>>>,
}

impl Facts {
    /// Whether the extended key usages are exactly these, in any order. A certificate with none has them all, so it is not.
    pub fn purposes_are(&self, dotted: &[&str]) -> bool {
        let Some(have) = &self.purposes else { return false };
        let want: Vec<Vec<u8>> = dotted.iter().map(|oid| der::oid(oid)[2..].to_vec()).collect();
        have.len() == want.len() && want.iter().all(|oid| have.contains(oid))
    }
}

/// Reads a certificate OpenSSL has already parsed, so a malformed one is `Malformed` and not a guess
pub(super) fn facts(cert: &X509Ref) -> Result<Facts, PkiError> {
    let bytes = cert.to_der().map_err(|_| PkiError::Malformed)?;
    let (_, certificate, _) = der::read(&bytes).ok_or(PkiError::Malformed)?;
    let (_, mut tbs, _) = der::read(certificate).ok_or(PkiError::Malformed)?;
    let mut extensions = None;
    while !tbs.is_empty() {
        let (tag, contents, rest) = der::read(tbs).ok_or(PkiError::Malformed)?;
        if tag == EXTENSIONS {
            extensions = Some(contents);
        }
        tbs = rest;
    }
    let mut found = Facts::default();
    let Some(extensions) = extensions else { return Ok(found) };
    let (_, mut list, _) = der::read(extensions).ok_or(PkiError::Malformed)?;
    while !list.is_empty() {
        let (_, extension, rest) = der::read(list).ok_or(PkiError::Malformed)?;
        list = rest;
        let (_, oid, mut after) = der::read(extension).ok_or(PkiError::Malformed)?;
        // `critical` is left out when false
        if let Some((BOOLEAN, _, rest)) = der::read(after) {
            after = rest;
        }
        let (_, value, _) = der::read(after).ok_or(PkiError::Malformed)?;
        let is = |dotted: &str| der::oid(dotted)[2..] == *oid;
        if is(OID_BASIC_CONSTRAINTS) && found.ca.is_none() {
            let (_, body, _) = der::read(value).ok_or(PkiError::Malformed)?;
            found.ca = Some(matches!(der::read(body), Some((BOOLEAN, [flag], _)) if *flag != 0));
        } else if is(OID_KEY_USAGE) && found.usage.is_none() {
            let (tag, bits, _) = der::read(value).ok_or(PkiError::Malformed)?;
            // The count of unused bits, then the bits
            found.usage = Some(match (tag, bits) {
                (BIT_STRING, [_, first, ..]) => *first,
                (BIT_STRING, _) => 0,
                _ => return Err(PkiError::Malformed),
            });
        } else if is(OID_EXTENDED_KEY_USAGE) && found.purposes.is_none() {
            let (_, mut body, _) = der::read(value).ok_or(PkiError::Malformed)?;
            let mut purposes = Vec::new();
            while !body.is_empty() {
                let (_, purpose, rest) = der::read(body).ok_or(PkiError::Malformed)?;
                purposes.push(purpose.to_vec());
                body = rest;
            }
            found.purposes = Some(purposes);
        }
    }
    Ok(found)
}
