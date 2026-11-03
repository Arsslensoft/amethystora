//! An account's vaults: the first, any others listed, and any hidden, one scrypt salt between them.
//!
//! ```text
//! <state>/<uid>/account            the salt, the two tables of records, and the listed vaults with their slots
//! <state>/<uid>/tries              how many wrong passwords in a row, and until when none is tried
//! <state>/<uid>/vaults/<id>/       each vault's folder (`store`)
//! ```
//!
//! **A hidden vault is simply not listed.** The `account` file names the listed vaults; a hidden one is only a record in
//! each table, which is noise until the right password or recovery key opens it.
//! A typed password takes one derivation and [`MAX_VAULTS`] decryptions however many vaults there are, and nothing that
//! is said or logged depends on anything but what was typed. Root can count the records and the folders, which the manual
//! says: this keeps a vault from someone looking at the app or from a program asking, not from someone reading the disk.
//!
//! A vault's name is in its own sealed `meta` and nowhere else, a listed vault's too: while closed, a listed vault is its id,
//! how it opens, and whether it is the first.
//!
//! Everything but the folders' contents is in the one file, written atomically, so a vault exists from the moment that
//! file names it or holds its records.

use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::protocol::valid_hex_id;
use crate::seal::{self, Key, Unopened};
use crate::slots::{self, Chip, Kdf, Record, RecoveryKey, Slot, SlotKind, Table, MAX_VAULTS};
use crate::store::{self, make_folder, now, read_bounded, write_atomic, Meta, Store, Trouble};

const MAX_ACCOUNT_FILE: u64 = 1024 * 1024;
/// Security keys and chip slots of one vault
pub const MAX_SLOTS: usize = 16;
/// The idle time a new account starts with: the plan's 15 minutes
pub const IDLE_MINUTES: u32 = 15;
/// Wrong passwords in an hour that cost nothing but their own derivation. After them, every try at a password is put to the
/// person first, which a program guessing through the socket cannot answer, and the person always can: nothing locks them
/// out, and nothing a program does, a right password to a vault of its own included, gives the free tries back. Only an hour
/// gone by does, or a right password the person approved.
const FREE_TRIES: u32 = 5;
/// What one account's folder may take on the disk in all, every vault of it: /var is shared, and a program of the account can
/// write a megabyte at a time
pub const QUOTA: u64 = 256 * 1024 * 1024;
const TRIES_WINDOW: u64 = 3600;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Listed {
    pub id: String,
    /// The first vault holds the account's CA and its certificates and Round 7's data keys
    #[serde(default)]
    pub first: bool,
    /// Whether a password opens it. A vault set up for fingerprint or no prompt has none until one is added.
    #[serde(default)]
    pub password: bool,
    #[serde(default)]
    pub slots: Vec<Slot>,
    pub created: u64,
}

impl Listed {
    /// How it opens, in the words the protocol uses
    pub fn methods(&self) -> Vec<&'static str> {
        let mut methods = Vec::new();
        if self.password {
            methods.push("password");
        }
        for slot in &self.slots {
            if !methods.contains(&slot.kind.name()) {
                methods.push(slot.kind.name());
            }
        }
        methods.push("recovery");
        methods
    }
}

/// The key of the `background` purpose, on the chip, released while the vault is closed
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Background {
    pub id: String,
    pub sealed: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountFile {
    pub(crate) format: u32,
    pub kdf: Kdf,
    pub(crate) passwords: Vec<String>,
    pub(crate) recoveries: Vec<String>,
    pub listed: Vec<Listed>,
    #[serde(default = "default_idle")]
    pub idle_minutes: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Background>,
}

fn default_idle() -> u32 {
    IDLE_MINUTES
}

pub(crate) fn encode_record(record: &Record) -> String {
    base64::engine::general_purpose::STANDARD.encode(record)
}

fn decode_record(text: &str) -> Option<Record> {
    base64::engine::general_purpose::STANDARD.decode(text).ok()?.try_into().ok()
}

pub(crate) fn record_nonce(record: &Record) -> String {
    seal::hex(&record[..12])
}

fn sound(file: &AccountFile) -> bool {
    let table = |entries: &[String]| entries.len() <= MAX_VAULTS && entries.iter().all(|entry| decode_record(entry).is_some());
    file.format == 1
        && file.kdf.is_sound()
        && table(&file.passwords)
        && table(&file.recoveries)
        && file.listed.len() <= MAX_VAULTS
        && (1..=1440).contains(&file.idle_minutes)
        && file.listed.iter().all(|vault| valid_hex_id(&vault.id) && vault.slots.len() <= MAX_SLOTS && vault.slots.iter().all(|slot| valid_hex_id(&slot.id)))
}

/// An account's file, as read
pub struct Account {
    pub(crate) root: PathBuf,
    pub file: AccountFile,
}

pub fn account_folder(state: &Path, uid: u32) -> PathBuf {
    state.join(uid.to_string())
}

impl Account {
    /// None: never set up, or forgotten
    pub fn load(state: &Path, uid: u32) -> Result<Option<Account>, Trouble> {
        let root = account_folder(state, uid);
        let bytes = match read_bounded(&root.join("account"), MAX_ACCOUNT_FILE) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) if error.kind() == ErrorKind::InvalidData => return Err(Trouble::Damaged),
            Err(_) => return Err(Trouble::Disk),
        };
        let file: AccountFile = serde_json::from_slice(&bytes).map_err(|_| Trouble::Damaged)?;
        if !sound(&file) {
            return Err(Trouble::Damaged);
        }
        Ok(Some(Account { root, file }))
    }

    pub fn save(&self) -> Result<(), Trouble> {
        let text = serde_json::to_vec(&self.file).map_err(|_| Trouble::Disk)?;
        make_folder(&self.root).map_err(|_| Trouble::Disk)?;
        write_atomic(&self.root.join("account"), &text).map_err(|_| Trouble::Disk)
    }

    pub fn folder_of(&self, vault: &str) -> PathBuf {
        self.vaults_folder().join(vault)
    }

    /// Where every vault of the account is, hidden or not
    pub fn vaults_folder(&self) -> PathBuf {
        self.root.join("vaults")
    }

    /// An account with no vault yet, for the first one a restore brings back
    pub fn empty(state: &Path, uid: u32, kdf: Kdf) -> Account {
        Account { root: account_folder(state, uid), file: AccountFile { format: 1, kdf, passwords: Vec::new(), recoveries: Vec::new(), listed: Vec::new(), idle_minutes: IDLE_MINUTES, background: None } }
    }

    /// What the account's folder takes on the disk, every vault in it: counted once when the account is opened
    pub fn disk_use(&self) -> u64 {
        let mut total = 0;
        let mut folders = vec![self.root.clone()];
        while let Some(folder) = folders.pop() {
            for entry in fs::read_dir(folder).into_iter().flatten().flatten() {
                match entry.metadata() {
                    Ok(meta) if meta.is_dir() => folders.push(entry.path()),
                    Ok(meta) => total += meta.len(),
                    Err(_) => {}
                }
            }
        }
        total
    }

    pub fn first(&self) -> Option<&Listed> {
        self.file.listed.iter().find(|vault| vault.first)
    }

    pub fn listed(&self, vault: &str) -> Option<&Listed> {
        self.file.listed.iter().find(|listed| listed.id == vault)
    }

    pub fn listed_mut(&mut self, vault: &str) -> Option<&mut Listed> {
        self.file.listed.iter_mut().find(|listed| listed.id == vault)
    }

    fn table(&self, table: Table) -> &Vec<String> {
        match table {
            Table::Password => &self.file.passwords,
            Table::Recovery => &self.file.recoveries,
        }
    }

    fn table_mut(&mut self, table: Table) -> &mut Vec<String> {
        match table {
            Table::Password => &mut self.file.passwords,
            Table::Recovery => &mut self.file.recoveries,
        }
    }

    pub fn records(&self, table: Table) -> Vec<Record> {
        self.table(table).iter().filter_map(|entry| decode_record(entry)).collect()
    }

    /// Whether another vault can be added, hidden ones counting. Only what `new` is told, after an approval and a password.
    pub fn has_room(&self) -> bool {
        self.file.recoveries.len() < MAX_VAULTS && self.file.passwords.len() < MAX_VAULTS
    }

    /// Takes out the record with this nonce, if it is there
    pub fn remove_record(&mut self, table: Table, nonce: &str) {
        self.table_mut(table).retain(|entry| decode_record(entry).is_none_or(|record| record_nonce(&record) != nonce));
    }

    /// Takes a vault out of the file, and its folder off the disk. Only one that is open can name its records.
    pub fn remove_vault(&mut self, meta: &Meta, id: &str) -> Result<(), Trouble> {
        if let Some(nonce) = &meta.password {
            self.remove_record(Table::Password, nonce);
        }
        self.remove_record(Table::Recovery, &meta.recovery);
        self.file.listed.retain(|listed| listed.id != id);
        self.save()?;
        let folder = self.folder_of(id);
        fs::remove_dir_all(&folder).or_else(|error| if error.kind() == ErrorKind::NotFound { Ok(()) } else { Err(error) }).map_err(|_| Trouble::Disk)
    }

    /// Puts a new or a changed password's record in the table, in place of the old one if there was one
    pub fn set_password_record(&mut self, old: Option<&str>, record: &Record) -> String {
        if let Some(nonce) = old {
            self.remove_record(Table::Password, nonce);
        }
        self.file.passwords.push(encode_record(record));
        record_nonce(record)
    }

    /// Whether the next try at a password has to be put to the person first
    pub fn needs_approval(&self) -> bool {
        let tries = read_tries(&self.root);
        current(&tries, now()) && tries.fails >= FREE_TRIES
    }

    /// A wrong password, which derived and opened nothing
    pub fn note_failure(&self) {
        let mut tries = read_tries(&self.root);
        let at = now();
        if !current(&tries, at) {
            tries = Tries { fails: 0, since: at };
        }
        tries.fails = tries.fails.saturating_add(1);
        write_tries(&self.root, &tries);
    }

    /// A right password the person approved, or the recovery key: the count starts again
    pub fn note_success(&self) {
        let tries = read_tries(&self.root);
        if tries.fails > 0 {
            write_tries(&self.root, &Tries::default());
        }
    }

    /// The whole account, every vault of it, hidden or not. In one step: the rename is what happens, and what follows only
    /// tidies up.
    pub fn forget_everything(state: &Path, uid: u32) -> Result<(), Trouble> {
        let root = account_folder(state, uid);
        let gone = state.join(format!("{uid}.gone-{}", seal::random_hex(6)));
        match fs::rename(&root, &gone) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err(Trouble::Disk),
        }
        store::sync_folder(state).map_err(|_| Trouble::Disk)?;
        fs::remove_dir_all(&gone).map_err(|_| Trouble::Disk)
    }
}

/// What `forget_everything` did not get to finish, a power cut's doing: read once, when the service starts
pub fn sweep_forgotten(state: &Path) {
    let Ok(entries) = fs::read_dir(state) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.split_once(".gone-").is_some_and(|(uid, _)| !uid.is_empty() && uid.bytes().all(|byte| byte.is_ascii_digit())) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Tries {
    fails: u32,
    /// When the first of them was
    since: u64,
}

/// A window that began in the future began with a wrong clock: it counts as over, rather than for as long as the clock was wrong
fn current(tries: &Tries, at: u64) -> bool {
    tries.since <= at && at - tries.since < TRIES_WINDOW
}

fn read_tries(root: &Path) -> Tries {
    read_bounded(&root.join("tries"), 4096).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

fn write_tries(root: &Path, tries: &Tries) {
    // Not synced: a count lost to a power cut costs a guess or two, and a sync here would make a wrong password take longer
    // than a right one
    let temporary = root.join(format!(".tmp-tries-{}", seal::random_hex(4)));
    if let Ok(bytes) = serde_json::to_vec(tries) {
        let written = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary).and_then(|mut file| file.write_all(&bytes));
        if written.is_err() || fs::rename(&temporary, root.join("tries")).is_err() {
            let _ = fs::remove_file(&temporary);
        }
    }
}

/// A vault made and not yet kept: setup does not finish until the recovery key has been typed back
pub struct Draft {
    pub id: String,
    pub key: Key,
    pub recovery: RecoveryKey,
    pub name: String,
    pub hidden: bool,
    pub first: bool,
    pub kdf: Kdf,
    password_record: Option<Record>,
    recovery_record: Record,
    pub slots: Vec<Slot>,
}

impl Draft {
    /// A vault with a new key and a new recovery key, and a password's record if one was given (`under` is what it derived)
    pub fn new(name: &str, hidden: bool, first: bool, kdf: Kdf, under: Option<&Key>) -> Result<Draft, Unopened> {
        let id = seal::random_hex(16);
        let key = Key::random();
        let recovery = RecoveryKey::generate();
        let recovery_record = slots::seal_record(&recovery.key(), &id, &key, Table::Recovery).ok_or(Unopened)?;
        let password_record = match under {
            Some(under) => Some(slots::seal_record(under, &id, &key, Table::Password).ok_or(Unopened)?),
            None => None,
        };
        Ok(Draft { id, key, recovery, name: name.to_string(), hidden, first, kdf, password_record, recovery_record, slots: Vec::new() })
    }

    /// A slot on the chip, of a draft being set up for fingerprint or for no prompt
    pub fn add_chip_slot(&mut self, kind: SlotKind, chip: &dyn Chip) -> Result<(), Unopened> {
        let slot = Slot::for_chip(kind, chip, &self.key)?;
        self.slots.push(slot);
        Ok(())
    }

    /// Keeps it: the folder first, then the account's file, which is what makes it exist. A crash between them leaves a
    /// folder nothing names.
    pub fn commit(self, state: &Path, uid: u32) -> Result<Store, Trouble> {
        let mut account = match Account::load(state, uid)? {
            Some(account) => account,
            None => Account {
                root: account_folder(state, uid),
                file: AccountFile { format: 1, kdf: self.kdf.clone(), passwords: Vec::new(), recoveries: Vec::new(), listed: Vec::new(), idle_minutes: IDLE_MINUTES, background: None },
            },
        };
        // Two setups approved one after the other would make two firsts, and only one the desktop could find
        if !account.has_room() || (self.first && !account.file.recoveries.is_empty()) {
            return Err(Trouble::Damaged);
        }
        let created = now();
        let password = self.password_record.as_ref().map(record_nonce);
        let meta = Meta { name: self.name.clone(), hidden: self.hidden, first: self.first, password: password.clone(), recovery: record_nonce(&self.recovery_record), created };
        let store = Store::create(&account.folder_of(&self.id), &self.id, self.key.copy(), meta)?;
        if let Some(record) = &self.password_record {
            account.file.passwords.push(encode_record(record));
        }
        account.file.recoveries.push(encode_record(&self.recovery_record));
        if !self.hidden {
            account.file.listed.push(Listed { id: self.id.clone(), first: self.first, password: password.is_some(), slots: self.slots, created });
        }
        account.save()?;
        Ok(store)
    }
}

/// The listed vaults, and those open now, as `status` says them. A name is known only while its vault is open, and a hidden
/// vault is not known at all until then.
pub fn describe(account: &Account, open: &[&Store]) -> Vec<Value> {
    let name_of = |id: &str| open.iter().find(|store| store.id() == id).map(|store| store.meta.name.clone());
    let mut vaults: Vec<Value> = account
        .file
        .listed
        .iter()
        .map(|vault| {
            let name = name_of(&vault.id);
            json!({"id": vault.id, "name": name, "first": vault.first, "open": name.is_some(), "hidden": false, "methods": vault.methods()})
        })
        .collect();
    for store in open.iter().filter(|store| store.meta.hidden) {
        vaults.push(json!({"id": store.id(), "name": store.meta.name, "first": false, "open": true, "hidden": true, "methods": ["password", "recovery"]}));
    }
    vaults
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slots::fake::FakeChip;
    use crate::slots::RECORD;
    use serde_json::json;

    struct Folder(PathBuf);

    impl Folder {
        fn new(name: &str) -> Folder {
            let path = std::env::temp_dir().join(format!("amethystora-vault-vaults-{name}-{}", seal::random_hex(6)));
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

    fn set_up(state: &Path, password: &[u8]) -> (Store, RecoveryKey, Kdf) {
        let kdf = Kdf::fresh(10);
        let under = slots::derive(password, &kdf).unwrap();
        let draft = Draft::new("Personal", false, true, kdf.clone(), Some(&under)).unwrap();
        let recovery_text = draft.recovery.text();
        let store = draft.commit(state, UID).unwrap();
        (store, RecoveryKey::parse(&recovery_text).unwrap(), kdf)
    }

    fn add(state: &Path, name: &str, hidden: bool, password: &[u8]) -> (Store, RecoveryKey) {
        let account = Account::load(state, UID).unwrap().unwrap();
        let under = slots::derive(password, &account.file.kdf).unwrap();
        let draft = Draft::new(name, hidden, false, account.file.kdf.clone(), Some(&under)).unwrap();
        let text = draft.recovery.text();
        let store = draft.commit(state, UID).unwrap();
        (store, RecoveryKey::parse(&text).unwrap())
    }

    #[test]
    fn nothing_is_set_up_until_it_is_kept() {
        let folder = Folder::new("draft");
        assert!(Account::load(&folder.0, UID).unwrap().is_none());
        let kdf = Kdf::fresh(10);
        let draft = Draft::new("Personal", false, true, kdf, None).unwrap();
        assert!(Account::load(&folder.0, UID).unwrap().is_none());
        draft.commit(&folder.0, UID).unwrap();
        let account = Account::load(&folder.0, UID).unwrap().unwrap();
        assert!(account.first().unwrap().first);
        // Its name is under its own key, and nowhere in the clear
        let text = fs::read_to_string(folder.0.join(UID.to_string()).join("account")).unwrap();
        assert!(!text.contains("Personal"));
    }

    #[test]
    fn every_way_in_opens_the_same_key() {
        let folder = Folder::new("ways");
        let (store, recovery, kdf) = set_up(&folder.0, b"correct horse");
        let account = Account::load(&folder.0, UID).unwrap().unwrap();
        let by_password = slots::search(&slots::derive(b"correct horse", &kdf).unwrap(), &account.records(Table::Password), Table::Password);
        let by_recovery = slots::search(&recovery.key(), &account.records(Table::Recovery), Table::Recovery);
        assert_eq!(by_password[0].key.bytes(), store.key().bytes());
        assert_eq!(by_password[0].vault, store.id());
        assert_eq!(by_recovery[0].key.bytes(), store.key().bytes());
        assert!(slots::search(&slots::derive(b"wrong", &kdf).unwrap(), &account.records(Table::Password), Table::Password).is_empty());
        // And it is the same vault that opens
        assert!(Store::open(&account.folder_of(store.id()), store.id(), by_recovery[0].key.copy()).is_ok());
    }

    #[test]
    fn a_hidden_vault_is_in_no_list() {
        let folder = Folder::new("hidden");
        let (first, _, kdf) = set_up(&folder.0, b"first password");
        let (hidden, hidden_recovery) = add(&folder.0, "Secret name", true, b"hidden password");
        let (work, _) = add(&folder.0, "Work", false, b"work password");
        let account = Account::load(&folder.0, UID).unwrap().unwrap();
        // Three records, two listed vaults, and the account's file nowhere says the hidden one's name or id
        assert_eq!(account.records(Table::Password).len(), 3);
        assert_eq!(account.file.listed.len(), 2);
        let text = fs::read_to_string(folder.0.join(UID.to_string()).join("account")).unwrap();
        assert!(!text.contains("Secret name"));
        assert!(!text.contains(hidden.id()));
        let listed = describe(&account, &[&first]);
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0]["name"], json!("Personal"));
        // A listed vault that is closed has no name to give
        assert_eq!(listed[1]["name"], Value::Null);
        assert_eq!(listed[1]["open"], json!(false));
        assert!(!serde_json::to_string(&listed).unwrap().contains("Secret name"));
        assert!(!serde_json::to_string(&listed).unwrap().contains(hidden.id()));
        // Named only while it is open
        let open = describe(&account, &[&first, &hidden, &work]);
        assert_eq!(open.len(), 3);
        assert_eq!(open[2]["name"], json!("Secret name"));
        assert_eq!(open[2]["hidden"], json!(true));
        // Its password and its own recovery key each find it
        let found = slots::search(&slots::derive(b"hidden password", &kdf).unwrap(), &account.records(Table::Password), Table::Password);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].vault, hidden.id());
        let found = slots::search(&hidden_recovery.key(), &account.records(Table::Recovery), Table::Recovery);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].vault, hidden.id());
        // And what it says of itself is in its sealed meta
        let reopened = Store::open(&account.folder_of(hidden.id()), hidden.id(), hidden.key().copy()).unwrap();
        assert_eq!(reopened.meta.name, "Secret name");
        assert!(reopened.meta.hidden);
        assert!(!reopened.meta.first);
    }

    #[test]
    fn removing_one_vault_leaves_the_rest_opening() {
        let folder = Folder::new("remove");
        let (first, _, kdf) = set_up(&folder.0, b"first password");
        let (work, _) = add(&folder.0, "Work", false, b"work password");
        let (hidden, _) = add(&folder.0, "Hidden", true, b"hidden password");
        let mut account = Account::load(&folder.0, UID).unwrap().unwrap();
        account.remove_vault(&work.meta, work.id()).unwrap();
        let account = Account::load(&folder.0, UID).unwrap().unwrap();
        assert!(!account.folder_of(work.id()).exists());
        assert!(account.listed(work.id()).is_none());
        let records = account.records(Table::Password);
        assert_eq!(records.len(), 2);
        assert!(slots::search(&slots::derive(b"work password", &kdf).unwrap(), &records, Table::Password).is_empty());
        assert_eq!(slots::search(&slots::derive(b"first password", &kdf).unwrap(), &records, Table::Password)[0].vault, first.id());
        assert_eq!(slots::search(&slots::derive(b"hidden password", &kdf).unwrap(), &records, Table::Password)[0].vault, hidden.id());
    }

    #[test]
    fn forgetting_takes_every_vault_and_the_hidden_ones_too() {
        let folder = Folder::new("forget");
        set_up(&folder.0, b"first password");
        add(&folder.0, "Hidden", true, b"hidden password");
        Account::forget_everything(&folder.0, UID).unwrap();
        assert!(Account::load(&folder.0, UID).unwrap().is_none());
        assert_eq!(fs::read_dir(&folder.0).unwrap().count(), 0);
        Account::forget_everything(&folder.0, UID).unwrap();
    }

    #[test]
    fn a_forgetting_cut_short_is_finished_at_the_next_start() {
        let folder = Folder::new("gone");
        fs::create_dir_all(folder.0.join("1000.gone-abc/vaults")).unwrap();
        fs::create_dir_all(folder.0.join("1000")).unwrap();
        fs::create_dir_all(folder.0.join("machine.gone-abc")).unwrap();
        sweep_forgotten(&folder.0);
        assert!(!folder.0.join("1000.gone-abc").exists());
        assert!(folder.0.join("1000").exists());
        assert!(folder.0.join("machine.gone-abc").exists());
    }

    #[test]
    fn accounts_are_folders_apart() {
        let folder = Folder::new("two");
        set_up(&folder.0, b"pw");
        assert!(Account::load(&folder.0, 1001).unwrap().is_none());
    }

    #[test]
    fn there_is_a_most_and_a_new_vault_past_it_is_refused() {
        let folder = Folder::new("most");
        let (_, _, kdf) = set_up(&folder.0, b"first");
        let under = slots::derive(b"other", &kdf).unwrap();
        for _ in 1..MAX_VAULTS {
            Draft::new("n", true, false, kdf.clone(), Some(&under)).unwrap().commit(&folder.0, UID).unwrap();
        }
        let account = Account::load(&folder.0, UID).unwrap().unwrap();
        assert!(!account.has_room());
        assert!(Draft::new("n", true, false, kdf, Some(&under)).unwrap().commit(&folder.0, UID).is_err());
    }

    #[test]
    fn a_damaged_account_file_is_refused_and_not_repaired() {
        let folder = Folder::new("damaged");
        set_up(&folder.0, b"pw");
        let file = folder.0.join(UID.to_string()).join("account");
        let good: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        let mut bad = Vec::new();
        for (key, value) in [
            ("format", json!(2)),
            ("kdf", json!({"log_n": 40, "r": 8, "p": 1, "salt": "AAAAAAAAAAAAAAAAAAAAAA=="})),
            ("passwords", json!(["short"])),
            ("recoveries", json!(vec![encode_record(&[0u8; RECORD]); MAX_VAULTS + 1])),
            ("idle_minutes", json!(0)),
            ("listed", json!([{"id": "../..", "created": 1}])),
        ] {
            let mut changed = good.clone();
            changed[key] = value;
            bad.push(changed.to_string());
        }
        bad.push("not json".to_string());
        bad.push("{}".to_string());
        for text in bad {
            fs::write(&file, &text).unwrap();
            assert_eq!(Account::load(&folder.0, UID).err(), Some(Trouble::Damaged), "{text}");
        }
        fs::write(&file, vec![b' '; MAX_ACCOUNT_FILE as usize + 1]).unwrap();
        assert_eq!(Account::load(&folder.0, UID).err(), Some(Trouble::Damaged));
    }

    #[test]
    fn wrong_tries_put_the_next_to_the_person_for_an_hour() {
        let folder = Folder::new("tries");
        set_up(&folder.0, b"pw");
        let account = Account::load(&folder.0, UID).unwrap().unwrap();
        for _ in 0..FREE_TRIES {
            assert!(!account.needs_approval());
            account.note_failure();
        }
        assert!(account.needs_approval());
        account.note_success();
        assert!(!account.needs_approval());
        // An hour on, or a window that began in the future (a clock put right), is over
        let file = folder.0.join(UID.to_string()).join("tries");
        fs::write(&file, format!(r#"{{"fails":9,"since":{}}}"#, now() - TRIES_WINDOW)).unwrap();
        assert!(!account.needs_approval());
        fs::write(&file, format!(r#"{{"fails":9,"since":{}}}"#, now() + 10 * 365 * 86400)).unwrap();
        assert!(!account.needs_approval());
        account.note_failure();
        assert_eq!(fs::read_to_string(&file).unwrap(), format!(r#"{{"fails":1,"since":{}}}"#, now()));
    }

    #[test]
    fn a_second_first_vault_is_refused() {
        let folder = Folder::new("second-first");
        set_up(&folder.0, b"pw");
        let draft = Draft::new("Other", false, true, Kdf::fresh(10), None).unwrap();
        assert!(draft.commit(&folder.0, UID).is_err());
        assert_eq!(Account::load(&folder.0, UID).unwrap().unwrap().file.listed.len(), 1);
    }

    #[test]
    fn a_slot_on_the_chip_comes_with_setup() {
        let folder = Folder::new("chip");
        let chip = FakeChip::new();
        let mut draft = Draft::new("Personal", false, true, Kdf::fresh(10), None).unwrap();
        draft.add_chip_slot(SlotKind::NoPrompt, &chip).unwrap();
        let store = draft.commit(&folder.0, UID).unwrap();
        let account = Account::load(&folder.0, UID).unwrap().unwrap();
        let listed = account.first().unwrap();
        assert_eq!(listed.methods(), vec!["no-prompt", "recovery"]);
        assert_eq!(listed.slots[0].open_on_chip(&chip).unwrap().bytes(), store.key().bytes());
    }
}
