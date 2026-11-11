//! `files` permissions as bind mounts: a place under the home folder, checked again as it really is when the plugin
//! starts. The manifest's shape (manifest.rs) is the first check; this is the second, after links are followed, since a
//! link in the path (or one made later) must not lead somewhere the first check would have refused.

use std::ffi::CString;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// `~/Documents/ci` and a mode: the real path to mount, or why not. Refused: the home itself, anything with a dot-folder in
/// it, `~/Private`, and a real path outside the home (through a link), a socket, a FIFO or a device
pub fn resolve(home: &Path, perm_path: &str, writable: bool) -> Result<PathBuf, String> {
    let rest = perm_path.strip_prefix("~/").ok_or("a path starts with ~/")?;
    let home = home.canonicalize().map_err(|e| format!("the home folder: {e}"))?;
    let real = home.join(rest).canonicalize().map_err(|_| format!("{perm_path} is not there"))?;
    let relative = real.strip_prefix(&home).map_err(|_| format!("{perm_path} leads out of the home folder"))?;
    let parts: Vec<_> = relative.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if parts.is_empty() {
        return Err("that is the home folder itself".into());
    }
    if parts.iter().any(|p| p.starts_with('.')) {
        return Err(format!("{perm_path} is, or leads into, a hidden folder"));
    }
    if parts[0] == "Private" {
        return Err("the private folder is never given to a plugin".into());
    }
    if writable && crate::manifest::starts_programs(&parts[0]) {
        return Err("a folder that starts programs is never given read-write".into());
    }
    let meta = std::fs::metadata(&real).map_err(|e| e.to_string())?;
    if !(meta.is_dir() || meta.is_file()) {
        return Err(format!("{perm_path} is not a plain file or folder"));
    }
    Ok(real)
}

/// A place of the home as it is now: opened once, link by link, so that bubblewrap binds this very file or folder (`--bind-fd`) and not
/// whatever a name leads to by the time it starts. `path` is where the plugin sees it
pub struct Opened {
    pub path: PathBuf,
    pub fd: OwnedFd,
}

fn open_at(parent: RawFd, name: &std::ffi::OsStr, directory: bool) -> Result<OwnedFd, String> {
    let c = CString::new(name.as_bytes()).map_err(|_| "a name with a NUL")?;
    // O_PATH|O_NOFOLLOW on a link opens the link itself, which fstat below refuses
    let flags = libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC | if directory { libc::O_DIRECTORY } else { 0 };
    // SAFETY: c is a NUL-terminated name and parent an open descriptor; the result is owned here
    let fd = unsafe { libc::openat(parent, c.as_ptr(), flags) };
    if fd < 0 {
        return Err(format!("{}: {}", name.to_string_lossy(), std::io::Error::last_os_error()));
    }
    // SAFETY: openat returned a new descriptor
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn kind(fd: RawFd) -> Result<u32, String> {
    // SAFETY: stat is plain data and fd is open
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut st) } < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(st.st_mode & libc::S_IFMT)
}

/// `resolve`, then the real path opened from the home with every component `O_NOFOLLOW`: a link put in the path after the check is
/// refused, and what is returned is the descriptor of what was checked
pub fn open(home: &Path, perm_path: &str, writable: bool) -> Result<Opened, String> {
    let real = resolve(home, perm_path, writable)?;
    let home = home.canonicalize().map_err(|e| format!("the home folder: {e}"))?;
    let relative = real.strip_prefix(&home).map_err(|_| "it leads out of the home folder".to_string())?;
    let names: Vec<_> = relative.components().map(|c| c.as_os_str().to_owned()).collect();
    let mut fd = open_at(libc::AT_FDCWD, home.as_os_str(), true)?;
    for (i, name) in names.iter().enumerate() {
        let last = i + 1 == names.len();
        let next = open_at(fd.as_raw_fd(), name, !last)?;
        let k = kind(next.as_raw_fd())?;
        if k == libc::S_IFLNK || (!last && k != libc::S_IFDIR) || (last && k != libc::S_IFDIR && k != libc::S_IFREG) {
            return Err(format!("{perm_path} changed while it was being opened"));
        }
        fd = next;
    }
    Ok(Opened { path: real, fd })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_refuses_a_link_swapped_in_after_the_check_and_holds_what_was_checked() {
        let home = std::env::temp_dir().join(format!("plugin-open-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("Documents/notes")).unwrap();
        std::fs::create_dir_all(home.join(".ssh")).unwrap();
        std::fs::write(home.join("Documents/notes/a.txt"), b"x").unwrap();
        let opened = open(&home, "~/Documents/notes", false).unwrap();
        // The descriptor is the folder that was checked, whatever its name leads to afterwards
        let held = format!("/proc/self/fd/{}", opened.fd.as_raw_fd());
        std::fs::rename(home.join("Documents/notes"), home.join("Documents/old")).unwrap();
        std::os::unix::fs::symlink("../../.ssh", home.join("Documents/notes")).unwrap();
        assert!(std::fs::read_dir(&held).unwrap().any(|e| e.unwrap().file_name() == "a.txt"));
        // And a new open of the swapped name is refused (resolve sees it leave the permitted places)
        assert!(open(&home, "~/Documents/notes", false).is_err());
        // A link inside the walk is refused too: the descriptor walk alone, as a link that appears after `resolve` returned
        let ssh_link = home.join("Documents/old");
        std::fs::remove_dir_all(&ssh_link).unwrap();
        std::os::unix::fs::symlink("a-real-name", &ssh_link).unwrap();
        let documents = open_at(libc::AT_FDCWD, home.join("Documents").as_os_str(), true).unwrap();
        let link = open_at(documents.as_raw_fd(), std::ffi::OsStr::new("old"), false).unwrap();
        assert_eq!(kind(link.as_raw_fd()).unwrap(), libc::S_IFLNK);
        assert!(open_at(documents.as_raw_fd(), std::ffi::OsStr::new("old"), true).is_err());
        // A file opens as a file
        std::fs::remove_file(&ssh_link).unwrap();
        std::fs::create_dir_all(home.join("Documents/ci")).unwrap();
        std::fs::write(home.join("Documents/ci/b.txt"), b"y").unwrap();
        assert_eq!(kind(open(&home, "~/Documents/ci/b.txt", true).unwrap().fd.as_raw_fd()).unwrap(), libc::S_IFREG);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn refuses_links_out_hidden_places_and_the_private_folder() {
        let home = std::env::temp_dir().join(format!("plugin-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        for dir in ["Documents/ci", ".ssh", "Private"] {
            std::fs::create_dir_all(home.join(dir)).unwrap();
        }
        std::fs::write(home.join("Documents/ci/a.txt"), b"x").unwrap();
        std::os::unix::fs::symlink("/etc", home.join("Documents/etc")).unwrap();
        std::os::unix::fs::symlink(home.join(".ssh"), home.join("Documents/keys")).unwrap();
        std::os::unix::fs::symlink(home.join("Private"), home.join("Documents/p")).unwrap();
        std::fs::create_dir_all(home.join("bin")).unwrap();
        assert!(resolve(&home, "~/Documents/ci", false).is_ok() && resolve(&home, "~/Documents/ci/a.txt", true).is_ok());
        assert!(resolve(&home, "~/bin", false).is_ok() && resolve(&home, "~/bin", true).is_err());
        for bad in ["~/Documents/etc", "~/Documents/keys", "~/Documents/p", "~/.ssh", "~/Private", "~/Documents/missing", "/etc", "~/Documents/../.ssh"] {
            assert!(resolve(&home, bad, false).is_err(), "{bad}");
        }
        std::fs::remove_dir_all(&home).unwrap();
    }
}
