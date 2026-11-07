//! The service on a real socket, with stand-ins for polkit, logind and the TPM. Everything a client can do to it, the contract
//! `apps/vault-client` fixes first.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use amethystora_vault_client::{forget_at, open_at, seal_at, Refusal, MAX_DATA, MAX_LINE};
use amethystora_vault_service::outside::{Action, Event};
use common::{b64, files_under, start, start_with, why, Running};
use serde_json::{json, Value};

const PASSWORD: &str = "correct horse battery";

fn opened(running: &Running) -> Vec<Value> {
    running.ask(json!({"op": "status"}))["vaults"].as_array().unwrap().iter().filter(|vault| vault["open"] == json!(true)).cloned().collect()
}

// --- The contract ----------------------------------------------------------------------------------------------------------

#[test]
fn the_client_seals_and_opens_through_every_state() {
    let running = start("contract");
    let socket = running.socket.clone();
    // Never set up
    assert_eq!(seal_at(&socket, "resume", "2026", b"x"), Err(Refusal::NotSetUp));
    assert_eq!(open_at(&socket, "resume", "2026", b"x"), Err(Refusal::NotSetUp));
    assert_eq!(forget_at(&socket, "resume"), Err(Refusal::NotSetUp));
    running.set_up(PASSWORD);
    // Open
    let sealed = seal_at(&socket, "resume", "2026", b"plain").unwrap();
    assert_eq!(&sealed[..4], b"AVs1");
    assert_eq!(sealed.len(), 32 + 5);
    assert_eq!(open_at(&socket, "resume", "2026", &sealed).unwrap(), b"plain");
    // Another record, purpose or a changed byte is one refusal
    assert_eq!(open_at(&socket, "resume", "2027", &sealed), Err(Refusal::Refused));
    assert_eq!(open_at(&socket, "digest", "2026", &sealed), Err(Refusal::Refused));
    let mut changed = sealed.clone();
    changed[20] ^= 1;
    assert_eq!(open_at(&socket, "resume", "2026", &changed), Err(Refusal::Refused));
    // A purpose's key is its own
    let other = seal_at(&socket, "digest", "2026", b"plain").unwrap();
    assert_ne!(sealed[32..], other[32..]);
    // Closed
    assert_eq!(running.ask(json!({"op": "lock"}))["ok"], json!(true));
    assert_eq!(seal_at(&socket, "resume", "2026", b"x"), Err(Refusal::Closed));
    assert_eq!(open_at(&socket, "resume", "2026", &sealed), Err(Refusal::Closed));
    assert_eq!(forget_at(&socket, "resume"), Err(Refusal::Closed));
    // Open again: what was sealed still opens
    assert_eq!(running.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))["ok"], json!(true));
    assert_eq!(open_at(&socket, "resume", "2026", &sealed).unwrap(), b"plain");
    // Forgotten, it is gone for good, and a new key is not the old one
    forget_at(&socket, "resume").unwrap();
    assert_eq!(open_at(&socket, "resume", "2026", &sealed), Err(Refusal::Refused));
    seal_at(&socket, "resume", "2026", b"new").unwrap();
    assert_eq!(open_at(&socket, "resume", "2026", &sealed), Err(Refusal::Refused));
    // Nobody was asked for any of it after setup, but for forgetting: that loses every file under the key, so a person says so
    assert_eq!(running.outside.asked(), 2);
    assert_eq!(running.outside.said.lock().unwrap().asked[1].item, "resume");
}

#[test]
fn the_largest_data_goes_through_and_more_does_not() {
    let running = start("large");
    running.set_up(PASSWORD);
    let socket = running.socket.clone();
    let most = vec![7u8; MAX_DATA];
    let sealed = seal_at(&socket, "big", "r", &most).unwrap();
    assert_eq!(open_at(&socket, "big", "r", &sealed).unwrap(), most);
    assert_eq!(seal_at(&socket, "big", "r", &vec![0u8; MAX_DATA + 1]), Err(Refusal::TooLarge));
    // Past the client's own check, on the wire
    let more = amethystora_vault_core::protocol::encode(&vec![0u8; MAX_DATA + 1]);
    let answer = running.ask(json!({"op": "seal", "purpose": "big", "record": "r", "data": more}));
    assert_eq!(why(&answer), "too-large");
}

#[test]
fn a_connection_can_carry_many_requests_in_turn() {
    let running = start("many");
    let stream = UnixStream::connect(&running.socket).unwrap();
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    writer.write_all(b"{\"v\":1,\"op\":\"status\"}\n{\"v\":1,\"op\":\"bad op\"}\n{\"v\":1,\"op\":\"status\"}\n").unwrap();
    let mut lines = Vec::new();
    for _ in 0..3 {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        lines.push(serde_json::from_str::<Value>(&line).unwrap());
    }
    assert_eq!(lines[0]["ok"], json!(true));
    assert_eq!(why(&lines[1]), "bad-request");
    assert_eq!(lines[2]["ok"], json!(true));
}

// --- Every way in ----------------------------------------------------------------------------------------------------------

#[test]
fn every_slot_opens_the_same_vault() {
    let running = start("slots");
    let recovery = running.set_up(PASSWORD);
    let put = running.ask(json!({"op": "put", "kind": "password", "id": "a", "name": "GitHub", "value": "hunter2", "approval": "while-open"}));
    assert_eq!(put["ok"], json!(true), "{put}");
    let secret = amethystora_vault_core::protocol::encode(&[5u8; 32]);
    let credential = amethystora_vault_core::protocol::encode(b"credential");
    let salt = amethystora_vault_core::protocol::encode(b"salt");
    let answer = running.ask(json!({"op": "add-security-key", "name": "Blue", "credential": credential, "salt": salt, "secret": secret}));
    assert_eq!(answer["ok"], json!(true), "{answer}");
    let key_slot = answer["slot"].as_str().unwrap().to_string();
    assert_eq!(running.ask(json!({"op": "add-slot", "kind": "approval"}))["ok"], json!(true));
    assert_eq!(running.ask(json!({"op": "add-slot", "kind": "no-prompt"}))["ok"], json!(true));
    let methods = running.ask(json!({"op": "status"}))["vaults"][0]["methods"].clone();
    assert_eq!(methods, json!(["password", "security-key", "approval", "no-prompt", "recovery"]));
    let attempts = [
        json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}),
        json!({"op": "unlock", "method": "recovery", "secret": b64(&recovery)}),
        json!({"op": "unlock", "method": "security-key", "slot": key_slot, "secret": secret}),
        json!({"op": "unlock", "method": "approval"}),
        json!({"op": "unlock", "method": "no-prompt"}),
    ];
    for attempt in attempts {
        running.ask(json!({"op": "lock"}));
        assert_eq!(why(&running.ask(json!({"op": "get", "kind": "password", "id": "a"}))), "closed");
        let answer = running.ask(attempt.clone());
        assert_eq!(answer["ok"], json!(true), "{attempt}: {answer}");
        assert_eq!(answer["opened"][0]["name"], json!("Personal"));
        assert_eq!(running.ask(json!({"op": "get", "kind": "password", "id": "a"}))["value"], json!("hunter2"), "{attempt}");
    }
    // Wrong secrets are all one refusal
    running.ask(json!({"op": "lock"}));
    let wrong_key = amethystora_vault_core::protocol::encode(&[6u8; 32]);
    assert_eq!(why(&running.ask(json!({"op": "unlock", "method": "security-key", "slot": key_slot, "secret": wrong_key}))), "refused");
    assert_eq!(why(&running.ask(json!({"op": "unlock", "method": "password", "secret": b64("not the password")}))), "refused");
    assert_eq!(why(&running.ask(json!({"op": "unlock", "method": "recovery", "secret": b64("not a recovery key")}))), "refused");
    // Removing a slot leaves the rest
    assert_eq!(running.ask(json!({"op": "remove-slot", "slot": key_slot}))["ok"], json!(true));
    assert_eq!(why(&running.ask(json!({"op": "unlock", "method": "security-key", "slot": key_slot, "secret": secret}))), "refused");
    assert_eq!(running.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))["ok"], json!(true));
}

#[test]
fn approving_is_what_the_approval_method_needs() {
    let running = start("approval");
    running.set_up(PASSWORD);
    running.ask(json!({"op": "add-slot", "kind": "approval"}));
    running.ask(json!({"op": "lock"}));
    running.outside.said.lock().unwrap().refuse = true;
    assert_eq!(why(&running.ask(json!({"op": "unlock", "method": "approval"}))), "refused");
    assert!(opened(&running).is_empty());
    running.outside.said.lock().unwrap().refuse = false;
    assert_eq!(running.ask(json!({"op": "unlock", "method": "approval"}))["ok"], json!(true));
}

#[test]
fn setup_finishes_only_when_the_recovery_key_comes_back() {
    let running = start("typed-back");
    let answer = running.ask(json!({"op": "setup", "method": "password", "secret": b64(PASSWORD)}));
    let recovery = answer["recovery"].as_str().unwrap();
    assert_eq!(recovery.len(), 65);
    assert_eq!(running.ask(json!({"op": "status"}))["set_up"], json!(false));
    assert_eq!(why(&running.ask(json!({"op": "confirm", "recovery": b64("00000-00000")}))), "refused");
    assert_eq!(running.ask(json!({"op": "status"}))["set_up"], json!(false));
    assert!(files_under(&running.state()).is_empty(), "nothing on the disk until it is typed back");
    let lower = recovery.to_lowercase().replace('-', " ");
    assert_eq!(running.ask(json!({"op": "confirm", "recovery": b64(&lower)}))["ok"], json!(true));
    let status = running.ask(json!({"op": "status"}));
    assert_eq!(status["set_up"], json!(true));
    assert_eq!(status["vaults"][0]["open"], json!(true));
    assert_eq!(status["vaults"][0]["first"], json!(true));
    // Set up only once
    assert_eq!(why(&running.ask(json!({"op": "setup", "method": "password", "secret": b64(PASSWORD)}))), "refused");
}

#[test]
fn setup_asks_the_person_and_a_no_means_nothing_happens() {
    let running = start("setup-asks");
    running.outside.said.lock().unwrap().refuse = true;
    assert_eq!(why(&running.ask(json!({"op": "setup", "method": "password", "secret": b64(PASSWORD)}))), "refused");
    assert_eq!(running.outside.asked(), 1);
    assert_eq!(running.outside.said.lock().unwrap().asked[0].action, Action::Open);
    assert_eq!(running.ask(json!({"op": "status"}))["set_up"], json!(false));
}

#[test]
fn a_password_may_change_and_keeps_the_one_salt() {
    let running = start("change");
    let recovery = running.set_up(PASSWORD);
    let salt = account_salt(&running);
    assert_eq!(why(&running.ask(json!({"op": "change-password", "current": b64("wrong password"), "secret": b64("the new password")}))), "refused");
    assert_eq!(running.ask(json!({"op": "change-password", "current": b64(PASSWORD), "secret": b64("the new password")}))["ok"], json!(true));
    assert_eq!(account_salt(&running), salt);
    running.ask(json!({"op": "lock"}));
    assert_eq!(why(&running.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))), "refused");
    assert_eq!(running.ask(json!({"op": "unlock", "method": "password", "secret": b64("the new password")}))["ok"], json!(true));
    // The recovery key is untouched by it
    running.ask(json!({"op": "lock"}));
    assert_eq!(running.ask(json!({"op": "unlock", "method": "recovery", "secret": b64(&recovery)}))["ok"], json!(true));
}

fn account_salt(running: &Running) -> Value {
    // SAFETY: getuid cannot fail
    let uid = unsafe { libc::getuid() };
    let text = std::fs::read(running.state().join(uid.to_string()).join("account")).unwrap();
    serde_json::from_slice::<Value>(&text).unwrap()["kdf"].clone()
}

fn account_text(running: &Running) -> String {
    // SAFETY: getuid cannot fail
    let uid = unsafe { libc::getuid() };
    String::from_utf8(std::fs::read(running.state().join(uid.to_string()).join("account")).unwrap()).unwrap()
}

#[test]
fn after_five_wrong_passwords_every_try_is_put_to_the_person() {
    let running = start("tries");
    let recovery = running.set_up(PASSWORD);
    running.ask(json!({"op": "lock"}));
    let wrong = || running.ask(json!({"op": "unlock", "method": "password", "secret": b64("not the password")}));
    let right = || running.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}));
    let before = running.outside.asked();
    for _ in 0..5 {
        assert_eq!(why(&wrong()), "refused");
    }
    assert_eq!(running.outside.asked(), before, "five tries are free");
    // The sixth is put to the person, and a no is the end of it: the password is not even tried
    running.outside.said.lock().unwrap().refuse = true;
    assert_eq!(why(&right()), "refused");
    assert_eq!(running.outside.asked(), before + 1);
    // A yes, and the right password opens, and the free tries are back
    running.outside.said.lock().unwrap().refuse = false;
    assert_eq!(right()["ok"], json!(true));
    assert_eq!(running.outside.asked(), before + 2);
    running.ask(json!({"op": "lock"}));
    assert_eq!(right()["ok"], json!(true));
    assert_eq!(running.outside.asked(), before + 2, "no longer asked");
    // The recovery key is not a guess worth counting, and is the way in when the person has lost the password
    running.ask(json!({"op": "lock"}));
    for _ in 0..5 {
        wrong();
    }
    running.outside.said.lock().unwrap().refuse = true;
    assert_eq!(running.ask(json!({"op": "unlock", "method": "recovery", "secret": b64(&recovery)}))["ok"], json!(true));
}

// --- Several and hidden vaults ---------------------------------------------------------------------------------------------

#[test]
fn a_hidden_vault_is_in_no_list_and_opens_by_its_password() {
    let running = start("hidden");
    running.set_up(PASSWORD);
    let hidden_recovery = running.new_vault("Hidden name", true, "the hidden password");
    let id = running.ask(json!({"op": "status"}))["vaults"].as_array().unwrap().iter().find(|vault| vault["hidden"] == json!(true)).unwrap()["id"].as_str().unwrap().to_string();
    assert_eq!(running.ask(json!({"op": "put", "kind": "password", "id": "h", "name": "Secret item", "value": "s3cret", "vault": id}))["ok"], json!(true));
    running.ask(json!({"op": "lock"}));
    // Closed, nothing says it exists
    for request in [json!({"op": "status"}), json!({"op": "methods"})] {
        let answer = running.ask(request).to_string();
        assert!(!answer.contains("Hidden name"), "{answer}");
        assert!(!answer.contains(&id), "{answer}");
    }
    assert_eq!(running.ask(json!({"op": "status"}))["vaults"].as_array().unwrap().len(), 1);
    assert_eq!(why(&running.ask(json!({"op": "list"}))), "closed");
    assert_eq!(why(&running.ask(json!({"op": "get", "kind": "password", "id": "h"}))), "closed");
    // Asked for by id, it answers as any vault id that is not open, or nothing at all
    assert_eq!(why(&running.ask(json!({"op": "unlock", "method": "approval", "vault": id}))), "refused");
    let made_up = "00".repeat(16);
    assert_eq!(why(&running.ask(json!({"op": "unlock", "method": "approval", "vault": made_up}))), "refused");
    // Its name and id are nowhere in the account's file
    let text = account_text(&running);
    assert!(!text.contains("Hidden name"));
    assert!(!text.contains(&id));
    assert!(!text.contains("Personal"), "a listed vault's name is sealed too");
    // Its own password opens it, and only it
    let answer = running.ask(json!({"op": "unlock", "method": "password", "secret": b64("the hidden password")}));
    assert_eq!(answer["opened"].as_array().unwrap().len(), 1);
    assert_eq!(answer["opened"][0]["name"], json!("Hidden name"));
    assert_eq!(answer["opened"][0]["hidden"], json!(true));
    let status = running.ask(json!({"op": "status"}));
    assert_eq!(status["vaults"].as_array().unwrap().len(), 2);
    assert_eq!(status["vaults"][0]["open"], json!(false));
    assert_eq!(status["vaults"][0]["name"], Value::Null, "a closed vault's name is not known");
    assert_eq!(running.ask(json!({"op": "get", "kind": "password", "id": "h"}))["value"], json!("s3cret"));
    // No slot but the password's and the recovery key's
    assert_eq!(why(&running.ask(json!({"op": "add-slot", "kind": "approval", "vault": id}))), "refused");
    // Locking closes all of them, and its recovery key opens it again
    running.ask(json!({"op": "lock"}));
    let answer = running.ask(json!({"op": "unlock", "method": "recovery", "secret": b64(&hidden_recovery)}));
    assert_eq!(answer["opened"][0]["id"], json!(id));
    // Data keys are only ever the first vault's, so the desktop never depends on one it cannot see
    assert_eq!(seal_at(&running.socket, "resume", "r", b"x"), Err(Refusal::Closed));
}

#[test]
fn a_password_that_opens_another_vault_is_refused_like_any_unusable_one() {
    let running = start("shadow");
    running.set_up(PASSWORD);
    running.new_vault("Hidden", true, "the hidden password");
    // Exactly the same answer: an in-use password, too short, and past the limit
    let in_use = running.ask(json!({"op": "new", "name": "n", "secret": b64("the hidden password")}));
    let short = running.ask(json!({"op": "new", "name": "n", "secret": b64("short")}));
    let long = running.ask(json!({"op": "new", "name": "n", "secret": b64(&"x".repeat(2000))}));
    assert_eq!(in_use, json!({"ok": false, "why": "refused"}));
    assert_eq!(short, in_use);
    assert_eq!(long, in_use);
    let changed = running.ask(json!({"op": "change-password", "current": b64("the hidden password"), "secret": b64(PASSWORD)}));
    assert_eq!(changed, in_use);
    let short = running.ask(json!({"op": "change-password", "current": b64("the hidden password"), "secret": b64("short")}));
    assert_eq!(short, in_use);
    // A vault may keep its own password
    assert_eq!(running.ask(json!({"op": "change-password", "current": b64("the hidden password"), "secret": b64("the hidden password")}))["ok"], json!(true));
}

#[test]
fn trying_a_password_does_not_depend_on_how_many_vaults_there_are_or_where_it_matches() {
    // Time for time, with one vault and with thirty, a wrong password and a right one at either end of the table, each the
    // least of several, which is what a busy machine's noise leaves alone: the derivation is all that counts. A right password
    // after each try keeps the wrong ones from starting to wait.
    let time_of = |running: &Running, password: &str| {
        (0..9)
            .map(|_| {
                running.ask(json!({"op": "lock"}));
                let started = Instant::now();
                running.ask(json!({"op": "unlock", "method": "password", "secret": b64(password)}));
                let took = started.elapsed().as_secs_f64();
                running.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}));
                took
            })
            .fold(f64::MAX, f64::min)
    };
    let one = start_with("timing-one", 14, Duration::from_secs(600));
    one.set_up(PASSWORD);
    let many = start_with("timing-many", 14, Duration::from_secs(600));
    many.set_up(PASSWORD);
    for at in 0..29 {
        many.new_vault(&format!("vault {at}"), at % 2 == 0, &format!("password number {at}"));
    }
    let wrong_with_one = time_of(&one, "not the password");
    let wrong_with_many = time_of(&many, "not the password");
    let right_with_many_first = time_of(&many, PASSWORD);
    let right_with_many_last = time_of(&many, "password number 28");
    let times = [wrong_with_one, wrong_with_many, right_with_many_first, right_with_many_last];
    let (least, most) = (times.iter().copied().fold(f64::MAX, f64::min), times.iter().copied().fold(0.0, f64::max));
    println!("wrong, 1 vault: {wrong_with_one:.4}s; wrong, 30 vaults: {wrong_with_many:.4}s; right, first of 30: {right_with_many_first:.4}s; right, last of 30: {right_with_many_last:.4}s");
    assert!(most - least < 0.05, "{times:?}");
}

#[test]
fn removing_one_vault_removes_only_the_open_one_and_forgetting_removes_all() {
    let running = start("removal");
    running.set_up(PASSWORD);
    running.new_vault("Work", false, "the work password");
    // Not the first
    let first = running.ask(json!({"op": "status"}))["vaults"].as_array().unwrap().iter().find(|vault| vault["first"] == json!(true)).unwrap()["id"].clone();
    assert_eq!(why(&running.ask(json!({"op": "remove-this", "vault": first}))), "refused");
    assert_eq!(running.ask(json!({"op": "remove-this"}))["ok"], json!(true));
    assert_eq!(running.ask(json!({"op": "status"}))["vaults"].as_array().unwrap().len(), 1);
    running.ask(json!({"op": "lock"}));
    assert_eq!(why(&running.ask(json!({"op": "unlock", "method": "password", "secret": b64("the work password")}))), "refused");
    assert_eq!(running.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))["ok"], json!(true));
    // A no keeps everything
    running.new_vault("Hidden", true, "the hidden password");
    running.outside.said.lock().unwrap().refuse = true;
    assert_eq!(why(&running.ask(json!({"op": "forget-everything"}))), "refused");
    assert_eq!(running.ask(json!({"op": "status"}))["set_up"], json!(true));
    running.outside.said.lock().unwrap().refuse = false;
    assert_eq!(running.ask(json!({"op": "forget-everything"}))["ok"], json!(true));
    assert_eq!(running.ask(json!({"op": "status"}))["set_up"], json!(false));
    assert!(opened(&running).is_empty());
    assert!(files_under(&running.state()).is_empty());
}

#[test]
fn a_vault_can_be_called_something_else_while_open() {
    let running = start("rename");
    running.set_up(PASSWORD);
    assert_eq!(running.ask(json!({"op": "rename", "name": "Mine"}))["ok"], json!(true));
    assert_eq!(running.ask(json!({"op": "status"}))["vaults"][0]["name"], json!("Mine"));
    assert_eq!(why(&running.ask(json!({"op": "rename", "name": ""}))), "bad-request");
    running.ask(json!({"op": "lock"}));
    assert_eq!(why(&running.ask(json!({"op": "rename", "name": "Other"}))), "closed");
    let answer = running.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}));
    assert_eq!(answer["opened"][0]["name"], json!("Mine"));
}

// --- Items -----------------------------------------------------------------------------------------------------------------

#[test]
fn items_ask_as_each_was_set_to() {
    let running = start("items");
    running.set_up(PASSWORD);
    let base = running.outside.asked();
    let put = |id: &str, approval: &str| running.ask(json!({"op": "put", "kind": "password", "id": id, "name": format!("Item {id}"), "value": {"secret": id}, "meta": {"user": "me"}, "approval": approval}));
    assert_eq!(put("a", "ask")["ok"], json!(true));
    assert_eq!(put("w", "while-open")["ok"], json!(true));
    assert_eq!(running.outside.asked(), base, "making an item that will ask needs no asking");
    // Never ask is only set with the person's word
    assert_eq!(put("n", "never")["ok"], json!(true));
    assert_eq!(running.outside.asked(), base + 1);
    let get = |id: &str| running.ask(json!({"op": "get", "kind": "password", "id": id}));
    let asked = || running.outside.asked();
    // Ask every time
    let before = asked();
    assert_eq!(get("a")["value"]["secret"], json!("a"));
    assert_eq!(get("a")["value"]["secret"], json!("a"));
    assert_eq!(asked(), before + 2);
    // While open: once, until it closes
    let before = asked();
    assert_eq!(get("w")["value"]["secret"], json!("w"));
    assert_eq!(get("w")["value"]["secret"], json!("w"));
    assert_eq!(asked(), before + 1);
    // Never: not at all
    let before = asked();
    assert_eq!(get("n")["value"]["secret"], json!("n"));
    assert_eq!(asked(), before);
    // The prompt names the item
    assert!(running.outside.said.lock().unwrap().asked.iter().any(|ask| ask.item == "Item a"));
    // Closing forgets While open's approval
    running.ask(json!({"op": "lock"}));
    running.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}));
    let before = asked();
    get("w");
    assert_eq!(asked(), before + 1);
    // A no gets nothing, and says no more than a missing item
    running.outside.said.lock().unwrap().refuse = true;
    assert_eq!(get("a"), json!({"ok": false, "why": "refused"}));
    assert_eq!(get("missing"), json!({"ok": false, "why": "refused"}));
    running.outside.said.lock().unwrap().refuse = false;
    // Lowering how often it asks asks; raising it does not need to
    let before = asked();
    assert_eq!(running.ask(json!({"op": "set-approval", "kind": "password", "id": "a", "approval": "never"}))["ok"], json!(true));
    assert_eq!(asked(), before + 1);
    let before = asked();
    get("a");
    assert_eq!(asked(), before);
    // Overwriting with a lower setting asks even when it would not otherwise
    let before = asked();
    assert_eq!(put("w", "never")["ok"], json!(true));
    assert_eq!(asked(), before + 1);
    // The list needs no approval and holds no value
    let before = asked();
    let list = running.ask(json!({"op": "list", "kind": "password"}));
    assert_eq!(asked(), before);
    assert_eq!(list["items"].as_array().unwrap().len(), 3);
    assert_eq!(list["items"][0]["meta"]["user"], json!("me"));
    assert!(!list.to_string().contains("secret"));
    // Removing always asks
    let before = asked();
    assert_eq!(running.ask(json!({"op": "remove", "kind": "password", "id": "n"}))["ok"], json!(true));
    assert_eq!(asked(), before + 1);
    assert_eq!(get("n"), json!({"ok": false, "why": "refused"}));
}

#[test]
fn what_only_the_service_may_hold_cannot_be_read_or_set() {
    let running = start("service-only");
    running.set_up(PASSWORD);
    seal_at(&running.socket, "resume", "r", b"x").unwrap();
    assert_eq!(why(&running.ask(json!({"op": "get", "kind": "data-key", "id": "resume"}))), "refused");
    assert_eq!(why(&running.ask(json!({"op": "put", "kind": "data-key", "id": "resume", "name": "n", "value": "00"}))), "refused");
    assert_eq!(why(&running.ask(json!({"op": "set-approval", "kind": "data-key", "id": "resume", "approval": "ask"}))), "refused");
    // It is listed, under the feature that uses it, and can be removed with the person's word
    let list = running.ask(json!({"op": "list", "kind": "data-key"}));
    assert_eq!(list["items"][0]["id"], json!("resume"));
    assert_eq!(running.ask(json!({"op": "remove", "kind": "data-key", "id": "resume"}))["ok"], json!(true));
}

#[test]
fn an_item_is_found_in_whichever_open_vault_has_it() {
    let running = start("several");
    running.set_up(PASSWORD);
    running.ask(json!({"op": "put", "kind": "k", "id": "first", "name": "n", "value": 1, "approval": "while-open"}));
    running.new_vault("Second", false, "the second password");
    running.ask(json!({"op": "put", "kind": "k", "id": "second", "name": "n", "value": 2, "approval": "while-open"}));
    assert_eq!(running.ask(json!({"op": "get", "kind": "k", "id": "first"}))["value"], json!(1));
    assert_eq!(running.ask(json!({"op": "get", "kind": "k", "id": "second"}))["value"], json!(2));
    assert_eq!(running.ask(json!({"op": "list", "kind": "k"}))["items"].as_array().unwrap().len(), 2);
    // Only the second open: the first's item is not there
    running.ask(json!({"op": "lock"}));
    running.ask(json!({"op": "unlock", "method": "password", "secret": b64("the second password")}));
    assert_eq!(why(&running.ask(json!({"op": "get", "kind": "k", "id": "first"}))), "refused");
    assert_eq!(running.ask(json!({"op": "list", "kind": "k"}))["items"].as_array().unwrap().len(), 1);
}

#[test]
fn a_file_put_back_from_before_is_said_and_not_opened() {
    let running = start("stale");
    running.set_up(PASSWORD);
    running.ask(json!({"op": "put", "kind": "k", "id": "a", "name": "n", "value": "old", "approval": "while-open"}));
    let file = files_under(&running.state()).into_iter().find(|path| path.ends_with("items/k.a")).unwrap();
    let before = std::fs::read(&file).unwrap();
    running.ask(json!({"op": "put", "kind": "k", "id": "a", "name": "n", "value": "new"}));
    std::fs::write(&file, before).unwrap();
    let answer = running.ask(json!({"op": "get", "kind": "k", "id": "a"}));
    assert_eq!(why(&answer), "failed");
    assert_eq!(answer["stale"], json!(true));
}

// --- Closing ---------------------------------------------------------------------------------------------------------------

/// The account's uid as the service sees it here
fn me() -> u32 {
    // SAFETY: getuid cannot fail
    unsafe { libc::getuid() }
}

fn closes_on(event: Event) {
    let running = start("closes");
    running.set_up(PASSWORD);
    assert_eq!(opened(&running).len(), 1);
    assert!(running.outside.said.lock().unwrap().watching);
    running.outside.push(event.clone());
    for _ in 0..200 {
        if opened(&running).is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(opened(&running).is_empty(), "{event:?}");
    assert_eq!(seal_at(&running.socket, "resume", "r", b"x"), Err(Refusal::Closed), "{event:?}");
    assert!(!running.outside.said.lock().unwrap().watching, "{event:?}: nothing left to watch for");
}

#[test]
fn it_closes_when_the_screen_locks() {
    closes_on(Event::Locked(me()));
}

#[test]
fn it_closes_before_the_machine_sleeps() {
    closes_on(Event::Sleeping);
}

#[test]
fn it_closes_when_the_account_has_no_session_left() {
    closes_on(Event::SessionsLeft(vec![me() + 1]));
}

#[test]
fn it_closes_when_nothing_can_tell_it_to() {
    closes_on(Event::Lost);
}

#[test]
fn another_account_locking_does_not_stop_this_one_opening() {
    let running = start_with("other-lock", 17, Duration::from_secs(600));
    running.set_up(PASSWORD);
    running.ask(json!({"op": "lock"}));
    let socket = running.socket.clone();
    let opening = std::thread::spawn(move || {
        amethystora_vault_client::exchange(&socket, &json!({"v": 1, "op": "unlock", "method": "password", "secret": b64(PASSWORD)}), Duration::from_secs(30)).unwrap()
    });
    std::thread::sleep(Duration::from_millis(50));
    running.outside.push(Event::Locked(me() + 1));
    assert_eq!(opening.join().unwrap()["ok"], json!(true));
}

#[test]
fn another_account_locking_or_leaving_leaves_this_one_open() {
    let running = start("others");
    running.set_up(PASSWORD);
    running.outside.push(Event::Locked(me() + 1));
    running.outside.push(Event::SessionsLeft(vec![me()]));
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(opened(&running).len(), 1);
}

#[test]
fn a_prompt_up_when_the_screen_locks_is_taken_back() {
    for (event, another_account) in [(Event::Locked(me()), false), (Event::Sleeping, false), (Event::Lost, false), (Event::SessionsLeft(vec![]), false), (Event::Locked(me() + 1), true)] {
        let running = start("prompt");
        running.set_up(PASSWORD);
        running.ask(json!({"op": "put", "kind": "k", "id": "a", "name": "n", "value": 1, "approval": "ask"}));
        running.outside.said.lock().unwrap().silent = true;
        let socket = running.socket.clone();
        let waiting = std::thread::spawn(move || amethystora_vault_client::exchange(&socket, &json!({"v": 1, "op": "get", "kind": "k", "id": "a"}), Duration::from_secs(3)));
        running.wait_until(|said| said.asked.iter().any(|ask| ask.item == "n"));
        running.outside.push(event.clone());
        if another_account {
            // Somebody else's lock is nothing to this prompt
            std::thread::sleep(Duration::from_millis(50));
            assert!(running.outside.said.lock().unwrap().cancelled.is_empty());
            running.outside.said.lock().unwrap().silent = false;
            assert_eq!(waiting.join().unwrap(), Err(Refusal::Failed), "the client gave up waiting");
        } else {
            assert_eq!(why(&waiting.join().unwrap().unwrap()), "closed", "{event:?}");
            assert_eq!(running.outside.said.lock().unwrap().cancelled.len(), 1, "{event:?}");
        }
    }
}

#[test]
fn nothing_opens_when_nothing_could_say_to_close_it() {
    let running = start("no-watch");
    running.set_up(PASSWORD);
    running.ask(json!({"op": "lock"}));
    running.outside.said.lock().unwrap().watch_fails = true;
    assert_eq!(why(&running.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}))), "failed");
    assert!(opened(&running).is_empty());
}

#[test]
fn an_opening_overtaken_by_a_lock_does_not_open() {
    let running = start_with("overtaken", 17, Duration::from_secs(600));
    running.set_up(PASSWORD);
    running.ask(json!({"op": "lock"}));
    let socket = running.socket.clone();
    let opening = std::thread::spawn(move || {
        amethystora_vault_client::exchange(&socket, &json!({"v": 1, "op": "unlock", "method": "password", "secret": b64(PASSWORD)}), Duration::from_secs(30)).unwrap()
    });
    // scrypt takes a good part of a second; the screen locks meanwhile
    std::thread::sleep(Duration::from_millis(50));
    running.outside.push(Event::Sleeping);
    let answer = opening.join().unwrap();
    assert_eq!(why(&answer), "closed", "{answer}");
    assert!(opened(&running).is_empty());
}

// --- What a client can do to it ------------------------------------------------------------------------------------------

#[test]
fn bad_input_gets_a_refusal_and_never_ends_it() {
    let running = start("bad");
    running.set_up(PASSWORD);
    let stream = UnixStream::connect(&running.socket).unwrap();
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    let lines: Vec<Vec<u8>> = vec![
        b"".to_vec(),
        b"not json".to_vec(),
        b"null".to_vec(),
        b"[]".to_vec(),
        vec![0xff, 0xfe, 0x00, 0x80],
        br#"{"v":1}"#.to_vec(),
        br#"{"v":2,"op":"status"}"#.to_vec(),
        br#"{"v":1,"op":"seal","purpose":"../../etc","record":"","data":""}"#.to_vec(),
        br#"{"v":1,"op":"seal","purpose":"a","record":"a\nb","data":""}"#.to_vec(),
        br#"{"v":1,"op":"seal","purpose":{},"record":[],"data":1}"#.to_vec(),
        br#"{"v":1,"op":"get","kind":"k","id":".."}"#.to_vec(),
        br#"{"v":1,"op":"get","kind":"../k","id":"a"}"#.to_vec(),
        br#"{"v":1,"op":"put","kind":"k","id":"a/b","name":"n","value":1}"#.to_vec(),
        br#"{"v":1,"op":"remove","kind":"k","id":"..\\.."}"#.to_vec(),
        br#"{"v":1,"op":"unlock","method":"password","vault":"../../other"}"#.to_vec(),
        br#"{"v":1,"op":"set-idle","minutes":99999999999999}"#.to_vec(),
        format!("{}1{}", "[".repeat(5000), "]".repeat(5000)).into_bytes(),
        format!(r#"{{"v":1,"op":"{}"}}"#, "x".repeat(10_000)).into_bytes(),
    ];
    for line in &lines {
        writer.write_all(line).unwrap();
        writer.write_all(b"\n").unwrap();
        let mut answer = String::new();
        reader.read_line(&mut answer).unwrap();
        let answer: Value = serde_json::from_str(&answer).unwrap();
        assert_eq!(answer["ok"], json!(false), "{}", String::from_utf8_lossy(line));
        assert_eq!(why(&answer), "bad-request", "{}", String::from_utf8_lossy(line));
    }
    // And nothing strayed outside the folder, and it still answers
    assert!(running.ask(json!({"op": "status"}))["set_up"] == json!(true));
    assert!(!running.folder.0.join("etc").exists());
}

#[test]
fn a_line_past_the_limit_is_refused_and_the_connection_closed() {
    let running = start("huge");
    let mut stream = UnixStream::connect(&running.socket).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(20))).unwrap();
    // More than the limit and no newline: writing stops being taken at the limit
    let chunk = vec![b'x'; 1 << 20];
    let mut written = 0usize;
    while written < MAX_LINE * 3 {
        match stream.write(&chunk) {
            Ok(count) => written += count,
            Err(_) => break,
        }
    }
    let mut answer = String::new();
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let _ = stream.read_to_string(&mut answer);
    let answer: Value = serde_json::from_str(&answer).unwrap();
    assert_eq!(why(&answer), "too-large");
    // The next client is not troubled by it
    assert_eq!(running.ask(json!({"op": "status"}))["ok"], json!(true));
}

#[test]
fn a_burst_of_empty_lines_is_answered_in_turn_and_runs_nothing_out() {
    let running = start("burst");
    let stream = UnixStream::connect(&running.socket).unwrap();
    let mut writer = stream.try_clone().unwrap();
    let count = MAX_LINE / 2;
    let writing = std::thread::spawn(move || {
        let lines = vec![b'\n'; count];
        let _ = writer.write_all(&lines);
    });
    let mut reader = BufReader::new(stream);
    let mut answered = 0;
    let mut line = String::new();
    while answered < count {
        line.clear();
        if reader.read_line(&mut line).unwrap() == 0 {
            break;
        }
        answered += 1;
    }
    writing.join().unwrap();
    assert_eq!(answered, count);
    assert_eq!(running.ask(json!({"op": "status"}))["ok"], json!(true));
}

#[test]
fn half_a_line_does_not_hold_anything_up() {
    let running = start("half");
    let mut slow = UnixStream::connect(&running.socket).unwrap();
    slow.write_all(b"{\"v\":1,\"op\":").unwrap();
    assert_eq!(running.ask(json!({"op": "status"}))["ok"], json!(true));
    drop(slow);
}

#[test]
fn someone_leaving_mid_request_is_nothing_to_anyone_else() {
    let running = start("leaving");
    running.set_up(PASSWORD);
    running.ask(json!({"op": "put", "kind": "k", "id": "a", "name": "n", "value": 1, "approval": "ask"}));
    running.outside.said.lock().unwrap().silent = true;
    let mut leaving = UnixStream::connect(&running.socket).unwrap();
    leaving.write_all(b"{\"v\":1,\"op\":\"get\",\"kind\":\"k\",\"id\":\"a\"}\n").unwrap();
    let before = running.outside.said.lock().unwrap().cancelled.len();
    running.wait_until(|said| said.asked.iter().any(|ask| ask.item == "n"));
    // Others are answered while the person thinks
    assert_eq!(running.ask(json!({"op": "status"}))["ok"], json!(true));
    drop(leaving);
    // The prompt goes with whoever asked
    running.wait_until(|said| said.cancelled.len() == before + 1);
    running.outside.said.lock().unwrap().silent = false;
    assert_eq!(running.ask(json!({"op": "status"}))["ok"], json!(true));
}

#[test]
fn scrypt_never_stalls_the_others() {
    let running = start_with("stall", 17, Duration::from_secs(600));
    let socket = running.socket.clone();
    let setting_up = std::thread::spawn(move || {
        amethystora_vault_client::exchange(&socket, &json!({"v": 1, "op": "setup", "method": "password", "secret": b64(PASSWORD)}), Duration::from_secs(60)).unwrap()
    });
    std::thread::sleep(Duration::from_millis(30));
    let started = Instant::now();
    let mut times = Vec::new();
    while !setting_up.is_finished() {
        let one = Instant::now();
        assert_eq!(running.ask(json!({"op": "status"}))["ok"], json!(true));
        times.push(one.elapsed());
        std::thread::sleep(Duration::from_millis(5));
    }
    times.sort();
    println!("{} answers in {:?} of scrypt, the slowest {:?}", times.len(), started.elapsed(), times.last());
    // Not the very slowest: on a machine this busy a thread can lose the processor for longer than any of this takes
    assert!(times.len() > 3, "{times:?}");
    assert!(times[times.len() * 9 / 10] < Duration::from_millis(100), "{times:?}");
    assert_eq!(setting_up.join().unwrap()["ok"], json!(true));
}

#[test]
fn too_much_work_at_once_is_refused_not_queued_for_ever() {
    let running = start_with("queue", 17, Duration::from_secs(600));
    running.set_up(PASSWORD);
    running.ask(json!({"op": "lock"}));
    let mut held = Vec::new();
    let line = format!("{}\n", json!({"v": 1, "op": "unlock", "method": "password", "secret": b64("not the password")}));
    for _ in 0..12 {
        let mut stream = match UnixStream::connect(&running.socket) {
            Ok(stream) => stream,
            Err(_) => break,
        };
        stream.write_all(line.as_bytes()).unwrap();
        held.push(stream);
    }
    // One running and four waiting at most: the rest hear so at once
    let mut failed = 0;
    for stream in &held[5..] {
        stream.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        let mut answer = String::new();
        BufReader::new(stream).read_line(&mut answer).unwrap();
        if answer.contains("failed") {
            failed += 1;
        }
    }
    assert!(failed >= 5, "{failed}");
    drop(held);
    assert_eq!(running.ask(json!({"op": "status"}))["ok"], json!(true));
}

#[test]
fn many_connections_are_turned_away_past_an_account_s_share() {
    let running = start("connections");
    let mut held = Vec::new();
    for _ in 0..40 {
        if let Ok(stream) = UnixStream::connect(&running.socket) {
            held.push(stream);
        }
    }
    // Everyone past the sixteenth was hung up on
    let mut answered = 0;
    for stream in &mut held {
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        if stream.write_all(b"{\"v\":1,\"op\":\"status\"}\n").is_err() {
            continue;
        }
        let mut line = String::new();
        if BufReader::new(&*stream).read_line(&mut line).is_ok() && line.contains("\"ok\":true") {
            answered += 1;
        }
    }
    assert_eq!(answered, 16);
    drop(held);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(running.ask(json!({"op": "status"}))["ok"], json!(true));
}

// --- Another account -------------------------------------------------------------------------------------------------------

/// Run by the test below, as another uid, against the socket it names: everything it can reach of the first account
#[test]
#[ignore = "the other account's half of a_second_account_cannot_reach_the_first_s_vault"]
fn other_account() {
    let socket = std::path::PathBuf::from(std::env::var("OTHER_SOCKET").unwrap());
    let ask = |request: Value| amethystora_vault_client::exchange(&socket, &request, Duration::from_secs(10)).unwrap();
    let status = ask(json!({"v": 1, "op": "status"}));
    assert_eq!(status["set_up"], json!(false), "{status}");
    assert_eq!(status["vaults"], json!([]));
    assert_eq!(seal_at(&socket, "resume", "r", b"x"), Err(Refusal::NotSetUp));
    let sealed = amethystora_vault_core::seal::unhex(&std::env::var("OTHER_SEALED").unwrap()).unwrap();
    let sealed = amethystora_vault_core::protocol::encode(&sealed);
    assert_eq!(why(&ask(json!({"v": 1, "op": "open", "purpose": "resume", "record": "r", "data": sealed}))), "not-set-up");
    assert_eq!(why(&ask(json!({"v": 1, "op": "get", "kind": "k", "id": "a"}))), "not-set-up");
    assert_eq!(why(&ask(json!({"v": 1, "op": "unlock", "method": "password", "secret": b64(PASSWORD)}))), "not-set-up");
    assert_eq!(why(&ask(json!({"v": 1, "op": "list"}))), "not-set-up");
    // What it sets up is its own, and even this account's password opens only that
    let answer = ask(json!({"v": 1, "op": "setup", "method": "password", "secret": b64("the other password")}));
    let recovery = answer["recovery"].as_str().unwrap();
    assert_eq!(ask(json!({"v": 1, "op": "confirm", "recovery": b64(recovery)}))["ok"], json!(true));
    assert_eq!(ask(json!({"v": 1, "op": "lock"}))["ok"], json!(true));
    assert_eq!(why(&ask(json!({"v": 1, "op": "unlock", "method": "password", "secret": b64(PASSWORD)}))), "refused");
    // Its own open fails on the first account's file: a different vault id and key
    assert_eq!(ask(json!({"v": 1, "op": "unlock", "method": "password", "secret": b64("the other password")}))["ok"], json!(true));
    let other = ask(json!({"v": 1, "op": "seal", "purpose": "resume", "record": "r", "data": ""}));
    assert_eq!(other["ok"], json!(true));
}

#[test]
fn a_second_account_cannot_reach_the_first_s_vault() {
    // SAFETY: getuid cannot fail
    if unsafe { libc::getuid() } != 0 {
        eprintln!("skipped: needs root, to run the other half as another account");
        return;
    }
    let running = start("two-accounts");
    running.set_up(PASSWORD);
    let sealed = seal_at(&running.socket, "resume", "r", b"plain").unwrap();
    use std::os::unix::process::CommandExt;
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "other_account", "--nocapture"])
        .env("OTHER_SOCKET", &running.socket)
        .env("OTHER_SEALED", amethystora_vault_core::seal::hex(&sealed))
        .uid(1234)
        .gid(1234)
        .status()
        .unwrap();
    assert!(status.success());
    // The first account's is as it was
    assert_eq!(open_at(&running.socket, "resume", "r", &sealed).unwrap(), b"plain");
    assert_eq!(running.ask(json!({"op": "status"}))["vaults"].as_array().unwrap().len(), 1);
    // Each account has its folder, 0700, and the other's sealed data does not open under this key
    use std::os::unix::fs::PermissionsExt;
    for account in ["0", "1234"] {
        let mode = std::fs::metadata(running.state().join(account)).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "{account}");
    }
}

// --- On the disk -----------------------------------------------------------------------------------------------------------

#[test]
fn nothing_secret_is_on_the_disk_in_the_clear() {
    let running = start("disk");
    let recovery = running.set_up(PASSWORD);
    running.new_vault("Hidden name", true, "the hidden password");
    running.ask(json!({"op": "put", "kind": "password", "id": "a", "name": "Item name", "value": "hunter2", "approval": "ask"}));
    let needles: [&[u8]; 6] = [PASSWORD.as_bytes(), recovery.as_bytes(), b"Personal", b"Hidden name", b"Item name", b"hunter2"];
    use std::os::unix::fs::PermissionsExt;
    let files = files_under(&running.state());
    assert!(files.len() >= 6, "{files:?}");
    for file in files {
        let bytes = std::fs::read(&file).unwrap();
        for needle in needles {
            assert!(!bytes.windows(needle.len()).any(|window| window == needle), "{file:?}");
        }
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600, "{file:?}");
    }
}

#[test]
fn the_background_key_works_with_the_vault_closed_and_only_it() {
    let running = start("background");
    running.set_up(PASSWORD);
    let socket = running.socket.clone();
    assert_eq!(running.ask(json!({"op": "background", "on": true}))["ok"], json!(true));
    let open_sealed = seal_at(&socket, "background", "daily", b"plain").unwrap();
    let other = seal_at(&socket, "resume", "r", b"x").unwrap();
    running.ask(json!({"op": "lock"}));
    // Closed: the same key, off the chip, both ways
    assert_eq!(open_at(&socket, "background", "daily", &open_sealed).unwrap(), b"plain");
    let closed_sealed = seal_at(&socket, "background", "weekly", b"more").unwrap();
    // No other purpose
    assert_eq!(open_at(&socket, "resume", "r", &other), Err(Refusal::Closed));
    assert_eq!(status_list_is_closed(&running), 0);
    // And what was sealed closed opens when it is open
    running.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}));
    assert_eq!(open_at(&socket, "background", "weekly", &closed_sealed).unwrap(), b"more");
    // Turned off, closed means closed
    assert_eq!(running.ask(json!({"op": "background", "on": false}))["ok"], json!(true));
    running.ask(json!({"op": "lock"}));
    assert_eq!(open_at(&socket, "background", "daily", &open_sealed), Err(Refusal::Closed));
}

fn status_list_is_closed(running: &Running) -> usize {
    opened(running).len()
}

#[test]
fn without_the_chip_there_is_no_background_key_and_no_no_prompt() {
    // The fake chip here is present, so this goes through what setup is told: an unknown method is a bad request
    let running = start("methods");
    assert_eq!(why(&running.ask(json!({"op": "setup", "method": "recovery"}))), "bad-request");
    assert_eq!(why(&running.ask(json!({"op": "setup", "method": "password"}))), "refused");
    assert_eq!(why(&running.ask(json!({"op": "setup", "method": "approval", "secret": b64(PASSWORD)}))), "bad-request");
}

#[test]
fn it_goes_away_when_nothing_is_open_and_stays_while_something_is() {
    let running = start_with("exit", 10, Duration::from_millis(300));
    running.set_up(PASSWORD);
    std::thread::sleep(Duration::from_millis(600));
    // Open: still here
    assert_eq!(running.ask(json!({"op": "status"}))["ok"], json!(true));
    running.ask(json!({"op": "lock"}));
    let thread_gone = {
        let started = Instant::now();
        loop {
            // Connecting succeeds while the socket's backlog has room, so ask for an answer
            let Ok(mut stream) = UnixStream::connect(&running.socket) else { break true };
            stream.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
            stream.write_all(b"{\"v\":1,\"op\":\"lock\"}\n").unwrap();
            let mut answer = String::new();
            if BufReader::new(stream).read_line(&mut answer).is_err() || answer.is_empty() {
                break true;
            }
            if started.elapsed() > Duration::from_secs(5) {
                break false;
            }
            std::thread::sleep(Duration::from_millis(400));
        }
    };
    assert!(thread_gone);
}

/// The sealed backup (plans/PLAN-vault.md, Backup and moving): taken with the vault closed and no question asked, holding a hidden vault
/// like any other, and bringing back everything from the recovery key and a new way in
#[test]
fn a_backup_is_taken_closed_and_brings_every_vault_back_from_its_recovery_key() {
    let running = start("backup-restore");
    running.set_up(PASSWORD);
    let hidden_recovery = running.new_vault("Not listed", true, "the hidden password");
    let hidden = opened(&running).iter().find(|vault| vault["hidden"] == json!(true)).unwrap()["id"].as_str().unwrap().to_string();
    assert_eq!(running.ask(json!({"op": "put", "kind": "password", "id": "h", "name": "In the hidden one", "value": "s3cret", "approval": "while-open", "vault": hidden}))["ok"], json!(true));
    // Larger than a chunk, so that the file made at the first is the one the rest are read from
    for id in ["big-1", "big-2"] {
        assert_eq!(running.ask(json!({"op": "put", "kind": "password", "id": id, "name": id, "value": "x".repeat(300 * 1024), "approval": "while-open"}))["ok"], json!(true));
    }
    running.ask(json!({"op": "lock"}));
    let asked = running.outside.asked();
    // A chunk at a time, as the command line reads it; the vault is closed and nobody is asked
    let mut file = Vec::new();
    let mut chunks = 0;
    loop {
        chunks += 1;
        let answer = running.ask(json!({"op": "vault-backup", "offset": file.len()}));
        assert_eq!(answer["ok"], json!(true), "{answer}");
        use base64::Engine;
        file.extend(base64::engine::general_purpose::STANDARD.decode(answer["data"].as_str().unwrap()).unwrap());
        if file.len() >= answer["size"].as_u64().unwrap() as usize {
            break;
        }
    }
    assert!(chunks >= 2, "{chunks} chunk");
    assert_eq!(running.outside.asked(), asked);
    let text = String::from_utf8_lossy(&file);
    assert!(!text.contains("Not listed") && !text.contains("In the hidden one") && !text.contains(PASSWORD));
    // Gone: this account forgot everything
    running.ask(json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}));
    assert_eq!(running.ask(json!({"op": "forget-everything"}))["ok"], json!(true));
    assert!(files_under(&running.state()).is_empty());
    // Not a backup, a stranger's recovery key, and then the right one
    let upload = |bytes: &[u8]| {
        use base64::Engine;
        for (at, chunk) in bytes.chunks(64 * 1024).enumerate() {
            assert_eq!(running.ask(json!({"op": "vault-restore-put", "offset": at * 64 * 1024, "data": base64::engine::general_purpose::STANDARD.encode(chunk)}))["ok"], json!(true));
        }
    };
    upload(b"this is not a backup");
    assert_eq!(why(&running.ask(json!({"op": "vault-restore", "recovery": b64(&hidden_recovery)}))), "bad-request");
    upload(&file);
    let stranger = amethystora_vault_core::slots::RecoveryKey::generate().text().to_string();
    assert_eq!(why(&running.ask(json!({"op": "vault-restore", "recovery": b64(&stranger)}))), "refused");
    assert!(files_under(&running.state()).is_empty());
    // A restore makes vaults, so it is asked as a new vault is: refused, nothing comes back
    upload(&file);
    running.outside.said.lock().unwrap().refuse = true;
    assert_eq!(why(&running.ask(json!({"op": "vault-restore", "recovery": b64(&hidden_recovery)}))), "refused");
    running.outside.said.lock().unwrap().refuse = false;
    assert!(files_under(&running.state()).is_empty());
    upload(&file);
    let asked = running.outside.asked();
    let restored = running.ask(json!({"op": "vault-restore", "recovery": b64(&hidden_recovery)}));
    assert_eq!(restored["vaults"], json!(1), "{restored}");
    assert_eq!(running.outside.asked(), asked + 1, "the restore was asked about");
    // It is not listed (it was hidden), has no password, and opens by its recovery key, which then sets the new way in
    assert!(running.ask(json!({"op": "status"}))["vaults"].as_array().unwrap().is_empty());
    let opened_by_key = running.ask(json!({"op": "unlock", "method": "recovery", "secret": b64(&hidden_recovery)}));
    assert_eq!(opened_by_key["ok"], json!(true), "{opened_by_key}");
    assert_eq!(running.ask(json!({"op": "change-password", "vault": hidden, "secret": b64("a new hidden password")}))["ok"], json!(true));
    assert_eq!(running.ask(json!({"op": "get", "kind": "password", "id": "h"}))["value"], json!("s3cret"));
    running.ask(json!({"op": "lock"}));
    assert_eq!(running.ask(json!({"op": "unlock", "method": "password", "secret": b64("a new hidden password")}))["ok"], json!(true));
}

/// A logo is a picture of a public site: kept at Never without the loosening question, and read without a question. Nothing else gains that.
#[test]
fn a_logo_is_kept_without_the_question_and_nothing_else_is() {
    let running = start("logo");
    running.set_up(PASSWORD);
    let before = running.outside.asked();
    let put = running.ask(json!({"op": "put", "kind": "logo", "id": "example-test", "name": "example.test", "value": "AAAA", "approval": "while-open"}));
    assert_eq!(put["ok"], json!(true), "{put}");
    let again = running.ask(json!({"op": "put", "kind": "logo", "id": "example-test", "name": "example.test", "value": "BBBB", "approval": "ask"}));
    assert_eq!(again["ok"], json!(true), "{again}");
    let got = running.ask(json!({"op": "get", "kind": "logo", "id": "example-test"}));
    assert_eq!(got["value"], json!("BBBB"), "{got}");
    let listed = running.ask(json!({"op": "list", "kind": "logo"}));
    assert_eq!(listed["items"][0]["approval"], json!("never"), "{listed}");
    assert_eq!(running.outside.asked(), before, "nobody was asked about a logo");
    // Another kind that asks to never ask is still asked about
    let other = running.ask(json!({"op": "put", "kind": "thing", "id": "t", "name": "t", "value": "x", "approval": "never"}));
    assert_eq!(other["ok"], json!(true), "{other}");
    assert_eq!(running.outside.asked(), before + 1);
}
