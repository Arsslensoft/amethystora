//! What the security watcher asks of the store (plans/done/PLAN-plugins.md, "Security, Control and the rest of the image"), run as
//! root by `amethystora-plugin-host audit --json`: a plugin whose files no longer match the digest a person approved, a
//! grants.json that is not the one the helper's last history line names, a file of the store that is not root's or that others
//! may write, something in an account's folder the helper never writes, a plugin's process that left its scope, and each approval
//! made with plugin-approval off. It changes nothing, and none of it can be allowed: the way out is putting it back.

use std::collections::BTreeSet;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use serde_json::{Value, json};

use crate::sandbox::SLICE;
use crate::store::Store;

/// One thing found: `id` is stable while the thing lasts, so that the watcher says it once
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub id: String,
    pub level: &'static str,
    pub uid: Option<u32>,
    pub plugin: Option<String>,
    pub text: String,
}

impl Finding {
    pub fn to_json(&self) -> Value {
        json!({"id": self.id, "level": self.level, "uid": self.uid, "account": self.uid.and_then(account_name), "plugin": self.plugin, "text": self.text})
    }
}

pub fn account_name(uid: u32) -> Option<String> {
    let mut buf = vec![0u8; 4096];
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut out: *mut libc::passwd = std::ptr::null_mut();
    // SAFETY: getpwuid_r writes into the buffers given, and `out` is null or points at `pwd`
    let rc = unsafe { libc::getpwuid_r(uid, &mut pwd, buf.as_mut_ptr().cast(), buf.len(), &mut out) };
    if rc != 0 || out.is_null() {
        return None;
    }
    // SAFETY: pw_name is a NUL-terminated string inside `buf`
    let name = unsafe { std::ffi::CStr::from_ptr(pwd.pw_name) }.to_string_lossy().into_owned();
    name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)).then_some(name)
}

/// What the helper itself leaves in an account's folder
fn helper_file(name: &str) -> bool {
    matches!(name, "grants.json" | "history.jsonl" | ".lock") || name.starts_with(".grants.json.new.")
}

/// Every entry of the store, never through a link: whose it is and who may write it
fn walk(dir: &Path, owner: u32, out: &mut Vec<Finding>, depth: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        let shown = path.display().to_string();
        if meta.file_type().is_symlink() || !(meta.is_dir() || meta.is_file()) {
            out.push(Finding { id: format!("plugin-store-kind:{shown}"), level: "critical", uid: None, plugin: None, text: format!("{shown} in the plugin store is not a plain file or folder") });
            continue;
        }
        if meta.uid() != owner || meta.permissions().mode() & 0o022 != 0 || meta.mode() & 0o6000 != 0 {
            out.push(Finding {
                id: format!("plugin-store-mode:{shown}:{}:{:o}", meta.uid(), meta.mode() & 0o7777),
                level: "critical",
                uid: None,
                plugin: None,
                text: format!("{shown} in the plugin store is not root's alone (owner {}, mode {:o}): another program could change a plugin", meta.uid(), meta.mode() & 0o7777),
            });
        }
        if meta.is_dir() && depth < 64 {
            walk(&path, owner, out, depth + 1);
        }
    }
}

/// The store's findings. `owner` is whose the store must be: root, or the tests' own account
pub fn store(store: &Store, owner: u32) -> (Vec<Finding>, Value) {
    let mut out = Vec::new();
    walk(&store.root, owner, &mut out, 0);
    let (mut plugins, mut network, mut accounts) = (0u64, 0u64, 0u64);
    let mut checked: BTreeSet<(String, String)> = BTreeSet::new();
    for entry in std::fs::read_dir(store.root.join("accounts")).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(uid) = name.parse::<u32>() else {
            out.push(Finding { id: format!("plugin-account-dir:{name}"), level: "critical", uid: None, plugin: None, text: format!("accounts/{name} in the plugin store is not an account's folder") });
            continue;
        };
        accounts += 1;
        for file in std::fs::read_dir(entry.path()).into_iter().flatten().flatten() {
            let f = file.file_name().to_string_lossy().into_owned();
            if !helper_file(&f) {
                out.push(Finding { id: format!("plugin-account-file:{uid}:{f}"), level: "critical", uid: Some(uid), plugin: None, text: format!("accounts/{uid}/{f} in the plugin store was not written by its helper") });
            }
        }
        let history = store.history(uid);
        let grants = match store.grants(uid) {
            Ok(g) => g,
            Err(why) => {
                out.push(Finding { id: format!("plugin-grants-unreadable:{uid}"), level: "critical", uid: Some(uid), plugin: None, text: format!("The plugins' grants of account {uid}: {why}") });
                continue;
            }
        };
        // The last line that carries a hash names the grants.json it wrote. Lines written before the hash was recorded name none
        if let Some(last) = history.iter().rev().find_map(|l| l.get("grants").map(|g| g.as_str().map(str::to_string))) {
            let now = store.grants_hash(uid);
            if now != last {
                out.push(Finding {
                    id: format!("plugin-grants-changed:{uid}:{}", now.as_deref().unwrap_or("none")),
                    level: "critical",
                    uid: Some(uid),
                    plugin: None,
                    text: format!("What the plugins of account {uid} may do was changed outside the plugin helper: its history does not record this change"),
                });
            }
        }
        for line in history.iter().filter(|l| l["approval"].as_str() == Some("none")) {
            let (id, t) = (line["id"].as_str().unwrap_or("?"), line["t"].as_u64().unwrap_or(0));
            let what = line["perms"].as_array().map(|p| p.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default();
            out.push(Finding {
                id: format!("plugin-unapproved:{uid}:{t}:{id}"),
                level: "normal",
                uid: Some(uid),
                plugin: Some(id.to_string()),
                text: format!("Plugin {id} was given {} with no approval asked: plugin-approval is off", if what.is_empty() { "what it asked for".to_string() } else { what }),
            });
        }
        for (id, grant) in &grants.plugins {
            plugins += 1;
            if grant.perms.iter().any(|p| p.starts_with("network=")) && !grant.blocked {
                network += 1;
            }
            let folder = store.folder_of(id, grant);
            if !checked.insert((folder.display().to_string(), grant.digest.clone())) {
                continue;
            }
            if let Err(why) = store.verify(&folder, &grant.digest) {
                out.push(Finding {
                    id: format!("plugin-files:{id}:{}", grant.digest),
                    level: "critical",
                    uid: Some(uid),
                    plugin: Some(id.clone()),
                    text: format!("Plugin {id}'s files changed after they were approved, so it does not run: {why}"),
                });
            }
        }
    }
    (out, json!({"accounts": accounts, "plugins": plugins, "network": network}))
}

/// The mount namespace a process is in, and the cgroup it is in, from /proc
fn proc_of(proc: &Path, pid: &str) -> Option<(String, String, String)> {
    let base = proc.join(pid);
    let ns = std::fs::read_link(base.join("ns/mnt")).ok()?.to_string_lossy().into_owned();
    let cgroup = std::fs::read_to_string(base.join("cgroup")).ok()?.lines().find_map(|l| l.strip_prefix("0::").map(str::to_string))?;
    let stat = std::fs::read_to_string(base.join("stat")).ok()?;
    // The start time, the 22nd field, after the command's name in brackets (which may hold anything)
    let start = stat.rsplit_once(')')?.1.split_whitespace().nth(19)?.to_string();
    Some((ns, cgroup, start))
}

/// A process that shares a plugin's sandbox (its mount namespace) but is not in the plugins' slice any more: it left the scope
/// that holds its memory, processor and time limits. The sandbox's first process is bubblewrap's, in the account's namespace,
/// so that one is left out: the namespaces of the user managers and of PID 1 are no sandbox's
pub fn escaped(proc: &Path) -> Vec<Finding> {
    let mut all: Vec<(String, String, String, String)> = Vec::new();
    for entry in std::fs::read_dir(proc).into_iter().flatten().flatten() {
        let pid = entry.file_name().to_string_lossy().into_owned();
        if !pid.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        if let Some((ns, cgroup, start)) = proc_of(proc, &pid) {
            all.push((pid, ns, cgroup, start));
        }
    }
    let inside = |cgroup: &str| cgroup.split('/').any(|part| part == SLICE);
    let mut plain: BTreeSet<&str> = BTreeSet::new();
    for (pid, ns, cgroup, _) in &all {
        if pid == "1" || cgroup.ends_with("/init.scope") {
            plain.insert(ns);
        }
    }
    let sandboxes: BTreeSet<&str> = all.iter().filter(|(_, ns, cgroup, _)| inside(cgroup) && !plain.contains(ns.as_str())).map(|(_, ns, _, _)| ns.as_str()).collect();
    let uid_of = |pid: &str| -> Option<u32> {
        std::fs::metadata(proc.join(pid)).ok().map(|m| m.uid())
    };
    let mut out = Vec::new();
    for (pid, ns, cgroup, start) in &all {
        if inside(cgroup) || !sandboxes.contains(ns.as_str()) {
            continue;
        }
        out.push(Finding {
            id: format!("plugin-escaped:{pid}:{start}"),
            level: "critical",
            uid: uid_of(pid),
            plugin: None,
            text: format!("Process {pid} is in a plugin's sandbox but left the plugins' scope ({cgroup}), and with it the limits a plugin runs under"),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{Grant, Grants};
    use crate::tree;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("plugin-audit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn me() -> u32 {
        // SAFETY: getuid has no arguments and cannot fail
        unsafe { libc::getuid() }
    }

    #[test]
    fn a_store_as_the_helper_left_it_has_nothing_to_say_and_each_change_is_found() {
        let dir = temp("store");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let store = Store { root: dir.clone(), shipped: dir.join("none") };
        let entries = vec![tree::Entry { path: "manifest.json".into(), dir: false, exec: false, data: b"{}".to_vec() }];
        let digest = tree::digest(&entries);
        let folder = store.store_dir(&digest);
        std::fs::create_dir_all(&folder).unwrap();
        tree::write_tree(&folder, &entries).unwrap();
        for d in [dir.join("store"), folder.clone()] {
            std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut grants = Grants::default();
        grants.plugins.insert("dev.example.a".into(), Grant { digest: digest.clone(), perms: vec!["network=a.example.com".into()], ..Grant::default() });
        store.write_grants(1000, &grants).unwrap();
        store.append_history(1000, json!({"action": "commit", "id": "dev.example.a"})).unwrap();
        for d in [dir.join("accounts"), store.account_dir(1000)] {
            std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let (found, summary) = super::store(&store, me());
        assert!(found.is_empty(), "{found:?}");
        assert_eq!(summary, json!({"accounts": 1, "plugins": 1, "network": 1}));

        // grants.json rewritten by something other than the helper
        let mut more = grants;
        more.plugins.get_mut("dev.example.a").unwrap().perms.push("files=~/Documents:rw".into());
        store.write_grants(1000, &more).unwrap();
        let (found, _) = super::store(&store, me());
        assert!(found.iter().any(|f| f.id.starts_with("plugin-grants-changed:1000:")), "{found:?}");
        // ...and recorded by the helper again, with plugin-approval off
        store.append_history(1000, json!({"action": "grant", "id": "dev.example.a", "perms": ["files=~/Documents:rw"], "approval": "none"})).unwrap();
        let (found, _) = super::store(&store, me());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].level, "normal");
        assert!(found[0].text.contains("files=~/Documents:rw"));

        // a changed byte, a file nobody writes, and a file others may write
        std::fs::write(folder.join("manifest.json"), b"{ }").unwrap();
        std::fs::write(store.account_dir(1000).join("extra"), b"").unwrap();
        std::fs::set_permissions(folder.join("manifest.json"), std::fs::Permissions::from_mode(0o666)).unwrap();
        let (found, _) = super::store(&store, me());
        let ids: Vec<&str> = found.iter().map(|f| f.id.split(':').next().unwrap()).collect();
        for id in ["plugin-files", "plugin-account-file", "plugin-store-mode", "plugin-unapproved"] {
            assert!(ids.contains(&id), "{id} in {ids:?}");
        }
        // A link anywhere in it
        std::os::unix::fs::symlink("/etc", dir.join("store").join("link")).unwrap();
        assert!(super::store(&store, me()).0.iter().any(|f| f.id.starts_with("plugin-store-kind:")));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_process_that_left_the_slice_but_shares_a_sandbox_is_found() {
        let dir = temp("proc");
        let one = |pid: &str, ns: &str, cgroup: &str| {
            let base = dir.join(pid);
            std::fs::create_dir_all(base.join("ns")).unwrap();
            std::os::unix::fs::symlink(format!("mnt:[{ns}]"), base.join("ns/mnt")).unwrap();
            std::fs::write(base.join("cgroup"), format!("0::{cgroup}\n")).unwrap();
            std::fs::write(base.join("stat"), format!("{pid} (a) b) S {}\n", (0..20).map(|n| n.to_string()).collect::<Vec<_>>().join(" "))).unwrap();
        };
        let slice = "/user.slice/user-1000.slice/user@1000.service/amethystora.slice/amethystora-plugins.slice/run-1.scope";
        one("1", "100", "/init.scope");
        one("50", "200", "/user.slice/user-1000.slice/user@1000.service/init.scope");
        // bubblewrap's first process, in the account's namespace, and the plugin in its own
        one("60", "200", slice);
        one("61", "300", slice);
        assert!(escaped(&dir).is_empty());
        // The plugin's child moved out of the slice
        one("62", "300", "/user.slice/user-1000.slice/user@1000.service/app.slice/x.scope");
        let found = escaped(&dir);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].id.starts_with("plugin-escaped:62:"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
