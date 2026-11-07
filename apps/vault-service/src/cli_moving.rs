//! `ame vault backup` and `restore FILE` (plans/PLAN-vault.md, Backup and moving).
//!
//! A backup is the account's vaults as they lie sealed in the service's folder, with the recovery records and nothing a password
//! or this machine's chip opens (`apps/vault-core/src/moving.rs`). The service hands it over a chunk at a time, with no vault open
//! and no approval, and this writes it to `~/.local/share/amethystora/vault-backup/vault-DATE.avb` as the account, seven kept.
//! The machine's root is not in it. `amethystora-backup` runs this before each restic run.

use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;

use amethystora_vault_client::MAX_DATA;
use amethystora_vault_core::moving;
use amethystora_vault_core::protocol::encode;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use super::items::input_of;
use super::{call, new_password, read_secret, Args, USAGE};

/// How many are kept
const KEPT: usize = 7;
/// What one chunk of an upload carries
const CHUNK: usize = MAX_DATA / 4;

/// The account has a vault: the folder its backups go in is made, which is the mark the daily backup and the report go by, so that
/// neither starts the service for an account that has none
pub fn mark() {
    if let Ok(folder) = folder() {
        let _ = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(folder);
    }
}

fn folder() -> Result<PathBuf, u8> {
    let data = std::env::var_os("XDG_DATA_HOME").filter(|path| !path.is_empty()).map(PathBuf::from).or_else(|| std::env::var_os("HOME").filter(|home| !home.is_empty()).map(|home| PathBuf::from(home).join(".local/share"))).ok_or(6u8)?;
    Ok(data.join("amethystora").join("vault-backup"))
}

/// Today's date where the person is, as the file's name has it
fn today() -> String {
    // SAFETY: time and localtime_r write only into the values given, which this owns
    let (year, month, day) = unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut local: libc::tm = std::mem::zeroed();
        libc::localtime_r(&raw const now, &raw mut local);
        (local.tm_year + 1900, local.tm_mon + 1, local.tm_mday)
    };
    format!("{year:04}-{month:02}-{day:02}")
}

/// The whole backup, read until it adds up: a vault written between two chunks makes a copy that does not, and it is read again
fn fetch() -> Result<Vec<u8>, u8> {
    for _ in 0..3 {
        let mut bytes = Vec::new();
        loop {
            let answer = call(json!({"op": "vault-backup", "offset": bytes.len()}))?;
            let size = answer["size"].as_u64().ok_or(6u8)? as usize;
            let chunk = decoded(&answer["data"])?;
            bytes.extend_from_slice(&chunk);
            if bytes.len() >= size || chunk.is_empty() {
                break;
            }
            if bytes.len() > moving::MAX_BACKUP + 32 {
                return Err(4);
            }
        }
        if moving::parse(&bytes).is_some() {
            return Ok(bytes);
        }
    }
    Err(6)
}

fn decoded(value: &Value) -> Result<Vec<u8>, u8> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(value.as_str().ok_or(6u8)?).map_err(|_| 6u8)
}

pub fn backup(args: &Args) -> Result<(), u8> {
    if args.word(1).is_some() {
        return Err(USAGE);
    }
    let bytes = fetch()?;
    let folder = folder()?;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&folder).map_err(|_| 6u8)?;
    let file = folder.join(format!("vault-{}.avb", today()));
    let temporary = folder.join(format!(".writing-{}", std::process::id()));
    let written = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&temporary).and_then(|mut handle| {
        handle.write_all(&bytes)?;
        handle.sync_all()
    });
    if written.is_err() || std::fs::rename(&temporary, &file).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(6);
    }
    // The oldest go once there are more than seven; the date in the name sorts as the time does
    let mut kept: Vec<_> = std::fs::read_dir(&folder).into_iter().flatten().flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).filter(|name| name.starts_with("vault-") && name.ends_with(".avb")).collect();
    kept.sort();
    while kept.len() > KEPT {
        let _ = std::fs::remove_file(folder.join(kept.remove(0)));
    }
    println!("{}", file.display());
    Ok(())
}

pub fn restore(args: &Args) -> Result<(), u8> {
    let path = args.word(1).ok_or(USAGE)?;
    if args.word(2).is_some() {
        return Err(USAGE);
    }
    if std::fs::metadata(path).is_err_and(|why| why.kind() == std::io::ErrorKind::NotFound) {
        return Err(11);
    }
    let file = input_of(Some(path))?;
    if file.len() > moving::MAX_BACKUP + 32 || moving::parse(&file).is_none() {
        return Err(5);
    }
    let recovery = read_secret(&args.say("say-restore-key", "Recovery key of the vault to bring back: ")).ok_or(1u8)?;
    for (at, chunk) in file.chunks(CHUNK).enumerate() {
        call(json!({"op": "vault-restore-put", "offset": at * CHUNK, "data": encode(chunk)}))?;
    }
    let answer = call(json!({"op": "vault-restore", "recovery": encode(recovery.as_bytes())}))?;
    let id = answer["ids"][0].as_str().ok_or(6u8)?.to_string();
    println!("{}", args.say("say-restored", "The vault is back. Choose how it opens now."));
    // It came back with no password: the recovery key opens it once, and the new way in is set while it is open
    call(json!({"op": "unlock", "method": "recovery", "secret": encode(recovery.as_bytes())}))?;
    let password: Zeroizing<String> = new_password(args)?;
    call(json!({"op": "change-password", "vault": id, "secret": encode(password.as_bytes())})).map(|_| ())
}
