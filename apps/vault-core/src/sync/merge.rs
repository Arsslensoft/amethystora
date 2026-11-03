//! A sync run (plans/PLAN-vault.md; spec 5 and 6): log in if need be, read `/sync`, check what it says of the account
//! before reading any cipher, then for each cipher the vault knows decide what moved where, write to the server, and say
//! what the vault should write. The vault is the source and nothing is lost: a change on both sides keeps both, a deletion
//! goes to the other side's trash, a counter never goes back, and a server that rolls back, swaps or empties is refused.
//!
//! The run writes nothing to the vault itself. It returns groups of writes, each with what the vault's items must still be
//! for it to apply, so that an edit made while the run talked to the server is never overwritten.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::items::{KIND_ACCOUNT, KIND_BACKUP_CODES, KIND_PASSWORD};
use crate::protocol::Approval;
use crate::seal::{self, Key};

use super::address::Server;
use super::api::{self, Api, Credentials, Expired, Extra, Http, Problem};
use super::cipher::{self, Content, Foreign, Opened, LOGIN, NOTE};
use super::crypto::{self, Kdf, SymKey};
use super::json;
use super::kinds;
use super::map::{self, Local, Parts};

/// The item the connection is kept in, and the kind of the items that hold the links: both of the first vault, and service-only.
/// The links (the server's last JSON of each cipher, a few KB) are in sixteen items, by the first digit of the cipher's id, so that a
/// run writes the shards that changed and a thousand links are not a thousand files.
pub const KIND_CONNECTION: &str = "sync-account";
pub const CONNECTION_ID: &str = "account";
pub const KIND_LINK: &str = "sync-links";
pub const SHARDS: &str = "0123456789abcdef";

/// Which of the sixteen items holds this cipher's link
pub fn shard_of(cipher: &str) -> char {
    cipher.chars().next().map(|c| c.to_ascii_lowercase()).filter(|c| SHARDS.contains(*c)).unwrap_or('0')
}

/// More than this many gone, trashed or arrived at once, or more than a tenth of the links, and a person has to say so first
/// (spec 5.5)
const FEW: usize = 5;
/// And the same over this many days: a server that empties a vault a few items a run is as much a server emptying it
pub const WINDOW_DAYS: u64 = 7;
const WINDOW_MS: i64 = WINDOW_DAYS as i64 * 24 * 3600 * 1000;
/// A tombstone of the vault is kept this long (the plan), and then goes
const KEPT_MS: i64 = 30 * 24 * 3600 * 1000;
/// A `revisionDate` further ahead of the clock than this is no date: it would freeze the item for good
const AHEAD_NS: i128 = 24 * 3600 * 1_000_000_000;
/// What a report keeps of the items it refused: the count is kept whole
const MOST_REFUSED: usize = 20;

/// The personal API key, as the person gave it
#[derive(Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,
    pub secret: String,
}

/// One connected account, kept as an item of the vault. The user key is here, so that opening the vault opens sync: whoever
/// opens the vault can reach the whole account.
#[derive(Clone, Serialize, Deserialize)]
pub struct Connection {
    pub address: String,
    pub identity: String,
    pub api: String,
    pub email: String,
    pub user_id: String,
    /// The user key, 64 bytes in hex
    pub user_key: String,
    /// The user key as the server holds it, which every login and `/sync` must still say
    pub protected_key: String,
    /// The settings of the last good login: a later prelogin asking for less is refused
    pub kdf: Kdf,
    pub device: String,
    #[serde(default)]
    pub api_key: Option<ApiKey>,
    #[serde(default)]
    pub refresh: Option<String>,
    #[serde(default)]
    pub remember: Option<String>,
    /// A private CA trusted for this connection alone, as PEM, with the SHA-256 the person saw
    #[serde(default)]
    pub ca: Option<String>,
    #[serde(default)]
    pub pin: Option<String>,
    /// `kind.id` -> `on`, `off` or `sealed`; absent is the kind's default
    #[serde(default)]
    pub choices: BTreeMap<String, String>,
    #[serde(default)]
    pub last_revision: Option<u64>,
    #[serde(default)]
    pub last_success_ms: Option<i64>,
    #[serde(default)]
    pub last: Option<Value>,
    /// Since when a run its timer started has waited for a connection that does not count data (at most a week)
    #[serde(default)]
    pub waiting_since_ms: Option<i64>,
    /// What went from the server or came from it over the last days, run by run (when, how many): a server that empties or fills
    /// the vault a few items at a time is stopped by the sum, which no one run shows
    #[serde(default)]
    pub recent: Vec<(i64, u32)>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.user_key.zeroize();
        for secret in [&mut self.refresh, &mut self.remember].into_iter().flatten() {
            secret.zeroize();
        }
        if let Some(key) = &mut self.api_key {
            key.secret.zeroize();
        }
    }
}

impl Connection {
    pub fn server(&self) -> Server {
        Server { address: self.address.clone(), identity: self.identity.clone(), api: self.api.clone() }
    }

    pub fn key(&self) -> Option<SymKey> {
        SymKey::from_slice(&Zeroizing::new(seal::unhex(&self.user_key)?))
    }

    /// What the person chose for an item, or its kind's default: `on`, `sealed` or `off`
    pub fn choice(&self, kind: &str, id: &str) -> &str {
        match self.choices.get(&format!("{kind}.{id}")).map(String::as_str) {
            Some(choice @ ("on" | "off" | "sealed")) => choice,
            _ if kinds::ON_BY_DEFAULT.contains(&kind) => "on",
            _ => "off",
        }
    }

    /// The same for an item that is not on the server yet: one set to ask every time is not sent unless it was chosen, since a person
    /// who set it so did not mean a copy to go by itself. One that is on the server already goes on as it was.
    pub fn choice_for_new(&self, kind: &str, id: &str, approval: Approval) -> &str {
        if approval == Approval::Ask && !self.choices.contains_key(&format!("{kind}.{id}")) { "off" } else { self.choice(kind, id) }
    }

    /// What has gone from the server or come from it in the window's days, before this run
    pub fn recent_total(&self, now_ms: i64) -> usize {
        self.recent.iter().filter(|(at, _)| *at > now_ms - WINDOW_MS && *at <= now_ms + WINDOW_MS).map(|(_, count)| *count as usize).sum()
    }

    /// What a run that was applied adds to the window; a person's acceptance empties it, since they have said what is gone is gone
    pub fn record(&mut self, churn: u32, accepted: bool, now_ms: i64) {
        self.recent.retain(|(at, _)| *at > now_ms - WINDOW_MS && *at <= now_ms + WINDOW_MS);
        if accepted {
            self.recent.clear();
        } else if churn > 0 {
            self.recent.push((now_ms, churn));
            // A window of days holds as many runs as a machine makes, and no more is kept than the sum needs
            if self.recent.len() > 200 {
                let (old, rest) = self.recent.split_at(self.recent.len() - 199);
                let folded: u32 = old.iter().map(|(_, count)| *count).fold(0, u32::saturating_add);
                let at = old.last().map_or(now_ms, |(at, _)| *at);
                self.recent = std::iter::once((at, folded)).chain(rest.iter().copied()).collect();
            }
        }
    }
}

/// A cipher the vault knows: the server's last raw JSON of it, and the vault items it carries with what each was then
#[derive(Clone, Serialize, Deserialize)]
pub struct Link {
    pub cipher: String,
    pub revision: String,
    pub raw: Value,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub sealed: bool,
    /// How many times a sealed note has been sent or taken: a note older than this is a copy the server kept, and never taken
    #[serde(default)]
    pub seal: u64,
    pub parts: Vec<PartRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartRef {
    pub kind: String,
    pub id: String,
    pub hash: String,
}

/// A token from a login, kept in memory only
pub struct Access {
    pub token: Zeroizing<String>,
    pub until_ms: i64,
}

pub struct Input {
    pub connection: Connection,
    pub access: Option<Access>,
    pub links: Vec<Link>,
    /// Every item that may sync: the service leaves out the never list, and the run checks again
    pub locals: Vec<Local>,
    /// The vault's own key for sealed notes, and its id
    pub sealing: Option<(Key, String)>,
    /// A person said that many items gone from the server, or arrived, may go on
    pub accept: bool,
    /// Read `/sync` whatever the revision date says: at each opening
    pub full: bool,
    pub now_ms: i64,
    /// Items of the vault that did not read, and shards of links that did not: with either, nothing is asked of the server. An
    /// item that is not there reads as removed, and links that are not there read as an account never seen.
    pub local_unreadable: usize,
    pub links_unreadable: bool,
}

/// A vault item to write
pub struct Write {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub meta: Value,
    pub approval: Approval,
    pub value: Value,
}

impl std::fmt::Debug for Write {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Write({}.{})", self.kind, self.id)
    }
}

/// What a vault item must still be for a group to apply: its hash at the start of the run, or absent
#[derive(Debug, PartialEq, Eq)]
pub struct Basis {
    pub kind: String,
    pub id: String,
    pub hash: Option<String>,
}

/// Writes that go together, with the cipher's link after them (None: no longer linked)
pub struct Group {
    pub cipher: String,
    pub link: Option<Link>,
    pub writes: Vec<Write>,
    pub basis: Vec<Basis>,
}

#[derive(Default, Serialize)]
pub struct Report {
    pub sent: usize,
    pub received: usize,
    pub created_there: usize,
    pub created_here: usize,
    pub trashed_there: usize,
    pub trashed_here: usize,
    pub restored: usize,
    pub conflicts: usize,
    /// Items without a key of their own that were given one
    pub rekeyed: usize,
    /// Cards, identities, SSH keys, plain notes and newer types: left alone
    pub left_alone: usize,
    /// An organization's: never read, never shown as the person's
    pub organization: usize,
    pub sealed_elsewhere: usize,
    /// Items with something wrong, each with a stable word: the first few, since the count is what matters and a server may send any
    /// number. This goes into the connection's item, and a long list in it would lose the connection (the review's M8).
    pub refused: Vec<Value>,
    pub refused_total: usize,
}

impl Report {
    fn refuse(&mut self, what: &str, why: &str) {
        self.refused_total += 1;
        if self.refused.len() < MOST_REFUSED {
            self.refused.push(json!({"item": what, "why": why}));
        }
    }
}

pub struct Outcome {
    pub problem: Option<Problem>,
    pub access: Option<Access>,
    pub refresh: Option<Zeroizing<String>>,
    pub revision: Option<u64>,
    pub groups: Vec<Group>,
    pub report: Report,
    /// What this run took off the server's side or put on it, for the window's sum (0 for a person's own acceptance and for the first
    /// pull of an account)
    pub churn: u32,
    /// A person accepted what had gone: the window starts again
    pub accepted: bool,
    /// Tombstones of the vault past their days, whose cipher is in the server's trash or was never there
    pub purge: Vec<Basis>,
}

// --- Logging in ------------------------------------------------------------------------------------------------------

struct Session<'a> {
    http: &'a mut dyn Http,
    server: Server,
    connection: &'a Connection,
    access: Option<Access>,
    refresh: Option<Zeroizing<String>>,
    now_ms: i64,
    renewed: bool,
}

impl Session<'_> {
    /// A token, by the API key (no hash goes anywhere) or the refresh token; never the master password, which is not kept
    fn login(&mut self) -> Result<(), Problem> {
        let connection = self.connection;
        let credentials = match (&connection.api_key, &self.refresh) {
            (Some(key), _) => Credentials::ApiKey { id: &key.id, secret: &key.secret },
            (None, Some(refresh)) => Credentials::Refresh(refresh),
            (None, None) => return Err(Problem::LoginExpired),
        };
        let token = api::token(self.http, &self.server, &connection.email, &connection.device, &credentials, &Extra::default(), Some(&connection.kdf))?;
        if let Some(protected) = &token.protected_key
            && *protected != connection.protected_key
        {
            return Err(Problem::KeysChanged);
        }
        if api::token_subject(&token.access).is_some_and(|subject| !subject.eq_ignore_ascii_case(&connection.user_id)) {
            return Err(Problem::IdentityChanged);
        }
        if let Some(refresh) = token.refresh {
            self.refresh = Some(refresh);
        }
        self.access = Some(Access { token: token.access, until_ms: self.now_ms + (token.expires_in as i64 - 60) * 1000 });
        Ok(())
    }

    /// An API call, logging in first if need be, and once more if the token ran out on the way
    fn call<T>(&mut self, mut what: impl FnMut(&mut Api<'_>) -> Result<T, Expired>) -> Result<T, Problem> {
        loop {
            if self.access.as_ref().is_none_or(|access| access.until_ms <= self.now_ms) {
                self.login()?;
            }
            let token = self.access.as_ref().map(|access| access.token.clone()).unwrap_or_default();
            let mut api = Api { http: self.http, server: &self.server, access: &token };
            match what(&mut api) {
                Ok(value) => return Ok(value),
                Err(Expired::Problem(problem)) => return Err(problem),
                Err(Expired::Token) if !self.renewed => {
                    self.renewed = true;
                    self.access = None;
                }
                Err(Expired::Token) => return Err(Problem::LoginExpired),
            }
        }
    }
}

// --- Connecting ------------------------------------------------------------------------------------------------------

/// What a connection carries between the steps a person takes: derived once, kept in memory until it is finished, given up,
/// or the vault closes. The master password is not in it.
pub struct Pending {
    pub server: Server,
    pub email: String,
    pub kdf: Kdf,
    pub stretched: SymKey,
    /// The master password hash, for a password login and the email code; none with an API key
    pub hash: Option<Zeroizing<String>>,
    pub api_key: Option<ApiKey>,
    pub device: String,
}

/// How the master key is made. The service has Argon2 made in a child of its own, whose memory goes with it; a test makes it here.
pub type Derive<'a> = dyn Fn(&[u8], &str, &Kdf) -> Result<Zeroizing<[u8; 32]>, crypto::KdfProblem> + 'a;

/// Prelogin, the bounds, the person's confirmation of a low PBKDF2 or a large Argon2, the pin of a reconnection, and the KDF. Slow:
/// off the loop. `confirmed` is the number the person was shown and agreed to: the rounds of a PBKDF2, the MiB of an Argon2.
#[allow(clippy::too_many_arguments)]
pub fn prepare(http: &mut dyn Http, derive: &Derive<'_>, server: Server, email: &str, password: &[u8], confirmed: Option<u32>, pinned: Option<&Kdf>, api_key: Option<ApiKey>, device: String) -> Result<Pending, Problem> {
    let kdf = api::prelogin(http, &server, email)?;
    if pinned.is_some_and(|pinned| kdf.below(pinned)) {
        return Err(Problem::Kdf(crypto::KdfProblem::BelowPin));
    }
    if kdf.check().map_err(Problem::Kdf)? && confirmed != Some(kdf.iterations) {
        return Err(Problem::KdfConfirm(kdf.iterations));
    }
    let memory = kdf.memory.unwrap_or(0);
    if kdf.kind == 1 && memory > super::bounds::ARGON2_CONFIRM_ABOVE_MIB && confirmed != Some(memory) {
        return Err(Problem::KdfMemoryConfirm(memory));
    }
    let master = derive(password, email, &kdf).map_err(Problem::Kdf)?;
    let stretched = crypto::stretch(&master);
    // With an API key no hash is made at all: a server that lowered the settings gets nothing to attack offline
    let hash = if api_key.is_none() { Some(crypto::master_hash(&master, password)) } else { None };
    Ok(Pending { server, email: crypto::normalize_email(email), kdf, stretched, hash, api_key, device })
}

/// The login, the user key unwrapped, and `/sync`'s word on the account, which has to agree. Nothing is kept if any fails.
pub fn finish(http: &mut dyn Http, pending: &Pending, extra: &Extra<'_>) -> Result<(Connection, Access), Problem> {
    let credentials = match (&pending.api_key, &pending.hash) {
        (Some(key), _) => Credentials::ApiKey { id: &key.id, secret: &key.secret },
        (None, Some(hash)) => Credentials::Password(hash),
        (None, None) => return Err(Problem::LoginExpired),
    };
    let token = api::token(http, &pending.server, &pending.email, &pending.device, &credentials, extra, Some(&pending.kdf))?;
    let protected = token.protected_key.clone().ok_or(Problem::NoKey)?;
    let user = api::unwrap_user_key(&protected, &pending.stretched)?;
    let now_ms = now_ms();
    let access = Access { token: token.access.clone(), until_ms: now_ms + (token.expires_in as i64 - 60) * 1000 };
    let mut api = Api { http, server: &pending.server, access: &access.token };
    let sync = api.sync().map_err(|expired| match expired {
        Expired::Token => Problem::LoginExpired,
        Expired::Problem(problem) => problem,
    })?;
    let user_id = json::path(&sync, &["profile", "id"]).and_then(Value::as_str).filter(|id| api::is_uuid(id)).ok_or(Problem::BadResponse)?.to_string();
    if api::token_subject(&access.token).is_some_and(|subject| !subject.eq_ignore_ascii_case(&user_id)) {
        return Err(Problem::IdentityChanged);
    }
    let connection = Connection {
        address: pending.server.address.clone(),
        identity: pending.server.identity.clone(),
        api: pending.server.api.clone(),
        email: pending.email.clone(),
        user_id,
        user_key: seal::hex(user.bytes()),
        protected_key: protected,
        kdf: pending.kdf,
        device: pending.device.clone(),
        api_key: pending.api_key.clone(),
        refresh: token.refresh.as_ref().map(|refresh| refresh.to_string()),
        remember: token.remember.as_ref().map(|remember| remember.to_string()),
        ca: None,
        pin: None,
        choices: BTreeMap::new(),
        last_revision: None,
        last_success_ms: None,
        last: None,
        waiting_since_ms: None,
        recent: Vec::new(),
    };
    check_account(&sync, &connection)?;
    Ok((connection, access))
}

/// The identifier a server knows this device by: a random UUID, made once for an account and kept, since a server counts a new
/// one as a new device
pub fn new_device_id() -> String {
    let mut bytes = [0u8; 16];
    seal::random(&mut bytes);
    bytes[6] = bytes[6] & 0x0f | 0x40;
    bytes[8] = bytes[8] & 0x3f | 0x80;
    let hex = seal::hex(&bytes);
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|since| since.as_millis() as i64).unwrap_or(0)
}

/// What `/sync` says of the account, before any cipher is read (spec 5.2): the same account, key and settings, and none of
/// the newer encryption's signs
pub fn check_account(sync: &Value, connection: &Connection) -> Result<(), Problem> {
    let profile = json::some(sync, "profile").ok_or(Problem::BadResponse)?;
    if json::text(profile, "id").is_none_or(|id| !id.eq_ignore_ascii_case(&connection.user_id)) {
        return Err(Problem::IdentityChanged);
    }
    if json::text(profile, "email").is_none_or(|email| crypto::normalize_email(email) != connection.email) {
        return Err(Problem::IdentityChanged);
    }
    if json::text(profile, "privateKey").is_some_and(|key| key.starts_with("7.")) {
        return Err(Problem::NewerEncryption);
    }
    for keys in [json::some(profile, "accountKeys"), json::some(sync, "accountKeys")].into_iter().flatten() {
        if json::some(keys, "signatureKeyPair").is_some() || json::some(keys, "securityState").is_some() {
            return Err(Problem::NewerEncryption);
        }
    }
    let decryption = json::some(sync, "userDecryption");
    if decryption.is_some_and(|d| json::some(d, "v2UpgradeToken").is_some()) {
        return Err(Problem::NewerEncryption);
    }
    match json::text(profile, "key") {
        Some(key) if key == connection.protected_key => {}
        Some(key) if key.starts_with("7.") => return Err(Problem::NewerEncryption),
        _ => return Err(Problem::KeysChanged),
    }
    if let Some(unlock) = decryption.and_then(|d| json::some(d, "masterPasswordUnlock")) {
        for name in ["masterKeyEncryptedUserKey", "masterKeyWrappedUserKey"] {
            if json::text(unlock, name).is_some_and(|key| key != connection.protected_key) {
                return Err(Problem::KeysChanged);
            }
        }
        if let Some(settings) = json::some(unlock, "kdf") {
            let number = |name: &str| json::get(settings, name).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok());
            let kdf = Kdf {
                kind: number("kdfType").and_then(|n| u8::try_from(n).ok()).ok_or(Problem::BadResponse)?,
                iterations: number("iterations").ok_or(Problem::BadResponse)?,
                memory: number("memory"),
                parallelism: number("parallelism"),
            };
            if !kdf.same(&connection.kdf) {
                return Err(Problem::Kdf(crypto::KdfProblem::BelowPin));
            }
        }
        if !crypto::salt_is_ours(json::text(unlock, "salt"), &connection.email) {
            return Err(Problem::SaltChanged);
        }
    }
    let ciphers = json::get(sync, "ciphers").and_then(Value::as_array).ok_or(Problem::BadResponse)?;
    if ciphers.iter().any(cipher::newer_encryption) {
        return Err(Problem::NewerEncryption);
    }
    Ok(())
}

// --- The run -----------------------------------------------------------------------------------------------------------

/// What the run is working on: its own copy of the links as they change, the vault's items, and what it has decided
struct Run<'a> {
    user: SymKey,
    user_id: String,
    sealing: Option<(Key, String)>,
    locals: HashMap<(String, String), &'a Local>,
    links: BTreeMap<String, Link>,
    /// Vault items some link carries, as the run goes
    linked: HashSet<(String, String)>,
    groups: Vec<Group>,
    report: Report,
    now_ms: i64,
    /// The passwords the vault holds, and the ones it had: no name, user name or address that arrives is allowed to be one
    secrets: HashSet<String>,
    /// Names (an EncString) that more than one cipher of the answer has: a copy a server made
    copies: HashSet<String>,
}

fn key_of(local: &Local) -> (String, String) {
    (local.kind.clone(), local.id.clone())
}

fn write_of(local: &Local, value: Value) -> Write {
    Write { kind: local.kind.clone(), id: local.id.clone(), name: map::name_of(&local.kind, &value), meta: map::meta_of(&local.kind, &value), approval: local.approval, value }
}

fn new_write(kind: &str, id: &str, value: Value) -> Write {
    Write { kind: kind.to_string(), id: id.to_string(), name: map::name_of(kind, &value), meta: map::meta_of(kind, &value), approval: Approval::WhileOpen, value }
}

fn hash_of_write(write: &Write) -> String {
    map::hash(&Local { kind: write.kind.clone(), id: write.id.clone(), name: write.name.clone(), meta: write.meta.clone(), approval: write.approval, value: write.value.clone() })
}

fn tombstone(local: &Local, now_ms: i64) -> Write {
    let mut value = local.value.clone();
    if let Some(object) = value.as_object_mut() {
        object.insert("deletedAt".into(), json!(now_ms));
        object.insert("updatedAt".into(), json!(now_ms));
    }
    write_of(local, value)
}

/// A kind sealed notes may carry: any that may sync. The never list stays out, whatever a note says.
fn may_be_synced(local: &Local) -> bool {
    kinds::may_sync(&local.kind) && local.meta.get("exportable") != Some(&Value::Bool(false))
}

/// The passwords of the vault's own, and the ones they had
fn secrets_of(locals: &[Local]) -> HashSet<String> {
    let mut secrets = HashSet::new();
    for local in locals.iter().filter(|local| local.kind == KIND_PASSWORD && !map::deleted(local)) {
        let Some(password) = map::password(local) else { continue };
        for text in std::iter::once(password.password.as_str()).chain(password.history.iter().map(|old| old.password.as_str())) {
            if text.len() >= 4 {
                secrets.insert(text.to_string());
            }
        }
    }
    secrets
}

impl Run<'_> {
    fn local(&self, part: &PartRef) -> Option<&Local> {
        self.locals.get(&(part.kind.clone(), part.id.clone())).copied()
    }

    fn basis(&self, part: &PartRef) -> Basis {
        Basis { kind: part.kind.clone(), id: part.id.clone(), hash: self.local(part).map(map::hash) }
    }

    fn now_ns(&self) -> i128 {
        i128::from(self.now_ms) * 1_000_000
    }

    /// The content this link's live parts make, over the server's. `seal` is the counter a sealed note carries, if it is one.
    fn compose(&self, old: Option<&Content>, parts: &[PartRef], seal: Option<u64>) -> Result<Content, map::Unmapped> {
        let live: Vec<&Local> = parts.iter().filter_map(|part| self.local(part)).filter(|local| !map::deleted(local)).collect();
        if let Some(counter) = seal {
            let local = live.first().ok_or(map::Unmapped::Unreadable)?;
            let (key, vault) = self.sealing.as_ref().ok_or(map::Unmapped::Unreadable)?;
            let text = map::sealed_text(local, key, vault, counter)?;
            return map::compose(old, &Parts::default(), Some(&text));
        }
        let find = |kind: &str| live.iter().copied().find(|local| local.kind == kind);
        let gone: Vec<&str> = parts.iter().map(|part| part.kind.as_str()).filter(|kind| find(kind).is_none()).collect();
        map::compose(old, &Parts { password: find(KIND_PASSWORD), account: find(KIND_ACCOUNT), codes: find(KIND_BACKUP_CODES), sealed: None, gone }, None)
    }

    fn part_refs(&self, parts: &[PartRef]) -> Vec<PartRef> {
        parts.iter().filter_map(|part| self.local(part).filter(|local| !map::deleted(local)).map(|local| PartRef { kind: part.kind.clone(), id: part.id.clone(), hash: map::hash(local) })).collect()
    }

    fn link_from(&self, raw: Value, parts: Vec<PartRef>, sealed: bool, seal: u64) -> Option<Link> {
        let cipher = json::text(&raw, "id")?.to_string();
        let revision = json::text(&raw, "revisionDate")?.to_string();
        let deleted = json::some(&raw, "deletedDate").is_some();
        Some(Link { cipher, revision, raw, deleted, sealed, seal, parts })
    }

    fn done(&mut self, cipher: &str, link: Option<Link>, writes: Vec<Write>, basis: Vec<Basis>) {
        match &link {
            Some(link) => {
                for part in &link.parts {
                    self.linked.insert((part.kind.clone(), part.id.clone()));
                }
                self.links.insert(cipher.to_string(), link.clone());
            }
            None => {
                self.links.remove(cipher);
            }
        }
        self.groups.push(Group { cipher: cipher.to_string(), link, writes, basis });
    }

    /// The vault's version over the server's, starting from its last raw JSON. A stale copy turns into a conflict.
    fn push(&mut self, session: &mut Session<'_>, link: &Link, opened: &Opened) -> Result<(), Problem> {
        let seal = link.sealed.then_some(link.seal + 1);
        let new = match self.compose(Some(&opened.content), &link.parts, seal) {
            Ok(new) => new,
            Err(why) => {
                self.report.refuse(&link.cipher, unmapped_word(why));
                return Ok(());
            }
        };
        let parts = self.part_refs(&link.parts);
        if new == opened.content {
            let link = Link { parts, ..link.clone() };
            self.done(&link.cipher.clone(), Some(link), Vec::new(), Vec::new());
            return Ok(());
        }
        let body = match cipher::update_body(&link.raw, &opened.content, &new, &self.user_id, &self.user) {
            Ok(body) => body,
            Err(cipher::UpdateRefused::TooLong) => {
                self.report.refuse(&link.cipher, "too-long");
                return Ok(());
            }
            Err(cipher::UpdateRefused::Unreadable) => {
                self.report.refuse(&link.cipher, "unreadable");
                return Ok(());
            }
        };
        match session.call(|api| api.update(&link.cipher, &body))? {
            Ok(raw) => {
                self.report.sent += 1;
                match self.link_from(raw, parts, link.sealed, seal.unwrap_or(0)) {
                    Some(new_link) if new_link.cipher == link.cipher => self.done(&link.cipher, Some(new_link), Vec::new(), Vec::new()),
                    _ => self.report.refuse(&link.cipher, "bad-response"),
                }
            }
            Err(_) => {
                // A refusal: stale (the server's copy moved), or anything else, told apart by fetching it
                let fetched = session.call(|api| api.fetch(&link.cipher))?;
                let moved = json::text(&fetched, "revisionDate").is_some_and(|revision| revision != link.raw_revision());
                if moved && let Ok(theirs) = cipher::open(&fetched, &self.user) {
                    self.conflict(link, &theirs, fetched);
                } else {
                    self.report.refuse(&link.cipher, "server-refused");
                }
            }
        }
        Ok(())
    }

    /// Changed on both sides: the server's version becomes new items of the vault, marked, carried by this cipher, and the
    /// vault's own go to the server as a new item of their own
    fn conflict(&mut self, link: &Link, theirs: &Opened, raw: Value) {
        let was = cipher::open(&link.raw, &self.user).ok();
        if !link.sealed && map::exposes(&theirs.content, was.as_ref().map(|was| &was.content), &self.secrets) {
            self.report.refuse(&link.cipher, "fields-swapped");
            return;
        }
        self.report.conflicts += 1;
        let (writes, parts, counter) = self.items_from(&theirs.content, theirs.uri_unproven, true);
        let link = Link { cipher: link.cipher.clone(), revision: theirs.revision.clone(), raw, deleted: theirs.deleted, sealed: link.sealed, seal: counter, parts };
        // The vault's own stay, unlinked: the end of the run sends them as new items
        for part in &self.links.get(&link.cipher).map(|old| old.parts.clone()).unwrap_or_default() {
            self.linked.remove(&(part.kind.clone(), part.id.clone()));
        }
        let basis = writes.iter().map(|write| Basis { kind: write.kind.clone(), id: write.id.clone(), hash: None }).collect();
        self.done(&link.cipher.clone(), Some(link), writes, basis);
    }

    /// New vault items from a cipher's content: a password, an authenticator account, backup codes, or the item a sealed note
    /// holds. `conflict` marks them as the server's version, and gives a sealed one an id of its own. The sealed counter comes back.
    fn items_from(&mut self, content: &Content, uri_unproven: bool, conflict: bool) -> (Vec<Write>, Vec<PartRef>, u64) {
        let mut writes = Vec::new();
        let mut counter = 0;
        let fresh = || crate::items::new_id().unwrap_or_else(|_| seal::random_hex(16));
        let mark = |mut value: Value| {
            if conflict && let Some(object) = value.as_object_mut() {
                object.insert("syncConflict".into(), json!({"from": "server"}));
            }
            value
        };
        if map::is_sealed_note(content) {
            if let (Some((key, vault)), Some(notes)) = (&self.sealing, &content.notes)
                && let Some((local, found)) = map::open_sealed(notes, key, vault)
            {
                counter = found;
                let (id, meta) = if conflict { (fresh(), json!({"syncConflict": "server"})) } else { (local.id.clone(), local.meta.clone()) };
                writes.push(Write { kind: local.kind.clone(), id, name: local.name.clone(), meta, approval: local.approval, value: local.value.clone() });
            }
        } else if map::is_codes_note(content) {
            let id = fresh();
            writes.push(new_write(KIND_BACKUP_CODES, &id, mark(serde_json::to_value(map::pull_codes(content, None, self.now_ms, &id)).unwrap_or_default())));
        } else if content.kind == LOGIN {
            let has_login = content.password.is_some() || content.username.is_some() || content.uri.is_some() || content.totp.is_none();
            if has_login {
                let id = fresh();
                writes.push(new_write(KIND_PASSWORD, &id, mark(serde_json::to_value(map::pull_password(content, None, self.now_ms, &id, uri_unproven)).unwrap_or_default())));
            }
            let id = fresh();
            if let Some(account) = map::pull_account(content, None, self.now_ms, &id) {
                writes.push(new_write(KIND_ACCOUNT, &id, mark(serde_json::to_value(account).unwrap_or_default())));
            }
        }
        let parts = writes.iter().map(|write| PartRef { kind: write.kind.clone(), id: write.id.clone(), hash: hash_of_write(write) }).collect();
        (writes, parts, counter)
    }

    /// The server's version into the vault's items, keeping what the server does not hold
    fn pull(&mut self, link: &Link, opened: &Opened, raw: Value) {
        let content = &opened.content;
        let mut writes = Vec::new();
        let mut parts = Vec::new();
        let mut basis = Vec::new();
        let mut counter = link.seal;
        if link.sealed {
            let opened_item = match (&self.sealing, &content.notes) {
                (Some((key, vault)), Some(notes)) => map::open_sealed(notes, key, vault),
                _ => None,
            };
            let Some((item, found)) = opened_item.filter(|(item, _)| link.parts.first().is_some_and(|part| part.kind == item.kind && part.id == item.id)) else {
                self.report.refuse(&link.cipher, "unreadable");
                return;
            };
            // A note older than the one this vault sent last is a copy the server kept, and one with no counter replaces nothing
            if found < link.seal || found == 0 {
                self.report.refuse(&link.cipher, "rolled-back");
                return;
            }
            counter = found;
            for part in &link.parts {
                basis.push(self.basis(part));
            }
            let mut write = Write { kind: item.kind.clone(), id: item.id.clone(), name: item.name.clone(), meta: item.meta.clone(), approval: item.approval, value: item.value.clone() };
            if let Some(local) = link.parts.first().and_then(|part| self.local(part)) {
                // The person's setting for how often it asks is never lowered by a server
                write.approval = stricter(local.approval, item.approval);
            }
            parts.push(PartRef { kind: write.kind.clone(), id: write.id.clone(), hash: hash_of_write(&write) });
            // The same note again, only dated later: nothing to write
            if found != link.seal || self.local(&parts[0]).is_none_or(|local| map::hash(local) != parts[0].hash) {
                writes.push(write);
            }
        } else {
            // A secret moved into another field, or another field's text into the secret: the one thing a server can do to an item
            // with an item key (BW05). The vault's version stays, and the person is told.
            let old_totp = link.parts.iter().find(|part| part.kind == KIND_ACCOUNT).and_then(|part| self.local(part)).and_then(map::account).and_then(|account| map::totp_text(&account));
            let swapped = content.kind == LOGIN
                && link
                    .parts
                    .iter()
                    .find(|part| part.kind == KIND_PASSWORD)
                    .and_then(|part| self.local(part))
                    .and_then(map::password)
                    .is_some_and(|old| map::password_moved(&old, old_totp.as_ref().map(|totp| totp.as_str()), content));
            let was = cipher::open(&link.raw, &self.user).ok();
            if swapped || map::exposes(content, was.as_ref().map(|was| &was.content), &self.secrets) {
                self.report.refuse(&link.cipher, "fields-swapped");
                return;
            }
            let find = |kind: &str| link.parts.iter().find(|part| part.kind == kind);
            let mut add = |kind: &str, make: &dyn Fn(Option<&Local>, &str) -> Option<Value>, this: &Self| {
                let existing = find(kind).and_then(|part| this.local(part));
                let id = find(kind).map(|part| part.id.clone()).unwrap_or_else(|| crate::items::new_id().unwrap_or_default());
                if let Some(part) = find(kind) {
                    basis.push(this.basis(part));
                }
                match make(existing, &id) {
                    Some(value) => {
                        let write = match existing {
                            Some(local) => write_of(local, value),
                            None => new_write(kind, &id, value),
                        };
                        parts.push(PartRef { kind: write.kind.clone(), id: write.id.clone(), hash: hash_of_write(&write) });
                        writes.push(write);
                    }
                    // The server's cipher has no such part any more: the vault's goes to its trash
                    None => {
                        if let Some(local) = existing.filter(|local| !map::deleted(local)) {
                            writes.push(tombstone(local, this.now_ms));
                        }
                    }
                }
            };
            let now = self.now_ms;
            if map::is_codes_note(content) {
                add(KIND_BACKUP_CODES, &|existing, id| Some(serde_json::to_value(map::pull_codes(content, existing.and_then(map::codes).as_ref(), now, id)).unwrap_or_default()), self);
            } else if content.kind == LOGIN {
                let has_login = content.password.is_some() || content.username.is_some() || content.uri.is_some() || content.totp.is_none() || find(KIND_PASSWORD).is_some();
                if has_login {
                    add(KIND_PASSWORD, &|existing, id| Some(serde_json::to_value(map::pull_password(content, existing.and_then(map::password).as_ref(), now, id, opened.uri_unproven)).unwrap_or_default()), self);
                }
                // A totp this vault cannot read leaves the vault's authenticator account as it is: it is not deleted
                let readable = content.totp.as_deref().is_some_and(|totp| map::read_totp(totp, &content.name).is_some());
                if readable || content.totp.is_none() {
                    add(KIND_ACCOUNT, &|existing, id| map::pull_account(content, existing.and_then(map::account).as_ref(), now, id).map(|a| serde_json::to_value(a).unwrap_or_default()), self);
                } else if let Some(part) = find(KIND_ACCOUNT) {
                    parts.push(part.clone());
                }
            }
        }
        self.report.received += 1;
        let link = Link { cipher: link.cipher.clone(), revision: opened.revision.clone(), raw, deleted: opened.deleted, sealed: link.sealed, seal: counter, parts };
        self.done(&link.cipher.clone(), Some(link), writes, basis);
    }

    fn create(&mut self, session: &mut Session<'_>, locals: Vec<&Local>, sealed: bool) -> Result<(), Problem> {
        let parts: Vec<PartRef> = locals.iter().map(|local| PartRef { kind: local.kind.clone(), id: local.id.clone(), hash: map::hash(local) }).collect();
        let content = match self.compose(None, &parts, sealed.then_some(1)) {
            Ok(content) => content,
            Err(why) => {
                for local in &locals {
                    self.report.refuse(&format!("{}.{}", local.kind, local.id), unmapped_word(why));
                }
                return Ok(());
            }
        };
        let body = match cipher::create_body(&content, &self.user_id, &self.user) {
            Ok(body) => body,
            Err(_) => {
                for local in &locals {
                    self.report.refuse(&format!("{}.{}", local.kind, local.id), "too-long");
                }
                return Ok(());
            }
        };
        let raw = session.call(|api| api.create(&body))?;
        self.report.created_there += 1;
        match self.link_from(raw, parts, sealed, u64::from(sealed)) {
            Some(link) if api::is_uuid(&link.cipher) && !self.links.contains_key(&link.cipher) => self.done(&link.cipher.clone(), Some(link), Vec::new(), Vec::new()),
            _ => self.report.refuse("created", "bad-response"),
        }
        Ok(())
    }

    /// A cipher with no key of its own that is about to be linked is given one (spec 6.3): without it, a server could put another
    /// item's password in it, and nothing here could tell. Its answer, if it is good, is the cipher as it is now; otherwise the cipher
    /// stays as it was, and a change to its password or address is a conflict, never taken.
    fn rekey(&mut self, session: &mut Session<'_>, raw: &Value, opened: &Opened) -> Result<Option<Value>, Problem> {
        if opened.has_key {
            return Ok(None);
        }
        let Ok(body) = cipher::rekeyed_body(raw, &self.user_id, &self.user) else { return Ok(None) };
        let id = opened.id.clone();
        match session.call(|api| api.update(&id, &body))? {
            Ok(new) => {
                let good = cipher::open(&new, &self.user).is_ok_and(|now| now.has_key && now.id == id && now.content == opened.content);
                if good {
                    self.report.rekeyed += 1;
                    Ok(Some(new))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }
}

fn stricter(a: Approval, b: Approval) -> Approval {
    let rank = |approval: Approval| match approval {
        Approval::Never => 0,
        Approval::WhileOpen => 1,
        Approval::Ask => 2,
    };
    if rank(a) >= rank(b) { a } else { b }
}

fn unmapped_word(why: map::Unmapped) -> &'static str {
    match why {
        map::Unmapped::TooLargeToSeal => "too-large-to-seal",
        map::Unmapped::NoLink => "no-link",
        map::Unmapped::Unreadable => "unreadable",
    }
}

impl Link {
    fn raw_revision(&self) -> &str {
        json::text(&self.raw, "revisionDate").unwrap_or(&self.revision)
    }

    fn kind(&self) -> Option<u64> {
        json::number(&self.raw, "type")
    }
}

/// One run, from the token to the last write. A problem stops it where it is, and what was done before still applies.
pub fn run(http: &mut dyn Http, input: Input) -> Outcome {
    let Input { connection, access, links, mut locals, sealing, accept, full, now_ms, local_unreadable, links_unreadable } = input;
    let mut outcome = Outcome { problem: None, access: None, refresh: None, revision: None, groups: Vec::new(), report: Report::default(), churn: 0, accepted: accept, purge: Vec::new() };
    // What is not known is not guessed: an item that did not read is not one that was removed, and links that did not read are
    // not an account never seen
    if links_unreadable {
        outcome.problem = Some(Problem::LinksUnreadable);
    } else if local_unreadable > 0 {
        outcome.problem = Some(Problem::LocalUnreadable(local_unreadable));
    } else if connection.key().is_none() {
        outcome.problem = Some(Problem::CannotOpen);
    }
    if outcome.problem.is_none()
        && let Some(user) = connection.key()
    {
        let mut session = Session {
            http,
            server: connection.server(),
            connection: &connection,
            access,
            refresh: connection.refresh.clone().map(Zeroizing::new),
            now_ms,
            renewed: false,
        };
        let secrets = secrets_of(&locals);
        let mut run = Run {
            user,
            user_id: connection.user_id.clone(),
            sealing,
            locals: locals.iter().filter(|local| may_be_synced(local)).map(|local| (key_of(local), local)).collect(),
            links: links.into_iter().map(|link| (link.cipher.clone(), link)).collect(),
            linked: HashSet::new(),
            groups: Vec::new(),
            report: Report::default(),
            now_ms,
            secrets,
            copies: HashSet::new(),
        };
        for link in run.links.values() {
            for part in &link.parts {
                run.linked.insert((part.kind.clone(), part.id.clone()));
            }
        }
        let result = steps(&mut session, &mut run, &connection, accept, full, &mut outcome);
        outcome.problem = result.err();
        outcome.access = session.access;
        outcome.refresh = session.refresh;
        outcome.groups = run.groups;
        outcome.report = run.report;
    }
    // Nothing of the vault's is kept past the run: the run's copies of its items are overwritten here, not left to be freed
    for local in &mut locals {
        map::wipe(local);
    }
    outcome
}

/// A cipher's name as an EncString, if it has one: what two ciphers of one answer must not share
fn name_of(raw: &Value) -> Option<&str> {
    json::text(raw, "name").filter(|name| !name.is_empty())
}

fn steps(session: &mut Session<'_>, run: &mut Run<'_>, connection: &Connection, accept: bool, full: bool, outcome: &mut Outcome) -> Result<(), Problem> {
    // A cheap look first: nothing changed there, and nothing here, means nothing to do (a hint only; each opening reads all)
    let unchanged_here = run.links.values().all(|link| link.parts.iter().all(|part| run.local(part).is_some_and(|local| map::hash(local) == part.hash)))
        && run.locals.values().all(|local| run.linked.contains(&key_of(local)) || map::deleted(local) || connection.choice_for_new(&local.kind, &local.id, local.approval) == "off");
    let date = session.call(|api| api.revision_date())?;
    if !full && unchanged_here && date.is_some() && date == connection.last_revision {
        outcome.revision = date;
        return Ok(());
    }
    let sync = session.call(|api| api.sync())?;
    check_account(&sync, connection)?;
    // Taken before /sync: a change made between the two is seen next time, never missed
    outcome.revision = date;
    let ciphers = json::get(&sync, "ciphers").and_then(Value::as_array).ok_or(Problem::BadResponse)?;
    let mut by_id: HashMap<String, &Value> = HashMap::new();
    let mut names: HashMap<&str, usize> = HashMap::new();
    for raw in ciphers {
        let Some(id) = json::text(raw, "id") else { continue };
        if by_id.insert(id.to_ascii_lowercase(), raw).is_some() {
            return Err(Problem::BadResponse);
        }
        if let Some(name) = name_of(raw) {
            *names.entry(name).or_default() += 1;
        }
    }
    // An honest client never makes two ciphers with the same ciphertext, since every string has an IV of its own: a server that
    // copies one cipher under many ids is filling the vault, or moving a password (the review's H3 and M4)
    run.copies = names.iter().filter(|(_, count)| **count > 1).map(|(name, _)| name.to_string()).collect();

    // Items gone from the server, items trashed there, and items arriving: many at once, or a few each run for days, is a server
    // emptying or filling the account, or a damaged answer. The first pull of an account is the person's own doing and counts for
    // nothing.
    let gone: Vec<String> = run.links.keys().filter(|cipher| !by_id.contains_key(&cipher.to_ascii_lowercase())).cloned().collect();
    let trashed = run.links.values().filter(|link| !link.deleted).filter(|link| by_id.get(&link.cipher.to_ascii_lowercase()).is_some_and(|raw| json::some(raw, "deletedDate").is_some())).count();
    let first_pull = run.links.is_empty();
    let arriving = if first_pull {
        0
    } else {
        let known: HashSet<String> = run.links.keys().map(|cipher| cipher.to_ascii_lowercase()).collect();
        ciphers
            .iter()
            .filter(|raw| json::text(raw, "id").is_some_and(|id| !known.contains(&id.to_ascii_lowercase())))
            .filter(|raw| json::some(raw, "deletedDate").is_none() && cipher::sort(raw).is_ok())
            .filter(|raw| cipher::open(raw, &run.user).is_ok_and(|opened| opened.content.kind == LOGIN || map::is_sealed_note(&opened.content) || map::is_codes_note(&opened.content)))
            .count()
    };
    let removed = gone.len() + trashed;
    let churn = removed + arriving;
    if !accept {
        let links = run.links.len();
        let in_window = connection.recent_total(run.now_ms) + churn;
        let direction = |total: usize| if removed >= arriving { Problem::ManyGone(total) } else { Problem::ManyNew(total) };
        if removed > FEW || (removed > 1 && removed * 10 > links) || arriving > FEW {
            return Err(direction(churn));
        }
        if churn > 0 && in_window > (4 * FEW).max(links / 10) {
            return Err(direction(in_window));
        }
    }
    outcome.churn = u32::try_from(churn).unwrap_or(u32::MAX);

    for cipher in &gone {
        let Some(link) = run.links.get(cipher).cloned() else { continue };
        let mut writes = Vec::new();
        let mut basis = Vec::new();
        for part in &link.parts {
            if let Some(local) = run.local(part).filter(|local| !map::deleted(local) && !link.sealed) {
                writes.push(tombstone(local, run.now_ms));
                basis.push(run.basis(part));
            }
        }
        if !writes.is_empty() {
            run.report.trashed_here += 1;
        }
        run.done(cipher, None, writes, basis);
    }
    // Every cipher the vault knows
    let known: Vec<Link> = run.links.values().cloned().collect();
    for link in known {
        let Some(raw) = by_id.get(&link.cipher.to_ascii_lowercase()).map(|raw| (*raw).clone()) else { continue };
        one_link(session, run, connection, &link, raw)?;
    }
    // Ciphers new to the vault
    let linked_ciphers: HashSet<String> = run.links.keys().map(|cipher| cipher.to_ascii_lowercase()).collect();
    let mut fresh: Vec<&Value> = ciphers.iter().filter(|raw| json::text(raw, "id").is_some_and(|id| !linked_ciphers.contains(&id.to_ascii_lowercase()))).collect();
    fresh.sort_by_key(|raw| json::text(raw, "id").unwrap_or("").to_string());
    for raw in fresh {
        new_cipher(session, run, connection, raw)?;
    }
    // Vault items new to the server
    let mut unlinked: Vec<&Local> = run
        .locals
        .values()
        .copied()
        .filter(|local| !run.linked.contains(&key_of(local)) && !map::deleted(local) && connection.choice_for_new(&local.kind, &local.id, local.approval) != "off")
        .collect();
    unlinked.sort_by(|a, b| (&a.kind, &a.id).cmp(&(&b.kind, &b.id)));
    for local in unlinked {
        let choice = connection.choice_for_new(&local.kind, &local.id, local.approval);
        if local.kind == KIND_ACCOUNT && choice == "on" && attach(session, run, local)? {
            continue;
        }
        run.create(session, vec![local], choice == "sealed")?;
    }
    // Tombstones past their days go, once the server's copy is in its trash too (or never was)
    let mut tombstones: Vec<&Local> = run.locals.values().copied().filter(|local| map::deleted(local)).collect();
    tombstones.sort_by(|a, b| (&a.kind, &a.id).cmp(&(&b.kind, &b.id)));
    for local in tombstones {
        let old = local.value.get("deletedAt").and_then(Value::as_i64).is_some_and(|at| at < run.now_ms - KEPT_MS);
        let carried = run.links.values().find(|link| link.parts.iter().any(|part| part.kind == local.kind && part.id == local.id));
        if old && carried.is_none_or(|link| link.deleted) {
            outcome.purge.push(Basis { kind: local.kind.clone(), id: local.id.clone(), hash: Some(map::hash(local)) });
        }
    }
    Ok(())
}

/// This item is left as it is, and the run goes on with the others
fn stop(run: &mut Run<'_>, cipher: &str, why: &str) -> Result<(), Problem> {
    run.report.refuse(cipher, why);
    Ok(())
}

/// A cipher the vault knows, as the server has it now
fn one_link(session: &mut Session<'_>, run: &mut Run<'_>, connection: &Connection, link: &Link, raw: Value) -> Result<(), Problem> {
    let cipher = link.cipher.clone();
    // A type or an organization changed on a known id, or a key that was there is gone: stop for this item (spec 5.5)
    match cipher::sort(&raw) {
        Ok(kind) if Some(u64::from(kind)) == link.kind() => {}
        Err(Foreign::Organization) => return stop(run, &cipher, "moved-to-organization"),
        Err(Foreign::TooBig) => return stop(run, &cipher, "too-big"),
        _ => return stop(run, &cipher, "type-changed"),
    }
    if json::some(&link.raw, "key").is_some() && json::some(&raw, "key").is_none() {
        return stop(run, &cipher, "key-removed");
    }
    let (Some(then), Some(now)) = (cipher::parse_time(&link.revision), json::text(&raw, "revisionDate").and_then(cipher::parse_time)) else {
        return stop(run, &cipher, "unreadable");
    };
    if now < then {
        return stop(run, &cipher, "rolled-back");
    }
    // A date far ahead of the clock would be the item's last for good: nothing older could ever replace it
    if now > run.now_ns() + AHEAD_NS {
        return stop(run, &cipher, "future-date");
    }
    let opened = match cipher::open(&raw, &run.user) {
        Ok(opened) => opened,
        Err(_) => return stop(run, &cipher, "unreadable"),
    };
    let server_changed = now > then;
    // The person's choice moved it off, or between a Login and a sealed note: to the server's trash, unlinked
    let choices: Vec<&str> = link.parts.iter().map(|part| connection.choice(&part.kind, &part.id)).collect();
    if choices.contains(&"off") || choices.iter().any(|choice| (*choice == "sealed") != link.sealed) {
        if !opened.deleted {
            session.call(|api| api.trash(&cipher))?;
            run.report.trashed_there += 1;
        }
        run.done(&cipher, None, Vec::new(), Vec::new());
        return Ok(());
    }
    // Under another key than the one this cipher was linked under: a server can put another cipher's whole content here, wrapped key
    // and all (the review's H3). It is not taken for the item's new version; both are kept.
    if !cipher::same_key(&link.raw, &raw, &run.user) {
        run.conflict(link, &opened, raw);
        return Ok(());
    }
    // A cipher with no key of its own could be handed another's password or address, and nothing here could tell (the review's H2):
    // a change to either is a conflict, never taken. Every cipher is given a key when it is first linked, so this is the old ones that
    // could not be, an attachment's or a passkey's.
    if server_changed && !opened.has_key && !link.sealed {
        let before = cipher::open(&link.raw, &run.user).ok();
        if before.is_some_and(|before| before.content.password != opened.content.password || before.content.uri != opened.content.uri) {
            run.conflict(link, &opened, raw);
            return Ok(());
        }
    }
    let live: Vec<&PartRef> = link.parts.iter().filter(|part| run.local(part).is_some_and(|local| !map::deleted(local))).collect();
    let local_changed = link.parts.iter().any(|part| run.local(part).is_none_or(|local| map::deleted(local) || map::hash(local) != part.hash));
    if live.is_empty() {
        if server_changed && !opened.deleted {
            // Edited there after it went here: the edit is kept, and comes back
            run.report.restored += 1;
            run.pull(link, &opened, raw);
        } else if !opened.deleted {
            session.call(|api| api.trash(&cipher))?;
            run.report.trashed_there += 1;
            run.done(&cipher, Some(Link { deleted: true, ..link.clone() }), Vec::new(), Vec::new());
        } else if server_changed {
            run.done(&cipher, Some(Link { revision: opened.revision.clone(), raw, deleted: true, ..link.clone() }), Vec::new(), Vec::new());
        }
        return Ok(());
    }
    if opened.deleted {
        if !link.deleted && !local_changed {
            // Trashed there: to the vault's trash too
            let mut writes = Vec::new();
            let mut basis = Vec::new();
            for part in &live {
                if let Some(local) = run.local(part) {
                    if !link.sealed {
                        writes.push(tombstone(local, run.now_ms));
                    }
                    basis.push(run.basis(part));
                }
            }
            run.report.trashed_here += 1;
            run.done(&cipher, Some(Link { revision: opened.revision.clone(), raw, deleted: true, ..link.clone() }), writes, basis);
            return Ok(());
        }
        // Alive here, after an edit or a restore: out of the server's trash, then the vault's version
        let restored = session.call(|api| api.restore(&cipher))?;
        run.report.restored += 1;
        let Ok(reopened) = cipher::open(&restored, &run.user) else { return stop(run, &cipher, "unreadable") };
        let link = Link { revision: reopened.revision.clone(), raw: restored, deleted: false, ..link.clone() };
        return run.push(session, &link, &reopened);
    }
    match (server_changed, local_changed) {
        (false, false) => {
            // Nothing moved; the server's form of the cipher is kept, as a write's answer may leave out what a sync gives
            if raw != link.raw {
                run.done(&cipher, Some(Link { raw, ..link.clone() }), Vec::new(), Vec::new());
            }
        }
        // The next write starts from what the server just said, not from the answer to the last write (the review's M9)
        (false, true) => run.push(session, &Link { raw: raw.clone(), ..link.clone() }, &opened)?,
        (true, false) => run.pull(link, &opened, raw),
        (true, true) => {
            // Sent, and the answer lost with a lock: the server holds what this vault holds, and the two are one
            let same = if link.sealed {
                sealed_same(run, link, &opened)
            } else {
                run.compose(Some(&opened.content), &link.parts, None).is_ok_and(|mine| mine == opened.content)
            };
            if same {
                let parts = run.part_refs(&link.parts);
                let counter = if link.sealed { sealed_counter(run, &opened).unwrap_or(link.seal) } else { link.seal };
                run.done(&cipher, Some(Link { revision: opened.revision.clone(), raw, deleted: false, parts, seal: counter, ..link.clone() }), Vec::new(), Vec::new());
            } else if link.sealed && sealed_counter(run, &opened).is_some_and(|found| found < link.seal) {
                run.report.refuse(&cipher, "rolled-back");
            } else {
                run.conflict(link, &opened, raw);
            }
        }
    }
    Ok(())
}

/// What a sealed note holds, if it is this vault's
fn sealed_item(run: &Run<'_>, opened: &Opened) -> Option<(Local, u64)> {
    let (key, vault) = run.sealing.as_ref()?;
    map::open_sealed(opened.content.notes.as_deref()?, key, vault)
}

fn sealed_counter(run: &Run<'_>, opened: &Opened) -> Option<u64> {
    sealed_item(run, opened).map(|(_, counter)| counter)
}

/// The note on the server holds the same item as the vault does now
fn sealed_same(run: &Run<'_>, link: &Link, opened: &Opened) -> bool {
    let Some((theirs, _)) = sealed_item(run, opened) else { return false };
    link.parts.first().and_then(|part| run.local(part)).is_some_and(|mine| mine.kind == theirs.kind && mine.id == theirs.id && mine.name == theirs.name && mine.value == theirs.value)
}

/// A cipher the vault has not seen: the account's own Logins, backup codes and this vault's sealed notes come in; everything
/// else is counted and left alone
fn new_cipher(session: &mut Session<'_>, run: &mut Run<'_>, connection: &Connection, raw: &Value) -> Result<(), Problem> {
    let id = json::text(raw, "id").unwrap_or("").to_string();
    match cipher::sort(raw) {
        Ok(_) => {}
        Err(Foreign::Organization) => {
            run.report.organization += 1;
            return Ok(());
        }
        Err(Foreign::Damaged) | Err(Foreign::OldFormat) | Err(Foreign::TooBig) => {
            run.report.refuse(&id, "unreadable");
            return Ok(());
        }
        Err(_) => {
            run.report.left_alone += 1;
            return Ok(());
        }
    }
    let opened = match cipher::open(raw, &run.user) {
        Ok(opened) => opened,
        Err(_) => {
            run.report.refuse(&id, "unreadable");
            return Ok(());
        }
    };
    if opened.deleted {
        return Ok(());
    }
    if cipher::parse_time(&opened.revision).is_some_and(|date| date > run.now_ns() + AHEAD_NS) {
        run.report.refuse(&id, "future-date");
        return Ok(());
    }
    // The same ciphertext on two ciphers: a copy a server made
    if name_of(raw).is_some_and(|name| run.copies.contains(name)) {
        run.report.refuse(&id, "copy");
        return Ok(());
    }
    let content = &opened.content;
    let sealed = map::is_sealed_note(content);
    if content.kind == NOTE && !sealed && !map::is_codes_note(content) {
        run.report.left_alone += 1;
        return Ok(());
    }
    // A name, a user name or an address that is the password of another item: a password moved to where `list` shows it
    if !sealed && map::exposes(content, None, &run.secrets) {
        run.report.refuse(&id, "fields-swapped");
        return Ok(());
    }
    // The same as an item the vault has and the server did not know of: the two are linked, not doubled
    if !sealed {
        let unlinked: Vec<&Local> = run.locals.values().copied().filter(|local| !run.linked.contains(&key_of(local)) && !map::deleted(local) && connection.choice(&local.kind, &local.id) == "on").collect();
        for local in unlinked {
            let parts = [PartRef { kind: local.kind.clone(), id: local.id.clone(), hash: map::hash(local) }];
            if run.compose(None, &parts, None).is_ok_and(|mine| mine == *content) {
                let (raw, revision) = match run.rekey(session, raw, &opened)? {
                    Some(new) => {
                        let revision = json::text(&new, "revisionDate").unwrap_or(&opened.revision).to_string();
                        (new, revision)
                    }
                    None => (raw.clone(), opened.revision.clone()),
                };
                let link = Link { cipher: id.clone(), revision, raw, deleted: false, sealed: false, seal: 0, parts: parts.to_vec() };
                run.done(&id, Some(link), Vec::new(), Vec::new());
                return Ok(());
            }
        }
    }
    let (writes, parts, counter) = run.items_from(content, opened.uri_unproven, false);
    if writes.is_empty() {
        if sealed {
            run.report.sealed_elsewhere += 1;
        } else {
            run.report.left_alone += 1;
        }
        return Ok(());
    }
    // A sealed note names its item: one the vault has already is linked, and the vault's version stays the source
    let mut basis = Vec::new();
    let mut kept = Vec::new();
    let mut writes_kept = Vec::new();
    for (write, part) in writes.into_iter().zip(parts) {
        let here = run.locals.get(&(write.kind.clone(), write.id.clone())).copied();
        match here {
            Some(_) if run.linked.contains(&(write.kind.clone(), write.id.clone())) => {
                run.report.refuse(&id, "duplicate");
                return Ok(());
            }
            Some(_) => kept.push(part),
            None => {
                basis.push(Basis { kind: write.kind.clone(), id: write.id.clone(), hash: None });
                kept.push(part);
                writes_kept.push(write);
            }
        }
    }
    run.report.created_here += writes_kept.len().min(1);
    // Its own key, before it is linked
    let (raw, revision) = match if sealed { None } else { run.rekey(session, raw, &opened)? } {
        Some(new) => {
            let revision = json::text(&new, "revisionDate").unwrap_or(&opened.revision).to_string();
            (new, revision)
        }
        None => (raw.clone(), opened.revision.clone()),
    };
    let link = Link { cipher: id.clone(), revision, raw, deleted: false, sealed, seal: counter, parts: kept };
    run.done(&id, Some(link), writes_kept, basis);
    Ok(())
}

/// An authenticator account goes into the `totp` of its Login: the one Login whose password item has its issuer's name and
/// no account yet, and no `totp` of its own. Otherwise it is a Login of its own.
fn attach(session: &mut Session<'_>, run: &mut Run<'_>, local: &Local) -> Result<bool, Problem> {
    let Some(account) = map::account(local) else { return Ok(false) };
    let issuer = account.issuer.trim().to_lowercase();
    if issuer.is_empty() {
        return Ok(false);
    }
    let candidates: Vec<Link> = run
        .links
        .values()
        .filter(|link| !link.sealed && !link.deleted && link.parts.iter().all(|part| part.kind != KIND_ACCOUNT))
        .filter(|link| json::get(&link.raw, "login").is_some_and(|login| json::some(login, "totp").is_none()))
        .filter(|link| {
            link.parts.iter().any(|part| part.kind == KIND_PASSWORD && run.local(part).and_then(map::password).is_some_and(|password| password.title.trim().to_lowercase() == issuer))
        })
        .cloned()
        .collect();
    let [link] = candidates.as_slice() else { return Ok(false) };
    let Ok(opened) = cipher::open(&link.raw, &run.user) else { return Ok(false) };
    let mut parts = link.parts.clone();
    // A hash nobody has: the new part counts as changed, so the push sends it
    parts.push(PartRef { kind: local.kind.clone(), id: local.id.clone(), hash: String::new() });
    let with = Link { parts, ..link.clone() };
    run.push(session, &with, &opened)?;
    Ok(run.links.get(&link.cipher).is_some_and(|now| now.parts.iter().any(|part| part.kind == KIND_ACCOUNT && part.id == local.id)))
}
