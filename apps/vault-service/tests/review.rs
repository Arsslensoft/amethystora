//! What the security review of the core found (plans/vault/work/security-core.md), each as a test that fails without its fix:
//! a stranger's signal, an idle connection, a guess through the socket, an item written into the wrong vault, a prompt left up
//! over a lock, and the rest. The fake outside answers every prompt with yes unless a test says otherwise.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use amethystora_vault_client::{exchange, forget_at, open_at, seal_at, Refusal};
use amethystora_vault_core::slots::{self, RecoveryKey, Table};
use amethystora_vault_core::vaults::Account;
use amethystora_vault_core::{protocol, seal};
use amethystora_vault_service::outside::Event;
use common::{b64, start, Running};
use serde_json::{json, Value};

const PASSWORD: &str = "correct horse battery";
const OTHER: &str = "another password here";

fn me() -> u32 {
    // SAFETY: getuid cannot fail
    unsafe { libc::getuid() }
}

fn lock(running: &Running) {
    assert_eq!(running.ask(json!({"op": "lock"}))["ok"], json!(true));
}

fn unlock(running: &Running, password: &str) -> Value {
    running.ask(json!({"op": "unlock", "method": "password", "secret": b64(password)}))
}

fn first_vault(running: &Running) -> String {
    running.ask(json!({"op": "status"}))["vaults"][0]["id"].as_str().unwrap().to_string()
}

fn refuse(running: &Running, refuse: bool) {
    running.outside.said.lock().unwrap().refuse = refuse;
}

fn why(answer: &Value) -> &str {
    answer["why"].as_str().unwrap_or("")
}

fn tries_file(running: &Running) -> std::path::PathBuf {
    running.state().join(me().to_string()).join("tries")
}

// --- Guessing through the socket -------------------------------------------------------------------------------------------

#[test]
fn what_cannot_be_a_password_is_refused_and_never_counted() {
    let running = start("unusable");
    running.set_up(PASSWORD);
    lock(&running);
    let asked = running.outside.asked();
    // Too short, not text: no password that can be set looks like these, and each costs nothing to send
    for _ in 0..20 {
        for secret in [b64("short"), protocol::encode(&[0xff, 0xfe, 0xfd, 0xfc, 0xfb, 0xfa, 0xf9, 0xf8, 0xf7])] {
            assert_eq!(why(&running.ask(json!({"op": "unlock", "method": "password", "secret": secret}))), "refused");
        }
    }
    assert!(!tries_file(&running).exists(), "nothing was counted");
    // So nothing was held against the person, who is not asked and not made to wait
    assert_eq!(unlock(&running, PASSWORD)["ok"], json!(true));
    assert_eq!(running.outside.asked(), asked);
}

#[test]
fn a_right_password_to_a_vault_of_its_own_does_not_give_the_tries_back() {
    let running = start("tries-back");
    running.set_up(PASSWORD);
    running.new_vault("Program's own", true, OTHER);
    lock(&running);
    let asked = running.outside.asked();
    // Four wrong, the other vault's right one, four wrong: eight wrong in all
    for _ in 0..4 {
        assert_eq!(why(&unlock(&running, "not the password")), "refused");
    }
    // The other vault's password is right and opens it, and is the fifth try: it takes nothing back
    assert_eq!(unlock(&running, OTHER)["ok"], json!(true));
    lock(&running);
    assert_eq!(why(&unlock(&running, "not the password")), "refused");
    assert_eq!(running.outside.asked(), asked, "the first five were free");
    // The next is put to the person, whoever's password it is: five wrong ones, and the right one in between gave nothing back
    refuse(&running, true);
    assert_eq!(why(&unlock(&running, OTHER)), "refused");
    assert_eq!(running.outside.asked(), asked + 1);
}

#[test]
fn a_clock_that_was_wrong_holds_nobody_out() {
    let running = start("clock");
    running.set_up(PASSWORD);
    lock(&running);
    let asked = running.outside.asked();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    // Nine wrong tries counted from ten years ahead, and from two hours ago: neither is a window that is open now
    for since in [now + 10 * 365 * 86400, now - 7200] {
        std::fs::write(tries_file(&running), format!(r#"{{"fails":9,"since":{since}}}"#)).unwrap();
        assert_eq!(unlock(&running, PASSWORD)["ok"], json!(true));
        lock(&running);
    }
    assert_eq!(running.outside.asked(), asked);
}

#[test]
fn changing_a_password_is_not_a_free_way_to_ask_whether_another_vault_has_it() {
    let running = start("oracle");
    running.set_up(PASSWORD);
    running.new_vault("Hidden", true, OTHER);
    let first = first_vault(&running);
    let asked = running.outside.asked();
    // A program that knows the current password asks whether a new one opens another vault: each is put to the person first
    refuse(&running, true);
    for guess in [OTHER, "a password nobody has"] {
        let answer = running.ask(json!({"op": "change-password", "vault": first, "current": b64(PASSWORD), "secret": b64(guess)}));
        assert_eq!(why(&answer), "refused", "{guess}");
    }
    assert_eq!(running.outside.asked(), asked + 2);
    // And a person who says yes is told no for the one that opens another vault, as for a short one
    refuse(&running, false);
    let used = running.ask(json!({"op": "change-password", "vault": first, "current": b64(PASSWORD), "secret": b64(OTHER)}));
    let short = running.ask(json!({"op": "change-password", "vault": first, "current": b64(PASSWORD), "secret": b64("short")}));
    assert_eq!((why(&used), why(&short)), ("refused", "refused"));
    assert_eq!(running.ask(json!({"op": "change-password", "vault": first, "current": b64(PASSWORD), "secret": b64("a new password")}))["ok"], json!(true));
}

// --- Items and approvals ---------------------------------------------------------------------------------------------------

#[test]
fn an_item_is_never_written_into_another_vault_than_the_one_that_holds_it() {
    let running = start("shadow");
    running.set_up(PASSWORD);
    running.new_vault("Second", false, OTHER);
    let status = running.ask(json!({"op": "status"}));
    let ids: Vec<String> = status["vaults"].as_array().unwrap().iter().map(|vault| vault["id"].as_str().unwrap().to_string()).collect();
    let (first, second) = (&ids[0], &ids[1]);
    let put = |vault: &str, value: &str| running.ask(json!({"op": "put", "vault": vault, "kind": "k", "id": "a", "name": "n", "value": value, "approval": "ask"}));
    assert_eq!(put(first, "first")["ok"], json!(true));
    // The same name in the second vault would be found first by `get`: refused, and the first is as it was
    assert_eq!(why(&put(second, "shadow")), "refused");
    assert_eq!(running.ask(json!({"op": "get", "kind": "k", "id": "a"}))["value"], json!("first"));
    // Writing over it where it is asks, as `Ask every time` says, and a no leaves it
    let asked = running.outside.asked();
    refuse(&running, true);
    assert_eq!(why(&put(first, "overwritten")), "refused");
    assert_eq!(running.outside.asked(), asked + 1);
    refuse(&running, false);
    assert_eq!(running.ask(json!({"op": "get", "kind": "k", "id": "a"}))["value"], json!("first"));
    // And lowering how often it asks is asked about with the right item, not found in the other vault
    let lowered = running.ask(json!({"op": "put", "vault": second, "kind": "k", "id": "a", "name": "n", "value": "x", "approval": "never"}));
    assert_eq!(why(&lowered), "refused");
}

#[test]
fn an_approval_does_not_outlive_its_item() {
    let running = start("approved");
    running.set_up(PASSWORD);
    let get = || running.ask(json!({"op": "get", "kind": "k", "id": "x"}));
    running.ask(json!({"op": "put", "kind": "k", "id": "x", "name": "n", "value": 1}));
    let asked = running.outside.asked();
    assert_eq!(get()["value"], json!(1));
    assert_eq!(get()["value"], json!(1));
    assert_eq!(running.outside.asked(), asked + 1, "once while the vault is open");
    assert_eq!(running.ask(json!({"op": "remove", "kind": "k", "id": "x"}))["ok"], json!(true));
    // A new item by the same name is another: the person was not asked about it
    running.ask(json!({"op": "put", "kind": "k", "id": "x", "name": "n", "value": 2}));
    let asked = running.outside.asked();
    assert_eq!(get()["value"], json!(2));
    assert_eq!(running.outside.asked(), asked + 1);
}

#[test]
fn the_vault_s_own_labels_cannot_be_kinds() {
    let running = start("labels");
    running.set_up(PASSWORD);
    for kind in ["meta", "index", "data", "slot", "chip"] {
        let answer = running.ask(json!({"op": "put", "kind": kind, "id": first_vault(&running), "name": "n", "value": 1}));
        assert_eq!(why(&answer), "bad-request", "{kind}");
    }
}

#[test]
fn a_line_put_back_from_before_is_noticed_in_the_index_too() {
    let running = start("rollback");
    running.set_up(PASSWORD);
    running.ask(json!({"op": "put", "kind": "k", "id": "a", "name": "n", "value": "old", "approval": "never"}));
    lock(&running);
    let id = first_vault(&running);
    let index = running.state().join(me().to_string()).join("vaults").join(&id).join("index");
    let old_index = std::fs::read(&index).unwrap();
    assert_eq!(unlock(&running, PASSWORD)["ok"], json!(true));
    for value in ["new", "newer", "newest"] {
        running.ask(json!({"op": "put", "kind": "k", "id": "a", "name": "n", "value": value}));
    }
    lock(&running);
    // The index as it was three writes ago, with the item as it is now: not a crash between two writes
    std::fs::write(&index, old_index).unwrap();
    assert_eq!(unlock(&running, PASSWORD)["ok"], json!(true));
    let answer = running.ask(json!({"op": "get", "kind": "k", "id": "a"}));
    assert_eq!((answer["ok"].clone(), answer["stale"].clone()), (json!(false), json!(true)), "{answer}");
}

// --- Opening and closing ---------------------------------------------------------------------------------------------------

#[test]
fn nothing_opens_while_the_screen_is_locked_whatever_the_way_in() {
    let running = start("locked-opens");
    let recovery = running.set_up(PASSWORD);
    assert_eq!(running.ask(json!({"op": "add-slot", "kind": "approval"}))["ok"], json!(true));
    assert_eq!(running.ask(json!({"op": "add-slot", "kind": "no-prompt"}))["ok"], json!(true));
    lock(&running);
    running.outside.said.lock().unwrap().locked = true;
    let open = || running.ask(json!({"op": "status"}))["vaults"][0]["open"].clone();
    for request in [
        json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}),
        json!({"op": "unlock", "method": "recovery", "secret": b64(&recovery)}),
        json!({"op": "unlock", "method": "approval"}),
        json!({"op": "unlock", "method": "no-prompt"}),
    ] {
        assert_eq!(why(&running.ask(request.clone())), "closed", "{request}");
        assert_eq!(open(), json!(false), "{request}");
    }
    running.outside.said.lock().unwrap().locked = false;
    assert_eq!(unlock(&running, PASSWORD)["ok"], json!(true));
}

#[test]
fn a_setup_the_screen_locked_on_is_dropped_and_cannot_be_confirmed_behind_the_lock() {
    let running = start("locked-setup");
    running.outside.said.lock().unwrap().silent = true;
    let socket = running.socket.clone();
    let setting_up = std::thread::spawn(move || exchange(&socket, &json!({"v": 1, "op": "setup", "method": "password", "secret": b64(PASSWORD)}), Duration::from_secs(20)));
    running.wait_until(|said| !said.asked.is_empty());
    running.outside.push(Event::Locked(me()));
    assert_eq!(why(&setting_up.join().unwrap().unwrap()), "closed");
    running.outside.said.lock().unwrap().silent = false;
    assert_eq!(running.ask(json!({"op": "status"}))["set_up"], json!(false), "nothing was kept");
    // And one that was made before the lock cannot be typed back behind it
    let answer = running.ask(json!({"op": "setup", "method": "password", "secret": b64(PASSWORD)}));
    let recovery = answer["recovery"].as_str().unwrap().to_string();
    running.outside.said.lock().unwrap().locked = true;
    assert_eq!(why(&running.ask(json!({"op": "confirm", "recovery": b64(&recovery)}))), "closed");
    assert_eq!(running.ask(json!({"op": "status"}))["set_up"], json!(false));
}

#[test]
fn two_setups_approved_one_after_the_other_make_one_vault() {
    let running = start("two-setups");
    running.outside.said.lock().unwrap().silent = true;
    let socket = running.socket.clone();
    let second = std::thread::spawn(move || exchange(&socket, &json!({"v": 1, "op": "setup", "method": "password", "name": "Second", "secret": b64("second setup password")}), Duration::from_secs(20)));
    running.wait_until(|said| !said.asked.is_empty());
    running.outside.said.lock().unwrap().silent = false;
    running.set_up(PASSWORD);
    for token in 1..50u64 {
        running.outside.push(Event::Approved(token, true));
    }
    let answer = second.join().unwrap().unwrap();
    if answer["ok"] == json!(true) {
        let recovery = answer["recovery"].as_str().unwrap();
        assert_eq!(why(&running.ask(json!({"op": "confirm", "recovery": b64(recovery)}))), "refused");
    }
    let status = running.ask(json!({"op": "status"}));
    let firsts = status["vaults"].as_array().unwrap().iter().filter(|vault| vault["first"] == json!(true)).count();
    assert_eq!((status["vaults"].as_array().unwrap().len(), firsts), (1, 1));
}

#[test]
fn a_lock_takes_back_a_prompt_and_what_it_was_for() {
    let running = start("lock-prompt");
    running.set_up(PASSWORD);
    running.outside.said.lock().unwrap().silent = true;
    let secret = protocol::encode(&[5u8; 32]);
    let credential = protocol::encode(b"credential");
    let socket = running.socket.clone();
    let (s, c) = (secret.clone(), credential.clone());
    let waiting = std::thread::spawn(move || exchange(&socket, &json!({"v": 1, "op": "add-security-key", "name": "Program's", "credential": c.clone(), "salt": c, "secret": s}), Duration::from_secs(10)));
    running.wait_until(|said| said.asked.len() >= 2);
    lock(&running);
    // The prompt is cancelled, whoever asked hears so, and a yes on a screen that still shows it is nobody's
    assert_eq!(why(&waiting.join().unwrap().unwrap()), "closed");
    assert_eq!(running.outside.said.lock().unwrap().cancelled.len(), 1);
    for token in 1..200 {
        running.outside.push(Event::Approved(token, true));
    }
    running.outside.said.lock().unwrap().silent = false;
    assert_eq!(unlock(&running, PASSWORD)["ok"], json!(true));
    assert_eq!(running.ask(json!({"op": "methods"}))["vaults"][0]["slots"], json!([]));
}

#[test]
fn a_change_of_password_the_screen_locked_on_changes_nothing() {
    let running = start("lock-change");
    running.set_up(PASSWORD);
    running.new_vault("Second", false, "second password here");
    running.outside.said.lock().unwrap().silent = true;
    let socket = running.socket.clone();
    let changing = std::thread::spawn(move || exchange(&socket, &json!({"v": 1, "op": "change-password", "current": b64("second password here"), "secret": b64("changed password here")}), Duration::from_secs(20)));
    running.wait_until(|said| said.asked.len() >= 3);
    running.outside.push(Event::Locked(me()));
    assert_eq!(why(&changing.join().unwrap().unwrap()), "closed");
    running.outside.said.lock().unwrap().silent = false;
    assert_eq!(why(&unlock(&running, "changed password here")), "refused");
    assert_eq!(unlock(&running, "second password here")["ok"], json!(true));
    let account: Value = serde_json::from_slice(&std::fs::read(running.state().join(me().to_string()).join("account")).unwrap()).unwrap();
    assert_eq!(account["passwords"].as_array().unwrap().len(), 2);
}

#[test]
fn keeping_the_vault_open_for_longer_is_the_person_s_to_say() {
    let running = start("idle-raise");
    running.set_up(PASSWORD);
    let idle = || running.ask(json!({"op": "status"}))["idle_minutes"].clone();
    let asked = running.outside.asked();
    refuse(&running, true);
    assert_eq!(why(&running.ask(json!({"op": "set-idle", "minutes": 1440}))), "refused");
    assert_eq!((idle(), running.outside.asked()), (json!(15), asked + 1));
    refuse(&running, false);
    assert_eq!(running.ask(json!({"op": "set-idle", "minutes": 30}))["ok"], json!(true));
    assert_eq!(idle(), json!(30));
    // Shorter is nobody's risk
    let asked = running.outside.asked();
    assert_eq!(running.ask(json!({"op": "set-idle", "minutes": 5}))["ok"], json!(true));
    assert_eq!(running.outside.asked(), asked);
}

#[test]
fn forgetting_a_key_loses_every_file_under_it_so_it_asks() {
    let running = start("forget-asks");
    running.set_up(PASSWORD);
    let socket = running.socket.clone();
    let sealed = seal_at(&socket, "resume", "r", b"plain").unwrap();
    refuse(&running, true);
    assert_eq!(forget_at(&socket, "resume"), Err(Refusal::Refused));
    assert_eq!(open_at(&socket, "resume", "r", &sealed).unwrap(), b"plain");
    // The key the chip also holds is no different
    refuse(&running, false);
    assert_eq!(running.ask(json!({"op": "background", "on": true}))["ok"], json!(true));
    let background = seal_at(&socket, "background", "r", b"plain").unwrap();
    refuse(&running, true);
    assert_eq!(forget_at(&socket, "background"), Err(Refusal::Refused));
    assert_eq!(open_at(&socket, "background", "r", &background).unwrap(), b"plain");
    refuse(&running, false);
    // A purpose with no key has nothing to lose: no prompt
    let asked = running.outside.asked();
    forget_at(&socket, "never-made").unwrap();
    assert_eq!(running.outside.asked(), asked);
    forget_at(&socket, "background").unwrap();
    assert_eq!(open_at(&socket, "background", "r", &background), Err(Refusal::Refused));
}

#[test]
fn a_program_the_person_keeps_refusing_is_not_put_in_front_of_them_again() {
    let running = start("refusals");
    running.set_up(PASSWORD);
    running.ask(json!({"op": "put", "kind": "k", "id": "x", "name": "n", "value": 1, "approval": "ask"}));
    refuse(&running, true);
    let asked = running.outside.asked();
    for count in 1..=3 {
        assert_eq!(why(&running.ask(json!({"op": "get", "kind": "k", "id": "x"}))), "refused");
        assert_eq!(running.outside.asked(), asked + count);
    }
    // The fourth is not asked, and a yes that comes after is not asked either, for a minute
    refuse(&running, false);
    let started = Instant::now();
    for _ in 0..50 {
        assert_eq!(why(&running.ask(json!({"op": "get", "kind": "k", "id": "x"}))), "refused");
    }
    assert_eq!(running.outside.asked(), asked + 3);
    assert!(started.elapsed() < Duration::from_secs(10));
}

// --- Room ------------------------------------------------------------------------------------------------------------------

#[test]
fn a_program_can_make_only_so_many_keys() {
    let running = start("purposes");
    running.set_up(PASSWORD);
    let socket = running.socket.clone();
    for at in 0..store_most() {
        seal_at(&socket, &format!("purpose-{at}"), "r", b"x").unwrap();
    }
    assert_eq!(seal_at(&socket, "one-more", "r", b"x"), Err(Refusal::TooLarge));
    // Those made still work, and so does a person's own item
    seal_at(&socket, "purpose-0", "r", b"x").unwrap();
    assert_eq!(running.ask(json!({"op": "put", "kind": "k", "id": "a", "name": "n", "value": 1}))["ok"], json!(true));
    lock(&running);
    assert_eq!(unlock(&running, PASSWORD)["ok"], json!(true), "the vault still opens");
}

fn store_most() -> usize {
    amethystora_vault_core::store::MAX_PURPOSES
}

#[test]
fn an_account_has_a_quota_of_disk_in_all() {
    let running = start("quota");
    running.set_up(PASSWORD);
    let value = "x".repeat(1_000_000);
    let mut written = 0;
    let refused = loop {
        let answer = running.ask(json!({"op": "put", "kind": "k", "name": "n", "value": value, "approval": "never"}));
        if answer["ok"] != json!(true) {
            break answer;
        }
        written += 1;
        assert!(written < 400, "no end to it");
    };
    assert_eq!(why(&refused), "too-large");
    let used = disk_use(&running.state().join(me().to_string()));
    assert!(used <= amethystora_vault_core::vaults::QUOTA, "{used}");
    assert!(written > 200, "{written}");
    // Taking an item away makes room for another, and what is already kept still reads
    let items = running.ask(json!({"op": "list", "kind": "k"}));
    let id = items["items"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(running.ask(json!({"op": "remove", "kind": "k", "id": id}))["ok"], json!(true));
    assert_eq!(running.ask(json!({"op": "put", "kind": "k", "name": "n", "value": value, "approval": "never"}))["ok"], json!(true));
    lock(&running);
    assert_eq!(unlock(&running, PASSWORD)["ok"], json!(true));
    // The quota is counted again from the disk when the vault opens: still full
    assert_eq!(why(&running.ask(json!({"op": "put", "kind": "k", "name": "n", "value": value, "approval": "never"}))), "too-large");
}

fn disk_use(folder: &std::path::Path) -> u64 {
    common::files_under(folder).iter().map(|file| std::fs::metadata(file).map(|meta| meta.len()).unwrap_or(0)).sum()
}

// --- Connections -----------------------------------------------------------------------------------------------------------

#[test]
fn a_connection_that_asks_nothing_is_let_go() {
    let running = start("idle-connections");
    let mut held: Vec<UnixStream> = (0..16).map(|_| UnixStream::connect(&running.socket).unwrap()).collect();
    // One that began a line and stopped
    held[0].write_all(b"{\"v\":1,\"op\":").unwrap();
    std::thread::sleep(Duration::from_secs(10));
    for (at, stream) in held.iter_mut().enumerate() {
        stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let mut byte = [0u8; 1];
        let read = stream.read(&mut byte);
        assert!(matches!(read, Ok(0)), "connection {at} was not hung up on: {read:?}");
    }
    // And there is room again for the account
    assert_eq!(running.ask(json!({"op": "status"}))["ok"], json!(true));
}

#[test]
fn an_account_with_no_session_is_not_served() {
    let running = start("no-session");
    running.outside.said.lock().unwrap().no_session.push(me());
    let mut stream = UnixStream::connect(&running.socket).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let _ = stream.write_all(b"{\"v\":1,\"op\":\"status\"}\n");
    let mut answer = String::new();
    let read = BufReader::new(&stream).read_line(&mut answer).unwrap_or(0);
    assert_eq!(read, 0, "{answer}");
    running.outside.said.lock().unwrap().no_session.clear();
    assert_eq!(running.ask(json!({"op": "status"}))["ok"], json!(true));
}

/// Round 7's unattended work (plans/done/PLAN-ui-ideas-r7-build.md, Keys): a backup timer at night runs as an account that is
/// logged out and lingers, and takes its `background` key from the chip. That account is served that and nothing else.
#[test]
fn an_account_logged_out_and_lingering_is_served_its_background_key_and_nothing_else() {
    let running = start("lingering");
    running.set_up(PASSWORD);
    let socket = running.socket.clone();
    assert_eq!(running.ask(json!({"op": "background", "on": true}))["ok"], json!(true));
    let sealed = seal_at(&socket, "background", "daily", b"plain").unwrap();
    let other = seal_at(&socket, "resume", "r", b"x").unwrap();
    running.ask(json!({"op": "lock"}));
    {
        let mut said = running.outside.said.lock().unwrap();
        said.no_session.push(me());
        said.lingering.push(me());
    }
    let asked = running.outside.asked();
    // The night's work: the key, both ways, with the vault closed and nobody there
    assert_eq!(open_at(&socket, "background", "daily", &sealed).unwrap(), b"plain");
    let more = seal_at(&socket, "background", "nightly", b"more").unwrap();
    assert_eq!(open_at(&socket, "background", "nightly", &more).unwrap(), b"more");
    // Nothing else: not another purpose, not the vault, not an item, not removing a key, and nobody is asked
    assert_eq!(open_at(&socket, "resume", "r", &other), Err(Refusal::Closed));
    assert_eq!(forget_at(&socket, "background"), Err(Refusal::Closed));
    for request in [
        json!({"op": "status"}),
        json!({"op": "unlock", "method": "password", "secret": b64(PASSWORD)}),
        json!({"op": "list"}),
        json!({"op": "get", "kind": "k", "id": "x"}),
        json!({"op": "background", "on": false}),
        json!({"op": "password-generate"}),
    ] {
        let answer = running.ask(request.clone());
        assert_eq!(answer["why"], json!("closed"), "{request}: {answer}");
    }
    assert_eq!(running.outside.asked(), asked);
    // An account that neither has a session nor lingers is not served at all
    running.outside.said.lock().unwrap().lingering.clear();
    let mut stream = UnixStream::connect(&running.socket).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let _ = stream.write_all(b"{\"v\":1,\"op\":\"status\"}\n");
    let mut answer = String::new();
    assert_eq!(BufReader::new(&stream).read_line(&mut answer).unwrap_or(0), 0, "{answer}");
}

/// Run by the test below, as another uid: sets its own vault up and seals a megabyte at a time on fourteen connections
#[test]
#[ignore = "the flooding account's half of one_account_cannot_hold_up_another"]
fn flooding_account() {
    let socket = std::path::PathBuf::from(std::env::var("OTHER_SOCKET").unwrap());
    let ask = |request: Value| exchange(&socket, &request, Duration::from_secs(30)).unwrap();
    let answer = ask(json!({"v": 1, "op": "setup", "method": "password", "secret": b64(PASSWORD)}));
    let recovery = answer["recovery"].as_str().unwrap().to_string();
    assert_eq!(ask(json!({"v": 1, "op": "confirm", "recovery": b64(&recovery)}))["ok"], json!(true));
    let line = format!("{{\"v\":1,\"op\":\"seal\",\"purpose\":\"a\",\"record\":\"r\",\"data\":\"{}\"}}\n", protocol::encode(&vec![1u8; 1024 * 1024]));
    let until = Instant::now() + Duration::from_secs(4);
    let workers: Vec<_> = (0..14)
        .map(|_| {
            let (socket, line) = (socket.clone(), line.clone());
            std::thread::spawn(move || {
                let stream = UnixStream::connect(&socket).unwrap();
                let mut writer = stream.try_clone().unwrap();
                let mut reader = BufReader::new(stream);
                while Instant::now() < until {
                    if writer.write_all(line.as_bytes()).is_err() {
                        return;
                    }
                    let mut answer = String::new();
                    if reader.read_line(&mut answer).unwrap_or(0) == 0 {
                        return;
                    }
                }
            })
        })
        .collect();
    for worker in workers {
        let _ = worker.join();
    }
}

#[test]
fn one_account_cannot_hold_up_another() {
    // SAFETY: getuid cannot fail
    if unsafe { libc::getuid() } != 0 {
        eprintln!("skipped: needs root, to run the other half as another account");
        return;
    }
    let running = start("fairness");
    running.set_up(PASSWORD);
    use std::os::unix::process::CommandExt;
    let mut flooder = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "flooding_account", "--nocapture"])
        .env("OTHER_SOCKET", &running.socket)
        .uid(1234)
        .gid(1234)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    let stream = UnixStream::connect(&running.socket).unwrap();
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    let mut times = Vec::new();
    for _ in 0..30 {
        let started = Instant::now();
        writer.write_all(b"{\"v\":1,\"op\":\"status\"}\n").unwrap();
        let mut answer = String::new();
        reader.read_line(&mut answer).unwrap();
        times.push(started.elapsed());
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = flooder.wait();
    times.sort();
    println!("status while another account seals megabytes: median {:?}, worst {:?}", times[15], times[29]);
    // On a machine this busy a thread can lose the processor for a second, so not the worst: before, the median was 120 ms
    assert!(times[15] < Duration::from_millis(30), "{times:?}");
    assert!(times[22] < Duration::from_millis(150), "{times:?}");
}

// --- Memory ----------------------------------------------------------------------------------------------------------------

const MASK: u8 = 0x5a;

/// Where in this process's memory these bytes are, given masked so that this process does not hold them itself
fn scan(masked: &[u8]) -> Vec<String> {
    use std::os::unix::fs::FileExt;
    let maps = std::fs::read_to_string("/proc/self/maps").unwrap();
    let mem = std::fs::File::open("/proc/self/mem").unwrap();
    let mut hits = Vec::new();
    for line in maps.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if !fields[1].starts_with("rw") || line.contains("[vvar") || line.contains("[vsys") {
            continue;
        }
        let (from, to) = fields[0].split_once('-').unwrap();
        let (from, to) = (u64::from_str_radix(from, 16).unwrap(), u64::from_str_radix(to, 16).unwrap());
        if to - from > 1 << 30 {
            continue;
        }
        let mut bytes = vec![0u8; (to - from) as usize];
        if mem.read_exact_at(&mut bytes, from).is_err() {
            continue;
        }
        for at in 0..bytes.len().saturating_sub(masked.len()) {
            if bytes[at] ^ MASK == masked[0] && bytes[at..at + masked.len()].iter().zip(masked).all(|(b, m)| b ^ MASK == *m) {
                hits.push(format!("{:x} in {} {}", from + at as u64, fields[0], fields.get(5).unwrap_or(&"(anonymous)")));
            }
        }
    }
    hits
}

fn send_masked(fd: i32, bytes: &[u8]) {
    let mut out = Vec::with_capacity(bytes.len() + 4);
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend(bytes.iter().map(|b| b ^ MASK));
    // SAFETY: a live buffer
    assert_eq!(unsafe { libc::write(fd, out.as_ptr().cast(), out.len()) }, out.len() as isize);
}

fn read_masked(fd: i32) -> Vec<u8> {
    let mut length = [0u8; 4];
    // SAFETY: live buffers
    assert_eq!(unsafe { libc::read(fd, length.as_mut_ptr().cast(), 4) }, 4);
    let mut bytes = vec![0u8; u32::from_le_bytes(length) as usize];
    let mut got = 0;
    while got < bytes.len() {
        // SAFETY: as above
        let read = unsafe { libc::read(fd, bytes[got..].as_mut_ptr().cast(), bytes.len() - got) };
        assert!(read > 0);
        got += read as usize;
    }
    bytes
}

#[test]
fn a_password_does_not_stay_in_the_service_after_a_lock_with_the_connection_still_up() {
    let running = start("heap");
    let (mut to_parent, mut to_child) = ([0; 2], [0; 2]);
    // SAFETY: pipes for the child below
    unsafe {
        assert_eq!(libc::pipe(to_parent.as_mut_ptr()), 0);
        assert_eq!(libc::pipe(to_child.as_mut_ptr()), 0);
    }
    // SAFETY: the child uses only its own memory, a socket and pipes, and leaves with _exit
    let child = unsafe { libc::fork() };
    if child == 0 {
        let password = format!("pw-{}", seal::random_hex(24));
        let value = format!("value-{}", seal::random_hex(24));
        let stream = UnixStream::connect(&running.socket).unwrap();
        let mut writer = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream);
        let mut ask = |request: Value| {
            writer.write_all(format!("{request}\n").as_bytes()).unwrap();
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            serde_json::from_str::<Value>(&line).unwrap()
        };
        let answer = ask(json!({"v": 1, "op": "setup", "method": "password", "secret": b64(&password)}));
        let recovery = answer["recovery"].as_str().unwrap().to_string();
        assert_eq!(ask(json!({"v": 1, "op": "confirm", "recovery": b64(&recovery)}))["ok"], json!(true));
        ask(json!({"v": 1, "op": "put", "kind": "k", "id": "x", "name": "n", "value": value, "approval": "never"}));
        ask(json!({"v": 1, "op": "get", "kind": "k", "id": "x"}));
        ask(json!({"v": 1, "op": "lock"}));
        // A new opening on this one connection, as a window's would be, then a lock
        assert_eq!(ask(json!({"v": 1, "op": "unlock", "method": "password", "secret": b64(&password)}))["ok"], json!(true));
        ask(json!({"v": 1, "op": "get", "kind": "k", "id": "x"}));
        ask(json!({"v": 1, "op": "lock"}));
        ask(json!({"v": 1, "op": "status"}));
        let account = Account::load(&running.state(), me()).unwrap().unwrap();
        let typed = RecoveryKey::parse(&recovery).unwrap();
        let found = slots::search(&typed.key(), &account.records(Table::Recovery), Table::Recovery);
        send_masked(to_parent[1], found[0].key.bytes());
        send_masked(to_parent[1], b64(&password).as_bytes());
        send_masked(to_parent[1], value.as_bytes());
        send_masked(to_parent[1], recovery.as_bytes());
        // Keeps the connection until told, as a window does
        let mut byte = [0u8; 1];
        // SAFETY: a live buffer
        unsafe {
            libc::read(to_child[0], byte.as_mut_ptr().cast(), 1);
            libc::_exit(0);
        }
    }
    let (key, password, value, recovery) = (read_masked(to_parent[0]), read_masked(to_parent[0]), read_masked(to_parent[0]), read_masked(to_parent[0]));
    std::thread::sleep(Duration::from_millis(200));
    let found = [("the vault key", scan(&key)), ("the password", scan(&password)), ("the password's last bytes", scan(&password[password.len() - 16..])), ("the item", scan(&value)), ("the recovery key", scan(&recovery))];
    // SAFETY: a live buffer, and the child this test made
    unsafe {
        libc::write(to_child[1], [1u8].as_ptr().cast(), 1);
        let mut status = 0;
        libc::waitpid(child, &mut status, 0);
    }
    for (what, hits) in found {
        assert!(hits.is_empty(), "{what} is still in the service: {hits:?}");
    }
}
