//! /var/lib/amethystora/plugins, the store of what a person approved (plans/done/PLAN-plugins.md, Integrity). It is root's:
//! the host and the commands only read it, and `amethystora-plugin-root` is the one writer.
//!
//!   store/<digest>/                  a plugin's files, written once
//!   accounts/<uid>/grants.json       for each id: its digest, what it may do, what is blocked
//!   accounts/<uid>/history.jsonl     every change, appended by the helper
//!   quarantine/<ticket>/             what `prepare` froze and a person has not approved yet

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tree;

pub const ROOT: &str = "/var/lib/amethystora/plugins";
/// What the image ships: read-only, in the signed image, in the same sandbox as anyone's
pub const SHIPPED: &str = "/usr/share/amethystora/plugins";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Grant {
    pub digest: String,
    #[serde(default)]
    pub previous: Option<String>,
    #[serde(default)]
    pub perms: Vec<String>,
    /// Shipped plugins live in the image, not the store
    #[serde(default)]
    pub shipped: bool,
    /// All of its network is blocked
    #[serde(default)]
    pub blocked: bool,
    #[serde(default)]
    pub blocked_hosts: Vec<String>,
    /// Bytes an hour a host may be sent, where the manifest asked for more than the default
    #[serde(default)]
    pub limits: BTreeMap<String, u64>,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct Grants {
    #[serde(default)]
    pub plugins: BTreeMap<String, Grant>,
}

#[derive(Clone, Debug)]
pub struct Store {
    pub root: PathBuf,
    pub shipped: PathBuf,
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Store {
    pub fn production() -> Store {
        Store { root: ROOT.into(), shipped: SHIPPED.into() }
    }

    pub fn store_dir(&self, digest: &str) -> PathBuf {
        self.root.join("store").join(digest)
    }

    pub fn account_dir(&self, uid: u32) -> PathBuf {
        self.root.join("accounts").join(uid.to_string())
    }

    pub fn quarantine(&self) -> PathBuf {
        self.root.join("quarantine")
    }

    /// Where a granted plugin's files are: the image's folder for a shipped one, the store's for the rest
    pub fn folder_of(&self, id: &str, grant: &Grant) -> PathBuf {
        if grant.shipped { self.shipped.join(id) } else { self.store_dir(&grant.digest) }
    }

    pub fn grants(&self, uid: u32) -> Result<Grants, String> {
        match std::fs::read_to_string(self.account_dir(uid).join("grants.json")) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| format!("grants.json is not readable: {e}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Grants::default()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// An exclusive lock on a folder's `.lock` file, held until the returned file is dropped: the helper's verbs read, change and write
    /// grants.json and the quarantine's tickets, which two of them at once would undo each other's changes in
    pub fn lock(&self, dir: &Path) -> Result<std::fs::File, String> {
        use std::os::fd::AsRawFd;
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(".lock")).map_err(|e| e.to_string())?;
        // SAFETY: a descriptor this function owns, for the call
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(file)
    }

    /// One atomic change: written beside, flushed, renamed over
    pub fn write_grants(&self, uid: u32, grants: &Grants) -> Result<(), String> {
        let dir = self.account_dir(uid);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        // A name of its own: two writers never take each other's file (the verbs also hold a lock)
        let temp = dir.join(format!(".grants.json.new.{}", std::process::id()));
        let _ = std::fs::remove_file(&temp);
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(grants).map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).and_then(|_| file.write_all(b"\n")).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, dir.join("grants.json")).map_err(|e| e.to_string())?;
        if let Ok(d) = std::fs::File::open(&dir) {
            let _ = d.sync_all();
        }
        Ok(())
    }

    /// The SHA-256 of the account's grants.json as it is on disk, which each line of the history carries: the watcher's
    /// audit takes a grants.json that is not the one the last line names as changed outside this helper
    pub fn grants_hash(&self, uid: u32) -> Option<String> {
        use sha2::{Digest, Sha256};
        let bytes = std::fs::read(self.account_dir(uid).join("grants.json")).ok()?;
        Some(Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect())
    }

    /// One line in the account's history. `fields` is an object; the time, the uid and the grants' hash are added here
    pub fn append_history(&self, uid: u32, mut fields: Value) -> Result<(), String> {
        let dir = self.account_dir(uid);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        fields["t"] = now().into();
        fields["uid"] = uid.into();
        fields["grants"] = self.grants_hash(uid).into();
        let mut file = std::fs::OpenOptions::new().append(true).create(true).open(dir.join("history.jsonl")).map_err(|e| e.to_string())?;
        file.write_all(format!("{fields}\n").as_bytes()).and_then(|_| file.sync_all()).map_err(|e| e.to_string())
    }

    pub fn history(&self, uid: u32) -> Vec<Value> {
        let text = std::fs::read_to_string(self.account_dir(uid).join("history.jsonl")).unwrap_or_default();
        text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
    }

    /// Whether a folder's files are those of the digest: every file, and nothing added or taken away
    pub fn verify(&self, folder: &Path, digest: &str) -> Result<(), String> {
        let entries = tree::read_dir(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        if tree::digest(&entries) == digest { Ok(()) } else { Err(format!("{} no longer matches the digest that was approved", folder.display())) }
    }

    /// Every digest some account's grants still name, or the previous one of
    pub fn referenced(&self) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        for entry in std::fs::read_dir(self.root.join("accounts")).into_iter().flatten().flatten() {
            let Ok(uid) = entry.file_name().to_string_lossy().parse::<u32>() else { continue };
            for grant in self.grants(uid).unwrap_or_default().plugins.values() {
                out.insert(grant.digest.clone());
                out.extend(grant.previous.clone());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("plugin-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn grants_and_history_round_trip() {
        let dir = temp("grants");
        let store = Store { root: dir.clone(), shipped: dir.join("shipped") };
        assert_eq!(store.grants(1000).unwrap(), Grants::default());
        let mut grants = Grants::default();
        grants.plugins.insert("dev.example.a".into(), Grant { digest: "ab".repeat(32), perms: vec!["notify".into()], ..Grant::default() });
        store.write_grants(1000, &grants).unwrap();
        assert_eq!(store.grants(1000).unwrap(), grants);
        store.append_history(1000, serde_json::json!({"action": "commit", "id": "dev.example.a"})).unwrap();
        store.append_history(1000, serde_json::json!({"action": "block"})).unwrap();
        let history = store.history(1000);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0]["uid"], 1000);
        assert_eq!(store.referenced().len(), 1);
        assert!(Store { root: dir.join("none"), shipped: dir.clone() }.referenced().is_empty(), "no accounts folder is not an error");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_changed_byte_fails_the_check() {
        let dir = temp("verify");
        let store = Store { root: dir.clone(), shipped: dir.join("shipped") };
        let folder = dir.join("tree");
        std::fs::create_dir_all(&folder).unwrap();
        let entries = vec![tree::Entry { path: "a".into(), dir: false, exec: false, data: b"one".to_vec() }];
        tree::write_tree(&folder, &entries).unwrap();
        let digest = tree::digest(&entries);
        assert!(store.verify(&folder, &digest).is_ok());
        std::fs::write(folder.join("a"), b"One").unwrap();
        assert!(store.verify(&folder, &digest).is_err());
        std::fs::write(folder.join("a"), b"one").unwrap();
        std::fs::write(folder.join("b"), b"added").unwrap();
        assert!(store.verify(&folder, &digest).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
