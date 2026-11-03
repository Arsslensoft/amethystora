//! One vault's folder: every item a file of its own, and a sealed index that lists them with a generation number each,
//! so that a file put back from before is noticed. Writes are atomic: a crash leaves the old file or the new one, and
//! between an item and the index it leaves an item one generation ahead, which is the new one.
//!
//! ```text
//! <vault>/meta           sealed: what this vault is called, and which records of the account's tables are its own
//! <vault>/index          sealed: {generation, items}
//! <vault>/items/<kind>.<id>   sealed under amethystora-vault:<kind>:<id>: {g, n, a, v}
//! ```
//!
//! Neither a kind nor an id can hold a dot (`protocol`), so an item's file name is that one pair and no other.

use std::collections::BTreeMap;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use zeroize::Zeroizing;

mod batch;
pub use batch::{BatchItem, BatchWriter};

use crate::protocol::{valid_id, valid_kind, Approval};
use crate::seal::{self, Key};

/// Why an item could not be had. The service answers the first three alike, since telling them apart leaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trouble {
    NotFound,
    /// It did not open or read: damaged, or not this vault's
    Damaged,
    /// It opens, and is older than the index says: a file put back from before
    Stale,
    /// The disk: nothing can be said of the item
    Disk,
    /// There is no room for it: a vault's most items, its index's most, or its most purposes
    Full,
}

/// What a file of the vault's may be at most: an item is 1 MiB of value and a little around it, an index some thousands of
/// names. Anything read is bounded, and refused rather than repaired.
const MAX_FILE: u64 = 16 * 1024 * 1024;
/// Items in a vault, and what the index may hold of their names and metas, which stay in memory while it is open
pub const MAX_ITEMS: usize = 5000;
pub const MAX_LISTED_BYTES: usize = 4 * 1024 * 1024;
/// What is listed of one item, besides its name
pub const MAX_META: usize = 4 * 1024;
/// Features' keys in one vault: each a purpose a program can make by sealing, so they have a most of their own, apart from a
/// person's items
pub const MAX_PURPOSES: usize = 256;

/// A file read whole, if it is no larger than this
pub fn read_bounded(file: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    let handle = fs::File::open(file)?;
    let mut bytes = Vec::new();
    handle.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::Error::from(ErrorKind::InvalidData));
    }
    Ok(bytes)
}

/// A write made ready beside its file and not yet in place: `commit` is the crash's other side
pub struct Prepared {
    temporary: PathBuf,
    file: PathBuf,
}

/// A temporary file in the same folder, whole and on the disk
pub fn prepare(file: &Path, bytes: &[u8]) -> std::io::Result<Prepared> {
    let folder = file.parent().ok_or_else(|| std::io::Error::from(ErrorKind::InvalidInput))?;
    let temporary = folder.join(format!(".tmp-{}", seal::random_hex(8)));
    let mut handle = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary)?;
    let written = handle.write_all(bytes).and_then(|()| handle.sync_all());
    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(Prepared { temporary, file: file.to_path_buf() })
}

impl Prepared {
    /// Renames it over the file and has the folder remember that
    pub fn commit(self) -> std::io::Result<()> {
        if let Err(error) = fs::rename(&self.temporary, &self.file) {
            let _ = fs::remove_file(&self.temporary);
            return Err(error);
        }
        sync_folder(self.file.parent().unwrap_or(Path::new(".")))
    }
}

pub fn sync_folder(folder: &Path) -> std::io::Result<()> {
    fs::File::open(folder)?.sync_all()
}

/// A temporary file, fsync, rename, fsync the folder: the file is only ever the old one or the new one
pub fn write_atomic(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    prepare(file, bytes)?.commit()
}

/// A folder only the service reaches
pub fn make_folder(folder: &Path) -> std::io::Result<()> {
    fs::DirBuilder::new().recursive(true).mode(0o700).create(folder)
}

/// What is left of writes a crash cut short
fn sweep(folder: &Path) {
    let Ok(entries) = fs::read_dir(folder) else { return };
    for entry in entries.flatten() {
        if entry.file_name().as_encoded_bytes().starts_with(b".tmp-") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// What a vault knows of itself, sealed in its folder. A hidden vault's name is nowhere else.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meta {
    pub name: String,
    pub hidden: bool,
    pub first: bool,
    /// The nonce of its record in each table, which is how it finds its own without knowing the other records' keys
    #[serde(default)]
    pub password: Option<String>,
    pub recovery: String,
    pub created: u64,
}

/// A line of the index: everything the list shows, and no secret
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub kind: String,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub meta: Value,
    pub approval: Approval,
    pub generation: u64,
    pub changed: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct Index {
    generation: u64,
    items: BTreeMap<String, Entry>,
}

#[derive(Serialize, Deserialize)]
struct ItemFile {
    g: u64,
    n: String,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    m: Value,
    a: Approval,
    t: u64,
    v: Value,
}

/// One item for `Store::put_many`
pub struct NewItem<'a> {
    pub kind: &'a str,
    pub id: &'a str,
    pub name: &'a str,
    pub meta: &'a Value,
    pub approval: Approval,
    pub value: &'a Value,
}

/// An open vault: its key, and the index in memory
pub struct Store {
    folder: PathBuf,
    id: String,
    key: Key,
    pub meta: Meta,
    index: Index,
}

fn entry_key(kind: &str, id: &str) -> String {
    format!("{kind}.{id}")
}

/// How long a value is as JSON, counted without writing it anywhere: the room checks add this up over every item of an open
/// vault, and a string made for each would be garbage a service held to a few megabytes cannot afford
fn json_len(value: &Value) -> usize {
    struct Count(usize);
    impl Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(0);
    let _ = serde_json::to_writer(&mut count, value);
    count.0
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|since| since.as_secs()).unwrap_or(0)
}

impl Store {
    /// A new vault's folder, with its meta and an empty index
    pub fn create(folder: &Path, id: &str, key: Key, meta: Meta) -> Result<Store, Trouble> {
        make_folder(&folder.join("items")).map_err(|_| Trouble::Disk)?;
        let store = Store { folder: folder.to_path_buf(), id: id.to_string(), key, meta, index: Index::default() };
        store.write_meta()?;
        store.write_index()?;
        Ok(store)
    }

    /// An existing vault, under the key a slot gave. Refused if its meta or index do not open under it.
    pub fn open(folder: &Path, id: &str, key: Key) -> Result<Store, Trouble> {
        let read = |name: &str, kind: &str| -> Result<Zeroizing<Vec<u8>>, Trouble> {
            let bytes = read_bounded(&folder.join(name), MAX_FILE).map_err(|_| Trouble::Disk)?;
            seal::open_item(&key, kind, id, &bytes).map_err(|_| Trouble::Damaged)
        };
        let meta: Meta = serde_json::from_slice(&read("meta", "meta")?).map_err(|_| Trouble::Damaged)?;
        let index: Index = serde_json::from_slice(&read("index", "index")?).map_err(|_| Trouble::Damaged)?;
        if !index.items.iter().all(|(name, entry)| valid_kind(&entry.kind) && valid_id(&entry.id) && *name == entry_key(&entry.kind, &entry.id)) {
            return Err(Trouble::Damaged);
        }
        sweep(folder);
        sweep(&folder.join("items"));
        Ok(Store { folder: folder.to_path_buf(), id: id.to_string(), key, meta, index })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn key(&self) -> &Key {
        &self.key
    }

    pub fn write_meta(&self) -> Result<(), Trouble> {
        let plain = Zeroizing::new(serde_json::to_vec(&self.meta).map_err(|_| Trouble::Disk)?);
        write_atomic(&self.folder.join("meta"), &seal::seal_item(&self.key, "meta", &self.id, &plain)).map_err(|_| Trouble::Disk)
    }

    fn write_index(&self) -> Result<(), Trouble> {
        let plain = Zeroizing::new(serde_json::to_vec(&self.index).map_err(|_| Trouble::Disk)?);
        write_atomic(&self.folder.join("index"), &seal::seal_item(&self.key, "index", &self.id, &plain)).map_err(|_| Trouble::Disk)
    }

    fn item_file(&self, kind: &str, id: &str) -> Result<PathBuf, Trouble> {
        // Everything calling this has checked already; a path is the one place it must never slip through
        if !valid_kind(kind) || !valid_id(id) {
            return Err(Trouble::NotFound);
        }
        Ok(self.folder.join("items").join(entry_key(kind, id)))
    }

    pub fn entry(&self, kind: &str, id: &str) -> Option<&Entry> {
        self.index.items.get(&entry_key(kind, id))
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.index.items.values()
    }

    /// The item written and in place, and the index not yet: where a crash between them leaves things
    pub fn put_item_file(&self, kind: &str, id: &str, name: &str, meta: &Value, approval: Approval, value: &Value) -> Result<Entry, Trouble> {
        let file = self.item_file(kind, id)?;
        let generation = self.index.generation + 1;
        let changed = now();
        let item = ItemFile { g: generation, n: name.to_string(), m: meta.clone(), a: approval, t: changed, v: value.clone() };
        let plain = Zeroizing::new(serde_json::to_vec(&item).map_err(|_| Trouble::Disk)?);
        write_atomic(&file, &seal::seal_item(&self.key, kind, id, &plain)).map_err(|_| Trouble::Disk)?;
        Ok(Entry { kind: kind.to_string(), id: id.to_string(), name: name.to_string(), meta: meta.clone(), approval, generation, changed })
    }

    /// What an item, the index and the meta take on the disk together: what a write changes, for an account's quota
    pub fn footprint(&self, kind: &str, id: &str) -> u64 {
        self.footprint_of(&[(kind, id)])
    }

    /// The same for several items written together: each one's file, and the index and the meta once
    pub fn footprint_of(&self, items: &[(&str, &str)]) -> u64 {
        let size = |path: PathBuf| fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
        let files: u64 = items.iter().map(|(kind, id)| self.item_file(kind, id).map(size).unwrap_or(0)).sum();
        files + size(self.folder.join("index")) + size(self.folder.join("meta"))
    }

    /// Whether one more item of this size still fits
    pub fn has_room(&self, name: &str, meta: &Value) -> bool {
        let listed: usize = self.index.items.values().map(|entry| entry.name.len() + json_len(&entry.meta)).sum();
        self.index.items.len() < MAX_ITEMS && listed + name.len() + json_len(meta) <= MAX_LISTED_BYTES
    }

    /// Whether a batch fits: the items it makes new, and the names and metas listed after it replaced what it names
    fn has_room_for(&self, items: &[NewItem<'_>]) -> bool {
        let size = |name: &str, meta: &Value| name.len() + json_len(meta);
        let mut count = self.index.items.len();
        let mut listed: usize = self.index.items.values().map(|entry| size(&entry.name, &entry.meta)).sum();
        let mut seen = std::collections::HashSet::new();
        for item in items {
            match self.entry(item.kind, item.id) {
                Some(entry) if seen.insert((item.kind, item.id)) => listed -= size(&entry.name, &entry.meta),
                Some(_) => {}
                None if seen.insert((item.kind, item.id)) => count += 1,
                None => {}
            }
            listed += size(item.name, item.meta);
        }
        count <= MAX_ITEMS && listed <= MAX_LISTED_BYTES
    }

    /// Writes an item, then the index that knows it
    pub fn put(&mut self, kind: &str, id: &str, name: &str, meta: &Value, approval: Approval, value: &Value) -> Result<Entry, Trouble> {
        // A new item has to fit, whoever writes it: past the most, the index would outgrow what opening reads, and the vault never
        // open again
        if self.entry(kind, id).is_none() && !self.has_room(name, meta) {
            return Err(Trouble::Full);
        }
        let entry = self.put_item_file(kind, id, name, meta, approval, value)?;
        self.index.generation = entry.generation;
        self.index.items.insert(entry_key(kind, id), entry.clone());
        self.write_index()?;
        Ok(entry)
    }

    /// Several at once, as atomic as one: every file written beside its place, the filesystem flushed once, each renamed into
    /// place, the folder flushed once, then the index once. Two syncs where one at a time took two per item, which made an
    /// import of 250 hold the service for seconds.
    pub fn put_many<'a>(&mut self, items: impl IntoIterator<Item = NewItem<'a>>) -> Result<(), Trouble> {
        let items: Vec<NewItem<'a>> = items.into_iter().collect();
        // The same room check as `put`, for the whole batch, whoever writes it: what it adds, less what it replaces
        if !self.has_room_for(&items) {
            return Err(Trouble::Full);
        }
        let folder = self.folder.join("items");
        let mut written: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut entries = Vec::new();
        let outcome = (|| -> Result<(), Trouble> {
            for item in items {
                let file = self.item_file(item.kind, item.id)?;
                let generation = self.index.generation + 1 + entries.len() as u64;
                let changed = now();
                let stored = ItemFile { g: generation, n: item.name.to_string(), m: item.meta.clone(), a: item.approval, t: changed, v: item.value.clone() };
                let plain = Zeroizing::new(serde_json::to_vec(&stored).map_err(|_| Trouble::Disk)?);
                let temporary = folder.join(format!(".tmp-{}", seal::random_hex(8)));
                let mut handle = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary).map_err(|_| Trouble::Disk)?;
                written.push((temporary, file));
                handle.write_all(&seal::seal_item(&self.key, item.kind, item.id, &plain)).map_err(|_| Trouble::Disk)?;
                entries.push(Entry { kind: item.kind.to_string(), id: item.id.to_string(), name: item.name.to_string(), meta: item.meta.clone(), approval: item.approval, generation, changed });
            }
            let handle = fs::File::open(&folder).map_err(|_| Trouble::Disk)?;
            // SAFETY: an open descriptor this function owns, for the length of the call
            if unsafe { libc::syncfs(std::os::fd::AsRawFd::as_raw_fd(&handle)) } != 0 {
                return Err(Trouble::Disk);
            }
            Ok(())
        })();
        if let Err(trouble) = outcome {
            for (temporary, _) in &written {
                let _ = fs::remove_file(temporary);
            }
            return Err(trouble);
        }
        for (temporary, file) in &written {
            fs::rename(temporary, file).map_err(|_| Trouble::Disk)?;
        }
        sync_folder(&folder).map_err(|_| Trouble::Disk)?;
        for entry in entries {
            self.index.generation = entry.generation;
            self.index.items.insert(entry_key(&entry.kind, &entry.id), entry);
        }
        self.write_index()
    }

    /// Several at once, the index first and once, as `remove` does for one
    pub fn remove_many(&mut self, items: &[(&str, &str)]) -> Result<(), Trouble> {
        let mut files = Vec::with_capacity(items.len());
        for (kind, id) in items {
            files.push(self.item_file(kind, id)?);
            self.index.items.remove(&entry_key(kind, id));
        }
        self.index.generation += 1;
        self.write_index()?;
        for file in files {
            match fs::remove_file(&file) {
                Err(error) if error.kind() != ErrorKind::NotFound => return Err(Trouble::Disk),
                _ => {}
            }
        }
        sync_folder(&self.folder.join("items")).map_err(|_| Trouble::Disk)
    }

    /// An item's value. One the index does not know is not there, whatever a crash left; one behind the index is a file put
    /// back from before, and refused; one ahead of it is a write a crash cut short, and is the new one.
    pub fn get(&mut self, kind: &str, id: &str) -> Result<Value, Trouble> {
        let known = self.entry(kind, id).ok_or(Trouble::NotFound)?.generation;
        let file = self.item_file(kind, id)?;
        let bytes = match read_bounded(&file, MAX_FILE) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Err(Trouble::Damaged),
            Err(_) => return Err(Trouble::Disk),
        };
        let plain = seal::open_item(&self.key, kind, id, &bytes).map_err(|_| Trouble::Damaged)?;
        let item: ItemFile = serde_json::from_slice(&plain).map_err(|_| Trouble::Damaged)?;
        // A crash between the item and the index leaves it one write ahead, never more: anything else is a file from another time
        if item.g < known || item.g > self.index.generation + 1 {
            return Err(Trouble::Stale);
        }
        if item.g > known {
            let entry = Entry { kind: kind.to_string(), id: id.to_string(), name: item.n, meta: item.m, approval: item.a, generation: item.g, changed: item.t };
            self.index.generation = self.index.generation.max(item.g);
            self.index.items.insert(entry_key(kind, id), entry);
            self.write_index()?;
        }
        Ok(item.v)
    }

    /// Takes it from the index first: a crash then leaves a file nothing lists, which is not an item
    pub fn remove(&mut self, kind: &str, id: &str) -> Result<(), Trouble> {
        let file = self.item_file(kind, id)?;
        if self.index.items.remove(&entry_key(kind, id)).is_none() {
            return Err(Trouble::NotFound);
        }
        self.index.generation += 1;
        self.write_index()?;
        match fs::remove_file(&file) {
            Err(error) if error.kind() != ErrorKind::NotFound => return Err(Trouble::Disk),
            _ => {}
        }
        sync_folder(&self.folder.join("items")).map_err(|_| Trouble::Disk)
    }

    /// Changes how often an item asks, in its file and in the index
    pub fn set_approval(&mut self, kind: &str, id: &str, approval: Approval) -> Result<(), Trouble> {
        let entry = self.entry(kind, id).ok_or(Trouble::NotFound)?.clone();
        let value = Zeroizing::new(self.get(kind, id)?.to_string());
        let value: Value = serde_json::from_str(&value).map_err(|_| Trouble::Damaged)?;
        self.put(kind, id, &entry.name, &entry.meta, approval, &value).map(|_| ())
    }

    /// A purpose's key, kept as an item of kind `data-key`, which never leaves the service. Made on first use if `make`
    /// says so; None if there is none. Never asks anyone, since features ask only after a person's own click.
    pub fn data_key(&mut self, purpose: &str, make: bool) -> Result<Option<Key>, Trouble> {
        match self.get(DATA_KEY, purpose) {
            Ok(value) => {
                let hex = Zeroizing::new(value.as_str().ok_or(Trouble::Damaged)?.to_string());
                let bytes = Zeroizing::new(seal::unhex(&hex).ok_or(Trouble::Damaged)?);
                Key::from_slice(&bytes).map(Some).ok_or(Trouble::Damaged)
            }
            Err(Trouble::NotFound) if make => {
                if self.entries().filter(|entry| entry.kind == DATA_KEY).count() >= MAX_PURPOSES {
                    return Err(Trouble::Full);
                }
                let key = Key::random();
                self.put_data_key(purpose, &key)?;
                Ok(Some(key))
            }
            Err(Trouble::NotFound) => Ok(None),
            Err(trouble) => Err(trouble),
        }
    }

    /// Keeps this as a purpose's key
    pub fn put_data_key(&mut self, purpose: &str, key: &Key) -> Result<(), Trouble> {
        let hex = Zeroizing::new(seal::hex(key.bytes()));
        self.put(DATA_KEY, purpose, purpose, &Value::Null, Approval::Never, &Value::String(hex.to_string())).map(|_| ())
    }
}

/// The kind a feature's key is kept under
pub const DATA_KEY: &str = "data-key";

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub struct Folder(pub PathBuf);

    impl Folder {
        pub fn new(name: &str) -> Folder {
            let path = std::env::temp_dir().join(format!("amethystora-vault-store-{name}-{}", seal::random_hex(6)));
            fs::create_dir_all(&path).unwrap();
            Folder(path)
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn meta() -> Meta {
        Meta { name: "Personal".into(), hidden: false, first: true, password: None, recovery: "aa".into(), created: 1 }
    }

    fn new_store(folder: &Folder) -> (Store, Key, String) {
        let key = Key::random();
        let id = seal::random_hex(16);
        let store = Store::create(&folder.0.join(&id), &id, key.copy(), meta()).unwrap();
        (store, key, id)
    }

    #[test]
    fn an_item_comes_back_and_is_listed() {
        let folder = Folder::new("back");
        let (mut store, key, id) = new_store(&folder);
        let entry = store.put("password", "a1", "GitHub", &json!({"user": "me"}), Approval::WhileOpen, &json!({"secret": "hunter2"})).unwrap();
        assert_eq!(entry.generation, 1);
        assert_eq!(store.get("password", "a1").unwrap(), json!({"secret": "hunter2"}));
        assert_eq!(store.entries().count(), 1);
        drop(store);
        // And after closing and opening again
        let mut store = Store::open(&folder.0.join(&id), &id, key.copy()).unwrap();
        assert_eq!(store.get("password", "a1").unwrap(), json!({"secret": "hunter2"}));
        assert_eq!(store.entry("password", "a1").unwrap().name, "GitHub");
        assert_eq!(store.meta.name, "Personal");
        // No secret in the clear anywhere in the folder
        for path in ["index", "meta", "items/password.a1"] {
            let bytes = fs::read(folder.0.join(&id).join(path)).unwrap();
            assert_eq!(&bytes[..4], b"AVv1", "{path}");
            assert!(!bytes.windows(6).any(|window| window == b"GitHub"), "{path}");
        }
    }

    #[test]
    fn a_wrong_key_opens_nothing() {
        let folder = Folder::new("wrong");
        let (store, _, id) = new_store(&folder);
        drop(store);
        assert_eq!(Store::open(&folder.0.join(&id), &id, Key::random()).err(), Some(Trouble::Damaged));
    }

    #[test]
    fn a_file_under_another_name_refuses() {
        let folder = Folder::new("swap");
        let (mut store, _, id) = new_store(&folder);
        store.put("password", "a", "A", &Value::Null, Approval::Never, &json!("first")).unwrap();
        store.put("password", "b", "B", &Value::Null, Approval::Never, &json!("second")).unwrap();
        let items = folder.0.join(&id).join("items");
        fs::copy(items.join("password.b"), items.join("password.a")).unwrap();
        assert_eq!(store.get("password", "a"), Err(Trouble::Damaged));
        assert_eq!(store.get("password", "b").unwrap(), json!("second"));
        // Nor can the index pass for an item, or the other way round
        fs::copy(folder.0.join(&id).join("index"), items.join("password.b")).unwrap();
        assert_eq!(store.get("password", "b"), Err(Trouble::Damaged));
    }

    #[test]
    fn a_file_put_back_from_before_is_noticed() {
        let folder = Folder::new("back-in-time");
        let (mut store, _, id) = new_store(&folder);
        store.put("password", "a", "A", &Value::Null, Approval::Never, &json!("old")).unwrap();
        let file = folder.0.join(&id).join("items/password.a");
        let before = fs::read(&file).unwrap();
        store.put("password", "a", "A", &Value::Null, Approval::Never, &json!("new")).unwrap();
        fs::write(&file, before).unwrap();
        assert_eq!(store.get("password", "a"), Err(Trouble::Stale));
    }

    #[test]
    fn a_crash_before_the_rename_leaves_the_old_file() {
        let folder = Folder::new("crash-rename");
        let file = folder.0.join("f");
        write_atomic(&file, b"old").unwrap();
        // The temporary file is whole and on the disk, and then nothing more happens
        let prepared = prepare(&file, b"new").unwrap();
        std::mem::forget(prepared);
        assert_eq!(fs::read(&file).unwrap(), b"old");
        assert_eq!(fs::read_dir(&folder.0).unwrap().count(), 2);
        // And what it left does not outlast the next opening
        sweep(&folder.0);
        assert_eq!(fs::read_dir(&folder.0).unwrap().count(), 1);
        prepare(&file, b"new").unwrap().commit().unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"new");
    }

    #[test]
    fn a_crash_between_an_item_and_the_index_leaves_the_new_item() {
        let folder = Folder::new("crash-index");
        let (mut store, key, id) = new_store(&folder);
        store.put("password", "a", "A", &Value::Null, Approval::Never, &json!("old")).unwrap();
        // The item is in place, and the process goes before the index is
        store.put_item_file("password", "a", "A2", &Value::Null, Approval::Ask, &json!("new")).unwrap();
        drop(store);
        let mut store = Store::open(&folder.0.join(&id), &id, key.copy()).unwrap();
        assert_eq!(store.entry("password", "a").unwrap().name, "A");
        assert_eq!(store.get("password", "a").unwrap(), json!("new"));
        // Taken at its word from then on, its name and how often it asks included
        assert_eq!(store.entry("password", "a").unwrap().name, "A2");
        assert_eq!(store.entry("password", "a").unwrap().approval, Approval::Ask);
        drop(store);
        let mut store = Store::open(&folder.0.join(&id), &id, key.copy()).unwrap();
        assert_eq!(store.get("password", "a").unwrap(), json!("new"));
    }

    #[test]
    fn a_crash_writing_a_new_item_leaves_no_item() {
        let folder = Folder::new("crash-new");
        let (store, key, id) = new_store(&folder);
        store.put_item_file("password", "a", "A", &Value::Null, Approval::Never, &json!("new")).unwrap();
        drop(store);
        let mut store = Store::open(&folder.0.join(&id), &id, key.copy()).unwrap();
        assert_eq!(store.get("password", "a"), Err(Trouble::NotFound));
        assert_eq!(store.entries().count(), 0);
    }

    #[test]
    fn a_removed_item_stays_removed_after_a_crash() {
        let folder = Folder::new("remove");
        let (mut store, key, id) = new_store(&folder);
        store.put("password", "a", "A", &Value::Null, Approval::Never, &json!("x")).unwrap();
        let file = folder.0.join(&id).join("items/password.a");
        let kept = fs::read(&file).unwrap();
        store.remove("password", "a").unwrap();
        assert!(!file.exists());
        assert_eq!(store.remove("password", "a"), Err(Trouble::NotFound));
        // As if the unlink had never happened
        fs::write(&file, kept).unwrap();
        drop(store);
        let mut store = Store::open(&folder.0.join(&id), &id, key.copy()).unwrap();
        assert_eq!(store.get("password", "a"), Err(Trouble::NotFound));
    }

    #[test]
    fn a_damaged_index_or_meta_refuses_the_vault() {
        let folder = Folder::new("damaged");
        let (store, key, id) = new_store(&folder);
        drop(store);
        let index = folder.0.join(&id).join("index");
        let good = fs::read(&index).unwrap();
        let mut bad = good.clone();
        bad[40] ^= 1;
        fs::write(&index, &bad).unwrap();
        assert_eq!(Store::open(&folder.0.join(&id), &id, key.copy()).err(), Some(Trouble::Damaged));
        fs::remove_file(&index).unwrap();
        assert_eq!(Store::open(&folder.0.join(&id), &id, key.copy()).err(), Some(Trouble::Disk));
        fs::write(&index, good).unwrap();
        assert!(Store::open(&folder.0.join(&id), &id, key.copy()).is_ok());
    }

    #[test]
    fn a_path_cannot_leave_the_folder() {
        let folder = Folder::new("paths");
        let (mut store, _, _) = new_store(&folder);
        for (kind, id) in [("../x", "a"), ("k", "../x"), ("k", "a/b"), ("k", ".."), ("k", ""), ("", "a")] {
            assert!(store.put(kind, id, "n", &Value::Null, Approval::Never, &json!(1)).is_err(), "{kind} {id}");
            assert!(store.get(kind, id).is_err());
            assert!(store.remove(kind, id).is_err());
        }
    }

    #[test]
    fn there_is_a_most_of_what_a_vault_lists() {
        let folder = Folder::new("room");
        let (mut store, _, _) = new_store(&folder);
        assert!(store.has_room("n", &Value::Null));
        store.put("k", "a", &"n".repeat(1000), &Value::Null, Approval::Never, &json!(1)).unwrap();
        assert!(store.has_room("n", &Value::Null));
        assert!(!store.has_room(&"n".repeat(MAX_LISTED_BYTES), &Value::Null));
    }

    #[test]
    fn a_full_vault_takes_no_new_item_but_changes_its_own() {
        let folder = Folder::new("full");
        let (mut store, _, _) = new_store(&folder);
        store.put("k", "a", "n", &Value::Null, Approval::Never, &json!(1)).unwrap();
        let big = "n".repeat(200);
        assert!(store.put("k", "b", &"n".repeat(MAX_LISTED_BYTES), &Value::Null, Approval::Never, &json!(1)).is_err());
        assert_eq!(store.put("k", "b", &"n".repeat(MAX_LISTED_BYTES), &Value::Null, Approval::Never, &json!(1)), Err(Trouble::Full));
        assert!(store.put("k", "a", &big, &Value::Null, Approval::Never, &json!(2)).is_ok());
    }

    #[test]
    fn a_batch_has_the_same_room_check_as_one_item() {
        let folder = Folder::new("batch-room");
        let (mut store, _, _) = new_store(&folder);
        store.put("k", "a", "n", &Value::Null, Approval::Never, &json!(1)).unwrap();
        let (value, null) = (json!(1), Value::Null);
        let item = |id, name| NewItem { kind: "k", id, name, meta: &null, approval: Approval::Never, value: &value };
        let big = "n".repeat(MAX_LISTED_BYTES);
        // A new item that does not fit is refused, and nothing of the batch is written
        assert_eq!(store.put_many([item("b", "small"), item("c", &big)]), Err(Trouble::Full));
        assert!(store.entry("k", "b").is_none());
        // One that replaces what it names, to a size that fits, is taken
        assert_eq!(store.put_many([item("a", "renamed"), item("b", "small")]), Ok(()));
        assert_eq!(store.entry("k", "a").map(|entry| entry.name.as_str()), Some("renamed"));
        assert!(store.entry("k", "b").is_some());
    }

    #[test]
    fn features_keys_have_a_most_of_their_own() {
        let folder = Folder::new("purposes");
        let (mut store, _, _) = new_store(&folder);
        for at in 0..MAX_PURPOSES {
            store.data_key(&format!("p{at}"), true).unwrap().unwrap();
        }
        assert_eq!(store.data_key("one-more", true).err(), Some(Trouble::Full));
        assert!(store.data_key("p0", true).unwrap().is_some());
        // And a person's item still has room
        assert!(store.put("k", "a", "n", &Value::Null, Approval::Never, &json!(1)).is_ok());
    }

    #[test]
    fn an_item_more_than_one_write_ahead_is_from_another_time() {
        let folder = Folder::new("ahead");
        let (mut store, key, id) = new_store(&folder);
        store.put("k", "a", "n", &Value::Null, Approval::Never, &json!("old")).unwrap();
        let index = folder.0.join(&id).join("index");
        let old_index = fs::read(&index).unwrap();
        store.put("k", "a", "n", &Value::Null, Approval::Never, &json!("new")).unwrap();
        store.put("k", "b", "n", &Value::Null, Approval::Never, &json!("other")).unwrap();
        store.put("k", "a", "n", &Value::Null, Approval::Never, &json!("newest")).unwrap();
        drop(store);
        // The index from three writes ago, put back: the item is three ahead of it
        fs::write(&index, old_index).unwrap();
        let mut store = Store::open(&folder.0.join(&id), &id, key.copy()).unwrap();
        assert_eq!(store.get("k", "a"), Err(Trouble::Stale));
    }

    #[test]
    fn a_data_key_is_made_once() {
        let folder = Folder::new("data-key");
        let (mut store, _, _) = new_store(&folder);
        assert!(store.data_key("resume", false).unwrap().is_none());
        let made = store.data_key("resume", true).unwrap().unwrap();
        assert_eq!(store.data_key("resume", true).unwrap().unwrap().bytes(), made.bytes());
        assert_eq!(store.entry(DATA_KEY, "resume").unwrap().approval, Approval::Never);
        store.remove(DATA_KEY, "resume").unwrap();
        assert!(store.data_key("resume", false).unwrap().is_none());
        assert_ne!(store.data_key("resume", true).unwrap().unwrap().bytes(), made.bytes());
    }

    #[test]
    fn how_often_it_asks_can_change() {
        let folder = Folder::new("approval");
        let (mut store, _, _) = new_store(&folder);
        store.put("k", "a", "A", &json!({"m": 1}), Approval::Ask, &json!("x")).unwrap();
        store.set_approval("k", "a", Approval::Never).unwrap();
        let entry = store.entry("k", "a").unwrap();
        assert_eq!(entry.approval, Approval::Never);
        assert_eq!(entry.meta, json!({"m": 1}));
        assert_eq!(store.get("k", "a").unwrap(), json!("x"));
    }

    #[test]
    fn a_temporary_file_is_not_an_item_and_does_not_last() {
        let folder = Folder::new("sweep");
        let (store, key, id) = new_store(&folder);
        drop(store);
        let items = folder.0.join(&id).join("items");
        fs::write(items.join(".tmp-abc"), b"x").unwrap();
        let store = Store::open(&folder.0.join(&id), &id, key.copy()).unwrap();
        assert!(!items.join(".tmp-abc").exists());
        drop(store);
    }

    #[test]
    fn a_file_too_large_is_refused_unread() {
        let folder = Folder::new("large");
        let file = folder.0.join("f");
        fs::write(&file, vec![0u8; 11]).unwrap();
        assert_eq!(read_bounded(&file, 11).unwrap().len(), 11);
        assert!(read_bounded(&file, 10).is_err());
    }
}
