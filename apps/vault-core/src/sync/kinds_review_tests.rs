//! The one review of the work on kinds (plans/vault/work/review-kinds.md): each finding that was fixed has its test here, and each one
//! fails on the code before the fix.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use serde_json::{json, Value};

use super::cipher::{self, Content};
use super::convert::{CertFacts, Convert, Direct, KeyRead, SshForms};
use super::crypto;
use super::fake::{Fake, USER_ID};
use super::map::{self, Local};
use super::merge::{self, Access, Connection, Input};
use super::tests::{bump, connect, password, totp_account, Vault, NOW_MS};
use crate::keys::{self, KeyType};
use crate::protocol::Approval;
use crate::seal::Key;

fn on(connection: &mut Connection, kind: &str, choice: &str) {
    connection.kinds.insert(kind.into(), choice.into());
}

fn live_ids(fake: &Fake) -> Vec<String> {
    fake.ciphers.iter().filter(|raw| raw["deletedDate"].is_null()).map(|raw| raw["id"].as_str().unwrap().to_string()).collect()
}

fn opened(fake: &Fake, id: &str) -> cipher::Opened {
    cipher::open(fake.find(id).unwrap(), &fake.user).unwrap()
}

fn sealed_vault() -> Vault {
    Vault { sealing: Some((Key::random(), "vault-1".into())), ..Vault::default() }
}

fn input(vault: &Vault, fake: &Fake, connection: &Connection) -> Input {
    Input {
        connection: connection.clone(),
        access: Some(Access { token: zeroize::Zeroizing::new(fake.access()), until_ms: i64::MAX }),
        links: vault.links.values().cloned().collect(),
        locals: vault.locals.values().cloned().collect(),
        sealing: vault.sealing.as_ref().map(|(k, v)| (k.copy(), v.clone())),
        accept: false,
        full: true,
        now_ms: NOW_MS,
        local_unreadable: 0,
        links_unreadable: false,
        retry_files: false,
    }
}

const FIELD: &str = "Recovery question";

/// A Login as another app makes it, with what the vault does not map: a folder, and a field of the person's own
fn app_login(fake: &mut Fake, name: &str, secret: &str) -> String {
    let content = Content { kind: cipher::LOGIN, name: name.into(), username: Some("person".into()), password: Some(secret.into()), uri: Some("https://app.example.test".into()), ..Content::default() };
    let id = fake.store(cipher::create_body(&content, USER_ID, &fake.user).unwrap());
    add_foreign_field(fake, &id);
    fake.find_mut(&id).unwrap()["folderId"] = json!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
    id
}

fn add_foreign_field(fake: &mut Fake, id: &str) {
    let item = cipher::key_of(fake.find(id).unwrap(), &fake.user).unwrap();
    let field = json!({"type": 0, "name": crypto::seal_text(&item, FIELD), "value": crypto::seal_text(&item, "first pet"), "linkedId": null});
    let raw = fake.find_mut(id).unwrap();
    let mut fields = raw["fields"].as_array().cloned().unwrap_or_default();
    fields.push(field);
    raw["fields"] = Value::Array(fields);
    let now = fake.now();
    fake.find_mut(id).unwrap()["revisionDate"] = json!(now);
}

/// The person's field is still on the cipher, as it was
fn has_foreign_field(fake: &Fake, id: &str) -> bool {
    let raw = fake.find(id).unwrap();
    let item = cipher::key_of(raw, &fake.user).unwrap();
    raw["fields"].as_array().into_iter().flatten().any(|field| field["name"].as_str().is_some_and(|name| crypto::open_text(name, &item).is_ok_and(|name| name.as_str() == FIELD)))
}

// --- F2: a kind switch never destroys what the vault did not write ----------------------------------------------------------------

#[test]
fn f2_a_login_of_the_persons_is_cleared_of_what_the_vault_wrote_and_never_trashed_when_its_kind_goes_sealed() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    let mut vault = sealed_vault();
    let id = app_login(&mut fake, "From the app", "app-secret-pw");
    vault.sync(&mut fake, &connection);
    assert_eq!(vault.of_kind("password").len(), 1, "linked as a password");
    on(&mut connection, "password", "sealed");
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none(), "{:?}", outcome.report.refused);
    // Still there, not in the trash, with the person's folder and field; what the vault wrote is out of it
    assert!(fake.find(&id).unwrap()["deletedDate"].is_null(), "the person's Login went to the trash");
    let content = opened(&fake, &id).content;
    assert_eq!((content.password, content.username, content.uri), (None, None, None));
    assert_eq!(fake.find(&id).unwrap()["folderId"], json!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"));
    assert!(has_foreign_field(&fake, &id));
    // The sealed note stands beside it, and the vault's item is whole
    assert_eq!(live_ids(&fake).len(), 2);
    assert_eq!(vault.of_kind("password").len(), 1);
    assert_eq!(vault.of_kind("password")[0].value["password"], json!("app-secret-pw"));
    // What is left is not brought in again as an item
    vault.sync(&mut fake, &connection);
    assert_eq!(vault.of_kind("password").len(), 1);
    assert_eq!(live_ids(&fake).len(), 2);
}

#[test]
fn f2_a_part_that_goes_off_comes_out_of_the_cipher_and_the_cipher_stays() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    vault.put("password", "p1", password("Site", "u", "pw-secret", "https://a.test"));
    vault.sync(&mut fake, &connection);
    vault.put("authenticator", "a1", totp_account("Site", "me"));
    vault.sync(&mut fake, &connection);
    assert_eq!(live_ids(&fake).len(), 1);
    let id = live_ids(&fake)[0].clone();
    assert!(opened(&fake, &id).content.totp.is_some(), "the code rides in the Login");
    on(&mut connection, "authenticator", "off");
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none(), "{:?}", outcome.report.refused);
    // The same cipher, without the code and with the password: not trashed and made again
    assert_eq!(fake.ciphers.len(), 1);
    assert!(fake.ciphers[0]["deletedDate"].is_null());
    let content = opened(&fake, &id).content;
    assert_eq!((content.totp, content.password.as_deref()), (None, Some("pw-secret")));
    assert_eq!(vault.links.values().next().unwrap().parts.len(), 1);
    // The code is the vault's still, and is not sent anywhere
    assert_eq!(vault.of_kind("authenticator").len(), 1);
}

#[test]
fn f2_a_login_the_vault_made_is_not_trashed_once_the_person_put_something_in_it() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    let mut vault = sealed_vault();
    vault.put("password", "p1", password("Site", "u", "pw-secret", "https://a.test"));
    vault.sync(&mut fake, &connection);
    let id = live_ids(&fake)[0].clone();
    add_foreign_field(&mut fake, &id);
    on(&mut connection, "password", "sealed");
    vault.sync(&mut fake, &connection);
    assert!(fake.find(&id).unwrap()["deletedDate"].is_null());
    assert!(has_foreign_field(&fake, &id));
    assert_eq!(opened(&fake, &id).content.password, None);
    assert_eq!(live_ids(&fake).len(), 2, "and the sealed note beside it");
}

// --- F3: nothing is left half made on the server ---------------------------------------------------------------------------------

fn file_vault() -> Vault {
    let mut vault = Vault::default();
    let local = Local {
        kind: "protected".into(),
        id: "f1".into(),
        name: ".npmrc".into(),
        meta: json!({"path": ".npmrc", "what": "file", "size": 6, "files": 1}),
        approval: Approval::WhileOpen,
        value: json!({"what": "file", "files": [{"at": "", "mode": 0o600, "data": B64.encode(b"secret")}], "folders": []}),
    };
    vault.locals.insert((local.kind.clone(), local.id.clone()), local);
    vault
}

#[test]
fn f3_a_failure_after_the_cipher_was_made_trashes_it_and_drops_the_slot() {
    let mut fake = Fake::default();
    fake.status.insert("upload", 503);
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = file_vault();
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.problem.as_ref().map(|p| p.name()), Some("server-down"));
    assert!(live_ids(&fake).is_empty(), "a cipher made and not filled is left alone on the server");
    assert_eq!(fake.dropped.len(), 1, "the slot was dropped (the one delete there is)");
    assert!(outcome.pending.is_empty());
    assert!(vault.links.is_empty());
}

#[test]
fn f3_what_could_not_be_cleaned_is_carried_and_cleaned_by_the_next_run_before_anything_is_read() {
    let mut fake = Fake::default();
    fake.status.insert("upload", 503);
    fake.status.insert("drop", 503);
    fake.status.insert("trash", 503);
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = file_vault();
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_some());
    assert_eq!(outcome.pending.len(), 1, "{:?}", outcome.pending);
    assert!(outcome.pending[0].slot.is_some());
    assert_eq!(live_ids(&fake).len(), 1);
    // The next run starts by putting it right, and then makes the item as it should
    fake.status.clear();
    connection.pending = outcome.pending.clone();
    let again = vault.sync(&mut fake, &connection);
    assert!(again.problem.is_none() && again.pending.is_empty(), "{:?} {:?}", again.problem.map(|p| p.name()), again.report.refused);
    assert_eq!(fake.dropped.len(), 1);
    assert_eq!(live_ids(&fake).len(), 1, "one copy, the whole one");
    assert_eq!(vault.links.len(), 1);
    assert_eq!(fake.ciphers.len(), 2);
}

#[test]
fn f3_an_old_copy_that_could_not_be_trashed_does_not_make_a_third() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = file_vault();
    vault.sync(&mut fake, &connection);
    let old = live_ids(&fake)[0].clone();
    // The file changes: a new cipher stands, and the old one cannot be put in the trash just now
    let key = ("protected".to_string(), "f1".to_string());
    let mut local = vault.locals.get(&key).unwrap().clone();
    local.value["files"][0]["data"] = json!(B64.encode(b"changed"));
    vault.locals.insert(key, local);
    fake.status.insert("trash", 503);
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_some());
    assert_eq!(vault.links.len(), 1);
    assert_ne!(vault.links.keys().next().unwrap(), &old, "the new cipher is the item's now");
    assert_eq!(outcome.pending.iter().map(|left| left.cipher.clone()).collect::<Vec<_>>(), vec![old.clone()]);
    fake.status.clear();
    connection.pending = outcome.pending.clone();
    let again = vault.sync(&mut fake, &connection);
    assert!(again.problem.is_none() && again.report.created_there == 0, "no third copy: {:?}", again.report.refused);
    assert_eq!(live_ids(&fake).len(), 1);
    assert!(!fake.find(&old).unwrap()["deletedDate"].is_null());
}

// --- F5: an SSH item of the person's, and a form that stays ------------------------------------------------------------------------

fn ssh_item(fake: &mut Fake, name: &str, private: &str, public: &str) -> String {
    ssh_item_with(fake, name, private, public, "SHA256:from-the-app")
}

fn ssh_item_with(fake: &mut Fake, name: &str, private: &str, public: &str, fingerprint: &str) -> String {
    let content = Content { kind: cipher::SSH, name: name.into(), ssh: Some(cipher::Ssh { private: private.into(), public: public.into(), fingerprint: fingerprint.into() }), ..Content::default() };
    fake.store(cipher::create_body(&content, USER_ID, &fake.user).unwrap())
}

#[test]
fn f5_an_ssh_item_the_vault_could_not_write_back_is_not_brought_in() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "key", "on");
    // An RSA 2048 key, which Bitwarden's apps import and an SSH item here does not hold (only the six they make)
    let pem = String::from_utf8(openssl::pkey::PKey::from_rsa(openssl::rsa::Rsa::generate(2048).unwrap()).unwrap().private_key_to_pem_pkcs8().unwrap()).unwrap();
    ssh_item(&mut fake, "Old RSA key", &pem, "ssh-rsa AAAA old");
    let mut vault = Vault::default();
    let outcome = vault.sync(&mut fake, &connection);
    assert!(vault.of_kind("key").is_empty());
    assert_eq!(outcome.report.left_alone, 1);
}

#[test]
fn f5_a_persons_ssh_item_is_not_rewritten_for_a_name_and_keeps_what_it_said() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "key", "on");
    let key = keys::generate(KeyType::Ed25519).unwrap();
    let forms = crate::sync::convert::ssh_forms("ed25519", key.as_bytes()).unwrap();
    let public = format!("{} work-laptop", forms.public);
    let id = ssh_item_with(&mut fake, "From the app", &forms.private, &public, &forms.fingerprint);
    let mut vault = Vault::default();
    vault.sync(&mut fake, &connection);
    assert_eq!(vault.of_kind("key").len(), 1);
    let mut local = vault.of_kind("key")[0].clone();
    local.name = "Renamed here".into();
    vault.locals.insert((local.kind.clone(), local.id.clone()), local);
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    let content = opened(&fake, &id).content;
    assert_eq!(content.name, "Renamed here");
    assert_eq!(content.ssh.unwrap().public, public, "the comment of the person's key is still there");
    assert_eq!((content.marker, content.fields.len()), (None, 0), "nothing of the vault's was added to it");
    assert_eq!(fake.ciphers.len(), 1);
}

/// The worker gave no answer
struct NoSsh;

impl Convert for NoSsh {
    fn ssh_forms(&self, _: &str, _: &[u8]) -> Option<SshForms> {
        None
    }

    fn key_from_text(&self, text: &str) -> Option<KeyRead> {
        Direct.key_from_text(text)
    }

    fn cert_facts(&self, certs: &[Vec<u8>], key: Option<(&str, &[u8])>) -> Option<CertFacts> {
        Direct.cert_facts(certs, key)
    }

    fn accepts(&self, kind: &str, value: &Value) -> bool {
        Direct.accepts(kind, value)
    }
}

#[test]
fn f5_a_linked_cipher_keeps_its_form_when_the_worker_gives_no_answer() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "key", "on");
    let key = keys::generate(KeyType::Ed25519).unwrap();
    let forms = key.public().unwrap();
    let value = json!({"key_kind": "ssh", "type": "ed25519", "key": B64.encode(key.as_bytes()), "exportable": true, "public": {"openssh": forms.openssh, "sha256": forms.sha256, "ssh_sha256": forms.ssh_sha256}, "comment": null});
    let mut vault = Vault::default();
    let local = Local { kind: "key".into(), id: "k1".into(), name: "Work key".into(), meta: json!({"key_kind": "ssh", "type": "ed25519", "exportable": true}), approval: Approval::WhileOpen, value };
    vault.locals.insert(("key".into(), "k1".into()), local.clone());
    vault.sync(&mut fake, &connection);
    assert_eq!(fake.ciphers[0]["type"], json!(5));
    let mut renamed = local;
    renamed.name = "Renamed".into();
    vault.locals.insert(("key".into(), "k1".into()), renamed);
    let asked = input(&vault, &fake, &connection);
    let mut outcome = merge::run(&mut fake, &NoSsh, asked);
    vault.apply(&mut outcome);
    assert!(outcome.report.refused.iter().any(|r| r["item"] == "key.k1" && r["why"] == "unreadable"), "{:?}", outcome.report.refused);
    assert_eq!(fake.ciphers.len(), 1, "no note made in its place");
    assert!(fake.ciphers[0]["deletedDate"].is_null() && fake.ciphers[0]["type"] == json!(5));
    assert_eq!(vault.links.len(), 1);
}

// --- F6: the machine's identity certificate goes only by its own word -----------------------------------------------------------------

#[test]
fn f6_the_identity_of_this_computer_is_never_covered_by_the_kind() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "certificate", "on");
    let mut vault = Vault::default();
    let normal = super::kinds_tests::cert_local("c1", "Signing", 0);
    let mut identity = super::kinds_tests::cert_local("c2", "This computer", 0);
    identity.meta["profile"] = json!("device-identity");
    vault.locals.insert(("certificate".into(), "c1".into()), normal);
    vault.locals.insert(("certificate".into(), "c2".into()), identity);
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.created_there, 1);
    assert_eq!(fake.ciphers.len(), 1);
    let name = opened(&fake, fake.ciphers[0]["id"].as_str().unwrap()).content.name;
    assert_eq!(name, "Signing");
    // Its own word sends it
    connection.choices.insert("certificate.c2".into(), "on".into());
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.created_there, 1);
    assert_eq!(fake.ciphers.len(), 2);
    assert_eq!(connection.choice_of("certificate", "c3", &json!({"profile": "device-identity"})), "off");
    assert_eq!(connection.choice_of("certificate", "c3", &json!({"profile": "code"})), "on");
}

// --- F7, F9, F10, F11 -----------------------------------------------------------------------------------------------------------

#[test]
fn f7_a_refusal_that_is_not_the_servers_does_not_turn_files_off() {
    let mut fake = Fake { slot_status: Some(404), ..Fake::default() };
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = file_vault();
    let mut second = vault.locals.values().next().unwrap().clone();
    second.id = "f2".into();
    second.name = ".netrc".into();
    vault.locals.insert(("protected".into(), "f2".into()), second);
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.no_files, None, "{:?}", outcome.report.refused);
    assert_eq!(outcome.report.refused_total, 2);
}

#[test]
fn f9_a_lost_file_goes_again_when_the_person_accepts() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = file_vault();
    vault.sync(&mut fake, &connection);
    let id = live_ids(&fake)[0].clone();
    fake.find_mut(&id).unwrap()["attachments"] = json!([]);
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "attachment-lost" && r["item"] == "protected.f1"));
    assert_eq!(live_ids(&fake), vec![id.clone()]);
    let mut asked = input(&vault, &fake, &connection);
    asked.accept = true;
    let mut outcome = merge::run(&mut fake, &Direct, asked);
    vault.apply(&mut outcome);
    assert!(outcome.problem.is_none() && outcome.report.created_there == 1, "{:?}", outcome.report.refused);
    assert_eq!(live_ids(&fake).len(), 1);
    assert_ne!(live_ids(&fake)[0], id);
    assert_eq!(cipher::attachments(fake.find(&live_ids(&fake)[0]).unwrap()).len(), 1);
}

#[test]
fn f10_a_chain_that_is_not_a_chain_is_not_taken_back() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "certificate", "on");
    let mut vault = Vault::default();
    let local = super::kinds_tests::cert_local("c1", "Signing", 0);
    vault.locals.insert(("certificate".into(), "c1".into()), local.clone());
    vault.sync(&mut fake, &connection);
    let id = live_ids(&fake)[0].clone();
    let item = cipher::key_of(fake.find(&id).unwrap(), &fake.user).unwrap();
    // The leaf, then a certificate that did not sign it
    let stranger = super::kinds_tests::cert_local("c2", "Stranger", 0);
    let text = format!("{}{}", keys::pem("CERTIFICATE", &B64.decode(local.value["cert"].as_str().unwrap()).unwrap()), keys::pem("CERTIFICATE", &B64.decode(stranger.value["cert"].as_str().unwrap()).unwrap()));
    bump(&mut fake, &id, |raw| raw["notes"] = json!(crypto::seal_text(&item, &text)));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "unreadable-note"), "{:?}", outcome.report.refused);
    assert_eq!(vault.value("certificate", "c1")["chain"], json!([]));
}

#[test]
fn f11_a_copy_trashed_on_the_server_is_not_a_thing_trashed_here() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "key", "on");
    let mut vault = Vault::default();
    let key = keys::generate(KeyType::X25519).unwrap();
    let value = json!({"key_kind": "exchange", "type": "x25519", "key": B64.encode(key.as_bytes()), "exportable": true, "public": {}, "comment": null});
    vault.locals.insert(("key".into(), "k1".into()), Local { kind: "key".into(), id: "k1".into(), name: "K".into(), meta: Value::Null, approval: Approval::WhileOpen, value });
    vault.sync(&mut fake, &connection);
    let id = live_ids(&fake)[0].clone();
    let now = fake.now();
    bump(&mut fake, &id, |raw| raw["deletedDate"] = json!(now));
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.trashed_here, 0);
    assert!(!map::deleted(vault.of_kind("key")[0]));
}

// --- Found with Vaultwarden: the mark that the vault made a cipher outlives an edit of it ----------------------------------------------

#[test]
fn a_note_the_vault_made_is_still_known_as_made_after_an_edit_and_goes_to_the_trash_whole_when_its_kind_goes_sealed() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "key", "on");
    let mut vault = sealed_vault();
    let key = keys::generate(KeyType::X25519).unwrap();
    let value = json!({"key_kind": "exchange", "type": "x25519", "key": B64.encode(key.as_bytes()), "exportable": true, "public": {}, "comment": null});
    let local = Local { kind: "key".into(), id: "k1".into(), name: "K".into(), meta: Value::Null, approval: Approval::WhileOpen, value };
    vault.locals.insert(("key".into(), "k1".into()), local.clone());
    vault.sync(&mut fake, &connection);
    let id = live_ids(&fake)[0].clone();
    let mut renamed = local;
    renamed.name = "Renamed".into();
    vault.locals.insert(("key".into(), "k1".into()), renamed);
    vault.sync(&mut fake, &connection);
    assert!(vault.links.get(&id).unwrap().made, "the edit forgot that this client made the note");
    // The other app's field on it, so that it holds more than the vault wrote
    add_foreign_field(&mut fake, &id);
    on(&mut connection, "key", "sealed");
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none(), "{:?}", outcome.report.refused);
    assert!(!fake.find(&id).unwrap()["deletedDate"].is_null(), "a plain key stayed on the server");
    assert_eq!(live_ids(&fake).len(), 1, "and one sealed note stands");
}
