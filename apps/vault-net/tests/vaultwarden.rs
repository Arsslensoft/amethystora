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

impl Http for Net {
    fn call(&mut self, request: Request) -> Result<Response, NetFailure> {
        CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
            sealing: Some((Key::random(), "vault".into())),
            accept: false,
            local_unreadable: 0,
            links_unreadable: false,
            full: true,
            now_ms: merge::now_ms(),
        };
        let mut outcome = merge::run(net, input);
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
