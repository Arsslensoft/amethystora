//! Cryptographic keys. Made, read and exported in the job process, never here: this only keeps them, sealed, and says what the
//! public forms were when the key was made, so that showing them takes no libcrypto.

use std::rc::Rc;

use serde::Deserialize;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use amethystora_vault_core::protocol::{encode, valid_name, Approval, Refusal, KIND_KEY};
use amethystora_vault_core::store::Trouble;

use super::{approval_of, base64_field, extra_of, flag, item_id, meta_fits, must_be_open, name_of, optional_text, passphrase, read, refuse_with, text, vault_of, work};
use crate::outside::{say, Action};
use crate::peer::Peer;
use crate::service::{Next, Service};

/// More than the largest key any type here has
const MOST_KEY: usize = 64 * 1024;

/// A key as it is sealed. It prints nothing.
#[derive(Deserialize)]
pub struct Held {
    pub key_kind: String,
    #[serde(rename = "type")]
    pub key_type: String,
    #[serde(deserialize_with = "wiping")]
    pub key: Zeroizing<String>,
    pub exportable: bool,
    #[serde(default)]
    pub public: Value,
}

fn wiping<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Zeroizing<String>, D::Error> {
    String::deserialize(deserializer).map(Zeroizing::new)
}

/// A key the SSH agent is about to use, once the item's setting is satisfied: the fields its job takes. Only a key made or
/// imported for SSH: a signing key's signature must never be had through the agent.
pub fn read_key(service: &mut Service, uid: u32, vault: &str, id: &str) -> Result<Held, Next> {
    let held: Held = read(service, uid, vault, KIND_KEY, id)?;
    if held.key_kind != "ssh" {
        return Err(Next::refuse(Refusal::Refused));
    }
    Ok(held)
}

fn meta_from(key_kind: &str, key_type: &str, exportable: bool, public: &Value, comment: Option<&str>) -> Value {
    let mut meta = json!({"key_kind": key_kind, "type": key_type, "exportable": exportable});
    for field in ["sha256", "ssh_sha256", "openssh"] {
        if let Some(value) = public.get(field).filter(|value| !value.is_null()) {
            meta[field] = value.clone();
        }
    }
    if let Some(comment) = comment {
        meta["comment"] = json!(comment);
    }
    meta
}

struct Keeping {
    target: String,
    name: String,
    exportable: bool,
    approval: Approval,
    comment: Option<String>,
}

/// What a job made, kept: its answer to the client holds the public forms and never the key
fn keep(service: &mut Service, uid: u32, keeping: &Keeping, made: &Value) -> Next {
    if work::refused(made) {
        return Next::Answer(made.clone());
    }
    let (Some(key_kind), Some(key_type), Some(key)) = (made["key_kind"].as_str(), made["type"].as_str(), made["key"].as_str()) else { return Next::refuse(Refusal::Failed) };
    let meta = meta_from(key_kind, key_type, keeping.exportable, &made["public"], keeping.comment.as_deref());
    if !meta_fits(&meta) {
        return Next::refuse(Refusal::TooLarge);
    }
    let id = amethystora_vault_core::seal::random_hex(8);
    let value = json!({"key_kind": key_kind, "type": key_type, "key": key, "exportable": keeping.exportable, "public": made["public"], "comment": keeping.comment});
    // The account's quota is `write_in`'s, and the room for one more item the store's
    let extra = extra_of(&keeping.name, &meta, &value);
    match service.write_in(uid, &keeping.target, KIND_KEY, &id, extra, |store| store.put(KIND_KEY, &id, &keeping.name, &meta, keeping.approval, &value)) {
        Ok(_) => {
            service.touch(uid);
            Next::Answer(json!({"ok": true, "id": id, "vault": keeping.target, "type": key_type, "public": made["public"]}))
        }
        // The vault closed while the job ran
        Err(Trouble::NotFound) => Next::refuse(Refusal::Closed),
        Err(trouble) => Next::trouble(trouble),
    }
}

/// An SSH key's use is approved once per opening unless the person chose otherwise; every other key's is every time
fn default_setting(key_kind: &str) -> Approval {
    if key_kind == "ssh" { Approval::WhileOpen } else { Approval::Ask }
}

pub fn types(service: &mut Service, _: &Rc<Peer>, _: &Value) -> Result<Next, Refusal> {
    let program = service.config.worker.clone();
    Ok(Next::work(move || work::run(&program, &json!({"job": "key-types"})), |_, _, answer: Value| Next::Answer(answer)))
}

pub fn new(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let name = name_of(body, "name")?;
    let key_kind = text(body, "key_kind")?;
    if !matches!(key_kind, "ssh" | "signing" | "exchange" | "symmetric") {
        return Err(Refusal::BadRequest);
    }
    let key_type = optional_text(body, "type")?;
    if key_type.is_some_and(|key_type| key_type.len() > 16) {
        return Err(Refusal::BadRequest);
    }
    let exportable = flag(body, "exportable")?.unwrap_or(true);
    let approval = approval_of(body)?.unwrap_or_else(|| default_setting(key_kind));
    let vault = vault_of(body)?;
    let target = service.open_store(uid, vault.as_deref())?.id().to_string();
    let request = json!({"job": "key-new", "key_kind": key_kind, "type": key_type});
    let keeping = Keeping { target, name, exportable, approval, comment: None };
    Ok(begin(service, keeping, request))
}

/// A key `keys::import_openssh` read in the client, finished: the service never takes a foreign file. Refused unless it reads as
/// that type of key, which only the job looks at.
pub fn import(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let name = name_of(body, "name")?;
    let key_type = text(body, "type")?;
    if key_type.len() > 16 {
        return Err(Refusal::BadRequest);
    }
    let key_kind = optional_text(body, "key_kind")?.unwrap_or("ssh");
    if !matches!(key_kind, "ssh" | "signing" | "exchange" | "symmetric") {
        return Err(Refusal::BadRequest);
    }
    let private = base64_field(body, "private", MOST_KEY)?;
    // Text from the file it came from: kept only if it is only a name, and dropped rather than refusing a good key
    let comment = optional_text(body, "comment")?.filter(|comment| valid_name(comment)).map(str::to_string);
    let exportable = flag(body, "exportable")?.unwrap_or(true);
    let approval = approval_of(body)?.unwrap_or_else(|| default_setting(key_kind));
    let vault = vault_of(body)?;
    let target = service.open_store(uid, vault.as_deref())?.id().to_string();
    let request = json!({"job": "key-check", "key_kind": key_kind, "type": key_type, "private": encode(&private)});
    let keeping = Keeping { target, name, exportable, approval, comment };
    Ok(begin(service, keeping, request))
}

/// `never` is only set after a person says so, and the rest as any new item is
fn begin(service: &Service, keeping: Keeping, request: Value) -> Next {
    let program = service.config.worker.clone();
    let work_on = move |keeping: Keeping| Next::work(move || work::run(&program, &request), move |service, peer, made: Value| keep(service, peer.uid, &keeping, &made));
    if keeping.approval == Approval::Never {
        let shown = keeping.name.clone();
        return Next::approve(Action::Use, say::LOOSEN_ITEM, &shown, move |_, _| work_on(keeping));
    }
    work_on(keeping)
}

pub fn public(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let id = item_id(body)?;
    let (vault, _, _) = service.locate(uid, KIND_KEY, &id, None)?;
    Ok(match read::<Held>(service, uid, &vault, KIND_KEY, &id) {
        Ok(held) => Next::Answer(json!({"ok": true, "public": held.public})),
        Err(next) => next,
    })
}

/// The private key out of the vault, always asked, and never for one made not to leave. An encrypted PEM, or a plain one if asked by
/// name for it, as a client says exactly which.
pub fn export(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let id = item_id(body)?;
    let plain = flag(body, "plain")?.unwrap_or(false);
    let phrase = match (plain, body.get("passphrase").is_some()) {
        (true, false) => None,
        (false, true) => match passphrase(body, "passphrase")? {
            Ok(phrase) => Some(phrase),
            Err(next) => return Ok(next),
        },
        _ => return Err(Refusal::BadRequest),
    };
    let (vault, _, _) = service.locate(uid, KIND_KEY, &id, None)?;
    // Refused for good, before anyone is asked: nothing changes this after a key is made
    let exportable = service.store_of(uid, &vault)?.entry(KIND_KEY, &id).is_some_and(|entry| entry.meta["exportable"] == json!(true));
    if !exportable {
        return Ok(refuse_with("not-exportable", None));
    }
    let program = service.config.worker.clone();
    let id_there = id.clone();
    Ok(service.with_approval(uid, KIND_KEY, &id_there, Action::Export, say::EXPORT_ITEM, true, move |service, vault| {
        let held: Held = match read(service, uid, vault, KIND_KEY, &id) {
            Ok(held) => held,
            Err(next) => return next,
        };
        if !held.exportable {
            return refuse_with("not-exportable", None);
        }
        let request = json!({"job": "key-export", "type": held.key_type, "key": held.key.as_str(), "plain": plain, "passphrase": phrase.as_ref().map(|phrase| phrase.as_str())});
        Next::work(move || work::run(&program, &request), |_, _, answer: Value| Next::Answer(answer))
    }))
}
