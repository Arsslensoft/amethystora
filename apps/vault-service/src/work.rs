//! amethystora-vault-work: one job of the Vault's that takes libcrypto, then gone. The service starts it from its work thread, under
//! the same unit, with the job as one line of JSON on standard input and the answer as one on standard output (`plugs/work.rs`).
//! libcrypto and its providers cost more than the service's whole memory budget, and its parsers have no place in the process that
//! holds every vault's key, so the service itself never loads it. Everything it was given goes with it.

use std::io::{Read, Write};
use std::process::ExitCode;

use base64::Engine;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use amethystora_vault_core::bmc;
use amethystora_vault_core::items::{Account, BackupService};
use amethystora_vault_core::keys::{self, KeyError, KeyKind, KeyType, PrivateKey, PublicForms};
use amethystora_vault_core::pki::{self, Authority, CaKey, Digest, Issuer, PkiError, Profile, Purpose, Request, Signer};
use amethystora_vault_core::protocol::encode;
// The allocator that wipes what it is given back
use amethystora_vault_service as _;

/// More than everything a vault can hold going to an export
const MOST_IN: u64 = 512 * 1024 * 1024;
/// RSA 4096 takes seconds and an export one scrypt: this is only so that nothing waits on a job for ever
const PATIENCE_SECONDS: u32 = 300;

/// A stable word and a field for the window, or plainly a failure. Never what was typed.
enum Why {
    Problem(&'static str, Option<&'static str>),
    Bad,
    Failed,
}

impl From<PkiError> for Why {
    fn from(error: PkiError) -> Why {
        match error {
            PkiError::UnknownProfile => Why::Problem("unknown-profile", None),
            PkiError::WrongIssuer => Why::Problem("wrong-issuer", None),
            PkiError::KeyNotAllowed => Why::Problem("key-not-allowed", None),
            PkiError::BadField(field) => Why::Problem("bad-field", Some(field)),
            PkiError::MissingField(field) => Why::Problem("missing-field", Some(field)),
            PkiError::NotInProfile(field) => Why::Problem("not-in-profile", Some(field)),
            PkiError::OutsideLimits => Why::Problem("outside-limits", None),
            PkiError::NameLooksLikeHost => Why::Problem("name-looks-like-host", None),
            PkiError::DigestNotAllowed => Why::Problem("digest-not-allowed", None),
            PkiError::BadDigest => Why::Problem("bad-digest", None),
            PkiError::BadPassword => Why::Problem("bad-password", None),
            PkiError::WrongCertificate => Why::Problem("wrong-purpose", None),
            PkiError::CertificateNotValid => Why::Problem("expired", None),
            PkiError::IssuerNotValid | PkiError::IssuerKeyMismatch | PkiError::KeyIsACas | PkiError::BadTime | PkiError::Malformed | PkiError::Failed => Why::Failed,
            PkiError::Key(error) => error.into(),
        }
    }
}

impl From<KeyError> for Why {
    fn from(error: KeyError) -> Why {
        match error {
            KeyError::Unavailable | KeyError::Unsupported | KeyError::NotForThisType => Why::Problem("key-not-allowed", None),
            KeyError::BadPassphrase | KeyError::WrongPassphrase => Why::Problem("bad-password", None),
            KeyError::Malformed => Why::Problem("malformed", None),
            KeyError::TooCostly => Why::Problem("too-costly", None),
            KeyError::Failed => Why::Failed,
        }
    }
}

type Answer = Result<Value, Why>;

fn text<'a>(request: &'a Value, name: &str) -> Result<&'a str, Why> {
    request.get(name).and_then(Value::as_str).ok_or(Why::Bad)
}

fn optional<'a>(request: &'a Value, name: &str) -> Option<&'a str> {
    request.get(name).and_then(Value::as_str)
}

fn bytes(request: &Value, name: &str) -> Result<Zeroizing<Vec<u8>>, Why> {
    base64::engine::general_purpose::STANDARD.decode(text(request, name)?).map(Zeroizing::new).map_err(|_| Why::Bad)
}

fn key_type(request: &Value) -> Result<KeyType, Why> {
    KeyType::parse(text(request, "type")?).ok_or(Why::Problem("key-not-allowed", None))
}

fn held_key(request: &Value) -> Result<PrivateKey, Why> {
    Ok(PrivateKey::from_bytes(key_type(request)?, &bytes(request, "key")?)?)
}

fn public_part(public: &PublicForms) -> Value {
    json!({"type": public.key_type.name(), "pem": public.spki_pem, "openssh": public.openssh, "sha256": public.sha256, "ssh_sha256": public.ssh_sha256})
}

fn kind_of(name: &str) -> Result<KeyKind, Why> {
    match name {
        "ssh" => Ok(KeyKind::Ssh),
        "signing" => Ok(KeyKind::Signing),
        "exchange" => Ok(KeyKind::Exchange),
        "symmetric" => Ok(KeyKind::Symmetric),
        _ => Err(Why::Bad),
    }
}

fn names(kind: KeyKind) -> Vec<&'static str> {
    kind.types().into_iter().map(KeyType::name).collect()
}

fn key_types() -> Answer {
    Ok(json!({"ssh": names(KeyKind::Ssh), "signing": names(KeyKind::Signing), "exchange": names(KeyKind::Exchange), "symmetric": names(KeyKind::Symmetric)}))
}

/// A new key of a kind's type: what comes back is its private part for the service to seal, and its public forms
fn made_key(key_kind: &str, key: &PrivateKey) -> Answer {
    let public = if key.key_type() == KeyType::Aes256 { Value::Null } else { public_part(&key.public()?) };
    Ok(json!({"key_kind": key_kind, "type": key.key_type().name(), "key": encode(key.as_bytes()), "public": public}))
}

fn key_new(request: &Value) -> Answer {
    let key_kind = text(request, "key_kind")?;
    let offered = kind_of(key_kind)?.types();
    let chosen = match optional(request, "type") {
        Some(name) => KeyType::parse(name).filter(|chosen| offered.contains(chosen)).ok_or(Why::Problem("key-not-allowed", None))?,
        None => *offered.first().ok_or(Why::Problem("key-not-allowed", None))?,
    };
    made_key(key_kind, &keys::generate(chosen)?)
}

/// A finished key from a client: it has to read as that type, and an SSH key has to have an SSH form
fn key_check(request: &Value) -> Answer {
    let key_kind = text(request, "key_kind")?;
    kind_of(key_kind)?;
    let key = PrivateKey::from_bytes(key_type(request)?, &bytes(request, "private")?)?;
    if key.key_type() != KeyType::Aes256 {
        let public = key.public()?;
        if !halves_agree(&key, &public.spki_der) {
            return Err(Why::Problem("malformed", None));
        }
        // Past RSA's sizes anywhere: a key that large only costs whoever uses it later
        if public.spki_der.len() > 4096 || (key_kind == "ssh" && public.openssh.is_none()) {
            return Err(Why::Problem("key-not-allowed", None));
        }
    } else if key_kind == "ssh" {
        return Err(Why::Problem("key-not-allowed", None));
    }
    made_key(key_kind, &key)
}

fn key_export(request: &Value) -> Answer {
    let key = held_key(request)?;
    let data = match optional(request, "passphrase") {
        Some(passphrase) => key.export_encrypted(passphrase.as_bytes())?,
        None if request["plain"] == json!(true) => key.export_plain()?.to_vec(),
        None => return Err(Why::Bad),
    };
    Ok(json!({"data": encode(&data)}))
}

fn certificates(request: &Value, name: &str) -> Result<Vec<Vec<u8>>, Why> {
    request[name].as_array().ok_or(Why::Bad)?.iter().map(|one| one.as_str().ok_or(Why::Bad).and_then(|one| base64::engine::general_purpose::STANDARD.decode(one).map_err(|_| Why::Bad))).collect()
}

fn pfx(request: &Value) -> Answer {
    let key = held_key(request)?;
    let cert = bytes(request, "cert")?;
    let made = pki::pfx(&key, &cert, &certificates(request, "chain")?, text(request, "name")?, text(request, "password")?)?;
    Ok(json!({"data": encode(&made)}))
}

fn digest_of(request: &Value) -> Result<Option<Digest>, Why> {
    match optional(request, "digest") {
        None => Ok(None),
        Some("sha256") => Ok(Some(Digest::Sha256)),
        Some("sha384") => Ok(Some(Digest::Sha384)),
        Some(_) => Err(Why::Problem("digest-not-allowed", None)),
    }
}

/// A private key whose public half is not the one it says, which a PEM put together by hand can be: a test message signed with
/// the private half has to verify under the public. A key that cannot sign (X25519, ML-KEM) has nothing to prove here.
fn halves_agree(key: &PrivateKey, spki: &[u8]) -> bool {
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

/// A leaf certificate's own signature over a digest. The service has already held the item to its profile and dates; `Signer::leaf`
/// holds the key to the certificate it came with: CA:FALSE, the purpose's profile, within its dates, and the key's own. A CA's or the
/// root's key is a type (`CaKey`) that cannot be given here at all.
fn sign(request: &Value) -> Answer {
    let key = held_key(request)?;
    let digest = digest_of(request)?.ok_or(Why::Bad)?;
    let purpose = match text(request, "purpose")? {
        "pdf" => Purpose::Pdf,
        "code" => Purpose::Code,
        _ => return Err(Why::Problem("wrong-purpose", None)),
    };
    let now = request["now"].as_i64().ok_or(Why::Bad)?;
    let signer = Signer::leaf(&key, &bytes(request, "cert")?, purpose, now)?;
    Ok(json!({"signature": encode(&pki::sign_digest(&signer, digest, &bytes(request, "value")?)?)}))
}

/// For the SSH agent: a whole message, as its algorithm takes one
fn sign_message(request: &Value) -> Answer {
    let key = held_key(request)?;
    Ok(json!({"signature": encode(&keys::sign_message(&key, text(request, "digest")?, &bytes(request, "data")?)?)}))
}

/// What a request's fields come to, the strings borrowed from it
fn fields<'a>(request: &'a Value, hosts: &'a [&'a str], public_key: &'a [u8], now: i64) -> Result<Request<'a>, Why> {
    Ok(Request {
        profile: text(request, "profile")?,
        public_key,
        digest: digest_of(request)?,
        now,
        name: optional(request, "name"),
        organization: optional(request, "organization"),
        country: optional(request, "country"),
        login: optional(request, "login"),
        email: optional(request, "email"),
        web: optional(request, "web"),
        local_names: hosts,
    })
}

fn hosts_of(request: &Value) -> Result<Vec<&str>, Why> {
    match request.get("hosts") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(list)) => list.iter().map(|host| host.as_str().ok_or(Why::Bad)).collect(),
        Some(_) => Err(Why::Bad),
    }
}

fn leaf_key_type(request: &Value, profile: Profile) -> Result<KeyType, Why> {
    match optional(request, "key_type") {
        Some(name) => KeyType::parse(name).filter(|chosen| profile.key_types().contains(chosen)).ok_or(Why::Problem("key-not-allowed", None)),
        None => profile.key_types().first().copied().ok_or(Why::Problem("key-not-allowed", None)),
    }
}

/// Everything `issue` would refuse of these fields, before anybody is asked and before a root is made
fn issue_check(request: &Value) -> Answer {
    let profile = Profile::parse(text(request, "profile")?).ok_or(Why::Problem("unknown-profile", None))?;
    if profile.makes().is_some() {
        return Err(Why::Problem("unknown-profile", None));
    }
    let chosen = leaf_key_type(request, profile)?;
    let hosts = hosts_of(request)?;
    pki::check(&fields(request, &hosts, &[], 0)?, Some(chosen))?;
    Ok(json!({}))
}

fn description(cert: &[u8]) -> Result<Value, Why> {
    let told = pki::describe(cert)?;
    Ok(json!({"subject": told.subject, "issuer": told.issuer, "not_before": told.not_before, "not_after": told.not_after, "sha256": told.sha256, "spki_sha256": told.key.sha256}))
}

/// A CA's or the root's key as the service keeps it: base64 of `CaKey::to_stored`, which never reads back as a leaf's key
fn held_ca(request: &Value) -> Result<CaKey, Why> {
    Ok(CaKey::from_stored(&bytes(request, "key")?)?)
}

fn make_root(request: &Value) -> Answer {
    let now = request["now"].as_i64().ok_or(Why::Bad)?;
    let key = CaKey::generate(Authority::Root)?;
    let cert = pki::make_root(&key, now)?;
    let public = key.public()?;
    Ok(json!({"key": encode(&key.to_stored()), "cert": encode(&cert), "type": public.key_type.name(), "number": pki::machine_number(&public.spki_der), "description": description(&cert)?}))
}

fn authority_of(profile: Profile) -> Result<Authority, Why> {
    match profile.name() {
        "pdf" | "code" | "smime" => Ok(Authority::AccountCa),
        "local-tls" => Ok(Authority::LocalServicesCa),
        "device-identity" | "service-login" => Ok(Authority::DeviceCa),
        _ => Err(Why::Problem("unknown-profile", None)),
    }
}

/// A CA under the root, which comes with its key for this one signature
fn make_ca(request: &Value) -> Answer {
    let now = request["now"].as_i64().ok_or(Why::Bad)?;
    let profile = Profile::parse(text(request, "profile")?).ok_or(Why::Problem("unknown-profile", None))?;
    let authority = profile.makes().ok_or(Why::Problem("unknown-profile", None))?;
    let key = CaKey::generate(authority)?;
    let public = key.public()?;
    let root = &request["root"];
    let root_cert = bytes(root, "cert")?;
    let root_key = held_ca(root)?;
    let issuer = Issuer { authority: Authority::Root, cert: &root_cert, key: &root_key, chain: &[] };
    let hosts = Vec::new();
    let made = pki::issue(&fields(request, &hosts, &public.spki_der, now)?, &issuer)?;
    Ok(json!({"key": encode(&key.to_stored()), "cert": encode(&made.cert), "type": public.key_type.name(), "description": description(&made.cert)?}))
}

/// A new key and its certificate, under the CA given
fn issue(request: &Value) -> Answer {
    let now = request["now"].as_i64().ok_or(Why::Bad)?;
    let profile = Profile::parse(text(request, "profile")?).ok_or(Why::Problem("unknown-profile", None))?;
    let authority = authority_of(profile)?;
    let chosen = leaf_key_type(request, profile)?;
    let key = keys::generate(chosen)?;
    let public = key.public()?;
    let ca = &request["ca"];
    let ca_cert = bytes(ca, "cert")?;
    let ca_key = held_ca(ca)?;
    let root = bytes(request, "root")?;
    let chain = [root.to_vec()];
    let hosts = hosts_of(request)?;
    let made = pki::issue(&fields(request, &hosts, &public.spki_der, now)?, &Issuer { authority, cert: &ca_cert, key: &ca_key, chain: &chain })?;
    let chain: Vec<String> = made.chain.iter().map(|one| encode(one)).collect();
    Ok(json!({"key": encode(key.as_bytes()), "type": chosen.name(), "cert": encode(&made.cert), "chain": chain, "description": description(&made.cert)?}))
}

fn auth_export(request: &Value) -> Answer {
    let accounts: Vec<Account> = serde_json::from_value(request["accounts"].clone()).map_err(|_| Why::Bad)?;
    let services: Vec<BackupService> = serde_json::from_value(request["services"].clone()).map_err(|_| Why::Bad)?;
    let password = text(request, "password")?;
    match text(request, "format")? {
        "file" => {
            let now = request["now"].as_i64().ok_or(Why::Bad)?;
            Ok(json!({"file": bmc::write_export(&accounts, &services, password, now).map_err(|_| Why::Failed)?}))
        }
        "qr" => {
            let chunks = bmc::write_bmc1(&accounts, &services, password).map_err(|_| Why::Failed)?;
            let chunks: Vec<&str> = chunks.iter().map(|chunk| chunk.as_str()).collect();
            Ok(json!({"chunks": chunks}))
        }
        _ => Err(Why::Bad),
    }
}

fn job(request: &Value) -> Answer {
    match text(request, "job")? {
        "key-types" => key_types(),
        "key-new" => key_new(request),
        "key-check" => key_check(request),
        "key-export" => key_export(request),
        "pfx" => pfx(request),
        "sign" => sign(request),
        "sign-message" => sign_message(request),
        "issue-check" => issue_check(request),
        "make-root" => make_root(request),
        "make-ca" => make_ca(request),
        "issue" => issue(request),
        "auth-export" => auth_export(request),
        _ => Err(Why::Bad),
    }
}

fn main() -> ExitCode {
    // SAFETY: both only change this process: no core file and no /proc/PID/mem for anyone, and a time to be gone by
    unsafe {
        libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
        libc::alarm(PATIENCE_SECONDS);
    }
    let mut line = Zeroizing::new(Vec::new());
    if std::io::stdin().lock().take(MOST_IN).read_to_end(&mut line).is_err() {
        return ExitCode::FAILURE;
    }
    let Ok(request) = serde_json::from_slice::<Value>(&line) else { return ExitCode::FAILURE };
    let answer = match job(&request) {
        Ok(mut answer) => {
            answer["ok"] = json!(true);
            answer
        }
        Err(Why::Problem(problem, field)) => json!({"ok": false, "why": "refused", "problem": problem, "field": field}),
        Err(Why::Bad) => json!({"ok": false, "why": "bad-request"}),
        Err(Why::Failed) => json!({"ok": false, "why": "failed"}),
    };
    let out = Zeroizing::new(answer.to_string());
    if std::io::stdout().lock().write_all(out.as_bytes()).is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
