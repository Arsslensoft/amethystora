//! The machine's root when its key will not open: sealed by another chip, or by this service under another user number, as on a
//! machine whose service user was given another number by an update. Nothing can issue under it again, and the person is told the
//! one way out, `root rotate`, rather than to try again.

mod common;

use std::rc::Rc;
use std::sync::Arc;

use amethystora_vault_core::slots::fake::FakeChip;
use amethystora_vault_service::outside::Person;
use amethystora_vault_service::peer::Peer;
use amethystora_vault_service::service::{Config, Next, Service};
use common::{b64, fake, Folder};
use serde_json::{json, Value};

fn carry_out(service: &mut Service, peer: &Rc<Peer>, next: Next) -> Value {
    match next {
        Next::Answer(answer) => answer,
        Next::Approve { then, .. } | Next::WhenUnlocked { then } => {
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

fn service_on(folder: &Folder) -> Service {
    let (outside, handle) = fake();
    handle.said.lock().unwrap().person = Some(Person { login: "ada".into(), full_name: Some("Ada".into()) });
    Service::new(Config { state: folder.0.clone(), log_n: 10, worker: env!("CARGO_BIN_EXE_amethystora-vault-work").into() }, Arc::new(FakeChip::new()), Box::new(outside))
}

#[test]
fn a_root_whose_key_will_not_open_says_so_and_rotating_makes_one_that_does() {
    let folder = Folder::new("root-unopened");
    let peer = Rc::new(Peer { uid: 1000, pid: 1, pidfd: None });
    let mut service = service_on(&folder);
    let answer = ask(&mut service, &peer, json!({"op": "setup", "method": "password", "secret": b64("a good password")}));
    let recovery = answer["recovery"].as_str().unwrap().to_string();
    assert_eq!(ask(&mut service, &peer, json!({"op": "confirm", "recovery": b64(&recovery)}))["ok"], json!(true));
    let made = ask(&mut service, &peer, json!({"op": "cert-new", "profile": "local-tls", "title": "Printer", "hosts": ["printer.local"]}));
    assert_eq!(made["ok"], json!(true), "{made}");
    drop(service);

    // The same files, another chip
    let mut service = service_on(&folder);
    assert_eq!(ask(&mut service, &peer, json!({"op": "unlock", "method": "password", "secret": b64("a good password")}))["ok"], json!(true));
    let refused = ask(&mut service, &peer, json!({"op": "cert-new", "profile": "local-tls", "title": "NAS", "hosts": ["nas.local"]}));
    assert_eq!((refused["why"].clone(), refused["problem"].clone()), (json!("refused"), json!("root-unopened")), "{refused}");
    let rotated = ask(&mut service, &peer, json!({"op": "root-rotate"}));
    assert_eq!(rotated["ok"], json!(true), "{rotated}");
    let made = ask(&mut service, &peer, json!({"op": "cert-new", "profile": "local-tls", "title": "NAS", "hosts": ["nas.local"]}));
    assert_eq!(made["ok"], json!(true), "{made}");
}
