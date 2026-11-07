//! Each kind, plain and sealed, with files (plans/done/PLAN-vault.md, "Each kind is one switch"; plans/vault-sync/spec-attachments.md): the
//! choices and which wins, every mapping both ways, the edges of a field, a note and a file, and what a server that refuses, lies
//! or loses a file leads to. The server is `fake.rs`, which keeps every request.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use serde_json::{json, Value};

use super::address::Server;
use super::attach;
use super::cipher::{self, Content};
use super::convert::{self, Direct};
use super::crypto::{self, EncString, SymKey};
use super::fake::{Fake, BASE};
use super::kinds;
use super::map::{self, Local};
use super::merge::{self, Access, Connection, Input};
use super::plain;
use super::tests::{assert_nothing_leaked, backup_codes, bump, connect, password, totp_account, Vault, NOW_MS};
use crate::keys::{self, KeyType};
use crate::protocol::Approval;
use crate::seal::Key;

const USER_ID: &str = super::fake::USER_ID;

fn on(connection: &mut Connection, kind: &str, choice: &str) {
    connection.kinds.insert(kind.into(), choice.into());
}

fn key_local(id: &str, name: &str, key_kind: &str, key_type: KeyType) -> Local {
    let key = keys::generate(key_type).unwrap();
    let public = key.public().ok();
    let public_json = public.as_ref().map_or(Value::Null, |forms| json!({"type": key_type.name(), "pem": forms.spki_pem, "openssh": forms.openssh, "sha256": forms.sha256, "ssh_sha256": forms.ssh_sha256}));
    let value = json!({"key_kind": key_kind, "type": key_type.name(), "key": B64.encode(key.as_bytes()), "exportable": true, "public": public_json, "comment": null});
    let meta = json!({"key_kind": key_kind, "type": key_type.name(), "exportable": true});
    Local { kind: "key".into(), id: id.into(), name: name.into(), meta, approval: Approval::WhileOpen, value }
}

/// A self-signed certificate of an Ed25519 key, as the vault's own certificate item holds one
pub(super) fn cert_local(id: &str, name: &str, chain_copies: usize) -> Local {
    use openssl::asn1::Asn1Time;
    use openssl::bn::BigNum;
    use openssl::hash::MessageDigest;
    use openssl::x509::{X509Builder, X509NameBuilder};
    let key = keys::generate(KeyType::Ed25519).unwrap();
    let pkey = key.pkey().unwrap();
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
    let told = crate::pki::describe(&der).unwrap();
    let chain: Vec<String> = (0..chain_copies).map(|_| B64.encode(&der)).collect();
    let value = json!({"profile": "code", "type": "ed25519", "key": B64.encode(key.as_bytes()), "cert": B64.encode(&der), "chain": chain, "fields": {}});
    let meta = json!({"profile": "code", "subject": told.subject, "issuer": told.issuer, "not_before": told.not_before, "not_after": told.not_after, "sha256": told.sha256, "spki_sha256": told.key.sha256, "key_type": "ed25519"});
    Local { kind: "certificate".into(), id: id.into(), name: name.into(), meta, approval: Approval::WhileOpen, value }
}

fn file_local(id: &str, path: &str, bytes: &[u8]) -> Local {
    let value = json!({"what": "file", "files": [{"at": "", "mode": 0o600, "data": B64.encode(bytes)}], "folders": []});
    Local { kind: "protected".into(), id: id.into(), name: path.into(), meta: json!({"path": path, "what": "file", "size": bytes.len(), "files": 1}), approval: Approval::WhileOpen, value }
}

fn folder_local(id: &str, path: &str) -> Local {
    let value = json!({"what": "folder", "files": [{"at": "a/one.txt", "mode": 0o600, "data": B64.encode(b"one")}, {"at": "two.bin", "mode": 0o644, "data": B64.encode(vec![9u8; 6000])}, {"at": "n".repeat(150), "mode": 0o600, "data": B64.encode(b"long name")}], "folders": [{"at": "a", "mode": 0o700}]});
    Local { kind: "protected".into(), id: id.into(), name: path.into(), meta: json!({"path": path, "what": "folder", "files": 3}), approval: Approval::WhileOpen, value }
}

fn add(vault: &mut Vault, local: Local) {
    vault.locals.insert((local.kind.clone(), local.id.clone()), local);
}

fn opened(fake: &Fake, id: &str) -> cipher::Opened {
    cipher::open(fake.find(id).unwrap(), &fake.user).unwrap()
}

fn live_ids(fake: &Fake) -> Vec<String> {
    fake.ciphers.iter().filter(|raw| raw["deletedDate"].is_null()).map(|raw| raw["id"].as_str().unwrap().to_string()).collect()
}

/// The file a cipher holds, opened with the cipher's own key
fn file_of(fake: &Fake, id: &str) -> Vec<u8> {
    let raw = fake.find(id).unwrap();
    let list = cipher::attachments(raw);
    assert_eq!(list.len(), 1, "one file");
    let item = cipher::key_of(raw, &fake.user).unwrap();
    let record = cipher::attachment_of(raw, &list[0].id).unwrap();
    let stored = fake.files.get(&(id.to_string(), list[0].id.clone())).unwrap();
    assert_eq!(stored.len() as u64, list[0].size);
    attach::open(&item, record, stored).unwrap().to_vec()
}

fn name_of_file(fake: &Fake, id: &str) -> String {
    let raw = fake.find(id).unwrap();
    let record = cipher::attachment_of(raw, &cipher::attachments(raw)[0].id).unwrap();
    crypto::open_text(record["fileName"].as_str().unwrap(), &cipher::key_of(raw, &fake.user).unwrap()).unwrap().to_string()
}

fn sealed_vault() -> Vault {
    Vault { sealing: Some((Key::random(), "vault-1".into())), ..Vault::default() }
}

fn copy_of(vault: &Vault) -> Vault {
    Vault { sealing: vault.sealing.as_ref().map(|(k, v)| (k.copy(), v.clone())), ..Vault::default() }
}

#[test]
fn a_kind_has_one_choice_and_an_items_own_wins() {
    let mut connection = connect(&mut Fake::default()).unwrap();
    for (kind, default) in [("password", "on"), ("authenticator", "on"), ("backup-codes", "on"), ("key", "off"), ("certificate", "off"), ("protected", "off")] {
        assert_eq!(connection.kind_choice(kind), default, "{kind}");
        assert_eq!(connection.choice(kind, "x"), default);
    }
    on(&mut connection, "key", "sealed");
    on(&mut connection, "password", "off");
    assert_eq!((connection.choice("key", "k1"), connection.choice("password", "p1")), ("sealed", "off"));
    // The item's own wins either way, and a word that is not one is no choice
    connection.choices.insert("password.p1".into(), "sealed".into());
    connection.choices.insert("key.k1".into(), "off".into());
    connection.choices.insert("key.k2".into(), "banana".into());
    assert_eq!((connection.choice("key", "k1"), connection.choice("password", "p1"), connection.choice("key", "k2")), ("off", "sealed", "sealed"));
    connection.kinds.insert("key".into(), "banana".into());
    assert_eq!(connection.kind_choice("key"), "off");
    // Of the words people type
    assert_eq!(kinds::of_word("keys"), Some("key"));
    assert_eq!(kinds::of_word("files"), Some("protected"));
    assert_eq!(kinds::of_word("codes"), Some("authenticator"));
    assert_eq!(kinds::of_word("passwords"), Some("password"));
    assert_eq!(kinds::of_word("bananas"), None);
}

#[test]
fn a_kind_set_to_on_takes_in_what_asks_every_time_and_what_is_added_later() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    let mut vault = Vault::default();
    vault.put("password", "p1", password("Ask me", "u", "asked-secret", ""));
    vault.locals.get_mut(&("password".to_string(), "p1".to_string())).unwrap().approval = Approval::Ask;
    vault.sync(&mut fake, &connection);
    assert!(fake.ciphers.is_empty(), "no word of the person's yet: an item that asks every time stays");
    // Said for the kind, which the approval says covers the items that ask every time
    on(&mut connection, "password", "on");
    vault.sync(&mut fake, &connection);
    assert_eq!(fake.ciphers.len(), 1);
    // An item added later follows its kind
    vault.put("password", "p2", password("Later", "u", "later-secret", ""));
    vault.sync(&mut fake, &connection);
    assert_eq!(fake.ciphers.len(), 2);
    // The item's own Off wins over its kind
    connection.choices.insert("password.p2".into(), "off".into());
    vault.sync(&mut fake, &connection);
    assert_eq!(live_ids(&fake).len(), 1);
}

#[test]
fn a_key_goes_as_an_ssh_item_and_comes_back_as_a_key() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "key", "on");
    let mut vault = Vault::default();
    let local = key_local("k1", "Work key", "ssh", KeyType::Ed25519);
    let secret_b64 = local.value["key"].as_str().unwrap().to_string();
    add(&mut vault, local.clone());
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.created_there == 1 && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    let id = fake.ciphers[0]["id"].as_str().unwrap().to_string();
    assert_eq!(fake.ciphers[0]["type"], json!(5));
    let there = opened(&fake, &id);
    let ssh = there.content.ssh.clone().unwrap();
    assert!(ssh.private.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----\n") && ssh.private.ends_with("-----END OPENSSH PRIVATE KEY-----\n"));
    assert_eq!(ssh.public, local.value["public"]["openssh"].as_str().unwrap());
    assert_eq!(ssh.fingerprint, local.value["public"]["ssh_sha256"].as_str().unwrap());
    assert_eq!(there.content.marker.as_deref(), Some("key"));
    // OpenSSH's own reader gets the same key out of it
    let read = keys::import_openssh(ssh.private.as_bytes(), b"").unwrap();
    assert_eq!(B64.encode(read.key.as_bytes()), secret_b64);
    assert_nothing_leaked(&fake, &[&secret_b64]);
    // A second run with nothing changed writes nothing
    let before = fake.seen.len();
    let again = vault.sync(&mut fake, &connection);
    assert!(again.problem.is_none() && again.report.sent == 0 && again.report.created_there == 0);
    assert!(fake.seen[before..].iter().all(|seen| seen.method == "GET"));
    // Another computer, with the vault's key and nothing else, takes it in as the same key
    let mut elsewhere = Vault::default();
    elsewhere.sync(&mut fake, &connection);
    assert_eq!(elsewhere.of_kind("key").len(), 1);
    assert_eq!(elsewhere.of_kind("key")[0].value["key"].as_str().unwrap(), secret_b64);
    // And one with Keys off does not take it
    let mut off = Vault::default();
    let off_connection = connect(&mut fake).unwrap();
    off.sync(&mut fake, &off_connection);
    assert!(off.locals.is_empty());
}

#[test]
fn a_key_made_in_another_app_comes_in_only_while_keys_is_on() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    // An SSH key item as Bitwarden's apps write it: no marker, nothing of ours
    let key = keys::generate(KeyType::EcdsaP256).unwrap();
    let forms = convert::ssh_forms("ecdsa-p256", key.as_bytes()).unwrap();
    let content = Content { kind: cipher::SSH, name: "From the app".into(), ssh: Some(cipher::Ssh { private: forms.private.to_string(), public: forms.public.clone(), fingerprint: forms.fingerprint.clone() }), ..Content::default() };
    let body = cipher::create_body(&content, USER_ID, &fake.user).unwrap();
    fake.store(body);
    let mut vault = Vault::default();
    let outcome = vault.sync(&mut fake, &connection);
    assert!(vault.locals.is_empty() && outcome.report.left_alone == 1);
    on(&mut connection, "key", "on");
    vault.sync(&mut fake, &connection);
    let got = vault.of_kind("key");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].name, "From the app");
    assert_eq!(got[0].value["type"], json!("ecdsa-p256"));
    assert_eq!(got[0].value["key"].as_str().unwrap(), B64.encode(key.as_bytes()));
    assert_eq!(got[0].approval, Approval::Ask);
    // It is the same key next time, not a second one
    vault.sync(&mut fake, &connection);
    assert_eq!(vault.of_kind("key").len(), 1);
}

#[test]
fn a_server_that_keeps_no_ssh_item_gets_the_key_as_a_note() {
    let mut fake = Fake { no_ssh: true, ..Fake::default() };
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "key", "on");
    let mut vault = Vault::default();
    add(&mut vault, key_local("k1", "Work key", "ssh", KeyType::Ed25519));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    // The empty SSH item went to the trash, and the key is a note with its private part hidden
    assert_eq!(fake.ciphers.len(), 2);
    let live = live_ids(&fake);
    assert_eq!(live.len(), 1);
    let content = opened(&fake, &live[0]).content;
    assert_eq!(content.kind, cipher::NOTE);
    let private = content.fields.iter().find(|field| field.name == "private key").unwrap();
    assert!(private.hidden && private.value.starts_with("-----BEGIN PRIVATE KEY-----\n"));
}

#[test]
fn any_other_key_is_a_note_with_its_private_part_in_a_hidden_field_and_back() {
    for (key_type, key_kind) in [(KeyType::X25519, "exchange"), (KeyType::Aes256, "symmetric"), (KeyType::EcdsaP384, "signing")] {
        let mut fake = Fake::default();
        let mut connection = connect(&mut fake).unwrap();
        on(&mut connection, "key", "on");
        let mut vault = Vault::default();
        let local = key_local("k1", "A key", key_kind, key_type);
        let wanted = local.value["key"].as_str().unwrap().to_string();
        add(&mut vault, local);
        vault.sync(&mut fake, &connection);
        let id = live_ids(&fake)[0].clone();
        let content = opened(&fake, &id).content;
        assert_eq!(content.kind, cipher::NOTE, "{key_type:?}");
        assert_eq!(content.fields.iter().filter(|field| field.hidden).count(), 1);
        assert!(content.fields.iter().any(|field| field.name == "kind" && field.value == "key" && !field.hidden));
        assert_nothing_leaked(&fake, &[&wanted]);
        let mut elsewhere = Vault::default();
        elsewhere.sync(&mut fake, &connection);
        assert_eq!(elsewhere.of_kind("key")[0].value["key"].as_str().unwrap(), wanted, "{key_type:?}");
    }
}

#[test]
fn a_key_that_cannot_be_exported_never_goes_and_what_is_not_a_key_is_not_read_as_one() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "key", "on");
    let mut vault = Vault::default();
    let mut locked = key_local("k9", "Locked", "ssh", KeyType::Ed25519);
    locked.meta["exportable"] = json!(false);
    add(&mut vault, locked);
    vault.sync(&mut fake, &connection);
    assert!(fake.ciphers.is_empty());
    // A PKCS#1 or SEC1 form is only ever read, and a key of a type the vault has no SSH form for is not an SSH item
    assert!(convert::key_from_text("-----BEGIN RSA PRIVATE KEY-----\nAAAA\n-----END RSA PRIVATE KEY-----\n").is_none());
    assert!(convert::key_from_text("-----BEGIN ENCRYPTED PRIVATE KEY-----\nAAAA\n-----END ENCRYPTED PRIVATE KEY-----\n").is_none());
    assert!(convert::ssh_forms("x25519", keys::generate(KeyType::X25519).unwrap().as_bytes()).is_none());
    assert!(convert::ssh_forms("aes-256", &[1u8; 32]).is_none());
}

#[test]
fn a_certificate_goes_as_a_note_with_its_chain_and_comes_back_only_if_it_is_the_same_certificate() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "certificate", "on");
    let mut vault = Vault::default();
    let local = cert_local("c1", "Signing", 2);
    let private = local.value["key"].as_str().unwrap().to_string();
    add(&mut vault, local.clone());
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    let id = live_ids(&fake)[0].clone();
    let content = opened(&fake, &id).content;
    let notes = content.notes.clone().unwrap();
    assert_eq!(notes.matches("-----BEGIN CERTIFICATE-----").count(), 3);
    assert!(content.fields.iter().any(|field| field.name == "expires" && field.value.ends_with('Z') && !field.hidden));
    assert!(content.fields.iter().any(|field| field.name == "private key" && field.hidden));
    assert!(cipher::attachments(fake.find(&id).unwrap()).is_empty());
    assert_nothing_leaked(&fake, &[&private]);
    // A person's app drops a certificate of the chain: it comes back, the leaf and key staying
    let der = B64.decode(local.value["cert"].as_str().unwrap()).unwrap();
    let shorter = keys::pem("CERTIFICATE", &der);
    let key_in_server = cipher::key_of(fake.find(&id).unwrap(), &fake.user).unwrap();
    bump(&mut fake, &id, |raw| raw["notes"] = json!(crypto::seal_text(&key_in_server, &shorter)));
    vault.sync(&mut fake, &connection);
    assert_eq!(vault.value("certificate", "c1")["chain"], json!([]));
    // A note that no longer reads as it did, or a leaf that is another certificate, changes nothing here and deletes nothing
    let other = cert_local("c2", "Other", 0);
    let other_pem = keys::pem("CERTIFICATE", &B64.decode(other.value["cert"].as_str().unwrap()).unwrap());
    bump(&mut fake, &id, |raw| raw["notes"] = json!(crypto::seal_text(&key_in_server, &other_pem)));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "unreadable-note"));
    assert_eq!(vault.value("certificate", "c1")["cert"], local.value["cert"]);
    bump(&mut fake, &id, |raw| raw["notes"] = json!(crypto::seal_text(&key_in_server, "just some words")));
    vault.sync(&mut fake, &connection);
    assert_eq!(vault.of_kind("certificate").len(), 1);
    assert!(!map::deleted(vault.of_kind("certificate")[0]));
}

#[test]
fn a_chain_that_does_not_fit_a_note_goes_with_its_key_as_a_pem_file() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "certificate", "on");
    let mut vault = Vault::default();
    // Forty certificates are more than a note's 7,439 bytes
    add(&mut vault, cert_local("c1", "Long chain", 40));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    let id = live_ids(&fake)[0].clone();
    let content = opened(&fake, &id).content;
    assert!(content.notes.is_none() && content.fields.iter().all(|field| !field.hidden));
    let file = String::from_utf8(file_of(&fake, &id)).unwrap();
    assert!(file.starts_with("-----BEGIN PRIVATE KEY-----\n") && file.matches("-----BEGIN CERTIFICATE-----").count() == 41);
    assert_eq!(name_of_file(&fake, &id), "Long chain.pem");
    // The server only ever got the attachment encrypted
    assert!(fake.files.values().all(|bytes| bytes[0] == 2 && !String::from_utf8_lossy(bytes).contains("BEGIN")));
}

#[test]
fn a_protected_file_is_a_note_named_after_it_with_the_file_attached_and_a_change_is_a_new_cipher() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = Vault::default();
    add(&mut vault, file_local("f1", ".npmrc", b"//registry.example/:_authToken=NPM-SECRET\n"));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    let first = live_ids(&fake)[0].clone();
    let content = opened(&fake, &first).content;
    assert_eq!((content.name.as_str(), content.marker.as_deref(), content.notes.clone()), (".npmrc", Some("file"), None));
    assert_eq!(file_of(&fake, &first), b"//registry.example/:_authToken=NPM-SECRET\n");
    assert_eq!(name_of_file(&fake, &first), ".npmrc");
    assert_nothing_leaked(&fake, &["NPM-SECRET"]);
    // Nothing moved: nothing is sent
    let before = fake.seen.len();
    vault.sync(&mut fake, &connection);
    assert!(fake.seen[before..].iter().all(|seen| seen.method == "GET"));
    // Changed: a new cipher with its own file, and the old one to the trash, whole; nothing destroyed
    add(&mut vault, file_local("f1", ".npmrc", b"//registry.example/:_authToken=NEW-NPM-SECRET\n"));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.created_there == 1 && outcome.report.trashed_there == 1, "{:?}", outcome.report.refused);
    let live = live_ids(&fake);
    assert_eq!(live.len(), 1);
    assert_ne!(live[0], first);
    assert_eq!(file_of(&fake, &live[0]), b"//registry.example/:_authToken=NEW-NPM-SECRET\n");
    assert!(!fake.find(&first).unwrap()["deletedDate"].is_null());
    assert_eq!(cipher::attachments(fake.find(&first).unwrap()).len(), 1, "the old file is still there, in the trash");
    assert!(fake.dropped.is_empty());
    assert_eq!(vault.links.len(), 1);
    assert_nothing_leaked(&fake, &["NPM-SECRET"]);
    // A name changed in another app comes back for nothing: the path is the vault's
    let item = cipher::key_of(fake.find(&live[0]).unwrap(), &fake.user).unwrap();
    bump(&mut fake, &live[0], |raw| raw["name"] = json!(crypto::seal_text(&item, "renamed there")));
    vault.sync(&mut fake, &connection);
    assert_eq!(vault.of_kind("protected")[0].name, ".npmrc");
}

#[test]
fn a_folder_goes_as_a_tar_sealed_or_plain_and_comes_back_as_the_same_folder() {
    // Plain: a tar archive of the folder, with a name longer than ustar's
    let tar = plain::tar_of(&folder_local("d1", "x").value).unwrap();
    let back = plain::tree_of_tar(&tar).unwrap();
    assert_eq!(back["folders"], json!([{"at": "a", "mode": 0o700}]));
    assert_eq!(back["files"].as_array().unwrap().len(), 3);
    assert_eq!(back["files"].as_array().unwrap().iter().filter(|file| file["at"].as_str().unwrap().len() == 150).count(), 1);
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = Vault::default();
    add(&mut vault, folder_local("d1", ".config/tool"));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    assert_eq!(name_of_file(&fake, &live_ids(&fake)[0]), "tool.tar");
    assert_eq!(file_of(&fake, &live_ids(&fake)[0]), tar.to_vec());
    // Sealed: the file is the vault's own blob, whatever it holds, and its name says nothing
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "sealed");
    let mut vault = sealed_vault();
    add(&mut vault, folder_local("d1", ".config/tool"));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    let id = live_ids(&fake)[0].clone();
    let content = opened(&fake, &id).content;
    assert_eq!((content.name.as_str(), content.marker.as_deref()), (map::SEALED_NAME, Some("sealed")));
    assert_eq!(content.notes.as_deref(), Some(map::SEALED_ATTACHED));
    let blob = file_of(&fake, &id);
    assert!(!String::from_utf8_lossy(&blob).contains("one.txt"));
    assert_eq!(name_of_file(&fake, &id), "amethystora-sealed.bin");
    let mut elsewhere = copy_of(&vault);
    elsewhere.sync(&mut fake, &connection);
    assert_eq!(elsewhere.value("protected", "d1"), vault.value("protected", "d1"));
}

#[test]
fn a_sealed_item_larger_than_a_note_is_a_file_and_the_counter_stays_inside_the_seal() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "certificate", "sealed");
    let mut vault = sealed_vault();
    let mut big = cert_local("c1", "Big", 0);
    big.value["fields"] = json!({"pad": "x".repeat(6000)});
    add(&mut vault, big.clone());
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    let id = live_ids(&fake)[0].clone();
    assert_eq!(opened(&fake, &id).content.notes.as_deref(), Some(map::SEALED_ATTACHED));
    assert_eq!(vault.links.values().next().unwrap().seal, 1);
    // Changed: a new cipher, the counter one higher, the old one in the trash
    big.value["fields"] = json!({"pad": "y".repeat(6000)});
    add(&mut vault, big);
    vault.sync(&mut fake, &connection);
    assert_eq!(vault.links.values().next().unwrap().seal, 2);
    assert_eq!(live_ids(&fake).len(), 1);
    let mut elsewhere = copy_of(&vault);
    elsewhere.sync(&mut fake, &connection);
    assert_eq!(elsewhere.value("certificate", "c1")["fields"]["pad"], json!("y".repeat(6000)));
    // Another vault sees a sealed note it cannot open
    let mut other = Vault { sealing: Some((Key::random(), "vault-2".into())), ..Vault::default() };
    let outcome = other.sync(&mut fake, &connection);
    assert_eq!(outcome.report.sealed_elsewhere, 1);
}

#[test]
fn what_fits_a_field_a_note_and_what_needs_a_file_at_the_edges() {
    // A private key in a hidden field: 3,695 bytes of its text at most
    let fits = |bytes: usize| {
        let local = Local { kind: "key".into(), id: "k".into(), name: "n".into(), meta: Value::Null, approval: Approval::WhileOpen, value: json!({"key_kind": "exchange", "type": "x25519", "key": B64.encode(vec![5u8; bytes]), "public": {}}) };
        let made = plain::key_made(&local, &Direct, true, false).unwrap();
        made.file.is_none() && made.content.check_lengths().is_ok()
    };
    let text_of = |bytes: usize| keys::pem("PRIVATE KEY", &vec![5u8; bytes]).len();
    let largest = (2000..3000).rev().find(|bytes| fits(*bytes)).unwrap();
    assert!(largest > 2400 && !fits(largest + 1), "the edge is {largest}");
    assert!(text_of(largest) <= 3695 && text_of(largest + 1) > 3695);
    assert_eq!(super::lengths::encstring(3695), 5000);
    assert!(super::lengths::encstring(3696) > 5000);
    assert_eq!(super::lengths::encstring(7439), 9992);
    assert!(super::lengths::encstring(7440) > 10_000);
}

#[test]
fn a_kind_switched_off_moves_its_copies_to_the_trash_and_on_again_makes_them_anew() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    let mut vault = sealed_vault();
    vault.put("password", "p1", password("Site", "u", "pw-secret", "https://a.test"));
    vault.sync(&mut fake, &connection);
    assert_eq!(live_ids(&fake).len(), 1);
    on(&mut connection, "password", "off");
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.report.trashed_there, 1, "{:?} {:?}", outcome.problem, outcome.report.refused);
    assert!(live_ids(&fake).is_empty());
    assert!(vault.links.is_empty(), "{:?} {:?}", vault.links.values().map(|l| (l.cipher.clone(), l.deleted, l.parts.len())).collect::<Vec<_>>(), outcome.groups.len());
    // The vault's item is still there, and nothing was destroyed
    assert_eq!(vault.of_kind("password").len(), 1);
    assert_eq!(fake.ciphers.len(), 1);
    on(&mut connection, "password", "sealed");
    vault.sync(&mut fake, &connection);
    assert_eq!(live_ids(&fake).len(), 1);
    let id = live_ids(&fake)[0].clone();
    assert_eq!(opened(&fake, &id).content.name, map::SEALED_NAME);
    // Plain and sealed are two forms of one item: switching moves it, and a sealed password opens in this vault again
    on(&mut connection, "password", "on");
    vault.sync(&mut fake, &connection);
    assert_eq!(live_ids(&fake).len(), 1);
    assert_eq!(opened(&fake, &live_ids(&fake)[0]).content.password.as_deref(), Some("pw-secret"));
    on(&mut connection, "password", "sealed");
    vault.sync(&mut fake, &connection);
    let mut elsewhere = copy_of(&vault);
    elsewhere.sync(&mut fake, &connection);
    assert_eq!(elsewhere.value("password", "p1")["password"], json!("pw-secret"));
}

#[test]
fn a_put_never_names_attachments_and_a_file_not_made_here_is_never_touched() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    on(&mut connection, "key", "on");
    let mut vault = Vault::default();
    add(&mut vault, file_local("f1", ".npmrc", b"secret-file"));
    let mut key = key_local("k1", "A key", "exchange", KeyType::X25519);
    add(&mut vault, key.clone());
    vault.sync(&mut fake, &connection);
    // A foreign file on a note of ours, added by an app, survives every PUT and is never fetched
    let key_cipher = fake.ciphers.iter().find(|raw| cipher::open(raw, &fake.user).is_ok_and(|o| o.content.marker.as_deref() == Some("key"))).unwrap()["id"].as_str().unwrap().to_string();
    let foreign = fake.attach(&key_cipher, "from-an-app.txt", b"hello from another app");
    key.name = "Renamed key".into();
    add(&mut vault, key);
    let before = fake.seen.len();
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none(), "{:?}", outcome.report.refused);
    let mut puts = 0;
    for seen in &fake.seen[before..] {
        if seen.method == "PUT" {
            puts += 1;
            assert!(!seen.body.contains("attachments"), "{}", seen.body);
        }
        assert!(!seen.url.contains(&foreign) || seen.method != "GET", "a file not made here was fetched");
    }
    assert_eq!(puts, 1);
    assert_eq!(opened(&fake, &key_cipher).content.name, "Renamed key");
    assert!(cipher::attachments(fake.find(&key_cipher).unwrap()).iter().any(|att| att.id == foreign));
    assert!(!fake.files.get(&(key_cipher, foreign)).unwrap().is_empty());
}

#[test]
fn a_file_the_server_lost_is_told_and_nothing_is_deleted_here() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = Vault::default();
    add(&mut vault, file_local("f1", ".npmrc", b"secret-file"));
    vault.sync(&mut fake, &connection);
    let id = live_ids(&fake)[0].clone();
    // Another client removes the attachment, and Vaultwarden does not move the cipher's date for it
    let date = fake.find(&id).unwrap()["revisionDate"].clone();
    fake.find_mut(&id).unwrap()["attachments"] = json!([]);
    assert_eq!(fake.find(&id).unwrap()["revisionDate"], date);
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "attachment-lost"), "{:?}", outcome.report.refused);
    assert_eq!(vault.of_kind("protected").len(), 1);
    assert!(!map::deleted(vault.of_kind("protected")[0]));
    // Changed here, it goes as a cipher of its own with its file, and the broken one to the trash
    add(&mut vault, file_local("f1", ".npmrc", b"secret-file-2"));
    vault.sync(&mut fake, &connection);
    assert_eq!(live_ids(&fake).len(), 1);
    assert_ne!(live_ids(&fake)[0], id);
    assert_eq!(file_of(&fake, &live_ids(&fake)[0]), b"secret-file-2");
}

#[test]
fn files_are_refused_with_a_reason_before_a_request_where_they_cannot_go() {
    // Bitwarden's own servers: before any request about a file, and the item stays here
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    connection.address = "https://vault.bitwarden.com".into();
    let mut vault = Vault::default();
    add(&mut vault, file_local("f1", ".npmrc", b"secret-file"));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none());
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "attachments-cloud"), "{:?}", outcome.report.refused);
    assert!(fake.ciphers.is_empty() && fake.seen.iter().all(|seen| !seen.url.contains("attachment")));
    assert_eq!(vault.of_kind("protected").len(), 1);
    // An account without premium
    let mut fake = Fake { premium: false, ..Fake::default() };
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = sealed_vault();
    add(&mut vault, file_local("f1", ".npmrc", b"secret-file"));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "attachments-premium"));
    assert!(fake.ciphers.is_empty());
    // What fits a note still goes there: a sealed password, for one
    on(&mut connection, "password", "sealed");
    vault.put("password", "p1", password("Site", "u", "pw", ""));
    vault.sync(&mut fake, &connection);
    assert_eq!(fake.ciphers.len(), 1);
}

fn retry(vault: &Vault, fake: &Fake, connection: &Connection) -> Input {
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
        retry_files: true,
    }
}

#[test]
fn a_server_that_says_no_is_asked_once_and_the_cipher_it_left_is_in_the_trash() {
    let mut fake = Fake { slot_status: Some(400), ..Fake::default() };
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = Vault::default();
    add(&mut vault, file_local("f1", ".npmrc", b"secret-file"));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "attachments-refused" && r["item"] == "protected.f1"), "{:?}", outcome.report.refused);
    // One refusal may be that file's: the server is not written off for it
    assert_eq!(outcome.no_files, None);
    // The note that was made for it is in the trash, not left half-made
    assert_eq!(fake.ciphers.len(), 1);
    assert!(live_ids(&fake).is_empty());
    assert!(vault.links.is_empty() && vault.of_kind("protected").len() == 1);
    // Two items refused is the server
    add(&mut vault, file_local("f2", ".netrc", b"other-secret"));
    let outcome = vault.sync(&mut fake, &connection);
    assert_eq!(outcome.no_files.as_deref(), Some("attachments-refused"));
    assert!(live_ids(&fake).is_empty());
    // The connection keeps the reason, and the next run does not ask again; a person's run does
    connection.no_files = outcome.no_files.clone();
    let before = fake.seen.len();
    let again = vault.sync(&mut fake, &connection);
    assert!(again.report.refused.iter().any(|r| r["why"] == "attachments-refused"));
    assert!(fake.seen[before..].iter().all(|seen| !seen.url.contains("attachment") && seen.method == "GET"));
    let ciphers_before = fake.ciphers.len();
    fake.slot_status = None;
    let input = retry(&vault, &fake, &connection);
    let mut outcome = merge::run(&mut fake, &Direct, input);
    vault.apply(&mut outcome);
    assert!(outcome.no_files.is_none() && outcome.report.created_there == 2, "{:?}", outcome.report.refused);
    assert_eq!(live_ids(&fake).len(), 2);
    assert_eq!(fake.ciphers.len(), ciphers_before + 2);
}

#[test]
fn a_server_that_keeps_files_elsewhere_gets_no_file_and_loses_its_slot() {
    let mut fake = Fake { upload_type: 1, ..Fake::default() };
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = Vault::default();
    add(&mut vault, file_local("f1", ".npmrc", b"secret-file"));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "attachments-elsewhere"));
    // No upload went anywhere, the slot was dropped once (the one delete there is), and the cipher is in the trash
    assert!(fake.seen.iter().all(|seen| !(seen.method == "POST" && seen.url.contains("/attachment/") && !seen.url.ends_with("/v2"))));
    assert_eq!(fake.dropped.len(), 1);
    assert!(live_ids(&fake).is_empty());
    // A download address at another host, or with no token, is never fetched
    for url in ["https://bucket.s3.example.test/x?X-Amz-Signature=1", "https://blob.core.windows.net/x?sv=1&sig=2"] {
        let mut fake = Fake { download_url: Some(url.into()), ..Fake::default() };
        let mut connection = connect(&mut fake).unwrap();
        on(&mut connection, "protected", "on");
        let mut vault = Vault::default();
        add(&mut vault, file_local("f1", ".npmrc", b"secret-file"));
        let outcome = vault.sync(&mut fake, &connection);
        assert!(outcome.report.refused.iter().any(|r| r["why"] == "attachments-elsewhere"), "{url}");
        assert!(fake.seen.iter().all(|seen| !seen.url.contains("bucket") && !seen.url.contains("blob.core")));
        assert!(live_ids(&fake).is_empty());
        assert_eq!(outcome.no_files.as_deref(), Some("attachments-elsewhere"));
    }
}

#[test]
fn a_wrong_domain_costs_nothing_and_no_credential_goes_with_a_download() {
    let mut fake = Fake { download_host: Some("https://wrong.test:1".into()), ..Fake::default() };
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = Vault::default();
    add(&mut vault, file_local("f1", ".npmrc", b"secret-file"));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    assert_eq!(live_ids(&fake).len(), 1);
    assert!(outcome.files_read);
    let mut downloads = 0;
    for seen in &fake.seen {
        assert!(seen.url.starts_with(BASE), "{}", seen.url);
        if seen.url.contains("/attachments/") {
            downloads += 1;
            assert!(!seen.headers.iter().any(|(name, _)| name.eq_ignore_ascii_case("authorization")), "a download carried the access token");
        }
    }
    assert_eq!(downloads, 1);
}

#[test]
fn an_upload_that_keeps_nothing_is_found_out_by_reading_the_first_file_back() {
    let mut fake = Fake { swallow_uploads: true, ..Fake::default() };
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = Vault::default();
    add(&mut vault, file_local("f1", ".npmrc", b"secret-file"));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "attachment-lost"), "{:?}", outcome.report.refused);
    assert!(live_ids(&fake).is_empty() && vault.links.is_empty());
    assert!(!outcome.files_read);
}

#[test]
fn a_foreign_file_a_hostile_one_and_a_swapped_one_open_as_nothing() {
    let item = SymKey::random();
    let other = SymKey::random();
    // A record with the key of another cipher, a type 0 key, no key, a cut file and a file under the item's key: all refused
    let good = attach::prepare(&item, "a.txt", b"hello").unwrap();
    let record = json!({"key": good.key});
    assert!(attach::open(&item, &record, &good.bytes).is_ok());
    assert!(attach::open(&other, &record, &good.bytes).is_err());
    assert!(attach::open(&item, &json!({"key": "0.AAAA|BBBB"}), &good.bytes).is_err());
    assert!(attach::open(&item, &json!({}), &good.bytes).is_err());
    assert!(attach::open(&item, &record, &good.bytes[..good.bytes.len() - 16]).is_err());
    let swapped = EncString::seal(&item, b"under the cipher's key, not the file's").binary();
    assert!(attach::open(&item, &record, &swapped).is_err());
    // Past the cap of our own, and the cloud's hosts, by the address alone
    assert!(attach::prepare(&item, "big", &vec![0u8; attach::MOST_FILE + 1]).is_err());
    assert!(Server::from_address("https://vault.bitwarden.com").unwrap().is_cloud() && Server::from_address("bitwarden.eu").unwrap().is_cloud());
    assert!(!Server::from_address("https://vault.example.test").unwrap().is_cloud());
}

#[test]
fn a_hidden_field_that_lost_its_line_breaks_is_the_same_key_and_costs_no_write() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "key", "on");
    let mut vault = Vault::default();
    add(&mut vault, key_local("k1", "Signing key", "signing", KeyType::EcdsaP256));
    vault.sync(&mut fake, &connection);
    let id = live_ids(&fake)[0].clone();
    // An app's single-line input took the line breaks out and saved it so
    let content = opened(&fake, &id).content;
    let joined: String = content.fields.iter().find(|field| field.hidden).unwrap().value.lines().collect::<Vec<_>>().join("");
    let mut fields = content.fields.clone();
    fields.iter_mut().filter(|field| field.hidden).for_each(|field| field.value = joined.clone());
    let body = cipher::update_body(fake.find(&id).unwrap(), &content, &Content { fields, ..content.clone() }, USER_ID, &fake.user).unwrap();
    bump(&mut fake, &id, |raw| raw["fields"] = body["fields"].clone());
    let before = fake.seen.len();
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    assert!(fake.seen[before..].iter().all(|seen| seen.method == "GET"), "nothing written");
    assert_eq!(vault.of_kind("key").len(), 1);
}

#[test]
fn a_key_changed_in_another_app_comes_back_and_one_that_is_no_longer_a_key_is_a_conflict_never_a_deletion() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "key", "on");
    let mut vault = Vault::default();
    add(&mut vault, key_local("k1", "Signing key", "signing", KeyType::EcdsaP256));
    vault.sync(&mut fake, &connection);
    let id = live_ids(&fake)[0].clone();
    let content = opened(&fake, &id).content;
    let set_hidden = |fake: &mut Fake, text: &str| {
        let mut fields = content.fields.clone();
        fields.iter_mut().filter(|field| field.hidden).for_each(|field| field.value = text.to_string());
        let body = cipher::update_body(fake.find(&id).unwrap(), &content, &Content { fields, ..content.clone() }, USER_ID, &fake.user).unwrap();
        bump(fake, &id, |raw| raw["fields"] = body["fields"].clone());
    };
    let held = vault.value("key", "k1");
    // Another key, even of the same type: it never replaces the vault's own. It is the server's version, kept beside it as a key of its own.
    let replacement = keys::generate(KeyType::EcdsaP256).unwrap();
    set_hidden(&mut fake, std::str::from_utf8(&replacement.export_plain().unwrap()).unwrap());
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.conflicts == 1, "{:?}", outcome.report.refused);
    assert_eq!(vault.value("key", "k1"), held, "the vault's key is the one it signs with still");
    let keys_now = vault.of_kind("key");
    assert_eq!(keys_now.len(), 2);
    let beside = keys_now.iter().find(|local| local.id != "k1").unwrap();
    assert_eq!(beside.value["key"].as_str().unwrap(), B64.encode(replacement.as_bytes()));
    assert!(beside.meta["syncConflict"] == json!("server") && beside.approval == Approval::Ask);
    // Another type does the same; words are no key at all, and said
    set_hidden(&mut fake, std::str::from_utf8(&keys::generate(KeyType::Ed25519).unwrap().export_plain().unwrap()).unwrap());
    vault.sync(&mut fake, &connection);
    assert_eq!(vault.value("key", "k1"), held);
    assert_eq!(vault.of_kind("key").len(), 3);
    set_hidden(&mut fake, "not a key at all");
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "unreadable-note"));
    assert_eq!(vault.value("key", "k1"), held);
    assert_eq!(vault.of_kind("key").len(), 3);
    assert!(vault.of_kind("key").iter().all(|local| !map::deleted(local)));
}

#[test]
fn what_the_vault_holds_goes_on_both_ways_with_every_kind_at_once() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    for kind in ["key", "certificate", "protected"] {
        on(&mut connection, kind, "on");
    }
    let mut vault = Vault::default();
    vault.put("password", "p1", password("Site", "u", "pw-1", "https://a.test"));
    vault.put("authenticator", "a1", totp_account("Other", "me"));
    vault.put("backup-codes", "b1", backup_codes("Site"));
    add(&mut vault, key_local("k1", "Work key", "ssh", KeyType::Rsa3072));
    add(&mut vault, cert_local("c1", "Signing", 1));
    add(&mut vault, file_local("f1", "notes.txt", b"words"));
    add(&mut vault, folder_local("d1", "folder"));
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    assert_eq!(live_ids(&fake).len(), 7);
    let mut elsewhere = Vault::default();
    elsewhere.sync(&mut fake, &connection);
    assert_eq!(elsewhere.of_kind("key").len(), 1);
    assert!(!elsewhere.of_kind("password").is_empty());
    // Files and certificates made elsewhere are not taken in: nothing says they are the person's
    assert!(elsewhere.of_kind("protected").is_empty() && elsewhere.of_kind("certificate").is_empty());
}

#[test]
fn a_file_replaced_in_another_app_is_taken_if_it_checks_out_and_said_if_not() {
    let mut fake = Fake::default();
    let mut connection = connect(&mut fake).unwrap();
    on(&mut connection, "protected", "on");
    let mut vault = Vault::default();
    add(&mut vault, file_local("f1", ".npmrc", b"old contents"));
    add(&mut vault, folder_local("d1", ".config/tool"));
    vault.sync(&mut fake, &connection);
    let ids = live_ids(&fake);
    let (file_id, folder_id) = {
        let name = |id: &String| opened(&fake, id).content.name;
        (ids.iter().find(|id| name(id) == ".npmrc").unwrap().clone(), ids.iter().find(|id| name(id) == ".config/tool").unwrap().clone())
    };
    // Another app deletes the file and attaches another: the vault takes it, with the mode it had, and the next run has nothing to say
    fake.find_mut(&file_id).unwrap()["attachments"] = json!([]);
    fake.attach(&file_id, ".npmrc", b"new contents from the app");
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.problem.is_none() && outcome.report.refused_total == 0, "{:?}", outcome.report.refused);
    let value = vault.value("protected", "f1");
    assert_eq!(value["files"][0]["data"], json!(B64.encode(b"new contents from the app")));
    assert_eq!(value["files"][0]["mode"], json!(0o600));
    let before = fake.seen.len();
    let again = vault.sync(&mut fake, &connection);
    assert!(again.problem.is_none() && again.groups.is_empty() && again.report.created_there == 0, "{:?}", again.report.refused);
    assert!(fake.seen[before..].iter().all(|seen| seen.method == "GET"));
    // A folder comes back from a tar; one that leaves its place, or is not one, is said and changes nothing
    let held = vault.value("protected", "d1");
    let mut tree = held.clone();
    tree["files"].as_array_mut().unwrap().push(json!({"at": "added.txt", "mode": 0o600, "data": B64.encode(b"added")}));
    fake.find_mut(&folder_id).unwrap()["attachments"] = json!([]);
    fake.attach(&folder_id, "tool.tar", &plain::tar_of(&tree).unwrap());
    vault.sync(&mut fake, &connection);
    assert_eq!(vault.value("protected", "d1")["files"].as_array().unwrap().len(), 4);
    let held = vault.value("protected", "d1");
    for hostile in [b"not a tar at all, only words".to_vec(), {
        let mut out = Vec::new();
        out.extend_from_slice(&[0u8; 512]);
        out
    }] {
        fake.find_mut(&folder_id).unwrap()["attachments"] = json!([]);
        fake.attach(&folder_id, "tool.tar", &hostile);
        let outcome = vault.sync(&mut fake, &connection);
        assert!(outcome.report.refused.iter().any(|r| r["why"] == "unreadable-note"), "{:?}", outcome.report.refused);
        assert_eq!(vault.value("protected", "d1"), held);
    }
    // Two new files at once is not one replaced: said, and nothing taken
    fake.find_mut(&file_id).unwrap()["attachments"] = json!([]);
    fake.attach(&file_id, "a", b"one");
    fake.attach(&file_id, "b", b"two");
    let outcome = vault.sync(&mut fake, &connection);
    assert!(outcome.report.refused.iter().any(|r| r["why"] == "attachment-lost"));
    assert_eq!(vault.value("protected", "f1")["files"][0]["data"], json!(B64.encode(b"new contents from the app")));
    assert!(vault.of_kind("protected").iter().all(|local| !map::deleted(local)));
}

#[test]
fn ssh_keygen_reads_the_private_text_this_writes_and_says_the_same_public_key_and_fingerprint() {
    // OpenSSH's own program, where it is (the vault's container has it): the text is what `ssh-keygen` takes for a key file
    if std::process::Command::new("ssh-keygen").arg("-?").stderr(std::process::Stdio::null()).stdout(std::process::Stdio::null()).status().is_err() {
        eprintln!("no ssh-keygen here: the key forms are checked against the reader of this crate only");
        return;
    }
    let dir = std::env::temp_dir().join(format!("amethystora-kinds-ssh-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for key_type in [KeyType::Ed25519, KeyType::EcdsaP256, KeyType::EcdsaP384, KeyType::Rsa3072] {
        let key = keys::generate(key_type).unwrap();
        let forms = convert::ssh_forms(key_type.name(), key.as_bytes()).unwrap();
        let file = dir.join(format!("{}.key", key_type.name()));
        std::fs::write(&file, forms.private.as_bytes()).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
        let said = std::process::Command::new("ssh-keygen").args(["-y", "-f"]).arg(&file).output().unwrap();
        assert!(said.status.success(), "{key_type:?}: {}", String::from_utf8_lossy(&said.stderr));
        assert_eq!(String::from_utf8_lossy(&said.stdout).trim(), forms.public, "{key_type:?}");
        let listed = std::process::Command::new("ssh-keygen").args(["-l", "-f"]).arg(&file).output().unwrap();
        assert!(String::from_utf8_lossy(&listed.stdout).contains(&forms.fingerprint), "{key_type:?}: {}", String::from_utf8_lossy(&listed.stdout));
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_download_is_bound_by_the_size_the_server_listed_and_what_it_names_is_never_followed() {
    use super::api::Api;
    let mut fake = Fake::default();
    let _ = connect(&mut fake).unwrap();
    let content = Content { kind: cipher::NOTE, name: "n".into(), ..Content::default() };
    let id = fake.store(cipher::create_body(&content, USER_ID, &fake.user).unwrap());
    let att = fake.attach(&id, "a.txt", b"small");
    let server = Server::plain_for_tests(BASE);
    let token = fake.access();
    // The size the server listed is a hint, and what comes is checked against it and the cap, never trusted for either
    let listed = cipher::attachments(fake.find(&id).unwrap())[0].size;
    fake.files.insert((id.clone(), att.clone()), vec![7u8; listed as usize + 2 * 1024 * 1024]);
    let answer = Api { http: &mut fake, server: &server, access: &token }.download(&id, &att, listed).unwrap();
    assert_eq!(answer.err(), Some(attach::Fail::Bad));
    fake.files.insert((id.clone(), att.clone()), vec![7u8; listed as usize + 1024]);
    assert!(Api { http: &mut fake, server: &server, access: &token }.download(&id, &att, listed).unwrap().is_ok());
    // An id that is not one, or a cipher that is not a UUID, never reaches a path
    assert!(Api { http: &mut fake, server: &server, access: &token }.download("../x", &att, 1).is_err());
    assert!(Api { http: &mut fake, server: &server, access: &token }.download(&id, "a/b", 1).is_err());
    assert!(Api { http: &mut fake, server: &server, access: &token }.drop_slot(&id, "../../x").is_err());
    // The address a server names for the bytes is read for its token and nothing else
    fake.download_host = Some("https://evil.example.test".into());
    fake.files.insert((id.clone(), att.clone()), vec![1u8; 8]);
    assert!(Api { http: &mut fake, server: &server, access: &token }.download(&id, &att, 8).unwrap().is_ok());
    assert!(fake.seen.iter().all(|seen| !seen.url.contains("evil.example.test")));
}
