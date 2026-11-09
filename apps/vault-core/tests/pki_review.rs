//! What the review of 2026-11-02 found in issuing (plans/vault/work/review-pki.md: M1, M2, M4, L1, L2, L4), each held to its
//! refusal. Nothing here needs another program.

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use amethystora_vault_core::keys::{generate, KeyError, KeyKind, KeyType, PrivateKey, PublicForms};
use amethystora_vault_core::pki::*;
use openssl::asn1::Asn1Time;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::x509::{X509NameBuilder, X509};

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

const DAY: i64 = 86400;

struct Machine {
    root_key: CaKey,
    root: Vec<u8>,
    account_key: CaKey,
    account: Issued,
    device_key: CaKey,
    device: Issued,
    local_key: CaKey,
    local: Issued,
}

fn make_ca(profile: &str, name: Option<&str>, root: &[u8], root_key: &CaKey, at: i64) -> (CaKey, Issued) {
    let authority = Profile::parse(profile).unwrap().makes().unwrap();
    let key = CaKey::generate(authority).unwrap();
    let spki = key.public().unwrap().spki_der;
    let root_issuer = Issuer { authority: Authority::Root, cert: root, key: root_key, chain: &[] };
    let issued = issue(&Request { profile, public_key: &spki, now: at, name, ..Request::default() }, &root_issuer).unwrap();
    (key, issued)
}

fn machine() -> &'static Machine {
    static MACHINE: OnceLock<Machine> = OnceLock::new();
    MACHINE.get_or_init(|| {
        let at = now();
        let (root_key, root) = rotate_root(at).unwrap();
        let (account_key, account) = make_ca("account-ca", Some("Ada Lovelace"), &root, &root_key, at);
        let (device_key, device) = make_ca("device-ca", None, &root, &root_key, at);
        let (local_key, local) = make_ca("local-services-ca", None, &root, &root_key, at);
        Machine { root_key, root, account_key, account, device_key, device, local_key, local }
    })
}

fn account(machine: &Machine) -> Issuer<'_> {
    Issuer { authority: Authority::AccountCa, cert: &machine.account.cert, key: &machine.account_key, chain: &machine.account.chain }
}

fn fresh_spki() -> Vec<u8> {
    generate(KeyType::EcdsaP256).unwrap().public().unwrap().spki_der
}

fn pdf(name: &str) -> Request<'_> {
    Request { profile: "pdf", name: Some(name), now: now(), ..Request::default() }
}

/// M1: the issuer is checked: a CA, the authority it claims to be, hanging from the root it names, current
#[test]
fn m1_an_issuer_is_what_it_says() {
    let machine = machine();
    let spki = fresh_spki();
    // A leaf and its key handed over as the account's CA
    let leaf_key = CaKey::generate(Authority::AccountCa).unwrap();
    let leaf_spki = leaf_key.public().unwrap().spki_der;
    let leaf = issue(&Request { public_key: &leaf_spki, ..pdf("Ada") }, &account(machine)).unwrap();
    let posing = Issuer { authority: Authority::AccountCa, cert: &leaf.cert, key: &leaf_key, chain: &leaf.chain[1..] };
    assert!(issue(&Request { profile: "code", public_key: &spki, name: Some("Ada"), now: now(), ..Request::default() }, &posing).is_err());
    // The Device Identity CA handed over as the account's
    let device_as_account = Issuer { authority: Authority::AccountCa, cert: &machine.device.cert, key: &machine.device_key, chain: &machine.device.chain };
    assert!(issue(&Request { public_key: &spki, ..pdf("Ada") }, &device_as_account).is_err());
    // A CA's chain left out, or naming another root
    let no_chain = Issuer { chain: &[], ..account(machine) };
    assert!(issue(&Request { public_key: &spki, ..pdf("Ada") }, &no_chain).is_err());
    let (new_root_key, new_root) = rotate_root(now()).unwrap();
    let other_chain = [new_root.clone()];
    let wrong_root = Issuer { chain: &other_chain, ..account(machine) };
    assert!(issue(&Request { public_key: &spki, ..pdf("Ada") }, &wrong_root).is_err());
    // After a rotation the old CA, with its old chain, issues nothing more once the service names the new root
    assert!(!ca_is_current(&machine.account.cert, &new_root, now()).unwrap());
    let _ = new_root_key;
    // The root handed over as a CA, and a CA as the root
    let root_as_ca = Issuer { authority: Authority::AccountCa, cert: &machine.root, key: &machine.root_key, chain: &[] };
    assert!(issue(&Request { public_key: &spki, ..pdf("Ada") }, &root_as_ca).is_err());
    let ca_spki = CaKey::generate(Authority::AccountCa).unwrap().public().unwrap().spki_der;
    let ca_as_root = Issuer { authority: Authority::Root, cert: &machine.account.cert, key: &machine.account_key, chain: &[] };
    let request = Request { profile: "account-ca", public_key: &ca_spki, name: Some("Eve"), now: now(), ..Request::default() };
    assert!(issue(&request, &ca_as_root).is_err());
    // What is right still issues
    assert!(issue(&Request { public_key: &spki, ..pdf("Ada") }, &account(machine)).is_ok());
}

/// M1: `issue` with a CA that rotation left behind
#[test]
fn m1_a_ca_left_behind_by_rotation_is_refused() {
    let at = now();
    let (old_key, old_root) = rotate_root(at).unwrap();
    let (ca_key, ca) = make_ca("account-ca", Some("Ada"), &old_root, &old_key, at);
    let (_, new_root) = rotate_root(at).unwrap();
    // The service hands over the CA with the root it has now: refused, as `ca_is_current` says
    let chain = [new_root];
    let issuer = Issuer { authority: Authority::AccountCa, cert: &ca.cert, key: &ca_key, chain: &chain };
    let refused = issue(&Request { public_key: &fresh_spki(), ..pdf("Ada") }, &issuer);
    assert_eq!(refused.unwrap_err(), PkiError::IssuerNotValid);
}

/// M2: the characters that separate a Distinguished Name's parts cannot be typed into one
#[test]
fn m2_dn_separators_are_refused() {
    let machine = machine();
    let spki = fresh_spki();
    for typed in ["Google LLC, C=US", "a=b", "a+b", "a;b", "a\"b", "a\\b", "a<b", "a>b", "#hash"] {
        let as_organization = issue(&Request { public_key: &spki, organization: Some(typed), ..pdf("Ada") }, &account(machine));
        assert_eq!(as_organization.unwrap_err(), PkiError::BadField("organization"), "{typed}");
        let as_name = issue(&Request { public_key: &spki, ..pdf(typed) }, &account(machine));
        assert_eq!(as_name.unwrap_err(), PkiError::BadField("name"), "{typed}");
    }
    // A hash inside a name is fine, as are the other punctuation marks names have
    assert!(issue(&Request { public_key: &spki, organization: Some("O'Brien & Sons No. #2"), ..pdf("Ada-Mae O'Brien") }, &account(machine)).is_ok());
}

/// K12: a service login's account is the account's name as the service has it, separators and all: a value of its own, which
/// `describe` escapes. What cannot be seen is still refused.
#[test]
fn k12_a_logins_account_takes_what_names_refuse() {
    let machine = machine();
    let device = Issuer { authority: Authority::DeviceCa, cert: &machine.device.cert, key: &machine.device_key, chain: &machine.device.chain };
    let spki = fresh_spki();
    let login = |account| Request { profile: "service-login", public_key: &spki, name: Some("Home VPN"), login: Some(account), now: now(), ..Request::default() };
    for (account, shown) in [("CORP\\ada", r"OU=CORP\\ada"), ("ada+work@example.org", r"OU=ada\+work@example.org"), ("O'Brien, Ada", r"OU=O'Brien\, Ada")] {
        let issued = issue(&login(account), &device).unwrap_or_else(|why| panic!("{account}: {why:?}"));
        let subject = describe(&issued.cert).unwrap().subject;
        assert!(subject.contains(shown), "{subject}");
    }
    for account in ["", " ada", "ada\u{200B}", "a\nb"] {
        assert_eq!(issue(&login(account), &device).unwrap_err(), PkiError::BadField("login"), "{account:?}");
    }
}

/// M2: `describe` escapes as RFC 4514 says, whatever a certificate from elsewhere holds
#[test]
fn m2_describe_escapes() {
    let key = PKey::from_ec_key(openssl::ec::EcKey::generate(&openssl::ec::EcGroup::from_curve_name(openssl::nid::Nid::X9_62_PRIME256V1).unwrap()).unwrap()).unwrap();
    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_text("O", "Google LLC, C=US").unwrap();
    name.append_entry_by_text("CN", "#Ada \"the\" <first>; a+b\\ ").unwrap();
    let name = name.build();
    let mut builder = X509::builder().unwrap();
    builder.set_version(2).unwrap();
    builder.set_subject_name(&name).unwrap();
    builder.set_issuer_name(&name).unwrap();
    builder.set_not_before(&Asn1Time::days_from_now(0).unwrap()).unwrap();
    builder.set_not_after(&Asn1Time::days_from_now(1).unwrap()).unwrap();
    builder.set_pubkey(&key).unwrap();
    builder.sign(&key, MessageDigest::sha256()).unwrap();
    let described = describe(&builder.build().to_der().unwrap()).unwrap();
    assert_eq!(described.subject, r#"CN=\#Ada \"the\" \<first\>\; a\+b\\\ , O=Google LLC\, C\=US"#);
}

/// L1: invisible characters, private use and unassigned code points cannot be typed into a name
#[test]
fn l1_invisible_characters_are_refused() {
    let machine = machine();
    let spki = fresh_spki();
    for odd in ['\u{061C}', '\u{206A}', '\u{206F}', '\u{200B}', '\u{2060}', '\u{00AD}', '\u{180E}', '\u{FFF9}', '\u{E0041}', '\u{3164}', '\u{115F}', '\u{034F}', '\u{E000}', '\u{0378}', '\u{10FFFD}', '\u{FDD0}'] {
        let typed = format!("Ada{odd}Lovelace");
        let refused = issue(&Request { public_key: &spki, ..pdf(&typed) }, &account(machine));
        assert_eq!(refused.unwrap_err(), PkiError::BadField("name"), "U+{:04X}", odd as u32);
    }
    // The joiners Persian and Indic names need stay
    assert!(issue(&Request { public_key: &spki, ..pdf("می\u{200C}خواهم") }, &account(machine)).is_ok());
    assert!(issue(&Request { public_key: &spki, ..pdf("क्\u{200D}ष") }, &account(machine)).is_ok());
}

/// K6: what reads as a separator without being one is refused as the separator is, in a name and an organization
#[test]
fn k6_separators_look_alikes_are_refused() {
    let machine = machine();
    let spki = fresh_spki();
    for typed in ["Google LLC\u{FF0C} C\u{FF1D}US", "a\u{FE50}b", "a\u{FE66}b", "a\u{3001}b", "a\u{060C}b", "a\u{037E}b", "a\u{FF1B}b", "a\u{FF0B}b", "a\u{FF02}b", "a\u{FF3C}b"] {
        let as_organization = issue(&Request { public_key: &spki, organization: Some(typed), ..pdf("Ada") }, &account(machine));
        assert_eq!(as_organization.unwrap_err(), PkiError::BadField("organization"), "{typed}");
        let as_name = issue(&Request { public_key: &spki, ..pdf(typed) }, &account(machine));
        assert_eq!(as_name.unwrap_err(), PkiError::BadField("name"), "{typed}");
    }
    // The full stop and the middle dots of other scripts are no separators
    assert!(issue(&Request { public_key: &spki, organization: Some("株式会社。日本"), ..pdf("Ada\u{00B7}Lovelace") }, &account(machine)).is_ok());
}

/// K7: a name has to have something to read, and a joiner or variation selector is taken only where a script or an emoji needs it
#[test]
fn k7_blank_names_and_hidden_joiners_are_refused() {
    let machine = machine();
    let spki = fresh_spki();
    for typed in ["\u{200D}", "\u{200C}", "\u{2800}", "\u{FE0F}", "-", "Ba\u{200D}nk", "Bank\u{200C}", "12\u{200D}3", "Ada\u{FE00}", "Ada\u{FE0E}", "Ada\u{E0100}", "Ada\u{180B}", "Ada\u{1D159}", "Ada\u{16FE4}", "Ada \u{2800}"] {
        let refused = issue(&Request { public_key: &spki, ..pdf(typed) }, &account(machine));
        assert_eq!(refused.unwrap_err(), PkiError::BadField("name"), "{typed:?}");
    }
    // A joiner where Persian, an Indic script or an emoji needs it, a mark on a letter, and a name of digits
    for typed in ["می\u{200C}خواهم", "क्\u{200D}ष", "Ada \u{2615}\u{FE0F}", "Ada \u{1F468}\u{200D}\u{1F469}", "Ada \u{2764}\u{FE0F}\u{200D}\u{1F525}", "Jose\u{0301}", "42"] {
        assert!(issue(&Request { public_key: &spki, ..pdf(typed) }, &account(machine)).is_ok(), "{typed:?}");
    }
}

/// L2: a public key given with bytes after it is refused, in a request and in its public forms
#[test]
fn l2_a_public_key_with_trailing_bytes_is_refused() {
    let mut spki = fresh_spki();
    assert!(PublicForms::from_spki(&spki).is_ok());
    spki.extend_from_slice(&[0xb7; 4]);
    assert_eq!(PublicForms::from_spki(&spki).unwrap_err(), KeyError::Malformed);
    let refused = issue(&Request { public_key: &spki, ..pdf("Ada") }, &account(machine()));
    assert_eq!(refused.unwrap_err(), PkiError::Key(KeyError::Malformed));
}

/// L4: a time outside 2000 to 9999 is refused, and a CA is not made with less life left than its certificates have
#[test]
fn l4_times_are_bounded() {
    let key = CaKey::generate(Authority::Root).unwrap();
    for at in [0, -1, 946_684_799, 253_402_300_800, i64::MAX / 2, i64::MIN / 2] {
        assert_eq!(make_root(&key, at).unwrap_err(), PkiError::BadTime, "{at}");
        let refused = issue(&Request { public_key: &fresh_spki(), now: at, ..pdf("Ada") }, &account(machine()));
        assert_eq!(refused.unwrap_err(), PkiError::BadTime, "{at}");
    }
    // A root near its end makes no more CAs, since they could not last as long as what they issue
    let at = now();
    let root = make_root(&key, at).unwrap();
    let root_ends = describe(&root).unwrap().not_after;
    let ca_spki = CaKey::generate(Authority::AccountCa).unwrap().public().unwrap().spki_der;
    let root_issuer = Issuer { authority: Authority::Root, cert: &root, key: &key, chain: &[] };
    for left in [5, 100 * DAY, 459 * DAY] {
        let request = Request { profile: "account-ca", public_key: &ca_spki, name: Some("Ada"), now: root_ends - left, ..Request::default() };
        assert_eq!(issue(&request, &root_issuer).unwrap_err(), PkiError::IssuerNotValid, "{left}");
    }
    let request = Request { profile: "account-ca", public_key: &ca_spki, name: Some("Ada"), now: root_ends - 461 * DAY, ..Request::default() };
    assert!(issue(&request, &root_issuer).is_ok());
}

/// K9: a root whose twenty years would end past 9999 is not made, since what it would be could not be read back
#[test]
fn k9_a_root_ends_by_9999() {
    let key = CaKey::generate(Authority::Root).unwrap();
    let last_start = 253_402_300_799 - 7305 * DAY;
    for at in [last_start + 1, 253_000_000_000, 253_402_300_799] {
        assert_eq!(make_root(&key, at).unwrap_err(), PkiError::BadTime, "{at}");
    }
    let root = make_root(&key, last_start).unwrap();
    assert_eq!(describe(&root).unwrap().not_after, 253_402_300_799);
}

/// M4: a certificate is never issued for a key of the CA that issues it, or of the root above it
#[test]
fn m4_a_cas_own_key_is_not_certified() {
    let machine = machine();
    let own = machine.account_key.public().unwrap().spki_der;
    let refused = issue(&Request { public_key: &own, profile: "code", name: Some("Ada"), now: now(), ..Request::default() }, &account(machine));
    assert_eq!(refused.unwrap_err(), PkiError::KeyIsACas);
    let root = machine.root_key.public().unwrap().spki_der;
    let refused = issue(&Request { public_key: &root, ..pdf("Ada") }, &account(machine));
    assert!(refused.is_err());
    let root_issuer = Issuer { authority: Authority::Root, cert: &machine.root, key: &machine.root_key, chain: &[] };
    let refused = issue(&Request { profile: "device-ca", public_key: &root, now: now(), ..Request::default() }, &root_issuer);
    assert_eq!(refused.unwrap_err(), PkiError::KeyIsACas);
    let _: Option<PrivateKey> = None;
}

/// A CA's key as an ordinary one: what a mistake in the service's wiring would hand over. The stored form carries a mark that
/// keeps `PrivateKey::from_bytes` from reading it, so the test strips it.
fn plain(key: &CaKey) -> PrivateKey {
    let stored = key.to_stored();
    let der = &stored[b"amethystora-ca-key\0".len()..];
    PrivateKey::from_bytes(KeyType::Rsa3072, der).or_else(|_| PrivateKey::from_bytes(KeyType::Rsa4096, der)).unwrap()
}

fn leaf_of(profile: &str, issuer: &Issuer<'_>, key_type: KeyType) -> (PrivateKey, Issued) {
    let key = generate(key_type).unwrap();
    let spki = key.public().unwrap().spki_der;
    let name = if profile == "local-tls" { None } else { Some("Ada") };
    let email = if profile == "smime" { Some("ada@example.org") } else { None };
    let local: &[&str] = if profile == "local-tls" { &["printer.local"] } else { &[] };
    let issued = issue(&Request { profile, public_key: &spki, now: now(), name, email, local_names: local, ..Request::default() }, issuer).unwrap();
    (key, issued)
}

/// H1: a CA's or the root's key cannot sign a digest, in any of the three ways it might reach `sign_digest`
#[test]
fn h1_a_cas_key_signs_nothing() {
    let machine = machine();
    // The type: what a CA's key is stored as is not a private key, and `sign_digest` takes a `Signer`
    assert!(PrivateKey::from_bytes(KeyType::Rsa3072, &machine.account_key.to_stored()).is_err());
    // And if a mistake handed its bytes over as an ordinary key, no certificate makes a signer of it: the account CA's own,
    // the root's and the machine's
    for (key, cert) in [
        (plain(&machine.account_key), &machine.account.cert),
        (plain(&machine.device_key), &machine.device.cert),
        (plain(&machine.root_key), &machine.root),
    ] {
        for purpose in [Purpose::Pdf, Purpose::Code] {
            assert_eq!(Signer::leaf(&key, cert, purpose, now()).err(), Some(PkiError::WrongCertificate));
        }
    }
    // A leaf made for the CA's key is refused at issue (M4), so no CA:FALSE certificate for it exists to be offered
    let own = machine.account_key.public().unwrap().spki_der;
    assert_eq!(issue(&Request { profile: "pdf", public_key: &own, now: now(), name: Some("Ada"), ..Request::default() }, &account(machine)).unwrap_err(), PkiError::KeyIsACas);
}

/// M5: a signer is a leaf of the profile the purpose names, within its dates
#[test]
fn m5_a_signer_has_the_purpose_and_the_dates() {
    let machine = machine();
    let account = account(machine);
    // A minute on: the certificates below are issued at the clock of their own moment
    let at = now() + 60;
    for key_type in [KeyType::EcdsaP256, KeyType::Rsa3072] {
        let (pdf_key, pdf) = leaf_of("pdf", &account, key_type);
        let (code_key, code) = leaf_of("code", &account, key_type);
        let (smime_key, smime) = leaf_of("smime", &account, key_type);
        assert!(Signer::leaf(&pdf_key, &pdf.cert, Purpose::Pdf, at).is_ok());
        assert!(Signer::leaf(&code_key, &code.cert, Purpose::Code, at).is_ok());
        // Another profile's certificate is no signer for this purpose
        assert_eq!(Signer::leaf(&pdf_key, &pdf.cert, Purpose::Code, at).err(), Some(PkiError::WrongCertificate));
        assert_eq!(Signer::leaf(&code_key, &code.cert, Purpose::Pdf, at).err(), Some(PkiError::WrongCertificate));
        assert_eq!(Signer::leaf(&smime_key, &smime.cert, Purpose::Pdf, at).err(), Some(PkiError::WrongCertificate));
        assert_eq!(Signer::leaf(&smime_key, &smime.cert, Purpose::Code, at).err(), Some(PkiError::WrongCertificate));
        // The certificate has to be the key's own
        assert_eq!(Signer::leaf(&code_key, &pdf.cert, Purpose::Pdf, at).err(), Some(PkiError::IssuerKeyMismatch));
        // Its dates, at the time of signing: before it starts, after it ends, and a time that is none
        assert_eq!(Signer::leaf(&pdf_key, &pdf.cert, Purpose::Pdf, at - 3600).err(), Some(PkiError::CertificateNotValid));
        assert_eq!(Signer::leaf(&pdf_key, &pdf.cert, Purpose::Pdf, at + 461 * DAY).err(), Some(PkiError::CertificateNotValid));
        assert_eq!(Signer::leaf(&pdf_key, &pdf.cert, Purpose::Pdf, i64::MAX / 2).err(), Some(PkiError::BadTime));
        // A signer made in time is checked again when it signs, and a code signer does not sign a PDF
        let signer = Signer::leaf(&pdf_key, &pdf.cert, Purpose::Pdf, at).unwrap();
        let digest = [3u8; 32];
        assert!(sign_pdf_digest(&signer, &pdf.chain, Digest::Sha256, &digest, at).is_ok());
        assert_eq!(sign_pdf_digest(&signer, &pdf.chain, Digest::Sha256, &digest, at + 461 * DAY).unwrap_err(), PkiError::CertificateNotValid);
        assert_eq!(sign_pdf_digest(&signer, &pdf.chain, Digest::Sha256, &digest, i64::MAX / 2).unwrap_err(), PkiError::BadTime);
        assert_eq!(sign_pdf_digest(&signer, &pdf.chain, Digest::Sha256, &digest, i64::MIN / 2).unwrap_err(), PkiError::BadTime);
        let coder = Signer::leaf(&code_key, &code.cert, Purpose::Code, at).unwrap();
        assert_eq!(sign_pdf_digest(&coder, &code.chain, Digest::Sha256, &digest, at).unwrap_err(), PkiError::WrongCertificate);
        assert!(sign_digest(&coder, Digest::Sha256, &digest).is_ok());
    }
    // Local TLS and the machine's own certificates sign nothing for a person
    let local = Issuer { authority: Authority::LocalServicesCa, cert: &machine.local.cert, key: &machine.local_key, chain: &machine.local.chain };
    let (tls_key, tls) = leaf_of("local-tls", &local, KeyType::EcdsaP256);
    for purpose in [Purpose::Pdf, Purpose::Code] {
        assert_eq!(Signer::leaf(&tls_key, &tls.cert, purpose, at).err(), Some(PkiError::WrongCertificate));
    }
    // An SSH key signs when it is marked for SSH, and for nothing else
    let ssh = generate(KeyType::EcdsaP256).unwrap();
    assert!(sign_digest(&Signer::ssh(&ssh, KeyKind::Ssh).unwrap(), Digest::Sha256, &[1u8; 32]).is_ok());
    for kind in [KeyKind::Signing, KeyKind::Exchange, KeyKind::Symmetric] {
        assert_eq!(Signer::ssh(&ssh, kind).err(), Some(PkiError::KeyNotAllowed));
    }
    assert_eq!(sign_pdf_digest(&Signer::ssh(&ssh, KeyKind::Ssh).unwrap(), &[], Digest::Sha256, &[1u8; 32], at).unwrap_err(), PkiError::WrongCertificate);
}
