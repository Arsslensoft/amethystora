//! Certificates against verifiers that are not this crate, as separate programs: `openssl`, NSS's `certutil` and `pk12util`,
//! `osslsigncode`, `pdfsig`, `curl` (plans/PLAN-vault.md, Tests). Each machine's root takes seconds to make, so a test binary
//! makes two and shares them. See `common` for what happens when a program is not installed.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use amethystora_vault_core::keys::{generate, KeyKind, KeyType, PrivateKey};
use amethystora_vault_core::pki::*;
use common::{attempt, need, output, run, Scratch};

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

fn key(key_type: KeyType) -> PrivateKey {
    generate(key_type).unwrap()
}

/// A CA's key as an ordinary one, for the tests that make a certificate as if the service's checks were not there
fn plain(key: &CaKey) -> PrivateKey {
    let stored = key.to_stored();
    let der = &stored[b"amethystora-ca-key\0".len()..];
    PrivateKey::from_bytes(KeyType::Rsa3072, der).or_else(|_| PrivateKey::from_bytes(KeyType::Rsa4096, der)).unwrap()
}

/// A machine: its root, the account's CA and the machine's two
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

impl Machine {
    fn new() -> Machine {
        let at = now();
        let (root_key, root) = rotate_root(at).unwrap();
        let root_issuer = Issuer { authority: Authority::Root, cert: &root, key: &root_key, chain: &[] };
        let make = |profile: &str, name: Option<&str>| {
            let ca_key = CaKey::generate(Profile::parse(profile).unwrap().makes().unwrap()).unwrap();
            let spki = ca_key.public().unwrap().spki_der;
            let issued = issue(&Request { profile, public_key: &spki, now: at, name, ..Request::default() }, &root_issuer).unwrap();
            (ca_key, issued)
        };
        let (account_key, account) = make("account-ca", Some("Ada Lovelace"));
        let (device_key, device) = make("device-ca", None);
        let (local_key, local) = make("local-services-ca", None);
        Machine { root_key, root, account_key, account, device_key, device, local_key, local }
    }

    fn issuer(&self, authority: Authority) -> Issuer<'_> {
        match authority {
            Authority::Root => Issuer { authority, cert: &self.root, key: &self.root_key, chain: &[] },
            Authority::AccountCa => Issuer { authority, cert: &self.account.cert, key: &self.account_key, chain: &self.account.chain },
            Authority::DeviceCa => Issuer { authority, cert: &self.device.cert, key: &self.device_key, chain: &self.device.chain },
            Authority::LocalServicesCa => Issuer { authority, cert: &self.local.cert, key: &self.local_key, chain: &self.local.chain },
        }
    }
}

fn machine() -> &'static Machine {
    static MACHINE: OnceLock<Machine> = OnceLock::new();
    MACHINE.get_or_init(Machine::new)
}

fn other_machine() -> &'static Machine {
    static MACHINE: OnceLock<Machine> = OnceLock::new();
    MACHINE.get_or_init(Machine::new)
}

/// A leaf and its key, from the machine's own CA for that profile
struct Leaf {
    key: PrivateKey,
    issued: Issued,
}

fn authority_of(profile: &str) -> Authority {
    match profile {
        "pdf" | "code" | "smime" => Authority::AccountCa,
        "device-identity" | "service-login" => Authority::DeviceCa,
        "local-tls" => Authority::LocalServicesCa,
        _ => Authority::Root,
    }
}

fn leaf_on(machine: &Machine, key_type: KeyType, request: Request<'_>) -> Leaf {
    let leaf_key = key(key_type);
    let spki = leaf_key.public().unwrap().spki_der;
    let issuer = machine.issuer(authority_of(request.profile));
    let issued = issue(&Request { public_key: &spki, now: now(), ..request }, &issuer).unwrap();
    Leaf { key: leaf_key, issued }
}

fn leaf(key_type: KeyType, request: Request<'_>) -> Leaf {
    leaf_on(machine(), key_type, request)
}

fn write_pem(scratch: &Scratch, name: &str, cert: &[u8]) -> PathBuf {
    scratch.write(name, pem_chain(cert, &[]).unwrap().as_bytes())
}

/// `openssl x509 -text`, names least significant first as the plan writes them
fn text_of(cert: &[u8]) -> String {
    let scratch = Scratch::new("text");
    let path = write_pem(&scratch, "cert.pem", cert);
    output(Command::new("openssl").args(["x509", "-noout", "-text", "-nameopt", "RFC2253,-esc_msb,utf8", "-in"]).arg(&path))
}

fn has_extension_critical(text: &str, name: &str, critical: bool) {
    let needle = if critical { format!("{name}: critical") } else { format!("{name}:") };
    let line = text.lines().find(|line| line.trim_start().starts_with(name)).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    assert_eq!(line.trim(), needle, "{text}");
}

fn line_after<'a>(text: &'a str, name: &str) -> &'a str {
    let mut lines = text.lines();
    lines.find(|line| line.trim_start().starts_with(name)).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    lines.next().unwrap().trim()
}

/// A SERIAL of 128 bits, positive, and SHA-256 unless asked otherwise
fn common_fields(text: &str, signature: &str) {
    assert!(text.contains("Version: 3 (0x2)"), "{text}");
    assert!(text.contains(&format!("Signature Algorithm: {signature}")), "{text}");
    let serial = line_after(text, "Serial Number:");
    let octets = serial.split(':').count();
    assert!((15..=17).contains(&octets), "{serial}");
    assert!(!text.contains("Serial Number: (Negative)"), "{text}");
    has_extension_critical(text, "X509v3 Subject Key Identifier", false);
}

fn days_of(cert: &[u8]) -> i64 {
    let description = describe(cert).unwrap();
    (description.not_after - description.not_before) / 86400
}

#[test]
fn the_roots_fields() {
    if !need("openssl") {
        return;
    }
    let machine = machine();
    let number = machine_number(&machine.root_key.public().unwrap().spki_der);
    assert_eq!(number.len(), 8);
    assert!(number.bytes().all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte)));
    let text = text_of(&machine.root);
    common_fields(&text, "sha256WithRSAEncryption");
    let subject = format!("CN=Amethystora OS Root CA,OU=Machine {number}");
    assert!(text.contains(&format!("Subject: {subject}")), "{text}");
    assert!(text.contains(&format!("Issuer: {subject}")), "{text}");
    assert!(text.contains("Public-Key: (4096 bit)"), "{text}");
    assert_eq!(days_of(&machine.root), 7305);
    has_extension_critical(&text, "X509v3 Basic Constraints", true);
    assert_eq!(line_after(&text, "X509v3 Basic Constraints"), "CA:TRUE, pathlen:1");
    has_extension_critical(&text, "X509v3 Key Usage", true);
    assert_eq!(line_after(&text, "X509v3 Key Usage"), "Certificate Sign, CRL Sign");
    assert!(!text.contains("Extended Key Usage"), "{text}");
    assert!(!text.contains("Authority Key Identifier"), "{text}");
    has_extension_critical(&text, "X509v3 Name Constraints", true);
    assert_eq!(subtrees(&text, "Permitted:"), PERMITTED, "{text}");
    assert!(!text.contains("Excluded:"), "{text}");
    assert!(text.contains("Signature Algorithm: sha256WithRSAEncryption"));

    // The number is the start of the SHA-256 of what openssl itself takes for the public key
    let scratch = Scratch::new("number");
    let root_pem = write_pem(&scratch, "root.pem", &machine.root);
    let spki_pem = scratch.path("spki.pem");
    run(Command::new("openssl").args(["x509", "-noout", "-pubkey", "-in"]).arg(&root_pem).arg("-out").arg(&spki_pem));
    let der = scratch.path("spki.der");
    run(Command::new("openssl").args(["pkey", "-pubin", "-outform", "DER", "-in"]).arg(&spki_pem).arg("-out").arg(&der));
    let sum = output(Command::new("sha256sum").arg(&der));
    assert_eq!(sum[..8].to_uppercase(), number);
    assert_eq!(machine_number(&std::fs::read(&der).unwrap()), number);
}

const PERMITTED: [&str; 10] = [
    "DNS:local",
    "DNS:home.arpa",
    "DNS:internal",
    "DNS:localhost",
    "IP:10.0.0.0/255.0.0.0",
    "IP:172.16.0.0/255.240.0.0",
    "IP:192.168.0.0/255.255.0.0",
    "IP:127.0.0.0/255.0.0.0",
    "IP:FD00:0:0:0:0:0:0:0/FF00:0:0:0:0:0:0:0",
    "IP:0:0:0:0:0:0:0:1/FFFF:FFFF:FFFF:FFFF:FFFF:FFFF:FFFF:FFFF",
];
const EXCLUDED: [&str; 3] = ["DNS:", "IP:0.0.0.0/0.0.0.0", "IP:0:0:0:0:0:0:0:0/0:0:0:0:0:0:0:0"];

/// The subtrees `openssl x509 -text` prints under a Name Constraints heading
fn subtrees<'a>(text: &'a str, heading: &str) -> Vec<&'a str> {
    text.lines()
        .skip_while(|line| !line.trim_start().starts_with(heading))
        .skip(1)
        .take_while(|line| line.trim_start().starts_with("DNS:") || line.trim_start().starts_with("IP:"))
        .map(str::trim)
        .collect()
}

/// A certificate's table row, as `openssl x509 -text` prints it. Of a key's size only the start: OpenSSL 4 goes on to say
/// "(256 bit field, 128 bit security level)" where 3.5 says "(256 bit)".
struct Row {
    subject: String,
    issuer: String,
    bits: &'static str,
    days: i64,
    ca: bool,
    usage: &'static str,
    purposes: &'static str,
    alternative: Option<&'static str>,
}

fn check_row(cert: &[u8], row: Row) {
    let text = text_of(cert);
    common_fields(&text, "sha256WithRSAEncryption");
    assert!(text.contains(&format!("Subject: {}\n", row.subject)), "{text}");
    assert!(text.contains(&format!("Issuer: {}\n", row.issuer)), "{text}");
    assert!(text.contains(row.bits), "{text}");
    assert_eq!(days_of(cert), row.days, "{text}");
    has_extension_critical(&text, "X509v3 Basic Constraints", true);
    assert_eq!(line_after(&text, "X509v3 Basic Constraints"), if row.ca { "CA:TRUE, pathlen:0" } else { "CA:FALSE" }, "{text}");
    has_extension_critical(&text, "X509v3 Key Usage", true);
    assert_eq!(line_after(&text, "X509v3 Key Usage"), row.usage, "{text}");
    has_extension_critical(&text, "X509v3 Extended Key Usage", false);
    assert_eq!(line_after(&text, "X509v3 Extended Key Usage"), row.purposes, "{text}");
    has_extension_critical(&text, "X509v3 Authority Key Identifier", false);
    assert_eq!(text.contains("X509v3 Name Constraints"), row.ca, "{text}");
    if row.ca {
        has_extension_critical(&text, "X509v3 Name Constraints", true);
    }
    match row.alternative {
        Some(names) => {
            has_extension_critical(&text, "X509v3 Subject Alternative Name", false);
            assert_eq!(line_after(&text, "X509v3 Subject Alternative Name"), names, "{text}");
        }
        None => assert!(!text.contains("Subject Alternative Name"), "{text}"),
    }
}

#[test]
fn every_profiles_fields() {
    if !need("openssl") {
        return;
    }
    let machine = machine();
    let number = machine_number(&machine.root_key.public().unwrap().spki_der);
    let root = format!("CN=Amethystora OS Root CA,OU=Machine {number}");
    let account = "CN=Ada Lovelace Code Signing CA,OU=Amethystora OS";
    let device = format!("CN=Device Identity CA,OU=Machine {number}");
    let local = format!("CN=Local Services CA,OU=Machine {number}");
    const SIGN: &str = "Digital Signature";
    const SIGN_ENCIPHER: &str = "Digital Signature, Key Encipherment";
    const CA: &str = "Certificate Sign, CRL Sign";
    const ACCOUNT_PURPOSES: &str = "Code Signing, E-mail Protection, 1.3.6.1.4.1.311.10.3.12";
    const PDF_PURPOSES: &str = "E-mail Protection, 1.3.6.1.4.1.311.10.3.12";

    check_row(&machine.account.cert, Row { subject: account.into(), issuer: root.clone(), bits: "(3072 bit", days: 1826, ca: true, usage: CA, purposes: ACCOUNT_PURPOSES, alternative: None });
    check_row(&machine.device.cert, Row { subject: device.clone(), issuer: root.clone(), bits: "(3072 bit", days: 1826, ca: true, usage: CA, purposes: "TLS Web Client Authentication, TLS Web Server Authentication", alternative: None });
    check_row(&machine.local.cert, Row { subject: local.clone(), issuer: root.clone(), bits: "(3072 bit", days: 1826, ca: true, usage: CA, purposes: "TLS Web Server Authentication", alternative: None });

    // Each CA's own limits, which are all that binds it when a client trusts it alone
    let account_text = text_of(&machine.account.cert);
    assert_eq!(subtrees(&account_text, "Excluded:"), EXCLUDED, "{account_text}");
    assert!(!account_text.contains("Permitted:"), "{account_text}");
    let device_text = text_of(&machine.device.cert);
    assert_eq!(subtrees(&device_text, "Excluded:"), EXCLUDED, "{device_text}");
    assert!(!device_text.contains("Permitted:"), "{device_text}");
    let local_text = text_of(&machine.local.cert);
    assert_eq!(subtrees(&local_text, "Permitted:"), PERMITTED, "{local_text}");
    assert!(!local_text.contains("Excluded:"), "{local_text}");

    let pdf = leaf(KeyType::Rsa3072, Request { profile: "pdf", name: Some("Ada Lovelace"), ..Request::default() });
    check_row(&pdf.issued.cert, Row { subject: "CN=Ada Lovelace,OU=Amethystora Vault".into(), issuer: account.into(), bits: "(3072 bit", days: 460, ca: false, usage: SIGN, purposes: PDF_PURPOSES, alternative: None });
    let pdf_full = leaf(KeyType::Rsa3072, Request { profile: "pdf", name: Some("Ada Lovelace"), organization: Some("Analytical Engines"), country: Some("GB"), email: Some("ada@example.org"), ..Request::default() });
    check_row(&pdf_full.issued.cert, Row { subject: "CN=Ada Lovelace,OU=Amethystora Vault,O=Analytical Engines,C=GB".into(), issuer: account.into(), bits: "(3072 bit", days: 460, ca: false, usage: SIGN, purposes: PDF_PURPOSES, alternative: Some("email:ada@example.org") });

    let code = leaf(KeyType::Rsa3072, Request { profile: "code", name: Some("Ada Lovelace"), web: Some("https://example.org/ada"), ..Request::default() });
    check_row(&code.issued.cert, Row { subject: "CN=Ada Lovelace,OU=Amethystora Vault".into(), issuer: account.into(), bits: "(3072 bit", days: 460, ca: false, usage: SIGN, purposes: "Code Signing", alternative: Some("URI:https://example.org/ada") });

    let smime = leaf(KeyType::Rsa3072, Request { profile: "smime", name: Some("Ada Lovelace"), email: Some("ada@example.org"), ..Request::default() });
    check_row(&smime.issued.cert, Row { subject: "CN=Ada Lovelace,OU=Amethystora Vault".into(), issuer: account.into(), bits: "(3072 bit", days: 460, ca: false, usage: SIGN_ENCIPHER, purposes: "E-mail Protection", alternative: Some("email:ada@example.org") });
    let smime_ec = leaf(KeyType::EcdsaP256, Request { profile: "smime", name: Some("Ada Lovelace"), email: Some("ada@example.org"), ..Request::default() });
    check_row(&smime_ec.issued.cert, Row { subject: "CN=Ada Lovelace,OU=Amethystora Vault".into(), issuer: account.into(), bits: "(256 bit", days: 460, ca: false, usage: SIGN, purposes: "E-mail Protection", alternative: Some("email:ada@example.org") });

    let names = ["printer.local", "NAS.home.arpa", "192.168.1.5", "localhost", "127.0.0.1", "fd00::5", "::1"];
    let tls = leaf(KeyType::EcdsaP256, Request { profile: "local-tls", local_names: &names, ..Request::default() });
    check_row(&tls.issued.cert, Row { subject: "CN=Local service,OU=Amethystora OS".into(), issuer: local.clone(), bits: "(256 bit", days: 397, ca: false, usage: SIGN, purposes: "TLS Web Server Authentication", alternative: Some("DNS:printer.local, DNS:nas.home.arpa, IP Address:192.168.1.5, DNS:localhost, IP Address:127.0.0.1, IP Address:FD00:0:0:0:0:0:0:5, IP Address:0:0:0:0:0:0:0:1") });
    let tls_rsa = leaf(KeyType::Rsa3072, Request { profile: "local-tls", local_names: &["printer.local"], ..Request::default() });
    check_row(&tls_rsa.issued.cert, Row { subject: "CN=Local service,OU=Amethystora OS".into(), issuer: local, bits: "(3072 bit", days: 397, ca: false, usage: SIGN_ENCIPHER, purposes: "TLS Web Server Authentication", alternative: Some("DNS:printer.local") });

    let identity = leaf(KeyType::EcdsaP256, Request { profile: "device-identity", ..Request::default() });
    check_row(&identity.issued.cert, Row { subject: format!("CN=Machine {number},OU=Amethystora OS"), issuer: device.clone(), bits: "(256 bit", days: 397, ca: false, usage: SIGN, purposes: "TLS Web Client Authentication, TLS Web Server Authentication", alternative: None });
    let login = leaf(KeyType::EcdsaP256, Request { profile: "service-login", name: Some("Home VPN"), login: Some("ada"), ..Request::default() });
    check_row(&login.issued.cert, Row { subject: format!("CN=Home VPN,OU=ada,OU=Machine {number}"), issuer: device, bits: "(256 bit", days: 397, ca: false, usage: SIGN, purposes: "TLS Web Client Authentication", alternative: None });

    // SHA-384 where it is offered, and each key the table takes
    let sha384 = leaf(KeyType::EcdsaP384, Request { profile: "code", name: Some("Ada Lovelace"), digest: Some(Digest::Sha384), ..Request::default() });
    let text = text_of(&sha384.issued.cert);
    assert!(text.contains("Signature Algorithm: sha384WithRSAEncryption") && text.contains("(384 bit"), "{text}");
    let rsa4096 = leaf(KeyType::Rsa4096, Request { profile: "pdf", name: Some("Ada Lovelace"), ..Request::default() });
    assert!(text_of(&rsa4096.issued.cert).contains("(4096 bit"));

    // The keys a certificate's Authority Key Identifier names are its issuer's Subject Key Identifier
    let leaf_text = text_of(&pdf.issued.cert);
    let ca_text = text_of(&machine.account.cert);
    assert_eq!(line_after(&leaf_text, "X509v3 Authority Key Identifier"), line_after(&ca_text, "X509v3 Subject Key Identifier"));
}

#[test]
fn names_outside_ascii_are_utf8_strings() {
    if !need("openssl") {
        return;
    }
    for name in ["José Núñez", "أروى إدريس", "山田 太郎", "Zoë Müller"] {
        let signed = leaf(KeyType::EcdsaP256, Request { profile: "pdf", name: Some(name), organization: Some("Société Générale"), ..Request::default() });
        let scratch = Scratch::new("utf8");
        let path = scratch.write("cert.der", &signed.issued.cert);
        let structure = output(Command::new("openssl").args(["asn1parse", "-inform", "DER", "-in"]).arg(&path));
        for line in structure.lines().filter(|line| line.contains("UTF8STRING")) {
            assert!(line.contains(":"), "{line}");
        }
        assert!(!structure.contains("T61STRING") && !structure.contains("BMPSTRING"), "{structure}");
        let text = text_of(&signed.issued.cert);
        assert!(text.contains(name), "{name}\n{text}");
        assert!(text.contains("Société Générale"), "{text}");
        assert_eq!(describe(&signed.issued.cert).unwrap().subject, format!("CN={name}, OU=Amethystora Vault, O=Société Générale"));
    }
    // A person's CA too
    let machine = machine();
    let ca_key = CaKey::generate(Authority::AccountCa).unwrap();
    let spki = ca_key.public().unwrap().spki_der;
    let ca = issue(&Request { profile: "account-ca", public_key: &spki, now: now(), name: Some("أروى"), ..Request::default() }, &machine.issuer(Authority::Root)).unwrap();
    assert!(text_of(&ca.cert).contains("CN=أروى Code Signing CA"));
}

#[test]
fn refused_before_anything_is_signed() {
    let machine = machine();
    let spki = key(KeyType::EcdsaP256).public().unwrap().spki_der;
    let at = now();
    let root = machine.issuer(Authority::Root);
    let account = machine.issuer(Authority::AccountCa);
    let local = machine.issuer(Authority::LocalServicesCa);
    let device = machine.issuer(Authority::DeviceCa);
    let ask = |request: Request<'_>, issuer: &Issuer<'_>| issue(&Request { public_key: &spki, now: at, ..request }, issuer).map(|_| ()).unwrap_err();

    // A profile the table does not have, whatever it is called
    for profile in ["", "root", "ca", "PDF", "pdf ", "anything", "../pdf"] {
        assert_eq!(ask(Request { profile, name: Some("Ada"), ..Request::default() }, &account), PkiError::UnknownProfile, "{profile}");
    }
    // Asked of the wrong CA
    assert_eq!(ask(Request { profile: "pdf", name: Some("Ada"), ..Request::default() }, &root), PkiError::WrongIssuer);
    assert_eq!(ask(Request { profile: "local-tls", local_names: &["a.local"], ..Request::default() }, &account), PkiError::WrongIssuer);
    assert_eq!(ask(Request { profile: "account-ca", name: Some("Ada"), ..Request::default() }, &account), PkiError::WrongIssuer);

    // A public name for local TLS, in every shape, and what is not a name at all
    for name in ["www.google.com", "8.8.8.8", "local.evil.example", "evillocal", "2001:db8::1", "172.32.0.1", "fe00::1", "::ffff:10.0.0.1", "1.1.1.1"] {
        assert_eq!(ask(Request { profile: "local-tls", local_names: &[name], ..Request::default() }, &local), PkiError::OutsideLimits, "{name}");
    }
    assert_eq!(ask(Request { profile: "local-tls", local_names: &["printer.local", "www.google.com"], ..Request::default() }, &local), PkiError::OutsideLimits);
    for name in ["*.local", "a b.local", "printer.local.", "-x.local", "a..local", "printer.local\n", "", "printer.local,DNS:www.google.com", "ÿ.local"] {
        assert_eq!(ask(Request { profile: "local-tls", local_names: &[name], ..Request::default() }, &local), PkiError::BadField("host name"), "{name}");
    }
    assert_eq!(ask(Request { profile: "local-tls", ..Request::default() }, &local), PkiError::MissingField("host name"));
    assert_eq!(ask(Request { profile: "local-tls", local_names: &["a.local"; 33], ..Request::default() }, &local), PkiError::BadField("host name"));

    // A typed name is checked
    let long = "x".repeat(65);
    for name in ["", " Ada", "Ada ", "Ada\nLovelace", "Ada\0", "Ada\u{7f}", "Ada\u{85}", "A\u{202e}da", "A\u{2066}da", "A\u{feff}da", long.as_str()] {
        assert_eq!(ask(Request { profile: "pdf", name: Some(name), ..Request::default() }, &account), PkiError::BadField("name"), "{name:?}");
    }
    assert_eq!(ask(Request { profile: "pdf", ..Request::default() }, &account), PkiError::MissingField("name"));
    // Non-ASCII counts in characters, and the longest passes
    let longest = "é".repeat(64);
    assert!(issue(&Request { profile: "pdf", public_key: &spki, now: at, name: Some(&longest), ..Request::default() }, &account).is_ok());
    let ca_key = key(KeyType::Rsa3072).public().unwrap().spki_der;
    let person_long = "x".repeat(49);
    assert_eq!(issue(&Request { profile: "account-ca", public_key: &ca_key, now: at, name: Some(&person_long), ..Request::default() }, &root).unwrap_err(), PkiError::BadField("name"));
    // A name that reads as a host, which every verifier would hold to the root's limits
    for name in ["j.smith", "www.google.com", "8.8.8.8", "a-b.c_d"] {
        assert_eq!(ask(Request { profile: "pdf", name: Some(name), ..Request::default() }, &account), PkiError::NameLooksLikeHost, "{name}");
        assert_eq!(ask(Request { profile: "service-login", name: Some(name), login: Some("ada"), ..Request::default() }, &device), PkiError::NameLooksLikeHost, "{name}");
    }
    for email in ["a", "a@", "@b", "a b@c.d", "a@b,DNS:www.google.com", "a,b@c.d", "é@c.d", "a@é.d", "a@b@c"] {
        assert_eq!(ask(Request { profile: "pdf", name: Some("Ada"), email: Some(email), ..Request::default() }, &account), PkiError::BadField("email"), "{email}");
    }
    for web in ["ftp://x", "https://", "https://a b", "javascript:alert(1)", "https://é"] {
        assert_eq!(ask(Request { profile: "code", name: Some("Ada"), web: Some(web), ..Request::default() }, &account), PkiError::BadField("web address"), "{web}");
    }
    for country in ["G", "GBR", "gb", "é1"] {
        assert_eq!(ask(Request { profile: "pdf", name: Some("Ada"), country: Some(country), ..Request::default() }, &account), PkiError::BadField("country"), "{country}");
    }

    // A field the profile has no place for is refused, never dropped
    assert_eq!(ask(Request { profile: "local-tls", name: Some("Ada"), local_names: &["a.local"], ..Request::default() }, &local), PkiError::NotInProfile("name"));
    assert_eq!(ask(Request { profile: "pdf", name: Some("Ada"), web: Some("https://a.example"), ..Request::default() }, &account), PkiError::NotInProfile("web address"));
    assert_eq!(ask(Request { profile: "pdf", name: Some("Ada"), local_names: &["a.local"], ..Request::default() }, &account), PkiError::NotInProfile("host name"));
    assert_eq!(ask(Request { profile: "smime", name: Some("Ada"), email: Some("a@b.c"), organization: Some("O"), ..Request::default() }, &account), PkiError::NotInProfile("organization"));
    assert_eq!(ask(Request { profile: "device-identity", name: Some("Ada"), ..Request::default() }, &device), PkiError::NotInProfile("name"));
    assert_eq!(ask(Request { profile: "device-identity", email: Some("a@b.c"), ..Request::default() }, &device), PkiError::NotInProfile("email"));
    assert_eq!(ask(Request { profile: "smime", name: Some("Ada"), ..Request::default() }, &account), PkiError::MissingField("email"));
    assert_eq!(ask(Request { profile: "service-login", name: Some("VPN"), ..Request::default() }, &device), PkiError::MissingField("login"));

    // A key the table does not take for the profile
    for key_type in [KeyType::Ed25519, KeyType::MlDsa65, KeyType::X25519] {
        if !key_type.available() {
            continue;
        }
        let public = key(key_type).public().unwrap().spki_der;
        let refused = issue(&Request { profile: "pdf", public_key: &public, now: at, name: Some("Ada"), ..Request::default() }, &account);
        assert_eq!(refused.unwrap_err(), PkiError::KeyNotAllowed, "{key_type:?}");
    }
    let refused = issue(&Request { profile: "account-ca", public_key: &spki, now: at, name: Some("Ada"), ..Request::default() }, &root);
    assert_eq!(refused.unwrap_err(), PkiError::KeyNotAllowed);
    assert_eq!(make_root(&CaKey::generate(Authority::AccountCa).unwrap(), at).unwrap_err(), PkiError::KeyNotAllowed);
    let refused = issue(&Request { profile: "account-ca", public_key: &ca_key, now: at, name: Some("Ada"), digest: Some(Digest::Sha384), ..Request::default() }, &root);
    assert_eq!(refused.unwrap_err(), PkiError::DigestNotAllowed);

    // Another CA's key under this CA's certificate
    let mismatched = Issuer { authority: Authority::AccountCa, cert: &machine.account.cert, key: &machine.device_key, chain: &machine.account.chain };
    assert_eq!(ask(Request { profile: "pdf", name: Some("Ada"), ..Request::default() }, &mismatched), PkiError::IssuerKeyMismatch);
    // A CA past its dates, or before them
    let late = issue(&Request { profile: "pdf", public_key: &spki, now: at + 3000 * 86400, name: Some("Ada"), ..Request::default() }, &account);
    assert_eq!(late.unwrap_err(), PkiError::IssuerNotValid);
    let early = issue(&Request { profile: "pdf", public_key: &spki, now: at - 86400, name: Some("Ada"), ..Request::default() }, &account);
    assert_eq!(early.unwrap_err(), PkiError::IssuerNotValid);
    // A public key that is not one
    let refused = issue(&Request { profile: "pdf", public_key: b"not a key", now: at, name: Some("Ada"), ..Request::default() }, &account);
    assert_eq!(refused.unwrap_err(), PkiError::Key(amethystora_vault_core::keys::KeyError::Malformed));
}

#[test]
fn validity_never_passes_the_issuers() {
    let machine = machine();
    let root_ends = describe(&machine.root).unwrap().not_after;
    // Near the end of the root's life, a CA stops with it, and a leaf with the CA. A CA is not made with less life left than
    // its leaves have, so this one has the least, and its leaves outlast it a month in.
    let at = root_ends - 461 * 86400;
    let ca_key = CaKey::generate(Authority::AccountCa).unwrap();
    let ca_spki = ca_key.public().unwrap().spki_der;
    let ca = issue(&Request { profile: "account-ca", public_key: &ca_spki, now: at, name: Some("Ada"), ..Request::default() }, &machine.issuer(Authority::Root)).unwrap();
    assert_eq!(describe(&ca.cert).unwrap().not_after, root_ends);
    assert!(ca_is_current(&ca.cert, &machine.root, at).unwrap());
    assert!(!ca_is_current(&ca.cert, &machine.root, root_ends).unwrap());
    let issuer = Issuer { authority: Authority::AccountCa, cert: &ca.cert, key: &ca_key, chain: &ca.chain };
    let spki = key(KeyType::EcdsaP256).public().unwrap().spki_der;
    let signed = issue(&Request { profile: "pdf", public_key: &spki, now: at + 30 * 86400, name: Some("Ada"), ..Request::default() }, &issuer).unwrap();
    let description = describe(&signed.cert).unwrap();
    assert_eq!(description.not_after, root_ends);
    assert_eq!(description.not_before, at + 30 * 86400);
    // And at the root's own end nothing more is issued
    let refused = issue(&Request { profile: "pdf", public_key: &spki, now: root_ends, name: Some("Ada"), ..Request::default() }, &issuer);
    assert_eq!(refused.unwrap_err(), PkiError::IssuerNotValid);
}

#[test]
fn rotation_makes_a_new_root_and_old_cas_stop_issuing() {
    if !need("openssl") {
        return;
    }
    let machine = machine();
    let at = now();
    assert!(ca_is_current(&machine.account.cert, &machine.root, at).unwrap());
    let (new_key, new_root) = rotate_root(at).unwrap();
    assert!(!ca_is_current(&machine.account.cert, &new_root, at).unwrap());
    assert_ne!(machine_number(&new_key.public().unwrap().spki_der), machine_number(&machine.root_key.public().unwrap().spki_der));
    // What the old root issued still verifies for whoever trusted it, and not for whoever trusts the new one
    let scratch = Scratch::new("rotate");
    let ca = write_pem(&scratch, "ca.pem", &machine.account.cert);
    let old = write_pem(&scratch, "old.pem", &machine.root);
    let new = write_pem(&scratch, "new.pem", &new_root);
    assert!(attempt(Command::new("openssl").args(["verify", "-x509_strict", "-CAfile"]).arg(&old).arg(&ca)).0);
    assert!(!attempt(Command::new("openssl").args(["verify", "-x509_strict", "-CAfile"]).arg(&new).arg(&ca)).0);
}

fn openssl_verify(root: &Path, chain: &[Vec<u8>], cert: &[u8], scratch: &Scratch, options: &[&str]) -> (bool, String) {
    let leaf_path = write_pem(scratch, "leaf.pem", cert);
    let untrusted = scratch.write("untrusted.pem", pem_chain(&chain[0], &chain[1..chain.len() - 1]).unwrap().as_bytes());
    attempt(Command::new("openssl").args(["verify", "-x509_strict"]).args(options).arg("-CAfile").arg(root).arg("-untrusted").arg(&untrusted).arg(&leaf_path))
}

#[test]
fn openssl_verifies_each_by_its_purpose() {
    if !need("openssl") {
        return;
    }
    let machine = machine();
    let scratch = Scratch::new("verify");
    let root = write_pem(&scratch, "root.pem", &machine.root);
    let other_root = write_pem(&scratch, "other.pem", &other_machine().root);

    let pdf = leaf(KeyType::Rsa3072, Request { profile: "pdf", name: Some("Ada Lovelace"), ..Request::default() });
    let code = leaf(KeyType::Rsa3072, Request { profile: "code", name: Some("Ada Lovelace"), ..Request::default() });
    let smime = leaf(KeyType::Rsa3072, Request { profile: "smime", name: Some("Ada Lovelace"), email: Some("ada@example.org"), ..Request::default() });
    let tls = leaf(KeyType::EcdsaP256, Request { profile: "local-tls", local_names: &["printer.local", "192.168.1.5"], ..Request::default() });
    let identity = leaf(KeyType::EcdsaP256, Request { profile: "device-identity", ..Request::default() });
    let login = leaf(KeyType::EcdsaP256, Request { profile: "service-login", name: Some("Home VPN"), login: Some("ada"), ..Request::default() });
    let cases: [(&str, &Leaf, &[&str], &[&str]); 6] = [
        ("pdf", &pdf, &["smimesign"], &["codesign", "sslserver", "sslclient"]),
        ("code", &code, &["codesign"], &["smimesign", "sslserver", "sslclient"]),
        ("smime", &smime, &["smimesign", "smimeencrypt"], &["codesign", "sslserver", "sslclient"]),
        ("local-tls", &tls, &["sslserver"], &["sslclient", "smimesign", "codesign"]),
        ("device-identity", &identity, &["sslclient", "sslserver"], &["smimesign", "codesign"]),
        ("service-login", &login, &["sslclient"], &["sslserver", "smimesign", "codesign"]),
    ];
    for (name, signed, right, wrong) in cases {
        for purpose in right {
            let (ok, said) = openssl_verify(&root, &signed.issued.chain, &signed.issued.cert, &scratch, &["-purpose", purpose]);
            assert!(ok, "{name} as {purpose}: {said}");
        }
        for purpose in wrong {
            let (ok, said) = openssl_verify(&root, &signed.issued.chain, &signed.issued.cert, &scratch, &["-purpose", purpose]);
            assert!(!ok, "{name} as {purpose} should fail: {said}");
        }
        // Another machine's root is refused for every one of them
        let (ok, said) = openssl_verify(&other_root, &signed.issued.chain, &signed.issued.cert, &scratch, &[]);
        assert!(!ok, "{name} under another root: {said}");
    }
    // The hostname and address are what the certificate says
    let (ok, said) = openssl_verify(&root, &tls.issued.chain, &tls.issued.cert, &scratch, &["-purpose", "sslserver", "-verify_hostname", "printer.local"]);
    assert!(ok, "{said}");
    let (ok, said) = openssl_verify(&root, &tls.issued.chain, &tls.issued.cert, &scratch, &["-purpose", "sslserver", "-verify_ip", "192.168.1.5"]);
    assert!(ok, "{said}");
    let (ok, _) = openssl_verify(&root, &tls.issued.chain, &tls.issued.cert, &scratch, &["-purpose", "sslserver", "-verify_hostname", "other.local"]);
    assert!(!ok);
    // A name with a single word, the common case, is fine under the root's limits
    let plain = leaf(KeyType::EcdsaP256, Request { profile: "pdf", name: Some("Ada"), ..Request::default() });
    let (ok, said) = openssl_verify(&root, &plain.issued.chain, &plain.issued.cert, &scratch, &["-purpose", "smimesign"]);
    assert!(ok, "{said}");
}

/// A certificate made as if the service's own check were not there, by `openssl` with a CA's key
fn forge(ca: &[u8], ca_key: &CaKey, scratch: &Scratch, serial: u32, subject: &str, san: Option<&str>) -> PathBuf {
    let ca_cert = write_pem(scratch, "ca.pem", ca);
    let ca_key = scratch.write("ca.key", &plain(ca_key).export_plain().unwrap());
    let csr = scratch.path("forged.csr");
    let key_path = scratch.path("forged.key");
    run(Command::new("openssl").args(["req", "-new", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256", "-nodes", "-subj", subject, "-keyout"]).arg(&key_path).arg("-out").arg(&csr));
    let mut extensions = "basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\nsubjectKeyIdentifier=hash\nauthorityKeyIdentifier=keyid\n".to_string();
    if let Some(san) = san {
        extensions.push_str(&format!("subjectAltName={san}\n"));
    }
    let extensions = scratch.write("forged.ext", extensions.as_bytes());
    let out = scratch.path("forged.pem");
    run(Command::new("openssl")
        .args(["x509", "-req", "-days", "30", "-sha256", "-set_serial", &serial.to_string(), "-in"])
        .arg(&csr)
        .arg("-CA")
        .arg(&ca_cert)
        .arg("-CAkey")
        .arg(&ca_key)
        .arg("-extfile")
        .arg(&extensions)
        .arg("-out")
        .arg(&out));
    out
}

/// An NSS database, certificates added by nickname with trust flags, as a person's would be
struct Nss {
    dir: String,
}

impl Nss {
    fn new(scratch: &Scratch, name: &str) -> Nss {
        let path = scratch.path(name);
        std::fs::create_dir_all(&path).unwrap();
        let dir = format!("sql:{}", path.display());
        run(Command::new("certutil").args(["-N", "--empty-password", "-d", &dir]));
        Nss { dir }
    }

    fn add(&self, nickname: &str, trust: &str, file: &Path) {
        run(Command::new("certutil").args(["-A", "-d", &self.dir, "-n", nickname, "-t", trust, "-a", "-i"]).arg(file));
    }

    fn add_der(&self, scratch: &Scratch, nickname: &str, trust: &str, cert: &[u8]) {
        self.add(nickname, trust, &write_pem(scratch, &format!("{nickname}.pem"), cert));
    }

    /// `certutil -V`, which says whether the certificate is valid for the usage letter
    fn verify(&self, nickname: &str, usage: &str) -> (bool, String) {
        attempt(Command::new("certutil").args(["-V", "-d", &self.dir, "-n", nickname, "-u", usage]))
    }
}

#[test]
fn nss_verifies_each_by_its_purpose() {
    if !need("certutil") {
        return;
    }
    let machine = machine();
    let scratch = Scratch::new("nss");
    let db = Nss::new(&scratch, "db");
    // T is what NSS asks of a CA for client certificates
    db.add_der(&scratch, "root", "CT,C,C", &machine.root);
    db.add_der(&scratch, "account", ",,", &machine.account.cert);
    db.add_der(&scratch, "device", ",,", &machine.device.cert);
    db.add_der(&scratch, "localca", ",,", &machine.local.cert);

    let cases: [(&str, KeyType, Request<'_>, &str); 7] = [
        ("pdf", KeyType::Rsa3072, Request { profile: "pdf", name: Some("Ada Lovelace"), ..Request::default() }, "S"),
        ("code", KeyType::Rsa3072, Request { profile: "code", name: Some("Ada Lovelace"), ..Request::default() }, "J"),
        ("smime", KeyType::Rsa3072, Request { profile: "smime", name: Some("Ada Lovelace"), email: Some("ada@example.org"), ..Request::default() }, "SR"),
        ("tls-ec", KeyType::EcdsaP256, Request { profile: "local-tls", local_names: &["printer.local", "nas.home.arpa", "192.168.1.5", "localhost", "127.0.0.1"], ..Request::default() }, "V"),
        ("tls-rsa", KeyType::Rsa3072, Request { profile: "local-tls", local_names: &["printer.local"], ..Request::default() }, "V"),
        ("identity", KeyType::EcdsaP256, Request { profile: "device-identity", ..Request::default() }, "C"),
        ("login", KeyType::EcdsaP256, Request { profile: "service-login", name: Some("Home VPN"), login: Some("ada"), ..Request::default() }, "C"),
    ];
    for (nickname, key_type, request, usages) in cases {
        let signed = leaf(key_type, request);
        db.add_der(&scratch, nickname, ",,", &signed.issued.cert);
        for usage in usages.chars() {
            let (ok, said) = db.verify(nickname, &usage.to_string());
            assert!(ok, "{nickname} as {usage}: {said}");
        }
    }
    // A PDF certificate cannot sign code nor a code certificate a contract: the plan's reason for two
    assert!(!db.verify("pdf", "J").0);
    assert!(!db.verify("code", "S").0);
    assert!(!db.verify("login", "V").0);
    assert!(!db.verify("tls-ec", "C").0);
    // NSS reads a server certificate's CN as a host name, and the Device Identity CA has none to give: it is no website
    assert!(!db.verify("identity", "V").0);

    // With only another machine's root trusted, none of it is
    let other = Nss::new(&scratch, "other");
    other.add_der(&scratch, "otherroot", "C,C,C", &other_machine().root);
    other.add_der(&scratch, "account", ",,", &machine.account.cert);
    let signed = leaf(KeyType::Rsa3072, Request { profile: "pdf", name: Some("Ada Lovelace"), ..Request::default() });
    other.add_der(&scratch, "pdf", ",,", &signed.issued.cert);
    assert!(!other.verify("pdf", "S").0);

    // Local TLS with the root out of the database and only the Local Services CA trusted, as `ame vault tls trust` does it
    let chosen = Nss::new(&scratch, "chosen");
    chosen.add_der(&scratch, "localca", "C,,", &machine.local.cert);
    let tls = leaf(KeyType::EcdsaP256, Request { profile: "local-tls", local_names: &["printer.local"], ..Request::default() });
    chosen.add_der(&scratch, "tls", ",,", &tls.issued.cert);
    let (ok, said) = chosen.verify("tls", "V");
    assert!(ok, "{said}");
}

#[test]
fn public_names_are_refused_by_verifiers_even_if_forced_past_the_service() {
    if !need("openssl") || !need("certutil") {
        return;
    }
    let machine = machine();
    let scratch = Scratch::new("forged");
    let root = write_pem(&scratch, "root.pem", &machine.root);
    let chain = scratch.write("chain.pem", pem_chain(&machine.local.cert, &[]).unwrap().as_bytes());
    let db = Nss::new(&scratch, "db");
    db.add_der(&scratch, "root", "C,C,C", &machine.root);
    db.add_der(&scratch, "localca", ",,", &machine.local.cert);
    let cases: [(&str, Option<&str>); 6] = [
        ("/CN=forged", Some("DNS:www.google.com")),
        ("/CN=forged", Some("IP:8.8.8.8")),
        ("/CN=forged", Some("DNS:local.evil.example")),
        ("/CN=forged", Some("DNS:printer.local,DNS:www.google.com")),
        ("/CN=forged", Some("IP:2001:db8::1")),
        ("/CN=www.google.com", None),
    ];
    for (index, (subject, san)) in cases.into_iter().enumerate() {
        let forged = forge(&machine.local.cert, &machine.local_key, &scratch, 1000 + index as u32, subject, san);
        let (ok, said) = attempt(Command::new("openssl").args(["verify", "-x509_strict", "-purpose", "sslserver", "-CAfile"]).arg(&root).arg("-untrusted").arg(&chain).arg(&forged));
        assert!(!ok, "OpenSSL took {subject} {san:?}");
        assert!(said.contains("permitted subtree violation"), "{subject} {san:?}: {said}");
        let nickname = format!("forged{index}");
        db.add(&nickname, ",,", &forged);
        let (ok, said) = db.verify(&nickname, "V");
        assert!(!ok, "NSS took {subject} {san:?}: {said}");
    }
    // The same forge with a name inside the limits is accepted by both, so the refusals above are the limits' doing
    let forged = forge(&machine.local.cert, &machine.local_key, &scratch, 2000, "/CN=forged", Some("DNS:printer.local"));
    let (ok, said) = attempt(Command::new("openssl").args(["verify", "-x509_strict", "-purpose", "sslserver", "-CAfile"]).arg(&root).arg("-untrusted").arg(&chain).arg(&forged));
    assert!(ok, "{said}");
    db.add("forged-good", ",,", &forged);
    let (ok, said) = db.verify("forged-good", "V");
    assert!(ok, "{said}");
}

/// What phase 0 found in Firefox: with only a CA trusted, the root's limits are never read, so each CA has to hold on its own
#[test]
fn each_ca_alone_refuses_public_names() {
    if !need("openssl") || !need("certutil") {
        return;
    }
    let machine = machine();
    let cases: [(&str, &[u8], &CaKey, &str); 3] = [
        ("local", &machine.local.cert, &machine.local_key, "permitted subtree violation"),
        ("device", &machine.device.cert, &machine.device_key, "excluded subtree violation"),
        ("account", &machine.account.cert, &machine.account_key, "excluded subtree violation"),
    ];
    let names: [(&str, Option<&str>); 5] = [
        ("/CN=forged", Some("DNS:www.example.com")),
        ("/CN=forged", Some("IP:8.8.8.8")),
        ("/CN=forged", Some("IP:2001:db8::1")),
        ("/CN=forged", Some("DNS:printer.local,DNS:www.example.org")),
        ("/CN=www.example.com", None),
    ];
    for (name, ca, ca_key, violation) in cases {
        let scratch = Scratch::new("alone");
        let anchor = write_pem(&scratch, "anchor.pem", ca);
        let db = Nss::new(&scratch, "db");
        // As a client that was given this CA alone: no root anywhere, every kind of trust it could be given
        db.add_der(&scratch, "anchor", "CT,C,C", ca);
        for (index, (subject, san)) in names.into_iter().enumerate() {
            let forged = forge(ca, ca_key, &scratch, 3000 + index as u32, subject, san);
            let (ok, said) = attempt(Command::new("openssl").args(["verify", "-x509_strict", "-partial_chain", "-CAfile"]).arg(&anchor).arg(&forged));
            assert!(!ok, "OpenSSL with {name} alone took {subject} {san:?}");
            assert!(said.contains(violation), "{name}, {subject} {san:?}: {said}");
            let nickname = format!("forged{index}");
            db.add(&nickname, ",,", &forged);
            let (ok, said) = db.verify(&nickname, "V");
            assert!(!ok, "NSS with {name} alone took {subject} {san:?}: {said}");
        }
    }

    // The same anchors still take what they are for, so the refusals above are the limits' doing
    let scratch = Scratch::new("alone-good");
    let local = write_pem(&scratch, "local.pem", &machine.local.cert);
    let forged = forge(&machine.local.cert, &machine.local_key, &scratch, 4000, "/CN=forged", Some("DNS:printer.local,IP:192.168.1.5"));
    let (ok, said) = attempt(Command::new("openssl").args(["verify", "-x509_strict", "-partial_chain", "-purpose", "sslserver", "-CAfile"]).arg(&local).arg(&forged));
    assert!(ok, "{said}");
    let db = Nss::new(&scratch, "db");
    db.add_der(&scratch, "anchor", "C,,", &machine.local.cert);
    db.add("forged", ",,", &forged);
    let (ok, said) = db.verify("forged", "V");
    assert!(ok, "{said}");

    // The account's CA alone: a signature, a mail and a program, none of them with a host name to refuse
    let account = write_pem(&scratch, "account.pem", &machine.account.cert);
    let db = Nss::new(&scratch, "account-db");
    db.add_der(&scratch, "anchor", ",C,C", &machine.account.cert);
    let signed: [(&str, KeyType, Request<'_>, &str, &str); 3] = [
        ("pdf", KeyType::Rsa3072, Request { profile: "pdf", name: Some("Ada Lovelace"), email: Some("ada@example.org"), ..Request::default() }, "smimesign", "S"),
        ("smime", KeyType::EcdsaP256, Request { profile: "smime", name: Some("Ada Lovelace"), email: Some("ada@example.org"), ..Request::default() }, "smimesign", "S"),
        ("code", KeyType::Rsa3072, Request { profile: "code", name: Some("Ada Lovelace"), web: Some("https://example.org/ada"), ..Request::default() }, "codesign", "J"),
    ];
    for (nickname, key_type, request, purpose, usage) in signed {
        let signer = leaf(key_type, request);
        let path = write_pem(&scratch, "leaf.pem", &signer.issued.cert);
        let (ok, said) = attempt(Command::new("openssl").args(["verify", "-x509_strict", "-partial_chain", "-purpose", purpose, "-CAfile"]).arg(&account).arg(&path));
        assert!(ok, "{nickname}: {said}");
        db.add(nickname, ",,", &path);
        let (ok, said) = db.verify(nickname, usage);
        assert!(ok, "{nickname}: {said}");
    }

    // The Device Identity CA alone: a machine's and a login's certificate by OpenSSL, and a login by NSS
    let device = write_pem(&scratch, "device.pem", &machine.device.cert);
    let db = Nss::new(&scratch, "device-db");
    db.add_der(&scratch, "anchor", "T,,", &machine.device.cert);
    let identity = leaf(KeyType::EcdsaP256, Request { profile: "device-identity", ..Request::default() });
    let login = leaf(KeyType::EcdsaP256, Request { profile: "service-login", name: Some("Home VPN"), login: Some("ada"), ..Request::default() });
    let identity_path = write_pem(&scratch, "identity.pem", &identity.issued.cert);
    let login_path = write_pem(&scratch, "login.pem", &login.issued.cert);
    for purpose in ["sslclient", "sslserver"] {
        let (ok, said) = attempt(Command::new("openssl").args(["verify", "-x509_strict", "-partial_chain", "-purpose", purpose, "-CAfile"]).arg(&device).arg(&identity_path));
        assert!(ok, "identity as {purpose}: {said}");
    }
    let (ok, said) = attempt(Command::new("openssl").args(["verify", "-x509_strict", "-partial_chain", "-purpose", "sslclient", "-CAfile"]).arg(&device).arg(&login_path));
    assert!(ok, "{said}");
    db.add("login", ",,", &login_path);
    let (ok, said) = db.verify("login", "C");
    assert!(ok, "{said}");
}

#[test]
fn a_host_shaped_signer_would_have_failed() {
    // Why `NameLooksLikeHost` exists: this is the certificate it keeps from being made
    if !need("openssl") {
        return;
    }
    let machine = machine();
    let scratch = Scratch::new("hostshaped");
    let ca_cert = write_pem(&scratch, "ca.pem", &machine.account.cert);
    let ca_key = scratch.write("ca.key", &plain(&machine.account_key).export_plain().unwrap());
    let root = write_pem(&scratch, "root.pem", &machine.root);
    let csr = scratch.path("x.csr");
    let key_path = scratch.path("x.key");
    run(Command::new("openssl").args(["req", "-new", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256", "-nodes", "-subj", "/OU=Amethystora Vault/CN=j.smith", "-keyout"]).arg(&key_path).arg("-out").arg(&csr));
    let extensions = scratch.write("x.ext", b"basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=emailProtection\nsubjectKeyIdentifier=hash\nauthorityKeyIdentifier=keyid\n");
    let out = scratch.path("x.pem");
    run(Command::new("openssl").args(["x509", "-req", "-days", "30", "-sha256", "-set_serial", "7", "-in"]).arg(&csr).arg("-CA").arg(&ca_cert).arg("-CAkey").arg(&ca_key).arg("-extfile").arg(&extensions).arg("-out").arg(&out));
    let (ok, said) = attempt(Command::new("openssl").args(["verify", "-purpose", "smimesign", "-CAfile"]).arg(&root).arg("-untrusted").arg(&ca_cert).arg(&out));
    assert!(!ok, "{said}");
    assert!(said.contains("permitted subtree violation"), "{said}");
}

/// The smallest thing `osslsigncode` takes for a Windows program: a DOS stub, PE32+ headers and one empty section
fn minimal_pe() -> Vec<u8> {
    let mut pe = vec![0u8; 1024];
    pe[0..2].copy_from_slice(b"MZ");
    pe[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    pe[0x40..0x44].copy_from_slice(b"PE\0\0");
    let coff = 0x44;
    pe[coff..coff + 2].copy_from_slice(&0x8664u16.to_le_bytes());
    pe[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes());
    pe[coff + 16..coff + 18].copy_from_slice(&240u16.to_le_bytes());
    pe[coff + 18..coff + 20].copy_from_slice(&0x22u16.to_le_bytes());
    let optional = coff + 20;
    pe[optional..optional + 2].copy_from_slice(&0x20bu16.to_le_bytes());
    pe[optional + 32..optional + 36].copy_from_slice(&0x1000u32.to_le_bytes());
    pe[optional + 36..optional + 40].copy_from_slice(&0x200u32.to_le_bytes());
    pe[optional + 56..optional + 60].copy_from_slice(&0x2000u32.to_le_bytes());
    pe[optional + 60..optional + 64].copy_from_slice(&0x200u32.to_le_bytes());
    pe[optional + 68..optional + 70].copy_from_slice(&3u16.to_le_bytes());
    pe[optional + 108..optional + 112].copy_from_slice(&16u32.to_le_bytes());
    let section = optional + 240;
    pe[section..section + 5].copy_from_slice(b".text");
    pe[section + 8..section + 12].copy_from_slice(&0x200u32.to_le_bytes());
    pe[section + 12..section + 16].copy_from_slice(&0x1000u32.to_le_bytes());
    pe[section + 16..section + 20].copy_from_slice(&0x200u32.to_le_bytes());
    pe[section + 20..section + 24].copy_from_slice(&0x200u32.to_le_bytes());
    pe[section + 36..section + 40].copy_from_slice(&0x60000020u32.to_le_bytes());
    pe
}

#[test]
fn a_program_signed_with_osslsigncode() {
    if !need("osslsigncode") || !need("openssl") {
        return;
    }
    let machine = machine();
    let scratch = Scratch::new("pe");
    let signer = leaf(KeyType::Rsa3072, Request { profile: "code", name: Some("Ada Lovelace"), ..Request::default() });
    let bundle = pfx(&signer.key, &signer.issued.cert, &signer.issued.chain, "Ada Lovelace", "a typed password").unwrap();
    let bundle = scratch.write("code.pfx", &bundle);
    let program = scratch.write("program.exe", &minimal_pe());
    let signed = scratch.path("signed.exe");
    run(Command::new("osslsigncode").args(["sign", "-pkcs12"]).arg(&bundle).args(["-pass", "a typed password", "-h", "sha256", "-n", "Test", "-in"]).arg(&program).arg("-out").arg(&signed));

    let root = write_pem(&scratch, "root.pem", &machine.root);
    let (ok, said) = attempt(Command::new("osslsigncode").args(["verify", "-CAfile"]).arg(&root).arg("-in").arg(&signed));
    assert!(ok && said.contains("Signature verification: ok"), "against the root: {said}");
    let account = write_pem(&scratch, "account.pem", &machine.account.cert);
    // osslsigncode verifies without OpenSSL's partial chains, so it asks for a chain up to a self-signed root and will not
    // stop at the account's CA, whichever way the certificates are made. The certificate itself is fine by that CA alone.
    let (ok, said) = attempt(Command::new("osslsigncode").args(["verify", "-CAfile"]).arg(&account).arg("-in").arg(&signed));
    assert!(!ok && said.contains("unable to get issuer certificate"), "against the account's CA alone: {said}");
    let cert = write_pem(&scratch, "signer.pem", &signer.issued.cert);
    let (ok, said) = attempt(Command::new("openssl").args(["verify", "-x509_strict", "-partial_chain", "-purpose", "codesign", "-CAfile"]).arg(&account).arg(&cert));
    assert!(ok, "openssl with the account's CA alone: {said}");
    let other = write_pem(&scratch, "other.pem", &other_machine().root);
    let (ok, said) = attempt(Command::new("osslsigncode").args(["verify", "-CAfile"]).arg(&other).arg("-in").arg(&signed));
    assert!(!ok, "against another machine's root: {said}");

    // A PDF certificate is refused for it: not by osslsigncode's signing, but by its purpose
    let pdf = leaf(KeyType::Rsa3072, Request { profile: "pdf", name: Some("Ada Lovelace"), ..Request::default() });
    let bundle = scratch.write("pdf.pfx", &pfx(&pdf.key, &pdf.issued.cert, &pdf.issued.chain, "Ada", "pw").unwrap());
    let wrong = scratch.path("wrong.exe");
    let signed_wrong = attempt(Command::new("osslsigncode").args(["sign", "-pkcs12"]).arg(&bundle).args(["-pass", "pw", "-h", "sha256", "-in"]).arg(&program).arg("-out").arg(&wrong));
    if signed_wrong.0 {
        let (ok, said) = attempt(Command::new("osslsigncode").args(["verify", "-CAfile"]).arg(&root).arg("-in").arg(&wrong));
        assert!(!ok, "a PDF certificate on a program: {said}");
    }
}

#[test]
fn a_jar_signed_with_jarsigner() {
    if !need("jarsigner") || !need("keytool") || !need("jar") {
        return;
    }
    let machine = machine();
    let scratch = Scratch::new("jar");
    let signer = leaf(KeyType::Rsa3072, Request { profile: "code", name: Some("Ada Lovelace"), ..Request::default() });
    let bundle = scratch.write("code.pfx", &pfx(&signer.key, &signer.issued.cert, &signer.issued.chain, "Ada Lovelace", "storepass").unwrap());
    scratch.write("hello.txt", b"hello");
    let jar = scratch.path("a.jar");
    run(Command::new("jar").arg("cf").arg(&jar).args(["-C"]).arg(scratch.dir()).arg("hello.txt"));
    let listed = output(Command::new("keytool").args(["-list", "-storetype", "PKCS12", "-storepass", "storepass", "-keystore"]).arg(&bundle));
    assert!(listed.contains("ada lovelace"), "{listed}");
    run(Command::new("jarsigner").args(["-storetype", "PKCS12", "-storepass", "storepass", "-digestalg", "SHA-256", "-keystore"]).arg(&bundle).arg(&jar).arg("ada lovelace"));
    let trust = |name: &str, cert: &[u8]| {
        let store = scratch.path(name);
        let file = write_pem(&scratch, &format!("{name}.pem"), cert);
        run(Command::new("keytool").args(["-importcert", "-noprompt", "-alias", "anchor", "-storetype", "PKCS12", "-storepass", "storepass", "-file"]).arg(&file).arg("-keystore").arg(&store));
        store
    };
    let verify = |store: &Path| attempt(Command::new("jarsigner").args(["-verify", "-verbose", "-certs", "-storetype", "PKCS12", "-storepass", "storepass", "-keystore"]).arg(store).arg(&jar));
    // jarsigner calls a jar verified whatever it thinks of the chain, and says that apart. Older JDKs word it differently.
    let refused = |said: &str| said.contains("not validated") || said.contains("chain is invalid") || said.contains("Invalid certificate chain");
    let (ok, said) = verify(&trust("root.p12", &machine.root));
    assert!(ok && said.contains("jar verified"), "{said}");
    assert!(!refused(&said), "against the root: {said}");
    let (ok, said) = verify(&trust("account.p12", &machine.account.cert));
    assert!(ok && said.contains("jar verified"), "{said}");
    assert!(!refused(&said), "against the account's CA alone: {said}");
    let (_, said) = verify(&trust("other.p12", &other_machine().root));
    assert!(refused(&said), "against another machine's root: {said}");
}

#[test]
fn pfx_opens_in_openssl_and_nss() {
    if !need("openssl") || !need("pk12util") {
        return;
    }
    let scratch = Scratch::new("pfx");
    let signer = leaf(KeyType::Rsa3072, Request { profile: "code", name: Some("Ada Lovelace"), ..Request::default() });
    let bundle = scratch.write("x.pfx", &pfx(&signer.key, &signer.issued.cert, &signer.issued.chain, "Ada Lovelace", "pässword").unwrap());
    let info = output(Command::new("openssl").args(["pkcs12", "-info", "-noout", "-passin", "pass:pässword", "-in"]).arg(&bundle));
    let said = attempt(Command::new("openssl").args(["pkcs12", "-info", "-noout", "-passin", "pass:pässword", "-in"]).arg(&bundle)).1;
    assert!(said.contains("MAC: sha256"), "{info} {said}");
    assert!(said.contains("PBES2, PBKDF2, AES-256-CBC"), "{said}");
    assert!(!said.contains("RC2") && !said.contains("3DES"), "{said}");
    assert!(said.contains("Iteration 600000"), "{said}");
    assert!(!attempt(Command::new("openssl").args(["pkcs12", "-info", "-noout", "-passin", "pass:wrong", "-in"]).arg(&bundle)).0);
    // The certificates and the key all come out
    let pem = output(Command::new("openssl").args(["pkcs12", "-nodes", "-passin", "pass:pässword", "-in"]).arg(&bundle));
    assert_eq!(pem.matches("BEGIN CERTIFICATE").count(), 3, "{pem}");
    assert!(pem.contains("BEGIN PRIVATE KEY"));
    let db = Nss::new(&scratch, "db");
    let (ok, said) = attempt(Command::new("pk12util").args(["-i"]).arg(&bundle).args(["-d", &db.dir, "-W", "pässword"]));
    assert!(ok, "{said}");
    // Refused: what would end the service, and what is no password
    let code = &signer.issued;
    assert_eq!(pfx(&signer.key, &code.cert, &code.chain, "x", "").unwrap_err(), PkiError::BadPassword);
    assert_eq!(pfx(&signer.key, &code.cert, &code.chain, "x", "a\0b").unwrap_err(), PkiError::BadPassword);
    assert_eq!(pfx(&signer.key, &code.cert, &code.chain, "a\0b", "x").unwrap_err(), PkiError::BadField("name"));
    assert_eq!(pfx(&key(KeyType::Rsa3072), &code.cert, &code.chain, "x", "y").unwrap_err(), PkiError::IssuerKeyMismatch);
    // The PEM chain reads back whole
    let chain = scratch.write("chain.pem", pem_chain(&code.cert, &code.chain).unwrap().as_bytes());
    assert_eq!(std::fs::read_to_string(&chain).unwrap().matches("BEGIN CERTIFICATE").count(), 3);
    assert_eq!(pem_chain(b"not a certificate", &[]).unwrap_err(), PkiError::Malformed);
}

#[test]
fn smime_with_openssl_cms() {
    if !need("openssl") {
        return;
    }
    let machine = machine();
    let scratch = Scratch::new("smime");
    let root = write_pem(&scratch, "root.pem", &machine.root);
    let other = write_pem(&scratch, "other.pem", &other_machine().root);
    let mail = scratch.write("mail.txt", b"Dear Charles,\r\nThe engine weaves algebraic patterns.\r\n");
    for key_type in [KeyType::Rsa3072, KeyType::EcdsaP256] {
        let signer = leaf(key_type, Request { profile: "smime", name: Some("Ada Lovelace"), email: Some("ada@example.org"), ..Request::default() });
        let cert = write_pem(&scratch, "signer.pem", &signer.issued.cert);
        let key_path = scratch.write("signer.key", &signer.key.export_plain().unwrap());
        let chain = scratch.write("chain.pem", pem_chain(&signer.issued.chain[0], &[]).unwrap().as_bytes());
        let signed = scratch.path("signed.p7m");
        run(Command::new("openssl").args(["cms", "-sign", "-nodetach", "-in"]).arg(&mail).arg("-signer").arg(&cert).arg("-inkey").arg(&key_path).arg("-certfile").arg(&chain).arg("-out").arg(&signed));
        let out = scratch.path("out.txt");
        run(Command::new("openssl").args(["cms", "-verify", "-x509_strict", "-purpose", "smimesign", "-in"]).arg(&signed).arg("-CAfile").arg(&root).arg("-out").arg(&out));
        assert!(std::fs::read_to_string(&out).unwrap().contains("algebraic"));
        assert!(!attempt(Command::new("openssl").args(["cms", "-verify", "-purpose", "smimesign", "-in"]).arg(&signed).arg("-CAfile").arg(&other)).0, "another machine's root");
        // RSA encrypts as well, which is what the profile adds keyEncipherment for
        if key_type == KeyType::Rsa3072 {
            let sealed = scratch.path("sealed.p7m");
            run(Command::new("openssl").args(["cms", "-encrypt", "-aes256", "-in"]).arg(&mail).arg("-out").arg(&sealed).arg(&cert));
            let opened = scratch.path("opened.txt");
            run(Command::new("openssl").args(["cms", "-decrypt", "-in"]).arg(&sealed).arg("-recip").arg(&cert).arg("-inkey").arg(&key_path).arg("-out").arg(&opened));
            assert!(std::fs::read_to_string(&opened).unwrap().contains("algebraic"));
        }
    }
}

#[test]
fn the_digest_signatures_verify() {
    if !need("openssl") {
        return;
    }
    let machine = machine();
    let scratch = Scratch::new("cms");
    let root = write_pem(&scratch, "root.pem", &machine.root);
    let document = scratch.write("doc.bin", b"the bytes a PDF's signature covers");
    let sha256 = openssl::sha::sha256(&std::fs::read(&document).unwrap()).to_vec();
    let sha384 = openssl::sha::sha384(&std::fs::read(&document).unwrap()).to_vec();
    for (key_type, digest, value) in [
        (KeyType::Rsa3072, Digest::Sha256, &sha256),
        (KeyType::Rsa4096, Digest::Sha384, &sha384),
        (KeyType::EcdsaP256, Digest::Sha256, &sha256),
        (KeyType::EcdsaP384, Digest::Sha384, &sha384),
    ] {
        let signer = leaf(key_type, Request { profile: "pdf", name: Some("Ada Lovelace"), ..Request::default() });
        let by = Signer::leaf(&signer.key, &signer.issued.cert, Purpose::Pdf, now()).unwrap();
        let cms = sign_pdf_digest(&by, &signer.issued.chain, digest, value, now()).unwrap();
        let path = scratch.write("sig.der", &cms);
        let (ok, said) = attempt(Command::new("openssl").args(["cms", "-verify", "-binary", "-inform", "DER", "-purpose", "smimesign", "-in"]).arg(&path).arg("-content").arg(&document).arg("-CAfile").arg(&root).arg("-out").arg("/dev/null"));
        assert!(ok, "{key_type:?}: {said}");
        // Another document is refused, and so is another machine's root
        let other_document = scratch.write("other.bin", b"some other bytes");
        assert!(!attempt(Command::new("openssl").args(["cms", "-verify", "-binary", "-inform", "DER", "-purpose", "smimesign", "-in"]).arg(&path).arg("-content").arg(&other_document).arg("-CAfile").arg(&root).arg("-out").arg("/dev/null")).0);
        let other_root = write_pem(&scratch, "otherroot.pem", &other_machine().root);
        assert!(!attempt(Command::new("openssl").args(["cms", "-verify", "-binary", "-inform", "DER", "-purpose", "smimesign", "-in"]).arg(&path).arg("-content").arg(&document).arg("-CAfile").arg(&other_root).arg("-out").arg("/dev/null")).0);
        // A bare signature over the digest, which `openssl pkeyutl` checks
        let bare = scratch.write("bare.sig", &sign_digest(&by, digest, value).unwrap());
        let cert = write_pem(&scratch, "signer.pem", &signer.issued.cert);
        let digest_file = scratch.write("digest.bin", value);
        let algorithm = if digest == Digest::Sha256 { "digest:sha256" } else { "digest:sha384" };
        let (ok, said) = attempt(Command::new("openssl").args(["pkeyutl", "-verify", "-certin", "-inkey"]).arg(&cert).arg("-in").arg(&digest_file).arg("-sigfile").arg(&bare).args(["-pkeyopt", algorithm]));
        assert!(ok, "{key_type:?}: {said}");
    }
    // A digest of the wrong length, a key that cannot, a certificate that is not this key's
    let signer = leaf(KeyType::EcdsaP256, Request { profile: "pdf", name: Some("Ada"), ..Request::default() });
    let issued = &signer.issued;
    let by = Signer::leaf(&signer.key, &issued.cert, Purpose::Pdf, now()).unwrap();
    assert_eq!(sign_pdf_digest(&by, &issued.chain, Digest::Sha256, &sha384, now()).unwrap_err(), PkiError::BadDigest);
    assert_eq!(sign_digest(&by, Digest::Sha384, &sha256).unwrap_err(), PkiError::BadDigest);
    let ed25519 = key(KeyType::Ed25519);
    assert_eq!(sign_digest(&Signer::ssh(&ed25519, KeyKind::Ssh).unwrap(), Digest::Sha256, &sha256).unwrap_err(), PkiError::KeyNotAllowed);
    assert_eq!(Signer::leaf(&key(KeyType::EcdsaP256), &issued.cert, Purpose::Pdf, now()).err(), Some(PkiError::IssuerKeyMismatch));
}

/// A one-page PDF with a signature whose /Contents is `sign` over the document's /ByteRange. What the Vault would write
/// itself if the PKCS#11 route ever fails: this much, and the same again for an existing file's incremental update.
fn pdf_signed_by(sign: impl Fn(&[u8]) -> Vec<u8>) -> Vec<u8> {
    const SPACE: usize = 16384;
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    let mut object = |pdf: &mut Vec<u8>, text: &str| {
        offsets.push(pdf.len());
        pdf.extend_from_slice(text.as_bytes());
        offsets.clone()
    };
    object(&mut pdf, "1 0 obj\n<</Type/Catalog/Pages 2 0 R/AcroForm<</Fields[5 0 R]/SigFlags 3>>>>\nendobj\n");
    object(&mut pdf, "2 0 obj\n<</Type/Pages/Kids[3 0 R]/Count 1>>\nendobj\n");
    object(&mut pdf, "3 0 obj\n<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]/Annots[5 0 R]>>\nendobj\n");
    let signature_at = pdf.len();
    let placeholder = "0".repeat(SPACE * 2);
    let range = "[0 0000000000 0000000000 0000000000]";
    object(&mut pdf, &format!("4 0 obj\n<</Type/Sig/Filter/Adobe.PPKLite/SubFilter/adbe.pkcs7.detached/M(D:20261102120000Z)/ByteRange{range}/Contents<{placeholder}>>>\nendobj\n"));
    let offsets = object(&mut pdf, "5 0 obj\n<</Type/Annot/Subtype/Widget/FT/Sig/T(Signature1)/V 4 0 R/F 132/Rect[0 0 0 0]/P 3 0 R>>\nendobj\n");
    let xref = pdf.len();
    let mut table = String::from("xref\n0 6\n0000000000 65535 f \n");
    for offset in &offsets {
        table.push_str(&format!("{offset:010} 00000 n \n"));
    }
    table.push_str(&format!("trailer\n<</Size 6/Root 1 0 R>>\nstartxref\n{xref}\n%%EOF\n"));
    pdf.extend_from_slice(table.as_bytes());

    let find = |needle: &[u8]| signature_at + pdf[signature_at..].windows(needle.len()).position(|window| window == needle).unwrap();
    let hex_at = find(b"/Contents<") + b"/Contents".len();
    let after = hex_at + 1 + SPACE * 2 + 1;
    let filled = format!("[0 {hex_at:010} {after:010} {:010}]", pdf.len() - after);
    let range_at = find(range.as_bytes());
    pdf[range_at..range_at + filled.len()].copy_from_slice(filled.as_bytes());

    let mut covered = pdf[..hex_at].to_vec();
    covered.extend_from_slice(&pdf[after..]);
    let cms = sign(&covered);
    assert!(cms.len() <= SPACE);
    let encoded: String = cms.iter().map(|byte| format!("{byte:02x}")).collect();
    pdf[hex_at + 1..hex_at + 1 + encoded.len()].copy_from_slice(encoded.as_bytes());
    pdf
}

fn pdfsig_says(file: &Path, db: &Nss) -> String {
    let out = Command::new("pdfsig").args(["-nssdir", &db.dir, "-no-ocsp"]).arg(file).stdin(Stdio::null()).output().unwrap();
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

#[test]
fn a_pdf_signed_from_its_digest_reads_trusted_in_pdfsig() {
    if !need("pdfsig") || !need("certutil") {
        return;
    }
    let machine = machine();
    let scratch = Scratch::new("pdf");
    for (key_type, digest) in [(KeyType::Rsa3072, Digest::Sha256), (KeyType::EcdsaP256, Digest::Sha256), (KeyType::EcdsaP384, Digest::Sha384)] {
        for name in ["Ada Lovelace", "أروى إدريس"] {
            let signer = leaf(key_type, Request { profile: "pdf", name: Some(name), ..Request::default() });
            let pdf = pdf_signed_by(|covered| {
                let value = match digest {
                    Digest::Sha256 => openssl::sha::sha256(covered).to_vec(),
                    Digest::Sha384 => openssl::sha::sha384(covered).to_vec(),
                };
                let by = Signer::leaf(&signer.key, &signer.issued.cert, Purpose::Pdf, now()).unwrap();
                sign_pdf_digest(&by, &signer.issued.chain, digest, &value, now()).unwrap()
            });
            let path = scratch.write("signed.pdf", &pdf);

            // Trusted with the root at `,C,`, as this machine's own readers have it
            let email = Nss::new(&scratch, &format!("email-{key_type:?}-{}", name.len()));
            email.add_der(&scratch, "root", ",C,", &machine.root);
            let said = pdfsig_says(&path, &email);
            assert!(said.contains("Signature is Valid"), "{key_type:?} {name}: {said}");
            assert!(said.contains("Certificate is Trusted"), "{key_type:?} {name}: {said}");
            assert!(said.contains("Total document signed"), "{key_type:?} {name}: {said}");
            assert!(said.contains(name), "{said}");

            // And with only the account's CA, given to someone else
            let given = Nss::new(&scratch, &format!("given-{key_type:?}-{}", name.len()));
            given.add_der(&scratch, "account", ",C,", &machine.account.cert);
            let said = pdfsig_says(&path, &given);
            assert!(said.contains("Signature is Valid") && said.contains("Certificate is Trusted"), "the account's CA alone: {said}");

            // TLS trust gives it nothing, and another machine's root refuses it
            let tls = Nss::new(&scratch, &format!("tls-{key_type:?}-{}", name.len()));
            tls.add_der(&scratch, "root", "C,,", &machine.root);
            let said = pdfsig_says(&path, &tls);
            assert!(said.contains("Signature is Valid") && !said.contains("Certificate is Trusted"), "{said}");
            let other = Nss::new(&scratch, &format!("other-{key_type:?}-{}", name.len()));
            other.add_der(&scratch, "root", ",C,", &other_machine().root);
            let said = pdfsig_says(&path, &other);
            assert!(said.contains("Signature is Valid") && !said.contains("Certificate is Trusted"), "another machine's root: {said}");

            // A changed byte breaks the signature
            let mut changed = pdf.clone();
            let at = changed.windows(8).position(|window| window == b"MediaBox").unwrap();
            changed[at + 12] = b'1';
            let path = scratch.write("changed.pdf", &changed);
            let said = pdfsig_says(&path, &email);
            assert!(!said.contains("Signature is Valid"), "{said}");
        }
    }
}

#[test]
fn pdfsig_signs_with_a_pfx_as_a_person_would() {
    if !need("pdfsig") || !need("pk12util") {
        return;
    }
    let machine = machine();
    let scratch = Scratch::new("pdfsig");
    let signer = leaf(KeyType::Rsa3072, Request { profile: "pdf", name: Some("Ada Lovelace"), ..Request::default() });
    let bundle = scratch.write("x.pfx", &pfx(&signer.key, &signer.issued.cert, &signer.issued.chain, "Ada Lovelace", "pw").unwrap());
    let db = Nss::new(&scratch, "db");
    run(Command::new("pk12util").arg("-i").arg(&bundle).args(["-d", &db.dir, "-W", "pw"]));
    let unsigned = scratch.write("unsigned.pdf", &unsigned_pdf());
    let out = scratch.path("out.pdf");
    run(Command::new("pdfsig").args(["-nssdir", &db.dir, "-add-signature", "-nick", "Ada Lovelace", "-new-signature-field-name", "S1"]).arg(&unsigned).arg(&out));
    let said = pdfsig_says(&out, &db);
    assert!(said.contains("Signature is Valid"), "{said}");
    let trusted = Nss::new(&scratch, "trusted");
    trusted.add_der(&scratch, "root", ",C,", &machine.root);
    let said = pdfsig_says(&out, &trusted);
    assert!(said.contains("Certificate is Trusted"), "{said}");
}

/// A one-page PDF with nothing signed yet, which is what `pdfsig -add-signature` takes
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

/// A real TLS server with the certificate, until dropped
struct Server(std::process::Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn s_server(cert: &Path, key: &Path, chain: &Path, port: u16) -> Server {
    let server = Server(Command::new("openssl")
        .args(["s_server", "-www", "-accept", &port.to_string(), "-cert"])
        .arg(cert)
        .arg("-key")
        .arg(key)
        .arg("-cert_chain")
        .arg(chain)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap());
    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return server;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("s_server did not start");
}

fn s_client(port: u16, ca: &Path, options: &[&str]) -> (bool, String) {
    let mut child = Command::new("openssl")
        .args(["s_client", "-connect", &format!("127.0.0.1:{port}"), "-verify_return_error", "-CAfile"])
        .arg(ca)
        .args(options)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    (out.status.success() && said.contains("Verify return code: 0 (ok)"), said)
}

#[test]
fn local_tls_in_a_real_handshake() {
    if !need("openssl") {
        return;
    }
    let machine = machine();
    let scratch = Scratch::new("tls");
    let root = write_pem(&scratch, "root.pem", &machine.root);
    let local_ca = write_pem(&scratch, "local.pem", &machine.local.cert);
    let other = write_pem(&scratch, "other.pem", &other_machine().root);
    let chain = scratch.write("chain.pem", pem_chain(&machine.local.cert, &[]).unwrap().as_bytes());
    for (index, key_type) in [KeyType::EcdsaP256, KeyType::Rsa3072].into_iter().enumerate() {
        let server = leaf(key_type, Request { profile: "local-tls", local_names: &["printer.local", "localhost", "127.0.0.1"], ..Request::default() });
        let cert = write_pem(&scratch, "server.pem", &server.issued.cert);
        let key_path = scratch.write("server.key", &server.key.export_plain().unwrap());
        let port = 38443 + index as u16;
        let _server = s_server(&cert, &key_path, &chain, port);

        // Trusting the root, and the name checked
        let (ok, said) = s_client(port, &root, &["-verify_hostname", "printer.local", "-x509_strict"]);
        assert!(ok, "{key_type:?} with the root: {said}");
        let (ok, said) = s_client(port, &root, &["-verify_ip", "127.0.0.1", "-tls1_2"]);
        assert!(ok, "{key_type:?} over TLS 1.2: {said}");
        // Trusting only the Local Services CA, as a client does by choice
        let (ok, said) = s_client(port, &local_ca, &["-partial_chain", "-verify_hostname", "printer.local"]);
        assert!(ok, "{key_type:?} with the Local Services CA alone: {said}");
        // The wrong name, and another machine's root
        assert!(!s_client(port, &root, &["-verify_hostname", "other.local"]).0);
        assert!(!s_client(port, &other, &["-verify_hostname", "printer.local"]).0);

        if need("curl") {
            let url = format!("https://printer.local:{port}/");
            let resolve = format!("printer.local:{port}:127.0.0.1");
            let (ok, said) = attempt(Command::new("curl").args(["-s", "-S", "-o", "/dev/null", "--resolve", &resolve, "--cacert"]).arg(&local_ca).arg(&url));
            assert!(ok, "curl with the Local Services CA: {said}");
            assert!(!attempt(Command::new("curl").args(["-s", "-S", "-o", "/dev/null", "--resolve", &resolve, "--cacert"]).arg(&other).arg(&url)).0);
        }
    }
}

#[test]
fn a_certificate_described() {
    let signer = leaf(KeyType::EcdsaP256, Request { profile: "pdf", name: Some("Ada Lovelace"), ..Request::default() });
    let description = describe(&signer.issued.cert).unwrap();
    assert_eq!(description.subject, "CN=Ada Lovelace, OU=Amethystora Vault");
    assert_eq!(description.issuer, "CN=Ada Lovelace Code Signing CA, OU=Amethystora OS");
    assert_eq!(description.sha256.len(), 64);
    assert_eq!(description.key, signer.key.public().unwrap());
    assert_eq!(describe(b"nonsense").unwrap_err(), PkiError::Malformed);
    assert_eq!(profile_purposes(Profile::Code), ["1.3.6.1.5.5.7.3.3"]);
    assert_eq!(Profile::Pdf.key_types()[0], KeyType::Rsa3072);
    assert_eq!(Profile::LocalTls.key_types()[0], KeyType::EcdsaP256);
    assert_eq!(Profile::all().count(), 9);
    assert_eq!(Profile::AccountCa.makes(), Some(Authority::AccountCa));
    assert_eq!(Profile::Pdf.makes(), None);
}

#[test]
fn a_change_to_one_byte_of_a_certificate_is_refused_not_a_panic() {
    let signer = leaf(KeyType::EcdsaP256, Request { profile: "pdf", name: Some("Ada Lovelace"), ..Request::default() });
    let der = &signer.issued.cert;
    for cut in [0, 1, 10, der.len() / 2, der.len() - 1] {
        assert!(describe(&der[..cut]).is_err());
    }
    let machine = machine();
    let at = now();
    let root = machine.root.clone();
    // A CA that is not this root's, or not a certificate at all
    assert!(!ca_is_current(&other_machine().account.cert, &root, at).unwrap());
    assert!(ca_is_current(b"nonsense", &root, at).is_err());
}
