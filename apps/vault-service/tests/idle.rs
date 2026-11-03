//! The idle time, without waiting for it: the service's own clock passed in, its requests carried out in line.

mod common;

use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use amethystora_vault_core::slots::fake::FakeChip;
use amethystora_vault_service::peer::Peer;
use amethystora_vault_service::service::{Config, Next, Service};
use common::{b64, fake, Folder};
use serde_json::{json, Value};

fn carry_out(service: &mut Service, peer: &Rc<Peer>, next: Next) -> Value {
    match next {
        Next::Answer(answer) => answer,
        Next::Approve { then, .. } => {
            let next = then(service, peer, true);
            carry_out(service, peer, next)
        }
        Next::WhenUnlocked { then } => {
            let next = then(service, peer, true);
            carry_out(service, peer, next)
        }
        Next::Work { job, then } | Next::Slow { job, then } => {
            let next = then(service, peer, job());
            carry_out(service, peer, next)
        }
    }
}

fn ask(service: &mut Service, peer: &Rc<Peer>, request: Value) -> Value {
    let mut request = request;
    request["v"] = json!(1);
    let next = service.request(peer, request.to_string().as_bytes());
    carry_out(service, peer, next)
}

struct Fixture {
    _folder: Folder,
    service: Service,
    peer: Rc<Peer>,
}

fn open_vault() -> Fixture {
    let folder = Folder::new("idle");
    let (outside, _) = fake();
    let mut service = Service::new(Config { state: folder.0.clone(), log_n: 10, worker: env!("CARGO_BIN_EXE_amethystora-vault-work").into() }, Arc::new(FakeChip::new()), Box::new(outside));
    let peer = Rc::new(Peer { uid: 1000, pid: 1, pidfd: None });
    let answer = ask(&mut service, &peer, json!({"op": "setup", "method": "password", "secret": b64("a good password")}));
    let recovery = answer["recovery"].as_str().unwrap().to_string();
    assert_eq!(ask(&mut service, &peer, json!({"op": "confirm", "recovery": b64(&recovery)}))["ok"], json!(true));
    Fixture { _folder: folder, service, peer }
}

#[test]
fn it_closes_after_the_idle_time_and_not_before() {
    let Fixture { mut service, peer, _folder } = open_vault();
    assert!(service.is_open(1000));
    let deadline = service.deadline().unwrap();
    let left = deadline - Instant::now();
    assert!(left > Duration::from_secs(14 * 60) && left <= Duration::from_secs(15 * 60), "{left:?}");
    service.tick(deadline - Duration::from_secs(1));
    assert!(service.is_open(1000));
    service.tick(deadline);
    assert!(!service.is_open(1000));
    assert_eq!(service.deadline(), None, "nothing left to wake for");
    assert_eq!(ask(&mut service, &peer, json!({"op": "list"}))["why"], json!("closed"));
}

#[test]
fn the_setting_is_kept_and_read() {
    let Fixture { mut service, peer, _folder } = open_vault();
    assert_eq!(ask(&mut service, &peer, json!({"op": "set-idle", "minutes": 1}))["ok"], json!(true));
    assert!(service.deadline().unwrap() - Instant::now() <= Duration::from_secs(60));
    assert_eq!(ask(&mut service, &peer, json!({"op": "status"}))["idle_minutes"], json!(1));
    assert_eq!(ask(&mut service, &peer, json!({"op": "set-idle", "minutes": 0}))["why"], json!("bad-request"));
}

#[test]
fn only_what_a_person_uses_keeps_it_open() {
    let Fixture { mut service, peer, _folder } = open_vault();
    let start = service.deadline().unwrap();
    std::thread::sleep(Duration::from_millis(5));
    // A feature sealing in the background is not the person
    assert_eq!(ask(&mut service, &peer, json!({"op": "seal", "purpose": "resume", "record": "r", "data": ""}))["ok"], json!(true));
    assert_eq!(service.deadline().unwrap(), start);
    assert_eq!(ask(&mut service, &peer, json!({"op": "put", "kind": "k", "name": "n", "value": 1}))["ok"], json!(true));
    assert!(service.deadline().unwrap() > start);
}

#[test]
fn a_setup_left_waiting_is_dropped_with_its_key() {
    let folder = Folder::new("draft");
    let (outside, _) = fake();
    let mut service = Service::new(Config { state: folder.0.clone(), log_n: 10, worker: env!("CARGO_BIN_EXE_amethystora-vault-work").into() }, Arc::new(FakeChip::new()), Box::new(outside));
    let peer = Rc::new(Peer { uid: 1000, pid: 1, pidfd: None });
    let answer = ask(&mut service, &peer, json!({"op": "setup", "method": "password", "secret": b64("a good password")}));
    let recovery = answer["recovery"].as_str().unwrap().to_string();
    assert!(!service.is_quiet());
    let deadline = service.deadline().unwrap();
    service.tick(deadline);
    assert!(service.is_quiet());
    assert_eq!(ask(&mut service, &peer, json!({"op": "confirm", "recovery": b64(&recovery)}))["why"], json!("refused"));
}

#[test]
fn what_fails_keeps_nothing_open() {
    let Fixture { mut service, peer, _folder } = open_vault();
    let start = service.deadline().unwrap();
    std::thread::sleep(Duration::from_millis(5));
    // Asking for what is not there, or for what is refused, is no use of the vault: a program could ask for ever
    for request in [
        json!({"op": "get", "kind": "k", "id": "nothing"}),
        json!({"op": "remove", "kind": "k", "id": "nothing"}),
        json!({"op": "set-approval", "kind": "k", "id": "nothing", "approval": "ask"}),
        json!({"op": "get", "kind": "data-key", "id": "resume"}),
        json!({"op": "put", "kind": "data-key", "name": "n", "value": 1}),
        json!({"op": "rename", "vault": "0".repeat(32), "name": "x"}),
        json!({"op": "put", "vault": "0".repeat(32), "kind": "k", "name": "n", "value": 1}),
    ] {
        assert_ne!(ask(&mut service, &peer, request.clone())["ok"], json!(true), "{request}");
        assert_eq!(service.deadline().unwrap(), start, "{request}");
    }
    // What succeeded is use
    assert_eq!(ask(&mut service, &peer, json!({"op": "list"}))["ok"], json!(true));
    assert!(service.deadline().unwrap() > start);
}
