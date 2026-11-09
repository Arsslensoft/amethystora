//! Protected files (plans/done/PLAN-vault.md): a file or folder taken out of the home and into the vault, sealed, and reached only
//! through `ame vault run`, which has it in a namespace of that one command for as long as it runs. All of this runs as the account, in
//! the command line: the service has no home, and holds the sealed item like any other.
//!
//! Every path is walked one name at a time from the home, never through a link. What an item says is a path is checked each time it
//! is used, since any program of the account can make one.

use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use amethystora_vault_core::protocol::MAX_DATA;
use amethystora_vault_core::seal;

pub const KIND: &str = "protected";
/// One item's value is at most `MAX_DATA` as JSON, and base64 takes a third more: a key or a login, not a disk
const MOST_BYTES: usize = MAX_DATA / 4 * 3 - 64 * 1024;
const MOST_ENTRIES: usize = 256;
const MOST_DEPTH: usize = 16;
const MOST_PATH: usize = 200;
/// The room a command has in memory for what it was given, what it rewrites and what it makes beside it: 512 MiB, used only as it is filled. What it
/// leaves in the file itself over `MOST_BYTES` cannot be sealed again, and says so.
const TMPFS_OPTIONS: &str = "size=512m,mode=0700";

/// Why not, each a way out of the command line's own numbers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// Not in the home, through a link, or not a plain file or folder
    NotPlain,
    /// Nothing is there
    Missing,
    /// `~/.ssh/config`, `known_hosts` or a `.pub`: they stay where they are
    NoSecret,
    TooLarge,
    /// A file with something in it is at the path a protected file would go
    FileThere,
    /// Another command has this one open
    InUse,
    /// It changed while it was being protected, and nothing was removed
    Changed,
    /// This kernel, or its settings, gives an account no namespace
    NoNamespace,
    Disk,
    /// It is protected already, and sealing this over it would lose what is sealed
    AlreadyProtected,
    /// The file has another name, which would keep its plain content
    Linked,
    /// Something above it cannot be changed, so nothing was moved
    ReadOnly,
    /// What a command ran over was taken from under it
    Detached,
}

impl Why {
    pub fn code(self) -> u8 {
        match self {
            Why::TooLarge => 4,
            Why::Disk => 6,
            Why::NotPlain => 10,
            Why::Missing => 11,
            Why::NoSecret => 12,
            Why::FileThere => 13,
            Why::InUse => 14,
            Why::Changed => 15,
            Why::NoNamespace => 16,
            Why::AlreadyProtected => 18,
            Why::Linked => 19,
            Why::ReadOnly => 20,
            Why::Detached => 21,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    File,
    Folder,
}

impl What {
    pub fn name(self) -> &'static str {
        match self {
            What::File => "file",
            What::Folder => "folder",
        }
    }
}

/// When it was read, to know whether what is about to be removed is what was read
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    inode: u64,
    size: i64,
    seconds: i64,
    nanos: i64,
}

fn stamp_of(stat: &libc::stat) -> Stamp {
    Stamp { inode: stat.st_ino, size: stat.st_size, seconds: stat.st_mtime, nanos: stat.st_mtime_nsec }
}

/// One file of a tree: `at` is its place under the tree's root, empty for a tree that is one file
pub struct Held {
    pub at: String,
    pub mode: u32,
    pub data: Zeroizing<Vec<u8>>,
    stamp: Option<Stamp>,
}

/// What is sealed: one file, or a folder of them
pub struct Tree {
    pub what: What,
    pub files: Vec<Held>,
    /// Under the root, parents first
    pub folders: Vec<(String, u32)>,
    /// Things a command left that are not plain files or folders, which are not sealed
    pub skipped: usize,
    /// A folder's own mode, which an item sealed before it was kept does not say
    pub root_mode: Option<u32>,
}

/// One name of a path as it may be stored: nothing that leaves the place it is in, and nothing that cannot be shown
fn plain_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && name.len() <= 255 && !name.contains('/') && !name.chars().any(char::is_control)
}

/// A path under the home as an item may hold it
pub fn plain_path(path: &str) -> bool {
    path.len() <= MOST_PATH && path.split('/').count() <= MOST_DEPTH && path.split('/').all(plain_name)
}

/// What the plan says holds no secret: it stays, even in a folder that is protected
pub fn holds_no_secret(path: &str) -> bool {
    path.ends_with(".pub") || matches!(path, ".ssh/config" | ".ssh/known_hosts" | ".ssh/known_hosts.old" | ".ssh/authorized_keys")
}

fn c_name(name: &str) -> Result<CString, Why> {
    CString::new(name).map_err(|_| Why::NotPlain)
}

fn c_path(path: &Path) -> Result<CString, Why> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| Why::NotPlain)
}

fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// `openat`, owning what it opens
fn open_at(folder: RawFd, name: &str, flags: i32, mode: u32) -> Result<OwnedFd, Why> {
    let name = c_name(name)?;
    // SAFETY: a live string; the descriptor returned is then owned
    let fd = unsafe { libc::openat(folder, name.as_ptr(), flags | libc::O_CLOEXEC | libc::O_NOFOLLOW, mode) };
    if fd < 0 {
        return Err(match errno() {
            libc::ENOENT => Why::Missing,
            libc::ELOOP | libc::ENOTDIR | libc::ENXIO | libc::EWOULDBLOCK => Why::NotPlain,
            _ => Why::Disk,
        });
    }
    // SAFETY: as above
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn stat_of(fd: RawFd) -> Result<libc::stat, Why> {
    // SAFETY: fstat writes one value of this type, and zero is a valid one
    unsafe {
        let mut stat = std::mem::zeroed::<libc::stat>();
        if libc::fstat(fd, &raw mut stat) != 0 {
            return Err(Why::Disk);
        }
        Ok(stat)
    }
}

const fn is_kind(stat: &libc::stat, kind: u32) -> bool {
    stat.st_mode & libc::S_IFMT == kind
}

/// The home as a descriptor, which every walk starts from
pub fn open_home(home: &Path) -> Result<OwnedFd, Why> {
    let path = c_path(home)?;
    // SAFETY: a live string; the home itself may be reached through a link (`/home` on an atomic image)
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(Why::Disk);
    }
    // SAFETY: as above
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// The folder a path's last name is in, walked from the home, never through a link. With `make`, what is missing is made, for the account alone.
fn walk(home: &OwnedFd, path: &str, make: bool) -> Result<(OwnedFd, String), Why> {
    if !plain_path(path) {
        return Err(Why::NotPlain);
    }
    let mut names: Vec<&str> = path.split('/').collect();
    let last = names.pop().ok_or(Why::NotPlain)?.to_string();
    let mut folder = open_at(home.as_raw_fd(), ".", libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
    for name in names {
        if make {
            let name = c_name(name)?;
            // SAFETY: a live string; there already is the usual answer
            unsafe { libc::mkdirat(folder.as_raw_fd(), name.as_ptr(), 0o700) };
        }
        folder = open_at(folder.as_raw_fd(), name, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
    }
    Ok((folder, last))
}

/// The path under the home that a path as typed names, or why not. Through a link above it is fine if it ends up in the home, since
/// the name kept is the real one, and the walk then takes no link.
pub fn under_home(home: &Path, typed: &Path) -> Result<String, Why> {
    let typed = if typed.is_absolute() { typed.to_path_buf() } else { std::env::current_dir().map_err(|_| Why::Disk)?.join(typed) };
    let mut plain = PathBuf::new();
    for part in typed.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => return Err(Why::NotPlain),
            part => plain.push(part),
        }
    }
    let inside = plain.strip_prefix(home).map(Path::to_path_buf).or_else(|_| {
        let real_home = home.canonicalize().map_err(|_| Why::Disk)?;
        let name = plain.file_name().ok_or(Why::NotPlain)?;
        let parent = plain.parent().ok_or(Why::NotPlain)?.canonicalize().map_err(|_| Why::Missing)?;
        parent.join(name).strip_prefix(&real_home).map(Path::to_path_buf).map_err(|_| Why::NotPlain)
    })?;
    let path = inside.to_str().ok_or(Why::NotPlain)?.to_string();
    if plain_path(&path) { Ok(path) } else { Err(Why::NotPlain) }
}

fn read_file(fd: OwnedFd, stat: &libc::stat, room: &mut usize) -> Result<Zeroizing<Vec<u8>>, Why> {
    let size = usize::try_from(stat.st_size).map_err(|_| Why::Disk)?;
    if size > *room {
        return Err(Why::TooLarge);
    }
    let mut data = Zeroizing::new(Vec::with_capacity(size + 1));
    // It can grow after the look at its size: no more than the room is ever read
    File::from(fd).take(*room as u64 + 1).read_to_end(&mut data).map_err(|_| Why::Disk)?;
    if data.len() > *room {
        return Err(Why::TooLarge);
    }
    *room -= data.len();
    Ok(data)
}

/// The names in a folder, in order, each of which can be shown
fn names_in(folder: &OwnedFd) -> Result<Vec<String>, Why> {
    names_of(folder, None)
}

/// With a count, a name that cannot be shown is passed over and counted instead of refused
fn names_of(folder: &OwnedFd, skipped: Option<&mut usize>) -> Result<Vec<String>, Why> {
    names_up_to(folder, skipped, MOST_ENTRIES)
}

/// `names_of` with a limit of its own: what the command made beside a file is bounded by its memory, not by what an item holds
fn names_up_to(folder: &OwnedFd, mut skipped: Option<&mut usize>, most: usize) -> Result<Vec<String>, Why> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(format!("/proc/self/fd/{}", folder.as_raw_fd())).map_err(|_| Why::Disk)? {
        let name = entry.map_err(|_| Why::Disk)?.file_name();
        let Some(name) = name.to_str().filter(|name| plain_name(name)) else {
            match skipped.as_deref_mut() {
                Some(count) => {
                    *count += 1;
                    continue;
                }
                None => return Err(Why::NotPlain),
            }
        };
        names.push(name.to_string());
        if names.len() > most {
            return Err(Why::TooLarge);
        }
    }
    names.sort();
    Ok(names)
}

#[allow(clippy::too_many_arguments)]
fn read_tree(folder: &OwnedFd, under: &str, whole: &str, tree: &mut Tree, room: &mut usize, skip: &[String], depth: usize, lenient: bool) -> Result<(), Why> {
    if depth >= MOST_DEPTH {
        return Err(Why::TooLarge);
    }
    let mut passed = 0;
    let names = names_of(folder, lenient.then_some(&mut passed))?;
    tree.skipped += passed;
    for name in names {
        let at = if under.is_empty() { name.clone() } else { format!("{under}/{name}") };
        let full = if whole.is_empty() { at.clone() } else { format!("{whole}/{at}") };
        if skip.contains(&at) {
            continue;
        }
        let fd = match open_at(folder.as_raw_fd(), &name, libc::O_RDONLY | libc::O_NONBLOCK, 0) {
            Ok(fd) => fd,
            Err(Why::NotPlain) if lenient => {
                tree.skipped += 1;
                continue;
            }
            Err(why) => return Err(why),
        };
        let stat = stat_of(fd.as_raw_fd())?;
        let folder_kind = is_kind(&stat, libc::S_IFDIR);
        // What holds no secret is a file by that name: a folder called `x.pub` is a folder like any other
        if !folder_kind && holds_no_secret(&full) {
            continue;
        }
        if tree.files.len() + tree.folders.len() >= MOST_ENTRIES {
            return Err(Why::TooLarge);
        }
        if folder_kind {
            tree.folders.push((at.clone(), stat.st_mode & 0o777));
            read_tree(&fd, &at, whole, tree, room, skip, depth + 1, lenient)?;
        } else if is_kind(&stat, libc::S_IFREG) {
            if stat.st_nlink > 1 {
                if lenient {
                    tree.skipped += 1;
                    continue;
                }
                return Err(Why::Linked);
            }
            let stamp = stamp_of(&stat);
            let data = read_file(fd, &stat, room)?;
            tree.files.push(Held { at, mode: stat.st_mode & 0o777, data, stamp: Some(stamp) });
        } else if lenient {
            tree.skipped += 1;
        } else {
            return Err(Why::NotPlain);
        }
    }
    Ok(())
}

/// What is at this path, read whole, or why it cannot be protected. Nothing is changed.
pub fn read(home: &OwnedFd, path: &str) -> Result<Tree, Why> {
    if holds_no_secret(path) {
        return Err(Why::NoSecret);
    }
    let (folder, last) = walk(home, path, false)?;
    let fd = open_at(folder.as_raw_fd(), &last, libc::O_RDONLY | libc::O_NONBLOCK, 0)?;
    read_opened(fd, path, &[], false)
}

/// With `lenient`, what a command left that is not a plain file or folder is passed over and counted rather than refused
fn read_opened(fd: OwnedFd, path: &str, skip: &[String], lenient: bool) -> Result<Tree, Why> {
    let stat = stat_of(fd.as_raw_fd())?;
    let mut room = MOST_BYTES;
    if is_kind(&stat, libc::S_IFREG) {
        let stamp = stamp_of(&stat);
        let mode = stat.st_mode & 0o777;
        let data = read_file(fd, &stat, &mut room)?;
        // A file a command left with another name is read all the same: that name was in its own private space, and is gone with it
        if stat.st_nlink > 1 && !lenient {
            return Err(Why::Linked);
        }
        return Ok(Tree::one_file(data, mode, Some(stamp)));
    }
    if !is_kind(&stat, libc::S_IFDIR) {
        return Err(Why::NotPlain);
    }
    let mut tree = Tree { what: What::Folder, files: Vec::new(), folders: Vec::new(), skipped: 0, root_mode: Some(stat.st_mode & 0o777) };
    read_tree(&fd, "", path, &mut tree, &mut room, skip, 0, lenient)?;
    Ok(tree)
}

impl Tree {
    fn one_file(data: Zeroizing<Vec<u8>>, mode: u32, stamp: Option<Stamp>) -> Tree {
        Tree { what: What::File, files: vec![Held { at: String::new(), mode, data, stamp }], folders: Vec::new(), skipped: 0, root_mode: None }
    }

    /// A file as it was sealed, from what it held
    pub fn file(data: &[u8], mode: u32) -> Tree {
        Tree::one_file(Zeroizing::new(data.to_vec()), mode, None)
    }

    /// As the item's value: all of it fits a line of the socket, or it is too large to be an item
    pub fn to_value(&self) -> Result<Value, Why> {
        let files: Vec<Value> = self.files.iter().map(|file| json!({"at": file.at, "mode": file.mode, "data": STANDARD.encode(&*file.data)})).collect();
        let folders: Vec<Value> = self.folders.iter().map(|(at, mode)| json!({"at": at, "mode": mode})).collect();
        let mut value = json!({"what": self.what.name(), "files": files, "folders": folders});
        if let (What::Folder, Some(mode)) = (self.what, self.root_mode) {
            value["mode"] = json!(mode);
        }
        if value.to_string().len() > MAX_DATA {
            return Err(Why::TooLarge);
        }
        Ok(value)
    }

    /// From an item's value, which is only as true as whatever wrote it: refused rather than repaired
    pub fn from_value(value: &Value) -> Result<Tree, Why> {
        let what = match value["what"].as_str() {
            Some("file") => What::File,
            Some("folder") => What::Folder,
            _ => return Err(Why::NotPlain),
        };
        let mode_of = |entry: &Value| entry["mode"].as_u64().filter(|mode| *mode <= 0o777).map(|mode| mode as u32).ok_or(Why::NotPlain);
        let at_of = |entry: &Value| -> Result<String, Why> {
            let at = entry["at"].as_str().ok_or(Why::NotPlain)?;
            if plain_path(at) { Ok(at.to_string()) } else { Err(Why::NotPlain) }
        };
        let mut files = Vec::new();
        let mut room = MAX_DATA;
        for entry in value["files"].as_array().ok_or(Why::NotPlain)?.iter().take(MOST_ENTRIES + 1) {
            let at = if what == What::File { String::new() } else { at_of(entry)? };
            let data = Zeroizing::new(STANDARD.decode(entry["data"].as_str().ok_or(Why::NotPlain)?).map_err(|_| Why::NotPlain)?);
            room = room.checked_sub(data.len()).ok_or(Why::TooLarge)?;
            files.push(Held { at, mode: mode_of(entry)?, data, stamp: None });
        }
        let mut folders = Vec::new();
        for entry in value["folders"].as_array().ok_or(Why::NotPlain)?.iter().take(MOST_ENTRIES + 1) {
            folders.push((at_of(entry)?, mode_of(entry)?));
        }
        if files.len() + folders.len() > MOST_ENTRIES || (what == What::File && (files.len() != 1 || !folders.is_empty())) {
            return Err(Why::NotPlain);
        }
        let root_mode = match &value["mode"] {
            Value::Null => None,
            _ if what == What::Folder => Some(mode_of(value)?),
            _ => return Err(Why::NotPlain),
        };
        Ok(Tree { what, files, folders, skipped: 0, root_mode })
    }

    /// This folder's files with another's added. A name both hold with something else in it is a conflict, never a choice made here.
    pub fn merged_with(self, other: &Tree) -> Result<Tree, Why> {
        if self.what != What::Folder || other.what != What::Folder {
            return Err(Why::AlreadyProtected);
        }
        let Tree { what, mut files, mut folders, skipped, root_mode } = self;
        for file in &other.files {
            match files.iter().find(|held| held.at == file.at) {
                Some(held) if held.mode == file.mode && seal::same(&held.data, &file.data) => {}
                Some(_) => return Err(Why::AlreadyProtected),
                None => files.push(Held { at: file.at.clone(), mode: file.mode, data: Zeroizing::new(file.data.to_vec()), stamp: None }),
            }
        }
        for folder in &other.folders {
            if !folders.iter().any(|(at, _)| *at == folder.0) {
                folders.push(folder.clone());
            }
        }
        folders.sort();
        files.sort_by(|one, two| one.at.cmp(&two.at));
        if files.len() + folders.len() > MOST_ENTRIES {
            return Err(Why::TooLarge);
        }
        Ok(Tree { what, files, folders, skipped, root_mode: root_mode.or(other.root_mode) })
    }

    pub fn size(&self) -> usize {
        self.files.iter().map(|file| file.data.len()).sum()
    }

    /// The same files with the same contents and modes
    pub fn same_as(&self, other: &Tree) -> bool {
        self.what == other.what
            && self.folders == other.folders
            && self.files.len() == other.files.len()
            && self.files.iter().zip(&other.files).all(|(one, two)| one.at == two.at && one.mode == two.mode && seal::same(&one.data, &two.data))
    }

    /// A digest of what it holds, for seeing whether a command changed it, without keeping it in memory while the command runs
    pub fn digest(&self) -> Vec<u8> {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        for (at, mode) in &self.folders {
            hasher.update((at.len() as u64).to_be_bytes());
            hasher.update(at.as_bytes());
            hasher.update(mode.to_be_bytes());
        }
        for file in &self.files {
            hasher.update((file.at.len() as u64).to_be_bytes());
            hasher.update(file.at.as_bytes());
            hasher.update(file.mode.to_be_bytes());
            hasher.update((file.data.len() as u64).to_be_bytes());
            hasher.update(file.data.as_slice());
        }
        hasher.finalize().to_vec()
    }
}

fn sync(fd: &OwnedFd) -> Result<(), Why> {
    // SAFETY: a live descriptor
    if unsafe { libc::fsync(fd.as_raw_fd()) } != 0 {
        return Err(Why::Disk);
    }
    Ok(())
}

fn folder_of(root: &OwnedFd, at: &str, make: bool) -> Result<(OwnedFd, String), Why> {
    let mut names: Vec<&str> = at.split('/').collect();
    let last = names.pop().ok_or(Why::NotPlain)?.to_string();
    let mut folder = open_at(root.as_raw_fd(), ".", libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
    for name in names {
        if make {
            let name = c_name(name)?;
            // SAFETY: a live string; there already is the usual answer
            unsafe { libc::mkdirat(folder.as_raw_fd(), name.as_ptr(), 0o700) };
        }
        folder = open_at(folder.as_raw_fd(), name, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
    }
    Ok((folder, last))
}

/// Takes the original away, after the sealed copy opened: unlinked, not wiped, since btrfs copies on write and a wipe would be a false
/// promise. Nothing goes unless all of it is still what was read.
pub fn remove_original(home: &OwnedFd, path: &str, tree: &Tree) -> Result<(), Why> {
    let (parent, last) = walk(home, path, false)?;
    let writable = |folder: &OwnedFd| -> Result<(), Why> {
        // SAFETY: a live descriptor and a live string
        if unsafe { libc::faccessat(folder.as_raw_fd(), c".".as_ptr(), libc::W_OK, 0) } == 0 { Ok(()) } else { Err(Why::ReadOnly) }
    };
    let same = |folder: &OwnedFd, name: &str, stamp: Option<Stamp>| -> Result<(), Why> {
        let fd = open_at(folder.as_raw_fd(), name, libc::O_PATH, 0)?;
        if Some(stamp_of(&stat_of(fd.as_raw_fd())?)) == stamp { Ok(()) } else { Err(Why::Changed) }
    };
    if tree.what == What::File {
        let file = tree.files.first().ok_or(Why::Disk)?;
        writable(&parent)?;
        same(&parent, &last, file.stamp)?;
        unlink(&parent, &last, 0)?;
        return sync(&parent);
    }
    let root = open_at(parent.as_raw_fd(), &last, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
    // Nothing is taken until all of it can be: a folder half-emptied is worse than one that was left
    for file in &tree.files {
        let (folder, name) = folder_of(&root, &file.at, false)?;
        writable(&folder)?;
        same(&folder, &name, file.stamp)?;
    }
    for file in &tree.files {
        let (folder, name) = folder_of(&root, &file.at, false)?;
        unlink(&folder, &name, 0)?;
        sync(&folder)?;
    }
    // A folder anything was left in stays
    for (at, _) in tree.folders.iter().rev() {
        let (folder, name) = folder_of(&root, at, false)?;
        let _ = unlink(&folder, &name, libc::AT_REMOVEDIR);
    }
    if unlink(&parent, &last, libc::AT_REMOVEDIR).is_ok() {
        sync(&parent)?;
    }
    Ok(())
}

fn unlink(folder: &OwnedFd, name: &str, flags: i32) -> Result<(), Why> {
    let name = c_name(name)?;
    // SAFETY: a live string
    if unsafe { libc::unlinkat(folder.as_raw_fd(), name.as_ptr(), flags) } != 0 {
        return Err(Why::Disk);
    }
    Ok(())
}

/// Puts a file in place: all of it or none, under a temporary name in the same folder, and never over a file that holds something
fn write_in(folder: &OwnedFd, name: &str, data: &[u8], mode: u32) -> Result<(), Why> {
    match open_at(folder.as_raw_fd(), name, libc::O_RDONLY | libc::O_NONBLOCK, 0) {
        Ok(fd) => {
            let stat = stat_of(fd.as_raw_fd())?;
            if !is_kind(&stat, libc::S_IFREG) || stat.st_size != 0 {
                return Err(Why::FileThere);
            }
        }
        Err(Why::Missing) => {}
        Err(Why::NotPlain) => return Err(Why::FileThere),
        Err(why) => return Err(why),
    }
    let temporary = format!(".amethystora-vault-{}", seal::random_hex(8));
    let fd = open_at(folder.as_raw_fd(), &temporary, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL, 0o600)?;
    let put = (|| -> Result<(), Why> {
        let mut file = File::from(fd);
        std::io::Write::write_all(&mut file, data).map_err(|_| Why::Disk)?;
        file.set_permissions(std::fs::Permissions::from_mode(mode)).map_err(|_| Why::Disk)?;
        file.sync_all().map_err(|_| Why::Disk)?;
        let (from, to) = (c_name(&temporary)?, c_name(name)?);
        // SAFETY: live strings and a live descriptor
        if unsafe { libc::renameat(folder.as_raw_fd(), from.as_ptr(), folder.as_raw_fd(), to.as_ptr()) } != 0 {
            return Err(Why::Disk);
        }
        sync(folder)
    })();
    if put.is_err() {
        let _ = unlink(folder, &temporary, 0);
    }
    put
}

/// Puts a tree back where it was: the way out of the vault, which makes an ordinary file again
pub fn restore(home: &OwnedFd, path: &str, tree: &Tree) -> Result<(), Why> {
    let (parent, last) = walk(home, path, true)?;
    if tree.what == What::File {
        let file = tree.files.first().ok_or(Why::Disk)?;
        write_in(&parent, &last, &file.data, file.mode)?;
    } else {
        let name = c_name(&last)?;
        // SAFETY: a live string; there already is the usual answer, for a folder something stayed in
        let made = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } == 0;
        let root = open_at(parent.as_raw_fd(), &last, libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
        for (at, mode) in &tree.folders {
            let (folder, name) = folder_of(&root, at, true)?;
            let wanted = c_name(&name)?;
            // SAFETY: a live string
            unsafe { libc::mkdirat(folder.as_raw_fd(), wanted.as_ptr(), *mode | 0o700) };
        }
        for file in &tree.files {
            let (folder, name) = folder_of(&root, &file.at, true)?;
            write_in(&folder, &name, &file.data, file.mode)?;
        }
        // The mode it had, once everything is in it. A folder something stayed in has its own already.
        if let (true, Some(mode)) = (made, tree.root_mode) {
            // SAFETY: a live descriptor
            if unsafe { libc::fchmod(root.as_raw_fd(), mode) } != 0 {
                return Err(Why::Disk);
            }
        }
    }
    // What was written reads back as it should, before anyone takes it from the vault
    let again = read_for_check(home, path, tree)?;
    if again { Ok(()) } else { Err(Why::Changed) }
}

fn read_for_check(home: &OwnedFd, path: &str, tree: &Tree) -> Result<bool, Why> {
    let (parent, last) = walk(home, path, false)?;
    let fd = open_at(parent.as_raw_fd(), &last, libc::O_RDONLY | libc::O_NONBLOCK, 0)?;
    let found = read_opened(fd, path, &[], false)?;
    Ok(tree.files.iter().all(|file| found.files.iter().any(|there| there.at == file.at && seal::same(&there.data, &file.data))))
}

// --- The namespace of one command ---------------------------------------------------------------------------------------------

fn write_file(path: &str, text: &str) -> Result<(), Why> {
    std::fs::write(path, text).map_err(|_| Why::NoNamespace)
}

/// Goes into a user, mount and process namespace of this process's own, with the same user: the next process it starts is the first in
/// the new one, and nothing started in there outlives it. Before any thread: the kernel refuses a process with
/// more than one. A namespace is worse than nothing for what is only a variable, which is why only a file comes here (plans/done/PLAN-vault.md).
pub fn enter_namespace() -> Result<(), Why> {
    // SAFETY: getuid and getgid cannot fail; unshare and mount are given live strings and flags
    unsafe {
        let (uid, gid) = (libc::getuid(), libc::getgid());
        if libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNS | libc::CLONE_NEWPID) != 0 {
            return Err(Why::NoNamespace);
        }
        write_file("/proc/self/setgroups", "deny")?;
        write_file("/proc/self/uid_map", &format!("{uid} {uid} 1"))?;
        write_file("/proc/self/gid_map", &format!("{gid} {gid} 1"))?;
        let root = c"/";
        if libc::mount(std::ptr::null(), root.as_ptr(), std::ptr::null(), libc::MS_REC | libc::MS_PRIVATE, std::ptr::null()) != 0 {
            return Err(Why::NoNamespace);
        }
        // What this process holds from here on is nobody else's. A `execve` sets it back, for the command (the spike's question 6).
        libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
    }
    Ok(())
}

fn mount_tmpfs(target: &Path) -> Result<(), Why> {
    let (target, options) = (c_path(target)?, c_name(TMPFS_OPTIONS)?);
    // SAFETY: live strings
    let done = unsafe { libc::mount(c"tmpfs".as_ptr(), target.as_ptr(), c"tmpfs".as_ptr(), libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC, options.as_ptr().cast()) };
    if done != 0 {
        return Err(Why::NoNamespace);
    }
    Ok(())
}

fn bind(from: &str, to: &Path) -> Result<(), Why> {
    let (from, to) = (c_name(from)?, c_path(to)?);
    // SAFETY: live strings
    let done = unsafe { libc::mount(from.as_ptr(), to.as_ptr(), std::ptr::null(), libc::MS_BIND | libc::MS_REC, std::ptr::null()) };
    if done != 0 {
        return Err(Why::NoNamespace);
    }
    Ok(())
}

fn detach(target: &Path) {
    if let Ok(target) = c_path(target) {
        // SAFETY: a live string
        unsafe { libc::umount2(target.as_ptr(), libc::MNT_DETACH) };
    }
}

fn write_private(path: &Path, data: &[u8], mode: u32) -> Result<(), Why> {
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(mode).custom_flags(libc::O_NOFOLLOW).open(path).map_err(|_| Why::Disk)?;
    std::io::Write::write_all(&mut file, data).map_err(|_| Why::Disk)?;
    file.set_permissions(std::fs::Permissions::from_mode(mode)).map_err(|_| Why::Disk)
}

/// How a command is given its file
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    /// A tmpfs over the file's whole folder, with what was beside it copied in or bound through: a tool that writes a temporary file
    /// next to its login and renames it over the login keeps both in the namespace, and nothing of it reaches the disk
    Parent,
    /// The file alone, bound over an empty one. For a file that sits in the home itself or in a folder too full to cover, where a
    /// rename over the mount fails and a temporary file beside it lands on the disk.
    Single,
    Folder,
}

/// Folders with more than this beside a protected file are not covered whole
const MOST_SIBLINGS: usize = 1024;
/// A file beside a protected one is copied into the command's memory up to this size, and all of them up to the second: a bind mount of
/// it could not be deleted, renamed or replaced by a rename (`sed -i`, `git config`, `ssh-keygen -R`). What is larger is bound.
const MOST_COPIED: u64 = 1024 * 1024;
const MOST_COPIED_ALL: u64 = 16 * 1024 * 1024;

/// A file or link that was beside a protected file, copied into the command's private space, with what it was on the disk
struct Beside {
    name: String,
    was: Was,
    /// As it was on the disk, to see whether something else changed it there while the command ran
    stamp: Stamp,
}

enum Was {
    File { data: Zeroizing<Vec<u8>>, mode: u32 },
    Link(Vec<u8>),
}

/// What a link says, read through its descriptor or its name in a folder (an empty name for the descriptor itself)
fn link_text(folder: RawFd, name: &str) -> Option<Vec<u8>> {
    let (name, mut target) = (c_name(name).ok()?, vec![0u8; 4096]);
    // SAFETY: a live string and a buffer of the length given
    let count = unsafe { libc::readlinkat(folder, name.as_ptr(), target.as_mut_ptr().cast(), target.len()) };
    if count <= 0 || count as usize >= target.len() {
        return None;
    }
    target.truncate(count as usize);
    Some(target)
}

/// What beside a protected file is copied, and what is bound through. Only the account's own plain files with no other name are copied, so
/// that putting a changed one back by a rename loses no owner and no other name.
fn copy_beside(parent: &OwnedFd, names: Vec<String>) -> Result<(Vec<Beside>, Vec<String>), Why> {
    let (mut copies, mut bound, mut room) = (Vec::new(), Vec::new(), MOST_COPIED_ALL);
    // SAFETY: getuid cannot fail
    let uid = unsafe { libc::getuid() };
    for name in names {
        let Ok(fd) = open_at(parent.as_raw_fd(), &name, libc::O_PATH, 0) else { continue };
        let stat = stat_of(fd.as_raw_fd())?;
        if is_kind(&stat, libc::S_IFLNK) {
            if let Some(target) = link_text(fd.as_raw_fd(), "") {
                copies.push(Beside { name, was: Was::Link(target), stamp: stamp_of(&stat) });
            }
            continue;
        }
        let size = u64::try_from(stat.st_size).unwrap_or(u64::MAX);
        if !is_kind(&stat, libc::S_IFREG) || stat.st_nlink != 1 || stat.st_uid != uid || size > MOST_COPIED || size > room {
            bound.push(name);
            continue;
        }
        let file = open_at(parent.as_raw_fd(), &name, libc::O_RDONLY | libc::O_NONBLOCK, 0)?;
        let mut left = usize::try_from(room).unwrap_or(usize::MAX);
        let data = read_file(file, &stat, &mut left)?;
        room -= data.len() as u64;
        copies.push(Beside { name, was: Was::File { data, mode: stat.st_mode & 0o777 }, stamp: stamp_of(&stat) });
    }
    Ok((copies, bound))
}

/// What stays of a protected folder on the disk and shows through it, as paths under it. A sealed file's path that is there, or something
/// other than a folder where a sealed folder goes, is a plain copy that came back: one of the two would hide the other, and be lost. A folder
/// both hold is looked into, so that only what stayed in it shows through and the rest of it is the vault's (a `.pub` beside a sealed key).
fn stays_in(folder: &OwnedFd, under: &str, tree: &Tree, stays: &mut Vec<String>, depth: usize) -> Result<(), Why> {
    if depth >= MOST_DEPTH {
        return Err(Why::TooLarge);
    }
    for name in names_in(folder)? {
        let at = if under.is_empty() { name.clone() } else { format!("{under}/{name}") };
        let inside = format!("{at}/");
        if tree.files.iter().any(|file| file.at == at) {
            return Err(Why::FileThere);
        }
        if tree.folders.iter().any(|(sealed, _)| *sealed == at) || tree.files.iter().any(|file| file.at.starts_with(&inside)) {
            let held = open_at(folder.as_raw_fd(), &name, libc::O_RDONLY | libc::O_DIRECTORY, 0).map_err(|_| Why::FileThere)?;
            stays_in(&held, &at, tree, stays, depth + 1)?;
        } else {
            stays.push(at);
        }
        if stays.len() > MOST_ENTRIES {
            return Err(Why::TooLarge);
        }
    }
    Ok(())
}

/// A protected file's place while its command runs, from before the namespace until after it. A mount needs something to go over, so for
/// that long a file's place holds an empty file and a folder's an empty folder, which is all anything outside can see there.
pub struct Place {
    full: PathBuf,
    path: String,
    what: What,
    style: Style,
    /// The last name of the path
    last: String,
    /// What the tmpfs is over
    covered: PathBuf,
    /// What stayed in the covered folder (`~/.ssh/config`, a file's large neighbours), as paths under it, and shows through it
    stays: Vec<String>,
    /// What was beside a file and is copied in instead, to be put back as the command left it
    copies: Vec<Beside>,
    /// For a file alone, the names beside it that hold its name, to know the ones a command leaves on the disk
    named_before: Vec<String>,
    /// The empty file that holds a single file's place
    placeholder: Option<OwnedFd>,
    /// The folder as it was on the disk, kept open to put back what the command made beside the file
    real: Option<OwnedFd>,
    /// The mode the covered folder had
    folder_mode: u32,
    /// The device of what is mounted, to see that it is still there when the command is over
    device: std::cell::Cell<Option<u64>>,
}

fn device_of(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|stat| std::os::unix::fs::MetadataExt::dev(&stat))
}

impl Place {
    /// Outside the namespace. A plain file with something in it at this path was put there by something else, and nothing here
    /// goes over it.
    pub fn prepare(home_path: &Path, home: &OwnedFd, path: &str, tree: &Tree) -> Result<Place, Why> {
        let (parent, last) = walk(home, path, true)?;
        let full = home_path.join(path);
        let make = |style, covered: PathBuf, stays, placeholder, real, folder_mode| Place {
            full: full.clone(),
            path: path.to_string(),
            what: tree.what,
            style,
            last: last.clone(),
            covered,
            stays,
            copies: Vec::new(),
            named_before: Vec::new(),
            placeholder,
            real,
            folder_mode,
            device: std::cell::Cell::new(None),
        };
        match tree.what {
            What::File => {
                match open_at(parent.as_raw_fd(), &last, libc::O_RDONLY | libc::O_NONBLOCK, 0) {
                    Ok(fd) => {
                        let stat = stat_of(fd.as_raw_fd())?;
                        if !is_kind(&stat, libc::S_IFREG) || stat.st_size != 0 {
                            return Err(Why::FileThere);
                        }
                    }
                    Err(Why::Missing) => {}
                    Err(Why::NotPlain) => return Err(Why::FileThere),
                    Err(why) => return Err(why),
                }
                let beside = if path.contains('/') { names_up_to(&parent, None, MOST_SIBLINGS).ok() } else { None };
                if let Some(mut names) = beside {
                    names.retain(|name| *name != last);
                    let mode = stat_of(parent.as_raw_fd())?.st_mode & 0o7777;
                    let covered = full.parent().ok_or(Why::NotPlain)?.to_path_buf();
                    let (copies, bound) = copy_beside(&parent, names)?;
                    let mut place = make(Style::Parent, covered, bound, None, Some(parent), mode);
                    place.copies = copies;
                    return Ok(place);
                }
                let fd = open_at(parent.as_raw_fd(), &last, libc::O_RDWR | libc::O_CREAT | libc::O_NONBLOCK, 0o600).map_err(|why| if why == Why::NotPlain { Why::FileThere } else { why })?;
                let mut passed = 0;
                let named_before = names_up_to(&parent, Some(&mut passed), usize::MAX)?.into_iter().filter(|name| *name != last && name.contains(last.as_str())).collect();
                let mut place = make(Style::Single, full.clone(), Vec::new(), Some(fd), Some(parent), 0);
                place.named_before = named_before;
                Ok(place)
            }
            What::Folder => {
                let name = c_name(&last)?;
                // SAFETY: a live string; there already is the usual answer
                unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) };
                let held = open_at(parent.as_raw_fd(), &last, libc::O_RDONLY | libc::O_DIRECTORY, 0).map_err(|why| if why == Why::NotPlain { Why::FileThere } else { why })?;
                let mut stays = Vec::new();
                stays_in(&held, "", tree, &mut stays, 0)?;
                Ok(make(Style::Folder, full.clone(), stays, None, None, 0))
            }
        }
    }

    /// Inside the namespace: the content on a tmpfs over the path. It reaches no disk, and goes when the last process in here does.
    pub fn mount(&self, tree: &Tree) -> Result<(), Why> {
        let result = match self.style {
            Style::Single => self.mount_single(tree),
            Style::Parent => self.mount_parent(tree),
            Style::Folder => self.mount_folder(tree),
        };
        self.device.set(device_of(&self.covered));
        result
    }

    fn mount_single(&self, tree: &Tree) -> Result<(), Why> {
        // A tmpfs's root is a folder, and this is a file, so the file is bound from one that is here for a moment, over a
        // folder that neither holds the home nor is in it. Only this namespace sees it covered, for a moment.
        let scratch = ["/tmp", "/var/tmp", "/dev/shm", "/mnt", "/srv", "/opt"]
            .into_iter()
            .map(Path::new)
            .find(|scratch| scratch.is_dir() && !self.full.starts_with(scratch) && !scratch.starts_with(&self.full))
            .ok_or(Why::NoNamespace)?;
        mount_tmpfs(scratch)?;
        let inside = scratch.join("protected");
        let file = tree.files.first().ok_or(Why::Disk)?;
        let written = write_private(&inside, &file.data, file.mode).and_then(|()| bind(inside.to_str().ok_or(Why::Disk)?, &self.full));
        detach(scratch);
        written
    }

    /// What stayed is opened here, in this namespace, before the tmpfs hides it: a mount takes a source only from the namespace it is
    /// made in. A link is made again, never followed; a pipe or a socket is put through like a file, and nothing else is.
    fn open_stays(&self, folder: &Path) -> Result<Vec<(String, OwnedFd, u32)>, Why> {
        let path = c_path(folder)?;
        // SAFETY: a live string; the descriptor is then owned
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_PATH | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC) };
        if fd < 0 {
            return Err(Why::NotPlain);
        }
        // SAFETY: as above
        let folder = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut stays = Vec::new();
        for at in &self.stays {
            // What stayed in a folder the vault holds too is a path under it, walked a name at a time
            let Ok((inner, name)) = folder_of(&folder, at, false) else { continue };
            let Ok(fd) = open_at(inner.as_raw_fd(), &name, libc::O_PATH, 0) else { continue };
            let stat = stat_of(fd.as_raw_fd())?;
            if [libc::S_IFREG, libc::S_IFDIR, libc::S_IFLNK, libc::S_IFIFO, libc::S_IFSOCK].into_iter().any(|kind| is_kind(&stat, kind)) {
                stays.push((at.clone(), fd, stat.st_mode & libc::S_IFMT));
            }
        }
        Ok(stays)
    }

    fn bind_stays(&self, stays: &[(String, OwnedFd, u32)]) -> Result<(), Why> {
        for (name, fd, kind) in stays {
            let on = self.covered.join(name);
            // A name the tree holds is the tree's: a plain copy that came back is what is hidden
            if on.symlink_metadata().is_ok() {
                continue;
            }
            match *kind {
                libc::S_IFDIR => {
                    std::fs::create_dir(&on).map_err(|_| Why::Disk)?;
                    bind(&format!("/proc/self/fd/{}", fd.as_raw_fd()), &on)?;
                }
                libc::S_IFLNK => {
                    if let Some(target) = link_text(fd.as_raw_fd(), "") {
                        let _ = std::os::unix::fs::symlink(std::ffi::OsStr::from_bytes(&target), &on);
                    }
                }
                _ => {
                    File::create(&on).map_err(|_| Why::Disk)?;
                    bind(&format!("/proc/self/fd/{}", fd.as_raw_fd()), &on)?;
                }
            }
        }
        Ok(())
    }

    fn mount_parent(&self, tree: &Tree) -> Result<(), Why> {
        let stays = self.open_stays(&self.covered)?;
        mount_tmpfs(&self.covered)?;
        std::fs::set_permissions(&self.covered, std::fs::Permissions::from_mode(self.folder_mode)).map_err(|_| Why::Disk)?;
        let file = tree.files.first().ok_or(Why::Disk)?;
        write_private(&self.full, &file.data, file.mode)?;
        for copy in &self.copies {
            let on = self.covered.join(&copy.name);
            match &copy.was {
                Was::File { data, mode } => write_private(&on, data, *mode)?,
                Was::Link(target) => std::os::unix::fs::symlink(std::ffi::OsStr::from_bytes(target), &on).map_err(|_| Why::Disk)?,
            }
        }
        self.bind_stays(&stays)
    }

    fn mount_folder(&self, tree: &Tree) -> Result<(), Why> {
        let stays = self.open_stays(&self.full)?;
        mount_tmpfs(&self.full)?;
        if let Some(mode) = tree.root_mode {
            std::fs::set_permissions(&self.full, std::fs::Permissions::from_mode(mode | 0o700)).map_err(|_| Why::Disk)?;
        }
        for (at, mode) in &tree.folders {
            let folder = self.full.join(at);
            std::fs::DirBuilder::new().mode(*mode | 0o700).create(&folder).map_err(|_| Why::Disk)?;
            // The mask would take bits off: the mode is what the folder had
            std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(*mode | 0o700)).map_err(|_| Why::Disk)?;
        }
        for file in &tree.files {
            write_private(&self.full.join(&file.at), &file.data, file.mode)?;
        }
        self.bind_stays(&stays)
    }

    /// Whether what was mounted is still what the path leads to. Something outside that removes or moves what a mount is over takes the
    /// mount from this namespace with it, and then the command wrote to the disk.
    pub fn intact(&self) -> bool {
        self.device.get().is_some() && device_of(&self.covered) == self.device.get()
    }

    /// What the command left, inside the namespace. `None` if it took it away, which is not a reason to lose what is sealed.
    pub fn read_back(&self) -> Result<Option<Tree>, Why> {
        let opened = (|| -> Result<OwnedFd, Why> {
            let path = c_path(&self.full)?;
            // SAFETY: a live string
            let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_NOFOLLOW) };
            if fd < 0 {
                return Err(if errno() == libc::ENOENT { Why::Missing } else { Why::NotPlain });
            }
            // SAFETY: as above
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        })();
        let fd = match opened {
            Ok(fd) => fd,
            Err(Why::Missing) => return Ok(None),
            Err(why) => return Err(why),
        };
        let tree = read_opened(fd, &self.path, &self.stays, true)?;
        Ok(if tree.files.is_empty() && tree.folders.is_empty() { None } else { Some(tree) })
    }

    /// What the command made beside a file, which the tmpfs over the folder would take with it, is put on the disk where it was made, whatever
    /// its size: only what the tmpfs held bounds it. Not what could be the protected file itself, which is never written out: a name that
    /// has the file's in it (an editor's swap and backup files) or content that is the file's old or new content (a temporary copy). Everything
    /// that is not kept is named, with the reason. Nothing is kept of a command that did not end well: a half-written copy of the file may be among it.
    /// What was there and was copied in goes back as the command left it, whichever way it ended, as it would have on the disk.
    pub fn keep_beside(&self, old: &[u8], new: &[u8], failed: bool) -> Left {
        let mut keeping = Keeping { old, new, last: &self.last, left: Left::default() };
        let Some(real) = &self.real else { return keeping.left };
        if self.style == Style::Single {
            self.take_off_named(real, &mut keeping);
            return keeping.left;
        }
        let Ok(path) = c_path(&self.covered) else { return keeping.left.unreadable() };
        // SAFETY: a live string; the descriptor is then owned
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC) };
        if fd < 0 {
            return keeping.left.unreadable();
        }
        // SAFETY: as above
        let inside = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut passed = 0;
        let Ok(names) = names_up_to(&inside, Some(&mut passed), usize::MAX) else { return keeping.left.unreadable() };
        let copied = |name: &String| self.copies.iter().any(|copy| copy.name == *name);
        for name in names.iter().filter(|name| !self.stays.contains(name) && **name != self.last && !copied(name)) {
            if failed {
                // A command that did not end well may have been between writing a temporary copy of the file and moving it into place
                keeping.left.on_purpose.push(name.clone());
            } else {
                keep_entry(&inside, real, name, name.clone(), &mut keeping, 0);
            }
        }
        for copy in &self.copies {
            carry_back(&inside, real, copy, &mut keeping);
        }
        keeping.left.unshowable += passed;
        keeping.left
    }

    /// A file alone has nothing over its folder, so what a command writes beside it is on the disk already (a tool's `.npmrc.tmp`, whose
    /// rename over the file failed). What has the file's name in it and was not there before is taken away, and named: it may be the file's
    /// content, which is never left on the disk.
    fn take_off_named(&self, real: &OwnedFd, keeping: &mut Keeping) {
        let mut passed = 0;
        let Ok(names) = names_up_to(real, Some(&mut passed), usize::MAX) else { return };
        for name in names.iter().filter(|name| **name != self.last && name.contains(self.last.as_str()) && !self.named_before.contains(name)) {
            let plain = open_at(real.as_raw_fd(), name, libc::O_PATH, 0).and_then(|fd| stat_of(fd.as_raw_fd())).is_ok_and(|stat| is_kind(&stat, libc::S_IFREG));
            if plain && unlink(real, name, 0).is_ok() {
                keeping.left.on_purpose.push(name.clone());
            }
        }
    }

    /// Whether something is at the file's path now: with the mount gone, that is on the disk
    pub fn on_disk(&self) -> bool {
        self.full.symlink_metadata().is_ok()
    }

    /// The tmpfs goes, and with it the only other copy, and so does the empty file or folder that held its place. If something else
    /// wrote into that meanwhile, it stays: it is not this command's.
    pub fn leave(self) {
        detach(&self.covered);
        match self.what {
            What::File => {
                let Some(held) = &self.placeholder else { return };
                let Ok(there) = stat_of(held.as_raw_fd()) else { return };
                let Ok(now) = std::fs::symlink_metadata(&self.full) else { return };
                if there.st_size == 0 && std::os::unix::fs::MetadataExt::ino(&now) == there.st_ino && now.is_file() {
                    let _ = std::fs::remove_file(&self.full);
                }
            }
            What::Folder => {
                let Ok(path) = c_path(&self.full) else { return };
                // SAFETY: a live string; a folder something stayed in is the usual answer
                unsafe { libc::rmdir(path.as_ptr()) };
            }
        }
    }
}

/// What `keep_beside` did not keep
#[derive(Default)]
pub struct Left {
    /// Left out because it is, or could be, a copy of the protected file
    pub on_purpose: Vec<String>,
    /// Could not be copied
    pub failed: Vec<String>,
    /// Names that cannot be shown, which were not kept either
    pub unshowable: usize,
    /// Files beside the protected one that something else changed on the disk while the command changed them too: kept under these names
    pub beside: Vec<String>,
}

impl Left {
    fn unreadable(mut self) -> Left {
        self.failed.push(String::new());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.on_purpose.is_empty() && self.failed.is_empty() && self.unshowable == 0 && self.beside.is_empty()
    }
}

/// What a file or link copied in from beside the protected file is when the command is over
enum Now {
    Gone,
    File { file: File, size: u64, mode: u32 },
    Link(Vec<u8>),
    Other,
}

impl Now {
    fn read(folder: &OwnedFd, name: &str) -> Now {
        match open_at(folder.as_raw_fd(), name, libc::O_RDONLY | libc::O_NONBLOCK, 0) {
            Err(Why::Missing) => Now::Gone,
            Err(Why::NotPlain) => link_text(folder.as_raw_fd(), name).map_or(Now::Other, Now::Link),
            Err(_) => Now::Other,
            Ok(fd) => match stat_of(fd.as_raw_fd()) {
                Ok(stat) if is_kind(&stat, libc::S_IFREG) => Now::File { file: File::from(fd), size: stat.st_size as u64, mode: stat.st_mode & 0o777 },
                _ => Now::Other,
            },
        }
    }

    fn is(&mut self, was: &Was) -> bool {
        match (self, was) {
            (Now::Link(target), Was::Link(before)) => target == before,
            (Now::File { file, size, mode }, Was::File { data, mode: before }) if *mode == *before && *size == data.len() as u64 => {
                let mut found = Vec::with_capacity(data.len());
                let same = file.take(*size + 1).read_to_end(&mut found).is_ok() && found == **data;
                let _ = std::io::Seek::rewind(file);
                same
            }
            _ => false,
        }
    }

    /// Written to a name in a folder on the disk, never over one that is there. A file with the protected file's content is not.
    fn put(self, to: &OwnedFd, name: &str, shown: &str, keeping: &mut Keeping) -> bool {
        match self {
            Now::Link(target) => {
                let made = c_name(name).ok().zip(CString::new(target).ok()).is_some_and(|(wanted, text)| {
                    // SAFETY: live strings
                    unsafe { libc::symlinkat(text.as_ptr(), to.as_raw_fd(), wanted.as_ptr()) == 0 }
                });
                if !made {
                    keeping.left.failed.push(shown.to_string());
                }
                made
            }
            Now::File { mut file, size, mode } => {
                if is_a_copy(&mut file, size, keeping) {
                    keeping.left.on_purpose.push(shown.to_string());
                    return false;
                }
                let written = open_at(to.as_raw_fd(), name, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL, 0o600).and_then(|target| {
                    let mut target = File::from(target);
                    std::io::copy(&mut file, &mut target).map_err(|_| Why::Disk)?;
                    target.set_permissions(std::fs::Permissions::from_mode(mode)).map_err(|_| Why::Disk)?;
                    target.sync_all().map_err(|_| Why::Disk)
                });
                if written.is_err() {
                    let _ = unlink(to, name, 0);
                    keeping.left.failed.push(shown.to_string());
                }
                written.is_ok()
            }
            Now::Gone | Now::Other => {
                keeping.left.failed.push(shown.to_string());
                false
            }
        }
    }
}

/// What became of a file or link that was beside the protected file and was copied in, put on the disk: a change by a rename over it, and
/// one that is gone, unlinked. Only over what is on the disk as it was copied: what something else wrote there meanwhile stays, and what the
/// command made of it goes beside it under another name.
fn carry_back(inside: &OwnedFd, real: &OwnedFd, copy: &Beside, keeping: &mut Keeping) {
    let name = copy.name.as_str();
    let mut now = Now::read(inside, name);
    if now.is(&copy.was) {
        return;
    }
    let there = open_at(real.as_raw_fd(), name, libc::O_PATH, 0).ok();
    let as_copied = there.as_ref().and_then(|fd| stat_of(fd.as_raw_fd()).ok()).is_some_and(|stat| stamp_of(&stat) == copy.stamp)
        && match &copy.was {
            Was::Link(target) => there.as_ref().and_then(|fd| link_text(fd.as_raw_fd(), "")).as_ref() == Some(target),
            Was::File { .. } => true,
        };
    if !as_copied {
        if !matches!(now, Now::Gone) {
            let other = format!("{name} (changed by the command)");
            if now.put(real, &other, &other, keeping) {
                keeping.left.beside.push(other);
            }
        }
        return;
    }
    match now {
        Now::Gone => {
            if unlink(real, name, 0).is_err() {
                keeping.left.failed.push(name.to_string());
            }
        }
        now => {
            let temporary = format!(".amethystora-vault-{}", seal::random_hex(8));
            if !now.put(real, &temporary, name, keeping) {
                return;
            }
            let renamed = c_name(&temporary).ok().zip(c_name(name).ok()).is_some_and(|(from, to)| {
                // SAFETY: live strings and a live descriptor
                unsafe { libc::renameat(real.as_raw_fd(), from.as_ptr(), real.as_raw_fd(), to.as_ptr()) == 0 }
            });
            if !renamed {
                let _ = unlink(real, &temporary, 0);
                keeping.left.failed.push(name.to_string());
            }
        }
    }
}

struct Keeping<'a> {
    old: &'a [u8],
    new: &'a [u8],
    last: &'a str,
    left: Left,
}

/// Folders inside folders the command made, as far as anyone makes them
const MOST_KEPT_DEPTH: usize = 64;

/// Whether a file's content is the protected file's old or new content
fn is_a_copy(file: &mut File, size: u64, keeping: &Keeping) -> bool {
    for wanted in [keeping.old, keeping.new] {
        if !wanted.is_empty() && size == wanted.len() as u64 {
            let mut found = Vec::with_capacity(wanted.len());
            if file.take(size + 1).read_to_end(&mut found).is_ok() && found == wanted {
                return true;
            }
            let _ = std::io::Seek::rewind(file);
        }
    }
    false
}

/// One thing the command made beside the file, put on the disk: a file or a folder of them, a link made again. Never followed, never over a name
/// that is there.
fn keep_entry(from: &OwnedFd, to: &OwnedFd, name: &str, shown: String, keeping: &mut Keeping, depth: usize) {
    if name.contains(keeping.last) {
        keeping.left.on_purpose.push(shown);
        return;
    }
    let source = match open_at(from.as_raw_fd(), name, libc::O_RDONLY | libc::O_NONBLOCK, 0) {
        Ok(source) => source,
        Err(Why::NotPlain) => {
            // A link, most likely: made again, with what it said
            let (wanted, mut target) = (c_name(name), vec![0u8; 4096]);
            let kept = wanted.is_ok_and(|wanted| {
                // SAFETY: live strings and a buffer of the length given
                let count = unsafe { libc::readlinkat(from.as_raw_fd(), wanted.as_ptr(), target.as_mut_ptr().cast(), target.len()) };
                if count <= 0 || count as usize >= target.len() {
                    return false;
                }
                target.truncate(count as usize);
                let Ok(text) = CString::new(target) else { return false };
                // SAFETY: live strings
                unsafe { libc::symlinkat(text.as_ptr(), to.as_raw_fd(), wanted.as_ptr()) == 0 }
            });
            if !kept {
                keeping.left.failed.push(shown);
            }
            return;
        }
        Err(_) => {
            keeping.left.failed.push(shown);
            return;
        }
    };
    let Ok(stat) = stat_of(source.as_raw_fd()) else {
        keeping.left.failed.push(shown);
        return;
    };
    if is_kind(&stat, libc::S_IFDIR) {
        let made = c_name(name).is_ok_and(|wanted| {
            // SAFETY: a live string; a name that is already there is left as it is
            unsafe { libc::mkdirat(to.as_raw_fd(), wanted.as_ptr(), stat.st_mode & 0o777 | 0o700) == 0 }
        });
        let Some(target) = made.then(|| open_at(to.as_raw_fd(), name, libc::O_RDONLY | libc::O_DIRECTORY, 0).ok()).flatten().filter(|_| depth < MOST_KEPT_DEPTH) else {
            keeping.left.failed.push(shown);
            return;
        };
        let mut passed = 0;
        match names_up_to(&source, Some(&mut passed), usize::MAX) {
            Ok(inner) => {
                for inner in inner {
                    let inside = format!("{shown}/{inner}");
                    keep_entry(&source, &target, &inner, inside, keeping, depth + 1);
                }
                keeping.left.unshowable += passed;
            }
            Err(_) => keeping.left.failed.push(shown),
        }
        return;
    }
    if !is_kind(&stat, libc::S_IFREG) {
        keeping.left.failed.push(shown);
        return;
    }
    let mut file = File::from(source);
    if is_a_copy(&mut file, stat.st_size as u64, keeping) {
        keeping.left.on_purpose.push(shown);
        return;
    }
    let mode = stat.st_mode & 0o777;
    let copied = open_at(to.as_raw_fd(), name, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL, 0o600).and_then(|target| {
        let mut target = File::from(target);
        let done = std::io::copy(&mut file, &mut target).map_err(|_| Why::Disk).and_then(|_| target.set_permissions(std::fs::Permissions::from_mode(mode)).map_err(|_| Why::Disk));
        if done.is_err() {
            let _ = unlink(to, name, 0);
        }
        done
    });
    if copied.is_err() {
        keeping.left.failed.push(shown);
    }
}

/// Where the usual credentials are, from the watcher's own list (`CREDENTIAL_FILES` in `amethystora-audit-home-rules`, and a test holds
/// the two together). What a browser keeps is left out: a browser that is running holds those open, and no command is started for it.
pub const USUAL: &[&str] = &[
    ".ssh/id_*",
    ".npmrc",
    ".pypirc",
    ".git-credentials",
    ".config/gh/hosts.yml",
    ".aws/credentials",
    ".docker/config.json",
    ".kube/config",
    ".claude/.credentials.json",
    ".codex/auth.json",
    ".codex/.credentials.json",
    ".gemini/oauth_creds.json",
    ".gemini/gemini-credentials.json",
    ".gemini/mcp-oauth-tokens.json",
    ".local/share/opencode/auth.json",
    ".local/share/opencode/mcp-auth.json",
    ".config/cursor/auth.json",
    ".gemini/antigravity-cli/antigravity-oauth-token",
];

/// The watcher's decoy (`DECOY`), a fake it keeps in its place, and how it knows its own
const DECOY_FILE: &str = ".ssh/id_backup";
const DECOY_MARK: &str = "YW1ldGh5c3RvcmEtZGVjb3k6";

/// What is at the usual places now: plain files, with their sizes
pub fn usual_places(home: &Path) -> Vec<(String, u64)> {
    let mut found = Vec::new();
    for pattern in USUAL {
        let (folder, last) = pattern.rsplit_once('/').unwrap_or(("", pattern));
        let inside = home.join(folder);
        let names: Vec<String> = match last.strip_suffix('*') {
            Some(start) => std::fs::read_dir(&inside).map(|names| names.flatten().filter_map(|entry| entry.file_name().to_str().map(str::to_string)).filter(|name| name.starts_with(start)).collect()).unwrap_or_default(),
            None => vec![last.to_string()],
        };
        for name in names {
            let path = if folder.is_empty() { name.clone() } else { format!("{folder}/{name}") };
            if holds_no_secret(&path) || !plain_path(&path) {
                continue;
            }
            let Ok(stat) = std::fs::symlink_metadata(home.join(&path)) else { continue };
            if !stat.is_file() || stat.len() == 0 || (path == DECOY_FILE && is_decoy(&home.join(&path))) {
                continue;
            }
            found.push((path, stat.len()));
        }
    }
    found.sort();
    found
}

fn is_decoy(path: &Path) -> bool {
    let mut head = vec![0u8; 256];
    let count = File::open(path).and_then(|mut file| file.read(&mut head)).unwrap_or(0);
    String::from_utf8_lossy(&head[..count]).lines().nth(1).is_some_and(|line| line.starts_with(DECOY_MARK))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct Home(PathBuf);

    impl Home {
        fn new() -> Home {
            let path = std::env::temp_dir().join(format!("avp-{}", seal::random_hex(6)));
            std::fs::create_dir_all(&path).unwrap();
            Home(path)
        }

        fn put(&self, path: &str, text: &str) {
            let full = self.0.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(&full, text).unwrap();
        }

        fn fd(&self) -> OwnedFd {
            open_home(&self.0).unwrap()
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_file_goes_in_and_comes_back_as_it_was() {
        let home = Home::new();
        home.put(".aws/credentials", "[default]\nsecret\n");
        std::fs::set_permissions(home.0.join(".aws/credentials"), std::fs::Permissions::from_mode(0o640)).unwrap();
        let tree = read(&home.fd(), ".aws/credentials").unwrap();
        let again = Tree::from_value(&tree.to_value().unwrap()).unwrap();
        assert!(tree.same_as(&again));
        assert_eq!(tree.digest(), again.digest());
        remove_original(&home.fd(), ".aws/credentials", &tree).unwrap();
        assert!(!home.0.join(".aws/credentials").exists());
        restore(&home.fd(), ".aws/credentials", &again).unwrap();
        assert_eq!(std::fs::read_to_string(home.0.join(".aws/credentials")).unwrap(), "[default]\nsecret\n");
        assert_eq!(std::fs::metadata(home.0.join(".aws/credentials")).unwrap().permissions().mode() & 0o777, 0o640);
    }

    #[test]
    fn a_folder_leaves_what_holds_no_secret() {
        let home = Home::new();
        home.put(".ssh/config", "Host x\n");
        home.put(".ssh/known_hosts", "x\n");
        home.put(".ssh/id_rsa", "private\n");
        home.put(".ssh/id_rsa.pub", "public\n");
        home.put(".ssh/more/deep", "deep\n");
        let tree = read(&home.fd(), ".ssh").unwrap();
        let names: Vec<&str> = tree.files.iter().map(|file| file.at.as_str()).collect();
        assert_eq!(names, ["id_rsa", "more/deep"]);
        remove_original(&home.fd(), ".ssh", &tree).unwrap();
        assert!(home.0.join(".ssh/config").exists() && home.0.join(".ssh/id_rsa.pub").exists());
        assert!(!home.0.join(".ssh/id_rsa").exists() && !home.0.join(".ssh/more").exists());
        restore(&home.fd(), ".ssh", &tree).unwrap();
        assert_eq!(std::fs::read_to_string(home.0.join(".ssh/more/deep")).unwrap(), "deep\n");
    }

    #[test]
    fn what_holds_no_secret_is_not_taken_when_named() {
        let home = Home::new();
        for path in [".ssh/config", ".ssh/known_hosts", ".ssh/id_rsa.pub"] {
            home.put(path, "x");
            assert_eq!(read(&home.fd(), path).err(), Some(Why::NoSecret), "{path}");
        }
    }

    #[test]
    fn no_link_is_ever_followed() {
        let home = Home::new();
        home.put("real/secret", "secret");
        symlink(home.0.join("real"), home.0.join("through")).unwrap();
        symlink(home.0.join("real/secret"), home.0.join("link")).unwrap();
        assert_eq!(read(&home.fd(), "through/secret").err(), Some(Why::NotPlain));
        assert_eq!(read(&home.fd(), "link").err(), Some(Why::NotPlain));
        symlink("/etc/passwd", home.0.join("real/out")).unwrap();
        assert_eq!(read(&home.fd(), "real").err(), Some(Why::NotPlain));
    }

    #[test]
    fn a_path_that_leaves_is_refused_in_every_shape() {
        let home = Home::new();
        home.put("a/b", "x");
        for path in ["", ".", "..", "../x", "a/../..", "a//b", "/etc/passwd", "a/./b", "a/\nb", "a/"] {
            assert_eq!(read(&home.fd(), path).err(), Some(Why::NotPlain), "{path:?}");
            assert!(!plain_path(path), "{path:?}");
        }
        assert_eq!(under_home(&home.0, Path::new("/etc/passwd")), Err(Why::NotPlain));
        assert_eq!(under_home(&home.0, &home.0.join("a/../../x")), Err(Why::NotPlain));
        assert_eq!(under_home(&home.0, &home.0).err(), Some(Why::NotPlain));
        assert_eq!(under_home(&home.0, &home.0.join("a/./b")).unwrap(), "a/b");
    }

    #[test]
    fn what_is_too_large_or_not_plain_is_refused_and_nothing_is_read_past_the_limit() {
        let home = Home::new();
        let file = File::create(home.0.join("large")).unwrap();
        file.set_len(4 * 1024 * 1024 * 1024).unwrap();
        assert_eq!(read(&home.fd(), "large").err(), Some(Why::TooLarge));
        for index in 0..MOST_ENTRIES + 1 {
            home.put(&format!("many/{index}"), "x");
        }
        assert_eq!(read(&home.fd(), "many").err(), Some(Why::TooLarge));
        let fifo = CString::new(home.0.join("pipe").as_os_str().as_bytes()).unwrap();
        // SAFETY: a live string
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        // Would hang a plain open
        assert_eq!(read(&home.fd(), "pipe").err(), Some(Why::NotPlain));
    }

    #[test]
    fn a_file_changed_after_it_was_read_is_not_removed() {
        let home = Home::new();
        home.put("key", "one");
        let tree = read(&home.fd(), "key").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        home.put("key", "two!");
        assert_eq!(remove_original(&home.fd(), "key", &tree), Err(Why::Changed));
        assert_eq!(std::fs::read_to_string(home.0.join("key")).unwrap(), "two!");
    }

    #[test]
    fn putting_back_never_goes_over_what_is_there() {
        let home = Home::new();
        home.put("key", "one");
        let tree = read(&home.fd(), "key").unwrap();
        home.put("key", "somebody else's");
        assert_eq!(restore(&home.fd(), "key", &tree), Err(Why::FileThere));
        assert_eq!(std::fs::read_to_string(home.0.join("key")).unwrap(), "somebody else's");
        symlink("/etc/hostname", home.0.join("link")).unwrap();
        assert_eq!(restore(&home.fd(), "link", &tree), Err(Why::FileThere));
    }

    #[test]
    fn an_item_is_only_as_true_as_what_it_says_and_is_refused_otherwise() {
        for value in [
            json!(null),
            json!({"what": "file"}),
            json!({"what": "disk", "files": [], "folders": []}),
            json!({"what": "file", "files": [], "folders": []}),
            json!({"what": "folder", "files": [{"at": "../x", "mode": 384, "data": ""}], "folders": []}),
            json!({"what": "folder", "files": [{"at": "/etc/x", "mode": 384, "data": ""}], "folders": []}),
            json!({"what": "folder", "files": [{"at": "x", "mode": 2496, "data": ""}], "folders": []}),
            json!({"what": "folder", "files": [{"at": "x", "mode": 384, "data": "*"}], "folders": []}),
            json!({"what": "folder", "files": [], "folders": [{"at": "a/../..", "mode": 448}]}),
        ] {
            assert!(Tree::from_value(&value).is_err(), "{value}");
        }
    }

    #[test]
    fn the_list_of_usual_places_is_the_watchers_less_the_browsers() {
        let Ok(root) = std::env::var("AMETHYSTORA_ROOT") else {
            eprintln!("AMETHYSTORA_ROOT is not set: the watcher's list was not compared");
            return;
        };
        let script = std::fs::read_to_string(Path::new(&root).join("usr/libexec/amethystora-audit-home-rules")).unwrap();
        let start = script.find("CREDENTIAL_FILES=(").unwrap() + "CREDENTIAL_FILES=(".len();
        let list = &script[start..start + script[start..].find(')').unwrap()];
        let theirs: Vec<&str> = list.split_whitespace().filter(|entry| !entry.contains(".mozilla") && !entry.starts_with(".var/app") && !entry.contains("google-chrome")).collect();
        assert_eq!(theirs.len(), USUAL.len(), "{theirs:?}");
        for entry in theirs {
            assert!(USUAL.contains(&entry), "{entry} is in the watcher's list and not here");
        }
    }

    #[test]
    fn the_usual_places_find_what_is_there_and_leave_the_rest() {
        let home = Home::new();
        home.put(".ssh/id_ed25519", "x");
        home.put(".ssh/id_ed25519.pub", "x");
        home.put(".ssh/config", "x");
        home.put(".ssh/id_backup", &format!("-----BEGIN OPENSSH PRIVATE KEY-----\n{DECOY_MARK}xxxx\n-----END OPENSSH PRIVATE KEY-----\n"));
        home.put(".aws/credentials", "abc");
        symlink("/etc/passwd", home.0.join(".npmrc")).unwrap();
        let found = usual_places(&home.0);
        assert_eq!(found, [(".aws/credentials".to_string(), 3), (".ssh/id_ed25519".to_string(), 1)]);
    }
}
