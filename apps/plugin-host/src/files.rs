//! `files` permissions as bind mounts: a place under the home folder, checked again as it really is when the plugin
//! starts. The manifest's shape (manifest.rs) is the first check; this is the second, after links are followed, since a
//! link in the path (or one made later) must not lead somewhere the first check would have refused.

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

#[cfg(test)]
mod tests {
    use super::*;

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
