//! The one table of what the machine's CAs may issue. A request names a row, and says nothing else about a certificate's
//! purposes, key usage or validity. plans/PLAN-vault.md has the fields; the choices it leaves open are marked here.

use crate::keys::KeyType;

pub const ROOT_NAME: &str = "Amethystora OS Root CA";
pub(super) const PERSON_CA_SUFFIX: &str = " Code Signing CA";
pub(super) const OS_UNIT: &str = "Amethystora OS";
pub(super) const VAULT_UNIT: &str = "Amethystora Vault";
const LOCAL_SERVICE: &str = "Local service";
const DEVICE_CA: &str = "Device Identity CA";
const LOCAL_CA: &str = "Local Services CA";

const SERVER_AUTH: &str = "1.3.6.1.5.5.7.3.1";
const CLIENT_AUTH: &str = "1.3.6.1.5.5.7.3.2";
const CODE_SIGNING: &str = "1.3.6.1.5.5.7.3.3";
const EMAIL_PROTECTION: &str = "1.3.6.1.5.5.7.3.4";
/// Microsoft's document signing
const DOCUMENT_SIGNING: &str = "1.3.6.1.4.1.311.10.3.12";

/// Five years, a CA's validity, never past the root's
const CA_DAYS: u32 = 1826;
/// 460 days, the public rule's limit for code signing (Baseline Requirements 6.3.2)
pub(super) const SIGNING_DAYS: u32 = 460;
const TLS_DAYS: u32 = 397;

/// What signs: the root, or one of the three CAs under it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Authority {
    Root,
    AccountCa,
    DeviceCa,
    LocalServicesCa,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    AccountCa,
    Pdf,
    Code,
    Smime,
    LocalTls,
    DeviceCa,
    DeviceIdentity,
    ServiceLogin,
    LocalServicesCa,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Purposes {
    /// The account's CA covers all its leaves, as NSS requires of a CA with purposes of its own
    AccountCa,
    Pdf,
    Code,
    Smime,
    Server,
    ClientAndServer,
    Client,
}

impl Purposes {
    pub(super) fn oids(self) -> &'static [&'static str] {
        match self {
            Purposes::AccountCa => &[CODE_SIGNING, EMAIL_PROTECTION, DOCUMENT_SIGNING],
            Purposes::Pdf => &[EMAIL_PROTECTION, DOCUMENT_SIGNING],
            Purposes::Code => &[CODE_SIGNING],
            Purposes::Smime => &[EMAIL_PROTECTION],
            Purposes::Server => &[SERVER_AUTH],
            Purposes::ClientAndServer => &[CLIENT_AUTH, SERVER_AUTH],
            Purposes::Client => &[CLIENT_AUTH],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SubjectShape {
    /// `CN=<person> Code Signing CA, OU=Amethystora OS`
    PersonCa,
    /// `CN=<signer>, OU=Amethystora Vault`, and where it says so `O` and `C` if typed
    Signer { organization: bool },
    /// `CN=<name>, OU=Machine 3FA107C2`, for the two CAs the machine itself holds
    MachineCa(&'static str),
    /// `CN=Machine 3FA107C2, OU=Amethystora OS`
    Machine,
    /// `CN=<name>, OU=Amethystora OS`
    Fixed(&'static str),
    /// `CN=<name you give it>, OU=<login>, OU=Machine 3FA107C2`
    Login,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Alternative {
    None,
    Email { required: bool },
    EmailOrWeb,
    LocalNames,
}

/// A CA's own Name Constraints. A path stops at the anchor, so with only a CA trusted the root's are never read: Firefox
/// reached `www.example.com` that way (plans/vault/work/spike.md, 10). Every CA a client may trust on its own carries these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Limits {
    /// The root's list
    LocalNames,
    /// No DNS name and no address at all, for a CA that has no business with either
    NoNames,
}

pub(super) struct Spec {
    pub profile: Profile,
    pub name: &'static str,
    pub issuer: Authority,
    /// Which CA this makes, or `None` for a certificate that cannot issue
    pub makes: Option<Authority>,
    pub days: u32,
    pub purposes: Purposes,
    /// The first is the default
    pub keys: &'static [KeyType],
    /// An RSA key here adds `keyEncipherment`: NSS refused a TLS one without it, and S/MIME encrypts to it
    pub rsa_enciphers: bool,
    pub subject: SubjectShape,
    pub alternative: Alternative,
    /// For a CA: its Name Constraints
    pub limits: Option<Limits>,
}

const SIGNING_KEYS: &[KeyType] = &[KeyType::Rsa3072, KeyType::Rsa4096, KeyType::EcdsaP256, KeyType::EcdsaP384];
const TLS_KEYS: &[KeyType] = &[KeyType::EcdsaP256, KeyType::Rsa3072, KeyType::Rsa4096];
const CA_KEYS: &[KeyType] = &[KeyType::Rsa3072];

#[rustfmt::skip]
const SPECS: [Spec; 9] = [
    Spec { profile: Profile::AccountCa, name: "account-ca", issuer: Authority::Root, makes: Some(Authority::AccountCa),
        days: CA_DAYS, purposes: Purposes::AccountCa, keys: CA_KEYS, rsa_enciphers: false,
        subject: SubjectShape::PersonCa, alternative: Alternative::None, limits: Some(Limits::NoNames) },
    Spec { profile: Profile::Pdf, name: "pdf", issuer: Authority::AccountCa, makes: None,
        days: SIGNING_DAYS, purposes: Purposes::Pdf, keys: SIGNING_KEYS, rsa_enciphers: false,
        subject: SubjectShape::Signer { organization: true }, alternative: Alternative::Email { required: false }, limits: None },
    Spec { profile: Profile::Code, name: "code", issuer: Authority::AccountCa, makes: None,
        days: SIGNING_DAYS, purposes: Purposes::Code, keys: SIGNING_KEYS, rsa_enciphers: false,
        subject: SubjectShape::Signer { organization: true }, alternative: Alternative::EmailOrWeb, limits: None },
    // Its validity is not in the plan's table: the same as its siblings under this CA
    Spec { profile: Profile::Smime, name: "smime", issuer: Authority::AccountCa, makes: None,
        days: SIGNING_DAYS, purposes: Purposes::Smime, keys: SIGNING_KEYS, rsa_enciphers: true,
        subject: SubjectShape::Signer { organization: false }, alternative: Alternative::Email { required: true }, limits: None },
    Spec { profile: Profile::LocalTls, name: "local-tls", issuer: Authority::LocalServicesCa, makes: None,
        days: TLS_DAYS, purposes: Purposes::Server, keys: TLS_KEYS, rsa_enciphers: true,
        subject: SubjectShape::Fixed(LOCAL_SERVICE), alternative: Alternative::LocalNames, limits: None },
    // The two machine CAs' key and subject are not in the plan's table: as an account's CA, with the machine's number,
    // which a person comparing two of them needs as they do the root's. Their purposes cover their leaves, no more.
    Spec { profile: Profile::DeviceCa, name: "device-ca", issuer: Authority::Root, makes: Some(Authority::DeviceCa),
        days: CA_DAYS, purposes: Purposes::ClientAndServer, keys: CA_KEYS, rsa_enciphers: false,
        subject: SubjectShape::MachineCa(DEVICE_CA), alternative: Alternative::None, limits: Some(Limits::NoNames) },
    // The validity and keys of the two identity certificates are not in the plan's tables either: local TLS's. Its CA has no
    // names, so NSS, which reads a server certificate's CN as a host name, never takes this for a website, and it should not:
    // it proves which machine to whoever compares it, and OpenSSL verifies it for both purposes.
    Spec { profile: Profile::DeviceIdentity, name: "device-identity", issuer: Authority::DeviceCa, makes: None,
        days: TLS_DAYS, purposes: Purposes::ClientAndServer, keys: TLS_KEYS, rsa_enciphers: true,
        subject: SubjectShape::Machine, alternative: Alternative::None, limits: None },
    Spec { profile: Profile::ServiceLogin, name: "service-login", issuer: Authority::DeviceCa, makes: None,
        days: TLS_DAYS, purposes: Purposes::Client, keys: TLS_KEYS, rsa_enciphers: false,
        subject: SubjectShape::Login, alternative: Alternative::None, limits: None },
    Spec { profile: Profile::LocalServicesCa, name: "local-services-ca", issuer: Authority::Root, makes: Some(Authority::LocalServicesCa),
        days: CA_DAYS, purposes: Purposes::Server, keys: CA_KEYS, rsa_enciphers: false,
        subject: SubjectShape::MachineCa(LOCAL_CA), alternative: Alternative::None, limits: Some(Limits::LocalNames) },
];

impl Profile {
    /// The row a request names. Anything else is refused: a profile only ever comes from this table.
    pub fn parse(name: &str) -> Option<Profile> {
        SPECS.iter().find(|spec| spec.name == name).map(|spec| spec.profile)
    }

    pub(super) fn spec(self) -> &'static Spec {
        // Every profile has its row: `every_profile_has_its_row` holds the table to the enum
        SPECS.iter().find(|spec| spec.profile == self).unwrap_or(&SPECS[0])
    }

    pub fn name(self) -> &'static str {
        self.spec().name
    }

    /// The types of key the profile takes, the default first
    pub fn key_types(self) -> &'static [KeyType] {
        self.spec().keys
    }

    /// The CA this profile makes, if it makes one
    pub fn makes(self) -> Option<Authority> {
        self.spec().makes
    }

    /// The row of the profile that makes this CA, which says what its certificate has to look like
    pub(super) fn of_authority(authority: Authority) -> Option<&'static Spec> {
        SPECS.iter().find(|spec| spec.makes == Some(authority))
    }

    pub fn all() -> impl Iterator<Item = Profile> {
        SPECS.iter().map(|spec| spec.profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_profile_has_its_row() {
        for profile in [
            Profile::AccountCa,
            Profile::Pdf,
            Profile::Code,
            Profile::Smime,
            Profile::LocalTls,
            Profile::DeviceCa,
            Profile::DeviceIdentity,
            Profile::ServiceLogin,
            Profile::LocalServicesCa,
        ] {
            assert_eq!(profile.spec().profile, profile);
            assert_eq!(Profile::parse(profile.name()), Some(profile));
        }
        assert_eq!(Profile::parse("root"), None);
    }
}
