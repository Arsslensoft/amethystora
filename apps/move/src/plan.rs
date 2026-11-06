//! What may cross, and how it lands. One place, asked by both sides: the old computer leaves out what the rules below refuse,
//! and the new one refuses it again when it arrives, since what comes over a network is somebody's text and the other side may
//! be anything. The rules are what the maintainer set for the move (plans/PLAN-ui-ideas-r7-build.md, R7-25):
//!
//! - **The person's own things:** the folders in the home that are not hidden (Documents, Pictures, Music, Videos, Downloads,
//!   Desktop and the person's own), the data of their Flatpak apps (`.var/app/ID`, less caches), and the Vault's own backup
//!   file, which is sealed under its recovery key and opens only with it. The Vault itself is the service's and never crosses.
//! - **Never:** passwords and keys. Browser profiles, logins and cookies, the keyrings, `.ssh` and `.gnupg`, the Private folder,
//!   and any hidden folder that is not named above (settings come with the setup file, which names things and holds no secret).
//!   Places that start programs (`bin`, `autostart`) are never written either.
//! - **It adds and never deletes:** a name that is already there is kept as it is, and what is written is written under a
//!   temporary name and put in place only when whole.
//! - A link, a device or any file that is not a plain one is not sent: nothing that crosses can point outside the new home.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

/// A thing the sender offers and the receiver chooses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    /// The home folder: the person's own folders
    Home,
    /// The setup file, which names the apps, theme and shortcuts (`ame setup`), to be applied on the new computer by its own command
    Setup,
}

impl Item {
    pub fn id(self) -> &'static str {
        match self {
            Item::Home => "home",
            Item::Setup => "setup",
        }
    }

    pub fn from_id(id: &str) -> Option<Item> {
        match id {
            "home" => Some(Item::Home),
            "setup" => Some(Item::Setup),
            _ => None,
        }
    }
}

const MOST_PATH: usize = 4096;
const MOST_NAME: usize = 255;
/// What is walked for at most: a home with more is not a person's folders
pub const MOST_FILES: u64 = 20_000_000;

/// Names that start programs or hold keys, wherever they are
const DENIED_ANYWHERE: [&str; 5] = [".ssh", ".gnupg", "keyrings", "autostart", ".mozilla"];
/// Under a Flatpak's data, where mail and browsers keep what they are logged in with
const DENIED_IN_APPS: [&str; 24] = [
    "logins.json", "key3.db", "key4.db", "cert9.db", "signons.sqlite", "Login Data", "Login Data For Account", "Cookies", "Web Data", "Local State",
    "BraveSoftware", "google-chrome", "chromium", "microsoft-edge", "vivaldi", "opera", "Cache", "Code Cache", "GPUCache", "cache", ".cache", "tmp",
    ".mozilla", "keyrings",
];
/// A Flatpak whose data is a browser's: its profile holds logins whichever file they are in
const BROWSERS: [&str; 14] = [
    "org.mozilla.firefox", "org.chromium.", "com.google.Chrome", "com.brave.", "com.microsoft.Edge", "com.vivaldi.", "com.opera.",
    "io.gitlab.librewolf-community", "org.gnome.Epiphany", "one.ablaze.floorp", "app.zen_browser.zen", "net.waterfox.", "org.torproject.", "io.github.ungoogled_software.",
];

/// A path made of plain names: relative, no empty part, no `.` or `..`, nothing a file system would read as more than a name
pub fn sane(parts: &[&str]) -> bool {
    !parts.is_empty()
        && parts.iter().map(|p| p.len() + 1).sum::<usize>() <= MOST_PATH
        && parts.iter().all(|p| !p.is_empty() && *p != "." && *p != ".." && p.len() <= MOST_NAME && !p.contains(['\0', '/']))
}

fn app_id_ok(id: &str) -> bool {
    id.contains('.') && id.len() <= 128 && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// A part of a path the rules refuse, in any place
fn blocked(parts: &[&str]) -> bool {
    if parts.iter().any(|p| DENIED_ANYWHERE.contains(p) || p.ends_with(".keyring") || p.ends_with(".keystore")) {
        return true;
    }
    if parts.first() == Some(&".var") && parts.len() > 2 && parts[1] == "app" {
        if BROWSERS.iter().any(|b| parts[2].starts_with(b)) {
            return true;
        }
        // The app's own folders: only what its own parts (data, config) hold, less what is a login or a cache
        return parts[3..].iter().any(|p| DENIED_IN_APPS.contains(p));
    }
    false
}

/// Whether a file at this place under the home may cross
pub fn allows(item: Item, rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    if !sane(&parts) {
        return false;
    }
    match item {
        Item::Setup => rel == "setup.json",
        Item::Home => {
            if blocked(&parts) {
                return false;
            }
            match parts[0] {
                ".var" => parts.len() >= 4 && parts[1] == "app" && app_id_ok(parts[2]),
                ".local" => parts.len() == 5 && parts[1] == "share" && parts[2] == "amethystora" && parts[3] == "vault-backup" && parts[4].ends_with(".avb"),
                first if first.starts_with('.') => false,
                "bin" | "Private" => false,
                _ => true,
            }
        }
    }
}

/// A file that is to go
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub rel: String,
    pub size: u64,
    pub mode: u32,
    pub mtime: i64,
}

/// What a walk found, and what it passed over
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Totals {
    pub files: u64,
    pub bytes: u64,
    /// Links, devices and the like, and what the rules refuse: said, never listed
    pub left_out: u64,
}

/// Walks the places of the home that the rules open, calling `visit` for each plain file that may go. A visit that returns
/// false stops the walk. Links are never followed and never sent.
pub fn walk(home: &Path, visit: &mut dyn FnMut(Entry) -> bool) -> io::Result<Totals> {
    let mut totals = Totals::default();
    let mut roots: Vec<String> = Vec::new();
    for entry in fs::read_dir(home)? {
        let name = entry?.file_name();
        let Some(name) = name.to_str() else {
            totals.left_out += 1;
            continue;
        };
        if !name.starts_with('.') && !matches!(name, "bin" | "Private") {
            roots.push(name.to_string());
        }
    }
    roots.sort();
    // The Flatpaks' data and the Vault's own backup are the only hidden places
    if let Ok(apps) = fs::read_dir(home.join(".var/app")) {
        let mut ids: Vec<String> = apps.flatten().filter_map(|e| e.file_name().to_str().map(str::to_string)).filter(|id| app_id_ok(id)).collect();
        ids.sort();
        roots.extend(ids.into_iter().map(|id| format!(".var/app/{id}")));
    }
    roots.push(".local/share/amethystora/vault-backup".to_string());
    for root in roots {
        if !walk_into(home, &root, visit, &mut totals)? {
            break;
        }
    }
    Ok(totals)
}

fn walk_into(home: &Path, rel: &str, visit: &mut dyn FnMut(Entry) -> bool, totals: &mut Totals) -> io::Result<bool> {
    let path = home.join(rel);
    let Ok(meta) = fs::symlink_metadata(&path) else { return Ok(true) };
    if meta.file_type().is_dir() {
        let parts: Vec<&str> = rel.split('/').collect();
        if !sane(&parts) || blocked(&parts) {
            totals.left_out += 1;
            return Ok(true);
        }
        let Ok(read) = fs::read_dir(&path) else { return Ok(true) };
        let mut names: Vec<String> = Vec::new();
        for entry in read.flatten() {
            match entry.file_name().to_str() {
                Some(name) => names.push(name.to_string()),
                None => totals.left_out += 1,
            }
        }
        names.sort();
        for name in names {
            if !walk_into(home, &format!("{rel}/{name}"), visit, totals)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    if !meta.file_type().is_file() || !allows(Item::Home, rel) {
        totals.left_out += 1;
        return Ok(true);
    }
    if totals.files >= MOST_FILES {
        return Ok(false);
    }
    totals.files += 1;
    totals.bytes += meta.len();
    Ok(visit(Entry { rel: rel.to_string(), size: meta.len(), mode: meta.mode() & 0o777, mtime: meta.mtime() }))
}

/// How many bytes can still be written under a folder
pub fn free_space(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a NUL-terminated path and `stat` is a statvfs the call fills
    (unsafe { libc::statvfs(c.as_ptr(), &mut stat) } == 0).then(|| stat.f_bavail as u64 * stat.f_frsize)
}

/// What came of one file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Landed {
    Written,
    /// There already: kept as it was
    Kept,
    /// The rules refuse it
    Refused,
}

/// Writes the files that come, under one root, by the rules above
pub struct Writer {
    root: PathBuf,
    item: Item,
    current: Option<Current>,
    pub written: u64,
    pub kept: u64,
    pub refused: u64,
    pub bytes: u64,
}

struct Current {
    rel: String,
    size: u64,
    mode: u32,
    mtime: i64,
    got: u64,
    /// None: this file is not written (kept or refused), and what comes is only counted
    target: Option<(File, PathBuf, PathBuf)>,
    landed: Landed,
}

fn io_err(why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, why.to_string())
}

impl Writer {
    pub fn new(root: PathBuf, item: Item) -> Writer {
        Writer { root, item, current: None, written: 0, kept: 0, refused: 0, bytes: 0 }
    }

    /// A file begins. Refused or kept, its data is still to come and is only counted.
    pub fn begin(&mut self, rel: &str, size: u64, mode: u32, mtime: i64) -> io::Result<()> {
        if self.current.is_some() {
            return Err(io_err("a file began before the last one ended"));
        }
        let mut cur = Current { rel: rel.to_string(), size, mode, mtime, got: 0, target: None, landed: Landed::Refused };
        if allows(self.item, rel) {
            let path = self.root.join(rel);
            if fs::symlink_metadata(&path).is_ok() {
                cur.landed = Landed::Kept;
            } else {
                let parent = path.parent().ok_or_else(|| io_err("no folder"))?;
                fs::create_dir_all(parent)?;
                let mut name = String::from(".amethystora-move-");
                let mut random = [0u8; 6];
                getrandom::fill(&mut random).map_err(|_| io_err("no random bytes"))?;
                for b in random {
                    name.push_str(&format!("{b:02x}"));
                }
                let temporary = parent.join(name);
                // Private until whole, never followed through a link, never over something
                let file = OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(&temporary)?;
                cur.target = Some((file, temporary, path));
                cur.landed = Landed::Written;
            }
        }
        self.current = Some(cur);
        Ok(())
    }

    pub fn data(&mut self, chunk: &[u8]) -> io::Result<()> {
        let cur = self.current.as_mut().ok_or_else(|| io_err("data with no file"))?;
        cur.got += chunk.len() as u64;
        if cur.got > cur.size {
            return Err(io_err("a file is longer than it said"));
        }
        if let Some((file, _, _)) = cur.target.as_mut() {
            file.write_all(chunk)?;
        }
        Ok(())
    }

    /// The file is whole: put in place if it is to be written
    pub fn end(&mut self) -> io::Result<Landed> {
        let cur = self.current.take().ok_or_else(|| io_err("the end of no file"))?;
        if cur.got != cur.size {
            if let Some((_, temporary, _)) = &cur.target {
                let _ = fs::remove_file(temporary);
            }
            return Err(io_err("a file is shorter than it said"));
        }
        self.bytes += cur.got;
        let landed = match cur.target {
            Some((file, temporary, path)) => {
                // Group and others may read what the owner could, and nobody gets a setuid bit or a write they did not have
                let mode = (cur.mode & 0o755) | 0o600;
                let finish = (|| -> io::Result<Landed> {
                    file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(mode))?;
                    if cur.mtime > 0 {
                        file.set_modified(UNIX_EPOCH + Duration::from_secs(cur.mtime as u64))?;
                    }
                    file.sync_all()?;
                    drop(file);
                    match fs::hard_link(&temporary, &path) {
                        Ok(()) => Ok(Landed::Written),
                        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(Landed::Kept),
                        // A file system with no hard links: the name is free as far as this can tell
                        Err(_) if !path.exists() => fs::rename(&temporary, &path).map(|()| Landed::Written),
                        Err(e) => Err(e),
                    }
                })();
                let _ = fs::remove_file(&temporary);
                finish?
            }
            None => cur.landed,
        };
        match landed {
            Landed::Written => self.written += 1,
            Landed::Kept => self.kept += 1,
            Landed::Refused => self.refused += 1,
        }
        let _ = cur.rel;
        Ok(landed)
    }

    /// A move that stops in the middle: the file being written goes
    pub fn abandon(&mut self) {
        if let Some(Current { target: Some((_, temporary, _)), .. }) = self.current.take() {
            let _ = fs::remove_file(temporary);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_may_cross_is_the_persons_own_and_no_secret() {
        for ok in ["Documents/a.odt", "Pictures/2026/x.jpg", "Music/a b/c.flac", "Downloads/x", "Desktop/y", "Projects/code/main.rs", ".var/app/org.gnome.Notes/data/n.db", ".local/share/amethystora/vault-backup/vault-2026-11-06.avb"] {
            assert!(allows(Item::Home, ok), "{ok}");
        }
        for no in [
            ".ssh/id_ed25519", "Documents/.ssh/x", ".gnupg/pubring.kbx", ".bashrc", ".config/autostart/a.desktop", ".local/bin/tool", ".local/share/keyrings/login.keyring",
            ".mozilla/firefox/p/logins.json", ".var/app/org.mozilla.firefox/.mozilla/x", ".var/app/com.brave.Browser/config/x", ".var/app/org.mozilla.Thunderbird/.thunderbird/p/logins.json",
            ".var/app/org.mozilla.Thunderbird/.thunderbird/p/key4.db", ".var/app/org.gnome.Notes/cache/x", ".var/app/org.gnome.Notes", "bin/x", "Private/secret.txt",
            ".local/share/amethystora/private/gocryptfs.conf", ".local/share/amethystora/vault-backup/other.txt", ".var/app/x/y", "Documents/login.keyring", "Documents/autostart/x",
            "../etc/passwd", "/etc/passwd", "Documents/../.ssh/x", "Documents//x", "", "a/./b", ".config/dconf/user",
        ] {
            assert!(!allows(Item::Home, no), "{no}");
        }
        assert!(allows(Item::Setup, "setup.json") && !allows(Item::Setup, "Documents/x") && !allows(Item::Setup, "setup.json/x"));
    }

    fn tree() -> PathBuf {
        let root = std::env::temp_dir().join(format!("amethystora-move-plan-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn put(root: &Path, rel: &str, content: &[u8]) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn a_walk_finds_the_files_that_may_go_and_follows_no_link() {
        let home = tree();
        put(&home, "Documents/a.txt", b"aaa");
        put(&home, "Pictures/p.jpg", b"pp");
        put(&home, ".ssh/id_rsa", b"KEY");
        put(&home, ".mozilla/firefox/logins.json", b"{}");
        put(&home, "Private/x", b"x");
        put(&home, ".var/app/org.gnome.Notes/data/n", b"n");
        put(&home, ".var/app/org.gnome.Notes/cache/c", b"c");
        put(&home, ".var/app/org.mozilla.firefox/data/f", b"f");
        put(&home, ".config/amethystora/connect/private-key.pem", b"KEY");
        put(&home, ".local/share/amethystora/vault-backup/v.avb", b"sealed");
        std::os::unix::fs::symlink("/etc", home.join("Documents/etc-link")).unwrap();
        std::os::unix::fs::symlink(home.join(".ssh/id_rsa"), home.join("Pictures/key-link")).unwrap();
        let mut seen = Vec::new();
        let totals = walk(&home, &mut |e| {
            seen.push(e.rel);
            true
        })
        .unwrap();
        seen.sort();
        assert_eq!(seen, vec![".local/share/amethystora/vault-backup/v.avb", ".var/app/org.gnome.Notes/data/n", "Documents/a.txt", "Pictures/p.jpg"]);
        assert_eq!(totals.files, 4);
        assert_eq!(totals.bytes, 3 + 2 + 1 + 6);
        assert!(totals.left_out >= 2, "the two links were passed over: {totals:?}");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn a_file_is_written_whole_under_its_name_and_never_over_one() {
        let root = tree();
        let mut w = Writer::new(root.clone(), Item::Home);
        w.begin("Documents/new.txt", 5, 0o644, 1_700_000_000).unwrap();
        w.data(b"he").unwrap();
        assert!(!root.join("Documents/new.txt").exists(), "not in place before it is whole");
        w.data(b"llo").unwrap();
        assert_eq!(w.end().unwrap(), Landed::Written);
        assert_eq!(fs::read(root.join("Documents/new.txt")).unwrap(), b"hello");
        assert_eq!(fs::metadata(root.join("Documents/new.txt")).unwrap().mtime(), 1_700_000_000);
        // A name that is there is kept
        put(&root, "Documents/mine.txt", b"mine");
        w.begin("Documents/mine.txt", 3, 0o644, 0).unwrap();
        w.data(b"old").unwrap();
        assert_eq!(w.end().unwrap(), Landed::Kept);
        assert_eq!(fs::read(root.join("Documents/mine.txt")).unwrap(), b"mine");
        // What the rules refuse is counted and not written
        w.begin(".ssh/id_rsa", 3, 0o600, 0).unwrap();
        w.data(b"key").unwrap();
        assert_eq!(w.end().unwrap(), Landed::Refused);
        assert!(!root.join(".ssh").exists());
        w.begin("../escape", 1, 0o644, 0).unwrap();
        w.data(b"x").unwrap();
        assert_eq!(w.end().unwrap(), Landed::Refused);
        assert!(!root.parent().unwrap().join("escape").exists());
        assert_eq!((w.written, w.kept, w.refused), (1, 1, 2));
        // No temporary file is left
        let left: Vec<_> = fs::read_dir(root.join("Documents")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(left.iter().all(|n| !n.starts_with(".amethystora-move-")), "{left:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_that_says_the_wrong_length_is_an_error_and_leaves_nothing() {
        let root = tree();
        let mut w = Writer::new(root.clone(), Item::Home);
        w.begin("Documents/short.txt", 10, 0o644, 0).unwrap();
        w.data(b"abc").unwrap();
        assert!(w.end().is_err());
        assert!(!root.join("Documents/short.txt").exists());
        let mut w = Writer::new(root.clone(), Item::Home);
        w.begin("Documents/long.txt", 2, 0o644, 0).unwrap();
        assert!(w.data(b"abc").is_err());
        w.abandon();
        assert!(fs::read_dir(root.join("Documents")).unwrap().flatten().count() == 0, "nothing left behind");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_mode_never_carries_a_setuid_bit_or_a_write_for_others() {
        let root = tree();
        let mut w = Writer::new(root.clone(), Item::Home);
        w.begin("Documents/x", 1, 0o4777, 0).unwrap();
        w.data(b"x").unwrap();
        w.end().unwrap();
        assert_eq!(fs::metadata(root.join("Documents/x")).unwrap().mode() & 0o7777, 0o755);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_may_not_be_written_through_a_link_that_is_in_the_way() {
        let root = tree();
        let outside = tree().join("outside");
        fs::create_dir_all(&outside).unwrap();
        // The new computer's own link (a folder moved to another disk) is followed: the person made it. A link to a file is the name, kept.
        std::os::unix::fs::symlink(outside.join("target.txt"), root.join("link.txt")).unwrap();
        let mut w = Writer::new(root.clone(), Item::Home);
        w.begin("link.txt", 1, 0o644, 0).unwrap();
        w.data(b"x").unwrap();
        assert_eq!(w.end().unwrap(), Landed::Kept, "a dangling link counts as the name being taken");
        assert!(!outside.join("target.txt").exists());
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(outside.parent().unwrap());
    }
}
