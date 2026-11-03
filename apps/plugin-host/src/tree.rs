//! A plugin's files as they travel and as they are kept: a bounded tar of regular files and folders (read here, and
//! written by `pack`), the digest of the whole tree, and the way the root helper writes it out. Nothing else is ever
//! accepted: no link, device, FIFO, setuid bit, absolute path or `..`, and the owner and modes are the helper's own.

use std::collections::BTreeMap;
use std::ffi::CString;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

pub const MAX_BYTES: u64 = 8 << 20;
pub const MAX_ENTRIES: usize = 512;
const BLOCK: usize = 512;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: String,
    pub dir: bool,
    /// Any execute bit on a file; the helper writes 0755 or 0644 and nothing else
    pub exec: bool,
    pub data: Vec<u8>,
}

/// One path of an archive, or the reason it is refused
pub fn clean_path(raw: &str) -> Result<String, String> {
    let path = raw.strip_prefix("./").unwrap_or(raw).trim_end_matches('/');
    if path.is_empty() || raw.starts_with('/') || path.len() > 240 {
        return Err(format!("path {raw:?} is empty, absolute or too long"));
    }
    for part in path.split('/') {
        if part.chars().any(crate::manifest::is_bidi) || part.is_empty() || part == "." || part == ".." || part.len() > 100 || part.chars().any(|c| c.is_control() || c == '\\') {
            return Err(format!("path {raw:?} leaves the folder or has a name no plugin may have"));
        }
    }
    Ok(path.to_string())
}

fn field(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())]
}

fn octal(bytes: &[u8]) -> Result<u64, String> {
    let text = std::str::from_utf8(field(bytes)).map_err(|_| "a number in the archive is not text")?.trim();
    if text.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(text, 8).map_err(|_| "a number in the archive is not octal".to_string())
}

/// Reads an archive of at most MAX_BYTES of content and MAX_ENTRIES entries, refusing everything but files and folders
pub fn read_tar(mut input: impl Read) -> Result<Vec<Entry>, String> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut seen: BTreeMap<String, bool> = BTreeMap::new();
    let mut total = 0u64;
    let mut header = [0u8; BLOCK];
    loop {
        match input.read_exact(&mut header) {
            Ok(()) => {}
            Err(_) => return Err("the archive ends in the middle of a header".into()),
        }
        if header.iter().all(|b| *b == 0) {
            break;
        }
        let sum: u64 = header.iter().enumerate().map(|(i, b)| if (148..156).contains(&i) { 32 } else { *b as u64 }).sum();
        if octal(&header[148..156])? != sum {
            return Err("the archive's header does not add up: it is not a tar file".into());
        }
        let size = octal(&header[124..136])?;
        let mode = octal(&header[100..108])? as u32;
        let kind = header[156];
        let mut name = String::from_utf8_lossy(field(&header[0..100])).into_owned();
        if &header[257..262] == b"ustar" && header[345] != 0 {
            name = format!("{}/{name}", String::from_utf8_lossy(field(&header[345..500])));
        }
        let padded = size.div_ceil(BLOCK as u64) * BLOCK as u64;
        match kind {
            // A pax global header names nothing of ours; whatever it says is ignored, and its data is read past
            b'g' => {
                skip(&mut input, padded)?;
                continue;
            }
            b'0' | 0 => {}
            b'5' => {}
            b'1' | b'2' => return Err(format!("{name:?} is a link, which a plugin never holds")),
            b'3' | b'4' | b'6' => return Err(format!("{name:?} is a device or a FIFO, which a plugin never holds")),
            _ => return Err(format!("{name:?} is of a kind the archive may not hold")),
        }
        let dir = kind == b'5';
        // `tar -c .` starts with the folder itself
        if dir && matches!(name.as_str(), "./" | ".") {
            continue;
        }
        let path = clean_path(&name)?;
        if mode & 0o7000 != 0 {
            return Err(format!("{path:?} has a setuid, setgid or sticky bit"));
        }
        if entries.len() >= MAX_ENTRIES {
            return Err(format!("the archive holds more than {MAX_ENTRIES} entries"));
        }
        total += size;
        if total > MAX_BYTES {
            return Err("the archive holds more than 8 MiB".into());
        }
        let mut data = vec![0u8; size as usize];
        if !dir {
            input.read_exact(&mut data).map_err(|_| "the archive ends in the middle of a file")?;
            skip(&mut input, padded - size)?;
        } else if size != 0 {
            return Err(format!("{path:?} is a folder with content"));
        }
        // The version control's own folder is not a plugin's: `read_dir` leaves it out, and so does this, so that the digests agree
        if path == ".git" || path.starts_with(".git/") {
            continue;
        }
        // A name used twice, or as both a file and a folder, or below a file
        let mut prefix = String::new();
        for part in path.split('/').take(path.matches('/').count()) {
            prefix = if prefix.is_empty() { part.to_string() } else { format!("{prefix}/{part}") };
            if seen.get(&prefix) == Some(&false) {
                return Err(format!("{path:?} is below the file {prefix:?}"));
            }
        }
        if seen.insert(path.clone(), dir).is_some() {
            return Err(format!("{path:?} is in the archive twice"));
        }
        entries.push(Entry { path, dir, exec: !dir && mode & 0o111 != 0, data });
    }
    Ok(entries)
}

fn skip(input: &mut impl Read, count: u64) -> Result<(), String> {
    let copied = std::io::copy(&mut input.take(count), &mut std::io::sink()).map_err(|e| e.to_string())?;
    if copied == count { Ok(()) } else { Err("the archive ends in the middle of a file".into()) }
}

fn put_octal(slot: &mut [u8], value: u64) {
    let text = format!("{value:0width$o}", width = slot.len() - 1);
    slot[..text.len()].copy_from_slice(text.as_bytes());
}

/// An archive of a folder, as `pack` writes it for `prepare`: files and folders only, names of up to 100 bytes
pub fn write_tar(entries: &[Entry]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for e in entries {
        if e.path.len() > 99 {
            return Err(format!("{:?} has a name longer than 99 bytes", e.path));
        }
        let mut h = [0u8; BLOCK];
        h[..e.path.len()].copy_from_slice(e.path.as_bytes());
        put_octal(&mut h[100..108], if e.dir || e.exec { 0o755 } else { 0o644 });
        put_octal(&mut h[108..116], 0);
        put_octal(&mut h[116..124], 0);
        put_octal(&mut h[124..136], e.data.len() as u64);
        put_octal(&mut h[136..148], 0);
        h[148..156].fill(32);
        h[156] = if e.dir { b'5' } else { b'0' };
        h[257..263].copy_from_slice(b"ustar\0");
        h[263..265].copy_from_slice(b"00");
        let sum: u64 = h.iter().map(|b| *b as u64).sum();
        let text = format!("{sum:06o}\0 ");
        h[148..156].copy_from_slice(text.as_bytes());
        out.extend_from_slice(&h);
        out.extend_from_slice(&e.data);
        out.resize(out.len().div_ceil(BLOCK) * BLOCK, 0);
    }
    out.resize(out.len() + 2 * BLOCK, 0);
    Ok(out)
}

/// A folder as entries, refusing a link, a device, a FIFO, a setuid bit or more than the limits allow
pub fn read_dir(root: &Path) -> Result<Vec<Entry>, String> {
    fn walk(root: &Path, relative: &str, out: &mut Vec<Entry>, total: &mut u64) -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        let mut names: Vec<_> = std::fs::read_dir(root.join(relative)).map_err(|e| e.to_string())?.filter_map(Result::ok).map(|e| e.file_name()).collect();
        names.sort();
        for name in names {
            let name = name.to_string_lossy().into_owned();
            let path = if relative.is_empty() { name } else { format!("{relative}/{name}") };
            clean_path(&path)?;
            let meta = std::fs::symlink_metadata(root.join(&path)).map_err(|e| e.to_string())?;
            let mode = meta.permissions().mode();
            if mode & 0o7000 != 0 {
                return Err(format!("{path:?} has a setuid, setgid or sticky bit"));
            }
            if out.len() >= MAX_ENTRIES {
                return Err(format!("more than {MAX_ENTRIES} files"));
            }
            if meta.is_dir() {
                out.push(Entry { path: path.clone(), dir: true, exec: false, data: Vec::new() });
                walk(root, &path, out, total)?;
            } else if meta.is_file() {
                *total += meta.len();
                if *total > MAX_BYTES {
                    return Err("more than 8 MiB".into());
                }
                let data = std::fs::read(root.join(&path)).map_err(|e| e.to_string())?;
                out.push(Entry { path, dir: false, exec: mode & 0o111 != 0, data });
            } else {
                return Err(format!("{path:?} is a link, a device or a FIFO, which a plugin never holds"));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, "", &mut out, &mut 0)?;
    // The version control's own folder is not a plugin's, and would only carry history along
    out.retain(|e| e.path != ".git" && !e.path.starts_with(".git/"));
    Ok(out)
}

/// The digest of a tree: each path in order with its kind, mode and content's SHA-256. It names the folder it is kept in
pub fn digest(entries: &[Entry]) -> String {
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut lines = Sha256::new();
    for e in sorted {
        if e.dir {
            lines.update(format!("D 755 {}\n", e.path));
        } else {
            lines.update(format!("F {} {} {}\n", if e.exec { "755" } else { "644" }, hex(&Sha256::digest(&e.data)), e.path));
        }
    }
    hex(&lines.finalize())
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn valid_digest(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Writes entries below `base` with the helper's own modes, each file made new (`O_EXCL`) and never through a link
/// (`O_NOFOLLOW`), one component at a time from a descriptor
pub fn write_tree(base: &Path, entries: &[Entry]) -> Result<(), String> {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::fs::PermissionsExt;
    let open_dir = |parent: Option<&OwnedFd>, name: &str| -> Result<OwnedFd, String> {
        let name = CString::new(name).map_err(|_| "a name holds a NUL")?;
        let at = parent.map_or(libc::AT_FDCWD, |p| p.as_raw_fd());
        // SAFETY: a valid C string, and a descriptor owned by the caller for the call
        let fd = unsafe { libc::openat(at, name.as_ptr(), libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_RDONLY) };
        if fd < 0 { Err(std::io::Error::last_os_error().to_string()) } else { Ok(unsafe { OwnedFd::from_raw_fd(fd) }) }
    };
    let root = open_dir(None, &base.to_string_lossy())?;
    for e in entries {
        let parts: Vec<&str> = e.path.split('/').collect();
        let mut dir = root.try_clone().map_err(|e| e.to_string())?;
        for part in &parts[..parts.len() - 1] {
            dir = open_dir(Some(&dir), part)?;
        }
        let leaf = CString::new(parts[parts.len() - 1]).map_err(|_| "a name holds a NUL")?;
        if e.dir {
            // SAFETY: as above
            if unsafe { libc::mkdirat(dir.as_raw_fd(), leaf.as_ptr(), 0o755) } != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            continue;
        }
        let mode = if e.exec { 0o755 } else { 0o644 };
        // SAFETY: as above
        let fd = unsafe {
            libc::openat(dir.as_raw_fd(), leaf.as_ptr(), libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC, mode as libc::c_uint)
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // SAFETY: the descriptor was just opened and nothing else owns it
        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        std::io::Write::write_all(&mut file, &e.data).map_err(|e| e.to_string())?;
        // The mode is set again past the umask
        file.set_permissions(std::fs::Permissions::from_mode(mode)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, data: &str, exec: bool) -> Entry {
        Entry { path: path.into(), dir: false, exec, data: data.as_bytes().to_vec() }
    }

    fn header(name: &str, kind: u8, mode: u64, size: u64) -> Vec<u8> {
        let mut raw = write_tar(&[file("x", "", false)]).unwrap();
        raw.truncate(BLOCK);
        raw[..100].fill(0);
        raw[..name.len()].copy_from_slice(name.as_bytes());
        put_octal(&mut raw[100..108], mode);
        put_octal(&mut raw[124..136], size);
        raw[156] = kind;
        raw[148..156].fill(32);
        let sum: u64 = raw.iter().map(|b| *b as u64).sum();
        raw[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
        raw
    }

    fn archive(headers: Vec<Vec<u8>>) -> Vec<u8> {
        let mut out: Vec<u8> = headers.concat();
        out.resize(out.len() + 2 * BLOCK, 0);
        out
    }

    #[test]
    fn round_trips_and_digests_the_same() {
        let entries = vec![Entry { path: "bin".into(), dir: true, exec: false, data: vec![] }, file("bin/run.sh", "echo hi\n", true), file("manifest.json", "{}", false)];
        let back = read_tar(&write_tar(&entries).unwrap()[..]).unwrap();
        assert_eq!(back, entries);
        assert_eq!(digest(&back), digest(&entries));
        let mut changed = entries.clone();
        changed[1].data[0] = b'E';
        assert_ne!(digest(&changed), digest(&entries));
        let mut other_mode = entries.clone();
        other_mode[1].exec = false;
        assert_ne!(digest(&other_mode), digest(&entries));
        assert!(valid_digest(&digest(&entries)));
    }

    #[test]
    fn refuses_what_a_plugin_never_holds() {
        for (what, kind, name, mode) in [
            ("a symlink", b'2', "a", 0o644),
            ("a hard link", b'1', "a", 0o644),
            ("a char device", b'3', "a", 0o644),
            ("a block device", b'4', "a", 0o644),
            ("a fifo", b'6', "a", 0o644),
            ("dot dot", b'0', "../a", 0o644),
            ("dot dot inside", b'0', "a/../../b", 0o644),
            ("absolute", b'0', "/etc/passwd", 0o644),
            ("setuid", b'0', "a", 0o4755),
            ("setgid", b'0', "a", 0o2755),
            ("sticky", b'0', "a", 0o1755),
            ("a backslash", b'0', "a\\b", 0o644),
            ("a control character", b'0', "a\nb", 0o644),
            ("an extended header", b'x', "a", 0o644),
        ] {
            let got = read_tar(&archive(vec![header(name, kind, mode, 0)])[..]);
            assert!(got.is_err(), "{what}");
        }
        // Twice, a file below a file, and a folder that holds content
        assert!(read_tar(&archive(vec![header("a", b'0', 0o644, 0), header("a", b'0', 0o644, 0)])[..]).is_err());
        assert!(read_tar(&archive(vec![header("a", b'0', 0o644, 0), header("a/b", b'0', 0o644, 0)])[..]).is_err());
        assert!(read_tar(&archive(vec![header("a", b'5', 0o755, 512)])[..]).is_err());
        // Not a tar file, a cut one, one that is too large, and one with too many entries
        assert!(read_tar(&b"hello"[..]).is_err());
        assert!(read_tar(&vec![7u8; 512][..]).is_err());
        assert!(read_tar(&archive(vec![header("a", b'0', 0o644, MAX_BYTES + 1)])[..]).is_err());
        let many: Vec<Entry> = (0..=MAX_ENTRIES).map(|i| file(&format!("f{i}"), "", false)).collect();
        assert!(read_tar(&write_tar(&many).unwrap()[..]).is_err());
    }

    #[test]
    fn a_leading_dot_slash_is_taken_off_and_a_pax_global_header_passed() {
        let mut raw = header("pax", b'g', 0o644, 3);
        raw.extend_from_slice(&[b'x'; 3]);
        raw.resize(2 * BLOCK, 0);
        raw.extend(header("./a.txt", b'0', 0o644, 0));
        let got = read_tar(&archive(vec![raw])[..]).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].path, "a.txt");
    }

    #[test]
    fn writes_a_tree_with_its_own_modes() {
        let dir = std::env::temp_dir().join(format!("plugin-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let entries = vec![Entry { path: "bin".into(), dir: true, exec: false, data: vec![] }, file("bin/run.sh", "x", true), file("manifest.json", "{}", false)];
        write_tree(&dir, &entries).unwrap();
        assert_eq!(read_dir(&dir).unwrap(), entries);
        // Made new: the same tree again is refused rather than written over
        assert!(write_tree(&dir, &entries).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_folder_with_a_link_is_refused() {
        let dir = std::env::temp_dir().join(format!("plugin-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", dir.join("a")).unwrap();
        assert!(read_dir(&dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
