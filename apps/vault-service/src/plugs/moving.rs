//! The sealed backup and its restore (plans/PLAN-vault.md, Backup and moving). A backup holds only what is sealed, so `vault-backup`
//! needs no vault open and no approval; it is read a chunk at a time, since the answer to a line is bounded and a vault may be
//! larger. A restore is the other way: `vault-restore-put` stages the file's chunks in memory, `vault-restore` puts the vaults back
//! that a recovery key opens, once the person has approved it as a new vault, and the client then opens one by the recovery key and sets the way in.

use std::rc::Rc;
use std::time::Instant;

use serde_json::{json, Value};

use amethystora_vault_core::moving::{self, Failed, MAX_BACKUP};
use amethystora_vault_core::protocol::{Refusal, MAX_DATA};
use amethystora_vault_core::slots::{Kdf, RecoveryKey};

use super::{base64_field, give_back, number};
use crate::outside::{say, Action};
use crate::peer::Peer;
use crate::service::{Next, Service};

/// A chunk, well inside what one line may carry
const CHUNK: usize = MAX_DATA / 2;

/// A backup's bytes from `offset`, and its whole size so that the client knows when it has it all. The file is made at the first
/// chunk and kept for the rest, so that a large vault is read once and not once a chunk on the loop every account waits on; a chunk
/// after it was let go makes it again, and the client's check of its sum at the end finds a copy that does not add up and asks again.
pub fn backup(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let offset = number(body, "offset", 0..=MAX_BACKUP as u64)?.unwrap_or(0) as usize;
    if offset == 0 || !service.made.contains_key(&peer.uid) {
        service.made.remove(&peer.uid);
        let account = service.account(peer.uid)?;
        let made = moving::make(&account).map_err(|trouble| if trouble == amethystora_vault_core::store::Trouble::Full { Refusal::TooLarge } else { Refusal::Failed })?;
        service.made.insert(peer.uid, (made, Instant::now()));
    }
    let Some((made, since)) = service.made.get_mut(&peer.uid) else { return Err(Refusal::Failed) };
    *since = Instant::now();
    let end = (offset + CHUNK).min(made.len());
    let chunk = made.get(offset..end).unwrap_or(&[]);
    let answer = json!({"ok": true, "size": made.len(), "data": amethystora_vault_core::protocol::encode(chunk)});
    if end >= made.len() {
        service.made.remove(&peer.uid);
        give_back();
    }
    Ok(Next::Answer(answer))
}

/// One chunk of a backup being brought in. Offset 0 starts again; any other has to be exactly where the last one ended.
pub fn put(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let offset = number(body, "offset", 0..=MAX_BACKUP as u64)?.ok_or(Refusal::BadRequest)? as usize;
    let data = base64_field(body, "data", CHUNK)?;
    if offset == 0 {
        service.staged.remove(&peer.uid);
    }
    let held = service.staged.get(&peer.uid).map_or(0, |(bytes, _)| bytes.len());
    if offset != held || held + data.len() > MAX_BACKUP + 32 {
        service.staged.remove(&peer.uid);
        return Err(Refusal::BadRequest);
    }
    let (bytes, since) = service.staged.entry(peer.uid).or_insert_with(|| (Vec::new(), Instant::now()));
    bytes.extend_from_slice(&data);
    *since = Instant::now();
    Ok(Next::Answer(json!({"ok": true, "held": bytes.len()})))
}

/// Puts back what the staged backup holds that this recovery key opens. A key that opens nothing is a refusal like any wrong one.
pub fn restore(service: &mut Service, peer: &Rc<Peer>, body: &Value) -> Result<Next, Refusal> {
    let typed = base64_field(body, "recovery", 256)?;
    // The upload is used once, whatever comes of it
    let (bytes, _) = service.staged.remove(&peer.uid).ok_or(Refusal::BadRequest)?;
    let archive = moving::parse(&bytes).ok_or(Refusal::BadRequest)?;
    drop(bytes);
    let recovery = std::str::from_utf8(&typed).ok().and_then(RecoveryKey::parse).ok_or(Refusal::Refused)?;
    // A restore makes vaults, so it is asked as a new vault is: without it, a program of the account could put back a vault
    // it holds the key to, open it by that key, and read what the person saves next in it
    let uid = peer.uid;
    Ok(Next::approve(Action::Open, say::NEW_VAULT, "", move |service, _| {
        let log_n = service.config.log_n;
        let restored = moving::restore(&service.config.state, uid, &archive, &recovery, || Kdf::fresh(log_n));
        drop(archive);
        give_back();
        match restored {
            Ok(ids) => Next::Answer(json!({"ok": true, "vaults": ids.len(), "ids": ids})),
            Err(Failed::NoMatch | Failed::Exists) => Next::refuse(Refusal::Refused),
            Err(Failed::Damaged) => Next::refuse(Refusal::BadRequest),
            Err(Failed::Full) => Next::refuse(Refusal::TooLarge),
            Err(Failed::Disk) => Next::refuse(Refusal::Failed),
        }
    }))
}

