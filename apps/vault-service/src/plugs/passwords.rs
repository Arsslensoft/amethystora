//! Passwords. They are put and got through the generic `put` and `get`, whose value this does not read: only making one, handing
//! over the current one alone, and saying which items share one are here.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use serde_json::{json, Value};

use amethystora_vault_core::generate::{self, Charsets, DEFAULT_LENGTH, DEFAULT_WORDS, MAX_LENGTH, MAX_WORDS, MIN_LENGTH, MIN_WORDS};
use amethystora_vault_core::items::{Password, KIND_PASSWORD};
use amethystora_vault_core::protocol::{encode, Refusal};

use super::{flag, flat, give_back, item_id, must_be_open, number, optional_text, read};
use crate::outside::{say, Action};
use crate::peer::Peer;
use crate::service::{Next, Service};

/// Touches no vault, so it needs none open: it is only random numbers
pub fn generate(_: &mut Service, _: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let made = match optional_text(body, "style")?.unwrap_or("random") {
        "random" => {
            let length = number(body, "length", MIN_LENGTH as u64..=MAX_LENGTH as u64)?.map_or(DEFAULT_LENGTH, |length| length as usize);
            let chosen = |name| flag(body, name).map(|chosen| chosen.unwrap_or(true));
            let sets = Charsets { lower: chosen("lower")?, upper: chosen("upper")?, digits: chosen("digits")?, symbols: chosen("symbols")? };
            generate::password(length, sets)
        }
        "words" => {
            let count = number(body, "count", MIN_WORDS as u64..=MAX_WORDS as u64)?.map_or(DEFAULT_WORDS, |count| count as usize);
            let separator = match optional_text(body, "separator")? {
                None => '-',
                Some(text) => {
                    let mut characters = text.chars();
                    match (characters.next(), characters.next()) {
                        (Some(one), None) if !one.is_control() => one,
                        _ => return Err(Refusal::BadRequest),
                    }
                }
            };
            generate::words(count, separator)
        }
        _ => return Err(Refusal::BadRequest),
    };
    let made = made.map_err(|_| Refusal::BadRequest)?;
    Ok(Next::Answer(json!({"ok": true, "secret": encode(made.password.as_str().as_bytes()), "bits": made.bits})))
}

/// The current password and nothing beside it, marked as one: a window copies it so that clipboard histories leave it alone
pub fn copy(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let id = item_id(body)?;
    let id_there = id.clone();
    Ok(service.with_approval(uid, KIND_PASSWORD, &id_there, Action::Use, say::USE_ITEM, false, move |service, vault| {
        flat(read::<Password>(service, uid, vault, KIND_PASSWORD, &id).map(|item| Next::Answer(json!({"ok": true, "secret": encode(item.password.as_str().as_bytes()), "copy": "secret", "vault": vault}))))
    }))
}

/// Which items of the open vaults hold one password, for the window's warning, among the items a program could read with no
/// question (set to never ask, or already said yes to while the vault is open). It asks nobody and tells nothing a program could
/// not read: over every item it would be an equality oracle, since a program may put a guess as a new item with no question and
/// see whether it joins an existing one's group. A new item asks on its first read, so a guess never takes part. The warning
/// therefore covers the items the person has used this time the vault was open.
pub fn reused(service: &mut Service, peer: &Rc<Peer>, _: &Value) -> Result<Next, Refusal> {
    let uid = peer.uid;
    must_be_open(service, uid)?;
    let readable: HashSet<(String, String)> = service.readable_unasked(uid, KIND_PASSWORD).into_iter().collect();
    let mut held: HashMap<String, Vec<String>> = HashMap::new();
    for store in service.open_stores(uid) {
        let vault = store.id().to_string();
        let ids: Vec<String> = store.entries().filter(|entry| entry.kind == KIND_PASSWORD).map(|entry| entry.id.clone()).filter(|id| readable.contains(&(vault.clone(), id.clone()))).collect();
        for id in ids {
            let Some(item) = store.get(KIND_PASSWORD, &id).ok().and_then(|value| serde_json::from_value::<Password>(value).ok()) else { continue };
            if !item.password.is_empty() {
                held.entry(item.password.as_str().to_string()).or_default().push(id);
            }
        }
    }
    let groups: Vec<Vec<String>> = held.into_values().filter(|ids| ids.len() > 1).collect();
    give_back();
    Ok(Next::Answer(json!({"ok": true, "groups": groups})))
}
