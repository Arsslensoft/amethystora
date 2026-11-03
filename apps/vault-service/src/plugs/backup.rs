//! Backup codes. A client gets one at a time, with what is left; showing them all takes its own question. The counts are in the list,
//! so a window draws them, and works out which accounts have none, with nothing decrypted.

use std::rc::Rc;

use serde_json::{json, Value};

use amethystora_vault_core::codes;
use amethystora_vault_core::items::{new_id, Account, BackupService, KIND_ACCOUNT, KIND_BACKUP_CODES, MAX_CODE, MAX_CODES, MAX_NAME, MAX_NOTE, MAX_TAGS, Secret};
use amethystora_vault_core::protocol::{valid_name, Approval, Refusal};
use amethystora_vault_core::store::Store;

use super::{approval_of, clean_name, extra_of, flag, flat, item_id, meta_fits, must_be_open, note_of, now_ms, number, optional_text, read, tags_of, text, vault_of};
use crate::outside::{say, Action};
use crate::peer::Peer;
use crate::service::{Next, Service};

pub(super) fn meta_of(service: &BackupService) -> Value {
    let total = service.codes.len();
    let used = service.codes.iter().filter(|code| code.used).count();
    let mut meta = json!({"service": service.service_name, "total": total, "used": used, "left": total - used, "low": codes::is_low(service), "has_note": service.note.is_some(), "added_at": service.added_at});
    if let Some(account) = &service.account {
        meta["account"] = json!(account);
    }
    if let Some(tags) = &service.tags {
        meta["tags"] = json!(tags);
    }
    if let Some(order) = service.order {
        meta["order"] = json!(order);
    }
    meta
}

pub(super) fn name_of(service: &BackupService) -> String {
    clean_name(&service.service_name, "Backup codes")
}

pub(super) fn well_made(service: &BackupService) -> bool {
    service.service_name.len() <= MAX_NAME
        && service.account.as_ref().is_none_or(|account| account.len() <= MAX_NAME)
        && service.note.as_ref().is_none_or(|note| note.as_str().len() <= MAX_NOTE)
        && service.space_id.as_ref().is_none_or(|space| space.len() <= MAX_NAME)
        && service.tags.as_ref().is_none_or(|tags| tags.len() <= MAX_TAGS && tags.iter().all(|tag| tag.len() <= MAX_NAME))
        && service.codes.len() <= MAX_CODES
        && service.codes.iter().all(|code| !code.code.is_empty() && code.code.as_str().len() <= MAX_CODE && code.unknown.fits())
        && service.unknown.fits()
        && service.deleted_at.is_none()
}

fn keep(service: &mut Service, uid: u32, vault: &str, item: &BackupService, approval: Approval) -> Result<(), Next> {
    let meta = meta_of(item);
    if !meta_fits(&meta) {
        return Err(Next::refuse(Refusal::TooLarge));
    }
    let value = serde_json::to_value(item).map_err(|_| Next::refuse(Refusal::Failed))?;
    let name = name_of(item);
    let extra = extra_of(&name, &meta, &value);
    service.write_in(uid, vault, KIND_BACKUP_CODES, &item.id, extra, |store| store.put(KIND_BACKUP_CODES, &item.id, &name, &meta, approval, &value).map(|_| ())).map_err(Next::trouble)?;
    service.touch(uid);
    Ok(())
}

fn setting(store: &Store, id: &str) -> Approval {
    store.entry(KIND_BACKUP_CODES, id).map_or(Approval::WhileOpen, |entry| entry.approval)
}

fn codes_of(body: &Value) -> Result<Vec<Secret>, Refusal> {
    let list = body.get("codes").and_then(Value::as_array).ok_or(Refusal::BadRequest)?;
    if list.len() > MAX_CODES {
        return Err(Refusal::TooLarge);
    }
    list.iter()
        .map(|code| code.as_str().filter(|code| !code.is_empty() && code.len() <= MAX_CODE).map(|code| Secret::new(code.to_string())).ok_or(Refusal::BadRequest))
        .collect()
}

fn add_to(service: &mut Service, uid: u32, vault: &str, id: &str, new_codes: Vec<Secret>) -> Result<Next, Next> {
    let mut item: BackupService = read(service, uid, vault, KIND_BACKUP_CODES, id)?;
    let added = codes::add_codes(&mut item, new_codes, now_ms());
    let store = service.store_of(uid, vault).map_err(Next::refuse)?;
    let approval = setting(store, id);
    keep(service, uid, vault, &item, approval)?;
    Ok(Next::Answer(json!({"ok": true, "id": id, "vault": vault, "added": added.added, "repeats": added.repeats, "over_limit": added.over_limit, "total": item.codes.len()})))
}

fn add_new(service: &mut Service, uid: u32, target: &str, item: BackupService, approval: Approval, added: codes::AddedCodes) -> Result<Next, Next> {
    let store = service.store_of(uid, target).map_err(Next::refuse)?;
    if !store.has_room(&name_of(&item), &meta_of(&item)) {
        return Err(Next::refuse(Refusal::TooLarge));
    }
    keep(service, uid, target, &item, approval)?;
    Ok(Next::Answer(json!({"ok": true, "id": item.id, "vault": target, "added": item.codes.len(), "repeats": added.repeats, "over_limit": added.over_limit, "total": item.codes.len()})))
}

/// The codes parsed in the client, from a paste, a file or a PDF. A new service, or more for one held.
pub fn add(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let new_codes = codes_of(body)?;
    if body.get("id").is_some() {
        let id = item_id(body)?;
        let id_there = id.clone();
        return Ok(service.with_approval(uid, KIND_BACKUP_CODES, &id_there, Action::Use, say::CHANGE_ITEM, false, move |service, vault| flat(add_to(service, uid, vault, &id, new_codes))));
    }
    let name = text(body, "service")?;
    if !valid_name(name) {
        return Err(Refusal::BadRequest);
    }
    let vault = vault_of(body)?;
    let approval = approval_of(body)?.unwrap_or(Approval::WhileOpen);
    let now = now_ms();
    let mut item = BackupService {
        id: new_id().map_err(|_| Refusal::Failed)?,
        service_name: name.to_string(),
        account: optional_text(body, "account")?.map(str::to_string),
        note: note_of(body)?.flatten(),
        added_at: now,
        updated_at: Some(now),
        ..BackupService::default()
    };
    let added = codes::add_codes(&mut item, new_codes, now);
    if !well_made(&item) {
        return Err(Refusal::BadRequest);
    }
    let target = service.open_store(uid, vault.as_deref())?.id().to_string();
    if approval == Approval::Never {
        let shown = name_of(&item);
        return Ok(Next::approve(Action::Use, say::LOOSEN_ITEM, &shown, move |service, peer| flat(add_new(service, peer.uid, &target, item, approval, added))));
    }
    Ok(flat(add_new(service, uid, &target, item, approval, added)))
}

pub fn list(service: &mut Service, peer: &Rc<Peer>, _: &Value) -> Result<Next, Refusal> {
    must_be_open(service, peer.uid)?;
    Ok(service.list(peer.uid, Some(KIND_BACKUP_CODES)))
}

fn counts(item: &BackupService) -> (usize, bool) {
    (item.codes.iter().filter(|code| !code.used).count(), codes::is_low(item))
}

/// The next code and what is left, not yet marked: a person marks it once it has worked
pub fn next(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let id = item_id(body)?;
    let id_there = id.clone();
    Ok(service.with_approval(uid, KIND_BACKUP_CODES, &id_there, Action::Use, say::USE_ITEM, false, move |service, vault| {
        flat(read::<BackupService>(service, uid, vault, KIND_BACKUP_CODES, &id).map(|item| {
            let (left, low) = counts(&item);
            let at = codes::next_unused(&item);
            let code = at.and_then(|at| item.codes.get(at)).map(|code| code.code.as_str());
            Next::Answer(json!({"ok": true, "index": at, "code": code, "left": left, "low": low, "copy": "secret"}))
        }))
    }))
}

fn mark(service: &mut Service, uid: u32, vault: &str, id: &str, index: usize, used: bool) -> Result<Next, Next> {
    let mut item: BackupService = read(service, uid, vault, KIND_BACKUP_CODES, id)?;
    let now = now_ms();
    let code = item.codes.get_mut(index).ok_or_else(|| Next::refuse(Refusal::BadRequest))?;
    if used {
        code.mark_used(now);
    } else {
        code.mark_unused();
    }
    let used_at = code.used_at;
    item.updated_at = Some(now);
    let store = service.store_of(uid, vault).map_err(Next::refuse)?;
    let approval = setting(store, id);
    keep(service, uid, vault, &item, approval)?;
    let (left, low) = counts(&item);
    Ok(Next::Answer(json!({"ok": true, "used_at": used_at, "left": left, "low": low})))
}

/// Marks a code used, or takes that back: `used` false is the window's Undo
pub fn used(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let id = item_id(body)?;
    let index = number(body, "index", 0..=MAX_CODES as u64)?.ok_or(Refusal::BadRequest)? as usize;
    let used = flag(body, "used")?.ok_or(Refusal::BadRequest)?;
    let id_there = id.clone();
    Ok(service.with_approval(uid, KIND_BACKUP_CODES, &id_there, Action::Use, say::CHANGE_ITEM, false, move |service, vault| flat(mark(service, uid, vault, &id, index, used))))
}

/// Every code, for showing and printing: always asks
pub fn all(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let id = item_id(body)?;
    let id_there = id.clone();
    Ok(service.with_approval(uid, KIND_BACKUP_CODES, &id_there, Action::Use, say::SHOW_CODES, true, move |service, vault| {
        flat(read::<BackupService>(service, uid, vault, KIND_BACKUP_CODES, &id).map(|item| {
            let list: Vec<Value> = item.codes.iter().map(|code| json!({"code": code.code.as_str(), "used": code.used, "used_at": code.used_at})).collect();
            Next::Answer(json!({"ok": true, "codes": list, "copy": "secret"}))
        }))
    }))
}

fn apply_update(service: &mut Service, uid: u32, vault: &str, id: &str, body: &Value) -> Result<Next, Next> {
    let bad = |_| Next::refuse(Refusal::BadRequest);
    let mut item: BackupService = read(service, uid, vault, KIND_BACKUP_CODES, id)?;
    if let Some(name) = optional_text(body, "service").map_err(bad)? {
        if !valid_name(name) {
            return Err(Next::refuse(Refusal::BadRequest));
        }
        item.service_name = name.to_string();
    }
    if let Some(account) = optional_text(body, "account").map_err(bad)? {
        item.account = Some(account.to_string());
    }
    if let Some(note) = note_of(body).map_err(bad)? {
        item.note = note;
    }
    if let Some(tags) = tags_of(body).map_err(bad)? {
        item.tags = Some(tags);
    }
    if let Some(order) = body.get("order").filter(|order| !order.is_null()) {
        item.order = Some(order.as_i64().ok_or(Refusal::BadRequest).map_err(bad)?);
    }
    if !well_made(&item) {
        return Err(Next::refuse(Refusal::BadRequest));
    }
    item.updated_at = Some(now_ms());
    let store = service.store_of(uid, vault).map_err(Next::refuse)?;
    let approval = setting(store, id);
    keep(service, uid, vault, &item, approval)?;
    Ok(Next::Answer(json!({"ok": true, "id": id})))
}

/// As an account's: a name is what prompts show, so changing one asks, and order and tags do not
pub fn update(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let id = item_id(body)?;
    let body = body.clone();
    let asks = ["service", "account", "note"].iter().any(|field| body.get(field).is_some());
    let id_there = id.clone();
    if asks {
        return Ok(service.with_approval(uid, KIND_BACKUP_CODES, &id_there, Action::Use, say::CHANGE_ITEM, false, move |service, vault| flat(apply_update(service, uid, vault, &id, &body))));
    }
    let (vault, _, _) = service.locate(uid, KIND_BACKUP_CODES, &id, None)?;
    Ok(flat(apply_update(service, uid, &vault, &id, &body)))
}

pub fn note(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let id = item_id(body)?;
    let id_there = id.clone();
    Ok(service.with_approval(uid, KIND_BACKUP_CODES, &id_there, Action::Use, say::USE_ITEM, false, move |service, vault| {
        flat(read::<BackupService>(service, uid, vault, KIND_BACKUP_CODES, &id).map(|item| Next::Answer(json!({"ok": true, "note": item.note.as_ref().map(|note| note.as_str())}))))
    }))
}

/// Which accounts have no backup codes, from the list alone: the names are all it needs, so nothing is decrypted
pub fn coverage(service: &mut Service, peer: &Rc<Peer>, _: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let (mut accounts, mut ids, mut services) = (Vec::new(), Vec::new(), Vec::new());
    for store in service.open_stores(uid) {
        for entry in store.entries() {
            let word = |name: &str| entry.meta.get(name).and_then(Value::as_str).unwrap_or("").to_string();
            if entry.kind == KIND_ACCOUNT {
                accounts.push(Account { issuer: word("issuer"), label: word("label"), no_backup_codes: entry.meta.get("no_backup_codes").and_then(Value::as_bool), ..Account::default() });
                ids.push((entry.id.clone(), entry.name.clone()));
            } else if entry.kind == KIND_BACKUP_CODES {
                services.push(BackupService { service_name: word("service"), ..BackupService::default() });
            }
        }
    }
    let found = codes::coverage(&accounts, &services);
    let missing: Vec<Value> = found
        .missing
        .iter()
        .filter_map(|at| ids.get(*at).zip(accounts.get(*at)))
        .map(|((id, name), account)| json!({"id": id, "name": name, "service": codes::account_service_name(account)}))
        .collect();
    Ok(Next::Answer(json!({"ok": true, "covered": found.covered, "total": found.total, "missing": missing})))
}
