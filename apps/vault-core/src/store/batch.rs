//! Many items written at once, away from the loop that owns the store (sync's pull of a thousand items, which wrote 2,000 fsynced
//! files on the one loop and stopped everyone for 12 seconds). A thread seals each item into a file beside its place and has the
//! filesystem flush them once; the loop then only renames, in slices, and writes the index once for each. Nothing is in place until
//! the loop puts it there, so an item changed meanwhile is the loop's to decide (the caller checks what each write was decided from),
//! and a crash leaves the old items and files nothing lists, which opening sweeps.
//!
//! Every item of a batch is written at the one generation after the index's at the time, so that a crash between the renames and the
//! index leaves them one ahead, which `get` takes for the new ones, and never more.

use super::*;

/// What a thread needs to write items for a store it does not hold
pub struct BatchWriter {
    folder: PathBuf,
    key: Key,
    generation: u64,
}

/// An item written beside its place and not yet in it
pub struct BatchItem {
    temporary: PathBuf,
    file: PathBuf,
    entry: Entry,
}

impl BatchItem {
    pub fn kind(&self) -> &str {
        &self.entry.kind
    }

    pub fn id(&self) -> &str {
        &self.entry.id
    }

    /// Takes the file away: this item will not be put in place
    pub fn discard(self) {
        let _ = fs::remove_file(&self.temporary);
    }
}

impl BatchWriter {
    /// Seals an item into a file of its own beside its place. Not on the disk until `flush`.
    pub fn write(&self, kind: &str, id: &str, name: &str, meta: &Value, approval: Approval, value: &Value) -> Result<BatchItem, Trouble> {
        if !valid_kind(kind) || !valid_id(id) {
            return Err(Trouble::NotFound);
        }
        let changed = now();
        let item = ItemFile { g: self.generation, n: name.to_string(), m: meta.clone(), a: approval, t: changed, v: value.clone() };
        let plain = Zeroizing::new(serde_json::to_vec(&item).map_err(|_| Trouble::Disk)?);
        let temporary = self.folder.join(format!(".tmp-{}", seal::random_hex(8)));
        let mut handle = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary).map_err(|_| Trouble::Disk)?;
        if handle.write_all(&seal::seal_item(&self.key, kind, id, &plain)).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(Trouble::Disk);
        }
        let entry = Entry { kind: kind.to_string(), id: id.to_string(), name: name.to_string(), meta: meta.clone(), approval, generation: self.generation, changed };
        Ok(BatchItem { temporary, file: self.folder.join(entry_key(kind, id)), entry })
    }

    /// Everything written so far is on the disk, in one flush of the filesystem rather than one for each file
    pub fn flush(&self) -> Result<(), Trouble> {
        let handle = fs::File::open(&self.folder).map_err(|_| Trouble::Disk)?;
        // SAFETY: an open descriptor this function owns, for the length of the call
        if unsafe { libc::syncfs(std::os::fd::AsRawFd::as_raw_fd(&handle)) } != 0 {
            return Err(Trouble::Disk);
        }
        Ok(())
    }
}

impl Store {
    /// A writer for items to be put in later by `commit_batch`
    pub fn batch_writer(&self) -> BatchWriter {
        BatchWriter { folder: self.folder.join("items"), key: self.key.copy(), generation: self.index.generation + 1 }
    }

    /// How many more entries the index takes, and how many more bytes of names and metas it lists: worked out once, for a slice
    pub fn room(&self) -> (usize, usize) {
        let listed: usize = self.index.items.values().map(|entry| entry.name.len() + entry.meta.to_string().len()).sum();
        (MAX_ITEMS.saturating_sub(self.index.items.len()), MAX_LISTED_BYTES.saturating_sub(listed))
    }

    /// Puts written items in place: each renamed, the folder flushed once and the index written once. The caller has seen that they
    /// fit (`room_after`) and that what each was decided from still holds.
    pub fn commit_batch(&mut self, items: Vec<BatchItem>) -> Result<(), Trouble> {
        let mut result = Ok(());
        let mut renamed = Vec::with_capacity(items.len());
        for item in items {
            match fs::rename(&item.temporary, &item.file) {
                Ok(()) => renamed.push(item.entry),
                Err(_) => {
                    let _ = fs::remove_file(&item.temporary);
                    result = Err(Trouble::Disk);
                }
            }
        }
        if renamed.is_empty() {
            return result;
        }
        sync_folder(&self.folder.join("items")).map_err(|_| Trouble::Disk)?;
        for entry in renamed {
            self.index.generation = self.index.generation.max(entry.generation);
            self.index.items.insert(entry_key(&entry.kind, &entry.id), entry);
        }
        self.write_index()?;
        result
    }
}
