//! What a key, a certificate and a protected file become when the person chooses Plain (plans/PLAN-vault.md's table), and back: a
//! secure note Bitwarden's apps open, or for an SSH key an SSH key item. The private part is in a hidden field (an SSH item's own
//! three strings), and a file, or text that does not fit its field, is the note's attachment (spec 7, spec-attachments 4.4 and 8).
//!
//! Every text that goes is written the same way each time, so that a note is not rewritten for nothing; every text that comes back
//! is read for what it says and not for how it is wrapped, since an app's single-line input may have taken the line breaks out.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use super::cipher::{self, Content, Field, Ssh, FIELD_EXPIRES, FIELD_KIND, FIELD_PRIVATE, MARKER_CERT, MARKER_FILE, MARKER_KEY, NOTE, SSH};
use super::convert::Convert;
use super::map::{Local, Unmapped};

/// The most plaintext a field's value may hold, and a note's (spec 7)
const MOST_FIELD: usize = 3695;
const MOST_NOTES: usize = 7439;
/// A protected folder holds at most this many files and folders (`protect.rs`'s own bound)
const MOST_ENTRIES: usize = 256;
const MOST_TAR: usize = crate::protocol::MAX_DATA + 64 * 1024;

pub const KIND_KEY: &str = crate::protocol::KIND_KEY;
pub const KIND_CERTIFICATE: &str = crate::protocol::KIND_CERTIFICATE;
pub const KIND_PROTECTED: &str = "protected";

/// A file to be the note's attachment
pub struct File {
    pub name: String,
    pub bytes: Zeroizing<Vec<u8>>,
}

/// What an item goes as: the note or SSH item, and the file with it where there is one
pub struct Made {
    pub content: Content,
    pub file: Option<File>,
}

/// Whether a kind goes as one of these when it is Plain
pub fn is_plain_kind(kind: &str) -> bool {
    matches!(kind, KIND_KEY | KIND_CERTIFICATE | KIND_PROTECTED)
}

/// Which marker a plain kind's note carries
pub fn marker_of(kind: &str) -> &'static str {
    match kind {
        KIND_KEY => MARKER_KEY,
        KIND_CERTIFICATE => MARKER_CERT,
        _ => MARKER_FILE,
    }
}

// --- Text ------------------------------------------------------------------------------------------------------------------

/// Every PEM block of a text: its label and the bytes of its base64, however it was wrapped
pub fn pem_blocks(text: &str) -> Vec<(String, Vec<u8>)> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(begin) = rest.find("-----BEGIN ") {
        let after = &rest[begin + 11..];
        let Some(label_end) = after.find("-----") else { break };
        let label = &after[..label_end];
        let body_start = &after[label_end + 5..];
        let end_marker = format!("-----END {label}-----");
        let Some(end) = body_start.find(&end_marker) else { break };
        let body: String = body_start[..end].chars().filter(|c| !c.is_whitespace()).collect();
        if label.len() <= 40 && label.bytes().all(|b| b.is_ascii_uppercase() || b == b' ')
            && let Ok(bytes) = STANDARD.decode(body.as_bytes())
        {
            found.push((label.to_string(), bytes));
        }
        rest = &body_start[end + end_marker.len()..];
    }
    found
}

/// The same blocks, each wrapped the one way: 64 columns, and 70 for an OpenSSH key as `ssh-keygen` writes it. A text with no block is
/// not a PEM and stays as it is.
pub fn repem(text: &str) -> String {
    let blocks = pem_blocks(text);
    if blocks.is_empty() {
        return text.to_string();
    }
    let mut out = String::new();
    for (label, bytes) in blocks {
        let width = if label == "OPENSSH PRIVATE KEY" { 70 } else { 64 };
        out.push_str(&format!("-----BEGIN {label}-----\n"));
        for line in STANDARD.encode(&bytes).as_bytes().chunks(width) {
            out.push_str(std::str::from_utf8(line).unwrap_or(""));
            out.push('\n');
        }
        out.push_str(&format!("-----END {label}-----\n"));
    }
    out
}

/// What a note and its fields say, wrapped the one way, to be compared with what this side would write
pub fn canon(content: &mut Content) {
    if content.marker.as_deref().is_some_and(|marker| matches!(marker, MARKER_KEY | MARKER_CERT)) || content.kind == SSH {
        if let Some(notes) = &mut content.notes
            && notes.contains("-----BEGIN ")
        {
            *notes = repem(notes);
        }
        for field in content.fields.iter_mut().filter(|field| field.hidden) {
            field.value = repem(&field.value);
        }
        if let Some(ssh) = &mut content.ssh {
            ssh.private = repem(&ssh.private);
        }
    }
}

fn plain_field(name: &str, value: &str) -> Field {
    Field { name: name.to_string(), value: value.to_string(), hidden: false }
}

fn hidden_field(value: &str) -> Field {
    Field { name: FIELD_PRIVATE.to_string(), value: value.to_string(), hidden: true }
}

fn base_of(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).to_string()
}

fn b64(value: &Value, name: &str) -> Option<Zeroizing<Vec<u8>>> {
    STANDARD.decode(value.get(name)?.as_str()?).ok().map(Zeroizing::new)
}

/// A private key as the text a person copies out: PKCS#8 PEM, or an AES key's base64 line
fn key_text(key_type: &str, bytes: &[u8]) -> Zeroizing<String> {
    if key_type == "aes-256" { Zeroizing::new(format!("{}\n", STANDARD.encode(bytes))) } else { Zeroizing::new(crate::keys::pem("PRIVATE KEY", bytes)) }
}

// --- A key -----------------------------------------------------------------------------------------------------------------

/// A key as an SSH key item where it is one of the six Bitwarden's tools make, and its text fits; otherwise a note with its public text,
/// its private key in a hidden field, and where that does not fit the field, `NAME.pem` as the attachment
///
/// `fixed` is for a cipher that exists: it keeps its form, and a key that cannot be written in it (the worker gave no answer, the key is of
/// a kind an SSH item does not hold, or its text no longer fits) is not written at all rather than made into a note in its place.
pub fn key_made(local: &Local, convert: &dyn Convert, ssh_items: bool, fixed: bool) -> Result<Made, Unmapped> {
    let value = &local.value;
    let (Some(key_type), Some(bytes)) = (value["type"].as_str(), b64(value, "key")) else { return Err(Unmapped::Unreadable) };
    let kind = plain_field(FIELD_KIND, "key");
    if ssh_items && value["key_kind"] == json!("ssh") && let Some(forms) = convert.ssh_forms(key_type, &bytes) {
        let ssh = Ssh { private: forms.private.to_string(), public: forms.public, fingerprint: forms.fingerprint };
        let content = Content { kind: SSH, name: local.name.clone(), marker: Some(MARKER_KEY.to_string()), fields: vec![kind.clone()], ssh: Some(ssh), ..Content::default() };
        if content.check_lengths().is_ok() {
            return Ok(Made { content, file: None });
        }
    }
    if fixed && ssh_items && value["key_kind"] == json!("ssh") {
        return Err(Unmapped::Unreadable);
    }
    let text = key_text(key_type, &bytes);
    let public = value["public"]["openssh"].as_str().map(str::to_string).or_else(|| value["public"]["pem"].as_str().map(str::to_string));
    let mut content = Content { kind: NOTE, name: local.name.clone(), marker: Some(MARKER_KEY.to_string()), notes: public.filter(|public| public.len() <= MOST_NOTES), fields: vec![kind], ..Content::default() };
    if text.len() <= MOST_FIELD {
        content.fields.push(hidden_field(&text));
        content.fields.sort_by(|a, b| a.name.cmp(&b.name));
        return Ok(Made { content, file: None });
    }
    Ok(Made { content, file: Some(File { name: format!("{}.pem", base_of(&local.name)), bytes: Zeroizing::new(text.as_bytes().to_vec()) }) })
}

/// A key back from the server's note, SSH item or file, as the item it was or a new one. None: it no longer reads as a key, or as this
/// one, which is a conflict and never a deletion.
pub fn key_read(existing: Option<&Local>, content: &Content, file: Option<&[u8]>, convert: &dyn Convert) -> Option<Read> {
    let text: Zeroizing<String> = match (&content.ssh, content.fields.iter().find(|field| field.name == FIELD_PRIVATE), file) {
        (Some(ssh), _, _) => Zeroizing::new(ssh.private.clone()),
        (None, Some(field), _) => Zeroizing::new(field.value.clone()),
        (None, None, Some(file)) => Zeroizing::new(String::from_utf8(file.to_vec()).ok()?),
        _ => return None,
    };
    let read = convert.key_from_text(&repem(&text))?;
    if !read.agree {
        return None;
    }
    let comment = existing.and_then(|local| local.value["comment"].as_str().map(str::to_string));
    let (mut value, mut meta) = match existing {
        Some(local) => {
            // Only the same key is taken back: its type, and its public key (or, with none, its bytes). Another key is the server's version
            // for the caller to keep beside it, never a replacement of what the vault signs and logs in with.
            if local.value["type"].as_str() != Some(read.key_type.as_str()) {
                return None;
            }
            let same = match (local.value["public"]["sha256"].as_str(), read.public["sha256"].as_str()) {
                (Some(held), Some(read)) => held == read,
                _ => b64(&local.value, "key").is_some_and(|held| crate::seal::same(&held, &read.key)),
            };
            if !same {
                return None;
            }
            (local.value.clone(), local.meta.clone())
        }
        None => {
            let key_kind = if content.ssh.is_some() { "ssh" } else { "signing" };
            (json!({"key_kind": key_kind, "type": read.key_type, "exportable": true, "comment": comment}), json!({"key_kind": key_kind, "type": read.key_type, "exportable": true}))
        }
    };
    value["key"] = json!(STANDARD.encode(&*read.key));
    value["public"] = read.public.clone();
    for field in ["sha256", "ssh_sha256", "openssh"] {
        match read.public.get(field).filter(|found| !found.is_null()) {
            Some(found) => meta[field] = found.clone(),
            None => {
                if let Some(object) = meta.as_object_mut() {
                    object.remove(field);
                }
            }
        }
    }
    Some(Read { value, name: content.name.clone(), meta })
}

// --- A certificate -----------------------------------------------------------------------------------------------------------

/// The chain as PEM in the note, the key as PEM in a hidden field, its kind and expiry as text; where either does not fit, both go as
/// `NAME.pem` and the note says nothing more
pub fn cert_made(local: &Local) -> Result<Made, Unmapped> {
    let value = &local.value;
    let (Some(key_type), Some(key), Some(cert)) = (value["type"].as_str(), b64(value, "key"), b64(value, "cert")) else { return Err(Unmapped::Unreadable) };
    let mut chain = crate::keys::pem("CERTIFICATE", &cert);
    for one in value["chain"].as_array().ok_or(Unmapped::Unreadable)? {
        let der = STANDARD.decode(one.as_str().ok_or(Unmapped::Unreadable)?).map_err(|_| Unmapped::Unreadable)?;
        chain.push_str(&crate::keys::pem("CERTIFICATE", &der));
    }
    let private = key_text(key_type, &key);
    let mut fields = vec![plain_field(FIELD_KIND, "certificate")];
    if let Some(until) = local.meta["not_after"].as_i64() {
        fields.push(plain_field(FIELD_EXPIRES, &cipher::time_text(until, None)));
    }
    let mut content = Content { kind: NOTE, name: local.name.clone(), marker: Some(MARKER_CERT.to_string()), fields, ..Content::default() };
    if chain.len() <= MOST_NOTES && private.len() <= MOST_FIELD {
        content.notes = Some(chain);
        content.fields.push(hidden_field(&private));
        content.fields.sort_by(|a, b| a.name.cmp(&b.name));
        return Ok(Made { content, file: None });
    }
    let mut both = Zeroizing::new(private.to_string());
    both.push_str(&chain);
    content.fields.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Made { content, file: Some(File { name: format!("{}.pem", base_of(&local.name)), bytes: Zeroizing::new(both.as_bytes().to_vec()) }) })
}

/// A certificate back: only its chain may change, and every certificate has to parse, the leaf be the one the item holds and the key
/// the leaf's own. Anything else is no longer this certificate.
pub fn cert_read(existing: &Local, content: &Content, file: Option<&[u8]>, convert: &dyn Convert) -> Option<Read> {
    let (chain_text, key_text): (Zeroizing<String>, Option<Zeroizing<String>>) = match file {
        Some(file) => {
            let all = String::from_utf8(file.to_vec()).ok()?;
            (Zeroizing::new(all.clone()), Some(Zeroizing::new(all)))
        }
        None => (Zeroizing::new(content.notes.clone()?), content.fields.iter().find(|field| field.name == FIELD_PRIVATE).map(|field| Zeroizing::new(field.value.clone()))),
    };
    let certs: Vec<Vec<u8>> = pem_blocks(&chain_text).into_iter().filter(|(label, _)| label == "CERTIFICATE").map(|(_, der)| der).collect();
    let key = convert.key_from_text(&repem(&key_text?))?;
    let held = (existing.value["type"].as_str()?, b64(&existing.value, "key")?);
    if key.key_type != held.0 || !crate::seal::same(&key.key, &held.1) || certs.is_empty() || certs.len() > 16 {
        return None;
    }
    let facts = convert.cert_facts(&certs, Some((held.0, &key.key)))?;
    if !facts.key_matches || !facts.chain_ok || existing.meta["sha256"].as_str() != Some(facts.leaf_sha256.as_str()) || existing.meta["spki_sha256"].as_str() != Some(facts.spki_sha256.as_str()) {
        return None;
    }
    let mut value = existing.value.clone();
    value["cert"] = json!(STANDARD.encode(&certs[0]));
    value["chain"] = json!(certs[1..].iter().map(|der| STANDARD.encode(der)).collect::<Vec<_>>());
    Some(Read { value, name: content.name.clone(), meta: existing.meta.clone() })
}

// --- A protected file ----------------------------------------------------------------------------------------------------------

/// The item's name for what it holds: the last name of its path
fn path_name(local: &Local) -> String {
    let path = local.meta["path"].as_str().unwrap_or(&local.name);
    let name = base_of(path.trim_end_matches('/'));
    if name.is_empty() { "file".to_string() } else { name }
}

/// A protected file as a note named after it, and the file as its attachment: a file's own bytes, a folder as a tar of what it holds
pub fn file_made(local: &Local) -> Result<Made, Unmapped> {
    let value = &local.value;
    let files = value["files"].as_array().ok_or(Unmapped::Unreadable)?;
    let name = path_name(local);
    let (file_name, bytes) = match value["what"].as_str() {
        Some("file") if files.len() == 1 => (name, b64(&files[0], "data").ok_or(Unmapped::Unreadable)?),
        Some("folder") => (format!("{name}.tar"), tar_of(value).ok_or(Unmapped::Unreadable)?),
        _ => return Err(Unmapped::Unreadable),
    };
    let content = Content { kind: NOTE, name: local.name.clone(), marker: Some(MARKER_FILE.to_string()), fields: vec![plain_field(FIELD_KIND, "file")], ..Content::default() };
    Ok(Made { content, file: Some(File { name: file_name, bytes }) })
}

/// A file back, which a replaced attachment is: the bytes as the item's single file (its mode kept), or the tar as its tree. The same
/// checks as the item had when it was made (`accepts`), or it is no longer this item.
pub fn file_read(existing: &Local, bytes: &[u8], convert: &dyn Convert) -> Option<Value> {
    let value = match existing.value["what"].as_str()? {
        "file" => {
            let mode = existing.value["files"][0]["mode"].as_u64().unwrap_or(0o600);
            json!({"what": "file", "files": [{"at": "", "mode": mode, "data": STANDARD.encode(bytes)}], "folders": []})
        }
        // A folder with nothing in it is not a thing to protect
        _ => tree_of_tar(bytes).filter(|tree| tree["files"].as_array().is_some_and(|files| !files.is_empty()))?,
    };
    convert.accepts(KIND_PROTECTED, &value).then_some(value)
}

/// What a plain item reads back as
pub struct Read {
    pub value: Value,
    pub name: String,
    pub meta: Value,
}

// --- Tar ---------------------------------------------------------------------------------------------------------------------

fn octal(out: &mut [u8], number: u64) {
    let width = out.len() - 1;
    let text = format!("{number:0width$o}");
    let used = text.len().min(width);
    out[..used].copy_from_slice(&text.as_bytes()[..used]);
}

fn header(name: &str, prefix: &str, mode: u64, size: u64, kind: u8) -> [u8; 512] {
    let mut block = [0u8; 512];
    let n = name.len().min(100);
    block[..n].copy_from_slice(&name.as_bytes()[..n]);
    octal(&mut block[100..108], mode);
    octal(&mut block[108..116], 0);
    octal(&mut block[116..124], 0);
    octal(&mut block[124..136], size);
    octal(&mut block[136..148], 0);
    block[156] = kind;
    block[257..263].copy_from_slice(b"ustar\0");
    block[263..265].copy_from_slice(b"00");
    let p = prefix.len().min(155);
    block[345..345 + p].copy_from_slice(&prefix.as_bytes()[..p]);
    block[148..156].copy_from_slice(b"        ");
    let sum: u64 = block.iter().map(|byte| u64::from(*byte)).sum();
    let text = format!("{sum:06o}\0 ");
    block[148..156].copy_from_slice(text.as_bytes());
    block
}

fn pad(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(512) {
        out.push(0);
    }
}

/// One entry: a name that fits ustar's name and prefix as it is, or a PAX header carrying the whole path
fn entry(out: &mut Vec<u8>, path: &str, mode: u64, data: &[u8], kind: u8) {
    let slash = |limit: usize| path.char_indices().filter(|(_, c)| *c == '/').map(|(at, _)| at).find(|at| *at <= 155 && path.len() - at - 1 <= limit);
    let (prefix, name, pax) = if path.len() <= 100 {
        ("", path, false)
    } else if let Some(at) = slash(100) {
        (&path[..at], &path[at + 1..], false)
    } else {
        // Cut at a character, and say the rest in the PAX header
        let mut cut = 100;
        while !path.is_char_boundary(cut) {
            cut -= 1;
        }
        ("", &path[..cut], true)
    };
    if pax {
        // `LEN path=PATH\n`, where LEN counts itself
        let body = format!(" path={path}\n");
        let mut length = body.len() + 1;
        while length.to_string().len() + body.len() != length {
            length = length.to_string().len() + body.len();
        }
        let record = format!("{length}{body}");
        out.extend_from_slice(&header("PaxHeader", "", 0o644, record.len() as u64, b'x'));
        out.extend_from_slice(record.as_bytes());
        pad(out);
    }
    out.extend_from_slice(&header(name, prefix, mode, data.len() as u64, kind));
    out.extend_from_slice(data);
    pad(out);
}

/// A folder item's tree as a tar archive: folders, then files, nothing else in it, the same bytes every time
pub fn tar_of(value: &Value) -> Option<Zeroizing<Vec<u8>>> {
    let mut out = Zeroizing::new(Vec::new());
    for folder in value["folders"].as_array()? {
        entry(&mut out, &format!("{}/", folder["at"].as_str()?), folder["mode"].as_u64()?, b"", b'5');
    }
    for file in value["files"].as_array()? {
        let data = b64(file, "data")?;
        entry(&mut out, file["at"].as_str()?, file["mode"].as_u64()?, &data, b'0');
    }
    out.extend_from_slice(&[0u8; 1024]);
    Some(out)
}

fn field(block: &[u8], from: usize, to: usize) -> &[u8] {
    let part = &block[from..to];
    &part[..part.iter().position(|byte| *byte == 0).unwrap_or(part.len())]
}

fn number(block: &[u8], from: usize, to: usize) -> Option<u64> {
    let text = std::str::from_utf8(field(block, from, to)).ok()?.trim();
    if text.is_empty() { Some(0) } else { u64::from_str_radix(text, 8).ok() }
}

/// The path of a PAX header's record, if it names one
fn pax_path(data: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(data).ok()?;
    for record in text.split_inclusive('\n') {
        let (_, rest) = record.split_once(' ')?;
        if let Some(path) = rest.strip_prefix("path=") {
            return Some(path.trim_end_matches('\n').to_string());
        }
    }
    None
}

/// A tar archive read as a folder item's tree. Only files and folders, in names that stay where they are, within the vault's own
/// bounds; anything else (a link, a device, an absolute path) is refused and the whole is not an item.
pub fn tree_of_tar(bytes: &[u8]) -> Option<Value> {
    if bytes.len() > MOST_TAR || !bytes.len().is_multiple_of(512) {
        return None;
    }
    let (mut files, mut folders) = (Vec::new(), Vec::new());
    let mut at = 0;
    let mut long: Option<String> = None;
    let mut room = crate::protocol::MAX_DATA;
    while at + 512 <= bytes.len() {
        let block = &bytes[at..at + 512];
        at += 512;
        if block.iter().all(|byte| *byte == 0) {
            break;
        }
        let stored = number(block, 148, 156)?;
        let sum: u64 = block.iter().enumerate().map(|(i, byte)| if (148..156).contains(&i) { 32 } else { u64::from(*byte) }).sum();
        if stored != sum {
            return None;
        }
        let size = usize::try_from(number(block, 124, 136)?).ok()?;
        let end = at.checked_add(size)?;
        if end > bytes.len() {
            return None;
        }
        let data = &bytes[at..end];
        at = end.div_ceil(512) * 512;
        let kind = block[156];
        if matches!(kind, b'x' | b'L') {
            long = Some(if kind == b'x' { pax_path(data)? } else { String::from_utf8(data.iter().copied().take_while(|byte| *byte != 0).collect()).ok()? });
            continue;
        }
        let name = String::from_utf8(field(block, 0, 100).to_vec()).ok()?;
        let prefix = String::from_utf8(field(block, 345, 500).to_vec()).ok()?;
        let path = long.take().unwrap_or_else(|| if prefix.is_empty() { name } else { format!("{prefix}/{name}") });
        let path = path.trim_end_matches('/').to_string();
        let mode = number(block, 100, 108)? & 0o777;
        if path.is_empty() || path.starts_with('/') || path.split('/').any(|part| part.is_empty() || part == "." || part == "..") || files.len() + folders.len() >= MOST_ENTRIES {
            return None;
        }
        match kind {
            b'0' | 0 => {
                room = room.checked_sub(data.len())?;
                files.push(json!({"at": path, "mode": mode, "data": STANDARD.encode(data)}));
            }
            b'5' => folders.push(json!({"at": path, "mode": mode})),
            _ => return None,
        }
    }
    files.sort_by(|a, b| a["at"].as_str().cmp(&b["at"].as_str()));
    folders.sort_by(|a, b| a["at"].as_str().cmp(&b["at"].as_str()));
    Some(json!({"what": "folder", "files": files, "folders": folders}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pem_is_read_for_what_it_says_not_how_it_is_wrapped() {
        let der: Vec<u8> = (0..200u8).collect();
        let proper = crate::keys::pem("PRIVATE KEY", &der);
        let joined = format!("-----BEGIN PRIVATE KEY-----{}-----END PRIVATE KEY-----", proper.lines().filter(|line| !line.starts_with("-----")).collect::<String>());
        let spaced = proper.replace('\n', "  \r\n ");
        assert_eq!(repem(&joined), proper);
        assert_eq!(repem(&spaced), proper);
        assert_eq!(pem_blocks(&joined), vec![("PRIVATE KEY".to_string(), der.clone())]);
        // Two blocks, a text with none, and a block that does not decode
        let two = format!("{proper}{}", crate::keys::pem("CERTIFICATE", &der));
        assert_eq!(pem_blocks(&two).len(), 2);
        assert_eq!(repem("ssh-ed25519 AAAA"), "ssh-ed25519 AAAA");
        assert!(pem_blocks("-----BEGIN PRIVATE KEY-----\n!!!\n-----END PRIVATE KEY-----").is_empty());
        assert!(pem_blocks("-----BEGIN PRIVATE KEY-----\nAAAA\n").is_empty());
    }

    fn tree() -> Value {
        json!({"what": "folder", "files": [
            {"at": "a/b.txt", "mode": 0o600, "data": STANDARD.encode(b"hello")},
            {"at": "a/c/d".to_string() + "/" + &"x".repeat(90) + "/" + &"y".repeat(90) + "/long.bin", "mode": 0o644, "data": STANDARD.encode(vec![7u8; 700])},
            {"at": "empty", "mode": 0o600, "data": ""},
            {"at": "p".repeat(150), "mode": 0o600, "data": STANDARD.encode(b"pax")},
        ], "folders": [{"at": "a", "mode": 0o700}, {"at": "a/c", "mode": 0o755}]})
    }

    #[test]
    fn a_folder_is_a_tar_and_back_even_with_a_long_path() {
        let first = tar_of(&tree()).unwrap();
        assert_eq!(first.len() % 512, 0);
        assert_eq!(tar_of(&tree()).unwrap().as_slice(), first.as_slice(), "the same bytes every time");
        let back = tree_of_tar(&first).unwrap();
        let mut sorted = tree();
        sorted["files"].as_array_mut().unwrap().sort_by(|a, b| a["at"].as_str().cmp(&b["at"].as_str()));
        assert_eq!(back, sorted);
    }

    #[test]
    fn a_tar_that_leaves_its_place_is_not_an_item() {
        let mut out = Vec::new();
        entry(&mut out, "../escape", 0o600, b"x", b'0');
        out.extend_from_slice(&[0u8; 1024]);
        assert!(tree_of_tar(&out).is_none());
        let mut out = Vec::new();
        entry(&mut out, "/etc/passwd", 0o600, b"x", b'0');
        assert!(tree_of_tar(&out).is_none());
        // A link, a bad checksum, a size past the end
        let mut out = Vec::new();
        entry(&mut out, "link", 0o777, b"", b'2');
        assert!(tree_of_tar(&out).is_none());
        let mut ok = Vec::new();
        entry(&mut ok, "fine", 0o600, b"x", b'0');
        ok.extend_from_slice(&[0u8; 1024]);
        assert!(tree_of_tar(&ok).is_some());
        let mut bad = ok.clone();
        bad[0] ^= 1;
        assert!(tree_of_tar(&bad).is_none());
        let mut long = ok.clone();
        long[124..135].copy_from_slice(b"77777777777");
        assert!(tree_of_tar(&long).is_none());
    }

    #[test]
    fn a_tar_is_what_tar_reads() {
        // A tar by hand of one file, as `tar` of GNU and BSD write it
        let mut out = Vec::new();
        entry(&mut out, "folder/file.txt", 0o644, b"hello\n", b'0');
        out.extend_from_slice(&[0u8; 1024]);
        let tree = tree_of_tar(&out).unwrap();
        assert_eq!(tree["files"][0]["at"], "folder/file.txt");
        assert_eq!(&out[257..263], b"ustar\0");
    }
}
