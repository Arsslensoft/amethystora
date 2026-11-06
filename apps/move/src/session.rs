//! The conversation of a move, over any stream (a TCP connection, or a pair of sockets in a test).
//!
//! 1. **Keys.** The sender commits, the receiver answers, the sender reveals (`crypto`): both have a key and six digits.
//! 2. **The digits.** Each person says whether the two screens show the same six digits, and each side sends its answer sealed;
//!    nothing goes on unless both say yes.
//! 3. **The offer.** The sender says what it has (the home folder with its size, the setup file); the receiver chooses.
//! 4. **The stream.** For each chosen thing, its files, one frame of header and frames of data each, written by `plan::Writer`,
//!    which refuses what the rules refuse whatever the sender says.
//! 5. **The end.** The receiver says what it wrote, kept and refused.
//!
//! Everything after the handshake is sealed, with a counter for a nonce: a frame cannot be changed, dropped, repeated or
//! reordered. A frame is at most a megabyte of data; a file header is at most 8 KiB.

use std::fmt;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::crypto::{self, Ephemeral, Keys, MOST_FRAME, Role};
use crate::plan::{self, Item, Writer};
use crate::text::tr;

const JSON: u8 = 1;
const FILE: u8 = 2;
const DATA: u8 = 3;
const ITEM_END: u8 = 4;

/// What a sealed data frame carries at most
const CHUNK: usize = 256 * 1024;
const MOST_HEADER: usize = 8 * 1024;
/// A file longer than this is not a person's file
const MOST_FILE: u64 = 1 << 50;
/// What the new computer keeps free, so that a move cannot fill its disk
pub const KEEP_FREE: u64 = 1 << 30;

#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error(e.to_string())
    }
}

impl From<crypto::Error> for Error {
    fn from(e: crypto::Error) -> Error {
        Error(match e {
            crypto::Error::Key => "the key exchange failed".into(),
            crypto::Error::Commitment => "the other computer changed its mind in the middle of the key exchange".into(),
            crypto::Error::Frame => "what came over the network was changed, or was not for this move".into(),
        })
    }
}

pub type Result<T> = std::result::Result<T, Error>;

fn fail<T>(why: &str) -> Result<T> {
    Err(Error(why.to_string()))
}

/// What the person at one computer is asked and told
pub trait Ui {
    /// Both screens show the same six digits?
    fn confirm_digits(&mut self, digits: &str) -> bool;
    /// What the old computer has; the answer is what to bring
    fn choose(&mut self, from: &str, offered: &[Offered]) -> Vec<Item>;
    fn say(&mut self, line: &str);
    /// Bytes done of bytes to come, for the thing being moved
    fn progress(&mut self, done: u64, total: u64);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offered {
    pub item: Item,
    pub bytes: u64,
    pub files: u64,
}

/// What a move did
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub written: u64,
    pub kept: u64,
    pub refused: u64,
    pub bytes: u64,
}

// --- Frames ----------------------------------------------------------------------------------------------------------

fn write_plain<S: Write>(s: &mut S, bytes: &[u8]) -> Result<()> {
    s.write_all(&(bytes.len() as u16).to_be_bytes())?;
    s.write_all(bytes)?;
    s.flush()?;
    Ok(())
}

fn read_plain<S: Read>(s: &mut S, most: usize) -> Result<Vec<u8>> {
    let mut len = [0u8; 2];
    s.read_exact(&mut len)?;
    let len = u16::from_be_bytes(len) as usize;
    if len > most {
        return fail("the other computer sent something too long");
    }
    let mut out = vec![0u8; len];
    s.read_exact(&mut out)?;
    Ok(out)
}

fn write_sealed<S: Write>(s: &mut S, keys: &mut Keys, kind: u8, body: &[u8]) -> Result<()> {
    let sealed = keys.seal(kind, body);
    s.write_all(&(sealed.len() as u32).to_be_bytes())?;
    s.write_all(&sealed)?;
    Ok(())
}

fn read_sealed<S: Read>(s: &mut S, keys: &mut Keys) -> Result<(u8, Vec<u8>)> {
    let mut len = [0u8; 4];
    s.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MOST_FRAME + 17 {
        return fail("the other computer sent a frame that is too long");
    }
    let mut sealed = vec![0u8; len];
    s.read_exact(&mut sealed)?;
    Ok(keys.open(&sealed)?)
}

fn write_json<S: Write>(s: &mut S, keys: &mut Keys, value: &Value) -> Result<()> {
    write_sealed(s, keys, JSON, value.to_string().as_bytes())
}

fn read_json<S: Read>(s: &mut S, keys: &mut Keys) -> Result<Value> {
    match read_sealed(s, keys)? {
        (JSON, body) if body.len() <= 64 * 1024 => serde_json::from_slice(&body).map_err(|_| Error("the other computer sent something that is not a message".into())),
        _ => fail("the other computer sent something out of turn"),
    }
}

// --- The handshake -------------------------------------------------------------------------------------------------------

/// The old computer's half: commit, hear the other's key, reveal
pub fn handshake_sender<S: Read + Write>(s: &mut S) -> Result<Keys> {
    let own = Ephemeral::new()?;
    let mut first = vec![b'1'];
    first.extend_from_slice(&own.commitment());
    write_plain(s, &first)?;
    let second = read_plain(s, 128)?;
    if second.len() != 1 + 65 + 16 || second[0] != b'2' {
        return fail("the other computer does not speak this move");
    }
    let nonce: [u8; 16] = second[66..].try_into().expect("sixteen bytes");
    let mut third = vec![b'3'];
    third.extend_from_slice(&own.public);
    third.extend_from_slice(&own.nonce);
    write_plain(s, &third)?;
    Ok(crypto::agree(Role::Sender, &own, &second[1..66], &nonce)?)
}

/// The new computer's half: hear the commitment, answer with a key, check the reveal against it
pub fn handshake_receiver<S: Read + Write>(s: &mut S) -> Result<Keys> {
    let first = read_plain(s, 128)?;
    if first.len() != 1 + 32 || first[0] != b'1' {
        return fail("the other computer does not speak this move");
    }
    let commitment: [u8; 32] = first[1..].try_into().expect("thirty-two bytes");
    let own = Ephemeral::new()?;
    let mut second = vec![b'2'];
    second.extend_from_slice(&own.public);
    second.extend_from_slice(&own.nonce);
    write_plain(s, &second)?;
    let third = read_plain(s, 128)?;
    if third.len() != 1 + 65 + 16 || third[0] != b'3' {
        return fail("the other computer does not speak this move");
    }
    if crypto::commit(&third[1..66], &third[66..]) != commitment {
        return Err(crypto::Error::Commitment.into());
    }
    let nonce: [u8; 16] = third[66..].try_into().expect("sixteen bytes");
    Ok(crypto::agree(Role::Receiver, &own, &third[1..66], &nonce)?)
}

/// Both people say what they see; both must say yes
fn compare<S: Read + Write>(s: &mut S, keys: &mut Keys, ui: &mut dyn Ui) -> Result<()> {
    let mine = ui.confirm_digits(&keys.digits.clone());
    write_json(s, keys, &json!({ "match": mine }))?;
    let theirs = read_json(s, keys)?.get("match").and_then(Value::as_bool).unwrap_or(false);
    if !(mine && theirs) {
        return fail("the digits were not the same on both screens, so nothing was moved");
    }
    Ok(())
}

// --- The old computer ----------------------------------------------------------------------------------------------------

/// What the old computer offers and sends: `setup` is the setup file's bytes, when there is one
pub fn send<S: Read + Write>(s: &mut S, keys: &mut Keys, ui: &mut dyn Ui, home: &Path, name: &str, setup: Option<&[u8]>) -> Result<Summary> {
    compare(s, keys, ui)?;
    ui.say(&tr("Counting what there is to bring…"));
    let counted = plan::walk(home, &mut |_| true)?;
    let mut items = vec![json!({ "id": Item::Home.id(), "bytes": counted.bytes, "files": counted.files })];
    if let Some(setup) = setup {
        items.push(json!({ "id": Item::Setup.id(), "bytes": setup.len(), "files": 1 }));
    }
    write_json(s, keys, &json!({ "v": 1, "name": name, "items": items }))?;
    let chosen = read_json(s, keys)?;
    let wanted: Vec<Item> = chosen.get("items").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().and_then(Item::from_id)).collect()).unwrap_or_default();
    let mut total = 0u64;
    for item in &wanted {
        total += match item {
            Item::Home => counted.bytes,
            Item::Setup => setup.map_or(0, |b| b.len() as u64),
        };
    }
    let mut done = 0u64;
    for item in wanted {
        write_json(s, keys, &json!({ "item": item.id() }))?;
        match item {
            Item::Home => {
                let mut failure: Option<Error> = None;
                plan::walk(home, &mut |entry| match send_file(s, keys, home, &entry, &mut done, total, ui) {
                    Ok(()) => true,
                    Err(e) => {
                        failure = Some(e);
                        false
                    }
                })?;
                if let Some(e) = failure {
                    return Err(e);
                }
            }
            Item::Setup => {
                let Some(bytes) = setup else { return fail("the other computer asked for what was not offered") };
                write_sealed(s, keys, FILE, json!({ "p": "setup.json", "m": 0o600, "t": 0, "s": bytes.len() }).to_string().as_bytes())?;
                for chunk in bytes.chunks(CHUNK) {
                    write_sealed(s, keys, DATA, chunk)?;
                }
                done += bytes.len() as u64;
                ui.progress(done, total);
            }
        }
        write_sealed(s, keys, ITEM_END, &[])?;
    }
    s.flush()?;
    let end = read_json(s, keys)?;
    let number = |k: &str| end.get(k).and_then(Value::as_u64).unwrap_or(0);
    Ok(Summary { written: number("written"), kept: number("kept"), refused: number("refused"), bytes: number("bytes") })
}

fn send_file<S: Read + Write>(s: &mut S, keys: &mut Keys, home: &Path, entry: &plan::Entry, done: &mut u64, total: u64, ui: &mut dyn Ui) -> Result<()> {
    let path: PathBuf = home.join(&entry.rel);
    // Opened without following a link, and sized from what is open, so that a file that changes meanwhile is still one length
    let file = match open_plain(&path) {
        Ok(f) => f,
        // Gone since it was counted: said nowhere, the move goes on
        Err(_) => return Ok(()),
    };
    let size = file.metadata().map(|m| m.len()).unwrap_or(entry.size);
    write_sealed(s, keys, FILE, json!({ "p": entry.rel, "m": entry.mode, "t": entry.mtime, "s": size }).to_string().as_bytes())?;
    let mut file = file;
    let mut left = size;
    let mut buffer = vec![0u8; CHUNK];
    while left > 0 {
        let want = (left as usize).min(CHUNK);
        let got = file.read(&mut buffer[..want])?;
        if got == 0 {
            // Shorter than it was when opened: the rest is sent as nothing, since the other side has been told the length
            buffer[..want].fill(0);
            write_sealed(s, keys, DATA, &buffer[..want])?;
            left -= want as u64;
        } else {
            write_sealed(s, keys, DATA, &buffer[..got])?;
            left -= got as u64;
        }
        *done += if got == 0 { want as u64 } else { got as u64 };
        ui.progress(*done, total.max(*done));
    }
    Ok(())
}

fn open_plain(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path)?;
    if file.metadata()?.is_file() { Ok(file) } else { Err(io::Error::other("not a plain file")) }
}

// --- The new computer ----------------------------------------------------------------------------------------------------

/// Receives into `home`: the home folder's files under it, the setup file in `From old computer`
pub fn receive<S: Read + Write>(s: &mut S, keys: &mut Keys, ui: &mut dyn Ui, home: &Path) -> Result<Summary> {
    compare(s, keys, ui)?;
    let offer = read_json(s, keys)?;
    let from = crate::text::clean(offer.get("name").and_then(Value::as_str).unwrap_or("?"));
    let mut offered = Vec::new();
    for item in offer.get("items").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]).iter().take(4) {
        let Some(known) = item.get("id").and_then(Value::as_str).and_then(Item::from_id) else { continue };
        if offered.iter().any(|o: &Offered| o.item == known) {
            continue;
        }
        offered.push(Offered { item: known, bytes: item.get("bytes").and_then(Value::as_u64).unwrap_or(0).min(MOST_FILE), files: item.get("files").and_then(Value::as_u64).unwrap_or(0).min(plan::MOST_FILES) });
    }
    let mut wanted: Vec<Item> = ui.choose(&from, &offered).into_iter().filter(|i| offered.iter().any(|o| o.item == *i)).collect();
    // Room for it: the disk is never filled by a move
    if wanted.contains(&Item::Home) {
        let bytes = offered.iter().find(|o| o.item == Item::Home).map_or(0, |o| o.bytes);
        if plan::free_space(home).is_some_and(|free| bytes > free.saturating_sub(KEEP_FREE)) {
            ui.say(&tr("There is not room for the home folder here, so it is left out."));
            wanted.retain(|i| *i != Item::Home);
        }
    }
    write_json(s, keys, &json!({ "items": wanted.iter().map(|i| i.id()).collect::<Vec<_>>() }))?;
    let total: u64 = offered.iter().filter(|o| wanted.contains(&o.item)).map(|o| o.bytes).sum();
    let free = plan::free_space(home);
    let mut summary = Summary::default();
    let mut done = 0u64;
    for item in &wanted {
        match read_json(s, keys)?.get("item").and_then(Value::as_str).and_then(Item::from_id) {
            Some(got) if got == *item => {}
            _ => return fail("the other computer sent the wrong thing"),
        }
        let root = match item {
            Item::Home => home.to_path_buf(),
            Item::Setup => home.join("From old computer"),
        };
        let mut writer = Writer::new(root, *item);
        let declared = offered.iter().find(|o| o.item == *item).map_or(0, |o| o.bytes);
        let budget = budget_for(*item, declared, free);
        let result = receive_item(s, keys, &mut writer, &mut done, total, budget, ui);
        if result.is_err() {
            writer.abandon();
        }
        result?;
        summary.written += writer.written;
        summary.kept += writer.kept;
        summary.refused += writer.refused;
        summary.bytes += writer.bytes;
    }
    write_json(s, keys, &json!({ "written": summary.written, "kept": summary.kept, "refused": summary.refused, "bytes": summary.bytes }))?;
    s.flush()?;
    Ok(summary)
}

/// The setup file is small, whatever the other side says
pub const MOST_SETUP: u64 = 1 << 20;

/// The most the files of one item may add up to: what the sender said, with room for what grew while it counted, and never more than
/// the disk has to spare. A sender that declares a little and streams a lot is stopped by it, and the disk is never filled.
pub fn budget_for(item: Item, declared: u64, free: Option<u64>) -> u64 {
    let said = match item {
        Item::Setup => declared.min(MOST_SETUP),
        Item::Home => declared.saturating_add(declared / 100).saturating_add(64 << 20),
    };
    free.map_or(said, |free| said.min(free.saturating_sub(KEEP_FREE)))
}

fn receive_item<S: Read + Write>(s: &mut S, keys: &mut Keys, writer: &mut Writer, done: &mut u64, total: u64, budget: u64, ui: &mut dyn Ui) -> Result<()> {
    // What the files of this item have promised so far, by their headers
    let mut promised = 0u64;
    // The file being written: how long it is and how much has come
    let mut open: Option<(u64, u64)> = None;
    loop {
        let (kind, body) = read_sealed(s, keys)?;
        match kind {
            FILE => {
                if open.is_some() || body.len() > MOST_HEADER {
                    return fail("the other computer sent a file out of turn");
                }
                let header: Value = serde_json::from_slice(&body).map_err(|_| Error("the other computer sent a file header that is not one".into()))?;
                let (Some(rel), Some(size)) = (header.get("p").and_then(Value::as_str), header.get("s").and_then(Value::as_u64)) else { return fail("a file header lacks its name or length") };
                if size > MOST_FILE {
                    return fail("a file is longer than any file");
                }
                promised = promised.saturating_add(size);
                if promised > budget {
                    return fail("the other computer is sending more than it said, or than there is room for");
                }
                let mode = header.get("m").and_then(Value::as_u64).unwrap_or(0o600) as u32;
                let mtime = header.get("t").and_then(Value::as_i64).unwrap_or(0);
                writer.begin(rel, size, mode, mtime)?;
                if size == 0 {
                    writer.end()?;
                } else {
                    open = Some((size, 0));
                }
            }
            DATA => {
                let Some((size, got)) = open.as_mut() else { return fail("data came with no file") };
                writer.data(&body)?;
                *got += body.len() as u64;
                *done += body.len() as u64;
                ui.progress(*done, total.max(*done));
                if *got == *size {
                    writer.end()?;
                    open = None;
                }
            }
            ITEM_END => {
                if open.is_some() {
                    return fail("the other computer ended in the middle of a file");
                }
                return Ok(());
            }
            _ => return fail("the other computer sent something out of turn"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    struct Script {
        digits_ok: bool,
        take: Vec<Item>,
        said: Arc<Mutex<Vec<String>>>,
        last_progress: Arc<Mutex<(u64, u64)>>,
        seen_digits: Arc<Mutex<String>>,
    }

    impl Script {
        fn new(digits_ok: bool, take: Vec<Item>) -> Script {
            Script { digits_ok, take, said: Arc::default(), last_progress: Arc::default(), seen_digits: Arc::default() }
        }
    }

    impl Ui for Script {
        fn confirm_digits(&mut self, digits: &str) -> bool {
            *self.seen_digits.lock().unwrap() = digits.to_string();
            self.digits_ok
        }
        fn choose(&mut self, _: &str, _: &[Offered]) -> Vec<Item> {
            self.take.clone()
        }
        fn say(&mut self, line: &str) {
            self.said.lock().unwrap().push(line.to_string());
        }
        fn progress(&mut self, done: u64, total: u64) {
            *self.last_progress.lock().unwrap() = (done, total);
        }
    }

    fn temp(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("amethystora-move-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn put(root: &Path, rel: &str, content: &[u8]) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    /// Runs both sides on a real loopback connection, the new computer in a thread
    fn run(old: &Path, new: &Path, setup: Option<Vec<u8>>, sender_ok: bool, receiver_ok: bool, take: Vec<Item>) -> (Result<Summary>, Result<Summary>, String, String) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let new_home = new.to_path_buf();
        let receiver_ui = Script::new(receiver_ok, take);
        let receiver_digits = receiver_ui.seen_digits.clone();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(20))).unwrap();
            let mut ui = receiver_ui;
            let mut keys = handshake_receiver(&mut stream)?;
            receive(&mut stream, &mut keys, &mut ui, &new_home)
        });
        let mut stream = TcpStream::connect(address).unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(20))).unwrap();
        let mut ui = Script::new(sender_ok, vec![]);
        let sender_digits = ui.seen_digits.clone();
        let sent = (|| {
            let mut keys = handshake_sender(&mut stream)?;
            send(&mut stream, &mut keys, &mut ui, old, "Old laptop", setup.as_deref())
        })();
        drop(stream);
        let received = thread.join().unwrap();
        let (a, b) = (sender_digits.lock().unwrap().clone(), receiver_digits.lock().unwrap().clone());
        (sent, received, a, b)
    }

    fn old_home() -> PathBuf {
        let old = temp("old");
        put(&old, "Documents/report.odt", b"the report");
        put(&old, "Documents/deep/er/notes.txt", &vec![7u8; 700_000]);
        put(&old, "Pictures/holiday.jpg", b"jpeg");
        put(&old, "Music/empty.flac", b"");
        put(&old, ".ssh/id_ed25519", b"PRIVATE KEY");
        put(&old, ".mozilla/firefox/p/logins.json", b"{\"logins\":[]}");
        put(&old, ".config/amethystora/connect/private-key.pem", b"PRIVATE KEY");
        put(&old, ".local/share/keyrings/login.keyring", b"KEYRING");
        put(&old, "Private/diary.txt", b"private");
        put(&old, ".var/app/org.mozilla.firefox/data/profile/key4.db", b"KEYS");
        put(&old, ".local/share/amethystora/vault-backup/vault-2026-11-06.avb", b"sealed backup");
        old
    }

    #[test]
    fn a_move_brings_the_persons_files_and_no_secret() {
        let (old, new) = (old_home(), temp("new"));
        put(&new, "Documents/report.odt", b"the new computer's own");
        let setup = br#"{"version":1,"theme":"nord"}"#.to_vec();
        let (sent, received, a, b) = run(&old, &new, Some(setup), true, true, vec![Item::Home, Item::Setup]);
        let (sent, received) = (sent.unwrap(), received.unwrap());
        assert_eq!(a, b, "the same six digits on both screens");
        assert_eq!(sent, received, "both say the same of what happened");
        assert_eq!(std::fs::read(new.join("Documents/deep/er/notes.txt")).unwrap(), vec![7u8; 700_000]);
        assert_eq!(std::fs::read(new.join("Pictures/holiday.jpg")).unwrap(), b"jpeg");
        assert_eq!(std::fs::read(new.join("Music/empty.flac")).unwrap(), b"");
        assert_eq!(std::fs::read(new.join("Documents/report.odt")).unwrap(), b"the new computer's own", "adds, never replaces");
        assert_eq!(std::fs::read(new.join("From old computer/Vault backups/vault-2026-11-06.avb")).unwrap(), b"sealed backup", "the Vault's backup lands where nothing rotates it");
        assert!(!new.join(".local").exists(), "and the new computer's own backup folder is not touched");
        assert_eq!(std::fs::read(new.join("From old computer/setup.json")).unwrap(), br#"{"version":1,"theme":"nord"}"#);
        for secret in [".ssh", ".mozilla", ".config", ".local/share/keyrings", "Private", ".var/app/org.mozilla.firefox"] {
            assert!(!new.join(secret).exists(), "{secret} never crosses");
        }
        assert_eq!(received.kept, 1, "the report that was there");
        assert_eq!(received.written, 5, "notes, holiday, empty, the vault backup and the setup file");
        // No file of the new home holds a canary of a secret
        let mut stack = vec![new.clone()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    let content = std::fs::read(&p).unwrap();
                    assert!(!content.windows(11).any(|w| w == b"PRIVATE KEY"), "{p:?}");
                    assert!(!content.windows(7).any(|w| w == b"private" || w == b"KEYRING"), "{p:?}");
                }
            }
        }
        let _ = std::fs::remove_dir_all(&old);
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn only_what_is_chosen_comes() {
        let (old, new) = (old_home(), temp("new-chosen"));
        let (sent, received, _, _) = run(&old, &new, Some(b"{}".to_vec()), true, true, vec![Item::Setup]);
        sent.unwrap();
        received.unwrap();
        assert!(!new.join("Documents").exists(), "the home folder was not chosen");
        assert!(new.join("From old computer/setup.json").exists());
        let _ = std::fs::remove_dir_all(&old);
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn nothing_moves_unless_both_say_the_digits_match() {
        for (sender_ok, receiver_ok) in [(true, false), (false, true), (false, false)] {
            let (old, new) = (old_home(), temp("new-no"));
            let (sent, received, _, _) = run(&old, &new, None, sender_ok, receiver_ok, vec![Item::Home]);
            assert!(sent.is_err() && received.is_err(), "{sender_ok} {receiver_ok}");
            assert_eq!(std::fs::read_dir(&new).unwrap().count(), 0, "nothing was written");
            let _ = std::fs::remove_dir_all(&old);
            let _ = std::fs::remove_dir_all(&new);
        }
    }

    /// A hostile old computer: it speaks the protocol and sends what the rules refuse
    #[test]
    fn the_new_computer_refuses_a_hostile_stream_whatever_it_says() {
        let new = temp("new-hostile");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let new_home = new.clone();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(20))).unwrap();
            let mut ui = Script::new(true, vec![Item::Home]);
            let mut keys = handshake_receiver(&mut stream)?;
            receive(&mut stream, &mut keys, &mut ui, &new_home)
        });
        let mut stream = TcpStream::connect(address).unwrap();
        let mut keys = handshake_sender(&mut stream).unwrap();
        let mut ui = Script::new(true, vec![]);
        compare(&mut stream, &mut keys, &mut ui).unwrap();
        write_json(&mut stream, &mut keys, &json!({ "v": 1, "name": "Evil\u{1b}[2J", "items": [{ "id": "home", "bytes": 100, "files": 5 }, { "id": "kernel", "bytes": 1, "files": 1 }] })).unwrap();
        let chosen = read_json(&mut stream, &mut keys).unwrap();
        assert_eq!(chosen["items"], json!(["home"]), "only what was offered and known can be chosen");
        write_json(&mut stream, &mut keys, &json!({ "item": "home" })).unwrap();
        let mut files = 0;
        for (rel, body) in [
            ("../../escape.txt", &b"x"[..]),
            ("/etc/cron.d/evil", b"x"),
            (".bashrc", b"evil"),
            (".config/autostart/evil.desktop", b"evil"),
            (".ssh/authorized_keys", b"evil"),
            (".local/bin/ls", b"evil"),
            ("bin/ls", b"evil"),
            ("Documents/ok.txt", b"fine"),
            ("Documents/../.bashrc", b"evil"),
        ] {
            write_sealed(&mut stream, &mut keys, FILE, json!({ "p": rel, "m": 0o4755, "t": 0, "s": body.len() }).to_string().as_bytes()).unwrap();
            write_sealed(&mut stream, &mut keys, DATA, body).unwrap();
            files += 1;
        }
        write_sealed(&mut stream, &mut keys, ITEM_END, &[]).unwrap();
        let end = read_json(&mut stream, &mut keys).unwrap();
        assert_eq!(end["written"], 1);
        assert_eq!(end["refused"], files - 1);
        thread.join().unwrap().unwrap();
        assert_eq!(std::fs::read(new.join("Documents/ok.txt")).unwrap(), b"fine");
        for evil in [".bashrc", ".config", ".ssh", ".local", "bin"] {
            assert!(!new.join(evil).exists(), "{evil}");
        }
        assert!(!new.parent().unwrap().join("escape.txt").exists());
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn what_may_come_is_what_was_said_and_what_there_is_room_for() {
        assert_eq!(budget_for(Item::Setup, 5_000_000_000, None), MOST_SETUP, "the setup file is small whatever is declared");
        assert_eq!(budget_for(Item::Setup, 10, None), 10);
        assert_eq!(budget_for(Item::Home, 1000, None), 1000 + 10 + (64 << 20));
        assert_eq!(budget_for(Item::Home, 10 << 30, Some(3 << 30)), (3 << 30) - KEEP_FREE, "never more than the disk has to spare");
        assert_eq!(budget_for(Item::Home, 10 << 30, Some(0)), 0);
    }

    /// A sender that declares little and streams a lot is stopped, and what was half written is not left
    #[test]
    fn a_sender_that_says_a_little_and_sends_a_lot_is_stopped() {
        let new = temp("new-liar");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let new_home = new.clone();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(20))).unwrap();
            let mut ui = Script::new(true, vec![Item::Home]);
            let mut keys = handshake_receiver(&mut stream)?;
            receive(&mut stream, &mut keys, &mut ui, &new_home)
        });
        let mut stream = TcpStream::connect(address).unwrap();
        let mut keys = handshake_sender(&mut stream).unwrap();
        let mut ui = Script::new(true, vec![]);
        compare(&mut stream, &mut keys, &mut ui).unwrap();
        write_json(&mut stream, &mut keys, &json!({ "v": 1, "name": "Old", "items": [{ "id": "home", "bytes": 1000, "files": 1 }] })).unwrap();
        read_json(&mut stream, &mut keys).unwrap();
        write_json(&mut stream, &mut keys, &json!({ "item": "home" })).unwrap();
        // One file within what it said, then a header that claims a terabyte
        write_sealed(&mut stream, &mut keys, FILE, json!({ "p": "Documents/a.txt", "m": 420, "t": 0, "s": 3 }).to_string().as_bytes()).unwrap();
        write_sealed(&mut stream, &mut keys, DATA, b"abc").unwrap();
        write_sealed(&mut stream, &mut keys, FILE, json!({ "p": "Documents/huge.bin", "m": 420, "t": 0, "s": 1u64 << 40 }).to_string().as_bytes()).unwrap();
        drop(stream);
        let error = thread.join().unwrap().unwrap_err();
        assert!(error.0.contains("more than it said"), "{error}");
        assert_eq!(std::fs::read(new.join("Documents/a.txt")).unwrap(), b"abc", "what was within bounds stays");
        assert!(!new.join("Documents/huge.bin").exists());
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn a_changed_frame_stops_the_move_and_leaves_no_half_file() {
        let new = temp("new-tamper");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let new_home = new.clone();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(20))).unwrap();
            let mut ui = Script::new(true, vec![Item::Home]);
            let mut keys = handshake_receiver(&mut stream)?;
            receive(&mut stream, &mut keys, &mut ui, &new_home)
        });
        let mut stream = TcpStream::connect(address).unwrap();
        let mut keys = handshake_sender(&mut stream).unwrap();
        let mut ui = Script::new(true, vec![]);
        compare(&mut stream, &mut keys, &mut ui).unwrap();
        write_json(&mut stream, &mut keys, &json!({ "v": 1, "name": "Old", "items": [{ "id": "home", "bytes": 10, "files": 1 }] })).unwrap();
        read_json(&mut stream, &mut keys).unwrap();
        write_json(&mut stream, &mut keys, &json!({ "item": "home" })).unwrap();
        write_sealed(&mut stream, &mut keys, FILE, json!({ "p": "Documents/half.txt", "m": 420, "t": 0, "s": 10 }).to_string().as_bytes()).unwrap();
        // A data frame with a bit changed on the way
        let mut sealed = keys.seal(DATA, b"0123456789");
        sealed[3] ^= 0x40;
        stream.write_all(&(sealed.len() as u32).to_be_bytes()).unwrap();
        stream.write_all(&sealed).unwrap();
        drop(stream);
        assert!(thread.join().unwrap().is_err());
        let left: Vec<_> = std::fs::read_dir(new.join("Documents")).map(|d| d.flatten().collect()).unwrap_or_default();
        assert!(left.is_empty(), "no half file stays: {left:?}");
        let _ = std::fs::remove_dir_all(&new);
    }

    #[test]
    fn a_stranger_that_changes_its_key_after_the_commitment_is_caught() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(20))).unwrap();
            handshake_receiver(&mut stream).map(|_| ())
        });
        let mut stream = TcpStream::connect(address).unwrap();
        let committed = Ephemeral::new().unwrap();
        let revealed = Ephemeral::new().unwrap();
        let mut first = vec![b'1'];
        first.extend_from_slice(&committed.commitment());
        write_plain(&mut stream, &first).unwrap();
        read_plain(&mut stream, 128).unwrap();
        let mut third = vec![b'3'];
        third.extend_from_slice(&revealed.public);
        third.extend_from_slice(&revealed.nonce);
        write_plain(&mut stream, &third).unwrap();
        let error = thread.join().unwrap().unwrap_err();
        assert!(error.0.contains("changed its mind"), "{error}");
    }

    #[test]
    fn it_never_writes_a_key_or_the_digits_anywhere() {
        // The keys are values of a struct that is wiped when dropped: nothing in the crate opens a file for them. The move leaves
        // nothing in the new home but what came, and the digits are only ever given to the Ui.
        let (old, new) = (old_home(), temp("new-keys"));
        let (sent, _received, digits, _) = run(&old, &new, None, true, true, vec![Item::Home]);
        sent.unwrap();
        let mut stack = vec![new.clone()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if std::fs::metadata(&p).unwrap().len() < 100 {
                    assert!(!String::from_utf8_lossy(&std::fs::read(&p).unwrap()).contains(&digits));
                }
            }
        }
        let _ = std::fs::remove_dir_all(&old);
        let _ = std::fs::remove_dir_all(&new);
    }
}
