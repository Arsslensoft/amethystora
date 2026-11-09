//! Certificates, the account's CA and the machine's: only in the first vault, and only ever issued in the job process. What a
//! certificate may say is the table in `pki`; a request names a row of it. An issue always asks, so polkit's `allow_active` is what
//! keeps it to the person at the seat: a session over SSH gets a no. A guest gets none, whatever polkit says.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};

use amethystora_vault_core::keys::pem;
use amethystora_vault_core::pki;
use amethystora_vault_core::protocol::{encode, Approval, Refusal, KIND_ACCOUNT_CA, KIND_CERTIFICATE};
use amethystora_vault_core::seal;
use amethystora_vault_core::slots::Chip;
use amethystora_vault_core::store::{self, Store, Trouble};

use super::{approval_of, base64_field, extra_of, flag, item_id, machine as files, meta_fits, must_be_open, name_of, optional_text, passphrase, read, refuse_with, text, work};
use crate::outside::{say, Action, Person};
use crate::peer::Peer;
use crate::service::{Next, Service};

/// The one account's CA a vault holds
const CA_ID: &str = "ca";
const GUEST: &str = "amethystora-guest";
/// A certificate's own name, as the table has them: what a request may ask for. A CA's profile is never requested.
const LEAVES: [(&str, Who); 6] = [
    ("pdf", Who::Account),
    ("code", Who::Account),
    ("smime", Who::Account),
    ("local-tls", Who::Local),
    ("service-login", Who::Device),
    ("device-identity", Who::Device),
];

/// Which CA issues a profile
#[derive(Clone, Copy, PartialEq, Eq)]
enum Who {
    Account,
    Device,
    Local,
}

fn who_issues(profile: &str) -> Option<Who> {
    LEAVES.iter().find(|(name, _)| *name == profile).map(|(_, who)| *who)
}

/// The machine says, not the client: a guest has none, and nobody without a line in the password database does
fn person(service: &Service, uid: u32) -> Result<Person, Next> {
    match service.outside.person(uid) {
        Some(person) if person.login != GUEST => Ok(person),
        Some(_) => Err(refuse_with("guest", None)),
        None => Err(Next::refuse(Refusal::Refused)),
    }
}

/// What goes in a CA's name, and a signer's unless one is typed: the full name, else the login. Shown first, never typed for a CA.
fn name_for_ca(person: &Person) -> String {
    match &person.full_name {
        Some(name) if pki::check_name(name, pki::MAX_NAME - " Code Signing CA".len()) => name.clone(),
        _ => person.login.clone(),
    }
}

fn first_store(service: &mut Service, uid: u32) -> Result<&mut Store, Refusal> {
    must_be_open(service, uid)?;
    service.first_open(uid).ok_or(Refusal::Closed)
}

/// What a certificate was asked to say, kept for its renewal: no secret, and each field only what was typed
#[derive(Clone, Default, serde::Serialize, Deserialize)]
struct Fields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    organization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    web: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hosts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    key_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    digest: Option<String>,
}

fn short(body: &Value, name: &str, limit: usize) -> Result<Option<String>, Refusal> {
    match optional_text(body, name)? {
        Some(text) if text.len() <= limit => Ok(Some(text.to_string())),
        Some(_) => Err(Refusal::BadRequest),
        None => Ok(None),
    }
}

fn fields_of(body: &Value) -> Result<Fields, Refusal> {
    let hosts = match body.get("hosts") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(list)) if list.len() <= 32 => list.iter().map(|host| host.as_str().filter(|host| host.len() <= 253).map(str::to_string).ok_or(Refusal::BadRequest)).collect::<Result<_, _>>()?,
        Some(_) => return Err(Refusal::BadRequest),
    };
    Ok(Fields {
        name: short(body, "name", 1024)?,
        organization: short(body, "organization", 1024)?,
        country: short(body, "country", 16)?,
        email: short(body, "email", 1024)?,
        web: short(body, "web", 1024)?,
        hosts,
        key_type: short(body, "key_type", 16)?,
        digest: short(body, "digest", 16)?,
    })
}

/// A job's request for this profile and these fields
fn job_fields(job: &str, profile: &str, fields: &Fields, login: Option<&str>) -> Value {
    let mut request = serde_json::to_value(fields).unwrap_or_else(|_| json!({}));
    request["job"] = json!(job);
    request["profile"] = json!(profile);
    if let Some(login) = login {
        request["login"] = json!(login);
    }
    request
}

/// Everything an issue needs off the loop
struct Issue {
    state: PathBuf,
    chip: Arc<dyn Chip>,
    program: PathBuf,
    now: i64,
    profile: String,
    who: Who,
    fields: Fields,
    login: Option<String>,
    /// The name an account's CA goes by, if one has to be made
    person: String,
    /// The account's CA as the vault holds it, if it has one
    account_ca: Option<Value>,
}

fn failed() -> Value {
    json!({"ok": false, "why": "failed"})
}

/// The machine's root, or a CA it made, is there and its key will not open: sealed to another chip, or by this service under another
/// user number. Nothing issues again until `root rotate` makes a new one, so the client is told that rather than to try again.
fn unopened() -> Value {
    json!({"ok": false, "why": "refused", "problem": "root-unopened"})
}

/// The answer to a job, or stops with it: a refusal goes back to the client as it is
fn job(program: &Path, request: &Value) -> Result<Value, Value> {
    let answer = work::run(program, request);
    if work::refused(&answer) { Err(answer) } else { Ok(answer) }
}

/// The root, made on first need: an RSA 4096 key, seconds of the job's time, once
fn the_root(issue: &Issue) -> Result<Value, Value> {
    if let Some(record) = files::read(&issue.state, files::ROOT).map_err(|()| failed())? {
        return Ok(record);
    }
    let made = job(&issue.program, &json!({"job": "make-root", "now": issue.now}))?;
    files::save(&issue.state, &issue.chip, files::ROOT, &made).map_err(|()| failed())
}

/// A CA made under the root, which has to be opened for it
fn make_ca(issue: &Issue, root: &Value, profile: &str, name: Option<&str>) -> Result<Value, Value> {
    let key = files::open_key(&issue.chip, files::ROOT, root).map_err(|()| unopened())?;
    let mut request = json!({"job": "make-ca", "profile": profile, "now": issue.now, "root": {"cert": root["cert"], "key": key.as_str(), "type": root["type"]}});
    if let Some(name) = name {
        request["name"] = json!(name);
    }
    let mut made = job(&issue.program, &request)?;
    made["root_sha256"] = root["description"]["sha256"].clone();
    Ok(made)
}

/// What came out: the leaf, and the account's CA if it had to be made
fn issuing(issue: Issue) -> Value {
    run_issue(&issue).unwrap_or_else(|why| why)
}

fn run_issue(issue: &Issue) -> Result<Value, Value> {
    let root = the_root(issue)?;
    let root_sha256 = root["description"]["sha256"].as_str().ok_or_else(failed)?.to_string();
    let mut new_ca = Value::Null;
    let (ca_cert, ca_key, ca_type) = match issue.who {
        Who::Account => {
            let held = issue.account_ca.clone().filter(|ca| files::current(ca, &root_sha256, issue.now));
            let ca = match held {
                Some(ca) => ca,
                None => {
                    let made = make_ca(issue, &root, "account-ca", Some(&issue.person))?;
                    new_ca = made.clone();
                    made
                }
            };
            (ca["cert"].clone(), ca["key"].as_str().ok_or_else(failed)?.to_string(), ca["type"].clone())
        }
        Who::Device | Who::Local => {
            let (which, profile) = if issue.who == Who::Device { (files::DEVICE_CA, "device-ca") } else { (files::LOCAL_CA, "local-services-ca") };
            let held = files::read(&issue.state, which).map_err(|()| failed())?.filter(|ca| files::current(ca, &root_sha256, issue.now));
            let ca = match held {
                Some(ca) => ca,
                None => {
                    let made = make_ca(issue, &root, profile, None)?;
                    files::save(&issue.state, &issue.chip, which, &made).map_err(|()| failed())?
                }
            };
            let key = files::open_key(&issue.chip, which, &ca).map_err(|()| unopened())?;
            (ca["cert"].clone(), key.as_str().to_string(), ca["type"].clone())
        }
    };
    let mut request = job_fields("issue", &issue.profile, &issue.fields, issue.login.as_deref());
    request["now"] = json!(issue.now);
    request["ca"] = json!({"cert": ca_cert, "key": ca_key, "type": ca_type});
    request["root"] = root["cert"].clone();
    let mut made = job(&issue.program, &request)?;
    made["new_ca"] = new_ca;
    Ok(made)
}

/// Keeps what was issued: the certificate as one item with its key and chain, and the account's CA if it is new. The vault may have
/// closed meanwhile, and then nothing is kept, which is why this is on the loop and only now.
#[allow(clippy::too_many_arguments)]
fn keep_issued(service: &mut Service, uid: u32, profile: &str, title: &str, fields: &Fields, renews: Option<&str>, approval: Approval, made: &Value) -> Next {
    if work::refused(made) {
        return Next::Answer(made.clone());
    }
    let state = service.config.state.clone();
    let Some(store) = service.first_open(uid) else { return Next::refuse(Refusal::Closed) };
    let description = &made["description"];
    // pki cannot see this: a key already certified for something else, or a CA's, signs for whatever it was certified for
    let mut fresh = vec![description["spki_sha256"].as_str()];
    if !made["new_ca"].is_null() {
        fresh.push(made["new_ca"]["description"]["spki_sha256"].as_str());
    }
    let mut known: Vec<String> = store.entries().filter(|entry| entry.kind == KIND_CERTIFICATE || entry.kind == KIND_ACCOUNT_CA).filter_map(|entry| entry.meta["spki_sha256"].as_str().map(str::to_string)).collect();
    for which in [files::ROOT, files::DEVICE_CA, files::LOCAL_CA] {
        if let Ok(Some(record)) = files::read(&state, which) {
            known.extend(record["description"]["spki_sha256"].as_str().map(str::to_string));
        }
    }
    if key_reused(&known, &fresh) {
        return refuse_with("key-reused", None);
    }
    let new_ca = (!made["new_ca"].is_null()).then(|| {
        let ca = &made["new_ca"];
        let meta = json!({"subject": ca["description"]["subject"], "not_after": ca["description"]["not_after"], "sha256": ca["description"]["sha256"], "spki_sha256": ca["description"]["spki_sha256"]});
        (meta, ca)
    });
    let mut meta = json!({
        "profile": profile,
        "subject": description["subject"],
        "issuer": description["issuer"],
        "not_before": description["not_before"],
        "not_after": description["not_after"],
        "sha256": description["sha256"],
        "spki_sha256": description["spki_sha256"],
        "key_type": made["type"],
    });
    if let Some(renews) = renews {
        meta["renews"] = json!(renews);
    }
    if !meta_fits(&meta) || !store.has_room(title, &meta) {
        return Next::refuse(Refusal::TooLarge);
    }
    let id = seal::random_hex(8);
    let value = json!({"profile": profile, "type": made["type"], "key": made["key"], "cert": made["cert"], "chain": made["chain"], "fields": fields});
    // The certificate this one renews is marked, with its own value and setting kept
    let renewed = renews.and_then(|old| {
        let entry = store.entry(KIND_CERTIFICATE, old).cloned()?;
        let value = store.get(KIND_CERTIFICATE, old).ok()?;
        let mut meta = entry.meta;
        meta["renewed_by"] = json!(id);
        Some((old, entry.name, meta, entry.approval, value))
    });
    let vault = store.id().to_string();
    let mut written: Vec<(&str, &str)> = vec![(KIND_CERTIFICATE, &id)];
    written.extend(new_ca.as_ref().map(|_| (KIND_ACCOUNT_CA, CA_ID)));
    written.extend(renewed.as_ref().map(|(old, ..)| (KIND_CERTIFICATE, *old)));
    let extra = extra_of(title, &meta, &value) + new_ca.as_ref().map_or(0, |(meta, ca)| extra_of("Code Signing CA", meta, ca)) + renewed.as_ref().map_or(0, |(_, name, meta, _, value)| extra_of(name, meta, value));
    // All through the account's quota, in the order they were always written
    let kept = service.write_many_in(uid, &vault, &written, extra, |store| {
        if let Some((meta, ca)) = &new_ca {
            store.put(KIND_ACCOUNT_CA, CA_ID, "Code Signing CA", meta, Approval::Ask, ca)?;
        }
        store.put(KIND_CERTIFICATE, &id, title, &meta, approval, &value)?;
        if let Some((old, name, meta, approval, value)) = &renewed {
            store.put(KIND_CERTIFICATE, old, name, meta, *approval, value)?;
        }
        Ok(())
    });
    if let Err(trouble) = kept {
        return Next::trouble(trouble);
    }
    service.touch(uid);
    let mut shown = description.clone();
    shown["profile"] = json!(profile);
    shown["key_type"] = made["type"].clone();
    Next::Answer(json!({"ok": true, "id": id, "vault": vault, "description": shown, "new_ca": new_ca.is_some()}))
}

/// Whether any of these new keys (by the hash of its public half) is one already certified, or two are the same, or one is not told
fn key_reused(known: &[String], fresh: &[Option<&str>]) -> bool {
    fresh.iter().enumerate().any(|(at, one)| match one {
        None | Some("") => true,
        Some(one) => known.iter().any(|held| held == one) || fresh[..at].contains(&Some(*one)),
    })
}

struct Asked {
    profile: String,
    title: String,
    fields: Fields,
    renews: Option<String>,
    approval: Approval,
}

/// Checks the fields, asks the person, then issues: a refused name costs no prompt and no root
fn start_issue(service: &mut Service, uid: u32, asked: Asked) -> Result<Next, Refusal> {
    let who = who_issues(&asked.profile).ok_or(Refusal::Refused)?;
    let found = match person(service, uid) {
        Ok(found) => found,
        Err(next) => return Ok(next),
    };
    // An account's CA and an account's own certificates live in the first vault, so nothing is said to a closed one
    first_store(service, uid)?;
    let mut asked = asked;
    let ca_name = name_for_ca(&found);
    if matches!(asked.profile.as_str(), "pdf" | "code" | "smime") && asked.fields.name.is_none() {
        asked.fields.name = Some(ca_name.clone());
    }
    let login = (asked.profile == "service-login").then(|| found.login.clone());
    let check = job_fields("issue-check", &asked.profile, &asked.fields, login.as_deref());
    let program = service.config.worker.clone();
    Ok(Next::work(
        move || work::run(&program, &check),
        move |_, _, answer: Value| {
            if work::refused(&answer) {
                return Next::Answer(answer);
            }
            let (action, message) = if asked.profile == "device-identity" { (Action::Admin, say::DEVICE_IDENTITY) } else { (Action::Use, say::ISSUE_CERT) };
            let shown = asked.title.clone();
            Next::approve(action, message, &shown, move |service, peer| {
                let uid = peer.uid;
                let account_ca = match (who, service.first_open(uid)) {
                    (_, None) => return Next::refuse(Refusal::Closed),
                    (Who::Account, Some(store)) => match store.get(KIND_ACCOUNT_CA, CA_ID) {
                        Ok(ca) => Some(ca),
                        Err(Trouble::NotFound) => None,
                        Err(trouble) => return Next::trouble(trouble),
                    },
                    _ => None,
                };
                let issue = Issue {
                    state: service.config.state.clone(),
                    chip: service.chip.clone(),
                    program: service.config.worker.clone(),
                    now: store::now() as i64,
                    profile: asked.profile.clone(),
                    who,
                    fields: asked.fields.clone(),
                    login,
                    person: ca_name,
                    account_ca,
                };
                Next::work(
                    move || issuing(issue),
                    move |service, peer, made: Value| keep_issued(service, peer.uid, &asked.profile, &asked.title, &asked.fields, asked.renews.as_deref(), asked.approval, &made),
                )
            })
        },
    ))
}

pub fn profiles(_: &mut Service, _: &Rc<Peer>, _: &Value) -> Result<Next, Refusal> {
    let list: Vec<Value> = LEAVES
        .iter()
        .filter_map(|(name, _)| pki::Profile::parse(name).map(|profile| (*name, profile)))
        .map(|(name, profile)| {
            let keys: Vec<&str> = profile.key_types().iter().map(|key_type| key_type.name()).collect();
            json!({"name": name, "keys": keys, "admin": name == "device-identity"})
        })
        .collect();
    Ok(Next::Answer(json!({"ok": true, "profiles": list})))
}

pub fn new(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    // A closed vault says so whatever else is wrong, as every op does
    must_be_open(service, uid)?;
    let profile = text(body, "profile")?;
    if who_issues(profile).is_none() {
        return Ok(refuse_with("unknown-profile", None));
    }
    let approval = approval_of(body)?.unwrap_or(Approval::Ask);
    // The prompt issuing it does not say this, so it is `set-approval`'s, with its own
    if approval == Approval::Never {
        return Err(Refusal::BadRequest);
    }
    let asked = Asked { profile: profile.to_string(), title: name_of(body, "title")?, fields: fields_of(body)?, renews: None, approval };
    start_issue(service, uid, asked)
}

/// What the list says, and for the PKCS#11 module the certificate and its chain, which are no secret
pub fn list(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let store = first_store(service, peer.uid)?;
    let purpose = optional_text(body, "purpose")?;
    let chain = flag(body, "chain")?.unwrap_or(false);
    let entries: Vec<(String, String, Value, &'static str, u64)> = store
        .entries()
        .filter(|entry| entry.kind == KIND_CERTIFICATE && purpose.is_none_or(|purpose| entry.meta["profile"] == json!(purpose)))
        .map(|entry| (entry.id.clone(), entry.name.clone(), entry.meta.clone(), entry.approval.name(), entry.changed))
        .collect();
    let mut items = Vec::with_capacity(entries.len());
    for (id, name, meta, approval, changed) in entries {
        let mut item = json!({"vault": store.id(), "kind": KIND_CERTIFICATE, "id": id, "name": name, "meta": meta, "approval": approval, "changed": changed});
        if chain {
            let Ok(held) = store.get(KIND_CERTIFICATE, &id) else { continue };
            item["cert"] = held["cert"].clone();
            item["chain"] = held["chain"].clone();
        }
        items.push(item);
    }
    Ok(Next::Answer(json!({"ok": true, "items": items})))
}

fn entry_of<'a>(store: &'a Store, id: &str) -> Result<&'a store::Entry, Refusal> {
    store.entry(KIND_CERTIFICATE, id).ok_or(Refusal::Refused)
}

pub fn show(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let id = item_id(body)?;
    let store = first_store(service, peer.uid)?;
    let entry = entry_of(store, &id)?;
    Ok(Next::Answer(json!({"ok": true, "name": entry.name, "description": entry.meta})))
}

/// The certificate and its chain as PEM, which needs no approval: they are no secret
fn chain_pem(held: &Value) -> Option<String> {
    let engine = base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    let mut out = pem("CERTIFICATE", &engine.decode(held["cert"].as_str()?).ok()?);
    for one in held["chain"].as_array()? {
        out.push_str(&pem("CERTIFICATE", &engine.decode(one.as_str()?).ok()?));
    }
    Some(out)
}

pub fn chain(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let id = item_id(body)?;
    let store = first_store(service, peer.uid)?;
    let held = match store.get(KIND_CERTIFICATE, &id) {
        Ok(held) => held,
        Err(trouble) => return Ok(Next::trouble(trouble)),
    };
    Ok(Next::Answer(json!({"ok": true, "pem": chain_pem(&held).ok_or(Refusal::Refused)?})))
}

/// PKCS#12 with a typed password, which `osslsigncode`, `signtool` and `jarsigner` take: always asks
pub fn pfx(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    first_store(service, uid)?;
    let id = item_id(body)?;
    let password = match passphrase(body, "password")? {
        Ok(password) if password.chars().count() <= 256 => password,
        Ok(_) => return Ok(refuse_with("bad-password", None)),
        Err(next) => return Ok(next),
    };
    let program = service.config.worker.clone();
    let id_there = id.clone();
    Ok(service.with_approval(uid, KIND_CERTIFICATE, &id_there, Action::Export, say::EXPORT_ITEM, true, move |service, vault| {
        let held: Value = match read(service, uid, vault, KIND_CERTIFICATE, &id) {
            Ok(held) => held,
            Err(next) => return next,
        };
        let name = service.store_of(uid, vault).ok().and_then(|store| store.entry(KIND_CERTIFICATE, &id).map(|entry| entry.name.clone())).unwrap_or_default();
        let request = json!({"job": "pfx", "type": held["type"], "key": held["key"], "cert": held["cert"], "chain": held["chain"], "name": name, "password": password.as_str()});
        Next::work(move || work::run(&program, &request), |_, _, answer: Value| Next::Answer(answer))
    }))
}

/// Its key as PEM beside the chain: under a passphrase, or plain for a local server alone, which cannot ask a person
pub fn key(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    let store = first_store(service, uid)?;
    let id = item_id(body)?;
    let is_local = entry_of(store, &id)?.meta["profile"] == json!("local-tls");
    let plain = flag(body, "plain")?.unwrap_or(false);
    if plain && !is_local {
        return Ok(refuse_with("wrong-purpose", None));
    }
    let phrase = match (plain, body.get("passphrase").is_some()) {
        (true, false) => None,
        (false, true) => match passphrase(body, "passphrase")? {
            Ok(phrase) => Some(phrase),
            Err(next) => return Ok(next),
        },
        _ => return Err(Refusal::BadRequest),
    };
    let program = service.config.worker.clone();
    let id_there = id.clone();
    Ok(service.with_approval(uid, KIND_CERTIFICATE, &id_there, Action::Export, say::EXPORT_ITEM, true, move |service, vault| {
        let held: Value = match read(service, uid, vault, KIND_CERTIFICATE, &id) {
            Ok(held) => held,
            Err(next) => return next,
        };
        let Some(pem) = chain_pem(&held) else { return Next::refuse(Refusal::Refused) };
        let request = json!({"job": "key-export", "type": held["type"], "key": held["key"], "plain": plain, "passphrase": phrase.as_ref().map(|phrase| phrase.as_str())});
        Next::work(
            move || work::run(&program, &request),
            move |_, _, answer: Value| {
                if work::refused(&answer) {
                    return Next::Answer(answer);
                }
                Next::Answer(json!({"ok": true, "key": answer["data"], "pem": pem}))
            },
        )
    }))
}

/// A new certificate under the same name, with a new key. The old one stays, marked, until removed.
pub fn renew(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    let store = first_store(service, uid)?;
    let id = item_id(body)?;
    let entry = entry_of(store, &id)?.clone();
    let held = match store.get(KIND_CERTIFICATE, &id) {
        Ok(held) => held,
        Err(trouble) => return Ok(Next::trouble(trouble)),
    };
    let fields: Fields = serde_json::from_value(held["fields"].clone()).map_err(|_| Refusal::Refused)?;
    let profile = held["profile"].as_str().ok_or(Refusal::Refused)?.to_string();
    let asked = Asked { profile, title: entry.name, fields, renews: Some(id), approval: entry.approval };
    let next = start_issue(service, uid, asked)?;
    Ok(next)
}

fn within_dates(meta: &Value, now: i64) -> bool {
    meta["not_before"].as_i64().is_some_and(|not_before| not_before <= now) && meta["not_after"].as_i64().is_some_and(|not_after| now < not_after)
}

/// A digest in, a signature out: the key never leaves, and each certificate signs only for its own purpose. The job signs, so the
/// time this takes is a process starting: a moment a person sees, beside the approval they just gave.
pub fn sign_digest(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    let store = first_store(service, uid)?;
    let id = item_id(body)?;
    let purpose = text(body, "purpose")?;
    if purpose != "pdf" && purpose != "code" {
        return Err(Refusal::BadRequest);
    }
    let digest = text(body, "digest")?;
    let length = match digest {
        "sha256" => 32,
        "sha384" => 48,
        _ => return Err(Refusal::BadRequest),
    };
    let value = base64_field(body, "value", 48)?;
    if value.len() != length {
        return Ok(refuse_with("bad-digest", None));
    }
    let entry = entry_of(store, &id)?;
    if entry.meta["profile"] != json!(purpose) {
        return Ok(refuse_with("wrong-purpose", None));
    }
    if !within_dates(&entry.meta, store::now() as i64) {
        return Ok(refuse_with("expired", None));
    }
    let program = service.config.worker.clone();
    let (digest, purpose) = (digest.to_string(), purpose.to_string());
    let id_there = id.clone();
    Ok(service.with_approval(uid, KIND_CERTIFICATE, &id_there, Action::Use, say::SIGN, false, move |service, vault| {
        let held: Value = match read(service, uid, vault, KIND_CERTIFICATE, &id) {
            Ok(held) => held,
            Err(next) => return next,
        };
        let request = json!({"job": "sign", "type": held["type"], "key": held["key"], "cert": held["cert"], "chain": held["chain"], "now": store::now(), "purpose": purpose, "digest": digest, "value": encode(&value)});
        Next::work(move || work::run(&program, &request), |_, _, answer: Value| Next::Answer(answer))
    }))
}

/// A CA's certificate, to give to somebody who has to trust it. Nothing is made for this: not made yet is `refused`.
pub fn ca_cert(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    let which = text(body, "which")?;
    let record: Value = match which {
        "account" => {
            let store = first_store(service, uid)?;
            match store.get(KIND_ACCOUNT_CA, CA_ID) {
                Ok(ca) => ca,
                Err(_) => return Err(Refusal::Refused),
            }
        }
        "device" | "local" | "root" => {
            let name = match which {
                "device" => files::DEVICE_CA,
                "local" => files::LOCAL_CA,
                _ => files::ROOT,
            };
            match files::read(&service.config.state, name) {
                Ok(Some(record)) => record,
                Ok(None) => return Err(Refusal::Refused),
                Err(()) => return Err(Refusal::Failed),
            }
        }
        _ => return Err(Refusal::BadRequest),
    };
    use base64::Engine;
    let der = base64::engine::general_purpose::STANDARD.decode(record["cert"].as_str().ok_or(Refusal::Failed)?).map_err(|_| Refusal::Failed)?;
    Ok(Next::Answer(json!({"ok": true, "pem": pem("CERTIFICATE", &der), "subject": record["description"]["subject"], "sha256": record["description"]["sha256"], "not_after": record["description"]["not_after"]})))
}

/// What the machine has, for the window and `ame vault root`. Never makes one.
pub fn machine(service: &mut Service, _: &Rc<Peer>, _: &Value) -> Result<Next, Refusal> {
    match files::read(&service.config.state, files::ROOT) {
        Ok(None) => Ok(Next::Answer(json!({"ok": true, "root": false}))),
        Ok(Some(record)) => {
            let public = files::public_part(record);
            Ok(Next::Answer(json!({"ok": true, "root": true, "number": public["number"], "subject": public["description"]["subject"], "sha256": public["description"]["sha256"], "not_after": public["description"]["not_after"]})))
        }
        Err(()) => Err(Refusal::Failed),
    }
}

/// A new root, for a stolen key: an administrator's word. The two machine CAs go with the old one and each account's CA is seen to be
/// old at its next certificate. Whoever trusted the old root has to stop by hand, which only they can do.
pub fn rotate(service: &mut Service, peer: &Rc<Peer>, _: &Value) -> Result<Next, Refusal> {
    if let Err(next) = person(service, peer.uid) {
        return Ok(next);
    }
    // There is nothing to rotate on a machine nobody has needed one on, and this is no way to make the first
    if !matches!(files::read(&service.config.state, files::ROOT), Ok(Some(_))) {
        return Err(Refusal::Refused);
    }
    Ok(Next::approve(Action::Admin, say::ROTATE_ROOT, "", |service, _| {
        let (state, chip, program) = (service.config.state.clone(), service.chip.clone(), service.config.worker.clone());
        let now = store::now() as i64;
        Next::work(
            move || {
                let made = work::run(&program, &json!({"job": "make-root", "now": now}));
                if work::refused(&made) {
                    return made;
                }
                match files::save(&state, &chip, files::ROOT, &made) {
                    Ok(record) => {
                        files::remove(&state, files::DEVICE_CA);
                        files::remove(&state, files::LOCAL_CA);
                        json!({"ok": true, "number": record["number"], "sha256": record["description"]["sha256"]})
                    }
                    Err(()) => failed(),
                }
            },
            |_, _, answer: Value| Next::Answer(answer),
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_certified_once() {
        let known = vec!["aa".to_string(), "bb".to_string()];
        assert!(!key_reused(&known, &[Some("cc")]));
        assert!(!key_reused(&known, &[Some("cc"), Some("dd")]));
        assert!(key_reused(&known, &[Some("bb")]));
        assert!(key_reused(&known, &[Some("cc"), Some("aa")]));
        assert!(key_reused(&known, &[Some("cc"), Some("cc")]));
        assert!(key_reused(&known, &[None]));
        assert!(key_reused(&known, &[Some("")]));
    }

    #[test]
    fn a_certificate_signs_only_within_its_dates() {
        let meta = json!({"not_before": 100, "not_after": 200});
        assert!(!within_dates(&meta, 99));
        assert!(within_dates(&meta, 100));
        assert!(within_dates(&meta, 199));
        assert!(!within_dates(&meta, 200));
        assert!(!within_dates(&json!({}), 150));
    }
}
