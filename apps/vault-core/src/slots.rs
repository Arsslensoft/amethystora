//! The ways a vault opens. A random vault key seals everything, and is itself sealed once per method (a slot), so that
//! methods come and go without touching the items.
//!
//! - **A vault password**: scrypt at Notes' cost, under one salt per account, whatever the number of its vaults.
//! - **A recovery key**: 256 random bits as grouped text with a checksum, needing no stretching.
//! - **A security key**: the 32 bytes FIDO2's `hmac-secret` gives the client (`apps/notes/src/passkeys.rs` says how), which
//!   the client hands over, since the device is the person's seat's.
//! - **A slot sealed to the chip**, for fingerprint-or-password approval and for no prompt, through [`Chip`].
//!
//! A password and a recovery key each find their vault in a table of [`Record`]s: every record is tried, always
//! [`MAX_VAULTS`] of them, and a record names the vault only once it has opened, which is what keeps a hidden vault
//! hidden (see `vaults`).

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

use crate::seal::{self, Key, Unopened};

/// An account's vaults, hidden ones included. Every attempt tries this many records, so it costs the same however many
/// there are.
pub const MAX_VAULTS: usize = 32;
/// 128 * 2^17 * 8 bytes of memory for every guess at a password, as Notes
pub const LOG_N: u8 = 17;
const R: u32 = 8;
const P: u32 = 1;
/// A password's bytes, after which nobody is typing
pub const MAX_PASSWORD: usize = 1024;
/// A password's characters, below which no cost of scrypt helps
pub const MIN_PASSWORD: usize = 8;

/// The cost and the salt, bounded when read, so that a damaged file cannot ask for all the memory there is
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Kdf {
    pub log_n: u8,
    pub r: u32,
    pub p: u32,
    pub salt: String,
}

impl Kdf {
    /// New settings, for a new account
    pub fn fresh(log_n: u8) -> Kdf {
        let mut salt = [0u8; 16];
        seal::random(&mut salt);
        Kdf { log_n, r: R, p: P, salt: base64::engine::general_purpose::STANDARD.encode(salt) }
    }

    pub fn is_sound(&self) -> bool {
        (10..=20).contains(&self.log_n) && (1..=16).contains(&self.r) && (1..=4).contains(&self.p) && self.salt_bytes().is_some()
    }

    fn salt_bytes(&self) -> Option<Vec<u8>> {
        base64::engine::general_purpose::STANDARD.decode(&self.salt).ok().filter(|salt| (16..=64).contains(&salt.len()))
    }
}

/// A new password this service will take. Whatever else makes one unusable, a password that already opens another vault
/// included, the answer is the same one refusal.
pub fn usable(password: &[u8]) -> bool {
    std::str::from_utf8(password).is_ok_and(|text| text.chars().count() >= MIN_PASSWORD) && password.len() <= MAX_PASSWORD
}

/// The key a password gives: scrypt of its NFC form, so that a letter typed either way opens the same vault. Slow on
/// purpose: never on the service's loop.
pub fn derive(password: &[u8], kdf: &Kdf) -> Result<Key, Unopened> {
    let text = std::str::from_utf8(password).map_err(|_| Unopened)?;
    if password.is_empty() || password.len() > MAX_PASSWORD || !kdf.is_sound() {
        return Err(Unopened);
    }
    let normal = Zeroizing::new(icu_normalizer::ComposingNormalizerBorrowed::new_nfc().normalize(text).into_owned());
    let salt = kdf.salt_bytes().ok_or(Unopened)?;
    let params = scrypt::Params::new(kdf.log_n, kdf.r, kdf.p).map_err(|_| Unopened)?;
    let mut out = [0u8; 32];
    scrypt::scrypt(normal.as_bytes(), &salt, &params, &mut out).map_err(|_| Unopened)?;
    let key = Key::from_bytes(&out);
    out.zeroize();
    Ok(key)
}

/// A record's nonce, tag, and the vault's id and key sealed. It holds no magic and no name: until it opens, it is noise.
pub const RECORD: usize = 12 + 16 + 16 + 32;
pub type Record = [u8; RECORD];

/// Which table a record is of, authenticated with it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    Password,
    Recovery,
}

impl Table {
    fn label(self) -> &'static [u8] {
        match self {
            Table::Password => b"amethystora-vault:password-slot",
            Table::Recovery => b"amethystora-vault:recovery-slot",
        }
    }
}

/// What opening a record gave
pub struct Found {
    /// The vault's id, in hex
    pub vault: String,
    pub key: Key,
    /// The record's own nonce, by which its vault can name it later
    pub record: String,
}

/// The vault key, sealed in a record under a password's or a recovery key's key
pub fn seal_record(under: &Key, vault: &str, key: &Key, table: Table) -> Option<Record> {
    let id: [u8; 16] = seal::unhex(vault)?.try_into().ok()?;
    let mut plain = Zeroizing::new([0u8; 48]);
    plain[..16].copy_from_slice(&id);
    plain[16..].copy_from_slice(key.bytes());
    let mut nonce = [0u8; 12];
    seal::random(&mut nonce);
    let cipher = Aes256Gcm::new_from_slice(under.bytes()).ok()?;
    let iv = Nonce::try_from(&nonce[..]).ok()?;
    let sealed = cipher.encrypt(&iv, Payload { msg: &plain[..], aad: table.label() }).ok()?;
    let mut record = [0u8; RECORD];
    record[..12].copy_from_slice(&nonce);
    record[12..28].copy_from_slice(&sealed[48..]);
    record[28..].copy_from_slice(&sealed[..48]);
    Some(record)
}

/// The vaults these records open under this key. Every attempt is [`MAX_VAULTS`] decryptions of the same size, stopping
/// for nothing, so how long it takes says neither how many vaults there are nor which record opened.
pub fn search(under: &Key, records: &[Record], table: Table) -> Vec<Found> {
    let mut found = Vec::new();
    let Ok(cipher) = Aes256Gcm::new_from_slice(under.bytes()) else { return found };
    let stand_in = [0u8; RECORD];
    for at in 0..MAX_VAULTS {
        let record = records.get(at).unwrap_or(&stand_in);
        let mut joined = [0u8; 64];
        joined[..48].copy_from_slice(&record[28..]);
        joined[48..].copy_from_slice(&record[12..28]);
        let Ok(iv) = Nonce::try_from(&record[..12]) else { continue };
        let opened = cipher.decrypt(&iv, Payload { msg: &joined[..], aad: table.label() });
        if let (Ok(plain), true) = (opened, at < records.len()) {
            let plain = Zeroizing::new(plain);
            if let Some(key) = Key::from_slice(&plain[16..]) {
                found.push(Found { vault: seal::hex(&plain[..16]), key, record: seal::hex(&record[..12]) });
            }
        }
    }
    found
}

// Crockford's alphabet: no I, L, O or U, which read as other letters
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const CHECKSUM: usize = 2;
const CHARACTERS: usize = 55;

/// A recovery key: 256 random bits, and 16 bits of checksum so that a mistyped one is told apart from a wrong one
pub struct RecoveryKey(Zeroizing<[u8; 32]>);

impl RecoveryKey {
    pub fn generate() -> RecoveryKey {
        let mut bytes = Zeroizing::new([0u8; 32]);
        seal::random(&mut bytes[..]);
        RecoveryKey(bytes)
    }

    fn checksum(bytes: &[u8; 32]) -> [u8; CHECKSUM] {
        let digest = Sha256::new().chain_update(b"amethystora-vault:recovery-check").chain_update(bytes).finalize();
        [digest[0], digest[1]]
    }

    /// As it is shown and written down: eleven groups of five
    pub fn text(&self) -> Zeroizing<String> {
        let mut all = Zeroizing::new([0u8; 32 + CHECKSUM]);
        all[..32].copy_from_slice(&self.0[..]);
        all[32..].copy_from_slice(&Self::checksum(&self.0));
        let mut text = Zeroizing::new(String::with_capacity(CHARACTERS + 10));
        let (mut buffer, mut bits, mut count) = (0u32, 0u32, 0usize);
        let mut push = |value: u32, text: &mut String| {
            if count > 0 && count.is_multiple_of(5) {
                text.push('-');
            }
            text.push(ALPHABET[(value & 31) as usize] as char);
            count += 1;
        };
        for byte in all.iter() {
            buffer = (buffer << 8) | u32::from(*byte);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                push(buffer >> bits, &mut text);
            }
            buffer &= (1 << bits) - 1;
        }
        push(buffer << (5 - bits), &mut text);
        text
    }

    /// As it is typed: in either case, with or without the dashes and spaces, and with the letters that read as others
    /// taken for them. None for anything else, a wrong checksum included.
    pub fn parse(text: &str) -> Option<RecoveryKey> {
        if text.len() > 256 {
            return None;
        }
        let mut all = Zeroizing::new([0u8; 32 + CHECKSUM]);
        let (mut buffer, mut bits, mut count, mut written) = (0u32, 0u32, 0usize, 0usize);
        for character in text.bytes() {
            let value = match character.to_ascii_uppercase() {
                b'-' | b' ' | b'\t' | b'\r' | b'\n' => continue,
                b'O' => 0,
                b'I' | b'L' => 1,
                upper => ALPHABET.iter().position(|letter| *letter == upper)? as u32,
            };
            count += 1;
            if count > CHARACTERS {
                return None;
            }
            buffer = (buffer << 5) | value;
            bits += 5;
            if bits >= 8 {
                bits -= 8;
                all[written] = (buffer >> bits) as u8;
                written += 1;
                buffer &= (1 << bits) - 1;
            }
        }
        // 272 bits in 55 characters leaves three, which are zero
        if count != CHARACTERS || written != all.len() || buffer != 0 {
            return None;
        }
        let bytes: [u8; 32] = all[..32].try_into().ok()?;
        if !seal::same(&Self::checksum(&bytes), &all[32..]) {
            return None;
        }
        Some(RecoveryKey(Zeroizing::new(bytes)))
    }

    /// The key its record is sealed under. 256 random bits need no stretching.
    pub fn key(&self) -> Key {
        let digest: [u8; 32] = Sha256::new().chain_update(b"amethystora-vault:recovery").chain_update(&self.0[..]).finalize().into();
        Key::from_bytes(&digest)
    }
}

/// What seals a slot to this machine: the TPM. A trait, so that tests run without one.
pub trait Chip: Send + Sync {
    fn present(&self) -> bool;
    fn seal(&self, name: &str, plain: &[u8]) -> Result<Vec<u8>, Unopened>;
    fn open(&self, name: &str, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, Unopened>;
}

/// The TPM through `systemd-creds --user`, its secret on standard input and output and never on a command line. As a user that
/// is not root it asks root's `systemd-creds@.service` over varlink, which refuses `--with-key=tpm2` and picks host key and TPM
/// by itself in this scope, with no PCR policy, since an update would break one (the spike, 2026-11-02). So a slot is bound to
/// the chip, the machine's own secret and the service's user. It takes half a second: off the loop.
pub struct SystemdCreds;

const CREDS: &str = "/usr/bin/systemd-creds";
const CREDS_PATIENCE: Duration = Duration::from_secs(30);

impl SystemdCreds {
    fn run(args: &[&str], input: &[u8]) -> Result<Zeroizing<Vec<u8>>, Unopened> {
        let mut child = Command::new(CREDS)
            .args(args)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Unopened)?;
        // What goes in and comes out is a few kilobytes at most, less than a pipe holds
        let mut stdin = child.stdin.take().ok_or(Unopened)?;
        let written = stdin.write_all(input);
        drop(stdin);
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() < CREDS_PATIENCE => std::thread::sleep(Duration::from_millis(20)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(Unopened);
                }
            }
        };
        let mut out = Zeroizing::new(Vec::new());
        child.stdout.take().ok_or(Unopened)?.read_to_end(&mut out).map_err(|_| Unopened)?;
        if written.is_err() || !status.success() {
            return Err(Unopened);
        }
        Ok(out)
    }
}

impl Chip for SystemdCreds {
    fn present(&self) -> bool {
        // From sysfs: the service has no devices of its own (PrivateDevices), and systemd-creds reaches the chip through systemd
        std::fs::read_dir("/sys/class/tpmrm").is_ok_and(|mut chips| chips.next().is_some())
    }

    fn seal(&self, name: &str, plain: &[u8]) -> Result<Vec<u8>, Unopened> {
        Self::run(&["--user", "encrypt", &format!("--name={name}"), "-", "-"], plain).map(|out| out.to_vec())
    }

    fn open(&self, name: &str, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, Unopened> {
        Self::run(&["--user", "decrypt", &format!("--name={name}"), "-", "-"], sealed)
    }
}

/// A slot that is not a password's or a recovery key's, kept beside a listed vault. A hidden vault has none: each would
/// need the service to know which vault to offer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slot {
    pub id: String,
    pub kind: SlotKind,
    /// What the person calls it, for a security key
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// A security key's credential and salt, which the client needs to ask it for the secret
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub credential: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub salt: String,
    /// False when there was no TPM: only the disk's encryption protects it at rest, which the window says
    #[serde(default)]
    pub chip: bool,
    /// The vault key, sealed, in base64
    pub sealed: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SlotKind {
    SecurityKey,
    /// Fingerprint or account password: polkit approves, and the slot is the chip's
    Approval,
    /// Nothing at all: it stops a stealer of files, and not a program of the account
    NoPrompt,
}

impl SlotKind {
    pub fn name(self) -> &'static str {
        match self {
            SlotKind::SecurityKey => "security-key",
            SlotKind::Approval => "approval",
            SlotKind::NoPrompt => "no-prompt",
        }
    }
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn unb64(text: &str) -> Result<Zeroizing<Vec<u8>>, Unopened> {
    base64::engine::general_purpose::STANDARD.decode(text).map(Zeroizing::new).map_err(|_| Unopened)
}

impl Slot {
    /// A security key's slot: the vault key sealed under the secret the key computes, which leaves the client only to
    /// make or open this
    pub fn for_security_key(name: &str, credential: &str, salt: &str, secret: &[u8], key: &Key) -> Result<Slot, Unopened> {
        let secret = Key::from_slice(secret).ok_or(Unopened)?;
        let id = seal::random_hex(16);
        let sealed = b64(&seal::seal_item(&secret, "slot", &id, key.bytes()));
        Ok(Slot { id, kind: SlotKind::SecurityKey, name: name.to_string(), credential: credential.to_string(), salt: salt.to_string(), chip: false, sealed })
    }

    pub fn open_with_secret(&self, secret: &[u8]) -> Result<Key, Unopened> {
        let secret = Key::from_slice(secret).ok_or(Unopened)?;
        let plain = seal::open_item(&secret, "slot", &self.id, &unb64(&self.sealed)?)?;
        Key::from_slice(&plain).ok_or(Unopened)
    }

    /// A slot on the chip. Without one, an approval slot holds the key as it is, which the window says first; no prompt
    /// is not offered then.
    pub fn for_chip(kind: SlotKind, chip: &dyn Chip, key: &Key) -> Result<Slot, Unopened> {
        let id = seal::random_hex(16);
        let empty = Slot { id: id.clone(), kind, name: String::new(), credential: String::new(), salt: String::new(), chip: false, sealed: String::new() };
        if chip.present() {
            let sealed = chip.seal(&chip_name(&id), key.bytes())?;
            Ok(Slot { chip: true, sealed: b64(&sealed), ..empty })
        } else if kind == SlotKind::Approval {
            Ok(Slot { sealed: b64(key.bytes()), ..empty })
        } else {
            Err(Unopened)
        }
    }

    /// Off the loop when `self.chip`: the TPM takes its time
    pub fn open_on_chip(&self, chip: &dyn Chip) -> Result<Key, Unopened> {
        let sealed = unb64(&self.sealed)?;
        if !self.chip {
            return Key::from_slice(&sealed).ok_or(Unopened);
        }
        let plain = chip.open(&chip_name(&self.id), &sealed)?;
        Key::from_slice(&plain).ok_or(Unopened)
    }
}

/// The credential's name, which systemd-creds authenticates with it
pub fn chip_name(id: &str) -> String {
    format!("amethystora-vault-{id}")
}

/// A chip that seals under a key of its own, for tests here and in the service
pub mod fake {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    pub struct FakeChip {
        pub here: AtomicBool,
        key: Key,
    }

    impl FakeChip {
        pub fn new() -> FakeChip {
            FakeChip { here: AtomicBool::new(true), key: Key::random() }
        }
    }

    impl Default for FakeChip {
        fn default() -> FakeChip {
            FakeChip::new()
        }
    }

    impl Chip for FakeChip {
        fn present(&self) -> bool {
            self.here.load(Ordering::Relaxed)
        }

        fn seal(&self, name: &str, plain: &[u8]) -> Result<Vec<u8>, Unopened> {
            Ok(seal::seal_item(&self.key, "chip", name, plain))
        }

        fn open(&self, name: &str, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, Unopened> {
            seal::open_item(&self.key, "chip", name, sealed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeChip;
    use super::*;

    fn test_kdf() -> Kdf {
        Kdf::fresh(10)
    }

    #[test]
    fn a_new_password_has_to_be_usable() {
        assert!(usable(b"long enough"));
        assert!(usable("éééééééé".as_bytes()));
        assert!(!usable(b""));
        assert!(!usable(b"short"));
        assert!(!usable(&[0xff; 16]));
        assert!(!usable(&vec![b'a'; MAX_PASSWORD + 1]));
    }

    #[test]
    fn a_letter_typed_either_way_gives_one_key() {
        let kdf = test_kdf();
        assert_eq!(derive("pa\u{301}ss".as_bytes(), &kdf).unwrap().bytes(), derive("p\u{e1}ss".as_bytes(), &kdf).unwrap().bytes());
        assert_ne!(derive(b"pass", &kdf).unwrap().bytes(), derive(b"pasS", &kdf).unwrap().bytes());
    }

    #[test]
    fn derives_as_scrypt_does() {
        // crypto.scryptSync('páss', Buffer.alloc(16, 3), 32, { N: 2 ** 14, r: 8, p: 1 }), as Notes' test has it
        let salt = base64::engine::general_purpose::STANDARD.encode([3u8; 16]);
        let kdf = Kdf { log_n: 14, r: 8, p: 1, salt };
        assert_eq!(seal::hex(derive("pa\u{301}ss".as_bytes(), &kdf).unwrap().bytes()), "f23a2d0c3d4f165a21982114b7dccaf873f4a05c149c309dbd4743994d44bec6");
    }

    #[test]
    fn bad_settings_and_bad_passwords_are_refused() {
        let good = test_kdf();
        assert!(good.is_sound());
        assert!(Kdf::fresh(LOG_N).is_sound());
        for bad in [
            Kdf { log_n: 21, ..good.clone() },
            Kdf { log_n: 9, ..good.clone() },
            Kdf { r: 17, ..good.clone() },
            Kdf { p: 5, ..good.clone() },
            Kdf { salt: "AAAA".into(), ..good.clone() },
            Kdf { salt: "not base64!".into(), ..good.clone() },
        ] {
            assert!(!bad.is_sound(), "{bad:?}");
            assert!(derive(b"x", &bad).is_err());
        }
        assert!(derive(b"", &good).is_err());
        assert!(derive(&[0xff, 0xfe], &good).is_err());
        assert!(derive(&vec![b'a'; MAX_PASSWORD + 1], &good).is_err());
    }

    #[test]
    fn a_record_opens_only_under_its_key_and_table() {
        let (under, key) = (Key::random(), Key::random());
        let id = seal::random_hex(16);
        let record = seal_record(&under, &id, &key, Table::Password).unwrap();
        let found = search(&under, &[record], Table::Password);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].vault, id);
        assert_eq!(found[0].key.bytes(), key.bytes());
        assert_eq!(found[0].record, seal::hex(&record[..12]));
        assert!(search(&Key::random(), &[record], Table::Password).is_empty());
        assert!(search(&under, &[record], Table::Recovery).is_empty());
        assert!(seal_record(&under, "not hex", &key, Table::Password).is_none());
    }

    #[test]
    fn the_stand_in_opens_nothing_under_any_key() {
        assert!(search(&Key::from_bytes(&[0; 32]), &[], Table::Password).is_empty());
    }

    #[test]
    fn many_records_are_all_tried() {
        let under = Key::random();
        let mut records = Vec::new();
        for _ in 0..MAX_VAULTS - 1 {
            records.push(seal_record(&Key::random(), &seal::random_hex(16), &Key::random(), Table::Password).unwrap());
        }
        let id = seal::random_hex(16);
        records.push(seal_record(&under, &id, &Key::random(), Table::Password).unwrap());
        let found = search(&under, &records, Table::Password);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].vault, id);
    }

    #[test]
    fn trying_costs_the_same_however_many_records_there_are() {
        let under = Key::random();
        let record = |_| seal_record(&Key::random(), &seal::random_hex(16), &Key::random(), Table::Password).unwrap();
        let full: Vec<Record> = (0..MAX_VAULTS).map(record).collect();
        let least = |records: &[Record]| {
            (0..300)
                .map(|_| {
                    let started = Instant::now();
                    std::hint::black_box(search(&under, records, Table::Password));
                    started.elapsed()
                })
                .min()
                .unwrap()
        };
        let (none, all) = (least(&[]), least(&full));
        println!("none: {none:?}, {MAX_VAULTS}: {all:?}");
        assert!(all.as_secs_f64() < none.as_secs_f64() * 1.5 && none.as_secs_f64() < all.as_secs_f64() * 1.5, "{none:?} {all:?}");
    }

    #[test]
    fn a_recovery_key_reads_back_however_it_is_typed() {
        let key = RecoveryKey::generate();
        let text = key.text();
        assert_eq!(text.len(), 55 + 10);
        assert_eq!(text.split('-').count(), 11);
        assert!(text.split('-').all(|group| group.len() == 5));
        let parsed = RecoveryKey::parse(&text).unwrap();
        assert_eq!(parsed.key().bytes(), key.key().bytes());
        let lower = text.to_lowercase();
        assert_eq!(RecoveryKey::parse(&lower).unwrap().key().bytes(), key.key().bytes());
        let spaced = text.replace('-', " ");
        assert_eq!(RecoveryKey::parse(&spaced).unwrap().key().bytes(), key.key().bytes());
        let run_on = text.replace('-', "");
        assert_eq!(RecoveryKey::parse(&run_on).unwrap().key().bytes(), key.key().bytes());
        let misread = text.replace('0', "O").replace('1', "l");
        assert_eq!(RecoveryKey::parse(&misread).unwrap().key().bytes(), key.key().bytes());
    }

    #[test]
    fn a_mistyped_recovery_key_is_refused() {
        let text = RecoveryKey::generate().text();
        let mut refused = 0;
        let letters: Vec<usize> = (0..text.len()).filter(|at| text.as_bytes()[*at] != b'-').collect();
        for at in &letters {
            let mut changed = text.as_bytes().to_vec();
            changed[*at] = if changed[*at] == b'2' { b'3' } else { b'2' };
            if RecoveryKey::parse(std::str::from_utf8(&changed).unwrap()).is_none() {
                refused += 1;
            }
        }
        // Sixteen bits: one chance in 65536 for each position that one passes
        assert!(refused + 2 >= letters.len(), "{refused} of {}", letters.len());
        assert!(RecoveryKey::parse("").is_none());
        assert!(RecoveryKey::parse("too short").is_none());
        assert!(RecoveryKey::parse(&"A".repeat(5000)).is_none());
        assert!(RecoveryKey::parse(&format!("{}A", *text)).is_none());
        assert!(RecoveryKey::parse("é").is_none());
    }

    #[test]
    fn every_slot_opens_the_same_key() {
        let key = Key::random();
        let chip = FakeChip::new();
        let secret = [9u8; 32];
        let security = Slot::for_security_key("Blue key", "cred", "salt", &secret, &key).unwrap();
        assert_eq!(security.open_with_secret(&secret).unwrap().bytes(), key.bytes());
        assert!(security.open_with_secret(&[8u8; 32]).is_err());
        assert!(security.open_with_secret(&[9u8; 31]).is_err());
        let approval = Slot::for_chip(SlotKind::Approval, &chip, &key).unwrap();
        assert!(approval.chip);
        assert_eq!(approval.open_on_chip(&chip).unwrap().bytes(), key.bytes());
        let prompt = Slot::for_chip(SlotKind::NoPrompt, &chip, &key).unwrap();
        assert_eq!(prompt.open_on_chip(&chip).unwrap().bytes(), key.bytes());
        // Another slot's sealed value does not pass for this one
        let swapped = Slot { sealed: prompt.sealed.clone(), ..approval.clone() };
        assert!(swapped.open_on_chip(&chip).is_err());
    }

    #[test]
    fn without_a_chip_approval_says_so_and_no_prompt_is_not_offered() {
        let (key, chip) = (Key::random(), FakeChip::new());
        chip.here.store(false, std::sync::atomic::Ordering::Relaxed);
        let approval = Slot::for_chip(SlotKind::Approval, &chip, &key).unwrap();
        assert!(!approval.chip);
        assert_eq!(approval.open_on_chip(&chip).unwrap().bytes(), key.bytes());
        assert!(Slot::for_chip(SlotKind::NoPrompt, &chip, &key).is_err());
    }

    #[test]
    fn a_slot_reads_back_from_its_file_form() {
        let slot = Slot::for_security_key("k", "cred", "salt", &[1; 32], &Key::random()).unwrap();
        let text = serde_json::to_string(&slot).unwrap();
        assert!(text.contains("\"security-key\""));
        assert_eq!(serde_json::from_str::<Slot>(&text).unwrap(), slot);
    }
}
