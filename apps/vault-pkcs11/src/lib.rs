//! A PKCS#11 module with the fewest entry points NSS needs to sign, and no key in it (plans/PLAN-vault.md, Signing
//! certificates). `pdfsig` loads it through an NSS database, finds each certificate the [`Backend`] lists with a private
//! key object beside it, and each signature comes back from the backend. Everything else a PKCS#11 module may do answers
//! `CKR_FUNCTION_NOT_SUPPORTED`.
//!
//! `pdfsig` always gets `-nssdir`: without it, it reads the first Firefox profile it finds and not the database that names
//! this module. The socket is wired in [`backend`]. NSS asks for `CKM_RSA_PKCS` over a DigestInfo and `CKM_ECDSA` over a bare digest, so
//! the module takes those apart and the backend only ever sees a digest and which one it is.

#![allow(non_snake_case, clippy::missing_safety_doc)]

use std::collections::HashMap;
use std::ffi::{c_ulong, c_void};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use openssl::ecdsa::EcdsaSig;
use openssl::pkey::Id;
use openssl::x509::X509;

type Rv = c_ulong;
type Ulong = c_ulong;

const CKR_OK: Rv = 0;
const CKR_HOST_MEMORY: Rv = 0x02;
const CKR_SLOT_ID_INVALID: Rv = 0x03;
const CKR_GENERAL_ERROR: Rv = 0x05;
const CKR_ARGUMENTS_BAD: Rv = 0x07;
const CKR_ATTRIBUTE_TYPE_INVALID: Rv = 0x12;
const CKR_DATA_LEN_RANGE: Rv = 0x21;
const CKR_FUNCTION_CANCELED: Rv = 0x50;
const CKR_FUNCTION_NOT_SUPPORTED: Rv = 0x54;
const CKR_KEY_HANDLE_INVALID: Rv = 0x60;
const CKR_MECHANISM_INVALID: Rv = 0x70;
const CKR_OBJECT_HANDLE_INVALID: Rv = 0x82;
const CKR_OPERATION_NOT_INITIALIZED: Rv = 0x91;
const CKR_SESSION_HANDLE_INVALID: Rv = 0xb3;
const CKR_BUFFER_TOO_SMALL: Rv = 0x150;
const CKR_CRYPTOKI_NOT_INITIALIZED: Rv = 0x190;
const CKR_CRYPTOKI_ALREADY_INITIALIZED: Rv = 0x191;

const CKF_TOKEN_PRESENT: Ulong = 0x1;
const CKF_WRITE_PROTECTED: Ulong = 0x2;
const CKF_TOKEN_INITIALIZED: Ulong = 0x400;
const CKF_SERIAL_SESSION: Ulong = 0x4;
const CKF_SIGN: Ulong = 0x800;
const CKF_EC_NAMED_CURVE: Ulong = 0x0080_0000;
const CKS_RO_PUBLIC_SESSION: Ulong = 0;
const UNAVAILABLE: Ulong = !0;

const CKO_CERTIFICATE: Ulong = 1;
const CKO_PRIVATE_KEY: Ulong = 3;
const CKK_RSA: Ulong = 0;
const CKK_EC: Ulong = 3;
const CKC_X_509: Ulong = 0;

const CKA_CLASS: Ulong = 0x000;
const CKA_TOKEN: Ulong = 0x001;
const CKA_PRIVATE: Ulong = 0x002;
const CKA_LABEL: Ulong = 0x003;
const CKA_VALUE: Ulong = 0x011;
const CKA_CERTIFICATE_TYPE: Ulong = 0x080;
const CKA_ISSUER: Ulong = 0x081;
const CKA_SERIAL_NUMBER: Ulong = 0x082;
const CKA_TRUSTED: Ulong = 0x086;
const CKA_KEY_TYPE: Ulong = 0x100;
const CKA_SUBJECT: Ulong = 0x101;
const CKA_ID: Ulong = 0x102;
const CKA_SENSITIVE: Ulong = 0x103;
const CKA_DECRYPT: Ulong = 0x105;
const CKA_UNWRAP: Ulong = 0x107;
const CKA_SIGN: Ulong = 0x108;
const CKA_SIGN_RECOVER: Ulong = 0x109;
const CKA_DERIVE: Ulong = 0x10c;
const CKA_MODULUS: Ulong = 0x120;
const CKA_PUBLIC_EXPONENT: Ulong = 0x122;
const CKA_EXTRACTABLE: Ulong = 0x162;
const CKA_EC_PARAMS: Ulong = 0x180;
const CKA_ALWAYS_AUTHENTICATE: Ulong = 0x202;

const CKM_RSA_PKCS: Ulong = 0x1;
const CKM_ECDSA: Ulong = 0x1041;

/// The two DigestInfo prefixes of RFC 8017, 9.2: the digest follows
const DIGEST_INFO_SHA256: [u8; 19] =
    [0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05, 0x00, 0x04, 0x20];
const DIGEST_INFO_SHA384: [u8; 19] =
    [0x30, 0x41, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x02, 0x05, 0x00, 0x04, 0x30];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Digest {
    Sha256,
    Sha384,
}

/// A certificate the person can sign with, and the id that names its key to the backend
pub struct Entry {
    pub id: Vec<u8>,
    pub label: String,
    pub cert: Vec<u8>,
    /// The CAs above it, which NSS finds here and puts in the signature, so that the database it runs on holds nothing
    pub chain: Vec<Vec<u8>>,
}

/// The vault is closed, or the person said no: NSS fails the signing
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused;

/// Where a signature comes from. The vault's socket, in the module that ships; nothing here holds a key.
pub trait Backend: Send + Sync {
    /// What can sign, read once when NSS loads the module
    fn entries(&self) -> Vec<Entry>;
    /// A digest signed with the key `id` names: PKCS#1 v1.5 for RSA, the DER `ECDSA-Sig-Value` for ECDSA
    fn sign(&self, id: &[u8], digest: Digest, value: &[u8]) -> Result<Vec<u8>, Refused>;
}

/// The backend the module uses: the service's socket, where each signature is the certificate's own to approve. It lists the PDF
/// certificates of the first vault, if it is open, with the item's id as the label, so that a name somebody typed never has to
/// survive a PKCS#11 URI. One renewed or past its dates is not offered.
#[cfg(not(feature = "test-backend"))]
fn backend() -> Box<dyn Backend> {
    use std::path::PathBuf;
    use std::time::Duration;

    use amethystora_vault_client::{exchange, SOCKET};
    use base64::Engine;
    use serde_json::{json, Value};

    /// A person has five minutes to answer the service's prompt, and it answers for them after that
    const PATIENCE: Duration = Duration::from_secs(6 * 60);
    const LISTING: Duration = Duration::from_secs(10);

    struct Socket(PathBuf);

    fn der(text: &Value) -> Option<Vec<u8>> {
        base64::engine::general_purpose::STANDARD.decode(text.as_str()?).ok()
    }

    impl Socket {
        fn ask(&self, request: &Value, wait: Duration) -> Option<Value> {
            exchange(&self.0, request, wait).ok().filter(|answer| answer["ok"] == json!(true))
        }
    }

    impl Backend for Socket {
        fn entries(&self) -> Vec<Entry> {
            let Some(answer) = self.ask(&json!({"v": 1, "op": "cert-list", "purpose": "pdf", "chain": true}), LISTING) else { return Vec::new() };
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs() as i64);
            let offered = |item: &&Value| item["meta"]["renewed_by"].is_null() && item["meta"]["not_after"].as_i64().is_some_and(|not_after| now < not_after);
            answer["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(offered)
                .filter_map(|item| {
                    let id = item["id"].as_str()?;
                    let chain = item["chain"].as_array()?.iter().map(der).collect::<Option<Vec<_>>>()?;
                    Some(Entry { id: id.as_bytes().to_vec(), label: id.to_string(), cert: der(&item["cert"])?, chain })
                })
                .collect()
        }

        fn sign(&self, id: &[u8], digest: Digest, value: &[u8]) -> Result<Vec<u8>, Refused> {
            let id = std::str::from_utf8(id).map_err(|_| Refused)?;
            let digest = if digest == Digest::Sha256 { "sha256" } else { "sha384" };
            let value = base64::engine::general_purpose::STANDARD.encode(value);
            let request = json!({"v": 1, "op": "sign-digest", "id": id, "purpose": "pdf", "digest": digest, "value": value});
            let answer = self.ask(&request, PATIENCE).ok_or(Refused)?;
            der(&answer["signature"]).ok_or(Refused)
        }
    }

    Box::new(Socket(std::env::var_os("AMETHYSTORA_VAULT_SOCKET").map_or_else(|| PathBuf::from(SOCKET), PathBuf::from)))
}

/// For the tests in `tests/`: a key from the folder `AMETHYSTORA_VAULT_PKCS11_TEST` names, counted in `signed`
#[cfg(feature = "test-backend")]
fn backend() -> Box<dyn Backend> {
    use amethystora_vault_core::keys::{KeyType, PrivateKey};
    use amethystora_vault_core::pki::{self, Purpose, Signer};

    struct Folder(std::path::PathBuf);
    impl Backend for Folder {
        fn entries(&self) -> Vec<Entry> {
            let cert = std::fs::read(self.0.join("cert.der")).unwrap_or_default();
            let chain = std::fs::read(self.0.join("chain.der")).map(|ca| vec![ca]).unwrap_or_default();
            vec![Entry { id: vec![1], label: "Ada Lovelace".into(), cert, chain }]
        }
        fn sign(&self, _: &[u8], digest: Digest, value: &[u8]) -> Result<Vec<u8>, Refused> {
            let bytes = std::fs::read(self.0.join("key.der")).map_err(|_| Refused)?;
            let name = std::fs::read_to_string(self.0.join("key.type")).map_err(|_| Refused)?;
            let key = PrivateKey::from_bytes(KeyType::parse(name.trim()).ok_or(Refused)?, &bytes).map_err(|_| Refused)?;
            let digest = if digest == Digest::Sha256 { pki::Digest::Sha256 } else { pki::Digest::Sha384 };
            // As the service does: the key signs only with its own leaf certificate, for a PDF, within its dates
            let cert = std::fs::read(self.0.join("cert.der")).map_err(|_| Refused)?;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_| Refused)?.as_secs() as i64;
            let signer = Signer::leaf(&key, &cert, Purpose::Pdf, now).map_err(|_| Refused)?;
            let signature = pki::sign_digest(&signer, digest, value).map_err(|_| Refused)?;
            let count = std::fs::read_to_string(self.0.join("signed")).unwrap_or_default().len();
            std::fs::write(self.0.join("signed"), "x".repeat(count + 1)).map_err(|_| Refused)?;
            Ok(signature)
        }
    }
    Box::new(Folder(std::env::var_os("AMETHYSTORA_VAULT_PKCS11_TEST").unwrap_or_default().into()))
}

/// One certificate as NSS sees it: two objects, a certificate and a private key that exists only as attributes
struct Object {
    /// What is needed to sign: its id, whether it is RSA, and how long a signature is
    id: Vec<u8>,
    rsa: bool,
    signature_length: usize,
    certificate: Vec<(Ulong, Vec<u8>)>,
    private_key: Vec<(Ulong, Vec<u8>)>,
}

fn number(value: Ulong) -> Vec<u8> {
    value.to_ne_bytes().to_vec()
}

fn truth(value: bool) -> Vec<u8> {
    vec![u8::from(value)]
}

fn certificate_of(der: &[u8], id: &[u8], label: &str) -> Option<Vec<(Ulong, Vec<u8>)>> {
    let cert = X509::from_der(der).ok()?;
    let mut serial = cert.serial_number().to_bn().ok()?.to_vec();
    if serial.first().is_some_and(|byte| byte & 0x80 != 0) {
        serial.insert(0, 0);
    }
    // One byte of length, in its short form: a longer serial is no certificate of the Vault's
    let mut serial_der = vec![0x02, u8::try_from(serial.len()).ok().filter(|length| *length < 0x80)?];
    serial_der.extend_from_slice(&serial);
    Some(vec![
        (CKA_CLASS, number(CKO_CERTIFICATE)),
        (CKA_TOKEN, truth(true)),
        (CKA_PRIVATE, truth(false)),
        (CKA_LABEL, label.as_bytes().to_vec()),
        (CKA_ID, id.to_vec()),
        (CKA_CERTIFICATE_TYPE, number(CKC_X_509)),
        (CKA_TRUSTED, truth(false)),
        (CKA_SUBJECT, cert.subject_name().to_der().ok()?),
        (CKA_ISSUER, cert.issuer_name().to_der().ok()?),
        (CKA_SERIAL_NUMBER, serial_der),
        (CKA_VALUE, der.to_vec()),
    ])
}

/// A CA above a certificate: a certificate object, with no key beside it. Its CKA_ID is empty, and an entry with an empty id is
/// never listed, so no key can ever share an id with a chain certificate, which NSS would pair with it and offer as a signer.
fn chain_object(der: &[u8]) -> Option<Object> {
    let certificate = certificate_of(der, &[], "Amethystora Vault CA")?;
    Some(Object { id: Vec::new(), rsa: false, signature_length: 0, certificate, private_key: Vec::new() })
}

fn object_of(entry: &Entry) -> Option<Object> {
    if entry.id.is_empty() {
        return None;
    }
    let cert = X509::from_der(&entry.cert).ok()?;
    let pkey = cert.public_key().ok()?;
    let subject = cert.subject_name().to_der().ok()?;
    let certificate = certificate_of(&entry.cert, &entry.id, &entry.label)?;
    let mut private_key = vec![
        (CKA_CLASS, number(CKO_PRIVATE_KEY)),
        (CKA_TOKEN, truth(true)),
        (CKA_PRIVATE, truth(false)),
        (CKA_LABEL, entry.label.as_bytes().to_vec()),
        (CKA_ID, entry.id.clone()),
        (CKA_SUBJECT, subject),
        (CKA_SIGN, truth(true)),
        (CKA_SIGN_RECOVER, truth(false)),
        (CKA_DECRYPT, truth(false)),
        (CKA_UNWRAP, truth(false)),
        (CKA_DERIVE, truth(false)),
        (CKA_SENSITIVE, truth(true)),
        (CKA_EXTRACTABLE, truth(false)),
        (CKA_ALWAYS_AUTHENTICATE, truth(false)),
    ];
    let (rsa, signature_length) = if pkey.id() == Id::RSA {
        let rsa = pkey.rsa().ok()?;
        private_key.push((CKA_KEY_TYPE, number(CKK_RSA)));
        private_key.push((CKA_MODULUS, rsa.n().to_vec()));
        private_key.push((CKA_PUBLIC_EXPONENT, rsa.e().to_vec()));
        (true, usize::try_from(rsa.size()).ok()?)
    } else if pkey.id() == Id::EC {
        let ec = pkey.ec_key().ok()?;
        let name = ec.group().curve_name()?;
        let parameters = name.short_name().ok()?;
        // The curve's identifier in DER, the only parameters a certificate from the Vault has
        let oid: &[u8] = match parameters {
            "prime256v1" => &[0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07],
            "secp384r1" => &[0x06, 0x05, 0x2b, 0x81, 0x04, 0x00, 0x22],
            _ => return None,
        };
        private_key.push((CKA_KEY_TYPE, number(CKK_EC)));
        private_key.push((CKA_EC_PARAMS, oid.to_vec()));
        (false, if parameters == "prime256v1" { 64 } else { 96 })
    } else {
        return None;
    };
    Some(Object { id: entry.id.clone(), rsa, signature_length, certificate, private_key })
}

#[derive(Default)]
struct Session {
    found: Vec<Ulong>,
    signing: Option<(usize, Ulong)>,
}

/// The objects of every entry the backend listed that this module can offer, each certificate with its key and the CAs above it
fn objects_of(entries: &[Entry]) -> Vec<Object> {
    let mut objects = Vec::new();
    for entry in entries {
        if let Some(object) = object_of(entry) {
            objects.push(object);
            objects.extend(entry.chain.iter().filter_map(|der| chain_object(der)));
        }
    }
    objects
}

#[derive(Default)]
struct State {
    objects: Vec<Object>,
    sessions: HashMap<Ulong, Session>,
}

/// Session handles, counted across every Initialize and Finalize: one a caller still holds from before never names a new one
static SESSIONS: AtomicUsize = AtomicUsize::new(0);

static STATE: Mutex<Option<State>> = Mutex::new(None);
static BACKEND: OnceLock<Box<dyn Backend>> = OnceLock::new();

fn state() -> MutexGuard<'static, Option<State>> {
    STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// Handles: 2 * index + 1 for a certificate, 2 * index + 2 for its private key

fn attributes(state: &State, handle: Ulong) -> Option<&[(Ulong, Vec<u8>)]> {
    let index = usize::try_from(handle.checked_sub(1)? / 2).ok()?;
    let object = state.objects.get(index)?;
    let attributes = if handle % 2 == 1 { &object.certificate } else { &object.private_key };
    (!attributes.is_empty()).then_some(attributes.as_slice())
}

fn pad<const N: usize>(text: &str) -> [u8; N] {
    let mut out = [b' '; N];
    let bytes = text.as_bytes();
    let count = bytes.len().min(N);
    out[..count].copy_from_slice(&bytes[..count]);
    out
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Version {
    major: u8,
    minor: u8,
}

const CRYPTOKI: Version = Version { major: 2, minor: 40 };
const OURS: Version = Version { major: 1, minor: 0 };

#[repr(C)]
pub struct Info {
    cryptoki_version: Version,
    manufacturer: [u8; 32],
    flags: Ulong,
    description: [u8; 32],
    library_version: Version,
}

#[repr(C)]
pub struct SlotInfo {
    description: [u8; 64],
    manufacturer: [u8; 32],
    flags: Ulong,
    hardware: Version,
    firmware: Version,
}

#[repr(C)]
pub struct TokenInfo {
    label: [u8; 32],
    manufacturer: [u8; 32],
    model: [u8; 16],
    serial: [u8; 16],
    flags: Ulong,
    counts: [Ulong; 10],
    hardware: Version,
    firmware: Version,
    time: [u8; 16],
}

#[repr(C)]
pub struct MechanismInfo {
    min_key: Ulong,
    max_key: Ulong,
    flags: Ulong,
}

#[repr(C)]
pub struct SessionInfo {
    slot: Ulong,
    state: Ulong,
    flags: Ulong,
    device_error: Ulong,
}

#[repr(C)]
pub struct Attribute {
    kind: Ulong,
    value: *mut u8,
    length: Ulong,
}

#[repr(C)]
pub struct Mechanism {
    kind: Ulong,
    parameter: *mut c_void,
    length: Ulong,
}

const SLOT: Ulong = 1;

/// Whichever entry point it is that NSS never needs
unsafe extern "C" fn not_supported() -> Rv {
    CKR_FUNCTION_NOT_SUPPORTED
}

unsafe extern "C" fn C_Initialize(_: *mut c_void) -> Rv {
    if state().is_some() {
        return CKR_CRYPTOKI_ALREADY_INITIALIZED;
    }
    let backend = BACKEND.get_or_init(backend);
    let objects = objects_of(&backend.entries());
    let mut guard = state();
    if guard.is_some() {
        return CKR_CRYPTOKI_ALREADY_INITIALIZED;
    }
    *guard = Some(State { objects, sessions: HashMap::new() });
    CKR_OK
}

unsafe extern "C" fn C_Finalize(_: *mut c_void) -> Rv {
    if state().take().is_none() {
        return CKR_CRYPTOKI_NOT_INITIALIZED;
    }
    CKR_OK
}

unsafe extern "C" fn C_GetInfo(info: *mut Info) -> Rv {
    if info.is_null() {
        return CKR_ARGUMENTS_BAD;
    }
    unsafe {
        info.write(Info {
            cryptoki_version: CRYPTOKI,
            manufacturer: pad("Amethystora"),
            flags: 0,
            description: pad("Amethystora Vault"),
            library_version: OURS,
        });
    }
    CKR_OK
}

unsafe extern "C" fn C_GetSlotList(_: u8, list: *mut Ulong, count: *mut Ulong) -> Rv {
    if count.is_null() {
        return CKR_ARGUMENTS_BAD;
    }
    unsafe {
        if !list.is_null() {
            if *count < 1 {
                *count = 1;
                return CKR_BUFFER_TOO_SMALL;
            }
            *list = SLOT;
        }
        *count = 1;
    }
    CKR_OK
}

unsafe extern "C" fn C_GetSlotInfo(slot: Ulong, info: *mut SlotInfo) -> Rv {
    if slot != SLOT {
        return CKR_SLOT_ID_INVALID;
    }
    if info.is_null() {
        return CKR_ARGUMENTS_BAD;
    }
    unsafe {
        info.write(SlotInfo {
            description: pad("Amethystora Vault"),
            manufacturer: pad("Amethystora"),
            flags: CKF_TOKEN_PRESENT,
            hardware: OURS,
            firmware: OURS,
        });
    }
    CKR_OK
}

unsafe extern "C" fn C_GetTokenInfo(slot: Ulong, info: *mut TokenInfo) -> Rv {
    if slot != SLOT {
        return CKR_SLOT_ID_INVALID;
    }
    if info.is_null() {
        return CKR_ARGUMENTS_BAD;
    }
    unsafe {
        info.write(TokenInfo {
            label: pad("Amethystora Vault"),
            manufacturer: pad("Amethystora"),
            model: pad("Vault"),
            serial: pad("1"),
            // No login: the person approves each signature in the Vault's own prompt, which NSS cannot answer
            flags: CKF_TOKEN_INITIALIZED | CKF_WRITE_PROTECTED,
            counts: [UNAVAILABLE; 10],
            hardware: OURS,
            firmware: OURS,
            time: [b' '; 16],
        });
    }
    CKR_OK
}

unsafe extern "C" fn C_GetMechanismList(slot: Ulong, list: *mut Ulong, count: *mut Ulong) -> Rv {
    if slot != SLOT {
        return CKR_SLOT_ID_INVALID;
    }
    if count.is_null() {
        return CKR_ARGUMENTS_BAD;
    }
    unsafe {
        if !list.is_null() {
            if *count < 2 {
                *count = 2;
                return CKR_BUFFER_TOO_SMALL;
            }
            *list = CKM_RSA_PKCS;
            *list.add(1) = CKM_ECDSA;
        }
        *count = 2;
    }
    CKR_OK
}

unsafe extern "C" fn C_GetMechanismInfo(slot: Ulong, kind: Ulong, info: *mut MechanismInfo) -> Rv {
    if slot != SLOT {
        return CKR_SLOT_ID_INVALID;
    }
    if info.is_null() {
        return CKR_ARGUMENTS_BAD;
    }
    let value = match kind {
        CKM_RSA_PKCS => MechanismInfo { min_key: 2048, max_key: 4096, flags: CKF_SIGN },
        CKM_ECDSA => MechanismInfo { min_key: 256, max_key: 384, flags: CKF_SIGN | CKF_EC_NAMED_CURVE },
        _ => return CKR_MECHANISM_INVALID,
    };
    unsafe { info.write(value) };
    CKR_OK
}

unsafe extern "C" fn C_OpenSession(slot: Ulong, _: Ulong, _: *mut c_void, _: *mut c_void, session: *mut Ulong) -> Rv {
    if slot != SLOT {
        return CKR_SLOT_ID_INVALID;
    }
    if session.is_null() {
        return CKR_ARGUMENTS_BAD;
    }
    let mut guard = state();
    let Some(state) = guard.as_mut() else {
        return CKR_CRYPTOKI_NOT_INITIALIZED;
    };
    let handle = SESSIONS.fetch_add(1, Ordering::Relaxed) as Ulong + 1;
    state.sessions.insert(handle, Session::default());
    unsafe { *session = handle };
    CKR_OK
}

unsafe extern "C" fn C_CloseSession(session: Ulong) -> Rv {
    if let Some(state) = state().as_mut() {
        state.sessions.remove(&session);
    }
    CKR_OK
}

unsafe extern "C" fn C_CloseAllSessions(_: Ulong) -> Rv {
    if let Some(state) = state().as_mut() {
        state.sessions.clear();
    }
    CKR_OK
}

unsafe extern "C" fn C_GetSessionInfo(session: Ulong, info: *mut SessionInfo) -> Rv {
    let guard = state();
    let Some(state) = guard.as_ref() else {
        return CKR_CRYPTOKI_NOT_INITIALIZED;
    };
    if !state.sessions.contains_key(&session) {
        return CKR_SESSION_HANDLE_INVALID;
    }
    if info.is_null() {
        return CKR_ARGUMENTS_BAD;
    }
    unsafe { info.write(SessionInfo { slot: SLOT, state: CKS_RO_PUBLIC_SESSION, flags: CKF_SERIAL_SESSION, device_error: 0 }) };
    CKR_OK
}

unsafe extern "C" fn C_Login(_: Ulong, _: Ulong, _: *const u8, _: Ulong) -> Rv {
    CKR_OK
}

unsafe extern "C" fn C_Logout(_: Ulong) -> Rv {
    CKR_OK
}

unsafe extern "C" fn C_GetAttributeValue(session: Ulong, handle: Ulong, template: *mut Attribute, count: Ulong) -> Rv {
    let guard = state();
    let Some(state) = guard.as_ref() else {
        return CKR_CRYPTOKI_NOT_INITIALIZED;
    };
    if !state.sessions.contains_key(&session) {
        return CKR_SESSION_HANDLE_INVALID;
    }
    let Some(known) = attributes(state, handle) else {
        return CKR_OBJECT_HANDLE_INVALID;
    };
    if template.is_null() {
        return CKR_ARGUMENTS_BAD;
    }
    let mut result = CKR_OK;
    for index in 0..count as usize {
        let attribute = unsafe { &mut *template.add(index) };
        match known.iter().find(|(kind, _)| *kind == attribute.kind) {
            None => {
                attribute.length = UNAVAILABLE;
                result = CKR_ATTRIBUTE_TYPE_INVALID;
            }
            Some((_, value)) => {
                if !attribute.value.is_null() {
                    if (attribute.length as usize) < value.len() {
                        attribute.length = UNAVAILABLE;
                        result = CKR_BUFFER_TOO_SMALL;
                        continue;
                    }
                    unsafe { std::ptr::copy_nonoverlapping(value.as_ptr(), attribute.value, value.len()) };
                }
                attribute.length = value.len() as Ulong;
            }
        }
    }
    result
}

unsafe extern "C" fn C_FindObjectsInit(session: Ulong, template: *const Attribute, count: Ulong) -> Rv {
    let mut guard = state();
    let Some(state) = guard.as_mut() else {
        return CKR_CRYPTOKI_NOT_INITIALIZED;
    };
    if !state.sessions.contains_key(&session) {
        return CKR_SESSION_HANDLE_INVALID;
    }
    if template.is_null() && count != 0 {
        return CKR_ARGUMENTS_BAD;
    }
    let wanted: Vec<(Ulong, &[u8])> = (0..count as usize)
        .map(|index| {
            let attribute = unsafe { &*template.add(index) };
            let value: &[u8] = if attribute.value.is_null() {
                &[]
            } else {
                unsafe { std::slice::from_raw_parts(attribute.value, attribute.length as usize) }
            };
            (attribute.kind, value)
        })
        .collect();
    let found: Vec<Ulong> = (1..=state.objects.len() as Ulong * 2)
        .filter(|handle| {
            attributes(state, *handle).is_some_and(|known| {
                wanted.iter().all(|(kind, value)| known.iter().any(|(other, have)| other == kind && have.as_slice() == *value))
            })
        })
        .collect();
    if let Some(session) = state.sessions.get_mut(&session) {
        session.found = found;
    }
    CKR_OK
}

unsafe extern "C" fn C_FindObjects(session: Ulong, out: *mut Ulong, room: Ulong, count: *mut Ulong) -> Rv {
    let mut guard = state();
    let Some(state) = guard.as_mut() else {
        return CKR_CRYPTOKI_NOT_INITIALIZED;
    };
    let Some(session) = state.sessions.get_mut(&session) else {
        return CKR_SESSION_HANDLE_INVALID;
    };
    if out.is_null() || count.is_null() {
        return CKR_ARGUMENTS_BAD;
    }
    let take = session.found.len().min(room as usize);
    for (index, handle) in session.found.drain(..take).enumerate() {
        unsafe { *out.add(index) = handle };
    }
    unsafe { *count = take as Ulong };
    CKR_OK
}

unsafe extern "C" fn C_FindObjectsFinal(session: Ulong) -> Rv {
    if let Some(session) = state().as_mut().and_then(|state| state.sessions.get_mut(&session)) {
        session.found.clear();
    }
    CKR_OK
}

unsafe extern "C" fn C_SignInit(session: Ulong, mechanism: *const Mechanism, key: Ulong) -> Rv {
    let mut guard = state();
    let Some(state) = guard.as_mut() else {
        return CKR_CRYPTOKI_NOT_INITIALIZED;
    };
    if mechanism.is_null() {
        return CKR_ARGUMENTS_BAD;
    }
    let kind = unsafe { (*mechanism).kind };
    if key == 0 || !key.is_multiple_of(2) {
        return CKR_KEY_HANDLE_INVALID;
    }
    let index = (key / 2 - 1) as usize;
    let Some(object) = state.objects.get(index).filter(|object| !object.private_key.is_empty()) else {
        return CKR_KEY_HANDLE_INVALID;
    };
    if kind != if object.rsa { CKM_RSA_PKCS } else { CKM_ECDSA } {
        return CKR_MECHANISM_INVALID;
    }
    let Some(session) = state.sessions.get_mut(&session) else {
        return CKR_SESSION_HANDLE_INVALID;
    };
    session.signing = Some((index, kind));
    CKR_OK
}

/// The digest inside what NSS hands over: a DigestInfo for RSA, or the digest itself for ECDSA
fn digest_of(rsa: bool, data: &[u8]) -> Option<(Digest, &[u8])> {
    if rsa {
        if let Some(value) = data.strip_prefix(&DIGEST_INFO_SHA256) {
            return (value.len() == 32).then_some((Digest::Sha256, value));
        }
        let value = data.strip_prefix(&DIGEST_INFO_SHA384)?;
        return (value.len() == 48).then_some((Digest::Sha384, value));
    }
    match data.len() {
        32 => Some((Digest::Sha256, data)),
        48 => Some((Digest::Sha384, data)),
        _ => None,
    }
}

/// CKM_ECDSA's result is r and s side by side, each the curve's length
fn raw_ecdsa(der: &[u8], length: usize) -> Option<Vec<u8>> {
    let signature = EcdsaSig::from_der(der).ok()?;
    let half = i32::try_from(length / 2).ok()?;
    let mut out = signature.r().to_vec_padded(half).ok()?;
    out.extend(signature.s().to_vec_padded(half).ok()?);
    Some(out)
}

unsafe extern "C" fn C_Sign(session: Ulong, data: *const u8, length: Ulong, out: *mut u8, out_length: *mut Ulong) -> Rv {
    if out_length.is_null() || data.is_null() {
        return CKR_ARGUMENTS_BAD;
    }
    // The lock is let go while the backend works: the person may take a while to approve
    let (id, rsa, signature_length) = {
        let mut guard = state();
        let Some(state) = guard.as_mut() else {
            return CKR_CRYPTOKI_NOT_INITIALIZED;
        };
        let Some(session_state) = state.sessions.get(&session) else {
            return CKR_SESSION_HANDLE_INVALID;
        };
        let Some((index, _)) = session_state.signing else {
            return CKR_OPERATION_NOT_INITIALIZED;
        };
        let Some(object) = state.objects.get(index) else {
            return CKR_OPERATION_NOT_INITIALIZED;
        };
        let wanted = object.signature_length;
        // A question about the length, not yet a signature, keeps the operation open
        if out.is_null() {
            unsafe { *out_length = wanted as Ulong };
            return CKR_OK;
        }
        if unsafe { (*out_length as usize) < wanted } {
            unsafe { *out_length = wanted as Ulong };
            return CKR_BUFFER_TOO_SMALL;
        }
        let details = (object.id.clone(), object.rsa, wanted);
        if let Some(session_state) = state.sessions.get_mut(&session) {
            session_state.signing = None;
        }
        details
    };
    let data = unsafe { std::slice::from_raw_parts(data, length as usize) };
    let Some((digest, value)) = digest_of(rsa, data) else {
        return CKR_DATA_LEN_RANGE;
    };
    let Some(backend) = BACKEND.get() else {
        return CKR_GENERAL_ERROR;
    };
    let Ok(signature) = backend.sign(&id, digest, value) else {
        return CKR_FUNCTION_CANCELED;
    };
    let signature = if rsa { Some(signature) } else { raw_ecdsa(&signature, signature_length) };
    let Some(signature) = signature else {
        return CKR_GENERAL_ERROR;
    };
    if signature.len() != signature_length {
        return CKR_HOST_MEMORY;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(signature.as_ptr(), out, signature.len());
        *out_length = signature.len() as Ulong;
    }
    CKR_OK
}

type Function = unsafe extern "C" fn() -> Rv;

/// CK_FUNCTION_LIST: the version, then the 68 entry points of PKCS#11 2.40 in their order, each a pointer
#[repr(C)]
pub struct FunctionList {
    version: Version,
    functions: [Function; 68],
}

macro_rules! entry {
    ($list:ident, $index:expr, $function:ident) => {
        // Every entry point goes in the table under one type: NSS calls each through its proper signature
        $list[$index] = unsafe { std::mem::transmute::<*const (), Function>($function as *const ()) };
    };
}

fn table() -> [Function; 68] {
    let mut list: [Function; 68] = [not_supported; 68];
    entry!(list, 0, C_Initialize);
    entry!(list, 1, C_Finalize);
    entry!(list, 2, C_GetInfo);
    entry!(list, 3, C_GetFunctionList);
    entry!(list, 4, C_GetSlotList);
    entry!(list, 5, C_GetSlotInfo);
    entry!(list, 6, C_GetTokenInfo);
    entry!(list, 7, C_GetMechanismList);
    entry!(list, 8, C_GetMechanismInfo);
    entry!(list, 12, C_OpenSession);
    entry!(list, 13, C_CloseSession);
    entry!(list, 14, C_CloseAllSessions);
    entry!(list, 15, C_GetSessionInfo);
    entry!(list, 18, C_Login);
    entry!(list, 19, C_Logout);
    entry!(list, 24, C_GetAttributeValue);
    entry!(list, 26, C_FindObjectsInit);
    entry!(list, 27, C_FindObjects);
    entry!(list, 28, C_FindObjectsFinal);
    entry!(list, 42, C_SignInit);
    entry!(list, 43, C_Sign);
    list
}

static LIST: OnceLock<FunctionList> = OnceLock::new();

/// The module's one exported symbol
#[unsafe(no_mangle)]
pub unsafe extern "C" fn C_GetFunctionList(out: *mut *const FunctionList) -> Rv {
    if out.is_null() {
        return CKR_ARGUMENTS_BAD;
    }
    let list = LIST.get_or_init(|| FunctionList { version: CRYPTOKI, functions: table() });
    unsafe { *out = list };
    CKR_OK
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::asn1::Asn1Time;
    use openssl::bn::BigNum;
    use openssl::ec::{EcGroup, EcKey};
    use openssl::hash::MessageDigest;
    use openssl::nid::Nid;
    use openssl::pkey::PKey;
    use openssl::x509::X509NameBuilder;

    /// A certificate of no consequence, with a serial of the length asked for
    fn certificate(serial_bytes: usize) -> Vec<u8> {
        let key = PKey::from_ec_key(EcKey::generate(&EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap()).unwrap()).unwrap();
        let mut name = X509NameBuilder::new().unwrap();
        name.append_entry_by_text("CN", "Test").unwrap();
        let name = name.build();
        let mut builder = X509::builder().unwrap();
        builder.set_version(2).unwrap();
        let mut serial = vec![0x41u8; serial_bytes];
        serial[0] = 0x41;
        builder.set_serial_number(&BigNum::from_slice(&serial).unwrap().to_asn1_integer().unwrap()).unwrap();
        builder.set_subject_name(&name).unwrap();
        builder.set_issuer_name(&name).unwrap();
        builder.set_not_before(&Asn1Time::days_from_now(0).unwrap()).unwrap();
        builder.set_not_after(&Asn1Time::days_from_now(1).unwrap()).unwrap();
        builder.set_pubkey(&key).unwrap();
        builder.sign(&key, MessageDigest::sha256()).unwrap();
        builder.build().to_der().unwrap()
    }

    fn id_of(attributes: &[(Ulong, Vec<u8>)]) -> Option<&Vec<u8>> {
        attributes.iter().find(|(kind, _)| *kind == CKA_ID).map(|(_, value)| value)
    }

    /// A chain certificate can never be paired with a key: its id is empty, and no key is listed with an empty id
    #[test]
    fn no_chain_certificate_shares_an_id_with_a_key() {
        let ca = certificate(16);
        let entry = |id: &[u8]| Entry { id: id.to_vec(), label: "x".into(), cert: certificate(16), chain: vec![ca.clone()] };
        // The ids the module used to give a chain, one that is empty, and an ordinary one
        let objects = objects_of(&[entry(&[0xca, 0]), entry(&[]), entry(&[1, 2, 3])]);
        let keys: Vec<&Vec<u8>> = objects.iter().filter(|object| !object.private_key.is_empty()).filter_map(|object| id_of(&object.private_key)).collect();
        assert_eq!(keys, [&vec![0xca, 0], &vec![1, 2, 3]], "an entry with no id is not listed");
        let chain: Vec<&Vec<u8>> = objects.iter().filter(|object| object.private_key.is_empty()).filter_map(|object| id_of(&object.certificate)).collect();
        assert_eq!(chain.len(), 2, "one chain certificate for each listed entry");
        assert!(chain.iter().all(|id| id.is_empty() && !keys.contains(id)));
    }

    #[test]
    fn a_serial_of_a_hundred_and_twenty_eight_bytes_is_not_listed() {
        let der = certificate(127);
        assert!(chain_object(&der).is_some());
        let der = certificate(128);
        assert!(chain_object(&der).is_none());
    }

    /// L7 of the review: a second Initialize and a second Finalize are refused, and a handle from before is never handed out again
    #[test]
    fn initialize_and_finalize_say_when_they_are_wrong() {
        static ONE: Mutex<()> = Mutex::new(());
        let _one = ONE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        unsafe {
            let _ = C_Finalize(std::ptr::null_mut());
            assert_eq!(C_Finalize(std::ptr::null_mut()), CKR_CRYPTOKI_NOT_INITIALIZED);
            assert_eq!(C_Initialize(std::ptr::null_mut()), CKR_OK);
            assert_eq!(C_Initialize(std::ptr::null_mut()), CKR_CRYPTOKI_ALREADY_INITIALIZED);
            let (mut old, mut new) = (0, 0);
            assert_eq!(C_OpenSession(SLOT, 0, std::ptr::null_mut(), std::ptr::null_mut(), &mut old), CKR_OK);
            assert_eq!(C_Finalize(std::ptr::null_mut()), CKR_OK);
            assert_eq!(C_Finalize(std::ptr::null_mut()), CKR_CRYPTOKI_NOT_INITIALIZED);
            assert_eq!(C_Initialize(std::ptr::null_mut()), CKR_OK);
            assert_eq!(C_OpenSession(SLOT, 0, std::ptr::null_mut(), std::ptr::null_mut(), &mut new), CKR_OK);
            assert_ne!(old, new);
            let mut info = std::mem::MaybeUninit::<SessionInfo>::uninit();
            assert_eq!(C_GetSessionInfo(old, info.as_mut_ptr()), CKR_SESSION_HANDLE_INVALID, "the old handle names nothing now");
            assert_eq!(C_Finalize(std::ptr::null_mut()), CKR_OK);
        }
    }
}
