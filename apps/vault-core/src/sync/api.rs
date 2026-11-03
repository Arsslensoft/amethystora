//! Talking to the server (spec 2, 3, 5, 6): prelogin, the token endpoint and what its answers mean, `/sync`, and the five
//! writes this client makes. Every request goes through `Http`, which in the service is the network service's socket and in a
//! test a fake server. What a response says is checked here before anything is believed.

use std::cell::Cell;
use std::fmt;

use base64::Engine;
use serde::de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::Deserializer;
use serde_json::{json, Map, Number, Value};
use zeroize::Zeroizing;

use super::address::{origin_of, Server};
use super::crypto::{self, Kdf, KdfProblem};
use super::json;
use super::{CLIENT_ID, CLIENT_VERSION, DEVICE_NAME, DEVICE_TYPE, USER_AGENT};

/// The largest response a run reads: a /sync of some thousands of items, and nothing near a machine's memory
pub const MOST_BODY: usize = 32 * 1024 * 1024;
/// More ciphers than this in a `/sync` is a damaged answer or a hostile one: the vault itself holds 5,000 items at most
pub const MOST_CIPHERS: usize = 20_000;
/// What one cipher, and the rest of the answer together, may be made of once parsed. A `/sync` of 32 MiB of `0,` would be 16
/// million values and over half a gigabyte; these keep what is parsed to a few tens of MB whatever the server sends.
const MOST_NODES_CIPHER: usize = 4_000;
const MOST_NODES_ALL: usize = 1_000_000;
const MOST_NODES_PROFILE: usize = 50_000;
const MOST_DEPTH: usize = 32;

pub struct Request {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    /// The form or JSON, which may hold the master password hash or a token
    pub body: Zeroizing<Vec<u8>>,
}

pub struct Response {
    pub status: u16,
    pub body: Zeroizing<Vec<u8>>,
}

/// Why nothing came back. The network service's words, as a fixed set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetFailure {
    Unreachable,
    /// TLS refused: a certificate nobody here trusts, or the wrong host
    Untrusted,
    Timeout,
    TooLarge,
    /// A host or scheme the connection may not reach
    NotAllowed,
    /// The vault closed, or the run took longer than a run may: nothing more was asked
    Cancelled,
}

impl NetFailure {
    pub fn name(self) -> &'static str {
        match self {
            NetFailure::Unreachable => "unreachable",
            NetFailure::Untrusted => "untrusted",
            NetFailure::Timeout => "timeout",
            NetFailure::TooLarge => "too-large",
            NetFailure::NotAllowed => "not-allowed",
            NetFailure::Cancelled => "cancelled",
        }
    }

    pub fn from_name(name: &str) -> NetFailure {
        match name {
            "untrusted" => NetFailure::Untrusted,
            "timeout" => NetFailure::Timeout,
            "too-large" => NetFailure::TooLarge,
            "not-allowed" => NetFailure::NotAllowed,
            "cancelled" => NetFailure::Cancelled,
            _ => NetFailure::Unreachable,
        }
    }
}

pub trait Http {
    fn call(&mut self, request: Request) -> Result<Response, NetFailure>;
}

/// What stopped a login or a run, as a stable word for the socket and a person's sentence in the window or recipe
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    Net(NetFailure),
    /// 5xx: the vault goes on, and tries at the next interval
    ServerDown,
    RateLimited,
    /// 403 or not JSON: refused before the application read it
    ServerRefused,
    /// The server's own message, cleaned: shown as data, never as a sentence of ours
    ServerSaid(String),
    /// The providers this client offers, of those the account has
    TwoFactor(Vec<u8>),
    /// Only providers this client does not offer: the API key does not ask
    TwoFactorUnoffered,
    NewDevice,
    Kdf(KdfProblem),
    /// PBKDF2 under 600,000: the number, for the person to confirm
    KdfConfirm(u32),
    /// Argon2 asking for more memory than a machine is asked for lightly (MiB): the person confirms before it is spent
    KdfMemoryConfirm(u32),
    SaltChanged,
    /// SSO, Key Connector, trusted devices, no master password, an organization's rules
    Unsupported,
    PasswordReset,
    /// Bitwarden's newer encryption (spec 4.5): nothing is written
    NewerEncryption,
    /// The user key did not open: a wrong password, or a key the server changed
    CannotOpen,
    /// No key or no private key: never made here (BWb1)
    NoKey,
    /// The account's key is not the one connected: changed elsewhere, reconnect
    KeysChanged,
    /// Another account answered
    IdentityChanged,
    /// The refresh token is no longer good: a password login has to be made again
    LoginExpired,
    ClientVersion,
    /// Not what the protocol says a response is
    BadResponse,
    /// Many items gone or trashed on the server, at once or over the days a window keeps: a person has to say so first
    ManyGone(usize),
    /// Many items arriving from the server in the same way
    ManyNew(usize),
    /// An item of the vault did not read: nothing is sent to the server while that is so, since an item that is not there
    /// reads as removed
    LocalUnreadable(usize),
    /// The links of a shard did not read: starting over would put every item of the shard on the server again
    LinksUnreadable,
}

impl Problem {
    pub fn name(&self) -> &'static str {
        match self {
            Problem::Net(failure) => failure.name(),
            Problem::ServerDown => "server-down",
            Problem::RateLimited => "rate-limited",
            Problem::ServerRefused => "server-refused",
            Problem::ServerSaid(_) => "server-said",
            Problem::TwoFactor(_) => "two-factor",
            Problem::TwoFactorUnoffered => "two-factor-unoffered",
            Problem::NewDevice => "new-device",
            Problem::Kdf(problem) => problem.name(),
            Problem::KdfConfirm(_) => "kdf-confirm",
            Problem::KdfMemoryConfirm(_) => "kdf-memory-confirm",
            Problem::SaltChanged => "salt-changed",
            Problem::Unsupported => "unsupported",
            Problem::PasswordReset => "password-reset",
            Problem::NewerEncryption => "newer-encryption",
            Problem::CannotOpen => "cannot-open",
            Problem::NoKey => "no-key",
            Problem::KeysChanged => "keys-changed",
            Problem::IdentityChanged => "identity-changed",
            Problem::LoginExpired => "login-expired",
            Problem::ClientVersion => "client-version",
            Problem::BadResponse => "bad-response",
            Problem::ManyGone(_) => "many-gone",
            Problem::ManyNew(_) => "many-new",
            Problem::LocalUnreadable(_) => "local-unreadable",
            Problem::LinksUnreadable => "links-unreadable",
        }
    }

    /// A problem that the next interval may not have: everything else waits for a person
    pub fn passes(&self) -> bool {
        matches!(self, Problem::Net(_) | Problem::ServerDown | Problem::RateLimited)
    }

    /// What goes on the socket beside `problem`
    pub fn detail(&self) -> Value {
        match self {
            Problem::ServerSaid(message) => json!({"message": message}),
            Problem::TwoFactor(providers) => json!({"providers": providers}),
            Problem::KdfConfirm(iterations) => json!({"iterations": iterations}),
            Problem::KdfMemoryConfirm(memory) => json!({"memory": memory}),
            Problem::ManyGone(count) | Problem::ManyNew(count) | Problem::LocalUnreadable(count) => json!({"count": count, "days": super::merge::WINDOW_DAYS}),
            _ => json!({}),
        }
    }
}

/// A server's text, fit to show as data: one line, no control or direction characters, cut short
pub fn clean(text: &str) -> String {
    let bidi = |c: char| matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{61c}');
    let one: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).filter(|c| !bidi(*c)).take(200).collect();
    one.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A server id that goes into a URL: a UUID, nothing else (a `..` or a `/` would reach another endpoint)
pub fn is_uuid(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

/// Percent-encodes a form value: everything but the unreserved characters
fn form_value(value: &str, out: &mut Vec<u8>) {
    for &b in value.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b);
        } else {
            out.extend_from_slice(format!("%{b:02X}").as_bytes());
        }
    }
}

fn form(pairs: &[(&str, &str)]) -> Zeroizing<Vec<u8>> {
    let mut out = Zeroizing::new(Vec::with_capacity(512));
    for (at, (name, value)) in pairs.iter().enumerate() {
        if at > 0 {
            out.push(b'&');
        }
        form_value(name, &mut out);
        out.push(b'=');
        form_value(value, &mut out);
    }
    out
}

pub(super) fn headers(content: Option<&'static str>) -> Vec<(&'static str, String)> {
    let mut headers = vec![
        ("Accept", "application/json".to_string()),
        ("Bitwarden-Client-Version", CLIENT_VERSION.to_string()),
        ("Bitwarden-Client-Name", CLIENT_ID.to_string()),
        ("Device-Type", DEVICE_TYPE.to_string()),
        ("User-Agent", USER_AGENT.to_string()),
    ];
    if let Some(content) = content {
        headers.push(("Content-Type", content.to_string()));
    }
    headers
}

/// A JSON value read into a `Value`, counting what it is made of and refusing past a budget: how deep, and how many values
struct Bounded<'a> {
    nodes: &'a Cell<usize>,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for Bounded<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        let left = self.nodes.get();
        if left == 0 || self.depth > MOST_DEPTH {
            return Err(D::Error::custom("too much"));
        }
        self.nodes.set(left - 1);
        deserializer.deserialize_any(BoundedVisitor { nodes: self.nodes, depth: self.depth })
    }
}

struct BoundedVisitor<'a> {
    nodes: &'a Cell<usize>,
    depth: usize,
}

impl<'de> Visitor<'de> for BoundedVisitor<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
        Ok(Number::from_f64(value).map_or(Value::Null, Value::Number))
    }

    fn visit_str<E>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_string()))
    }

    fn visit_string<E>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(Bounded { nodes: self.nodes, depth: self.depth + 1 })? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            let value = map.next_value_seed(Bounded { nodes: self.nodes, depth: self.depth + 1 })?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}

/// What a `/sync` is read into: only the parts this client looks at, the ciphers one at a time and counted, and nothing else
struct SyncSeed;

impl<'de> DeserializeSeed<'de> for SyncSeed {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_map(SyncVisitor)
    }
}

struct SyncVisitor;

impl<'de> Visitor<'de> for SyncVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a sync")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = Map::new();
        let all = Cell::new(MOST_NODES_ALL);
        while let Some(key) = map.next_key::<String>()? {
            if key.eq_ignore_ascii_case("ciphers") {
                object.insert(key, map.next_value_seed(CiphersSeed { all: &all })?);
            } else if ["profile", "userDecryption", "accountKeys"].iter().any(|wanted| key.eq_ignore_ascii_case(wanted)) {
                let nodes = Cell::new(MOST_NODES_PROFILE);
                object.insert(key, map.next_value_seed(Bounded { nodes: &nodes, depth: 1 })?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(Value::Object(object))
    }
}

struct CiphersSeed<'a> {
    all: &'a Cell<usize>,
}

impl<'de> DeserializeSeed<'de> for CiphersSeed<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for CiphersSeed<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ciphers")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut ciphers = Vec::new();
        loop {
            // Each cipher has a budget of its own, and what it uses comes off the whole answer's
            let nodes = Cell::new(MOST_NODES_CIPHER.min(self.all.get()));
            let before = nodes.get();
            let Some(cipher) = seq.next_element_seed(Bounded { nodes: &nodes, depth: 1 })? else { break };
            if ciphers.len() >= MOST_CIPHERS || !cipher.is_object() {
                return Err(A::Error::custom("not a list of ciphers"));
            }
            self.all.set(self.all.get() - (before - nodes.get()));
            ciphers.push(cipher);
        }
        Ok(Value::Array(ciphers))
    }
}

/// A `/sync` answer, read within bounds: a count of ciphers, and a number of values for each and all together
fn parse_sync(response: &Response) -> Result<Value, Problem> {
    if response.body.len() > MOST_BODY {
        return Err(Problem::Net(NetFailure::TooLarge));
    }
    let mut reader = serde_json::Deserializer::from_slice(&response.body);
    let value = SyncSeed.deserialize(&mut reader).map_err(|_| Problem::BadResponse)?;
    reader.end().map_err(|_| Problem::BadResponse)?;
    Ok(value)
}

/// The body as JSON, bounded, or the problem its status says
pub(super) fn parse(response: &Response) -> Result<Value, Problem> {
    if response.body.len() > MOST_BODY {
        return Err(Problem::Net(NetFailure::TooLarge));
    }
    serde_json::from_slice(&response.body).map_err(|_| Problem::BadResponse)
}

/// What an unsuccessful answer means (spec 3.7)
pub(super) fn failure(response: &Response) -> Problem {
    match response.status {
        429 => return Problem::RateLimited,
        500..=599 => return Problem::ServerDown,
        300..=399 => return Problem::Net(NetFailure::NotAllowed),
        _ => {}
    }
    let Ok(body) = serde_json::from_slice::<Value>(&response.body) else { return Problem::ServerRefused };
    if response.status == 403 || !body.is_object() {
        return Problem::ServerRefused;
    }
    if let Some(providers) = json::some(&body, "TwoFactorProviders2").and_then(Value::as_object).filter(|providers| !providers.is_empty()) {
        // Authenticator, email and YubiKey OTP are offered; never the recovery code, which turns every second factor off
        let offered: Vec<u8> = providers.keys().filter_map(|key| key.parse::<u8>().ok()).filter(|n| matches!(n, 0 | 1 | 3)).collect();
        return if offered.is_empty() { Problem::TwoFactorUnoffered } else { Problem::TwoFactor(offered) };
    }
    let error = json::text(&body, "error").unwrap_or("");
    let message = json::path(&body, &["ErrorModel", "Message"]).and_then(Value::as_str);
    if message == Some("new device verification required") {
        return Problem::NewDevice;
    }
    if json::some(&body, "SsoOrganizationIdentifier").is_some() {
        return Problem::Unsupported;
    }
    if matches!(error, "version_header_missing" | "invalid_client_version") {
        return Problem::ClientVersion;
    }
    let said = message.or_else(|| json::text(&body, "message")).or_else(|| json::text(&body, "error_description")).unwrap_or(error);
    Problem::ServerSaid(clean(said))
}

/// The settings prelogin gives for an email (spec 2): read from `kdfSettings` where present, else the older names, refused if
/// the two differ or the salt is not the email's
pub fn prelogin(http: &mut dyn Http, server: &Server, email: &str) -> Result<Kdf, Problem> {
    let body = Zeroizing::new(serde_json::to_vec(&json!({"email": crypto::normalize_email(email)})).unwrap_or_default());
    let mut ask = |path: &str| {
        http.call(Request { method: "POST", url: format!("{}{path}", server.identity), headers: headers(Some("application/json")), body: body.clone() }).map_err(Problem::Net)
    };
    let mut response = ask("/accounts/prelogin/password")?;
    if response.status == 404 {
        response = ask("/accounts/prelogin")?;
    }
    if response.status != 200 {
        return Err(failure(&response));
    }
    let answer = parse(&response)?;
    let number = |value: Option<&Value>| -> Result<Option<u32>, Problem> {
        match value {
            None | Some(Value::Null) => Ok(None),
            Some(value) => value.as_u64().and_then(|n| u32::try_from(n).ok()).map(Some).ok_or(Problem::BadResponse),
        }
    };
    let read = |at: &Value, names: [&str; 4]| -> Result<Option<Kdf>, Problem> {
        let Some(kind) = number(json::get(at, names[0]))? else { return Ok(None) };
        let kind = u8::try_from(kind).map_err(|_| Problem::Kdf(KdfProblem::Unknown))?;
        Ok(Some(Kdf {
            kind,
            iterations: number(json::get(at, names[1]))?.ok_or(Problem::BadResponse)?,
            memory: number(json::get(at, names[2]))?,
            parallelism: number(json::get(at, names[3]))?,
        }))
    };
    let new = match json::some(&answer, "kdfSettings") {
        Some(settings) => read(settings, ["kdfType", "iterations", "memory", "parallelism"])?,
        None => None,
    };
    let old = read(&answer, ["kdf", "kdfIterations", "kdfMemory", "kdfParallelism"])?;
    let kdf = match (new, old) {
        (Some(new), Some(old)) if !new.same(&old) => return Err(Problem::Kdf(KdfProblem::Unknown)),
        (Some(kdf), _) | (None, Some(kdf)) => kdf,
        (None, None) => return Err(Problem::BadResponse),
    };
    if !crypto::salt_is_ours(json::text(&answer, "salt"), email) {
        return Err(Problem::SaltChanged);
    }
    kdf.check().map_err(Problem::Kdf)?;
    Ok(kdf)
}

/// How a login proves itself
pub enum Credentials<'a> {
    /// The master password hash, base64
    Password(&'a str),
    ApiKey { id: &'a str, secret: &'a str },
    Refresh(&'a str),
}

/// What a retry of a login adds: a second factor, a remembered device, a new device's code
#[derive(Default)]
pub struct Extra<'a> {
    pub two_factor: Option<(u8, &'a str, bool)>,
    pub remembered: Option<&'a str>,
    pub new_device: Option<&'a str>,
}

/// A successful login, checked
pub struct Token {
    pub access: Zeroizing<String>,
    pub expires_in: u64,
    pub refresh: Option<Zeroizing<String>>,
    /// The user key as the server holds it, a type 2 EncString; absent on a refresh
    pub protected_key: Option<String>,
    /// A second factor's remember token, to be kept as a secret
    pub remember: Option<Zeroizing<String>>,
}

/// The token endpoint (spec 3). `kdf` is what the master key was derived with: a response that says otherwise is refused before
/// anything is unwrapped.
pub fn token(http: &mut dyn Http, server: &Server, email: &str, device: &str, credentials: &Credentials<'_>, extra: &Extra<'_>, kdf: Option<&Kdf>) -> Result<Token, Problem> {
    let email = crypto::normalize_email(email);
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    match credentials {
        Credentials::Password(hash) => pairs.extend([("grant_type", "password"), ("client_id", CLIENT_ID), ("scope", "api offline_access"), ("username", email.as_str()), ("password", *hash)]),
        Credentials::ApiKey { id, secret } => pairs.extend([("grant_type", "client_credentials"), ("client_id", *id), ("client_secret", *secret), ("scope", "api")]),
        Credentials::Refresh(refresh) => pairs.extend([("grant_type", "refresh_token"), ("client_id", CLIENT_ID), ("refresh_token", *refresh)]),
    }
    if !matches!(credentials, Credentials::Refresh(_)) {
        pairs.extend([("deviceType", DEVICE_TYPE), ("deviceIdentifier", device), ("deviceName", DEVICE_NAME)]);
    }
    let provider;
    match (extra.two_factor, extra.remembered) {
        (Some((number, code, remember)), _) => {
            provider = number.to_string();
            pairs.extend([("twoFactorToken", code), ("twoFactorProvider", provider.as_str()), ("twoFactorRemember", if remember { "1" } else { "0" })]);
        }
        (None, Some(remembered)) => pairs.extend([("twoFactorToken", remembered), ("twoFactorProvider", "5"), ("twoFactorRemember", "0")]),
        (None, None) => {}
    }
    if let Some(code) = extra.new_device {
        pairs.push(("newDeviceOtp", code));
    }
    let body = form(&pairs);
    let response = http
        .call(Request { method: "POST", url: format!("{}/connect/token", server.identity), headers: headers(Some("application/x-www-form-urlencoded; charset=utf-8")), body })
        .map_err(Problem::Net)?;
    if response.status != 200 {
        let problem = failure(&response);
        return Err(match (credentials, problem) {
            (Credentials::Refresh(_), Problem::ServerSaid(_)) => Problem::LoginExpired,
            (_, problem) => problem,
        });
    }
    let answer = parse(&response)?;
    let access = json::text(&answer, "access_token").filter(|token| !token.is_empty()).ok_or(Problem::BadResponse)?;
    let expires_in = json::number(&answer, "expires_in").unwrap_or(3600).clamp(60, 24 * 3600);
    let refresh = json::text(&answer, "refresh_token").filter(|token| !token.is_empty()).map(|token| Zeroizing::new(token.to_string()));
    let remember = json::text(&answer, "TwoFactorToken").filter(|token| !token.is_empty()).map(|token| Zeroizing::new(token.to_string()));
    let protected_key = if matches!(credentials, Credentials::Refresh(_)) { None } else { Some(check_login(&answer, kdf, &email)?) };
    Ok(Token { access: Zeroizing::new(access.to_string()), expires_in, refresh, protected_key, remember })
}

fn kdf_of(at: &Value, names: [&str; 4]) -> Option<Kdf> {
    let number = |name: &str| json::get(at, name).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok());
    Some(Kdf { kind: u8::try_from(number(names[0])?).ok()?, iterations: number(names[1])?, memory: number(names[2]), parallelism: number(names[3]) })
}

/// What a login response must say (spec 3.4) before its key is looked at, and the one protected key it carries
fn check_login(answer: &Value, kdf: Option<&Kdf>, email: &str) -> Result<String, Problem> {
    let truthy = |at: &Value, name: &str| json::get(at, name).and_then(Value::as_bool) == Some(true);
    if truthy(answer, "ForcePasswordReset") {
        return Err(Problem::PasswordReset);
    }
    if truthy(answer, "ApiUseKeyConnector") || json::some(answer, "KeyConnectorUrl").is_some() {
        return Err(Problem::Unsupported);
    }
    let options = json::some(answer, "UserDecryptionOptions");
    if let Some(options) = options
        && (json::get(options, "HasMasterPassword").and_then(Value::as_bool) == Some(false)
            || json::some(options, "KeyConnectorOption").is_some()
            || json::some(options, "TrustedDeviceOption").is_some())
    {
        return Err(Problem::Unsupported);
    }
    let unlock = options.and_then(|options| json::some(options, "MasterPasswordUnlock"));
    // Every place the settings appear says the same as what was derived with
    let mut claims = Vec::new();
    if json::some(answer, "Kdf").is_some() {
        claims.push(kdf_of(answer, ["Kdf", "KdfIterations", "KdfMemory", "KdfParallelism"]).ok_or(Problem::BadResponse)?);
    }
    if let Some(settings) = unlock.and_then(|unlock| json::some(unlock, "Kdf")) {
        claims.push(kdf_of(settings, ["KdfType", "Iterations", "Memory", "Parallelism"]).ok_or(Problem::BadResponse)?);
    }
    if let Some(kdf) = kdf
        && claims.iter().any(|claim| !claim.same(kdf))
    {
        return Err(Problem::Kdf(KdfProblem::BelowPin));
    }
    if !crypto::salt_is_ours(unlock.and_then(|unlock| json::text(unlock, "Salt")), email) {
        return Err(Problem::SaltChanged);
    }
    // One protected key, wherever it appears
    let mut keys: Vec<&str> = Vec::new();
    for key in [json::some(answer, "Key"), unlock.and_then(|u| json::some(u, "MasterKeyEncryptedUserKey")), unlock.and_then(|u| json::some(u, "MasterKeyWrappedUserKey"))].into_iter().flatten() {
        keys.push(key.as_str().ok_or(Problem::BadResponse)?);
    }
    let Some(first) = keys.first().copied() else { return Err(Problem::NoKey) };
    if first.is_empty() || keys.iter().any(|key| *key != first) {
        return Err(Problem::NoKey);
    }
    let private = json::text(answer, "PrivateKey").filter(|key| !key.is_empty()).ok_or(Problem::NoKey)?;
    if private.starts_with("7.") || first.starts_with("7.") {
        return Err(Problem::NewerEncryption);
    }
    if let Some(keys) = json::some(answer, "AccountKeys")
        && (json::some(keys, "signatureKeyPair").is_some() || json::some(keys, "securityState").is_some())
    {
        return Err(Problem::NewerEncryption);
    }
    if !crypto::is_type2(first) {
        return Err(Problem::NoKey);
    }
    Ok(first.to_string())
}

/// The user key, from the protected key under the stretched master key: type 2, a MAC that verifies, exactly 64 bytes
pub fn unwrap_user_key(protected: &str, stretched: &crypto::SymKey) -> Result<crypto::SymKey, Problem> {
    match crypto::wrapped_length(protected, stretched) {
        None => Err(Problem::CannotOpen),
        Some(64) => crypto::open_key(protected, stretched).map_err(|_| Problem::CannotOpen),
        Some(length) if length > 64 => Err(Problem::NewerEncryption),
        // 32 is the old unauthenticated form, and the downgrade of BW12
        Some(_) => Err(Problem::CannotOpen),
    }
}

/// The `sub` of an access token, which has to be the account's id (spec 5.7). The token is otherwise opaque.
pub fn token_subject(access: &str) -> Option<String> {
    let payload = access.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let claims: Value = serde_json::from_slice(&bytes).ok()?;
    json::text(&claims, "sub").map(str::to_string)
}

/// A connected account's API calls
pub struct Api<'a> {
    pub http: &'a mut dyn Http,
    pub server: &'a Server,
    pub access: &'a str,
}

impl Api<'_> {
    pub(super) fn call(&mut self, method: &'static str, path: &str, body: Option<&Value>) -> Result<Response, Problem> {
        let mut headers = headers(body.map(|_| "application/json"));
        headers.push(("Authorization", format!("Bearer {}", self.access)));
        let body = Zeroizing::new(body.map(|body| serde_json::to_vec(body).unwrap_or_default()).unwrap_or_default());
        let url = format!("{}{path}", self.server.api);
        // Nothing but the connected server: the bases are ours, and the path is ours or a checked id
        if origin_of(&url).is_none_or(|origin| !self.server.origins().contains(&origin)) {
            return Err(Problem::Net(NetFailure::NotAllowed));
        }
        self.http.call(Request { method, url, headers, body }).map_err(Problem::Net)
    }

    pub(super) fn expect(&mut self, method: &'static str, path: &str, body: Option<&Value>) -> Result<Value, Expired> {
        let response = self.call(method, path, body)?;
        match response.status {
            200 if response.body.is_empty() => Ok(Value::Null),
            200 => Ok(parse(&response)?),
            401 => Err(Expired::Token),
            _ => Err(Expired::Problem(failure(&response))),
        }
    }

    /// The account's last change in milliseconds: a hint only (spec 5.1)
    pub fn revision_date(&mut self) -> Result<Option<u64>, Expired> {
        Ok(self.expect("GET", "/accounts/revision-date", None)?.as_u64())
    }

    pub fn sync(&mut self) -> Result<Value, Expired> {
        let response = self.call("GET", "/sync?excludeDomains=true", None)?;
        match response.status {
            200 => Ok(parse_sync(&response)?),
            401 => Err(Expired::Token),
            _ => Err(Expired::Problem(failure(&response))),
        }
    }

    pub fn create(&mut self, body: &Value) -> Result<Value, Expired> {
        self.expect("POST", "/ciphers", Some(body))
    }

    /// The whole item replaced; a 400 is a stale copy or another refusal, which `cipher` tells apart
    pub fn update(&mut self, id: &str, body: &Value) -> Result<Result<Value, u16>, Expired> {
        self.one_of(id)?;
        let response = self.call("PUT", &format!("/ciphers/{id}"), Some(body))?;
        match response.status {
            200 => Ok(Ok(parse(&response)?)),
            400 => Ok(Err(400)),
            401 => Err(Expired::Token),
            _ => Err(Expired::Problem(failure(&response))),
        }
    }

    /// Into the server's trash. Only `PUT .../delete`: the same path with `POST`, or `DELETE`, destroys it for good.
    pub fn trash(&mut self, id: &str) -> Result<(), Expired> {
        self.one_of(id)?;
        self.expect("PUT", &format!("/ciphers/{id}/delete"), None).map(|_| ())
    }

    pub fn restore(&mut self, id: &str) -> Result<Value, Expired> {
        self.one_of(id)?;
        self.expect("PUT", &format!("/ciphers/{id}/restore"), None)
    }

    pub fn fetch(&mut self, id: &str) -> Result<Value, Expired> {
        self.one_of(id)?;
        self.expect("GET", &format!("/ciphers/{id}"), None)
    }

    fn one_of(&self, id: &str) -> Result<(), Expired> {
        if is_uuid(id) { Ok(()) } else { Err(Expired::Problem(Problem::BadResponse)) }
    }
}

/// An API call that failed: the token ran out (a new one once, then a login), or anything else
#[derive(Debug, PartialEq, Eq)]
pub enum Expired {
    Token,
    Problem(Problem),
}

impl From<Problem> for Expired {
    fn from(problem: Problem) -> Expired {
        Expired::Problem(problem)
    }
}

/// Asks for the email code of a second factor (spec 3.5), for a password login
pub fn send_email_code(http: &mut dyn Http, server: &Server, email: &str, hash: &str, device: &str) -> Result<(), Problem> {
    let body = json!({"email": crypto::normalize_email(email), "masterPasswordHash": hash, "deviceIdentifier": device});
    let body = Zeroizing::new(serde_json::to_vec(&body).unwrap_or_default());
    let response = http.call(Request { method: "POST", url: format!("{}/two-factor/send-email-login", server.api), headers: headers(Some("application/json")), body }).map_err(Problem::Net)?;
    if response.status == 200 { Ok(()) } else { Err(failure(&response)) }
}

/// Asks Bitwarden's server to send a new device's code again (spec 3.6)
pub fn resend_device_code(http: &mut dyn Http, server: &Server, email: &str, hash: &str) -> Result<(), Problem> {
    let body = Zeroizing::new(serde_json::to_vec(&json!({"email": crypto::normalize_email(email), "masterPasswordHash": hash})).unwrap_or_default());
    let response = http.call(Request { method: "POST", url: format!("{}/accounts/resend-new-device-otp", server.api), headers: headers(Some("application/json")), body }).map_err(Problem::Net)?;
    if response.status == 200 { Ok(()) } else { Err(failure(&response)) }
}

/// What the server claims to be, for showing only: unauthenticated, so it decides nothing (spec 1)
pub fn config(http: &mut dyn Http, server: &Server) -> Option<(String, String)> {
    let response = http.call(Request { method: "GET", url: format!("{}/config", server.api), headers: headers(None), body: Zeroizing::new(Vec::new()) }).ok()?;
    if response.status != 200 {
        return None;
    }
    let answer = parse(&response).ok()?;
    let name = json::path(&answer, &["server", "name"]).and_then(Value::as_str).unwrap_or("");
    let version = json::text(&answer, "version").unwrap_or("");
    Some((clean(name), clean(version)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_form_hides_nothing_in_plain_sight() {
        assert_eq!(&form(&[("password", "a+b/c="), ("scope", "api offline_access")])[..], b"password=a%2Bb%2Fc%3D&scope=api%20offline_access");
    }

    #[test]
    fn server_text_is_data() {
        assert_eq!(clean("Bad\npassword\u{202e}  <b>x</b>"), "Bad password <b>x</b>");
        assert_eq!(clean(&"x".repeat(500)).len(), 200);
    }

    #[test]
    fn only_a_uuid_reaches_a_path() {
        assert!(is_uuid("3f2504e0-4f89-11d3-9a0c-0305e82c3301"));
        assert!(!is_uuid("../accounts/delete-account-00000000000"));
        assert!(!is_uuid("3f2504e0-4f89-11d3-9a0c-0305e82c330/"));
    }

    fn answer(status: u16, body: Value) -> Response {
        Response { status, body: Zeroizing::new(serde_json::to_vec(&body).unwrap()) }
    }

    #[test]
    fn what_a_refusal_means() {
        assert_eq!(failure(&answer(400, json!({"error": "invalid_grant", "TwoFactorProviders2": {"0": null, "1": {"Email": "p***@x"}, "8": null}}))), Problem::TwoFactor(vec![0, 1]));
        assert_eq!(failure(&answer(400, json!({"error": "invalid_grant", "TwoFactorProviders2": {"7": {}, "2": {}}}))), Problem::TwoFactorUnoffered);
        assert_eq!(failure(&answer(400, json!({"error": "device_error", "ErrorModel": {"Message": "new device verification required"}}))), Problem::NewDevice);
        assert_eq!(failure(&answer(400, json!({"error": "version_header_missing"}))), Problem::ClientVersion);
        assert_eq!(failure(&answer(400, json!({"error": "", "message": "Username or password is incorrect. Try again"}))), Problem::ServerSaid("Username or password is incorrect. Try again".into()));
        assert_eq!(failure(&answer(429, json!({}))), Problem::RateLimited);
        assert_eq!(failure(&answer(503, json!({}))), Problem::ServerDown);
        assert_eq!(failure(&Response { status: 403, body: Zeroizing::new(b"<html>".to_vec()) }), Problem::ServerRefused);
        assert_eq!(failure(&answer(302, json!({}))), Problem::Net(NetFailure::NotAllowed));
    }

    fn good_login(key: &str) -> Value {
        json!({"access_token": "t", "expires_in": 7200, "Key": key, "PrivateKey": "2.x|y|z", "Kdf": 0, "KdfIterations": 600000,
               "UserDecryptionOptions": {"HasMasterPassword": true, "MasterPasswordUnlock": {"Kdf": {"KdfType": 0, "Iterations": 600000}, "MasterKeyEncryptedUserKey": key, "Salt": "a@b.test"}}})
    }

    const KEY: &str = "2.oKGio6SlpqeoqaqrrK2urw==|x0aUEHmhwY/5srCpXnAac90yp7puDnUZ702oADiY9gkTZKONsg7857LaHClYehfITqFRl+lL5z6YgsPFzB5TuryhPI+3McbYkZeqqvsAEPo=|Eft0auj9Ifs1MvhUezN9uhLdQFgTihg2xr7U4BT33Jk=";

    #[test]
    fn a_login_response_is_checked_before_its_key() {
        let kdf = Kdf { kind: 0, iterations: 600_000, memory: None, parallelism: None };
        assert_eq!(check_login(&good_login(KEY), Some(&kdf), "a@b.test"), Ok(KEY.to_string()));
        let lowered = Kdf { iterations: 700_000, ..kdf };
        assert_eq!(check_login(&good_login(KEY), Some(&lowered), "a@b.test"), Err(Problem::Kdf(KdfProblem::BelowPin)));
        let mut two = good_login(KEY);
        two["UserDecryptionOptions"]["MasterPasswordUnlock"]["MasterKeyEncryptedUserKey"] = json!("2.AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=");
        assert_eq!(check_login(&two, Some(&kdf), "a@b.test"), Err(Problem::NoKey));
        let mut none = good_login(KEY);
        none["Key"] = Value::Null;
        none["UserDecryptionOptions"]["MasterPasswordUnlock"]["MasterKeyEncryptedUserKey"] = Value::Null;
        assert_eq!(check_login(&none, Some(&kdf), "a@b.test"), Err(Problem::NoKey));
        let zero = "0.sLGys7S1tre4ubq7vL2+vw==|M3zqN2ch0xEveb6Pl2M8YA==";
        assert_eq!(check_login(&good_login(zero), Some(&kdf), "a@b.test"), Err(Problem::NoKey));
        let mut no_private = good_login(KEY);
        no_private["PrivateKey"] = Value::Null;
        assert_eq!(check_login(&no_private, Some(&kdf), "a@b.test"), Err(Problem::NoKey));
        let mut v2 = good_login(KEY);
        v2["PrivateKey"] = json!("7.abc");
        assert_eq!(check_login(&v2, Some(&kdf), "a@b.test"), Err(Problem::NewerEncryption));
        let mut signed = good_login(KEY);
        signed["AccountKeys"] = json!({"publicKeyEncryptionKeyPair": {}, "signatureKeyPair": {"x": 1}});
        assert_eq!(check_login(&signed, Some(&kdf), "a@b.test"), Err(Problem::NewerEncryption));
        let mut salted = good_login(KEY);
        salted["UserDecryptionOptions"]["MasterPasswordUnlock"]["Salt"] = json!("other@b.test");
        assert_eq!(check_login(&salted, Some(&kdf), "a@b.test"), Err(Problem::SaltChanged));
        let mut connector = good_login(KEY);
        connector["KeyConnectorUrl"] = json!("https://evil.test");
        assert_eq!(check_login(&connector, Some(&kdf), "a@b.test"), Err(Problem::Unsupported));
        let mut reset = good_login(KEY);
        reset["ForcePasswordReset"] = json!(true);
        assert_eq!(check_login(&reset, Some(&kdf), "a@b.test"), Err(Problem::PasswordReset));
    }

    #[test]
    fn a_user_key_is_exactly_64_bytes_under_a_good_mac() {
        let master: [u8; 32] = crate::seal::unhex("088585f32855255992919c03a92bb85e6e72a649eda55604d731da55b0d4015f").unwrap().try_into().unwrap();
        let stretched = crypto::stretch(&master);
        assert!(unwrap_user_key(KEY, &stretched).is_ok());
        let short = "2.kJGSk5SVlpeYmZqbnJ2enw==|hD88Rc65KsK1Yn87VbU1+0AiuGqzC224dWy6A2sAYIYeDRcbgLbKCZ6ARy4+V6jL|HmQ95/+1ep3RwvAgRla8jiaUS3Fe/vVolMAcnwiJ1Ds=";
        assert_eq!(unwrap_user_key(short, &stretched).err(), Some(Problem::CannotOpen));
        let long = crypto::EncString::seal(&stretched, &[7u8; 80]).text();
        assert_eq!(unwrap_user_key(&long, &stretched).err(), Some(Problem::NewerEncryption));
        let wrong = crypto::stretch(&[0u8; 32]);
        assert_eq!(unwrap_user_key(KEY, &wrong).err(), Some(Problem::CannotOpen));
    }

    #[test]
    fn the_token_names_its_account() {
        let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"sub":"3f2504e0-4f89-11d3-9a0c-0305e82c3301"}"#);
        assert_eq!(token_subject(&format!("h.{claims}.s")).as_deref(), Some("3f2504e0-4f89-11d3-9a0c-0305e82c3301"));
        assert_eq!(token_subject("opaque"), None);
    }
}
