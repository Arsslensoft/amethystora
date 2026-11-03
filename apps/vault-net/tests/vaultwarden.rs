//! Against Vaultwarden in a container (plans/vault-sync/IMPLEMENTERS.md 3 and 4): the client of `apps/vault-core/src/sync`,
//! through this crate's own HTTP code, to a real server. Run by hand, with the container sharing this one's network so
//! that it answers on a loopback address, which is all the shipped rules allow for plain http:
//!
//! ```text
//! docker run -d --name vw-test --memory=512m --tmpfs /data -e I_REALLY_WANT_VOLATILE_STORAGE=true -e DOMAIN=http://127.0.0.1:8080 \
//!   -e ROCKET_PORT=8080 -e SIGNUPS_ALLOWED=true -e SIGNUPS_VERIFY=false -e DISABLE_ICON_DOWNLOAD=true -e LOGIN_RATELIMIT_SECONDS=1 \
//!   -e LOGIN_RATELIMIT_MAX_BURST=1000 vaultwarden/server@sha256:4fe17f415f90087377d37c755e0ea3cf189dcc6794e49cdd2a9adbb4937d9e1f
//! cargo test -p amethystora-vault-net --test vaultwarden -- --ignored --test-threads=1   (in a container with --network container:vw-test)
//! ```
//!
//! Nothing here ever contacts a host that is not the one container.

use std::collections::BTreeMap;
use std::time::Instant;

use serde_json::{json, Value};
use zeroize::Zeroizing;

use amethystora_vault_core::items::{Account, BackupCode, BackupService, OtpKind, Password, Secret};
use amethystora_vault_core::protocol::Approval;
use amethystora_vault_core::seal::Key;
use amethystora_vault_core::sync::address::Server;
use amethystora_vault_core::sync::api::{self, Credentials, Extra, Http, NetFailure, Problem, Request, Response};
use amethystora_vault_core::sync::cipher::{self, Content, LOGIN};
use amethystora_vault_core::sync::crypto::{self, EncString, Kdf, SymKey};
use amethystora_vault_core::sync::map::{self, Local};
use amethystora_vault_core::sync::merge::{self, Access, ApiKey, Connection, Input, Link, Outcome};
use amethystora_vault_core::sync::register;
use amethystora_vault_net::http::Session;

const ADDRESS: &str = "http://127.0.0.1:8080";
const PASSWORD: &str = "a master password for the container";

/// The crate's own HTTP, as the client's `Http`
struct Net(Session);

impl Net {
    fn new() -> Net {
        Net(Session::new(vec!["http://127.0.0.1:8080".into()], None, None).expect("a session"))
    }
}

/// Every request that left, so that a test can say there were no more than it asked for
static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// And where each went
static URLS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

impl Http for Net {
    fn call(&mut self, request: Request) -> Result<Response, NetFailure> {
        CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        URLS.lock().unwrap().push(request.url.clone());
        let headers: Vec<(String, String)> = request.headers.iter().map(|(name, value)| (name.to_string(), value.clone())).collect();
        self.0.request(request.method, &request.url, &headers, &request.body).map(|(status, body)| Response { status, body }).map_err(|failure| NetFailure::from_name(failure.name()))
    }
}

fn server() -> Server {
    Server::from_address(ADDRESS).unwrap()
}

fn kdf() -> Kdf {
    Kdf { kind: 0, iterations: 600_000, memory: None, parallelism: None }
}

fn unique_email(name: &str) -> String {
    format!("{name}-{}@example.test", amethystora_vault_core::seal::random_hex(4))
}

fn account(net: &mut Net, name: &str) -> String {
    let email = unique_email(name);
    register::register(net, &server().identity, &email, PASSWORD, &kdf()).expect("registered");
    email
}

fn login(net: &mut Net, email: &str) -> (Connection, Access) {
    let pending = merge::prepare(net, &crypto::master_key, server(), email, PASSWORD.as_bytes(), None, None, None, merge::new_device_id()).expect("prelogin");
    merge::finish(net, &pending, &Extra::default()).expect("login")
}

/// A raw call as a second client would make it, with or without the version header
fn raw(net: &mut Net, method: &'static str, path: &str, access: &str, body: Option<&Value>, version: bool) -> (u16, Value) {
    let mut headers = vec![("Accept", "application/json".to_string()), ("Content-Type", "application/json".to_string()), ("Device-Type", "8".to_string()), ("Authorization", format!("Bearer {access}"))];
    if version {
        headers.push(("Bitwarden-Client-Version", amethystora_vault_core::sync::CLIENT_VERSION.to_string()));
    }
    let body = Zeroizing::new(body.map(|body| serde_json::to_vec(body).unwrap()).unwrap_or_default());
    let response = net.call(Request { method, url: format!("{}{path}", server().api), headers, body }).expect("an answer");
    (response.status, serde_json::from_slice(&response.body).unwrap_or(Value::Null))
}

/// What the service keeps, as a map, and applies a run the way the service does
#[derive(Default)]
struct Vault {
    locals: BTreeMap<(String, String), Local>,
    links: BTreeMap<String, Link>,
    /// The vault's own key for sealed notes, kept between runs: a new one each run would seal what no later run could open
    sealing: Option<(Key, String)>,
}

impl Vault {
    fn put(&mut self, kind: &str, id: &str, value: Value) {
        self.locals.insert((kind.into(), id.into()), Local { kind: kind.into(), id: id.into(), name: map::name_of(kind, &value), meta: Value::Null, approval: Approval::WhileOpen, value });
    }

    fn value(&self, kind: &str, id: &str) -> Value {
        self.locals.get(&(kind.into(), id.into())).map(|local| local.value.clone()).unwrap_or(Value::Null)
    }

    fn of_kind(&self, kind: &str) -> Vec<&Local> {
        self.locals.values().filter(|local| local.kind == kind).collect()
    }

    fn sync(&mut self, net: &mut Net, connection: &Connection, access: &Access) -> Outcome {
        let input = Input {
            connection: connection.clone(),
            access: Some(Access { token: access.token.clone(), until_ms: access.until_ms }),
            links: self.links.values().cloned().collect(),
            locals: self.locals.values().cloned().collect(),
            sealing: Some(match &self.sealing {
                Some((key, vault)) => (key.copy(), vault.clone()),
                None => {
                    let made = (Key::random(), "vault".to_string());
                    self.sealing = Some((made.0.copy(), made.1.clone()));
                    made
                }
            }),
            accept: false,
            local_unreadable: 0,
            links_unreadable: false,
            full: true,
            now_ms: merge::now_ms(),
            retry_files: false,
        };
        let mut outcome = merge::run(net, &amethystora_vault_core::sync::convert::Direct, input);
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
        outcome
    }
}

fn password_value(title: &str, username: &str, secret: &str, address: &str) -> Value {
    serde_json::to_value(Password { title: title.into(), username: username.into(), password: Secret::new(secret), address: address.into(), ..Password::default() }).unwrap()
}

fn all_ciphers(net: &mut Net, access: &str) -> Vec<Value> {
    let (status, sync) = raw(net, "GET", "/sync?excludeDomains=true", access, None, true);
    assert_eq!(status, 200);
    sync["ciphers"].as_array().cloned().unwrap_or_default()
}

fn cipher_by_id<'a>(ciphers: &'a [Value], id: &str) -> &'a Value {
    ciphers.iter().find(|raw| raw["id"] == json!(id)).expect("it is on the server")
}

// --- The facts the spec's appendix says to pin ---------------------------------------------------------------------------------

#[test]
#[ignore = "needs the Vaultwarden container"]
fn the_appendix_of_the_spec_holds_on_this_server() {
    let mut net = Net::new();
    let email = account(&mut net, "appendix");
    let (connection, access) = login(&mut net, &email);
    let user = connection.key().unwrap();
    let token = access.token.to_string();
    let id = connection.user_id.clone();
    let login_content = |name: &str, password: &str| Content { kind: LOGIN, name: name.into(), username: Some("person".into()), password: Some(password.into()), uri: Some("https://example.test/login".into()), ..Content::default() };

    // 1. A create without `encryptedFor` fails; with another account's id it is a 422
    let mut body = cipher::create_body(&login_content("One", "pw1"), &id, &user).unwrap();
    body.as_object_mut().unwrap().remove("encryptedFor");
    let (status, _) = raw(&mut net, "POST", "/ciphers", &token, Some(&body), true);
    assert_ne!(status, 200, "Vaultwarden took a body without encryptedFor");
    body["encryptedFor"] = json!("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee");
    assert_eq!(raw(&mut net, "POST", "/ciphers", &token, Some(&body), true).0, 422);
    body["encryptedFor"] = json!(id);
    let (status, created) = raw(&mut net, "POST", "/ciphers", &token, Some(&body), true);
    assert_eq!(status, 200, "{created}");
    let cipher_id = created["id"].as_str().unwrap().to_string();

    // 2. A body with `data` is ignored: the structured fields stand
    let mut with_data = cipher::create_body(&login_content("Two", "pw2"), &id, &user).unwrap();
    with_data["data"] = json!("{\"name\":\"2.stale|stale|stale\",\"username\":null}");
    let (status, two) = raw(&mut net, "POST", "/ciphers", &token, Some(&with_data), true);
    assert_eq!(status, 200, "{two}");
    assert_eq!(two["name"], with_data["name"]);
    assert!(two.get("data").is_none_or(Value::is_null), "Vaultwarden does not send data");

    // 3. A stale `lastKnownRevisionDate` is a 400, the returned one a 200
    let first = cipher::open(&created, &user).unwrap();
    let changed = Content { password: Some("pw1 changed".into()), ..first.content.clone() };
    let mut stale = cipher::update_body(&created, &first.content, &changed, &id, &user).unwrap();
    stale["lastKnownRevisionDate"] = json!("2001-01-01T00:00:00Z");
    assert_eq!(raw(&mut net, "PUT", &format!("/ciphers/{cipher_id}"), &token, Some(&stale), true).0, 400);
    let fresh = cipher::update_body(&created, &first.content, &changed, &id, &user).unwrap();
    let (status, updated) = raw(&mut net, "PUT", &format!("/ciphers/{cipher_id}"), &token, Some(&fresh), true);
    assert_eq!(status, 200, "{updated}");
    assert_eq!(cipher::open(&updated, &user).unwrap().content.password.as_deref(), Some("pw1 changed"));
    assert_eq!(updated["key"], created["key"], "the item key is kept");

    // 4. PUT /delete leaves the item with a deletedDate, PUT /restore clears it
    assert_eq!(raw(&mut net, "PUT", &format!("/ciphers/{cipher_id}/delete"), &token, None, true).0, 200);
    let in_trash = all_ciphers(&mut net, &token);
    assert!(cipher_by_id(&in_trash, &cipher_id)["deletedDate"].is_string());
    let (status, restored) = raw(&mut net, "PUT", &format!("/ciphers/{cipher_id}/restore"), &token, None, true);
    assert_eq!(status, 200);
    assert!(restored["deletedDate"].is_null());

    // 5. A passkey and every field this client does not read survive a PUT that changes the password, byte for byte
    let item_key = cipher::key_of(&restored, &user).unwrap();
    let passkey = json!([{
        "credentialId": crypto::seal_text(&item_key, "credential-id"), "keyType": crypto::seal_text(&item_key, "public-key"), "keyAlgorithm": crypto::seal_text(&item_key, "ECDSA"),
        "keyCurve": crypto::seal_text(&item_key, "P-256"), "keyValue": crypto::seal_text(&item_key, "key-material"), "rpId": crypto::seal_text(&item_key, "example.test"),
        "counter": crypto::seal_text(&item_key, "0"), "discoverable": crypto::seal_text(&item_key, "true"), "creationDate": "2026-01-01T00:00:00Z"
    }]);
    let current = cipher::open(&restored, &user).unwrap();
    let mut with_passkey = cipher::update_body(&restored, &current.content, &current.content, &id, &user).unwrap();
    with_passkey["login"]["fido2Credentials"] = passkey.clone();
    with_passkey["fields"] = json!([{"type": 0, "name": crypto::seal_text(&item_key, "note to self"), "value": crypto::seal_text(&item_key, "keep me"), "linkedId": null}]);
    with_passkey["favorite"] = json!(true);
    let (status, with) = raw(&mut net, "PUT", &format!("/ciphers/{cipher_id}"), &token, Some(&with_passkey), true);
    assert_eq!(status, 200, "{with}");
    let before = cipher::open(&with, &user).unwrap();
    let new = Content { password: Some("pw1 third".into()), ..before.content.clone() };
    let body = cipher::update_body(&with, &before.content, &new, &id, &user).unwrap();
    let (status, third) = raw(&mut net, "PUT", &format!("/ciphers/{cipher_id}"), &token, Some(&body), true);
    assert_eq!(status, 200, "{third}");
    let on_server = all_ciphers(&mut net, &token);
    let stored = cipher_by_id(&on_server, &cipher_id);
    assert_eq!(stored["login"]["fido2Credentials"], with["login"]["fido2Credentials"], "the passkey came back as it went");
    assert_eq!(stored["fields"], with["fields"]);
    assert_eq!(stored["favorite"], json!(true));
    assert_eq!(cipher::open(stored, &user).unwrap().content.password.as_deref(), Some("pw1 third"));
    assert_eq!(stored["passwordHistory"].as_array().map(Vec::len), Some(2), "each old password is kept in the history, newest first");

    // 6. An SSH key item is missing from /sync without the version header, and there with it
    let item = SymKey::random();
    let ssh = json!({
        "encryptedFor": id, "type": 5, "organizationId": null, "folderId": null, "favorite": false, "reprompt": 0, "fields": null, "notes": null, "passwordHistory": null,
        "key": EncString::seal(&user, item.bytes()).text(), "name": crypto::seal_text(&item, "A key"),
        "sshKey": {"privateKey": crypto::seal_text(&item, "-----BEGIN OPENSSH PRIVATE KEY-----\nAAAA\n-----END OPENSSH PRIVATE KEY-----\n"), "publicKey": crypto::seal_text(&item, "ssh-ed25519 AAAA"), "keyFingerprint": crypto::seal_text(&item, "SHA256:abc")},
    });
    let (status, made) = raw(&mut net, "POST", "/ciphers", &token, Some(&ssh), true);
    assert_eq!(status, 200, "{made}");
    let ssh_id = made["id"].as_str().unwrap().to_string();
    let (_, without) = raw(&mut net, "GET", "/sync?excludeDomains=true", &token, None, false);
    assert!(!without["ciphers"].as_array().unwrap().iter().any(|raw| raw["id"] == json!(ssh_id)), "without the header the SSH item is left out");
    let with_header = all_ciphers(&mut net, &token);
    assert!(with_header.iter().any(|raw| raw["id"] == json!(ssh_id)));
    println!("appendix: all six hold on Vaultwarden 1.37.5");
}

// --- Login ---------------------------------------------------------------------------------------------------------------------

#[test]
#[ignore = "needs the Vaultwarden container"]
fn login_by_password_by_api_key_and_by_refresh() {
    let mut net = Net::new();
    let email = account(&mut net, "login");
    let (connection, access) = login(&mut net, &email);
    assert!(api::is_uuid(&connection.user_id));
    assert!(connection.refresh.is_some(), "a password login has a refresh token");
    let pinned = connection.kdf;
    // A refresh
    let refreshed = api::token(&mut net, &server(), &email, &connection.device, &Credentials::Refresh(connection.refresh.as_deref().unwrap()), &Extra::default(), None).unwrap();
    assert!(refreshed.protected_key.is_none());
    assert!(!refreshed.access.is_empty());
    // The personal API key: no master password hash is made or sent, and the same key opens the same vault
    let secret = register::api_key_secret(&mut net, &server().api, &access.token, &email, PASSWORD, &pinned).unwrap();
    let client_id = format!("user.{}", connection.user_id);
    let pending = merge::prepare(&mut net, &crypto::master_key, server(), &email, PASSWORD.as_bytes(), None, None, Some(ApiKey { id: client_id.clone(), secret: secret.clone() }), connection.device.clone()).unwrap();
    assert!(pending.hash.is_none());
    let (by_key, by_key_access) = merge::finish(&mut net, &pending, &Extra::default()).unwrap();
    assert_eq!(by_key.key().unwrap().bytes()[..], connection.key().unwrap().bytes()[..]);
    assert!(by_key.refresh.is_none(), "the API key has no refresh token");
    // The token works for a sync
    let mut vault = Vault::default();
    assert!(vault.sync(&mut net, &by_key, &by_key_access).problem.is_none());
    // A wrong password is the server's own message, as data
    let pending = merge::prepare(&mut net, &crypto::master_key, server(), &email, b"not the password", None, None, None, merge::new_device_id()).unwrap();
    assert!(matches!(merge::finish(&mut net, &pending, &Extra::default()), Err(Problem::ServerSaid(_))));
    // A made-up address gets made-up settings that are within bounds, and the same each time
    let kdf_a = api::prelogin(&mut net, &server(), "nobody-at-all@example.test").unwrap();
    let kdf_b = api::prelogin(&mut net, &server(), "nobody-at-all@example.test").unwrap();
    assert!(kdf_a.same(&kdf_b));
    // And what the server says of itself is only for showing
    let (name, version) = api::config(&mut net, &server()).unwrap();
    println!("login: the server calls itself {name:?} {version:?}; the settings were {pinned:?}");
}

// --- Every kind, both ways -------------------------------------------------------------------------------------------------------

#[test]
#[ignore = "needs the Vaultwarden container"]
fn every_kind_round_trips_both_ways_and_a_second_clients_writes_come_in() {
    let mut net = Net::new();
    let email = account(&mut net, "kinds");
    let (connection, access) = login(&mut net, &email);
    let user = connection.key().unwrap();
    let token = access.token.to_string();
    let mut vault = Vault::default();

    // Here: a password with an address, an authenticator account with its own Login, backup codes
    vault.put("password", "p1", password_value("Example", "person", "hunter2 ✓", "https://example.test/login"));
    let totp = Account { id: "a1".into(), kind: Some(OtpKind::Totp), issuer: "Elsewhere".into(), label: "me".into(), secret: Secret::new("JBSWY3DPEHPK3PXP"), ..Account::default() };
    vault.put("authenticator", "a1", serde_json::to_value(&totp).unwrap());
    let codes = BackupService { id: "c1".into(), service_name: "Example".into(), codes: vec![BackupCode::new("1111-2222"), BackupCode::new("3333-4444")], ..BackupService::default() };
    vault.put("backup-codes", "c1", serde_json::to_value(&codes).unwrap());
    let outcome = vault.sync(&mut net, &connection, &access);
    assert!(outcome.problem.is_none(), "{:?}", outcome.problem.map(|p| p.name()));
    assert_eq!(outcome.report.created_there, 3, "{:?}", outcome.report.refused);
    let ciphers = all_ciphers(&mut net, &token);
    assert_eq!(ciphers.len(), 3);
    for raw in &ciphers {
        assert!(raw["key"].as_str().is_some_and(|key| key.starts_with("2.")), "every item this client creates has an item key");
        cipher::open(raw, &user).unwrap();
    }
    let contents: Vec<Content> = ciphers.iter().map(|raw| cipher::open(raw, &user).unwrap().content).collect();
    assert!(contents.iter().any(|c| c.password.as_deref() == Some("hunter2 ✓") && c.username.as_deref() == Some("person") && c.uri.as_deref() == Some("https://example.test/login")));
    assert!(contents.iter().any(|c| c.totp.as_deref().is_some_and(|t| t.starts_with("otpauth://totp/Elsewhere:me?"))));
    assert!(contents.iter().any(|c| c.kind == 2 && c.notes.as_deref() == Some("[ ] 1111-2222\n[ ] 3333-4444")));
    // Nothing changed: a second run writes nothing
    let outcome = vault.sync(&mut net, &connection, &access);
    assert!(outcome.groups.is_empty() && outcome.problem.is_none());

    // There: another client adds a Login with a totp, a card and an identity; the Login comes in, the rest are counted
    let item = SymKey::random();
    let other = Content { kind: LOGIN, name: "Other".into(), username: Some("you".into()), password: Some("s3cret".into()), totp: Some("otpauth://totp/Other:you?secret=JBSWY3DPEHPK3PXP&issuer=Other".into()), uri: Some("https://other.test".into()), ..Content::default() };
    let body = cipher::create_body(&other, &connection.user_id, &user).unwrap();
    assert_eq!(raw(&mut net, "POST", "/ciphers", &token, Some(&body), true).0, 200);
    let card = json!({"encryptedFor": connection.user_id, "type": 3, "organizationId": null, "folderId": null, "favorite": false, "reprompt": 0, "fields": null, "notes": null,
        "key": EncString::seal(&user, item.bytes()).text(), "name": crypto::seal_text(&item, "A card"),
        "card": {"cardholderName": crypto::seal_text(&item, "A Person"), "brand": crypto::seal_text(&item, "Visa"), "number": crypto::seal_text(&item, "4111111111111111"), "expMonth": crypto::seal_text(&item, "1"), "expYear": crypto::seal_text(&item, "2030"), "code": crypto::seal_text(&item, "123")}});
    assert_eq!(raw(&mut net, "POST", "/ciphers", &token, Some(&card), true).0, 200);
    let identity = json!({"encryptedFor": connection.user_id, "type": 4, "organizationId": null, "folderId": null, "favorite": false, "reprompt": 0, "fields": null, "notes": null,
        "key": EncString::seal(&user, item.bytes()).text(), "name": crypto::seal_text(&item, "An identity"),
        "identity": {"firstName": crypto::seal_text(&item, "A"), "lastName": crypto::seal_text(&item, "Person")}});
    assert_eq!(raw(&mut net, "POST", "/ciphers", &token, Some(&identity), true).0, 200);
    let outcome = vault.sync(&mut net, &connection, &access);
    assert!(outcome.problem.is_none(), "{:?}", outcome.problem.map(|p| p.name()));
    assert_eq!(outcome.report.left_alone, 2, "the card and the identity are left alone");
    let new = vault.of_kind("password").into_iter().find(|local| local.value["title"] == "Other").expect("it came in").clone();
    assert_eq!(new.value["password"], "s3cret");
    assert!(vault.of_kind("authenticator").iter().any(|local| local.value["issuer"] == "Other"));
    let after = all_ciphers(&mut net, &token);
    assert_eq!(after.len(), 6);
    assert!(after.iter().any(|raw| raw["type"] == json!(3) && raw["card"]["number"].is_string()), "the card is as it was");

    // The other client edits the password; here it comes in. An edit here goes there.
    let other_id = vault.links.values().find(|link| link.parts.iter().any(|part| part.id == new.id)).unwrap().cipher.clone();
    let raw_other = cipher_by_id(&after, &other_id).clone();
    let opened = cipher::open(&raw_other, &user).unwrap();
    let edited = Content { password: Some("changed there".into()), ..opened.content.clone() };
    let body = cipher::update_body(&raw_other, &opened.content, &edited, &connection.user_id, &user).unwrap();
    assert_eq!(raw(&mut net, "PUT", &format!("/ciphers/{other_id}"), &token, Some(&body), true).0, 200);
    vault.sync(&mut net, &connection, &access);
    assert_eq!(vault.value("password", &new.id)["password"], "changed there");
    let mut mine: Password = serde_json::from_value(vault.value("password", "p1")).unwrap();
    mine.set_password(Secret::new("changed here"), 5);
    vault.put("password", "p1", serde_json::to_value(mine).unwrap());
    let outcome = vault.sync(&mut net, &connection, &access);
    assert_eq!(outcome.report.sent, 1, "{:?}", outcome.report.refused);
    let there = all_ciphers(&mut net, &token);
    let p1_cipher = vault.links.values().find(|link| link.parts.iter().any(|part| part.id == "p1")).unwrap().cipher.clone();
    assert_eq!(cipher::open(cipher_by_id(&there, &p1_cipher), &user).unwrap().content.password.as_deref(), Some("changed here"));

    // Both sides changed: both are kept
    let raw_p1 = cipher_by_id(&there, &p1_cipher).clone();
    let opened = cipher::open(&raw_p1, &user).unwrap();
    let theirs = Content { password: Some("their edit".into()), ..opened.content.clone() };
    let body = cipher::update_body(&raw_p1, &opened.content, &theirs, &connection.user_id, &user).unwrap();
    assert_eq!(raw(&mut net, "PUT", &format!("/ciphers/{p1_cipher}"), &token, Some(&body), true).0, 200);
    let mut mine: Password = serde_json::from_value(vault.value("password", "p1")).unwrap();
    mine.set_password(Secret::new("my edit"), 6);
    vault.put("password", "p1", serde_json::to_value(mine).unwrap());
    let outcome = vault.sync(&mut net, &connection, &access);
    assert_eq!(outcome.report.conflicts, 1, "{:?}", outcome.report.refused);
    let secrets: Vec<String> = vault.of_kind("password").iter().filter_map(|local| local.value["password"].as_str().map(str::to_string)).collect();
    assert!(secrets.contains(&"their edit".to_string()) && secrets.contains(&"my edit".to_string()), "{secrets:?}");

    // Removed here: into the server's trash, never destroyed. Trashed there: into this vault's trash, and back when restored.
    let removed_id = vault.links.values().find(|link| link.parts.iter().any(|part| part.id == "c1")).unwrap().cipher.clone();
    vault.locals.remove(&("backup-codes".into(), "c1".into()));
    vault.sync(&mut net, &connection, &access);
    let there = all_ciphers(&mut net, &token);
    assert!(cipher_by_id(&there, &removed_id)["deletedDate"].is_string(), "in the trash, still there");
    assert_eq!(raw(&mut net, "PUT", &format!("/ciphers/{other_id}/delete"), &token, None, true).0, 200);
    vault.sync(&mut net, &connection, &access);
    assert!(vault.value("password", &new.id)["deletedAt"].is_number());
    assert_eq!(raw(&mut net, "PUT", &format!("/ciphers/{other_id}/restore"), &token, None, true).0, 200);
    vault.sync(&mut net, &connection, &access);
    assert!(vault.value("password", &new.id).get("deletedAt").is_none_or(Value::is_null));
    println!("kinds: both ways, conflicts and trash all as the spec says on Vaultwarden 1.37.5");
}

// --- A thousand items --------------------------------------------------------------------------------------------------------------

#[test]
#[ignore = "measures, by hand"]
fn a_thousand_items_each_way() {
    let mut net = Net::new();
    let email = account(&mut net, "thousand");
    let (connection, access) = login(&mut net, &email);
    let mut vault = Vault::default();
    for n in 0..1000 {
        vault.put("password", &format!("p{n:04}"), password_value(&format!("Site {n}"), &format!("user{n}"), &format!("secret-{n}"), &format!("https://site{n}.example.test/login")));
    }
    let started = Instant::now();
    let outcome = vault.sync(&mut net, &connection, &access);
    println!("push of 1000 new items (a POST each): {:?}, created {}, problem {:?}", started.elapsed(), outcome.report.created_there, outcome.problem.map(|p| p.name()));
    let started = Instant::now();
    let outcome = vault.sync(&mut net, &connection, &access);
    println!("a run with nothing changed: {:?}, groups {}", started.elapsed(), outcome.groups.len());
    let mut changed: Password = serde_json::from_value(vault.value("password", "p0500")).unwrap();
    changed.set_password(Secret::new("a new one"), 9);
    vault.put("password", "p0500", serde_json::to_value(changed).unwrap());
    let started = Instant::now();
    let outcome = vault.sync(&mut net, &connection, &access);
    println!("a run with one change among 1000: {:?}, sent {}", started.elapsed(), outcome.report.sent);
    // A second machine with an empty vault takes all of them
    let mut second = Vault::default();
    let started = Instant::now();
    let outcome = second.sync(&mut net, &connection, &access);
    println!("pull of 1000 into an empty vault: {:?}, received {}, problem {:?}", started.elapsed(), outcome.report.received, outcome.problem.map(|p| p.name()));
    assert_eq!(second.of_kind("password").len(), 1000);
}

// --- Argon2id ----------------------------------------------------------------------------------------------------------------------

#[test]
#[ignore = "needs the Vaultwarden container"]
fn an_argon2_account_logs_in_and_the_pin_holds() {
    let mut net = Net::new();
    let email = unique_email("argon");
    let argon = Kdf { kind: 1, iterations: 3, memory: Some(64), parallelism: Some(4) };
    register::register(&mut net, &server().identity, &email, PASSWORD, &argon).expect("registered");
    let pending = merge::prepare(&mut net, &crypto::master_key, server(), &email, PASSWORD.as_bytes(), None, None, None, merge::new_device_id()).expect("prelogin");
    let (connection, access) = merge::finish(&mut net, &pending, &Extra::default()).expect("login");
    assert_eq!(connection.kdf, argon, "the settings of the last good login are what is pinned");
    let mut vault = Vault::default();
    assert!(vault.sync(&mut net, &connection, &access).problem.is_none());
    // A server that asks for less than the pin is refused before anything is computed or sent
    let higher = Kdf { iterations: 4, ..argon };
    let refused = merge::prepare(&mut net, &crypto::master_key, server(), &email, PASSWORD.as_bytes(), None, Some(&higher), None, merge::new_device_id());
    assert_eq!(refused.err(), Some(Problem::Kdf(crypto::KdfProblem::BelowPin)));
    let pbkdf2 = Kdf { kind: 0, iterations: 600_000, memory: None, parallelism: None };
    let refused = merge::prepare(&mut net, &crypto::master_key, server(), &email, PASSWORD.as_bytes(), None, Some(&pbkdf2), None, merge::new_device_id());
    assert_eq!(refused.err(), Some(Problem::Kdf(crypto::KdfProblem::BelowPin)), "another KDF than the pin is below it");
}

// --- A second factor ------------------------------------------------------------------------------------------------------------

#[test]
#[ignore = "needs the Vaultwarden container"]
fn an_authenticator_code_is_asked_for_and_answered() {
    let mut net = Net::new();
    let email = account(&mut net, "twofactor");
    let (connection, access) = login(&mut net, &email);
    let master = crypto::master_key(PASSWORD.as_bytes(), &email, &kdf()).unwrap();
    let hash = crypto::master_hash(&master, PASSWORD.as_bytes());
    // Turn the Authenticator on through the API, as the web vault does: the server makes a key, and the code proves it was kept
    let (status, got) = register::call(&mut net, "POST", &format!("{}/two-factor/get-authenticator", server().api), Some(&access.token), Some(&json!({"masterPasswordHash": hash.as_str()})));
    assert_eq!(status, 200, "{got}");
    let key = amethystora_vault_core::sync::json::path(&got, &["authenticator", "key"]).and_then(Value::as_str).unwrap_or_else(|| panic!("a key in {got}")).to_string();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let totp = Account { kind: Some(OtpKind::Totp), secret: Secret::new(key.as_str()), ..Account::default() };
    let code = amethystora_vault_core::otp::code_at(&totp, now, 0);
    let body = json!({"key": key, "token": code.as_str(), "masterPasswordHash": hash.as_str(), "userVerificationToken": amethystora_vault_core::sync::json::get(&got, "userVerificationToken")});
    let (status, enabled) = register::call(&mut net, "PUT", &format!("{}/two-factor/authenticator", server().api), Some(&access.token), Some(&body));
    assert_eq!(status, 200, "{enabled}");
    // A password login now asks, and says which providers it offers; the API key does not ask
    let pending = merge::prepare(&mut net, &crypto::master_key, server(), &email, PASSWORD.as_bytes(), None, None, None, connection.device.clone()).unwrap();
    assert_eq!(merge::finish(&mut net, &pending, &Extra::default()).err(), Some(Problem::TwoFactor(vec![0])));
    let wrong = Extra { two_factor: Some((0, "000000", false)), ..Extra::default() };
    assert!(matches!(merge::finish(&mut net, &pending, &wrong), Err(Problem::ServerSaid(_))));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    // The next period's code: the server takes one step of drift, and refuses a code it has already taken
    let code = amethystora_vault_core::otp::code_at(&totp, now, 1);
    let extra = Extra { two_factor: Some((0, code.as_str(), false)), ..Extra::default() };
    let (second, _) = merge::finish(&mut net, &pending, &extra).expect("the code was accepted");
    assert_eq!(second.key().unwrap().bytes()[..], connection.key().unwrap().bytes()[..]);
    let secret = register::api_key_secret(&mut net, &server().api, &access.token, &email, PASSWORD, &kdf()).unwrap();
    let pending = merge::prepare(&mut net, &crypto::master_key, server(), &email, PASSWORD.as_bytes(), None, None, Some(ApiKey { id: format!("user.{}", connection.user_id), secret }), connection.device.clone()).unwrap();
    assert!(merge::finish(&mut net, &pending, &Extra::default()).is_ok(), "the API key asks for no second factor");
}

// --- The login limit ---------------------------------------------------------------------------------------------------------------

#[test]
#[ignore = "needs a Vaultwarden with its default login limits: VW_RATE= vwrun.sh"]
fn too_many_logins_are_a_wait_and_never_a_loop() {
    use std::sync::atomic::Ordering;
    let mut net = Net::new();
    let email = account(&mut net, "limit");
    let pending = merge::prepare(&mut net, &crypto::master_key, server(), &email, b"not the password", None, None, None, merge::new_device_id()).unwrap();
    let before = CALLS.load(Ordering::SeqCst);
    let mut attempts = 0;
    let mut limited = false;
    for _ in 0..30 {
        attempts += 1;
        match merge::finish(&mut net, &pending, &Extra::default()) {
            Err(Problem::RateLimited) => {
                limited = true;
                break;
            }
            Err(Problem::ServerSaid(_)) => {}
            other => panic!("unexpected: {:?}", other.err().map(|problem| problem.name())),
        }
    }
    assert!(limited, "Vaultwarden's default limit never came");
    // Each attempt is one login request, and a /sync after a good one: the client retried nothing by itself
    assert_eq!(CALLS.load(Ordering::SeqCst) - before, attempts, "one request for each attempt");
    println!("limit: the {attempts}th login in a row was refused with 429, with no retry by the client");
}

// --- Each kind, plain and sealed, with files (plans/vault-sync/spec-attachments.md, its appendix) ----------------------------------------

use amethystora_vault_core::keys::{self, KeyType};
use amethystora_vault_core::protocol::encode as b64;
use amethystora_vault_core::sync::attach;

fn key_local(id: &str, name: &str, key_kind: &str, key_type: KeyType) -> Local {
    let key = keys::generate(key_type).unwrap();
    let public = key.public().ok();
    let public_json = public.as_ref().map_or(Value::Null, |forms| json!({"type": key_type.name(), "pem": forms.spki_pem, "openssh": forms.openssh, "sha256": forms.sha256, "ssh_sha256": forms.ssh_sha256}));
    let value = json!({"key_kind": key_kind, "type": key_type.name(), "key": b64(key.as_bytes()), "exportable": true, "public": public_json, "comment": null});
    Local { kind: "key".into(), id: id.into(), name: name.into(), meta: json!({"key_kind": key_kind, "type": key_type.name(), "exportable": true}), approval: Approval::WhileOpen, value }
}

fn cert_local(id: &str, name: &str, chain_copies: usize) -> Local {
    use openssl::asn1::Asn1Time;
    use openssl::bn::BigNum;
    use openssl::hash::MessageDigest;
    use openssl::pkey::PKey;
    use openssl::x509::{X509Builder, X509NameBuilder};
    let pkey = PKey::generate_ed25519().unwrap();
    let mut subject = X509NameBuilder::new().unwrap();
    subject.append_entry_by_text("CN", name).unwrap();
    let subject = subject.build();
    let mut builder = X509Builder::new().unwrap();
    builder.set_version(2).unwrap();
    builder.set_serial_number(&BigNum::from_u32(7).unwrap().to_asn1_integer().unwrap()).unwrap();
    builder.set_subject_name(&subject).unwrap();
    builder.set_issuer_name(&subject).unwrap();
    builder.set_pubkey(&pkey).unwrap();
    builder.set_not_before(&Asn1Time::days_from_now(0).unwrap()).unwrap();
    builder.set_not_after(&Asn1Time::days_from_now(365).unwrap()).unwrap();
    builder.sign(&pkey, MessageDigest::null()).unwrap();
    let der = builder.build().to_der().unwrap();
    let told = amethystora_vault_core::pki::describe(&der).unwrap();
    let key = pkey.private_key_to_pkcs8().unwrap();
    let chain: Vec<String> = (0..chain_copies).map(|_| b64(&der)).collect();
    let value = json!({"profile": "code", "type": "ed25519", "key": b64(&key), "cert": b64(&der), "chain": chain, "fields": {}});
    let meta = json!({"profile": "code", "subject": told.subject, "issuer": told.issuer, "not_before": told.not_before, "not_after": told.not_after, "sha256": told.sha256, "spki_sha256": told.key.sha256, "key_type": "ed25519"});
    Local { kind: "certificate".into(), id: id.into(), name: name.into(), meta, approval: Approval::WhileOpen, value }
}

fn file_local(id: &str, path: &str, bytes: &[u8]) -> Local {
    let value = json!({"what": "file", "files": [{"at": "", "mode": 0o600, "data": b64(bytes)}], "folders": []});
    Local { kind: "protected".into(), id: id.into(), name: path.into(), meta: json!({"path": path, "what": "file", "size": bytes.len(), "files": 1}), approval: Approval::WhileOpen, value }
}

fn folder_local(id: &str, path: &str) -> Local {
    let value = json!({"what": "folder", "files": [{"at": "a/one.txt", "mode": 0o600, "data": b64(b"one")}, {"at": "two.bin", "mode": 0o644, "data": b64(&vec![9u8; 6000])}], "folders": [{"at": "a", "mode": 0o700}]});
    Local { kind: "protected".into(), id: id.into(), name: path.into(), meta: json!({"path": path, "what": "folder", "files": 2}), approval: Approval::WhileOpen, value }
}

fn add(vault: &mut Vault, local: Local) {
    vault.locals.insert((local.kind.clone(), local.id.clone()), local);
}

fn live(ciphers: &[Value]) -> Vec<&Value> {
    ciphers.iter().filter(|raw| raw["deletedDate"].is_null()).collect()
}

/// A file as Bitwarden's own clients fetch it: the address from the server, then the bytes with no credential, then the opening
fn download(net: &mut Net, access: &str, user: &SymKey, raw: &Value) -> (String, Vec<u8>) {
    let id = raw["id"].as_str().unwrap();
    let list = cipher::attachments(raw);
    assert_eq!(list.len(), 1, "one file on {id}");
    let (status, info) = self::raw(net, "GET", &format!("/ciphers/{id}/attachment/{}", list[0].id), access, None, true);
    assert_eq!(status, 200, "{info}");
    // The server may name any host (its DOMAIN): what is taken of the address is its path and token, built onto the connected address
    let url = info["url"].as_str().unwrap();
    let at = url.find("/attachments/").expect("an address under /attachments/");
    assert!(url.contains("?token="), "{url}");
    let fetched = net.call(Request { method: "GET", url: format!("{ADDRESS}{}", &url[at..]), headers: vec![("Accept", "*/*".to_string())], body: Zeroizing::new(Vec::new()) }).expect("an answer");
    assert_eq!(fetched.status, 200);
    assert_eq!(fetched.body.len() as u64, list[0].size, "the size the server lists is the encrypted bytes'");
    let item = cipher::key_of(raw, user).unwrap();
    let record = cipher::attachment_of(raw, &list[0].id).unwrap();
    let name = crypto::open_text(record["fileName"].as_str().unwrap(), &item).unwrap().to_string();
    (name, attach::open(&item, record, &fetched.body).expect("opens under the cipher's key").to_vec())
}

#[test]
#[ignore = "needs the Vaultwarden container"]
fn every_kind_plain_and_sealed_goes_both_ways_and_files_open_as_bitwardens_own_clients_open_them() {
    let mut net = Net::new();
    let email = account(&mut net, "kindsfiles");
    let (mut connection, access) = login(&mut net, &email);
    let user = connection.key().unwrap();
    let token = access.token.to_string();
    for kind in ["key", "certificate", "protected"] {
        connection.kinds.insert(kind.into(), "on".into());
    }
    let mut vault = Vault::default();
    let ssh = key_local("k1", "Work key", "ssh", KeyType::Ed25519);
    let ssh_b64 = ssh.value["key"].as_str().unwrap().to_string();
    add(&mut vault, ssh);
    add(&mut vault, key_local("k2", "Exchange key", "exchange", KeyType::X25519));
    add(&mut vault, cert_local("c1", "Signing", 1));
    add(&mut vault, cert_local("c2", "Long chain", 40));
    add(&mut vault, file_local("f1", ".npmrc", b"//registry.example/:_authToken=NPM-SECRET\n"));
    add(&mut vault, folder_local("d1", ".config/tool"));
    vault.put("password", "p1", password_value("Site", "person", "hunter2", "https://example.test"));
    let outcome = vault.sync(&mut net, &connection, &access);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?} {:?}", outcome.problem.map(|p| p.name()), outcome.report.refused);
    assert_eq!(outcome.report.created_there, 7);
    assert!(outcome.files_read, "the first file came back whole");
    let ciphers = all_ciphers(&mut net, &token);
    assert_eq!(live(&ciphers).len(), 7);
    // An SSH key item, as the server returns it with the version header
    let ssh_item = live(&ciphers).into_iter().find(|raw| raw["type"] == json!(5)).expect("an SSH key item");
    let opened = cipher::open(ssh_item, &user).unwrap();
    assert!(opened.content.ssh.as_ref().is_some_and(|ssh| ssh.private.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----")));
    // Files open in Bitwarden's own terms: the address the server gives, no credential, the cipher's key
    let mut names = Vec::new();
    for raw in live(&ciphers).into_iter().filter(|raw| !cipher::attachments(raw).is_empty()) {
        let (name, bytes) = download(&mut net, &token, &user, raw);
        if name == ".npmrc" {
            assert_eq!(bytes, b"//registry.example/:_authToken=NPM-SECRET\n");
        } else if name == "Long chain.pem" {
            assert!(String::from_utf8(bytes).unwrap().matches("-----BEGIN CERTIFICATE-----").count() == 41);
        } else {
            assert_eq!(name, "tool.tar");
            assert!(bytes.len() % 512 == 0 && bytes.starts_with(b"a/"), "a tar archive");
        }
        names.push(name);
    }
    names.sort();
    assert_eq!(names, [".npmrc", "Long chain.pem", "tool.tar"]);
    // Nothing changed: a second run writes nothing
    let again = vault.sync(&mut net, &connection, &access);
    assert!(again.groups.is_empty() && again.problem.is_none(), "{:?}", again.report.refused);
    // Another computer takes the key in, and not the certificate or the file
    let mut elsewhere = Vault::default();
    let outcome = elsewhere.sync(&mut net, &connection, &access);
    assert!(outcome.problem.is_none(), "{:?}", outcome.report.refused);
    assert_eq!(elsewhere.of_kind("key").iter().map(|local| local.value["key"].as_str().unwrap().to_string()).filter(|key| *key == ssh_b64).count(), 1);
    assert!(elsewhere.of_kind("certificate").is_empty() && elsewhere.of_kind("protected").is_empty());
    // A file another app adds to an item of ours survives our next PUT, byte for byte, and is never fetched by this client
    let note = live(&ciphers).into_iter().find(|raw| cipher::open(raw, &user).is_ok_and(|o| o.content.marker.as_deref() == Some("key") && o.content.kind == 2)).unwrap();
    let note_id = note["id"].as_str().unwrap().to_string();
    let item = cipher::key_of(note, &user).unwrap();
    let foreign = attach::prepare(&item, "from-an-app.txt", b"hello from another app").unwrap();
    let (status, slot) = raw(&mut net, "POST", &format!("/ciphers/{note_id}/attachment/v2"), &token, Some(&json!({"key": foreign.key, "fileName": foreign.name, "fileSize": foreign.size(), "adminRequest": false, "lastKnownRevisionDate": note["revisionDate"]})), true);
    assert_eq!(status, 200, "{slot}");
    assert_eq!((slot["object"].as_str(), slot["fileUploadType"].as_i64()), (Some("attachment-fileUpload"), Some(0)));
    // The slot is on the cipher before any file, and the cipher's date has not moved (Vaultwarden bumps the account's only)
    let (_, fetched) = raw(&mut net, "GET", &format!("/ciphers/{note_id}"), &token, None, true);
    assert_eq!(fetched["revisionDate"], note["revisionDate"]);
    assert_eq!(cipher::attachments(&fetched).len(), 1);
    // The upload, in the one part
    let boundary = "----testboundary";
    let upload = attach::multipart(boundary, &foreign.bytes);
    let response = net.call(Request { method: "POST", url: format!("{}/ciphers/{note_id}/attachment/{}", server().api, slot["attachmentId"].as_str().unwrap()), headers: vec![("Accept", "application/json".to_string()), ("Authorization", format!("Bearer {token}")), ("Content-Type", format!("multipart/form-data; boundary={boundary}"))], body: upload }).unwrap();
    assert_eq!(response.status, 200);
    // Our own change: the key's name
    let mut renamed = vault.locals.get(&("key".to_string(), "k2".to_string())).unwrap().clone();
    renamed.name = "Renamed exchange key".into();
    add(&mut vault, renamed);
    let before_calls: Vec<String> = URLS.lock().unwrap().clone();
    let outcome = vault.sync(&mut net, &connection, &access);
    assert!(outcome.problem.is_none(), "{:?}", outcome.report.refused);
    let after = URLS.lock().unwrap().clone();
    assert!(after[before_calls.len()..].iter().all(|url| !url.contains(slot["attachmentId"].as_str().unwrap())), "a file not made here was fetched");
    let (_, fetched) = raw(&mut net, "GET", &format!("/ciphers/{note_id}"), &token, None, true);
    assert_eq!(cipher::open(&fetched, &user).unwrap().content.name, "Renamed exchange key");
    assert_eq!(cipher::attachments(&fetched).len(), 1, "the file another app added is still there");
    let (_, bytes) = download(&mut net, &token, &user, &fetched);
    assert_eq!(bytes, b"hello from another app");
    // A changed file is a new cipher with a file of its own, and the old one is in the trash with its file
    add(&mut vault, file_local("f1", ".npmrc", b"//registry.example/:_authToken=NEW-NPM-SECRET\n"));
    let old = ciphers.iter().find(|raw| cipher::open(raw, &user).is_ok_and(|o| o.content.name == ".npmrc")).unwrap()["id"].as_str().unwrap().to_string();
    let outcome = vault.sync(&mut net, &connection, &access);
    assert!(outcome.problem.is_none() && outcome.report.created_there == 1 && outcome.report.trashed_there == 1, "{:?}", outcome.report.refused);
    let ciphers = all_ciphers(&mut net, &token);
    assert!(!cipher_by_id(&ciphers, &old)["deletedDate"].is_null());
    assert_eq!(cipher::attachments(cipher_by_id(&ciphers, &old)).len(), 1);
    // Sealed, the whole kinds: the old ones to the trash, new ones nobody but this vault reads, files included
    for kind in ["key", "certificate", "protected", "password"] {
        connection.kinds.insert(kind.into(), "sealed".into());
    }
    let outcome = vault.sync(&mut net, &connection, &access);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    let ciphers = all_ciphers(&mut net, &token);
    // The plain note holds a file of the vault's, which cannot be taken off a cipher: it goes to the trash whole, the file another app
    // added to it with it (the trash keeps it 30 days), and nothing of the plain is left in a live cipher
    assert_eq!(live(&ciphers).len(), 7);
    let left = cipher_by_id(&ciphers, &note_id);
    assert!(!left["deletedDate"].is_null() && cipher::attachments(left).len() == 1, "the note with the other app's file is in the trash with it");
    for raw in live(&ciphers) {
        let content = cipher::open(raw, &user).unwrap().content;
        assert_eq!(content.name, "Amethystora sealed item", "{}", content.name);
        let notes = content.notes.unwrap();
        assert!(notes.starts_with("amethystora-vault-sealed-1\n") && !notes.contains("BEGIN") && !notes.contains("NPM-SECRET"));
        if !cipher::attachments(raw).is_empty() {
            let (name, bytes) = download(&mut net, &token, &user, raw);
            assert_eq!(name, "amethystora-sealed.bin");
            assert!(!String::from_utf8_lossy(&bytes).contains("NPM-SECRET") && !String::from_utf8_lossy(&bytes).contains("BEGIN") && bytes.starts_with(b"AVs1"), "a blob only the vault opens");
        }
    }
    // A second computer with this vault restored opens all of it
    let mut restored = Vault { sealing: vault.sealing.as_ref().map(|(key, id)| (key.copy(), id.clone())), ..Vault::default() };
    let outcome = restored.sync(&mut net, &connection, &access);
    assert!(outcome.problem.is_none(), "{:?}", outcome.report.refused);
    for (kind, id) in [("key", "k1"), ("key", "k2"), ("certificate", "c1"), ("certificate", "c2"), ("protected", "f1"), ("protected", "d1"), ("password", "p1")] {
        assert_eq!(restored.value(kind, id), vault.value(kind, id), "{kind}.{id}");
    }
    // Off moves every copy to the trash, and nothing is destroyed
    for kind in ["key", "certificate", "protected", "password"] {
        connection.kinds.insert(kind.into(), "off".into());
    }
    let outcome = vault.sync(&mut net, &connection, &access);
    assert!(outcome.problem.is_none(), "{:?}", outcome.report.refused);
    let ciphers = all_ciphers(&mut net, &token);
    assert!(live(&ciphers).iter().all(|raw| raw["id"] == json!(note_id)), "only what is not ours is live");
    assert!(ciphers.len() >= 14);
}

#[test]
#[ignore = "needs the Vaultwarden container"]
fn a_slot_opened_and_never_filled_is_the_one_delete_and_nothing_else_is_destroyed() {
    let mut net = Net::new();
    let email = account(&mut net, "ghost");
    let (connection, access) = login(&mut net, &email);
    let user = connection.key().unwrap();
    let token = access.token.to_string();
    let content = Content { kind: cipher::NOTE, name: "A note".into(), ..Content::default() };
    let (status, created) = raw(&mut net, "POST", "/ciphers", &token, Some(&cipher::create_body(&content, &connection.user_id, &user).unwrap()), true);
    assert_eq!(status, 200, "{created}");
    let id = created["id"].as_str().unwrap().to_string();
    let item = cipher::key_of(&created, &user).unwrap();
    let prepared = attach::prepare(&item, "never-filled.bin", b"data").unwrap();
    let (status, slot) = raw(&mut net, "POST", &format!("/ciphers/{id}/attachment/v2"), &token, Some(&json!({"key": prepared.key, "fileName": prepared.name, "fileSize": prepared.size(), "adminRequest": false, "lastKnownRevisionDate": created["revisionDate"]})), true);
    assert_eq!(status, 200, "{slot}");
    let attachment = slot["attachmentId"].as_str().unwrap().to_string();
    assert!(attachment.len() == 20 && attachment.bytes().all(|b| b.is_ascii_hexdigit()), "Vaultwarden 1.37.5 makes 20 hex digits, not the 10 the spec says: {attachment}");
    // The record is there with no file, and a download of it fails
    let (_, listed) = raw(&mut net, "GET", &format!("/ciphers/{id}"), &token, None, true);
    assert_eq!(cipher::attachments(&listed).len(), 1);
    let (status, info) = raw(&mut net, "GET", &format!("/ciphers/{id}/attachment/{attachment}"), &token, None, true);
    assert_eq!(status, 200);
    let url = info["url"].as_str().unwrap();
    let fetched = net.call(Request { method: "GET", url: format!("{ADDRESS}{}", &url[url.find("/attachments/").unwrap()..]), headers: Vec::new(), body: Zeroizing::new(Vec::new()) }).unwrap();
    assert_ne!(fetched.status, 200, "a ghost has no file");
    // The client's one delete, through the network service's own rules
    let server = server();
    let mut api = api::Api { http: &mut net, server: &server, access: &token };
    api.drop_slot(&id, &attachment).expect("the delete goes through");
    let (_, listed) = raw(&mut net, "GET", &format!("/ciphers/{id}"), &token, None, true);
    assert!(cipher::attachments(&listed).is_empty(), "the slot is gone");
    // And no other delete is let through by the same session
    let (status, _) = raw(&mut net, "GET", &format!("/ciphers/{id}"), &token, None, true);
    assert_eq!(status, 200);
    let denied = net.0.request("DELETE", &format!("{}/ciphers/{id}", server.api), &[], b"");
    assert!(denied.is_err(), "a permanent delete of a cipher is refused");
}

// --- Bitwarden's own `bw` program, as a black box (spec-attachments 7), in three steps with `vault-tools/vwbw.sh` ------------------------------
//
// A makes the account and what this client sends and writes what `bw` has to check; `bw` (a throwaway Node container, `@bitwarden/cli` pinned) reads
// everything, adds a file to an item of ours and makes a Login; C changes an item of ours, and checks that what `bw` wrote survived and what it read
// was what was sent.

fn state_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("KINDS_BW_DIR").expect("KINDS_BW_DIR: a folder shared with the bw container"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    amethystora_vault_core::seal::hex(&openssl::sha::sha256(bytes))
}

fn dump(vault: &Vault) -> Value {
    let locals: Vec<Value> = vault.locals.values().map(|local| json!({"kind": local.kind, "id": local.id, "name": local.name, "meta": local.meta, "approval": local.approval, "value": local.value})).collect();
    json!({"locals": locals, "links": vault.links.values().collect::<Vec<_>>()})
}

fn restore(state: &Value) -> Vault {
    let mut vault = Vault::default();
    for local in state["locals"].as_array().unwrap() {
        add(&mut vault, Local { kind: local["kind"].as_str().unwrap().into(), id: local["id"].as_str().unwrap().into(), name: local["name"].as_str().unwrap().into(), meta: local["meta"].clone(), approval: serde_json::from_value(local["approval"].clone()).unwrap(), value: local["value"].clone() });
    }
    for link in state["links"].as_array().unwrap() {
        let link: Link = serde_json::from_value(link.clone()).unwrap();
        vault.links.insert(link.cipher.clone(), link);
    }
    vault
}

#[test]
#[ignore = "needs the Vaultwarden container and bw: vault-tools/vwbw.sh"]
fn bw_a_what_this_client_sends() {
    let dir = state_dir();
    let mut net = Net::new();
    let email = account(&mut net, "bw");
    let (mut connection, access) = login(&mut net, &email);
    connection.kinds.insert("key".into(), "on".into());
    connection.kinds.insert("protected".into(), "on".into());
    connection.kinds.insert("certificate".into(), "sealed".into());
    let mut vault = Vault::default();
    add(&mut vault, key_local("k1", "Work key", "ssh", KeyType::Ed25519));
    add(&mut vault, key_local("k2", "Exchange key", "exchange", KeyType::X25519));
    add(&mut vault, cert_local("c1", "Sealed certificate", 40));
    add(&mut vault, file_local("f1", ".npmrc", b"//registry.example/:_authToken=NPM-SECRET\n"));
    add(&mut vault, folder_local("d1", ".config/tool"));
    vault.put("password", "p1", password_value("Site", "person", "hunter2", "https://example.test"));
    let outcome = vault.sync(&mut net, &connection, &access);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    // What `bw` has to see
    let user = connection.key().unwrap();
    let ciphers = all_ciphers(&mut net, access.token.as_ref());
    let mut files = serde_json::Map::new();
    for raw in live(&ciphers).into_iter().filter(|raw| !cipher::attachments(raw).is_empty()) {
        let (name, bytes) = download(&mut net, access.token.as_ref(), &user, raw);
        files.insert(raw["id"].as_str().unwrap().to_string(), json!({"name": name, "sha256": sha256_hex(&bytes), "length": bytes.len()}));
    }
    let key_note = live(&ciphers).into_iter().find(|raw| cipher::open(raw, &user).is_ok_and(|o| o.content.kind == 2 && o.content.marker.as_deref() == Some("key"))).unwrap()["id"].as_str().unwrap().to_string();
    let ssh_item = live(&ciphers).into_iter().find(|raw| raw["type"] == json!(5)).unwrap();
    let ssh = cipher::open(ssh_item, &user).unwrap().content.ssh.unwrap();
    let state = json!({"email": email, "password": PASSWORD, "connection": connection, "vault": dump(&vault), "files": files, "key_note": key_note, "ssh_private": ssh.private, "ssh_fingerprint": ssh.fingerprint});
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("state.json"), serde_json::to_vec_pretty(&state).unwrap()).unwrap();
}

#[test]
#[ignore = "needs the Vaultwarden container and bw: vault-tools/vwbw.sh"]
fn bw_c_what_bw_read_and_wrote() {
    let dir = state_dir();
    let state: Value = serde_json::from_slice(&std::fs::read(dir.join("state.json")).unwrap()).unwrap();
    let mut net = Net::new();
    let email = state["email"].as_str().unwrap();
    let (mut connection, access) = login(&mut net, email);
    let stored: Connection = serde_json::from_value(state["connection"].clone()).unwrap();
    connection.kinds = stored.kinds.clone();
    connection.files_read = true;
    let user = connection.key().unwrap();
    let token = access.token.to_string();
    // What `bw` read is what was sent: each file by its digest, and the sealed ones are nothing readable
    let read: Value = serde_json::from_slice(&std::fs::read(dir.join("bw-read.json")).unwrap()).unwrap();
    let mut seen = 0;
    for (id, expected) in state["files"].as_object().unwrap() {
        let got = &read["files"][id];
        if expected["name"] == json!("amethystora-sealed.bin") {
            // `bw` opens Bitwarden's layer, and what it prints is the vault's own sealed blob, with no word of the secret
            assert_eq!(got["sha256"], expected["sha256"], "bw read the blob that was sent");
            assert_eq!(got["has_plaintext"], json!(false), "a sealed file shows nothing readable");
        } else {
            assert_eq!(got["name"], expected["name"], "{id}");
            assert_eq!(got["sha256"], expected["sha256"], "{id}: bw opened {} byte(s)", expected["length"]);
        }
        seen += 1;
    }
    assert!(seen >= 3, "{seen}");
    // The SSH key item opens in `bw` as an SSH key with this fingerprint, and the items are what was sent
    assert_eq!(read["ssh"]["fingerprint"], state["ssh_fingerprint"]);
    assert!(read["ssh"]["private_starts"].as_str().unwrap().starts_with("-----BEGIN OPENSSH PRIVATE KEY-----"));
    assert_eq!(read["sealed_notes_readable"], json!(false), "the sealed note's text says nothing of the secret");
    // `bw` added a file to an item of ours, and made a Login. Our next change must leave the file, and take the Login in.
    let note_id = state["key_note"].as_str().unwrap();
    let before = cipher::attachments(&all_ciphers(&mut net, &token).into_iter().find(|raw| raw["id"] == json!(note_id)).unwrap());
    assert_eq!(before.len(), 1, "the key is a note with no file of its own, so the one file is the one bw added: {before:?}");
    let mut vault = restore(&state["vault"]);
    let mut renamed = vault.locals.get(&("key".to_string(), "k2".to_string())).unwrap().clone();
    renamed.name = "Renamed by this client".into();
    add(&mut vault, renamed);
    let calls = URLS.lock().unwrap().len();
    let outcome = vault.sync(&mut net, &connection, &access);
    assert!(outcome.problem.is_none(), "{:?} {:?}", outcome.problem.map(|p| p.name()), outcome.report.refused);
    let run_urls: Vec<String> = URLS.lock().unwrap()[calls..].to_vec();
    let ciphers = all_ciphers(&mut net, &token);
    let note = ciphers.iter().find(|raw| raw["id"] == json!(note_id)).unwrap();
    assert_eq!(cipher::open(note, &user).unwrap().content.name, "Renamed by this client");
    let after = cipher::attachments(note);
    assert_eq!(after, before, "bw's file survived our update, with the same size");
    let (name, bytes) = download(&mut net, &token, &user, note);
    assert_eq!((name.as_str(), sha256_hex(&bytes)), (state["bw_file_name"].as_str().unwrap_or("from-bw.txt"), read["bw_file_sha256"].as_str().unwrap().to_string()));
    assert!(run_urls.iter().all(|url| !url.contains(&after[0].id)), "this client never fetched a file it did not make");
    // The Login `bw` made is a password here
    assert!(vault.of_kind("password").iter().any(|local| local.value["title"] == json!("Made by bw")), "{:?}", vault.of_kind("password").iter().map(|l| l.value["title"].clone()).collect::<Vec<_>>());
}
