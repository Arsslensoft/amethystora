//! The machine's root, its CAs, and every certificate profile (plans/done/PLAN-vault.md, Signing certificates, The machine's
//! root, Its other purposes). What a certificate may say is the table in `profiles`, chosen in code and never in a
//! request. No socket and no files: bytes in and out, a key an OpenSSL key only for the length of one call.

pub mod der;
mod cms;
mod export;
mod facts;
mod names;
mod profiles;

use std::fmt;
use std::net::IpAddr;

use openssl::asn1::{Asn1Integer, Asn1Object, Asn1OctetString, Asn1Time, Asn1Type};
use openssl::bn::BigNum;
use openssl::error::ErrorStack;
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Public};
use openssl::sha::sha256;
use openssl::x509::{X509Builder, X509Extension, X509Name, X509NameBuilder, X509Ref, X509};

use crate::keys::{classify, hex, KeyError, KeyType, PrivateKey, PublicForms};

pub use cms::{sign_digest, sign_pdf_digest, Purpose, Signer};
pub use export::{pem_chain, pfx};
pub use names::{check_name, MAX_NAME};
pub use profiles::{Authority, Profile, ROOT_NAME};

use facts::{facts, CERT_SIGN};
use names::{check_email, check_local_name, check_text, check_web, host_shaped, LocalName, PERMITTED_DNS, PERMITTED_IP};
use profiles::{Alternative, Limits, Purposes, Spec, SubjectShape};

const DAY: i64 = 86400;
/// 2000-01-01 and 9999-12-31 at the last second: a time outside them is a mistake, and is refused before it is used
const FIRST_TIME: i64 = 946_684_800;
const LAST_TIME: i64 = 253_402_300_799;
/// Twenty years, the root's validity
const ROOT_DAYS: i64 = 7305;

const OID_SUBJECT_KEY_ID: &str = "2.5.29.14";
const OID_KEY_USAGE: &str = "2.5.29.15";
const OID_ALT_NAME: &str = "2.5.29.17";
const OID_BASIC_CONSTRAINTS: &str = "2.5.29.19";
const OID_NAME_CONSTRAINTS: &str = "2.5.29.30";
const OID_AUTHORITY_KEY_ID: &str = "2.5.29.35";
const OID_EXTENDED_KEY_USAGE: &str = "2.5.29.37";

/// keyCertSign and cRLSign
const USAGE_CA: [u8; 4] = [0x03, 0x02, 0x01, 0x06];
const USAGE_SIGN: [u8; 4] = [0x03, 0x02, 0x07, 0x80];
/// digitalSignature and keyEncipherment
const USAGE_SIGN_ENCIPHER: [u8; 4] = [0x03, 0x02, 0x05, 0xa0];

/// Why nothing was signed. A name's refusal says which field, and never repeats what was typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PkiError {
    UnknownProfile,
    /// This CA does not issue this profile
    WrongIssuer,
    /// The profile does not take this type of key
    KeyNotAllowed,
    /// A field that fails its check: a control character, too long, or not the shape it has to be
    BadField(&'static str),
    /// A field the profile needs that the request left out
    MissingField(&'static str),
    /// A field this profile has no place for
    NotInProfile(&'static str),
    /// A name or address the root's limits refuse
    OutsideLimits,
    /// A signer's name a verifier would read as a host name, and refuse under the root's limits
    NameLooksLikeHost,
    /// The issuer's certificate has expired, or has not started
    IssuerNotValid,
    /// The issuer's key is not the one its certificate names
    IssuerKeyMismatch,
    /// Only SHA-256 and SHA-384, and for a CA SHA-256
    DigestNotAllowed,
    /// A digest of the wrong length for its algorithm
    BadDigest,
    /// A password that is empty, too long, or holds a NUL
    BadPassword,
    /// A certificate or chain that could not be read
    Malformed,
    Key(KeyError),
    /// A time before 2000 or after 9999
    BadTime,
    /// The key is the issuing CA's, or the root's
    KeyIsACas,
    /// A certificate that is not the leaf of the profile it is asked to sign for: a CA's, or another purpose's
    WrongCertificate,
    /// A certificate past its dates, or not yet within them
    CertificateNotValid,
    Failed,
}

impl fmt::Display for PkiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PkiError::UnknownProfile => f.write_str("no such kind of certificate"),
            PkiError::WrongIssuer => f.write_str("this CA does not issue that kind of certificate"),
            PkiError::KeyNotAllowed => f.write_str("that kind of certificate does not take this type of key"),
            PkiError::BadField(field) => write!(f, "the {field} is not valid"),
            PkiError::MissingField(field) => write!(f, "the {field} is missing"),
            PkiError::NotInProfile(field) => write!(f, "this kind of certificate has no {field}"),
            PkiError::OutsideLimits => f.write_str("only local names and private addresses are allowed"),
            PkiError::NameLooksLikeHost => f.write_str("a name with dots and no spaces reads as a host name"),
            PkiError::IssuerNotValid => f.write_str("the issuing certificate is not valid now"),
            PkiError::IssuerKeyMismatch => f.write_str("the issuing key does not fit its certificate"),
            PkiError::DigestNotAllowed => f.write_str("that hash is not allowed here"),
            PkiError::BadDigest => f.write_str("a digest of the wrong length"),
            PkiError::BadPassword => f.write_str("a password must be 1 to 256 characters, with no NUL"),
            PkiError::Malformed => f.write_str("not a certificate this can read"),
            PkiError::Key(error) => error.fmt(f),
            PkiError::BadTime => f.write_str("the time is not between 2000 and 9999"),
            PkiError::KeyIsACas => f.write_str("that key belongs to a certificate authority"),
            PkiError::WrongCertificate => f.write_str("that certificate is not one to sign this with"),
            PkiError::CertificateNotValid => f.write_str("the certificate is not valid now"),
            PkiError::Failed => f.write_str("the operation failed"),
        }
    }
}

impl std::error::Error for PkiError {}

impl From<ErrorStack> for PkiError {
    fn from(_: ErrorStack) -> PkiError {
        PkiError::Failed
    }
}

impl From<KeyError> for PkiError {
    fn from(error: KeyError) -> PkiError {
        PkiError::Key(error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Digest {
    Sha256,
    Sha384,
}

impl Digest {
    pub(crate) fn md(self) -> MessageDigest {
        match self {
            Digest::Sha256 => MessageDigest::sha256(),
            Digest::Sha384 => MessageDigest::sha384(),
        }
    }

    /// Its length in bytes
    pub fn size(self) -> usize {
        match self {
            Digest::Sha256 => 32,
            Digest::Sha384 => 48,
        }
    }
}

/// A CA able to issue: its certificate, its key, and the chain above it, the root last. The root has no chain.
pub struct Issuer<'a> {
    pub authority: Authority,
    pub cert: &'a [u8],
    pub key: &'a CaKey,
    pub chain: &'a [Vec<u8>],
}

/// What a CA's or the root's key is stored as begins with this, so that it is never read back as a [`PrivateKey`]
const CA_KEY_MAGIC: &[u8] = b"amethystora-ca-key\0";

/// The root's or a CA's key, a type of its own: nothing that signs for a leaf takes one, and what it is stored as does not
/// read back as a [`PrivateKey`]. It only issues, through [`issue`] and [`make_root`].
pub struct CaKey(PrivateKey);

impl fmt::Debug for CaKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CaKey({})", self.0.key_type().name())
    }
}

impl CaKey {
    /// RSA 4096 for the root, RSA 3072 for a CA, as the plan's table has them. Slow: off the service's main loop.
    pub fn generate(authority: Authority) -> Result<CaKey, PkiError> {
        let key_type = if authority == Authority::Root { KeyType::Rsa4096 } else { KeyType::Rsa3072 };
        Ok(CaKey(crate::keys::generate(key_type)?))
    }

    /// What `to_stored` gave, from the service's own sealed storage
    pub fn from_stored(bytes: &[u8]) -> Result<CaKey, PkiError> {
        let key = bytes.strip_prefix(CA_KEY_MAGIC).ok_or(PkiError::Key(KeyError::Malformed))?;
        let key_type = PKey::private_key_from_pkcs8(key).ok().as_deref().and_then(classify).ok_or(PkiError::Key(KeyError::Malformed))?;
        if !matches!(key_type, KeyType::Rsa3072 | KeyType::Rsa4096) {
            return Err(PkiError::Key(KeyError::Malformed));
        }
        Ok(CaKey(PrivateKey::from_bytes(key_type, key)?))
    }

    /// What the service seals and keeps beside the root's certificate
    pub fn to_stored(&self) -> zeroize::Zeroizing<Vec<u8>> {
        let mut out = zeroize::Zeroizing::new(Vec::with_capacity(CA_KEY_MAGIC.len() + self.0.as_bytes().len()));
        out.extend_from_slice(CA_KEY_MAGIC);
        out.extend_from_slice(self.0.as_bytes());
        out
    }

    pub fn public(&self) -> Result<PublicForms, PkiError> {
        Ok(self.0.public()?)
    }

    fn key_type(&self) -> KeyType {
        self.0.key_type()
    }

    fn pkey(&self) -> Result<PKey<openssl::pkey::Private>, PkiError> {
        Ok(self.0.pkey()?)
    }
}

/// What a request can say. A profile takes only some of these, and any other given is refused.
#[derive(Default)]
pub struct Request<'a> {
    /// The profile by name. An unknown one is refused before anything else is looked at.
    pub profile: &'a str,
    /// The new certificate's public key, a SubjectPublicKeyInfo in DER: `PrivateKey::public().spki_der`
    pub public_key: &'a [u8],
    pub digest: Option<Digest>,
    /// Seconds since the epoch, from the service's clock
    pub now: i64,
    /// The signer's name, the person's for their CA, or the name of a login to a service
    pub name: Option<&'a str>,
    pub organization: Option<&'a str>,
    /// Two letters
    pub country: Option<&'a str>,
    /// The account, for a login to a service
    pub login: Option<&'a str>,
    pub email: Option<&'a str>,
    pub web: Option<&'a str>,
    /// Local TLS: host names and addresses
    pub local_names: &'a [&'a str],
}

/// A certificate in DER, and the chain that comes with it, the root last
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issued {
    pub cert: Vec<u8>,
    pub chain: Vec<Vec<u8>>,
}

/// The machine's number: the start of the SHA-256 of the root's public key. Never the host name or `/etc/machine-id`,
/// since signed documents travel.
pub fn machine_number(root_spki: &[u8]) -> String {
    hex(&sha256(root_spki)[..4]).to_uppercase()
}

fn check_now(now: i64) -> Result<(), PkiError> {
    if (FIRST_TIME..=LAST_TIME).contains(&now) {
        Ok(())
    } else {
        Err(PkiError::BadTime)
    }
}

fn extension(oid: &str, critical: bool, value: &[u8]) -> Result<X509Extension, ErrorStack> {
    X509Extension::new_from_der(&*Asn1Object::from_str(oid)?, critical, &*Asn1OctetString::new_from_bytes(value)?)
}

/// RFC 7093's first method, close enough: 160 bits of a SHA-256. Nothing but this and the issuer's matching reads it.
fn key_id(spki: &[u8]) -> [u8; 20] {
    let mut id = [0u8; 20];
    id.copy_from_slice(&sha256(spki)[..20]);
    id
}

fn random_serial() -> Result<Asn1Integer, ErrorStack> {
    // 128 bits with the top one free, and not all zero: a positive, non-zero INTEGER
    loop {
        let mut bytes = [0u8; 16];
        openssl::rand::rand_bytes(&mut bytes)?;
        if bytes.iter().any(|byte| *byte != 0) {
            return BigNum::from_slice(&bytes)?.to_asn1_integer();
        }
    }
}

fn name_of(entries: &[(Nid, &str, Asn1Type)]) -> Result<X509Name, PkiError> {
    let mut builder = X509NameBuilder::new()?;
    for (nid, value, kind) in entries {
        builder.append_entry_by_nid_with_type(*nid, value, *kind)?;
    }
    Ok(builder.build())
}

/// What is the same in every certificate: the serial, dates and names, in version 3
fn start(subject: &X509Name, issuer: &X509Name, spki: &PKey<Public>, not_before: i64, not_after: i64) -> Result<X509Builder, ErrorStack> {
    let mut builder = X509::builder()?;
    builder.set_version(2)?;
    builder.set_serial_number(&*random_serial()?)?;
    builder.set_subject_name(subject)?;
    builder.set_issuer_name(issuer)?;
    builder.set_not_before(&*Asn1Time::from_unix(not_before as _)?)?;
    builder.set_not_after(&*Asn1Time::from_unix(not_after as _)?)?;
    builder.set_pubkey(spki)?;
    Ok(builder)
}

/// The root's limits, from the same table the service's own check reads
fn permitted_names() -> Vec<u8> {
    let mut subtrees: Vec<Vec<u8>> = PERMITTED_DNS.iter().map(|name| der::seq(&[&der::tlv(0x82, &[name.as_bytes()])])).collect();
    for (address, mask) in PERMITTED_IP {
        let mut bytes = match address {
            IpAddr::V4(address) => address.octets().to_vec(),
            IpAddr::V6(address) => address.octets().to_vec(),
        };
        bytes.extend_from_slice(mask);
        subtrees.push(der::seq(&[&der::tlv(0x87, &[&bytes])]));
    }
    let parts: Vec<&[u8]> = subtrees.iter().map(Vec::as_slice).collect();
    der::seq(&[&der::tlv(0xa0, &parts)])
}

/// No name at all: an empty DNS name is every DNS name, and an address with an empty mask every address of its family
fn no_names() -> Vec<u8> {
    let any_v4 = der::seq(&[&der::tlv(0x87, &[&[0u8; 8]])]);
    let any_v6 = der::seq(&[&der::tlv(0x87, &[&[0u8; 32]])]);
    let any_dns = der::seq(&[&der::tlv(0x82, &[])]);
    der::seq(&[&der::tlv(0xa1, &[&any_dns, &any_v4, &any_v6])])
}

/// The machine's root, and its public key is what numbers the machine. Takes only an RSA 4096 key, which
/// `generate(KeyType::Rsa4096)` makes, off the service's main loop.
pub fn make_root(key: &CaKey, now: i64) -> Result<Vec<u8>, PkiError> {
    check_now(now)?;
    // Its end too: a year past 9999 is written in a form `describe` cannot read
    check_now(now + ROOT_DAYS * DAY)?;
    if key.key_type() != KeyType::Rsa4096 {
        return Err(PkiError::KeyNotAllowed);
    }
    let pkey = key.pkey()?;
    let spki = pkey.public_key_to_der()?;
    let organizational_unit = format!("Machine {}", machine_number(&spki));
    let subject = name_of(&[
        (Nid::ORGANIZATIONALUNITNAME, &organizational_unit, Asn1Type::UTF8STRING),
        (Nid::COMMONNAME, ROOT_NAME, Asn1Type::UTF8STRING),
    ])?;
    let public = PKey::public_key_from_der(&spki)?;
    let mut builder = start(&subject, &subject, &public, now, now + ROOT_DAYS * DAY)?;
    builder.append_extension(extension(OID_BASIC_CONSTRAINTS, true, &der::seq(&[&[0x01, 0x01, 0xff], &der::uint(&[1])]))?)?;
    builder.append_extension(extension(OID_KEY_USAGE, true, &USAGE_CA)?)?;
    builder.append_extension(extension(OID_NAME_CONSTRAINTS, true, &permitted_names())?)?;
    builder.append_extension(extension(OID_SUBJECT_KEY_ID, false, &der::octets(&key_id(&spki)))?)?;
    builder.sign(&pkey, MessageDigest::sha256())?;
    Ok(builder.build().to_der()?)
}

/// A new root in place of one whose key was stolen, which `ame vault root rotate` asks for. The old root's certificates
/// stay as they were for whoever trusted it, and each CA is made again at its next certificate: `ca_is_current`.
pub fn rotate_root(now: i64) -> Result<(CaKey, Vec<u8>), PkiError> {
    let key = CaKey::generate(Authority::Root)?;
    let cert = make_root(&key, now)?;
    Ok((key, cert))
}

/// Whether a CA still hangs from this root and is within its dates, and so can go on issuing
pub fn ca_is_current(ca: &[u8], root: &[u8], now: i64) -> Result<bool, PkiError> {
    check_now(now)?;
    let ca = X509::from_der(ca).map_err(|_| PkiError::Malformed)?;
    let root = X509::from_der(root).map_err(|_| PkiError::Malformed)?;
    Ok(ca.verify(&*root.public_key()?).unwrap_or(false) && valid_at(&ca, now)?)
}

fn unix_time(time: &openssl::asn1::Asn1TimeRef) -> Result<i64, ErrorStack> {
    let difference = Asn1Time::from_unix(0)?.diff(time)?;
    Ok(i64::from(difference.days) * DAY + i64::from(difference.secs))
}

fn valid_at(cert: &X509Ref, now: i64) -> Result<bool, ErrorStack> {
    Ok(unix_time(cert.not_before())? <= now && now < unix_time(cert.not_after())?)
}

/// An authority's certificate and the root above it, checked to be what they say before anything is issued from them: the
/// issuer a CA with the key to its certificate, which the root signed and which the root still stands behind (a rotation leaves
/// the old CAs behind), within their dates. `issue` trusts nothing the caller labelled.
fn check_issuer(issuer: &Issuer<'_>, now: i64) -> Result<(X509, X509), PkiError> {
    let cert = X509::from_der(issuer.cert).map_err(|_| PkiError::Malformed)?;
    let expected_chain = usize::from(issuer.authority != Authority::Root);
    if issuer.chain.len() != expected_chain {
        return Err(PkiError::WrongIssuer);
    }
    let root = match issuer.chain.first() {
        Some(root) => X509::from_der(root).map_err(|_| PkiError::Malformed)?,
        None => cert.clone(),
    };
    // The root: a CA that signed itself, under its own name
    let root_facts = facts(&root)?;
    let self_signed = root.subject_name().to_der()? == root.issuer_name().to_der()? && root.verify(&*root.public_key()?)?;
    if !self_signed || root_facts.ca != Some(true) || !has_name(&root, ROOT_NAME, false) || root_facts.usage.unwrap_or(0) & CERT_SIGN == 0 {
        return Err(PkiError::WrongIssuer);
    }
    if issuer.authority != Authority::Root {
        // A CA of the kind it says: a CA that can sign certificates, with the name and purposes that kind has
        let spec = Profile::of_authority(issuer.authority).ok_or(PkiError::WrongIssuer)?;
        let own = facts(&cert)?;
        let named = match spec.subject {
            SubjectShape::PersonCa => has_name(&cert, profiles::PERSON_CA_SUFFIX, true),
            SubjectShape::MachineCa(name) => has_name(&cert, name, false),
            _ => false,
        };
        if own.ca != Some(true) || own.usage.unwrap_or(0) & CERT_SIGN == 0 || !named || !own.purposes_are(spec.purposes.oids()) {
            return Err(PkiError::WrongIssuer);
        }
        // Hung from this root, or left behind by a rotation
        if cert.issuer_name().to_der()? != root.subject_name().to_der()? || !cert.verify(&*root.public_key()?)? {
            return Err(PkiError::IssuerNotValid);
        }
    }
    for link in [&cert, &root] {
        if !valid_at(link, now)? {
            return Err(PkiError::IssuerNotValid);
        }
    }
    if !issuer.key.pkey()?.public_eq(&*cert.public_key()?) {
        return Err(PkiError::IssuerKeyMismatch);
    }
    Ok((cert, root))
}

/// Whether a certificate's Common Name is `name`, or ends with it
fn has_name(cert: &X509Ref, name: &str, suffix: bool) -> bool {
    cert.subject_name().entries_by_nid(Nid::COMMONNAME).any(|entry| {
        entry.data().to_string().is_ok_and(|common| if suffix { common.ends_with(name) } else { common == name })
    })
}

/// What `issue` would refuse of a request, with no CA and no key yet: so that it is refused before a person is asked, and
/// before a root is made for nothing
pub fn check(request: &Request<'_>, key_type: Option<KeyType>) -> Result<(), PkiError> {
    let spec = Profile::parse(request.profile).ok_or(PkiError::UnknownProfile)?.spec();
    if key_type.is_some_and(|key_type| !spec.keys.contains(&key_type)) {
        return Err(PkiError::KeyNotAllowed);
    }
    subject_of(spec, request, "00000000")?;
    alternative_names(spec, request)?;
    Ok(())
}

/// Issues one certificate of a profile. Everything a request says is checked before the issuer's key is touched.
pub fn issue(request: &Request<'_>, issuer: &Issuer<'_>) -> Result<Issued, PkiError> {
    check_now(request.now)?;
    let spec = Profile::parse(request.profile).ok_or(PkiError::UnknownProfile)?.spec();
    if issuer.authority != spec.issuer {
        return Err(PkiError::WrongIssuer);
    }
    let (issuer_cert, root_cert) = check_issuer(issuer, request.now)?;

    let public = PKey::public_key_from_der(request.public_key).map_err(|_| PkiError::Key(KeyError::Malformed))?;
    // A key written with anything past its canonical form is refused: the key's identifier is made from the bytes, and
    // the issuer's, which a verifier compares, from the form OpenSSL writes
    let spki = public.public_key_to_der()?;
    if spki != request.public_key {
        return Err(PkiError::Key(KeyError::Malformed));
    }
    // The issuer's own key, and the root's, certify nobody
    if public.public_eq(&*issuer_cert.public_key()?) || public.public_eq(&*root_cert.public_key()?) {
        return Err(PkiError::KeyIsACas);
    }
    let key_type = classify(&public).ok_or(PkiError::KeyNotAllowed)?;
    if !spec.keys.contains(&key_type) {
        return Err(PkiError::KeyNotAllowed);
    }
    let digest = request.digest.unwrap_or(Digest::Sha256);
    if spec.makes.is_some() && digest != Digest::Sha256 {
        return Err(PkiError::DigestNotAllowed);
    }

    let machine = machine_number(&root_cert.public_key()?.public_key_to_der()?);
    let subject = subject_of(spec, request, &machine)?;
    let alternative = alternative_names(spec, request)?;

    let issuer_ends = unix_time(issuer_cert.not_after())?;
    // A CA that would end before the certificates it can sign would is not made
    if spec.makes.is_some() && issuer_ends - request.now < i64::from(profiles::SIGNING_DAYS) * DAY {
        return Err(PkiError::IssuerNotValid);
    }
    let not_after = (request.now + i64::from(spec.days) * DAY).min(issuer_ends);

    let issuer_key = issuer.key.pkey()?;
    let issuer_spki = issuer_key.public_key_to_der()?;

    let mut builder = start(&subject, &to_name(issuer_cert.subject_name())?, &public, request.now, not_after)?;
    let basic = match spec.makes {
        Some(_) => der::seq(&[&[0x01, 0x01, 0xff], &der::uint(&[0])]),
        None => der::seq(&[]),
    };
    builder.append_extension(extension(OID_BASIC_CONSTRAINTS, true, &basic)?)?;
    let rsa = matches!(key_type, KeyType::Rsa3072 | KeyType::Rsa4096);
    let usage: &[u8] = if spec.makes.is_some() {
        &USAGE_CA
    } else if rsa && spec.rsa_enciphers {
        &USAGE_SIGN_ENCIPHER
    } else {
        &USAGE_SIGN
    };
    builder.append_extension(extension(OID_KEY_USAGE, true, usage)?)?;
    match spec.limits {
        Some(Limits::LocalNames) => builder.append_extension(extension(OID_NAME_CONSTRAINTS, true, &permitted_names())?)?,
        Some(Limits::NoNames) => builder.append_extension(extension(OID_NAME_CONSTRAINTS, true, &no_names())?)?,
        None => {}
    }
    let purposes: Vec<Vec<u8>> = spec.purposes.oids().iter().map(|oid| der::oid(oid)).collect();
    let purpose_parts: Vec<&[u8]> = purposes.iter().map(Vec::as_slice).collect();
    builder.append_extension(extension(OID_EXTENDED_KEY_USAGE, false, &der::seq(&purpose_parts))?)?;
    builder.append_extension(extension(OID_SUBJECT_KEY_ID, false, &der::octets(&key_id(&spki)))?)?;
    let authority_id = der::seq(&[&der::tlv(0x80, &[&key_id(&issuer_spki)])]);
    builder.append_extension(extension(OID_AUTHORITY_KEY_ID, false, &authority_id)?)?;
    if !alternative.is_empty() {
        let parts: Vec<&[u8]> = alternative.iter().map(Vec::as_slice).collect();
        builder.append_extension(extension(OID_ALT_NAME, false, &der::seq(&parts))?)?;
    }
    builder.sign(&issuer_key, digest.md())?;
    let mut chain = vec![issuer.cert.to_vec()];
    chain.extend_from_slice(issuer.chain);
    Ok(Issued { cert: builder.build().to_der()?, chain })
}

fn to_name(name: &openssl::x509::X509NameRef) -> Result<X509Name, ErrorStack> {
    X509Name::from_der(&name.to_der()?)
}

/// The person's name, or the signer's: what it cannot be in any certificate, and what only a leaf cannot be
fn checked_name<'a>(name: Option<&'a str>, field: &'static str, limit: usize, leaf: bool) -> Result<&'a str, PkiError> {
    let name = name.ok_or(PkiError::MissingField(field))?;
    if !check_name(name, limit) {
        return Err(PkiError::BadField(field));
    }
    if leaf && host_shaped(name) {
        return Err(PkiError::NameLooksLikeHost);
    }
    Ok(name)
}

/// The subject, built from the most significant part down, which reads `CN=..., OU=...` the way the plan writes it
fn subject_of(spec: &Spec, request: &Request<'_>, machine: &str) -> Result<X509Name, PkiError> {
    let takes_name = matches!(spec.subject, SubjectShape::PersonCa | SubjectShape::Signer { .. } | SubjectShape::Login);
    let takes_org = matches!(spec.subject, SubjectShape::Signer { organization: true });
    if !takes_name && request.name.is_some() {
        return Err(PkiError::NotInProfile("name"));
    }
    if !takes_org && request.organization.is_some() {
        return Err(PkiError::NotInProfile("organization"));
    }
    if !takes_org && request.country.is_some() {
        return Err(PkiError::NotInProfile("country"));
    }
    if !matches!(spec.subject, SubjectShape::Login) && request.login.is_some() {
        return Err(PkiError::NotInProfile("login"));
    }
    let machine_unit = format!("Machine {machine}");
    let utf8 = Asn1Type::UTF8STRING;
    let unit = Nid::ORGANIZATIONALUNITNAME;
    let common = Nid::COMMONNAME;
    match spec.subject {
        SubjectShape::PersonCa => {
            // What goes in front of " Code Signing CA" has to leave the whole within the attribute's limit
            let person = checked_name(request.name, "name", MAX_NAME - profiles::PERSON_CA_SUFFIX.len(), false)?;
            let name = format!("{person}{}", profiles::PERSON_CA_SUFFIX);
            name_of(&[(unit, profiles::OS_UNIT, utf8), (common, &name, utf8)])
        }
        SubjectShape::Signer { .. } => {
            let signer = checked_name(request.name, "name", MAX_NAME, true)?;
            let mut entries = Vec::new();
            if let Some(country) = request.country {
                if country.len() != 2 || !country.bytes().all(|byte| byte.is_ascii_uppercase()) {
                    return Err(PkiError::BadField("country"));
                }
                entries.push((Nid::COUNTRYNAME, country, Asn1Type::PRINTABLESTRING));
            }
            if let Some(organization) = request.organization {
                if !check_name(organization, MAX_NAME) {
                    return Err(PkiError::BadField("organization"));
                }
                entries.push((Nid::ORGANIZATIONNAME, organization, utf8));
            }
            entries.push((unit, profiles::VAULT_UNIT, utf8));
            entries.push((common, signer, utf8));
            name_of(&entries)
        }
        SubjectShape::MachineCa(name) => name_of(&[(unit, &machine_unit, utf8), (common, name, utf8)]),
        SubjectShape::Machine => {
            let name = format!("Machine {machine}");
            name_of(&[(unit, profiles::OS_UNIT, utf8), (common, &name, utf8)])
        }
        SubjectShape::Fixed(name) => name_of(&[(unit, profiles::OS_UNIT, utf8), (common, name, utf8)]),
        SubjectShape::Login => {
            let name = checked_name(request.name, "name", MAX_NAME, true)?;
            let login = request.login.ok_or(PkiError::MissingField("login"))?;
            // An account's name may hold what separates a name's parts (`CORP\ada`, `ada+work@example.org`): it is a value of its
            // own, and `describe` escapes it
            if !check_text(login, MAX_NAME) {
                return Err(PkiError::BadField("login"));
            }
            name_of(&[(unit, &machine_unit, utf8), (unit, login, utf8), (common, name, utf8)])
        }
    }
}

/// The Subject Alternative Names a profile takes, each as a tagged GeneralName
fn alternative_names(spec: &Spec, request: &Request<'_>) -> Result<Vec<Vec<u8>>, PkiError> {
    let takes_email = matches!(spec.alternative, Alternative::Email { .. } | Alternative::EmailOrWeb);
    let takes_web = matches!(spec.alternative, Alternative::EmailOrWeb);
    let takes_local = matches!(spec.alternative, Alternative::LocalNames);
    if !takes_email && request.email.is_some() {
        return Err(PkiError::NotInProfile("email"));
    }
    if !takes_web && request.web.is_some() {
        return Err(PkiError::NotInProfile("web address"));
    }
    if !takes_local && !request.local_names.is_empty() {
        return Err(PkiError::NotInProfile("host name"));
    }
    if matches!(spec.alternative, Alternative::Email { required: true }) && request.email.is_none() {
        return Err(PkiError::MissingField("email"));
    }
    let mut out = Vec::new();
    if let Some(email) = request.email {
        check_email(email).map_err(|_| PkiError::BadField("email"))?;
        out.push(der::tlv(0x81, &[email.as_bytes()]));
    }
    if let Some(web) = request.web {
        check_web(web).map_err(|_| PkiError::BadField("web address"))?;
        out.push(der::tlv(0x86, &[web.as_bytes()]));
    }
    if takes_local {
        if request.local_names.is_empty() {
            return Err(PkiError::MissingField("host name"));
        }
        if request.local_names.len() > 32 {
            return Err(PkiError::BadField("host name"));
        }
        for name in request.local_names {
            out.push(match check_local_name(name)? {
                LocalName::Dns(name) => der::tlv(0x82, &[name.as_bytes()]),
                LocalName::Ip(IpAddr::V4(address)) => der::tlv(0x87, &[&address.octets()]),
                LocalName::Ip(IpAddr::V6(address)) => der::tlv(0x87, &[&address.octets()]),
            });
        }
    }
    Ok(out)
}

/// What the window shows of a certificate
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Description {
    pub subject: String,
    pub issuer: String,
    pub not_before: i64,
    pub not_after: i64,
    /// Lowercase hex of the SHA-256 of the certificate
    pub sha256: String,
    pub key: PublicForms,
}

/// A value as RFC 4514 writes it, so that what a certificate holds can never read as more fields: the separators escaped, a
/// leading `#` or space and a trailing space, and a control character as its hex
fn escape_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let last = value.chars().count().saturating_sub(1);
    for (index, c) in value.chars().enumerate() {
        let edge = (index == 0 && matches!(c, '#' | ' ')) || (index == last && c == ' ');
        if edge || matches!(c, ',' | '+' | '"' | '\\' | '<' | '>' | ';' | '=') {
            out.push('\\');
            out.push(c);
        } else if c.is_control() {
            let mut bytes = [0u8; 4];
            for byte in c.encode_utf8(&mut bytes).bytes() {
                out.push_str(&format!("\\{byte:02x}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn printable_name(name: &openssl::x509::X509NameRef) -> String {
    // The least significant first, as `CN=..., OU=...`. Text somebody typed: the window shows it as text.
    let parts: Vec<String> = name
        .entries()
        .map(|entry| {
            let label = entry.object().nid().short_name().unwrap_or("?");
            let value = entry.data().to_string().unwrap_or_default();
            format!("{label}={}", escape_value(&value))
        })
        .collect();
    parts.into_iter().rev().collect::<Vec<_>>().join(", ")
}

pub fn describe(cert: &[u8]) -> Result<Description, PkiError> {
    let parsed = X509::from_der(cert).map_err(|_| PkiError::Malformed)?;
    Ok(Description {
        subject: printable_name(parsed.subject_name()),
        issuer: printable_name(parsed.issuer_name()),
        not_before: unix_time(parsed.not_before())?,
        not_after: unix_time(parsed.not_after())?,
        sha256: hex(&sha256(cert)),
        key: PublicForms::from_spki(&parsed.public_key()?.public_key_to_der()?)?,
    })
}

/// The purposes a profile's certificates carry, for tests and for the window to word
pub fn profile_purposes(profile: Profile) -> &'static [&'static str] {
    let purposes: Purposes = profile.spec().purposes;
    purposes.oids()
}
