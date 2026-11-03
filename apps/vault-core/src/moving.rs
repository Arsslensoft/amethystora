//! The sealed backup of an account's vaults, and putting one back (plans/PLAN-vault.md, Backup and moving).
//!
//! It holds only what is already sealed: every vault's files as they lie on the disk, and the table of **recovery** records. The
//! password table, the slots on the chip or a security key, the machine's root and the `background` key are not in it, so a stolen
//! copy has no password to guess and nothing machine-bound: only the recovery key (256 random bits) opens a vault out of it.
//! It lists every vault of the account, hidden ones included, in one shape, in the order of their random ids: nothing in it says
//! which is hidden, and only its size can say how many there are, which the manual says.
//!
//! ```text
//! "AVB1" | u16 records | records (RECORD bytes each) | u16 vaults |
//!   per vault: id (16 bytes) | u16 files | per file: u8 name length | name | u32 length | bytes
//! | SHA-256 of all of it
//! ```
//!
//! A file's name is `meta`, `index` or `items/<kind>.<id>`, checked as the store checks them. The trailing sum is not security, the
//! seals are: it is how a copy read while the service wrote, or cut short, is told from one that is whole.

use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::protocol::{valid_hex_id, valid_id, valid_kind};
use crate::seal;
use crate::slots::{self, RecoveryKey, Record, Table, MAX_VAULTS, RECORD};
use crate::store::{make_folder, read_bounded, sync_folder, Store, Trouble, MAX_ITEMS};
use crate::vaults::{encode_record, Account, Listed, QUOTA};

/// The most a backup holds. The disk is shared, and an account's quota is 256 MiB of which a vault of real use is a small part.
pub const MAX_BACKUP: usize = 48 * 1024 * 1024;
const MAX_FILE: u64 = 16 * 1024 * 1024;
const MAGIC: &[u8; 4] = b"AVB1";
/// An item, the index, the meta
const MAX_FILES: usize = MAX_ITEMS + 2;

pub struct ArchivedVault {
    /// Hex, as the folder is named
    pub id: String,
    pub files: Vec<(String, Vec<u8>)>,
}

pub struct Archive {
    pub recoveries: Vec<Record>,
    pub vaults: Vec<ArchivedVault>,
}

/// A file of a vault, by name: the vault's own two and its items
fn valid_file(name: &str) -> bool {
    match name.strip_prefix("items/") {
        Some(item) => item.split_once('.').is_some_and(|(kind, id)| valid_kind(kind) && valid_id(id)),
        None => name == "meta" || name == "index",
    }
}

/// The account's vaults, sealed as they are on the disk. Needs no vault open and no approval: there is nothing in it to learn.
pub fn make(account: &Account) -> Result<Vec<u8>, Trouble> {
    let mut ids: Vec<String> = fs::read_dir(account.vaults_folder())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| valid_hex_id(name))
        .collect();
    ids.sort();
    let records = account.records(Table::Recovery);
    if records.len() > MAX_VAULTS || ids.len() > MAX_VAULTS {
        return Err(Trouble::Damaged);
    }
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(records.len() as u16).to_be_bytes());
    for record in &records {
        out.extend_from_slice(record);
    }
    out.extend_from_slice(&(ids.len() as u16).to_be_bytes());
    for id in &ids {
        let folder = account.folder_of(id);
        let mut files: Vec<(String, Vec<u8>)> = Vec::new();
        for name in ["meta", "index"] {
            match read_bounded(&folder.join(name), MAX_FILE) {
                Ok(bytes) => files.push((name.to_string(), bytes)),
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(_) => return Err(Trouble::Disk),
            }
        }
        let mut items: Vec<String> = fs::read_dir(folder.join("items")).into_iter().flatten().flatten().filter_map(|entry| entry.file_name().into_string().ok()).collect();
        items.sort();
        for item in items {
            let name = format!("items/{item}");
            if !valid_file(&name) {
                continue;
            }
            files.push((name, read_bounded(&folder.join("items").join(&item), MAX_FILE).map_err(|_| Trouble::Disk)?));
        }
        if files.len() > MAX_FILES {
            return Err(Trouble::Full);
        }
        out.extend_from_slice(&seal::unhex(id).ok_or(Trouble::Damaged)?);
        out.extend_from_slice(&(files.len() as u16).to_be_bytes());
        for (name, bytes) in files {
            out.push(name.len() as u8);
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            out.extend_from_slice(&bytes);
        }
        if out.len() > MAX_BACKUP {
            return Err(Trouble::Full);
        }
    }
    let sum = Sha256::digest(&out);
    out.extend_from_slice(&sum);
    Ok(out)
}

/// A backup read back, every number bounded before it is used. Anything that is not exactly what [`make`] writes is refused.
pub fn parse(bytes: &[u8]) -> Option<Archive> {
    if bytes.len() > MAX_BACKUP + 32 || bytes.len() < MAGIC.len() + 2 + 2 + 32 {
        return None;
    }
    let (body, sum) = bytes.split_at(bytes.len() - 32);
    if !seal::same(&Sha256::digest(body), sum) {
        return None;
    }
    let mut at = Reader { bytes: body, at: 0 };
    if at.take(MAGIC.len())? != MAGIC {
        return None;
    }
    let count = usize::from(at.u16()?);
    if count > MAX_VAULTS {
        return None;
    }
    let mut recoveries = Vec::with_capacity(count);
    for _ in 0..count {
        recoveries.push(Record::try_from(at.take(RECORD)?).ok()?);
    }
    let count = usize::from(at.u16()?);
    if count > MAX_VAULTS {
        return None;
    }
    let mut vaults: Vec<ArchivedVault> = Vec::with_capacity(count);
    for _ in 0..count {
        let id = seal::hex(at.take(16)?);
        if vaults.iter().any(|vault| vault.id == id) {
            return None;
        }
        let files_count = usize::from(at.u16()?);
        if files_count > MAX_FILES {
            return None;
        }
        let mut files: Vec<(String, Vec<u8>)> = Vec::with_capacity(files_count.min(64));
        for _ in 0..files_count {
            let length = usize::from(at.take(1)?[0]);
            let name = String::from_utf8(at.take(length)?.to_vec()).ok().filter(|name| valid_file(name))?;
            if files.iter().any(|(held, _)| *held == name) {
                return None;
            }
            let size = u32::from_be_bytes(at.take(4)?.try_into().ok()?) as usize;
            if size as u64 > MAX_FILE {
                return None;
            }
            files.push((name, at.take(size)?.to_vec()));
        }
        vaults.push(ArchivedVault { id, files });
    }
    if at.at != body.len() {
        return None;
    }
    Some(Archive { recoveries, vaults })
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(count).filter(|end| *end <= self.bytes.len())?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Some(taken)
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_be_bytes(self.take(2)?.try_into().ok()?))
    }
}

/// Why a restore did nothing
#[derive(Debug, PartialEq, Eq)]
pub enum Failed {
    /// The recovery key opens nothing in this backup
    NoMatch,
    /// A vault by that id is here already, or a second first vault would come of it
    Exists,
    /// A vault in the backup does not open under its key
    Damaged,
    /// The account has no room for it
    Full,
    Disk,
}

fn write_new(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut handle = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(file)?;
    handle.write_all(bytes)?;
    handle.sync_all()
}

/// Puts back the vaults this recovery key opens (one, as a vault's key is its own): its folder, its recovery record, and a
/// listing unless it was hidden. It comes back with no password, no slot and no `background` key: those are this machine's to
/// make again, and the recovery key opens it to do that. `kdf` is the account's salt, if it has none yet. Returns the ids.
pub fn restore(state: &Path, uid: u32, archive: &Archive, recovery: &RecoveryKey, kdf: impl FnOnce() -> slots::Kdf) -> Result<Vec<String>, Failed> {
    let found = slots::search(&recovery.key(), &archive.recoveries, Table::Recovery);
    if found.is_empty() {
        return Err(Failed::NoMatch);
    }
    let mut account = match Account::load(state, uid).map_err(|_| Failed::Disk)? {
        Some(account) => account,
        None => Account::empty(state, uid, kdf()),
    };
    let brought: usize = archive.vaults.iter().filter(|vault| found.iter().any(|f| f.vault == vault.id)).flat_map(|vault| &vault.files).map(|(_, bytes)| bytes.len()).sum();
    if account.disk_use() + brought as u64 > QUOTA {
        return Err(Failed::Full);
    }
    let vaults = account.vaults_folder();
    make_folder(&vaults).map_err(|_| Failed::Disk)?;
    let mut ids = Vec::new();
    for one in found {
        let vault = archive.vaults.iter().find(|vault| vault.id == one.vault).ok_or(Failed::Damaged)?;
        let record = archive.recoveries.iter().find(|record| seal::hex(&record[..12]) == one.record).ok_or(Failed::Damaged)?;
        let folder = account.folder_of(&vault.id);
        if folder.exists() || !account.has_room() {
            return Err(if folder.exists() { Failed::Exists } else { Failed::Full });
        }
        // Beside its place first, opened under its key to prove it is whole, then renamed in
        let temporary = vaults.join(format!(".restoring-{}", seal::random_hex(8)));
        let outcome = (|| {
            make_folder(&temporary.join("items")).map_err(|_| Failed::Disk)?;
            for (name, bytes) in &vault.files {
                write_new(&temporary.join(name), bytes).map_err(|_| Failed::Disk)?;
            }
            let mut store = Store::open(&temporary, &vault.id, one.key.copy()).map_err(|_| Failed::Damaged)?;
            if store.meta.first && account.first().is_some() {
                return Err(Failed::Exists);
            }
            // The password it had was this machine's account's, and is not in the backup
            store.meta.password = None;
            store.write_meta().map_err(|_| Failed::Disk)?;
            fs::rename(&temporary, &folder).map_err(|_| Failed::Disk)?;
            sync_folder(&vaults).map_err(|_| Failed::Disk)?;
            Ok(store)
        })();
        let store = match outcome {
            Ok(store) => store,
            Err(why) => {
                let _ = fs::remove_dir_all(&temporary);
                return Err(why);
            }
        };
        account.file.recoveries.push(encode_record(record));
        if !store.meta.hidden {
            account.file.listed.push(Listed { id: vault.id.clone(), first: store.meta.first, password: false, slots: Vec::new(), created: store.meta.created });
        }
        if account.save().is_err() {
            let _ = fs::remove_dir_all(&folder);
            return Err(Failed::Disk);
        }
        ids.push(vault.id.clone());
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Approval;
    use crate::slots::Kdf;
    use crate::vaults::Draft;
    use serde_json::{json, Value};

    struct Folder(std::path::PathBuf);

    impl Folder {
        fn new(name: &str) -> Folder {
            let path = std::env::temp_dir().join(format!("amethystora-vault-moving-{name}-{}", seal::random_hex(6)));
            fs::create_dir_all(&path).unwrap();
            Folder(path)
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const UID: u32 = 1000;

    fn make_vault(state: &Path, name: &str, hidden: bool, first: bool, password: &[u8]) -> (String, RecoveryKey) {
        let kdf = Account::load(state, UID).unwrap().map_or_else(|| Kdf::fresh(10), |account| account.file.kdf.clone());
        let under = slots::derive(password, &kdf).unwrap();
        let draft = Draft::new(name, hidden, first, kdf, Some(&under)).unwrap();
        let text = draft.recovery.text();
        let mut store = draft.commit(state, UID).unwrap();
        store.put("password", "p1", "mail", &Value::Null, Approval::Ask, &json!({"secret": format!("in {name}")})).unwrap();
        (store.id().to_string(), RecoveryKey::parse(&text).unwrap())
    }

    #[test]
    fn everything_comes_back_from_the_recovery_key_alone() {
        let state = Folder::new("round");
        let (first, first_key) = make_vault(&state.0, "Personal", false, true, b"personal password");
        let (hidden, hidden_key) = make_vault(&state.0, "Not listed", true, false, b"hidden password");
        let archive = make(&Account::load(&state.0, UID).unwrap().unwrap()).unwrap();
        // No password, slot or name of a vault is in it
        let text = String::from_utf8_lossy(&archive);
        assert!(!text.contains("Personal") && !text.contains("Not listed") && !text.contains("personal password"));
        // Gone, as when forget is run or the disk is new
        Account::forget_everything(&state.0, UID).unwrap();
        assert!(Account::load(&state.0, UID).unwrap().is_none());
        let parsed = parse(&archive).unwrap();
        assert_eq!(parsed.vaults.len(), 2);
        // A key that opens nothing here changes nothing
        let stranger = RecoveryKey::generate();
        assert_eq!(restore(&state.0, UID, &parsed, &stranger, || Kdf::fresh(10)), Err(Failed::NoMatch));
        assert!(Account::load(&state.0, UID).unwrap().is_none());
        // The first vault returns listed, with no password
        assert_eq!(restore(&state.0, UID, &parsed, &first_key, || Kdf::fresh(10)).unwrap(), vec![first.clone()]);
        let account = Account::load(&state.0, UID).unwrap().unwrap();
        assert_eq!(account.file.listed.len(), 1);
        assert!(account.first().is_some_and(|listed| listed.id == first && !listed.password));
        assert!(account.records(Table::Password).is_empty());
        let found = slots::search(&first_key.key(), &account.records(Table::Recovery), Table::Recovery);
        let mut store = Store::open(&account.folder_of(&first), &first, found[0].key.copy()).unwrap();
        assert!(store.meta.password.is_none() && store.meta.first);
        assert_eq!(store.get("password", "p1").unwrap(), json!({"secret": "in Personal"}));
        // The hidden one is still not listed, and the same twice is refused
        assert_eq!(restore(&state.0, UID, &parsed, &first_key, || Kdf::fresh(10)), Err(Failed::Exists));
        assert_eq!(restore(&state.0, UID, &parsed, &hidden_key, || Kdf::fresh(10)).unwrap(), vec![hidden.clone()]);
        let account = Account::load(&state.0, UID).unwrap().unwrap();
        assert_eq!(account.file.listed.len(), 1);
        assert_eq!(account.records(Table::Recovery).len(), 2);
        let text = fs::read_to_string(state.0.join(UID.to_string()).join("account")).unwrap();
        assert!(!text.contains(&hidden));
    }

    #[test]
    fn a_second_first_vault_is_not_made_by_a_restore() {
        let state = Folder::new("second-first");
        let (_, first_key) = make_vault(&state.0, "Personal", false, true, b"personal password");
        let archive = make(&Account::load(&state.0, UID).unwrap().unwrap()).unwrap();
        Account::forget_everything(&state.0, UID).unwrap();
        make_vault(&state.0, "Another first", false, true, b"another password");
        let parsed = parse(&archive).unwrap();
        assert_eq!(restore(&state.0, UID, &parsed, &first_key, || Kdf::fresh(10)), Err(Failed::Exists));
        // And nothing was left beside the vaults
        let account = Account::load(&state.0, UID).unwrap().unwrap();
        assert_eq!(fs::read_dir(account.vaults_folder()).unwrap().count(), 1);
    }

    #[test]
    fn a_damaged_or_cut_short_backup_is_refused() {
        let state = Folder::new("damaged");
        make_vault(&state.0, "Personal", false, true, b"personal password");
        let archive = make(&Account::load(&state.0, UID).unwrap().unwrap()).unwrap();
        assert!(parse(&archive).is_some());
        for length in [0, 4, archive.len() / 2, archive.len() - 1] {
            assert!(parse(&archive[..length]).is_none(), "{length}");
        }
        for at in [0, 10, archive.len() / 2, archive.len() - 1] {
            let mut flipped = archive.clone();
            flipped[at] ^= 1;
            assert!(parse(&flipped).is_none(), "{at}");
        }
        let mut longer = archive.clone();
        longer.push(0);
        assert!(parse(&longer).is_none());
    }

    #[test]
    fn a_file_name_that_is_a_path_is_refused_even_with_a_right_sum() {
        let mut body = Vec::new();
        body.extend_from_slice(MAGIC);
        body.extend_from_slice(&0u16.to_be_bytes());
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&[7u8; 16]);
        body.extend_from_slice(&1u16.to_be_bytes());
        let name = b"items/../account";
        body.push(name.len() as u8);
        body.extend_from_slice(name);
        body.extend_from_slice(&0u32.to_be_bytes());
        let sum = Sha256::digest(&body);
        body.extend_from_slice(&sum);
        assert!(parse(&body).is_none());
    }
}
