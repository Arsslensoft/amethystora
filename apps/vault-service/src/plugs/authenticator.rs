//! Authenticator accounts. The secret stays here: one code goes out, never the key, but for the link a person asks to see.
//! Making a code touches nothing but the item's file and the stack: no libcrypto, and well inside the 5 ms budget.

use std::rc::Rc;

use serde_json::{json, Value};

use amethystora_vault_core::import::{self, Imported, Source};
use amethystora_vault_core::items::{new_id, Account, BackupService, OtpKind, KIND_ACCOUNT, KIND_BACKUP_CODES, MAX_NAME, MAX_NOTE, MAX_SECRET_CHARS, MAX_TAGS};
use amethystora_vault_core::otp::{self, Algorithm};
use amethystora_vault_core::otpauth::{self, MAX_COUNTER};
use amethystora_vault_core::protocol::{Approval, Refusal, MAX_LINE};
use amethystora_vault_core::store::{NewItem, Store, Trouble, MAX_ITEMS, MAX_LISTED_BYTES};

use super::backup;
use super::{approval_of, clean_name, extra_of, give_back, flag, flat, item_id, meta_fits, must_be_open, note_of, now_ms, optional_text, passphrase, read, tags_of, text, vault_of, work};
use crate::outside::{say, Action};
use crate::peer::Peer;
use crate::service::{Next, Service};

/// One request's worth of an import, so that the loop is held for only so long: a client sends a big import in several
const MOST_AT_ONCE: usize = 256;

fn kind_name(kind: OtpKind) -> &'static str {
    match kind {
        OtpKind::Totp => "totp",
        OtpKind::Hotp => "hotp",
        OtpKind::Steam => "steam",
    }
}

/// What the list shows of an account: everything but the secret and the note
fn meta_of(account: &Account) -> Value {
    let mut meta = json!({
        "type": kind_name(account.otp_kind()),
        "issuer": account.issuer,
        "label": account.label,
        "algorithm": account.algorithm,
        "digits": account.digits,
        "period": account.period,
        "has_note": account.note.is_some(),
        "added_at": account.added_at,
    });
    if let Some(counter) = account.counter {
        meta["counter"] = json!(counter);
    }
    if account.pinned == Some(true) {
        meta["pinned"] = json!(true);
    }
    if let Some(tags) = &account.tags {
        meta["tags"] = json!(tags);
    }
    if let Some(order) = account.order {
        meta["order"] = json!(order);
    }
    if account.no_backup_codes == Some(true) {
        meta["no_backup_codes"] = json!(true);
    }
    meta
}

fn name_of(account: &Account) -> String {
    let joined = match (account.issuer.is_empty(), account.label.is_empty()) {
        (false, false) => format!("{} ({})", account.issuer, account.label),
        (false, true) => account.issuer.clone(),
        _ => account.label.clone(),
    };
    clean_name(&joined, "Account")
}

/// A finished account from a client, which the service takes on trust for nothing. `strict` is the door one account comes in by; an
/// import keeps what BackMyCodes keeps, a secret that will only ever show dashes included, but is bounded all the same.
fn well_made(account: &Account, strict: bool) -> bool {
    let secret = account.secret.as_str();
    let tags_fit = account.tags.as_ref().is_none_or(|tags| tags.len() <= MAX_TAGS && tags.iter().all(|tag| tag.len() <= MAX_NAME));
    let bounded = account.issuer.len() <= MAX_NAME
        && account.label.len() <= MAX_NAME
        && account.algorithm.len() <= 32
        && !account.secret.is_empty()
        && secret.chars().count() <= MAX_SECRET_CHARS
        && account.note.as_ref().is_none_or(|note| note.as_str().len() <= MAX_NOTE)
        && account.space_id.as_ref().is_none_or(|space| space.len() <= MAX_NAME)
        && account.counter.is_none_or(|counter| (0..=MAX_COUNTER).contains(&counter))
        && tags_fit
        && account.unknown.fits()
        && account.deleted_at.is_none()
        && !otp::holds_no_bytes(secret);
    if !strict {
        return bounded;
    }
    bounded
        && (!account.issuer.is_empty() || !account.label.is_empty())
        && otp::base32_decode(secret).is_some()
        && Algorithm::from_stored(&account.algorithm).is_some()
        && (1..=10).contains(&account.digits)
        && (1..=86_400).contains(&account.period)
}

/// Every account and service an open vault holds. Read on the loop: a thousand items take a few milliseconds each way, and only
/// an import and an export ask.
fn held(store: &mut Store) -> Result<(Vec<Account>, Vec<BackupService>), Trouble> {
    let ids = |store: &Store, kind: &str| -> Vec<String> { store.entries().filter(|entry| entry.kind == kind).map(|entry| entry.id.clone()).collect() };
    let mut accounts = Vec::new();
    for id in ids(store, KIND_ACCOUNT) {
        accounts.push(serde_json::from_value(store.get(KIND_ACCOUNT, &id)?).map_err(|_| Trouble::Damaged)?);
    }
    let mut services = Vec::new();
    for id in ids(store, KIND_BACKUP_CODES) {
        services.push(serde_json::from_value(store.get(KIND_BACKUP_CODES, &id)?).map_err(|_| Trouble::Damaged)?);
    }
    Ok((accounts, services))
}

fn keep(service: &mut Service, uid: u32, vault: &str, account: &Account, approval: Approval) -> Result<(), Next> {
    let meta = meta_of(account);
    if !meta_fits(&meta) {
        return Err(Next::refuse(Refusal::TooLarge));
    }
    let value = serde_json::to_value(account).map_err(|_| Next::refuse(Refusal::Failed))?;
    let name = name_of(account);
    let extra = extra_of(&name, &meta, &value);
    service.write_in(uid, vault, KIND_ACCOUNT, &account.id, extra, |store| store.put(KIND_ACCOUNT, &account.id, &name, &meta, approval, &value).map(|_| ())).map_err(Next::trouble)?;
    service.touch(uid);
    Ok(())
}

fn add_now(service: &mut Service, uid: u32, target: &str, account: Account, approval: Approval) -> Result<Next, Next> {
    let store = service.store_of(uid, target).map_err(Next::refuse)?;
    let (accounts, _) = held(store).map_err(Next::trouble)?;
    if let Some(same) = accounts.iter().find(|one| one.deleted_at.is_none() && import::same_account(one, &account)) {
        return Ok(Next::Answer(json!({"ok": true, "id": same.id, "vault": target, "added": false})));
    }
    let mut account = account;
    account.id = new_id().map_err(|_| Next::refuse(Refusal::Failed))?;
    account.added_at = now_ms();
    account.updated_at = Some(account.added_at);
    if !store.has_room(&name_of(&account), &meta_of(&account)) {
        return Err(Next::refuse(Refusal::TooLarge));
    }
    keep(service, uid, target, &account, approval)?;
    Ok(Next::Answer(json!({"ok": true, "id": account.id, "vault": target, "added": true})))
}

/// One account, from a link the window decoded, parsed strictly here, or finished by the add form
pub fn add(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let vault = vault_of(body)?;
    let approval = approval_of(body)?.unwrap_or(Approval::WhileOpen);
    let account: Account = match (optional_text(body, "link")?, body.get("account")) {
        (Some(link), None) => otpauth::read_link(link).ok_or(Refusal::Refused)?,
        (None, Some(account)) => serde_json::from_value(account.clone()).map_err(|_| Refusal::BadRequest)?,
        _ => return Err(Refusal::BadRequest),
    };
    if !well_made(&account, true) {
        return Err(Refusal::Refused);
    }
    let target = service.open_store(uid, vault.as_deref())?.id().to_string();
    if approval == Approval::Never {
        let shown = name_of(&account);
        return Ok(Next::approve(Action::Use, say::LOOSEN_ITEM, &shown, move |service, peer| flat(add_now(service, peer.uid, &target, account, approval))));
    }
    Ok(flat(add_now(service, uid, &target, account, approval)))
}

/// The same as the generic `list` of this kind, whose meta holds no secret
pub fn list(service: &mut Service, peer: &Rc<Peer>, _: &Value) -> Result<Next, Refusal> {
    must_be_open(service, peer.uid)?;
    Ok(service.list(peer.uid, Some(KIND_ACCOUNT)))
}

fn code_now(service: &mut Service, uid: u32, vault: &str, id: &str, step: bool) -> Result<Next, Next> {
    let mut account: Account = read(service, uid, vault, KIND_ACCOUNT, id)?;
    let kind = account.otp_kind();
    if step {
        if kind != OtpKind::Hotp {
            return Err(Next::refuse(Refusal::BadRequest));
        }
        // On first, and on the disk before the code is seen: a code never comes twice, and the counter never goes back
        let next = account.counter.unwrap_or(0).checked_add(1).filter(|next| *next <= MAX_COUNTER).ok_or_else(|| Next::refuse(Refusal::Refused))?;
        account.counter = Some(next);
        account.updated_at = Some(now_ms());
        let store = service.store_of(uid, vault).map_err(Next::refuse)?;
        let approval = store.entry(KIND_ACCOUNT, id).map_or(Approval::WhileOpen, |entry| entry.approval);
        keep(service, uid, vault, &account, approval)?;
    }
    let now = now_ms();
    let seconds = now.div_euclid(1000);
    let code = otp::code_at(&account, seconds, 0);
    if !code.is_usable() {
        return Err(Next::refuse(Refusal::Refused));
    }
    let mut answer = json!({"ok": true, "code": code.as_str(), "vault": vault});
    match kind {
        OtpKind::Hotp => answer["counter"] = json!(account.counter.unwrap_or(0)),
        _ => {
            let left = otp::seconds_left(account.period, now);
            answer["seconds_left"] = json!(left);
            answer["period"] = json!(account.period);
            if left < 5 {
                answer["next"] = json!(otp::code_at(&account, seconds, 1).as_str());
            }
        }
    }
    Ok(Next::Answer(answer))
}

pub fn code(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let id = item_id(body)?;
    let step = flag(body, "step")?.unwrap_or(false);
    let id_there = id.clone();
    Ok(service.with_approval(uid, KIND_ACCOUNT, &id_there, Action::Use, say::USE_ITEM, false, move |service, vault| flat(code_now(service, uid, vault, &id, step))))
}

/// The link for the QR, and the key in groups for an app that cannot scan: always asks, whatever the item's setting
pub fn link(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let id = item_id(body)?;
    let id_there = id.clone();
    Ok(service.with_approval(uid, KIND_ACCOUNT, &id_there, Action::Export, say::SHOW_QR, true, move |service, vault| {
        flat(read::<Account>(service, uid, vault, KIND_ACCOUNT, &id).map(|account| match otpauth::link_for_qr(&account) {
            Some(link) => Next::Answer(json!({"ok": true, "link": link.as_str(), "key": otp::group_secret(account.secret.as_str())})),
            None => Next::refuse(Refusal::Refused),
        }))
    }))
}

fn apply_update(service: &mut Service, uid: u32, vault: &str, id: &str, body: &Value) -> Result<Next, Next> {
    let bad = |_| Next::refuse(Refusal::BadRequest);
    let mut account: Account = read(service, uid, vault, KIND_ACCOUNT, id)?;
    if let Some(issuer) = optional_text(body, "issuer").map_err(bad)? {
        account.issuer = issuer.to_string();
    }
    if let Some(label) = optional_text(body, "label").map_err(bad)? {
        account.label = label.to_string();
    }
    if let Some(note) = note_of(body).map_err(bad)? {
        account.note = note;
    }
    if let Some(tags) = tags_of(body).map_err(bad)? {
        account.tags = Some(tags);
    }
    if let Some(order) = body.get("order").filter(|order| !order.is_null()) {
        account.order = Some(order.as_i64().ok_or(Refusal::BadRequest).map_err(bad)?);
    }
    if let Some(pinned) = flag(body, "pinned").map_err(bad)? {
        account.pinned = Some(pinned);
    }
    if let Some(none) = flag(body, "no_backup_codes").map_err(bad)? {
        account.no_backup_codes = Some(none);
    }
    if !well_made(&account, false) || (account.issuer.is_empty() && account.label.is_empty()) {
        return Err(Next::refuse(Refusal::BadRequest));
    }
    account.updated_at = Some(now_ms());
    let store = service.store_of(uid, vault).map_err(Next::refuse)?;
    let approval = store.entry(KIND_ACCOUNT, id).map_or(Approval::WhileOpen, |entry| entry.approval);
    keep(service, uid, vault, &account, approval)?;
    Ok(Next::Answer(json!({"ok": true, "id": id})))
}

/// Order, pins and tags say nothing and change nothing a prompt shows, so they ask nobody. A name is what prompts show, and a program
/// that could change it quietly could have a person approve under another name.
pub fn update(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let id = item_id(body)?;
    let body = body.clone();
    let asks = ["issuer", "label", "note"].iter().any(|field| body.get(field).is_some());
    let id_there = id.clone();
    if asks {
        return Ok(service.with_approval(uid, KIND_ACCOUNT, &id_there, Action::Use, say::CHANGE_ITEM, false, move |service, vault| flat(apply_update(service, uid, vault, &id, &body))));
    }
    let (vault, _, _) = service.locate(uid, KIND_ACCOUNT, &id, None)?;
    Ok(flat(apply_update(service, uid, &vault, &id, &body)))
}

pub fn note(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let id = item_id(body)?;
    let id_there = id.clone();
    Ok(service.with_approval(uid, KIND_ACCOUNT, &id_there, Action::Use, say::USE_ITEM, false, move |service, vault| {
        flat(read::<Account>(service, uid, vault, KIND_ACCOUNT, &id).map(|account| Next::Answer(json!({"ok": true, "note": account.note.as_ref().map(|note| note.as_str())}))))
    }))
}

fn merge_now(service: &mut Service, uid: u32, target: &str, import: Imported, dry: bool, replace: bool) -> Result<Next, Next> {
    let store = service.store_of(uid, target).map_err(Next::refuse)?;
    let (accounts, services) = held(store).map_err(Next::trouble)?;
    let skipped = import.skipped;
    let now = now_ms();
    let (add_accounts, add_services, answer, gone) = if replace {
        let plan = import::plan_replace(&accounts, &services, import, now).map_err(|_| Next::refuse(Refusal::Refused))?;
        let answer = json!({"ok": true, "added": plan.add_accounts.len(), "added_services": plan.add_services.len(), "removed": plan.remove_accounts.len() + plan.remove_services.len(), "skipped": skipped, "dry": dry});
        (plan.add_accounts, plan.add_services, answer, Some((plan.remove_accounts, plan.remove_services)))
    } else {
        let plan = import::plan_merge(&accounts, &services, import, now).map_err(|_| Next::refuse(Refusal::Refused))?;
        let answer = json!({"ok": true, "added": plan.accounts.len(), "added_services": plan.services.len(), "same": plan.duplicate_accounts, "same_services": plan.duplicate_services, "duplicate": plan.duplicate, "skipped": skipped, "dry": dry});
        (plan.accounts, plan.services, answer, None)
    };
    if dry {
        return Ok(Next::Answer(answer));
    }
    let mut written = Vec::with_capacity(add_accounts.len() + add_services.len());
    for account in &add_accounts {
        let meta = meta_of(account);
        written.push((KIND_ACCOUNT, account.id.clone(), name_of(account), meta, serde_json::to_value(account).map_err(|_| Next::refuse(Refusal::Failed))?));
    }
    for service_item in &add_services {
        let meta = backup::meta_of(service_item);
        written.push((KIND_BACKUP_CODES, service_item.id.clone(), backup::name_of(service_item), meta, serde_json::to_value(service_item).map_err(|_| Next::refuse(Refusal::Failed))?));
    }
    if written.iter().any(|(_, _, _, meta, _)| !meta_fits(meta)) {
        return Err(Next::refuse(Refusal::TooLarge));
    }
    let listed: usize = store.entries().map(|entry| entry.name.len() + entry.meta.to_string().len()).sum::<usize>() + written.iter().map(|(_, _, name, meta, _)| name.len() + meta.to_string().len()).sum::<usize>();
    if store.entries().count() + written.len() > MAX_ITEMS || listed > MAX_LISTED_BYTES {
        return Err(Next::refuse(Refusal::TooLarge));
    }
    let (old_accounts, old_services) = gone.unwrap_or_default();
    let old: Vec<(&str, &str)> = old_accounts.iter().map(|id| (KIND_ACCOUNT, id.as_str())).chain(old_services.iter().map(|id| (KIND_BACKUP_CODES, id.as_str()))).collect();
    let touched: Vec<(&str, &str)> = written.iter().map(|(kind, id, ..)| (*kind, id.as_str())).chain(old.iter().copied()).collect();
    let extra = written.iter().map(|(_, _, name, meta, value)| extra_of(name, meta, value)).sum();
    // What is new first, then what goes: a crash leaves both, never neither
    service
        .write_many_in(uid, target, &touched, extra, |store| {
            store.put_many(written.iter().map(|(kind, id, name, meta, value)| NewItem { kind, id, name, meta, approval: Approval::WhileOpen, value }))?;
            if old.is_empty() { Ok(()) } else { store.remove_many(&old) }
        })
        .map_err(Next::trouble)?;
    for (kind, id) in &old {
        service.forget_approved(uid, target, kind, id);
    }
    service.touch(uid);
    give_back();
    Ok(Next::Answer(answer))
}

/// Finished items from an import the client parsed. The same account is the same issuer and secret, and only the service holds
/// the secrets, so this is the only place that can say. Merging only adds, as `put` does, and asks nobody; a dry run says what it would do; a
/// replace takes something away for good, so it asks.
pub fn merge(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let vault = vault_of(body)?;
    let dry = flag(body, "dry")?.unwrap_or(false);
    let replace = flag(body, "replace")?.unwrap_or(false);
    let source = match optional_text(body, "source")? {
        Some("backmycodes") => Source::BackMyCodes,
        Some(_) | None => Source::Links,
    };
    let list = |name: &str| -> Result<&[Value], Refusal> {
        match body.get(name) {
            None | Some(Value::Null) => Ok(&[]),
            Some(Value::Array(list)) if list.len() <= MOST_AT_ONCE => Ok(list),
            Some(Value::Array(_)) => Err(Refusal::TooLarge),
            Some(_) => Err(Refusal::BadRequest),
        }
    };
    let mut skipped = 0;
    let mut accounts = Vec::new();
    for value in list("accounts")? {
        match serde_json::from_value::<Account>(value.clone()) {
            Ok(account) if well_made(&account, false) => accounts.push(account),
            Ok(_) | Err(_) => skipped += 1,
        }
    }
    let mut services = Vec::new();
    for value in list("services")? {
        match serde_json::from_value::<BackupService>(value.clone()) {
            Ok(service_item) if backup::well_made(&service_item) => services.push(service_item),
            Ok(_) | Err(_) => skipped += 1,
        }
    }
    let import = Imported { source, accounts, services, skipped, plain: false };
    let target = service.open_store(uid, vault.as_deref())?.id().to_string();
    if replace && !dry {
        return Ok(Next::approve(Action::Use, say::REPLACE_ITEMS, "", move |service, peer| flat(merge_now(service, peer.uid, &target, import, dry, replace))));
    }
    Ok(flat(merge_now(service, uid, &target, import, dry, replace)))
}

/// Every account and service of a vault, as BackMyCodes' file or its QR transfer, always under a password. One scrypt for the
/// file, in the job.
pub fn export(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let vault = vault_of(body)?;
    let format = text(body, "format")?;
    if format != "file" && format != "qr" {
        return Err(Refusal::BadRequest);
    }
    let format = format.to_string();
    let password = match passphrase(body, "password")? {
        Ok(password) => password,
        Err(next) => return Ok(next),
    };
    let target = service.open_store(uid, vault.as_deref())?.id().to_string();
    let program = service.config.worker.clone();
    Ok(Next::approve(Action::Export, say::EXPORT_ALL, "", move |service, peer| {
        let store = match service.store_of(peer.uid, &target) {
            Ok(store) => store,
            Err(why) => return Next::refuse(why),
        };
        let (accounts, services) = match held(store) {
            Ok(held) => held,
            Err(trouble) => return Next::trouble(trouble),
        };
        let request = json!({"job": "auth-export", "format": format, "password": password.as_str(), "now": now_ms(), "accounts": accounts, "services": services});
        give_back();
        Next::work(
            move || work::run(&program, &request),
            |_, _, answer: Value| {
                give_back();
                // What cannot go down one line is refused rather than cut
                if answer.to_string().len() > MAX_LINE { Next::refuse(Refusal::TooLarge) } else { Next::Answer(answer) }
            },
        )
    }))
}
