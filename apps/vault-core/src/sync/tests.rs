//! Runs against a server of our own (`fake.rs`): round trips of every kind both ways, a second client's writes, conflicts,
//! deletions, and every attack of the spec's section 10.2 a client can refuse. A vault here is a map, applied the way the
//! service applies a run's groups.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::api::{Extra, Problem};
use super::cipher::{self, Content, LOGIN};
use super::crypto::{self, EncString, Kdf, KdfProblem, SymKey};
use super::fake::{Fake, API_ID, API_SECRET, BASE, EMAIL, PASSWORD, USER_ID};
use super::map::{self, Local};
use super::merge::{self, Access, ApiKey, Connection, Input, Link, Outcome};
use super::address::Server;
use crate::items::{Account, BackupCode, BackupService, OtpKind, Password, Secret};
use crate::protocol::Approval;
use crate::seal::Key;

/// A day after the clock of the fake server begins
pub(super) const NOW_MS: i64 = 1_791_000_000_000;

pub(super) fn server() -> Server {
    Server { address: BASE.into(), identity: format!("{BASE}/identity"), api: format!("{BASE}/api") }
}

pub(super) fn connect(fake: &mut Fake) -> Result<Connection, Problem> {
    let pending = merge::prepare(fake, &crypto::master_key, server(), EMAIL, PASSWORD.as_bytes(), None, None, None, "dev-1".into())?;
    merge::finish(fake, &pending, &Extra::default()).map(|(connection, _)| connection)
}

#[derive(Default)]
pub(super) struct Vault {
    pub(super) locals: BTreeMap<(String, String), Local>,
    pub(super) links: BTreeMap<String, Link>,
    pub(super) sealing: Option<(Key, String)>,
    /// Items the service could not read, and whether it could not read the links
    pub(super) unreadable: usize,
    pub(super) links_unreadable: bool,
}

impl Vault {
    pub(super) fn put(&mut self, kind: &str, id: &str, value: Value) {
        self.locals.insert((kind.into(), id.into()), Local { kind: kind.into(), id: id.into(), name: map::name_of(kind, &value), meta: Value::Null, approval: Approval::WhileOpen, value });
    }

    pub(super) fn value(&self, kind: &str, id: &str) -> Value {
        self.locals.get(&(kind.into(), id.into())).map(|local| local.value.clone()).unwrap_or(Value::Null)
    }

    pub(super) fn of_kind(&self, kind: &str) -> Vec<&Local> {
        self.locals.values().filter(|local| local.kind == kind).collect()
    }

    /// As the service applies a run: a group whose basis no longer holds is skipped whole, and so is every later one for
    /// that cipher
    pub(super) fn apply(&mut self, outcome: &mut Outcome) {
        let mut skipped = std::collections::HashSet::new();
        for group in outcome.groups.drain(..) {
            let holds = group.basis.iter().all(|basis| self.locals.get(&(basis.kind.clone(), basis.id.clone())).map(map::hash) == basis.hash);
            if skipped.contains(&group.cipher) || !holds {
                skipped.insert(group.cipher.clone());
                continue;
            }
            for write in group.writes {
                self.locals.insert((write.kind.clone(), write.id.clone()), Local { kind: write.kind, id: write.id, name: write.name, meta: write.meta, approval: write.approval, value: write.value });
            }
            match group.link {
                Some(link) => {
                    self.links.insert(group.cipher, link);
                }
                None => {
                    self.links.remove(&group.cipher);
                }
            }
        }
    }

    pub(super) fn sync(&mut self, fake: &mut Fake, connection: &Connection) -> Outcome {
        self.sync_with(fake, connection, false)
    }

    fn sync_with(&mut self, fake: &mut Fake, connection: &Connection, accept_gone: bool) -> Outcome {
        let sealing = self.sealing.as_ref().map(|(key, vault)| (key.copy(), vault.clone()));
        let input = Input {
            connection: connection.clone(),
            access: Some(Access { token: zeroize::Zeroizing::new(fake.access()), until_ms: i64::MAX }),
            links: self.links.values().cloned().collect(),
            locals: self.locals.values().cloned().collect(),
            sealing,
            accept: accept_gone,
            full: true,
            now_ms: NOW_MS,
            local_unreadable: self.unreadable,
            links_unreadable: self.links_unreadable,
            retry_files: false,
        };
        let mut outcome = merge::run(fake, &super::convert::Direct, input);
        self.apply(&mut outcome);
        outcome
    }
}

pub(super) fn password(title: &str, username: &str, secret: &str, address: &str) -> Value {
    serde_json::to_value(Password { title: title.into(), username: username.into(), password: Secret::new(secret), address: address.into(), ..Password::default() }).unwrap()
}

pub(super) fn totp_account(issuer: &str, label: &str) -> Value {
    serde_json::to_value(Account { kind: Some(OtpKind::Totp), issuer: issuer.into(), label: label.into(), secret: Secret::new("JBSWY3DPEHPK3PXP"), ..Account::default() }).unwrap()
}

pub(super) fn backup_codes(service: &str) -> Value {
    serde_json::to_value(BackupService { service_name: service.into(), codes: vec![BackupCode::new("1111-2222"), BackupCode::new("3333-4444")], ..BackupService::default() }).unwrap()
}

/// Nothing decrypted, and no key, ever reached the server in the clear
pub(super) fn assert_nothing_leaked(fake: &Fake, secrets: &[&str]) {
    let user_hex = crate::seal::hex(fake.user.bytes());
    for seen in &fake.seen {
        let everything = format!("{} {} {:?} {}", seen.method, seen.url, seen.headers, seen.body);
        assert!(!everything.contains(PASSWORD), "the master password went out: {}", seen.url);
        assert!(!everything.contains(&user_hex), "the user key went out");
        for secret in secrets {
            assert!(!everything.contains(secret), "{secret:?} went out in the clear: {} {}", seen.method, seen.url);
        }
        assert!(seen.url.starts_with(BASE), "a request left the connected server: {}", seen.url);
        assert!(seen.method != "DELETE" && !(seen.method == "POST" && seen.url.ends_with("/delete")), "a permanent delete");
        assert!(seen.body.starts_with("--") || !seen.body.contains("\"data\""), "data was sent");
    }
}

fn add_login_elsewhere(fake: &mut Fake, content: &Content) -> String {
    let body = cipher::create_body(content, USER_ID, &fake.user).unwrap();
    fake.store(body)
}

fn open_on_server(fake: &Fake, id: &str) -> Content {
    cipher::open(fake.find(id).unwrap(), &fake.user).unwrap().content
}

fn login(name: &str, username: &str, secret: &str) -> Content {
    Content { kind: LOGIN, name: name.into(), username: Some(username.into()), password: Some(secret.into()), ..Content::default() }
}

#[test]
fn connecting_by_password_and_by_api_key() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    assert_eq!(connection.user_id, USER_ID);
    assert_eq!(connection.key().unwrap().bytes()[..], fake.user.bytes()[..]);
    let token = fake.seen.iter().find(|seen| seen.url.ends_with("/connect/token")).unwrap();
    assert!(token.body.contains("client_id=cli") && token.body.contains("deviceName=Amethystora%20Vault") && token.body.contains("deviceType=8"));
    assert!(token.headers.iter().any(|(name, value)| name == "Bitwarden-Client-Version" && value == super::CLIENT_VERSION));
    // With an API key, no master password hash is made or sent: nothing to attack offline
    let mut fake = Fake::default();
    let hash = fake.hash.clone();
    let pending = merge::prepare(&mut fake, &crypto::master_key, server(), EMAIL, PASSWORD.as_bytes(), None, None, Some(ApiKey { id: API_ID.into(), secret: API_SECRET.into() }), "dev-1".into()).unwrap();
    assert!(pending.hash.is_none());
    merge::finish(&mut fake, &pending, &Extra::default()).unwrap();
    assert_nothing_leaked(&fake, &[&hash]);
    // A wrong password: the server's own words, as data
    let mut fake = Fake::default();
    let pending = merge::prepare(&mut fake, &crypto::master_key, server(), EMAIL, b"wrong", None, None, None, "dev-1".into()).unwrap();
    assert!(matches!(merge::finish(&mut fake, &pending, &Extra::default()), Err(Problem::ServerSaid(_))));
}

#[test]
fn a_second_factor_is_asked_and_answered() {
    let mut fake = Fake { two_factor: true, ..Fake::default() };
    let pending = merge::prepare(&mut fake, &crypto::master_key, server(), EMAIL, PASSWORD.as_bytes(), None, None, None, "dev-1".into()).unwrap();
    assert_eq!(merge::finish(&mut fake, &pending, &Extra::default()).err(), Some(Problem::TwoFactor(vec![0])));
    let extra = Extra { two_factor: Some((0, "123456", false)), ..Extra::default() };
    assert!(merge::finish(&mut fake, &pending, &extra).is_ok());
    let retry = fake.seen.iter().rev().find(|seen| seen.url.ends_with("/connect/token")).unwrap();
    assert!(retry.body.contains("twoFactorToken=123456") && retry.body.contains("twoFactorProvider=0") && retry.body.contains("twoFactorRemember=0"));
}

#[test]
fn every_kind_goes_and_comes_back() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    vault.put("password", "p1", password("Example", "person", "hunter2 ✓", "https://example.test/login"));
    vault.put("authenticator", "a1", totp_account("Elsewhere", "me"));
    vault.put("backup-codes", "c1", backup_codes("Example"));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none(), "{:?}", outcome.problem.map(|p| p.name()));
    assert_eq!(outcome.report.created_there, 3);
    assert_eq!(fake.ciphers.len(), 3);
    // On the server, as another client reads them
    let contents: Vec<Content> = fake.ciphers.iter().map(|raw| cipher::open(raw, &fake.user).unwrap().content).collect();
    assert!(contents.iter().any(|c| c.password.as_deref() == Some("hunter2 ✓") && c.uri.as_deref() == Some("https://example.test/login")));
    assert!(contents.iter().any(|c| c.totp.as_deref().is_some_and(|t| t.starts_with("otpauth://totp/Elsewhere:me?"))));
    assert!(contents.iter().any(|c| c.kind == 2 && c.notes.as_deref() == Some("[ ] 1111-2222\n[ ] 3333-4444")));
    for raw in &fake.ciphers {
        assert!(raw["key"].as_str().unwrap().starts_with("2."), "every item created has its own key");
    }
    assert_nothing_leaked(&fake, &["hunter2", "JBSWY3DPEHPK3PXP", "1111-2222", "example.test/login"]);
    // Nothing changed: nothing written either way
    let writes = fake.seen.len();
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.groups.is_empty() && outcome.problem.is_none());
    assert!(fake.seen[writes..].iter().all(|seen| seen.method == "GET"));

    // Another client adds a Login with a totp and edits the codes; this vault takes both
    let id = add_login_elsewhere(&mut fake, &Content { totp: Some("otpauth://totp/Other:you?secret=JBSWY3DPEHPK3PXP&issuer=Other".into()), ..login("Other", "you", "s3cret") });
    let codes_id = vault.links.values().find(|link| link.parts[0].kind == "backup-codes").unwrap().cipher.clone();
    let mut codes = open_on_server(&fake, &codes_id);
    codes.notes = Some("[x] 1111-2222\n[ ] 3333-4444\n[ ] 5555-6666".into());
    let raw = fake.find(&codes_id).unwrap().clone();
    let mut body = cipher::update_body(&raw, &open_on_server(&fake, &codes_id), &codes, USER_ID, &fake.user).unwrap();
    body["id"] = json!(codes_id);
    *fake.find_mut(&codes_id).unwrap() = body;
    fake.touch(&codes_id);
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none());
    let new_password = vault.of_kind("password").into_iter().find(|local| local.value["title"] == "Other").unwrap();
    assert_eq!(new_password.value["password"], "s3cret");
    let new_account = vault.of_kind("authenticator").into_iter().find(|local| local.value["issuer"] == "Other").unwrap();
    assert_eq!(new_account.value["secret"], "JBSWY3DPEHPK3PXP");
    assert_eq!(vault.links[&id].parts.len(), 2);
    let codes = vault.value("backup-codes", "c1");
    assert_eq!(codes["codes"].as_array().unwrap().len(), 3);
    assert_eq!(codes["codes"][0]["used"], true);
}

#[test]
fn an_update_keeps_what_this_client_does_not_read() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    vault.put("password", "p1", password("Example", "person", "old", "https://example.test"));
    vault.sync(&mut fake, &connection);
    let id = fake.ciphers[0]["id"].as_str().unwrap().to_string();
    // Another client adds a passkey, a second URI, a field and keys this client has never heard of
    let passkey = json!([{"credentialId": "2.AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=", "keyValue": "2.BBBB|CCCC|DDDD", "creationDate": "2026-01-01T00:00:00Z"}]);
    {
        let raw = fake.find_mut(&id).unwrap();
        raw["login"]["fido2Credentials"] = passkey.clone();
        raw["futureThing"] = json!({"x": [1, 2]});
        raw["login"]["futureLoginThing"] = json!(true);
        raw["favorite"] = json!(true);
        raw["folderId"] = json!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        raw["archivedDate"] = json!("2026-01-01T00:00:00Z");
    }
    fake.touch(&id);
    vault.sync(&mut fake, &connection);
    let key_before = fake.find(&id).unwrap()["key"].clone();
    // The person changes the password here
    let mut changed: Password = serde_json::from_value(vault.value("password", "p1")).unwrap();
    changed.set_password(Secret::new("new"), 5);
    vault.put("password", "p1", serde_json::to_value(changed).unwrap());
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.sent, 1);
    let put = fake.seen.iter().rev().find(|seen| seen.method == "PUT").unwrap();
    let body: Value = serde_json::from_str(&put.body).unwrap();
    assert_eq!(body["login"]["fido2Credentials"], passkey, "the passkey went back byte for byte");
    assert_eq!(body["futureThing"], json!({"x": [1, 2]}));
    assert_eq!(body["login"]["futureLoginThing"], json!(true));
    assert_eq!(body["favorite"], json!(true));
    assert_eq!(body["folderId"], json!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"));
    assert_eq!(body["archivedDate"], json!("2026-01-01T00:00:00Z"));
    assert_eq!(body["key"], key_before, "the item key is kept");
    assert!(body.get("data").is_none() && body.get("id").is_none() && body["login"].get("uri").is_none());
    assert_eq!(body["encryptedFor"], json!(USER_ID));
    let on_server = open_on_server(&fake, &id);
    assert_eq!(on_server.password.as_deref(), Some("new"));
    let history = fake.find(&id).unwrap()["passwordHistory"].as_array().unwrap().clone();
    assert_eq!(crypto::open_text(history[0]["password"].as_str().unwrap(), &cipher::key_of(fake.find(&id).unwrap(), &fake.user).unwrap()).unwrap().as_str(), "old");
}

#[test]
fn changed_on_both_sides_keeps_both() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    vault.put("password", "p1", password("Example", "person", "one", ""));
    vault.sync(&mut fake, &connection);
    let id = fake.ciphers[0]["id"].as_str().unwrap().to_string();
    // There
    let raw = fake.find(&id).unwrap().clone();
    let theirs = Content { password: Some("two".into()), ..open_on_server(&fake, &id) };
    let mut body = cipher::update_body(&raw, &open_on_server(&fake, &id), &theirs, USER_ID, &fake.user).unwrap();
    body["id"] = json!(id);
    *fake.find_mut(&id).unwrap() = body;
    fake.touch(&id);
    // And here
    vault.put("password", "p1", password("Example", "person", "three", ""));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none());
    assert_eq!(outcome.report.conflicts, 1);
    let passwords: Vec<String> = vault.of_kind("password").iter().map(|local| local.value["password"].as_str().unwrap().to_string()).collect();
    assert!(passwords.contains(&"two".to_string()) && passwords.contains(&"three".to_string()));
    let marked = vault.of_kind("password").into_iter().find(|local| local.value["password"] == "two").unwrap();
    assert_eq!(marked.value["syncConflict"]["from"], "server");
    // The server has both too
    let there: Vec<String> = fake.ciphers.iter().filter_map(|raw| cipher::open(raw, &fake.user).ok()?.content.password).collect();
    assert!(there.contains(&"two".to_string()) && there.contains(&"three".to_string()));
}

#[test]
fn a_stale_put_turns_into_keeping_both() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    vault.put("password", "p1", password("Example", "person", "one", ""));
    vault.sync(&mut fake, &connection);
    let id = fake.ciphers[0]["id"].as_str().unwrap().to_string();
    vault.put("password", "p1", password("Example", "person", "mine", ""));
    // Another client writes between this run's /sync and its PUT: the sync shows the old revision, the PUT finds a new one
    let old = fake.find(&id).unwrap().clone();
    let raw = old.clone();
    let theirs = Content { password: Some("theirs".into()), ..open_on_server(&fake, &id) };
    let mut body = cipher::update_body(&raw, &open_on_server(&fake, &id), &theirs, USER_ID, &fake.user).unwrap();
    body["id"] = json!(id);
    *fake.find_mut(&id).unwrap() = body;
    fake.touch(&id);
    let newer = fake.find(&id).unwrap().clone();
    let stale = old.clone();
    let idc = id.clone();
    fake.hooks.insert("sync", Box::new(move |sync: &mut Value| {
        for cipher in sync["ciphers"].as_array_mut().unwrap() {
            if cipher["id"] == json!(idc) {
                *cipher = stale.clone();
            }
        }
    }));
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.conflicts, 1, "{:?}", outcome.report.refused);
    assert_eq!(fake.find(&id).unwrap()["revisionDate"], newer["revisionDate"], "the other client's write stands");
    let passwords: Vec<String> = vault.of_kind("password").iter().map(|local| local.value["password"].as_str().unwrap().to_string()).collect();
    assert!(passwords.contains(&"theirs".to_string()) && passwords.contains(&"mine".to_string()));
}

#[test]
fn a_deletion_goes_to_each_sides_trash() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    vault.put("password", "p1", password("Here", "a", "1", ""));
    vault.put("password", "p2", password("There", "b", "2", ""));
    vault.sync(&mut fake, &connection);
    let id_of = |vault: &Vault, local: &str| vault.links.values().find(|link| link.parts[0].id == local).unwrap().cipher.clone();
    let (here, there) = (id_of(&vault, "p1"), id_of(&vault, "p2"));
    // Removed here: trashed there, never destroyed
    vault.locals.remove(&("password".into(), "p1".into()));
    // Trashed there
    let now = fake.now();
    fake.find_mut(&there).unwrap()["deletedDate"] = json!(now);
    fake.touch(&there);
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none());
    assert!(fake.find(&here).unwrap()["deletedDate"].is_string());
    assert!(vault.value("password", "p2")["deletedAt"].is_number(), "in this vault's trash, not gone");
    // Restored there: back here
    fake.find_mut(&there).unwrap()["deletedDate"] = Value::Null;
    fake.touch(&there);
    vault.sync(&mut fake, &connection);
    assert!(vault.value("password", "p2").get("deletedAt").is_none_or(Value::is_null));
    assert_nothing_leaked(&fake, &[]);
}

#[test]
fn a_counter_never_goes_back() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    let account = Account { kind: Some(OtpKind::Hotp), issuer: "Bank".into(), label: "me".into(), secret: Secret::new("JBSWY3DPEHPK3PXP"), counter: Some(10), ..Account::default() };
    vault.put("authenticator", "a1", serde_json::to_value(&account).unwrap());
    vault.sync(&mut fake, &connection);
    let id = fake.ciphers[0]["id"].as_str().unwrap().to_string();
    // A server with an older copy of the counter
    let raw = fake.find(&id).unwrap().clone();
    let old = open_on_server(&fake, &id);
    let behind = Content { totp: Some(old.totp.clone().unwrap().replace("counter=10", "counter=4")), name: "Bank renamed".into(), ..old.clone() };
    let mut body = cipher::update_body(&raw, &old, &behind, USER_ID, &fake.user).unwrap();
    body["id"] = json!(id);
    *fake.find_mut(&id).unwrap() = body;
    fake.touch(&id);
    vault.sync(&mut fake, &connection);
    assert_eq!(vault.value("authenticator", "a1")["counter"], 10);
}

#[test]
fn an_authenticator_joins_its_login() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    vault.put("password", "p1", password("Example", "person", "pw", ""));
    vault.sync(&mut fake, &connection);
    vault.put("authenticator", "a1", totp_account("example", "person"));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none());
    assert_eq!(fake.ciphers.len(), 1, "no Login of its own");
    let content = open_on_server(&fake, fake.ciphers[0]["id"].as_str().unwrap());
    assert_eq!(content.password.as_deref(), Some("pw"));
    assert!(content.totp.unwrap().starts_with("otpauth://totp/"));
}

#[test]
fn a_sealed_item_is_a_note_only_this_vault_opens() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    let mut vault = Vault { sealing: Some((Key::random(), "vault-1".into())), ..Vault::default() };
    vault.put("key", "k1", json!({"private": "PRIVATE-KEY-MATERIAL"}));
    vault.put("certificate", "c1", json!({"pem": "x".repeat(6000)}));
    vault.put("key", "k2", json!({"private": "never"}));
    // Keys and certificates are off until chosen; the large one cannot fit a note and goes as the note's file
    connection.choices.insert("key.k1".into(), "sealed".into());
    connection.choices.insert("certificate.c1".into(), "sealed".into());
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(fake.ciphers.len(), 2);
    assert_eq!(outcome.report.refused_total, 0, "{:?}", outcome.report.refused);
    let small = fake.ciphers.iter().find(|raw| raw["attachments"].is_null()).unwrap()["id"].as_str().unwrap().to_string();
    let content = open_on_server(&fake, &small);
    assert_eq!(content.name, map::SEALED_NAME);
    assert!(!content.notes.as_deref().unwrap().contains("PRIVATE"));
    assert_nothing_leaked(&fake, &["PRIVATE-KEY-MATERIAL", "never"]);
    // Another machine with this vault restored opens it; another vault does not
    let mut restored = Vault { sealing: vault.sealing.as_ref().map(|(k, v)| (k.copy(), v.clone())), ..Vault::default() };
    restored.sync(&mut fake, &connection);
    assert_eq!(restored.value("key", "k1"), json!({"private": "PRIVATE-KEY-MATERIAL"}));
    let mut other = Vault { sealing: Some((Key::random(), "vault-2".into())), ..Vault::default() };
    let outcome = other.sync(&mut fake, &connection);
    assert_eq!(outcome.report.sealed_elsewhere, 2);
    assert!(other.locals.is_empty());
}

#[test]
fn the_never_list_never_goes() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    let mut vault = Vault { sealing: Some((Key::random(), "vault-1".into())), ..Vault::default() };
    for kind in ["data-key", "account-ca", "sync-account", "sync-cipher", "recovery"] {
        vault.put(kind, "x1", json!("00112233"));
        connection.choices.insert(format!("{kind}.x1"), "sealed".into());
    }
    let mut locked = Local { kind: "key".into(), id: "k9".into(), name: "k".into(), meta: json!({"exportable": false}), approval: Approval::Ask, value: json!({"private": "NOT-EXPORTABLE"}) };
    connection.choices.insert("key.k9".into(), "sealed".into());
    vault.locals.insert(("key".into(), "k9".into()), locked.clone());
    vault.sync(&mut fake, &connection);
    assert!(fake.ciphers.is_empty());
    locked.meta = Value::Null;
    assert_nothing_leaked(&fake, &["NOT-EXPORTABLE", "00112233"]);
}

// --- The malicious server (spec 10.2) ------------------------------------------------------------------------------------

#[test]
fn a_lowered_kdf_is_refused_before_anything_is_computed_or_sent() {
    // Below the floor
    let mut fake = Fake::new(Kdf { kind: 0, iterations: 5000, memory: None, parallelism: None });
    assert_eq!(merge::prepare(&mut fake, &crypto::master_key, server(), EMAIL, PASSWORD.as_bytes(), None, None, None, "d".into()).err(), Some(Problem::Kdf(KdfProblem::TooWeak)));
    assert!(fake.seen.iter().all(|seen| !seen.url.ends_with("/connect/token")));
    // Below the pin of the last login, though within bounds
    let pinned = Kdf { kind: 0, iterations: 600_000, memory: None, parallelism: None };
    let mut fake = Fake::new(Kdf { kind: 0, iterations: 100_000, memory: None, parallelism: None });
    assert_eq!(merge::prepare(&mut fake, &crypto::master_key, server(), EMAIL, PASSWORD.as_bytes(), Some(100_000), Some(&pinned), None, "d".into()).err(), Some(Problem::Kdf(KdfProblem::BelowPin)));
    // Between the floor and 600,000: the person is asked first
    let mut fake = Fake::new(Kdf { kind: 0, iterations: 100_000, memory: None, parallelism: None });
    assert_eq!(merge::prepare(&mut fake, &crypto::master_key, server(), EMAIL, PASSWORD.as_bytes(), None, None, None, "d".into()).err(), Some(Problem::KdfConfirm(100_000)));
    assert!(merge::prepare(&mut fake, &crypto::master_key, server(), EMAIL, PASSWORD.as_bytes(), Some(100_000), None, None, "d".into()).is_ok());
    // Another KDF than the pin, Argon2 with too little memory, and two places that disagree
    let mut fake = Fake::new(Kdf { kind: 1, iterations: 3, memory: Some(64), parallelism: Some(4) });
    assert_eq!(merge::prepare(&mut fake, &crypto::master_key, server(), EMAIL, PASSWORD.as_bytes(), None, Some(&pinned), None, "d".into()).err(), Some(Problem::Kdf(KdfProblem::BelowPin)));
    fake.hooks.insert("prelogin", Box::new(|answer: &mut Value| answer["kdfSettings"]["memory"] = json!(15)));
    assert!(matches!(merge::prepare(&mut fake, &crypto::master_key, server(), EMAIL, PASSWORD.as_bytes(), None, None, None, "d".into()), Err(Problem::Kdf(_))));
    let mut fake = Fake::default();
    fake.hooks.insert("prelogin", Box::new(|answer: &mut Value| answer["kdfIterations"] = json!(700_000)));
    assert!(matches!(merge::prepare(&mut fake, &crypto::master_key, server(), EMAIL, PASSWORD.as_bytes(), None, None, None, "d".into()), Err(Problem::Kdf(_))));
    let mut fake = Fake::default();
    fake.hooks.insert("prelogin", Box::new(|answer: &mut Value| answer["salt"] = json!("attacker@evil.test")));
    assert_eq!(merge::prepare(&mut fake, &crypto::master_key, server(), EMAIL, PASSWORD.as_bytes(), None, None, None, "d".into()).err(), Some(Problem::SaltChanged));
    // At login, a response whose settings are not what was derived with
    let mut fake = Fake::default();
    fake.hooks.insert("token", Box::new(|answer: &mut Value| answer["KdfIterations"] = json!(1)));
    assert!(matches!(connect(&mut fake), Err(Problem::Kdf(_))));
}

fn connect_with(hook: impl FnMut(&mut Value) + Send + 'static) -> (Fake, Result<Connection, Problem>) {
    let mut fake = Fake::default();
    fake.hooks.insert("token", Box::new(hook));
    let result = connect(&mut fake);
    (fake, result)
}

#[test]
fn a_forged_user_key_is_never_taken() {
    // Null: never made here (BWb1)
    let (fake, result) = connect_with(|answer| {
        answer["Key"] = Value::Null;
        answer["UserDecryptionOptions"]["MasterPasswordUnlock"]["MasterKeyEncryptedUserKey"] = Value::Null;
    });
    assert_eq!(result.err(), Some(Problem::NoKey));
    assert!(fake.seen.iter().all(|seen| !seen.url.contains("/api/")), "nothing more after a missing key");
    // Type 0, no MAC (BW11)
    let (_, result) = connect_with(|answer| {
        let zero = "0.sLGys7S1tre4ubq7vL2+vw==|M3zqN2ch0xEveb6Pl2M8YA==";
        answer["Key"] = json!(zero);
        answer["UserDecryptionOptions"]["MasterPasswordUnlock"]["MasterKeyEncryptedUserKey"] = json!(zero);
    });
    assert_eq!(result.err(), Some(Problem::NoKey));
    // A 32-byte key under a MAC that verifies (BW12): the server cannot make one without the master key, so this one is
    // made with it, as the paper's gadget would have
    let master = crypto::master_key(PASSWORD.as_bytes(), EMAIL, &Kdf { kind: 0, iterations: 600_000, memory: None, parallelism: None }).unwrap();
    let short = EncString::seal(&crypto::stretch(&master), &[9u8; 32]).text();
    let (fake, result) = connect_with(move |answer| {
        answer["Key"] = json!(short);
        answer["UserDecryptionOptions"]["MasterPasswordUnlock"]["MasterKeyEncryptedUserKey"] = json!(short);
    });
    assert_eq!(result.err(), Some(Problem::CannotOpen));
    assert!(fake.seen.iter().all(|seen| !seen.url.contains("/api/")));
    // A key under some other key: cannot open, and nothing goes back about it
    let other = EncString::seal(&SymKey::random(), &[1u8; 64]).text();
    let (_, result) = connect_with(move |answer| {
        answer["Key"] = json!(other);
        answer["UserDecryptionOptions"]["MasterPasswordUnlock"]["MasterKeyEncryptedUserKey"] = json!(other);
    });
    assert_eq!(result.err(), Some(Problem::CannotOpen));
}

#[test]
fn a_newer_encryption_account_is_refused_and_nothing_written() {
    let (_, result) = connect_with(|answer| answer["AccountKeys"]["securityState"] = json!({"securityVersion": 2}));
    assert_eq!(result.err(), Some(Problem::NewerEncryption));
    // Signs that appear in /sync once connected
    for sign in 0..4 {
        let mut fake = Fake::default();
        let connection = connect(&mut fake).unwrap();
        let mut vault = Vault::default();
        vault.put("password", "p1", password("A", "b", "c", ""));
        fake.hooks.insert("sync", Box::new(move |sync: &mut Value| match sign {
            0 => sync["userDecryption"]["v2UpgradeToken"] = json!("x"),
            1 => sync["profile"]["accountKeys"] = json!({"signatureKeyPair": {"x": 1}}),
            2 => sync["profile"]["privateKey"] = json!("7.abc"),
            _ => sync["ciphers"] = json!([{"id": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee", "type": 1, "name": null, "data": "{\"format_version\":1}"}]),
        }));
        let outcome = vault.sync(&mut fake, &connection);
        assert_eq!(outcome.problem, Some(Problem::NewerEncryption), "sign {sign}");
        assert!(outcome.groups.is_empty());
        assert!(fake.ciphers.is_empty() && fake.seen.iter().all(|seen| seen.method == "GET" || seen.url.ends_with("/token") || seen.url.contains("prelogin")));
    }
}

/// A connected vault with one Login on the server, and a hook on /sync
fn hostile(hook: impl FnMut(&mut Value) + Send + 'static) -> (Fake, Connection, Vault, String, Outcome) {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    vault.put("password", "p1", password("Example", "person", "good", "https://example.test"));
    vault.put("password", "p2", password("Other", "x", "y", ""));
    vault.sync(&mut fake, &connection);
    let id = vault.links.values().find(|link| link.parts[0].id == "p1").unwrap().cipher.clone();
    fake.hooks.insert("sync", Box::new(hook));
    let writes = fake.seen.len();
    let outcome = vault.sync(&mut fake, &connection);
    assert!(fake.seen[writes..].iter().all(|seen| seen.method == "GET"), "nothing was written to the server");
    (fake, connection, vault, id, outcome)
}

fn cipher_of<'a>(sync: &'a mut Value, title_key: &SymKey) -> &'a mut Value {
    sync["ciphers"].as_array_mut().unwrap().iter_mut().find(|raw| cipher::open(raw, title_key).is_ok_and(|o| o.content.name == "Example")).unwrap()
}

#[test]
fn a_tampered_item_never_overwrites_the_good_one() {
    let good = password("Example", "person", "good", "https://example.test");
    let check = |vault: &Vault| assert_eq!(vault.value("password", "p1")["password"], good["password"]);
    // A field under another key: a bad MAC
    let user = std::sync::Arc::new(std::sync::Mutex::new(None::<SymKey>));
    let (_, _, vault, _, outcome) = hostile(|sync| {
        let raw = sync["ciphers"].as_array_mut().unwrap().iter_mut().find(|raw| raw["login"]["uris"].is_array()).unwrap();
        raw["login"]["password"] = json!(crypto::seal_text(&SymKey::random(), "evil"));
        raw["revisionDate"] = json!("2026-10-01T00:00:00Z");
    });
    check(&vault);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "unreadable"));
    drop(user);
    // A URI swapped, its checksum left (BW06)
    let mut fake = Fake::default();
    let key = fake.user.clone();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    vault.put("password", "p1", good.clone());
    vault.sync(&mut fake, &connection);
    let keyc = key.clone();
    fake.hooks.insert("sync", Box::new(move |sync: &mut Value| {
        let raw = cipher_of(sync, &keyc);
        let item = cipher::key_of(raw, &keyc).unwrap();
        raw["login"]["uris"][0]["uri"] = json!(crypto::seal_text(&item, "https://attacker.test"));
        raw["revisionDate"] = json!("2026-10-01T00:00:00Z");
    }));
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(vault.value("password", "p1")["address"], "https://example.test");
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "unreadable"));
    assert_nothing_leaked(&fake, &["attacker.test", "https://example.test", "good"]);
}

#[test]
fn a_swap_of_fields_inside_an_item_is_refused_and_the_vault_keeps_its_own() {
    // BW05: with an item key the server cannot make a value, but it can move the ciphertexts of one item around
    for (from, to) in [("password", "username"), ("username", "password")] {
        let (_, _, vault, _, outcome) = hostile(move |sync| {
            let raw = sync["ciphers"].as_array_mut().unwrap().iter_mut().find(|raw| raw["login"]["uris"].is_array()).unwrap();
            let (a, b) = (raw["login"][from].clone(), raw["login"][to].clone());
            raw["login"][from] = b;
            raw["login"][to] = a;
            raw["revisionDate"] = json!("2026-10-01T00:00:00Z");
        });
        assert!(outcome.report.refused.iter().any(|r| r["why"] == "fields-swapped"), "{from} {to}: {:?}", outcome.report.refused);
        assert_eq!(vault.value("password", "p1")["password"], "good");
        assert_eq!(vault.value("password", "p1")["username"], "person");
    }
    // The password's text moved into the title, which is shown in lists
    let (_, _, vault, _, outcome) = hostile(|sync| {
        let raw = sync["ciphers"].as_array_mut().unwrap().iter_mut().find(|raw| raw["login"]["uris"].is_array()).unwrap();
        raw["name"] = raw["login"]["password"].clone();
        raw["revisionDate"] = json!("2026-10-01T00:00:00Z");
    });
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "fields-swapped"));
    assert_eq!(vault.value("password", "p1")["title"], "Example");
}

#[test]
fn a_type_flip_a_rollback_and_a_vanished_key_stop_for_that_item() {
    for (attack, word) in [("type", "type-changed"), ("rollback", "rolled-back"), ("key", "key-removed"), ("org", "moved-to-organization")] {
        let (_, _, vault, _, outcome) = hostile(move |sync| {
            let raw = sync["ciphers"].as_array_mut().unwrap().iter_mut().find(|raw| raw["login"]["uris"].is_array()).unwrap();
            match attack {
                "type" => {
                    raw["type"] = json!(2);
                    raw["secureNote"] = json!({"type": 0});
                    raw["revisionDate"] = json!("2026-10-01T00:00:00Z");
                }
                "rollback" => raw["revisionDate"] = json!("2001-01-01T00:00:00Z"),
                "key" => {
                    raw["key"] = Value::Null;
                    raw["revisionDate"] = json!("2026-10-01T00:00:00Z");
                }
                _ => raw["organizationId"] = json!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
            }
        });
        assert!(outcome.report.refused.iter().any(|r| r["why"] == word), "{attack}: {:?}", outcome.report.refused);
        assert_eq!(vault.value("password", "p1")["password"], "good", "{attack}");
        assert!(vault.value("password", "p1").get("deletedAt").is_none_or(Value::is_null), "{attack}");
    }
}

#[test]
fn injected_organization_items_are_never_the_persons() {
    let (_, _, vault, _, outcome) = hostile(|sync| {
        let ciphers = sync["ciphers"].as_array_mut().unwrap();
        let mut planted = ciphers[0].clone();
        planted["id"] = json!("99999999-9999-4999-8999-999999999999");
        planted["organizationId"] = json!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        ciphers.push(planted);
        sync["profile"]["organizations"] = json!([{"id": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee", "key": "4.AAAA"}]);
    });
    assert_eq!(outcome.report.organization, 1);
    assert_eq!(vault.of_kind("password").len(), 2);
}

#[test]
fn one_vanished_item_goes_to_the_trash_and_many_stop_the_run() {
    let (_, _, vault, _, outcome) = hostile(|sync| {
        let ciphers = sync["ciphers"].as_array_mut().unwrap();
        ciphers.remove(0);
    });
    assert!(outcome.problem.is_none());
    assert!(vault.locals.values().any(map::deleted), "one gone is in the vault's trash");
    assert_eq!(vault.locals.len(), 2, "and still there");
    // Everything gone at once: nothing applied until a person says so
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    for n in 0..10 {
        vault.put("password", &format!("p{n}"), password(&format!("Site {n}"), "u", "p", ""));
    }
    vault.sync(&mut fake, &connection);
    fake.hooks.insert("sync", Box::new(|sync: &mut Value| sync["ciphers"] = json!([])));
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.problem, Some(Problem::ManyGone(10)));
    assert!(vault.locals.values().all(|local| !map::deleted(local)));
    let outcome = vault.sync_with(&mut fake, &connection, true);
    assert!(outcome.problem.is_none());
    assert!(vault.locals.values().all(map::deleted), "to the trash, never removed");
    assert_eq!(vault.locals.len(), 10);
}

#[test]
fn another_account_or_key_on_the_server_stops_the_run() {
    for attack in ["id", "email", "key", "kdf"] {
        let (_, _, _, _, outcome) = hostile(move |sync| match attack {
            "id" => sync["profile"]["id"] = json!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
            "email" => sync["profile"]["email"] = json!("attacker@evil.test"),
            "key" => sync["profile"]["key"] = json!("2.AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="),
            _ => sync["userDecryption"]["masterPasswordUnlock"]["kdf"]["iterations"] = json!(5000),
        });
        assert!(matches!(outcome.problem, Some(Problem::IdentityChanged | Problem::KeysChanged | Problem::Kdf(_))), "{attack}");
        assert!(outcome.groups.is_empty(), "{attack}");
    }
}

#[test]
fn a_redirect_a_huge_answer_and_a_server_that_is_down_change_nothing() {
    for (status, problem) in [(302u16, "not-allowed"), (503, "server-down"), (429, "rate-limited")] {
        let mut fake = Fake::default();
        let connection = connect(&mut fake).unwrap();
        let mut vault = Vault::default();
        vault.put("password", "p1", password("A", "b", "c", ""));
        vault.sync(&mut fake, &connection);
        fake.status.insert("sync", status);
        let outcome = vault.sync(&mut fake, &connection);
        assert_eq!(outcome.problem.as_ref().map(Problem::name), Some(problem));
        assert!(outcome.groups.is_empty());
        assert_eq!(vault.value("password", "p1")["password"], "c");
    }
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    fake.hooks.insert("sync", Box::new(|sync: &mut Value| sync["padding"] = json!("x".repeat(super::api::MOST_BODY + 1))));
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.problem.as_ref().map(Problem::name), Some("too-large"));
}

#[test]
fn the_server_never_says_where_to_go() {
    // /config names other hosts: nothing reaches them, since the client never asks /config what to use
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    assert_eq!(super::api::config(&mut fake, &server()), Some(("Fake".into(), "2026.6.0".into())));
    let mut vault = Vault::default();
    vault.put("password", "p1", password("A", "b", "c", ""));
    vault.sync(&mut fake, &connection);
    assert_nothing_leaked(&fake, &[]);
    // An id that is not a UUID never reaches a path
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    fake.hooks.insert("sync", Box::new(|sync: &mut Value| {
        sync["ciphers"] = json!([{"id": "../accounts/delete", "type": 1, "name": "2.AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=", "login": {}, "revisionDate": "2026-01-01T00:00:00Z"}]);
    }));
    vault.sync(&mut fake, &connection);
    assert!(fake.seen.iter().all(|seen| !seen.url.contains("..")));
}

#[test]
fn an_expired_token_is_renewed_once_and_a_dead_one_asks_the_person() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    vault.put("password", "p1", password("A", "b", "c", ""));
    // The token in memory is no longer good: refreshed once
    fake.status.insert("revision", 401);
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(fake.refresh_count, 1);
    assert_eq!(outcome.problem, Some(Problem::LoginExpired));
    fake.status.clear();
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none());
}

#[test]
fn identical_items_on_both_sides_are_linked_not_doubled() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    add_login_elsewhere(&mut fake, &login("Same", "u", "p"));
    let mut vault = Vault::default();
    vault.put("password", "p1", password("Same", "u", "p", ""));
    vault.sync(&mut fake, &connection);
    assert_eq!(fake.ciphers.len(), 1);
    assert_eq!(vault.of_kind("password").len(), 1);
}

// --- The review's findings (plans/vault/work/security-sync.md), each as the review showed it, and what must happen now ----------

pub(super) fn bump(fake: &mut Fake, id: &str, change: impl FnOnce(&mut Value)) {
    let now = fake.now();
    let raw = fake.find_mut(id).unwrap();
    change(raw);
    raw["revisionDate"] = json!(now);
}

fn ids(fake: &Fake) -> Vec<String> {
    fake.ciphers.iter().map(|raw| raw["id"].as_str().unwrap().to_string()).collect()
}

fn many(fake: &mut Fake, connection: &Connection, count: usize) -> Vault {
    let mut vault = Vault::default();
    for n in 0..count {
        vault.put("password", &format!("p{n}"), password(&format!("t{n}"), "u", &format!("secret{n}"), "https://a.test"));
    }
    let outcome = vault.sync(fake, connection);
    assert!(outcome.problem.is_none());
    vault
}

fn trashed(vault: &Vault) -> usize {
    vault.locals.values().filter(|local| map::deleted(local)).count()
}

/// An older item: no key of its own, every field under the user key, and no checksum on its address
fn keyless(fake: &mut Fake, name: &str, secret: &str, uri: &str, attached: bool) -> String {
    let user = fake.user.clone();
    let mut cipher = json!({"type": 1, "organizationId": null, "name": crypto::seal_text(&user, name), "notes": null,
        "login": {"username": crypto::seal_text(&user, "me"), "password": crypto::seal_text(&user, secret),
                  "uris": [{"uri": crypto::seal_text(&user, uri), "uriChecksum": null, "match": null}]}});
    if attached {
        cipher["attachments"] = json!([{"id": "a1", "fileName": crypto::seal_text(&user, "scan.pdf"), "size": "3"}]);
    }
    fake.store(cipher)
}

impl Vault {
    /// As the service does after a run: what it took off the server's side or put on it goes into the window
    fn sync_window(&mut self, fake: &mut Fake, connection: &mut Connection) -> Outcome {
        let outcome = self.sync_with(fake, connection, false);
        connection.record(outcome.churn, outcome.accepted, NOW_MS);
        outcome
    }
}

#[test]
fn h5_every_item_trashed_at_once_stops_the_run() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = many(&mut fake, &connection, 50);
    for id in ids(&fake) {
        bump(&mut fake, &id, |raw| raw["deletedDate"] = json!("2026-10-01T00:00:00.000Z"));
    }
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.problem, Some(Problem::ManyGone(50)));
    assert_eq!(trashed(&vault), 0, "nothing applied");
    // A person says so: to the vault's trash, never removed
    let outcome = vault.sync_with(&mut fake, &connection, true);
    assert!(outcome.problem.is_none());
    assert_eq!(trashed(&vault), 50);
    assert_eq!(vault.locals.len(), 50);
}

#[test]
fn m2_a_slow_emptying_is_stopped_by_the_sum_over_the_days() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    let mut vault = many(&mut fake, &connection, 100);
    let mut stopped = None;
    for run in 1..=40 {
        fake.ciphers.pop();
        fake.revision += 1;
        let outcome = vault.sync_window(&mut fake, &mut connection);
        if let Some(problem) = outcome.problem {
            stopped = Some((run, problem));
            break;
        }
    }
    let (run, problem) = stopped.expect("a server that empties the vault one item a run is stopped");
    assert!(matches!(problem, Problem::ManyGone(_)), "{problem:?}");
    assert!(run <= 25 && trashed(&vault) <= 25, "stopped at run {run} with {} in the trash", trashed(&vault));
    // And a person's yes starts the window again
    let outcome = vault.sync_with(&mut fake, &connection, true);
    connection.record(outcome.churn, outcome.accepted, NOW_MS);
    assert!(outcome.problem.is_none() && connection.recent.is_empty());
}

#[test]
fn m4_a_cipher_replayed_under_many_ids_fills_nothing() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let body = cipher::create_body(&login("x", "u", "p"), USER_ID, &fake.user).unwrap();
    let id = fake.store(body);
    let raw = fake.find(&id).unwrap().clone();
    for n in 0..6000 {
        let mut copy = raw.clone();
        copy["id"] = json!(format!("{:08x}-1111-4000-8000-{:012x}", n, n));
        fake.ciphers.push(copy);
    }
    let mut vault = Vault::default();
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none());
    assert!(vault.locals.is_empty(), "a copy is not an item: {}", vault.locals.len());
    assert_eq!(outcome.report.refused_total, 6001);
    assert!(outcome.report.refused.len() <= 20, "what is kept of the list is a few");
}

#[test]
fn m4_many_new_items_after_the_first_pull_ask_the_person() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    let mut vault = many(&mut fake, &connection, 3);
    for n in 0..8 {
        add_login_elsewhere(&mut fake, &login(&format!("New {n}"), "you", &format!("pw {n} x")));
    }
    let outcome = vault.sync_window(&mut fake, &mut connection);
    assert_eq!(outcome.problem, Some(Problem::ManyNew(8)));
    assert_eq!(vault.of_kind("password").len(), 3);
    let outcome = vault.sync_with(&mut fake, &connection, true);
    assert!(outcome.problem.is_none());
    assert_eq!(vault.of_kind("password").len(), 11);
}

#[test]
fn h6_an_item_that_did_not_read_is_not_an_item_removed() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = many(&mut fake, &connection, 1);
    // What the service reads when a file is damaged, or a folder came back from a backup: the item is not in `locals`
    vault.locals.clear();
    vault.unreadable = 1;
    let before = fake.seen.len();
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.problem, Some(Problem::LocalUnreadable(1)));
    assert_eq!(fake.seen.len(), before, "the server was asked nothing");
    assert!(fake.ciphers[0]["deletedDate"].is_null());
    // Links that did not read: not an account never seen
    vault.unreadable = 0;
    vault.links_unreadable = true;
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.problem, Some(Problem::LinksUnreadable));
    assert_eq!(fake.seen.len(), before);
    // An item truly removed is another thing: the server's copy goes to its trash
    vault.links_unreadable = false;
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none());
    assert!(fake.ciphers[0]["deletedDate"].is_string());
}

#[test]
fn h2_an_old_item_is_given_a_key_of_its_own_when_first_linked() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let bank = keyless(&mut fake, "Bank", "BANK-SECRET", "https://bank.test", false);
    let forum = keyless(&mut fake, "Forum", "forum-secret", "https://forum.attacker.test", false);
    let mut vault = Vault::default();
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.rekeyed, 2);
    for id in [&bank, &forum] {
        assert!(fake.find(id).unwrap()["key"].is_string(), "its own key now");
    }
    // Nothing was lost on the way, and the address kept its checksum
    let there = open_on_server(&fake, &forum);
    assert_eq!((there.password.as_deref(), there.uri.as_deref(), there.username.as_deref()), (Some("forum-secret"), Some("https://forum.attacker.test"), Some("me")));
    assert_eq!(vault.of_kind("password").len(), 2);
    // A password moved from one item to the other is one that does not open there
    let ciphertext = fake.find(&bank).unwrap()["login"]["password"].clone();
    bump(&mut fake, &forum, |raw| raw["login"]["password"] = ciphertext);
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "unreadable"), "{:?}", outcome.report.refused);
    let forum_local = vault.of_kind("password").into_iter().find(|local| local.value["title"] == "Forum").unwrap();
    assert_eq!(forum_local.value["password"], "forum-secret");
    assert_nothing_leaked(&fake, &["BANK-SECRET", "forum-secret"]);
}

#[test]
fn h2_an_old_item_that_cannot_be_rekeyed_conflicts_when_its_password_changes() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    // An attachment hangs off the key that opened it: such an item stays as it is
    let bank = keyless(&mut fake, "Bank", "BANK-SECRET", "https://bank.test", true);
    let forum = keyless(&mut fake, "Forum", "forum-secret", "https://forum.attacker.test", true);
    let mut vault = Vault::default();
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.rekeyed, 0);
    assert!(fake.find(&forum).unwrap()["key"].is_null());
    let ciphertext = fake.find(&bank).unwrap()["login"]["password"].clone();
    bump(&mut fake, &forum, |raw| raw["login"]["password"] = ciphertext);
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.conflicts, 1);
    // The vault's own Forum keeps its password and its address; what the server said is a second item, marked, whose address is
    // for showing only
    let forums: Vec<&Local> = vault.of_kind("password").into_iter().filter(|local| local.value["title"] == "Forum").collect();
    assert_eq!(forums.len(), 2);
    let mine = forums.iter().find(|local| local.value.get("syncConflict").is_none()).unwrap();
    assert_eq!((mine.value["password"].as_str(), mine.value["address"].as_str()), (Some("forum-secret"), Some("https://forum.attacker.test")));
    let theirs = forums.iter().find(|local| local.value.get("syncConflict").is_some()).unwrap();
    assert_eq!(theirs.value["addressUnproven"], json!(true));
}

#[test]
fn h3_a_cipher_moved_over_another_replaces_nothing() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    vault.put("password", "pa", password("Bank", "me", "BANK-SECRET", "https://bank.test"));
    let mut forum: Password = serde_json::from_value(password("Forum", "other", "forum-secret", "https://forum.test")).unwrap();
    forum.note = Some(Secret::new("recovery words only here"));
    vault.put("password", "pb", serde_json::to_value(&forum).unwrap());
    vault.sync(&mut fake, &connection);
    let by_name = |fake: &Fake, name: &str| fake.ciphers.iter().find(|raw| cipher::open(raw, &fake.user).unwrap().content.name == name).unwrap()["id"].as_str().unwrap().to_string();
    let (bank, forum_id) = (by_name(&fake, "Bank"), by_name(&fake, "Forum"));
    // The server serves Bank's cipher as Forum's, its name the ciphertext of Bank's password
    let mut copy = fake.find(&bank).unwrap().clone();
    copy["name"] = copy["login"]["password"].clone();
    bump(&mut fake, &forum_id, move |raw| {
        let keep = raw["id"].clone();
        *raw = copy;
        raw["id"] = keep;
    });
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "fields-swapped"), "{:?}", outcome.report.refused);
    let kept = vault.value("password", "pb");
    assert_eq!((kept["title"].as_str(), kept["address"].as_str(), kept["note"].as_str()), (Some("Forum"), Some("https://forum.test"), Some("recovery words only here")));
    assert!(vault.locals.values().all(|local| local.name != "BANK-SECRET"), "no name `list` shows is a password");
}

#[test]
fn h3_a_key_that_changes_is_a_conflict_that_keeps_both() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = many(&mut fake, &connection, 2);
    let (a, b) = (ids(&fake)[0].clone(), ids(&fake)[1].clone());
    // All of B, wrapped key and fields, under A's id
    let copy = fake.find(&b).unwrap().clone();
    bump(&mut fake, &a, move |raw| {
        let keep = raw["id"].clone();
        *raw = copy;
        raw["id"] = keep;
    });
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none());
    assert_eq!(outcome.report.conflicts, 1);
    // The vault's own item for A is as it was, and what the server said is another, marked
    let own: Vec<&Local> = vault.locals.values().filter(|local| local.value["password"] == "secret0").collect();
    assert_eq!(own.len(), 1);
    assert_eq!((own[0].value["title"].as_str(), own[0].value.get("syncConflict").is_none()), (Some("t0"), true));
    assert!(vault.locals.values().any(|local| local.value["password"] == "secret1" && local.value.get("syncConflict").is_some()));
}

#[test]
fn h4_an_older_sealed_note_is_never_taken() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    let mut vault = Vault { sealing: Some((Key::random(), "vault-1".into())), ..Vault::default() };
    vault.put("key", "k1", json!({"private": "OLD-KEY"}));
    connection.choices.insert("key.k1".into(), "sealed".into());
    vault.sync(&mut fake, &connection);
    let id = ids(&fake)[0].clone();
    let old = fake.find(&id).unwrap()["notes"].clone();
    vault.put("key", "k1", json!({"private": "NEW-KEY"}));
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.sent, 1);
    assert_eq!(vault.links[&id].seal, 2);
    // The server serves the note it kept, dated later
    bump(&mut fake, &id, |raw| raw["notes"] = old);
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "rolled-back"), "{:?}", outcome.report.refused);
    assert_eq!(vault.value("key", "k1"), json!({"private": "NEW-KEY"}));
}

#[test]
fn a_sealed_push_whose_answer_a_lock_dropped_is_the_same_item_next_time() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    let mut vault = Vault { sealing: Some((Key::random(), "vault-1".into())), ..Vault::default() };
    vault.put("key", "k1", json!({"private": "OLD-KEY"}));
    connection.choices.insert("key.k1".into(), "sealed".into());
    vault.sync(&mut fake, &connection);
    let links_before = vault.links.clone();
    vault.put("key", "k1", json!({"private": "NEW-KEY"}));
    vault.sync(&mut fake, &connection);
    // The vault closed before the run's result was written
    vault.links = links_before;
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.refused.is_empty() && outcome.report.conflicts == 0, "{:?}", outcome.report.refused);
    assert_eq!(vault.value("key", "k1"), json!({"private": "NEW-KEY"}));
    let id = ids(&fake)[0].clone();
    assert_eq!(vault.links[&id].seal, 2);
    // And two machines that both sent keep both: the other's copy is a second item
    let key = vault.sealing.as_ref().unwrap().0.copy();
    let mut changed = Local { kind: "key".into(), id: "k1".into(), name: "x".into(), meta: Value::Null, approval: Approval::WhileOpen, value: json!({"private": "THEIRS"}) };
    changed.name = "k1".into();
    let theirs = map::sealed_text(&changed, &key, "vault-1", 3).unwrap();
    let raw = fake.find(&id).unwrap().clone();
    let body = cipher::update_body(&raw, &open_on_server(&fake, &id), &Content { notes: Some(theirs.to_string()), ..open_on_server(&fake, &id) }, USER_ID, &fake.user).unwrap();
    let mut body = body;
    body["id"] = json!(id);
    *fake.find_mut(&id).unwrap() = body;
    fake.touch(&id);
    vault.put("key", "k1", json!({"private": "MINE"}));
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.conflicts, 1, "{:?}", outcome.report.refused);
    let values: Vec<Value> = vault.locals.values().filter(|local| local.kind == "key").map(|local| local.value.clone()).collect();
    assert!(values.contains(&json!({"private": "THEIRS"})) && values.contains(&json!({"private": "MINE"})), "{values:?}");
}

#[test]
fn m1_a_password_moved_in_two_runs_is_caught_by_its_history() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = many(&mut fake, &connection, 1);
    let id = ids(&fake)[0].clone();
    let ciphertext = fake.find(&id).unwrap()["login"]["password"].clone();
    // Run 1: the password and the user name blanked, which a person may do
    bump(&mut fake, &id, |raw| {
        raw["login"]["password"] = Value::Null;
        raw["login"]["username"] = Value::Null;
    });
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.is_empty());
    // Run 2: the old password's ciphertext as the user name and the title
    bump(&mut fake, &id, |raw| {
        raw["login"]["username"] = ciphertext.clone();
        raw["name"] = ciphertext;
    });
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "fields-swapped"), "{:?}", outcome.report.refused);
    let local = vault.locals.values().next().unwrap();
    assert_eq!(local.name, "t0");
    assert_eq!(local.meta["username"], "");
}

#[test]
fn a_password_moved_into_the_authenticator_field_is_refused() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = many(&mut fake, &connection, 1);
    let id = ids(&fake)[0].clone();
    let key = cipher::key_of(fake.find(&id).unwrap(), &fake.user).unwrap();
    // A password that reads as a base32 secret
    let mut item: Password = serde_json::from_value(vault.value("password", "p0")).unwrap();
    item.set_password(Secret::new("JBSWY3DPEHPK3PXP"), 7);
    vault.put("password", "p0", serde_json::to_value(item).unwrap());
    vault.sync(&mut fake, &connection);
    let ciphertext = fake.find(&id).unwrap()["login"]["password"].clone();
    assert_eq!(crypto::open_text(ciphertext.as_str().unwrap(), &key).unwrap().as_str(), "JBSWY3DPEHPK3PXP");
    bump(&mut fake, &id, |raw| {
        raw["login"]["password"] = Value::Null;
        raw["login"]["totp"] = ciphertext;
    });
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "fields-swapped"), "{:?}", outcome.report.refused);
    assert!(vault.of_kind("authenticator").is_empty());
    assert_eq!(vault.value("password", "p0")["password"], "JBSWY3DPEHPK3PXP");
}

#[test]
fn m3_a_cipher_too_big_or_too_deep_is_left_alone_and_the_links_stay_readable() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = many(&mut fake, &connection, 2);
    let nest = |depth: usize| serde_json::from_str::<Value>(&format!("{}{}", "[".repeat(depth), "]".repeat(depth))).unwrap();
    let user = fake.user.clone();
    let deep = fake.store(cipher::create_body(&login("Deep", "u", "deep pw"), USER_ID, &user).unwrap());
    let wide = fake.store(cipher::create_body(&login("Wide", "u", "wide pw"), USER_ID, &user).unwrap());
    bump(&mut fake, &deep, |raw| raw["x"] = nest(20));
    bump(&mut fake, &wide, |raw| raw["x"] = json!("y".repeat(40_000)));
    let outcome = vault.sync_with(&mut fake, &connection, true);
    assert!(outcome.problem.is_none());
    assert_eq!(outcome.report.refused_total, 2, "{:?}", outcome.report.refused);
    assert_eq!(vault.of_kind("password").len(), 2, "neither became an item");
    // A linked cipher that grows past the bound stops for that item alone
    let known = ids(&fake)[0].clone();
    bump(&mut fake, &known, |raw| raw["x"] = json!("y".repeat(40_000)));
    let outcome = vault.sync_with(&mut fake, &connection, true);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "too-big"), "{:?}", outcome.report.refused);
    // What the links hold is read back whole
    let value = serde_json::to_value(vault.links.values().cloned().collect::<Vec<Link>>()).unwrap();
    assert!(serde_json::from_value::<Vec<Link>>(value).is_ok());
}

#[test]
fn m8_a_long_refusal_list_is_a_count_and_a_few() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    for n in 0..3000 {
        fake.ciphers.push(json!({"id": format!("{:08x}-2222-4000-8000-{:012x}", n, n), "type": 1}));
    }
    fake.revision += 1;
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.refused_total, 3000);
    assert_eq!(outcome.report.refused.len(), 20);
    assert!(serde_json::to_vec(&outcome.report).unwrap().len() < 4096, "what goes into the connection's item is small");
}

#[test]
fn m9_the_next_put_starts_from_what_the_server_said_not_from_the_answer_to_the_last() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut body = cipher::create_body(&login("x", "u", "p"), USER_ID, &fake.user).unwrap();
    body["folderId"] = json!("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee");
    body["favorite"] = json!(true);
    fake.store(body);
    let mut vault = Vault::default();
    vault.sync(&mut fake, &connection);
    let pid = vault.of_kind("password")[0].id.clone();
    // The answer to a write is a form without them
    fake.hooks.insert("write", Box::new(|answer| {
        if let Some(object) = answer.as_object_mut() {
            object.remove("folderId");
            object.remove("favorite");
        }
    }));
    vault.put("password", &pid, password("x", "u", "second", ""));
    vault.sync(&mut fake, &connection);
    vault.put("password", &pid, password("x", "u", "third", ""));
    vault.sync(&mut fake, &connection);
    let last = fake.seen.iter().rev().find(|seen| seen.method == "PUT").unwrap();
    let sent: Value = serde_json::from_str(&last.body).unwrap();
    assert_eq!((sent["folderId"].as_str(), sent["favorite"].as_bool()), (Some("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"), Some(true)));
}

#[test]
fn l3_a_date_far_ahead_of_the_clock_is_refused_and_does_not_freeze_the_item() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = many(&mut fake, &connection, 1);
    let id = ids(&fake)[0].clone();
    fake.find_mut(&id).unwrap()["revisionDate"] = json!("9999-01-01T00:00:00Z");
    fake.revision += 1;
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "future-date"), "{:?}", outcome.report.refused);
    // The server is honest again: the item is taken again, and the person's edit goes
    bump(&mut fake, &id, |_| {});
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.is_empty(), "{:?}", outcome.report.refused);
    vault.put("password", "p0", password("t0", "u", "edited here", "https://a.test"));
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.sent, 1, "{:?}", outcome.report.refused);
}

#[test]
fn an_item_set_to_ask_every_time_stays_off_the_server_unless_chosen() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    vault.put("password", "p1", password("Ask me", "u", "asked-item-secret", ""));
    vault.locals.get_mut(&("password".to_string(), "p1".to_string())).unwrap().approval = Approval::Ask;
    vault.put("password", "p2", password("Plain", "u", "plain-item-secret", ""));
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.created_there, 1);
    assert_nothing_leaked(&fake, &["asked-item-secret"]);
    // Chosen, it goes
    connection.choices.insert("password.p1".into(), "on".into());
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.created_there, 1);
    assert_eq!(fake.ciphers.len(), 2);
}

#[test]
fn tombstones_go_after_thirty_days_and_say_so_in_the_list() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    let mut old = password("Old", "u", "old pw x", "");
    old["deletedAt"] = json!(NOW_MS - 31 * 24 * 3600 * 1000);
    let mut recent = password("Recent", "u", "recent pw x", "");
    recent["deletedAt"] = json!(NOW_MS - 24 * 3600 * 1000);
    vault.put("password", "o1", old);
    vault.put("password", "r1", recent);
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.purge.iter().map(|basis| basis.id.as_str()).collect::<Vec<_>>(), ["o1"]);
    assert_eq!(map::meta_of("password", &vault.value("password", "r1"))["deletedAt"], json!(NOW_MS - 24 * 3600 * 1000));
    assert!(map::meta_of("password", &password("Live", "u", "p", "")).get("deletedAt").is_none());
}

#[test]
fn m6_a_sync_is_read_within_bounds() {
    let mut fake = Fake::default();
    let connection = connect(&mut fake).unwrap();
    for (name, hook) in [
        ("numbers where ciphers are", Box::new(|sync: &mut Value| sync["ciphers"] = json!(vec![0; 400_000])) as Box<dyn FnMut(&mut Value) + Send>),
        ("more ciphers than any account has", Box::new(|sync: &mut Value| sync["ciphers"] = json!(vec![json!({}); 20_001]))),
        ("one cipher of many small values", Box::new(|sync: &mut Value| sync["ciphers"] = json!([{"x": vec![0; 5_000]}]))),
        ("one that nests too deep", Box::new(|sync: &mut Value| sync["ciphers"] = json!([{"x": serde_json::from_str::<Value>(&format!("{}{}", "[".repeat(40), "]".repeat(40))).unwrap()}]))),
    ] {
        fake.hooks.insert("sync", hook);
        let mut vault = Vault::default();
        let outcome = vault.sync(&mut fake, &connection);
        assert_eq!(outcome.problem, Some(Problem::BadResponse), "{name}");
    }
    // And an honest answer is read whole
    fake.hooks.clear();
    let mut vault = many(&mut fake, &connection, 5);
    assert_eq!(vault.sync(&mut fake, &connection).problem, None);
}

#[test]
fn m6_a_large_argon2_is_confirmed_before_it_is_spent() {
    let mut fake = Fake { kdf: Kdf { kind: 1, iterations: 3, memory: Some(512), parallelism: Some(4) }, ..Fake::default() };
    // Nothing is made until the person has seen the memory: the stand-in is never called
    let never = |_: &[u8], _: &str, _: &Kdf| -> Result<zeroize::Zeroizing<[u8; 32]>, KdfProblem> { panic!("memory was spent before the person confirmed") };
    assert_eq!(merge::prepare(&mut fake, &never, server(), EMAIL, PASSWORD.as_bytes(), None, None, None, "d".into()).err(), Some(Problem::KdfMemoryConfirm(512)));
    assert_eq!(merge::prepare(&mut fake, &never, server(), EMAIL, PASSWORD.as_bytes(), Some(500), None, None, "d".into()).err(), Some(Problem::KdfMemoryConfirm(512)));
    let stand_in = |_: &[u8], _: &str, _: &Kdf| -> Result<zeroize::Zeroizing<[u8; 32]>, KdfProblem> { Ok(zeroize::Zeroizing::new([7u8; 32])) };
    assert!(merge::prepare(&mut fake, &stand_in, server(), EMAIL, PASSWORD.as_bytes(), Some(512), None, None, "d".into()).is_ok());
    // At the limit it is not asked
    fake.kdf = Kdf { kind: 1, iterations: 3, memory: Some(256), parallelism: Some(4) };
    assert!(merge::prepare(&mut fake, &stand_in, server(), EMAIL, PASSWORD.as_bytes(), None, None, None, "d".into()).is_ok());
}
