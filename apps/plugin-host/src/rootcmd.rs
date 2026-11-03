//! What `amethystora-plugin-root` does, one verb at a time (plans/PLAN-plugins.md, Integrity). It is started by pkexec,
//! one polkit action per verb (org.amethystora.plugins.policy), and trusts nothing it is given: the account is pkexec's
//! `PKEXEC_UID`, never an argument; every argument is held to a short set of characters and parsed here, not by a shell;
//! what is granted is checked against the manifest in the frozen files, and never against what the caller says it holds.
//!
//!   prepare                      a tar on standard input, frozen in quarantine/<ticket>; prints the ticket and the digest
//!   inspect TICKET               what was frozen, for a person to read
//!   commit ID DIGEST PERM...     the frozen files into the store, and exactly these permissions granted
//!   commit-shipped ID            the image's own plugin, granted what its manifest names
//!   grant ID DIGEST PERM...      more permissions for the files already installed
//!   revoke ID [PERM...]          takes permissions away (all, with none named)
//!   block ID [HOST] / unblock    blocks a host, or all of a plugin's network, and lifts it
//!   remove ID                    the plugin goes
//!   rollback ID                  back to the previous files, with no more permissions than it had

use std::io::{Read, Write};
use std::path::PathBuf;

use serde_json::json;

use crate::manifest::{Manifest, shipped_refuses, valid_host, valid_id, valid_perm_arg};
use crate::store::{Grant, Store, now};
use crate::tree::{self, MAX_BYTES, MAX_ENTRIES};

const TICKET_LIFE: u64 = 3600;
const MAX_TICKETS: usize = 4;
const QUOTA: u64 = 3 * MAX_BYTES;

pub struct Ctx {
    pub store: Store,
    /// pkexec's PKEXEC_UID: the account that asked
    pub uid: Option<u32>,
}

fn id_arg(arg: Option<&String>, system: bool) -> Result<&str, String> {
    let id = arg.map(String::as_str).ok_or("an id is missing")?;
    valid_id(id, system).map_err(|_| format!("{id:?} is not a plugin id"))?;
    Ok(id)
}

fn digest_arg(arg: Option<&String>) -> Result<&str, String> {
    let digest = arg.map(String::as_str).ok_or("a digest is missing")?;
    if tree::valid_digest(digest) { Ok(digest) } else { Err("a digest is 64 lower-case hex digits".into()) }
}

fn perm_args(args: &[String]) -> Result<Vec<String>, String> {
    let mut perms = Vec::new();
    for arg in args {
        if !valid_perm_arg(arg) {
            return Err(format!("{arg:?} has a character a permission never has"));
        }
        perms.push(arg.clone());
    }
    perms.sort();
    perms.dedup();
    Ok(perms)
}

fn ticket_arg(arg: Option<&String>) -> Result<&str, String> {
    let ticket = arg.map(String::as_str).ok_or("a ticket is missing")?;
    if ticket.len() == 32 && ticket.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) { Ok(ticket) } else { Err("that is not a ticket".into()) }
}

fn read_manifest(entries: &[tree::Entry], system: bool) -> Result<Manifest, String> {
    let entry = entries.iter().find(|e| e.path == "manifest.json" && !e.dir).ok_or("the plugin has no manifest.json")?;
    Manifest::parse_with(std::str::from_utf8(&entry.data).map_err(|_| "manifest.json is not text")?, system)
}

struct Ticket {
    dir: PathBuf,
    id: String,
    digest: String,
}

impl Ctx {
    fn uid(&self) -> Result<u32, String> {
        self.uid.filter(|u| *u != 0).ok_or_else(|| "it is started only through pkexec, by an account".to_string())
    }

    /// Tickets of this account that are still within their hour; older ones, of anyone, are cleared first
    fn tickets(&self, uid: u32) -> Vec<(Ticket, u64)> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(self.store.quarantine()).into_iter().flatten().flatten() {
            let dir = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let meta: Option<serde_json::Value> = std::fs::read_to_string(dir.join("meta.json")).ok().and_then(|t| serde_json::from_str(&t).ok());
            let Some(meta) = meta else {
                let _ = std::fs::remove_dir_all(&dir);
                continue;
            };
            if now().saturating_sub(meta["created"].as_u64().unwrap_or(0)) > TICKET_LIFE {
                let _ = std::fs::remove_dir_all(&dir);
                continue;
            }
            if meta["uid"].as_u64() == Some(uid as u64) {
                let (id, digest) = (meta["id"].as_str().unwrap_or("").to_string(), meta["digest"].as_str().unwrap_or("").to_string());
                out.push((Ticket { dir, id, digest }, meta["bytes"].as_u64().unwrap_or(0)));
            }
        }
        out
    }

    pub fn run(&self, args: &[String], stdin: impl Read, out: &mut impl Write) -> Result<(), String> {
        let uid = self.uid()?;
        let rest = &args.get(1..).unwrap_or(&[]);
        // The quarantine first and the account second, always, so that two verbs cannot wait on each other
        let _quarantine = self.store.lock(&self.store.quarantine())?;
        let _account = self.store.lock(&self.store.account_dir(uid))?;
        match args.first().map(String::as_str) {
            Some("prepare") => self.prepare(uid, stdin, out),
            Some("inspect") => self.inspect(uid, ticket_arg(rest.first())?, out),
            Some("commit") => self.commit(uid, id_arg(rest.first(), false)?, digest_arg(rest.get(1))?, perm_args(rest.get(2..).unwrap_or(&[]))?),
            Some("commit-shipped") => self.commit_shipped(uid, id_arg(rest.first(), true)?),
            Some("grant") => self.grant(uid, id_arg(rest.first(), false)?, digest_arg(rest.get(1))?, perm_args(rest.get(2..).unwrap_or(&[]))?),
            Some("revoke") => self.revoke(uid, id_arg(rest.first(), true)?, perm_args(rest.get(1..).unwrap_or(&[]))?),
            Some(verb @ ("block" | "unblock")) => {
                let host = match rest.get(1) {
                    Some(h) if valid_host(h) => Some(h.as_str()),
                    Some(_) => return Err("that is not a host name".into()),
                    None => None,
                };
                self.block(uid, id_arg(rest.first(), true)?, host, verb == "block")
            }
            Some("remove") => self.remove(uid, id_arg(rest.first(), true)?),
            Some("rollback") => self.rollback(uid, id_arg(rest.first(), true)?),
            _ => return Err("usage: amethystora-plugin-root prepare|inspect|commit|commit-shipped|grant|revoke|block|unblock|remove|rollback".into()),
        }?;
        writeln!(out, "ok").map_err(|e| e.to_string())
    }

    fn prepare(&self, uid: u32, stdin: impl Read, out: &mut impl Write) -> Result<(), String> {
        let held = self.tickets(uid);
        if held.len() >= MAX_TICKETS || held.iter().map(|(_, b)| b).sum::<u64>() >= QUOTA {
            return Err("this account has plugins waiting that nobody approved: they are cleared after an hour".into());
        }
        // The archive's own headers and padding come on top of its content
        let entries = tree::read_tar(stdin.take(MAX_BYTES + (MAX_ENTRIES as u64 + 4) * 1024))?;
        let manifest = read_manifest(&entries, false)?;
        let digest = tree::digest(&entries);
        let mut random = [0u8; 16];
        std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut random)).map_err(|e| e.to_string())?;
        let ticket = tree::hex(&random);
        let quarantine = self.store.quarantine();
        std::fs::create_dir_all(&quarantine).map_err(|e| e.to_string())?;
        let dir = quarantine.join(&ticket);
        let made = (|| -> Result<(), String> {
            std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
            set_mode(&dir, 0o700)?;
            std::fs::create_dir(dir.join("tree")).map_err(|e| e.to_string())?;
            tree::write_tree(&dir.join("tree"), &entries)?;
            let bytes: u64 = entries.iter().map(|e| e.data.len() as u64).sum();
            let meta = json!({"uid": uid, "id": manifest.id, "digest": digest, "created": now(), "bytes": bytes});
            std::fs::write(dir.join("meta.json"), meta.to_string()).map_err(|e| e.to_string())
        })();
        if let Err(e) = made {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e);
        }
        writeln!(out, "ticket\t{ticket}\ndigest\t{digest}\nid\t{}", manifest.id).map_err(|e| e.to_string())
    }

    fn ticket(&self, uid: u32, ticket: &str) -> Result<Ticket, String> {
        let dir = self.store.quarantine().join(ticket);
        self.tickets(uid).into_iter().map(|(t, _)| t).find(|t| t.dir == dir).ok_or_else(|| "there is no such plugin waiting for you (an hour passes, or it was approved)".into())
    }

    fn inspect(&self, uid: u32, ticket: &str, out: &mut impl Write) -> Result<(), String> {
        let t = self.ticket(uid, ticket)?;
        self.store.verify(&t.dir.join("tree"), &t.digest)?;
        let entries = tree::read_dir(&t.dir.join("tree"))?;
        let m = read_manifest(&entries, false)?;
        let mut text = format!("id\t{}\ndigest\t{}\nname\t{}\nversion\t{}\nlicense\t{}\n", m.id, t.digest, m.name, m.version, m.license);
        for need in &m.needs {
            text += &format!("perm\t{}\t{}\n", need.perm, need.why);
        }
        for e in entries.iter().filter(|e| !e.dir) {
            text += &format!("file\t{}\t{}\n", e.data.len(), e.path);
        }
        out.write_all(text.as_bytes()).map_err(|e| e.to_string())
    }

    fn commit(&self, uid: u32, id: &str, digest: &str, perms: Vec<String>) -> Result<(), String> {
        let ticket = self.tickets(uid).into_iter().map(|(t, _)| t).find(|t| t.id == id && t.digest == digest).ok_or("no plugin with that id and digest is waiting for you")?;
        let frozen = ticket.dir.join("tree");
        // The files are checked again, as they are now, against the digest the person was shown
        let entries = tree::read_dir(&frozen)?;
        if tree::digest(&entries) != digest {
            return Err("the frozen files do not match their digest".into());
        }
        let manifest = read_manifest(&entries, false)?;
        let allowed = manifest.permissions();
        if let Some(p) = perms.iter().find(|p| !allowed.contains(*p)) {
            return Err(format!("{p:?} is not something this plugin asks for"));
        }
        let dest = self.store.store_dir(digest);
        std::fs::create_dir_all(dest.parent().unwrap_or(&dest)).map_err(|e| e.to_string())?;
        if dest.exists() {
            self.store.verify(&dest, digest)?;
        } else {
            std::fs::rename(&frozen, &dest).map_err(|e| e.to_string())?;
        }
        let mut grants = self.store.grants(uid)?;
        let old = grants.plugins.remove(id).unwrap_or_default();
        let previous = if !old.digest.is_empty() && old.digest != digest && !old.shipped { Some(old.digest.clone()) } else { old.previous.clone() };
        let limits = manifest.limits.iter().filter(|(host, _)| perms.contains(&format!("network={host}"))).cloned().collect();
        // A block survives an update: only the person lifts it
        grants.plugins.insert(
            id.to_string(),
            Grant { digest: digest.into(), previous: previous.clone(), perms: perms.clone(), shipped: false, blocked: old.blocked, blocked_hosts: old.blocked_hosts, limits },
        );
        self.store.write_grants(uid, &grants)?;
        self.store.append_history(uid, json!({"action": "commit", "id": id, "digest": digest, "previous": previous, "perms": perms}))?;
        let _ = std::fs::remove_dir_all(&ticket.dir);
        self.gc();
        Ok(())
    }

    fn commit_shipped(&self, uid: u32, id: &str) -> Result<(), String> {
        let folder = self.store.shipped.join(id);
        let entries = tree::read_dir(&folder)?;
        let manifest = read_manifest(&entries, true)?;
        if manifest.id != id {
            return Err("the folder's name is not the plugin's id".into());
        }
        let perms: Vec<String> = manifest.permissions().into_iter().collect();
        if let Some(p) = perms.iter().find(|p| shipped_refuses(p)) {
            return Err(format!("{p:?}: a plugin of the image may not ask for that"));
        }
        let digest = tree::digest(&entries);
        let mut grants = self.store.grants(uid)?;
        let old = grants.plugins.remove(id).unwrap_or_default();
        grants.plugins.insert(id.into(), Grant { digest: digest.clone(), perms: perms.clone(), shipped: true, blocked: old.blocked, blocked_hosts: old.blocked_hosts, ..Grant::default() });
        self.store.write_grants(uid, &grants)?;
        self.store.append_history(uid, json!({"action": "commit-shipped", "id": id, "digest": digest, "perms": perms}))
    }

    fn grant(&self, uid: u32, id: &str, digest: &str, perms: Vec<String>) -> Result<(), String> {
        let mut grants = self.store.grants(uid)?;
        let grant = grants.plugins.get_mut(id).ok_or("that plugin is not installed")?;
        if grant.digest != digest || grant.shipped {
            return Err("those are not the files that are installed".into());
        }
        let folder = self.store.store_dir(digest);
        self.store.verify(&folder, digest)?;
        let manifest = read_manifest(&tree::read_dir(&folder)?, false)?;
        let allowed = manifest.permissions();
        if let Some(p) = perms.iter().find(|p| !allowed.contains(*p)) {
            return Err(format!("{p:?} is not something this plugin asks for"));
        }
        grant.perms.extend(perms.iter().cloned());
        grant.perms.sort();
        grant.perms.dedup();
        for (host, limit) in manifest.limits.iter().filter(|(h, _)| perms.contains(&format!("network={h}"))) {
            grant.limits.insert(host.clone(), *limit);
        }
        let all = grant.perms.clone();
        self.store.write_grants(uid, &grants)?;
        self.store.append_history(uid, json!({"action": "grant", "id": id, "digest": digest, "perms": perms, "now": all}))
    }

    fn revoke(&self, uid: u32, id: &str, perms: Vec<String>) -> Result<(), String> {
        let mut grants = self.store.grants(uid)?;
        let grant = grants.plugins.get_mut(id).ok_or("that plugin is not installed")?;
        if let Some(p) = perms.iter().find(|p| !grant.perms.contains(*p)) {
            return Err(format!("{p:?} is not granted"));
        }
        if perms.is_empty() { grant.perms.clear() } else { grant.perms.retain(|p| !perms.contains(p)) }
        self.store.write_grants(uid, &grants)?;
        self.store.append_history(uid, json!({"action": "revoke", "id": id, "perms": perms}))
    }

    fn block(&self, uid: u32, id: &str, host: Option<&str>, on: bool) -> Result<(), String> {
        let mut grants = self.store.grants(uid)?;
        let grant = grants.plugins.get_mut(id).ok_or("that plugin is not installed")?;
        match (host, on) {
            (None, on) => grant.blocked = on,
            (Some(h), true) if !grant.blocked_hosts.iter().any(|b| b == h) => grant.blocked_hosts.push(h.to_string()),
            (Some(h), false) => grant.blocked_hosts.retain(|b| b != h),
            _ => {}
        }
        self.store.write_grants(uid, &grants)?;
        self.store.append_history(uid, json!({"action": if on { "block" } else { "unblock" }, "id": id, "host": host}))
    }

    fn remove(&self, uid: u32, id: &str) -> Result<(), String> {
        let mut grants = self.store.grants(uid)?;
        let old = grants.plugins.remove(id).ok_or("that plugin is not installed")?;
        self.store.write_grants(uid, &grants)?;
        self.store.append_history(uid, json!({"action": "remove", "id": id, "digest": old.digest}))?;
        self.gc();
        Ok(())
    }

    fn rollback(&self, uid: u32, id: &str) -> Result<(), String> {
        let mut grants = self.store.grants(uid)?;
        let grant = grants.plugins.get_mut(id).ok_or("that plugin is not installed")?;
        let previous = grant.previous.clone().ok_or("there is no earlier version")?;
        let folder = self.store.store_dir(&previous);
        self.store.verify(&folder, &previous)?;
        let allowed = read_manifest(&tree::read_dir(&folder)?, false)?.permissions();
        // Going back never gives more than the person approved for the version that was running
        grant.perms.retain(|p| allowed.contains(p));
        let (from, now_perms) = (grant.digest.clone(), grant.perms.clone());
        grant.previous = Some(from.clone());
        grant.digest = previous.clone();
        self.store.write_grants(uid, &grants)?;
        self.store.append_history(uid, json!({"action": "rollback", "id": id, "digest": previous, "from": from, "perms": now_perms}))
    }

    /// Folders of the store that no account's grants name any more
    fn gc(&self) {
        let used = self.store.referenced();
        for entry in std::fs::read_dir(self.store.root.join("store")).into_iter().flatten().flatten() {
            if !used.contains(entry.file_name().to_string_lossy().as_ref()) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

fn set_mode(path: &std::path::Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Entry;

    fn manifest_with(needs: &str) -> String {
        format!(
            r#"{{"schema":1,"id":"dev.example.t","name":"T","version":"1.0.0","license":"MIT","command":["./run.sh"],"needs":{needs}}}"#
        )
    }

    fn archive(manifest: &str) -> Vec<u8> {
        tree::write_tar(&[
            Entry { path: "manifest.json".into(), dir: false, exec: false, data: manifest.as_bytes().to_vec() },
            Entry { path: "run.sh".into(), dir: false, exec: true, data: b"echo hi\n".to_vec() },
        ])
        .unwrap()
    }

    fn setup(name: &str) -> (Ctx, PathBuf) {
        let dir = std::env::temp_dir().join(format!("plugin-root-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (Ctx { store: Store { root: dir.clone(), shipped: dir.join("shipped") }, uid: Some(1000) }, dir)
    }

    fn call(ctx: &Ctx, words: &str, stdin: &[u8]) -> Result<String, String> {
        let args: Vec<String> = words.split(' ').filter(|w| !w.is_empty()).map(str::to_string).collect();
        let mut out = Vec::new();
        ctx.run(&args, stdin, &mut out)?;
        Ok(String::from_utf8(out).unwrap())
    }

    fn field(output: &str, name: &str) -> String {
        output.lines().find_map(|l| l.strip_prefix(&format!("{name}\t"))).unwrap().to_string()
    }

    #[test]
    fn prepare_inspect_commit_and_the_rest() {
        let (ctx, dir) = setup("flow");
        let needs = r#"{"notify":{"why":"Says"},"network":[{"host":"api.example.com","why":"Reads","limit":2097152}]}"#;
        let out = call(&ctx, "prepare", &archive(&manifest_with(needs))).unwrap();
        let (ticket, digest) = (field(&out, "ticket"), field(&out, "digest"));
        let seen = call(&ctx, &format!("inspect {ticket}"), b"").unwrap();
        assert!(seen.contains("perm\tnotify\tSays") && seen.contains("file\t8\trun.sh"));
        // Not what the manifest names, a character it never has, a digest that differs
        assert!(call(&ctx, &format!("commit dev.example.t {digest} files=~/x:ro"), b"").is_err());
        assert!(call(&ctx, &format!("commit dev.example.t {digest} notify;open"), b"").is_err());
        assert!(call(&ctx, &format!("commit dev.example.t {} notify", "0".repeat(64)), b"").is_err());
        // Another account cannot take it
        let other = Ctx { store: ctx.store.clone(), uid: Some(1001) };
        assert!(call(&other, &format!("commit dev.example.t {digest} notify"), b"").is_err());
        assert!(call(&other, &format!("inspect {ticket}"), b"").is_err());
        call(&ctx, &format!("commit dev.example.t {digest} notify network=api.example.com"), b"").unwrap();
        let grants = ctx.store.grants(1000).unwrap();
        let grant = &grants.plugins["dev.example.t"];
        assert_eq!(grant.perms, ["network=api.example.com", "notify"]);
        assert_eq!(grant.limits["api.example.com"], 2097152);
        assert!(ctx.store.store_dir(&digest).join("run.sh").exists());
        assert!(ctx.store.grants(1001).unwrap().plugins.is_empty());
        // Blocks, revokes and removal
        call(&ctx, "block dev.example.t api.example.com", b"").unwrap();
        call(&ctx, "block dev.example.t", b"").unwrap();
        let grant = &ctx.store.grants(1000).unwrap().plugins["dev.example.t"];
        assert!(grant.blocked && grant.blocked_hosts == ["api.example.com"]);
        call(&ctx, "revoke dev.example.t notify", b"").unwrap();
        assert!(call(&ctx, "revoke dev.example.t notify", b"").is_err());
        call(&ctx, &format!("grant dev.example.t {digest} notify"), b"").unwrap();
        assert!(call(&ctx, &format!("grant dev.example.t {digest} window"), b"").is_err());
        call(&ctx, "unblock dev.example.t", b"").unwrap();
        // An update keeps the block on the host, and the old files for a rollback
        let v2 = manifest_with(needs).replace("1.0.0", "1.1.0");
        let out = call(&ctx, "prepare", &archive(&v2)).unwrap();
        let digest2 = field(&out, "digest");
        call(&ctx, &format!("commit dev.example.t {digest2} notify"), b"").unwrap();
        let grant = &ctx.store.grants(1000).unwrap().plugins["dev.example.t"];
        assert_eq!(grant.previous.as_deref(), Some(digest.as_str()));
        assert_eq!(grant.blocked_hosts, ["api.example.com"]);
        call(&ctx, "rollback dev.example.t", b"").unwrap();
        assert_eq!(ctx.store.grants(1000).unwrap().plugins["dev.example.t"].digest, digest);
        assert!(ctx.store.history(1000).len() >= 8);
        call(&ctx, "remove dev.example.t", b"").unwrap();
        assert!(ctx.store.grants(1000).unwrap().plugins.is_empty());
        assert!(!ctx.store.store_dir(&digest).exists(), "an unreferenced folder is cleared");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_what_it_must() {
        let (ctx, dir) = setup("refuse");
        let none = Ctx { store: ctx.store.clone(), uid: None };
        assert!(call(&none, "prepare", &archive(&manifest_with("{}"))).is_err(), "no pkexec");
        let root = Ctx { store: ctx.store.clone(), uid: Some(0) };
        assert!(call(&root, "prepare", &archive(&manifest_with("{}"))).is_err());
        assert!(call(&ctx, "frobnicate", b"").is_err());
        assert!(call(&ctx, "commit amethystora.x 00", b"").is_err());
        assert!(call(&ctx, "inspect ../../etc", b"").is_err());
        // An archive with a link, and one without a manifest
        let mut raw = archive(&manifest_with("{}"));
        raw[156] = b'2';
        assert!(call(&ctx, "prepare", &raw).is_err());
        let bare = tree::write_tar(&[Entry { path: "run.sh".into(), dir: false, exec: true, data: vec![] }]).unwrap();
        assert!(call(&ctx, "prepare", &bare).is_err());
        // A manifest that asks for a thing no plugin may
        assert!(call(&ctx, "prepare", &archive(&manifest_with(r#"{"root":{"why":"x"}}"#))).is_err());
        // The quarantine's quota
        for _ in 0..MAX_TICKETS {
            call(&ctx, "prepare", &archive(&manifest_with("{}"))).unwrap();
        }
        assert!(call(&ctx, "prepare", &archive(&manifest_with("{}"))).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_shipped_plugin_gets_its_manifest_and_never_a_reach_outside() {
        let (ctx, dir) = setup("shipped");
        let make = |id: &str, needs: &str| {
            let folder = ctx.store.shipped.join(id);
            std::fs::create_dir_all(&folder).unwrap();
            let manifest = manifest_with(needs).replace("dev.example.t", id);
            tree::write_tree(&folder, &[Entry { path: "manifest.json".into(), dir: false, exec: false, data: manifest.into_bytes() }]).unwrap();
        };
        make("amethystora.clock", r#"{"state":{"names":["clock"],"why":"The time"}}"#);
        make("amethystora.leaky", r#"{"network":[{"host":"a.example.com","why":"x"}]}"#);
        call(&ctx, "commit-shipped amethystora.clock", b"").unwrap();
        let grants = ctx.store.grants(1000).unwrap();
        assert!(grants.plugins["amethystora.clock"].shipped && grants.plugins["amethystora.clock"].perms == ["state=clock"]);
        assert!(call(&ctx, "commit-shipped amethystora.leaky", b"").is_err());
        assert!(call(&ctx, "commit-shipped amethystora.missing", b"").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
