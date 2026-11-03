//! Sealing: AES-256-GCM, as Notes does it (`apps/notes/src/vault.rs`, which says why this cipher), in two forms.
//!
//! An item of the vault is `AVv1`, a fresh 96-bit nonce, the 128-bit tag, then the ciphertext, under the vault key, with
//! `amethystora-vault:<kind>:<id>` authenticated, so that a file cannot pass for another. What a feature gets back from
//! `seal` (`apps/vault-client`) is the same with `AVs1`, under that purpose's own key, with
//! `amethystora-vault:data:<vault>:<purpose>:<record>` authenticated, so that it cannot pass for another record, another
//! purpose's or another vault's, and so not another account's.

use std::fmt;
use std::ptr::NonNull;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use zeroize::{Zeroize, Zeroizing};

pub const ITEM_MAGIC: &[u8; 4] = b"AVv1";
pub const DATA_MAGIC: &[u8; 4] = b"AVs1";
/// The magic, the nonce and the tag
pub const OVERHEAD: usize = 32;

/// What did not open: a wrong key, a label that is not its own, or a changed file. Which of them is never said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unopened;

/// Random bytes from the kernel
pub fn random(out: &mut [u8]) {
    getrandom::fill(out).expect("the kernel gives random bytes");
}

/// `count` random bytes in lowercase hex: what ids are made of
pub fn random_hex(count: usize) -> String {
    let mut bytes = vec![0u8; count];
    random(&mut bytes);
    hex(&bytes)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok()).collect()
}

/// Compares two secrets without saying where they first differ
pub fn same(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let difference = a.iter().zip(b).fold(0u8, |sum, (x, y)| sum | (x ^ y));
    std::hint::black_box(difference) == 0
}

/// A 256-bit key, on a page of its own: locked out of swap, left out of core files and of a forked child, and wiped and
/// given back when it is dropped. A page each, since unlocking a page unlocks everything on it, and a service holds a
/// handful at most.
pub struct Key {
    page: NonNull<[u8; 32]>,
}

// SAFETY: the page belongs to this value alone, and is only written when it is made and dropped
unsafe impl Send for Key {}
// SAFETY: as above: shared references only read it
unsafe impl Sync for Key {}

const PAGE: usize = 4096;

impl Key {
    fn empty() -> Key {
        // SAFETY: a new anonymous mapping, which nothing else knows about
        unsafe {
            let page = libc::mmap(std::ptr::null_mut(), PAGE, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS, -1, 0);
            if page == libc::MAP_FAILED {
                // Out of memory, where a key has nowhere safe to go
                std::process::abort();
            }
            // A failure of any of these costs a protection and no secret: RLIMIT_MEMLOCK, in a small container
            libc::mlock(page, PAGE);
            libc::madvise(page, PAGE, libc::MADV_DONTDUMP);
            libc::madvise(page, PAGE, libc::MADV_WIPEONFORK);
            Key { page: NonNull::new_unchecked(page.cast()) }
        }
    }

    pub fn random() -> Key {
        let mut key = Key::empty();
        random(key.bytes_mut());
        key
    }

    pub fn from_bytes(bytes: &[u8; 32]) -> Key {
        let mut key = Key::empty();
        key.bytes_mut().copy_from_slice(bytes);
        key
    }

    /// A key of exactly 32 bytes
    pub fn from_slice(bytes: &[u8]) -> Option<Key> {
        Some(Key::from_bytes(bytes.try_into().ok()?))
    }

    pub fn bytes(&self) -> &[u8; 32] {
        // SAFETY: the page lives as long as self, and is 32 bytes and more
        unsafe { self.page.as_ref() }
    }

    fn bytes_mut(&mut self) -> &mut [u8; 32] {
        // SAFETY: as above, and self is held exclusively
        unsafe { self.page.as_mut() }
    }

    pub fn copy(&self) -> Key {
        Key::from_bytes(self.bytes())
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        self.bytes_mut().zeroize();
        // SAFETY: the mapping made in empty(), which nothing refers to once self is gone
        unsafe {
            libc::munlock(self.page.as_ptr().cast(), PAGE);
            libc::munmap(self.page.as_ptr().cast(), PAGE);
        }
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(…)")
    }
}

/// `magic`, the nonce, the tag, then the ciphertext
fn seal_with(magic: &[u8; 4], key: &Key, nonce: &[u8; 12], plain: &[u8], label: &str) -> Vec<u8> {
    let cipher = Aes256Gcm::new_from_slice(key.bytes()).expect("a 256-bit key");
    let iv = Nonce::try_from(&nonce[..]).expect("a 96-bit nonce");
    let sealed = cipher.encrypt(&iv, Payload { msg: plain, aad: label.as_bytes() }).expect("AES-GCM seals anything that fits in memory");
    // The crate puts the tag after the ciphertext, and the file before it
    let (body, tag) = sealed.split_at(sealed.len() - 16);
    let mut out = Vec::with_capacity(OVERHEAD + body.len());
    out.extend_from_slice(magic);
    out.extend_from_slice(nonce);
    out.extend_from_slice(tag);
    out.extend_from_slice(body);
    out
}

fn seal_as(magic: &[u8; 4], key: &Key, plain: &[u8], label: &str) -> Vec<u8> {
    let mut nonce = [0u8; 12];
    random(&mut nonce);
    seal_with(magic, key, &nonce, plain, label)
}

fn open_as(magic: &[u8; 4], key: &Key, bytes: &[u8], label: &str) -> Result<Zeroizing<Vec<u8>>, Unopened> {
    if bytes.len() < OVERHEAD || &bytes[..4] != magic {
        return Err(Unopened);
    }
    let cipher = Aes256Gcm::new_from_slice(key.bytes()).map_err(|_| Unopened)?;
    let iv = Nonce::try_from(&bytes[4..16]).map_err(|_| Unopened)?;
    let mut joined = Vec::with_capacity(bytes.len() - 16);
    joined.extend_from_slice(&bytes[OVERHEAD..]);
    joined.extend_from_slice(&bytes[16..OVERHEAD]);
    cipher.decrypt(&iv, Payload { msg: &joined, aad: label.as_bytes() }).map(Zeroizing::new).map_err(|_| Unopened)
}

fn item_label(kind: &str, id: &str) -> String {
    format!("amethystora-vault:{kind}:{id}")
}

/// An item under the vault key. `kind` and `id` have been checked for their shape (`protocol`).
pub fn seal_item(key: &Key, kind: &str, id: &str, plain: &[u8]) -> Vec<u8> {
    seal_as(ITEM_MAGIC, key, plain, &item_label(kind, id))
}

pub fn open_item(key: &Key, kind: &str, id: &str, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, Unopened> {
    open_as(ITEM_MAGIC, key, bytes, &item_label(kind, id))
}

// A purpose has no colon and a vault's id is hex, so the record, last, cannot run into either
fn data_label(vault: &str, purpose: &str, record: &str) -> String {
    format!("amethystora-vault:data:{vault}:{purpose}:{record}")
}

/// A feature's data under its purpose's key
pub fn seal_data(key: &Key, vault: &str, purpose: &str, record: &str, plain: &[u8]) -> Vec<u8> {
    seal_as(DATA_MAGIC, key, plain, &data_label(vault, purpose, record))
}

pub fn open_data(key: &Key, vault: &str, purpose: &str, record: &str, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, Unopened> {
    open_as(DATA_MAGIC, key, bytes, &data_label(vault, purpose, record))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_opens_under_its_own_name_only() {
        let key = Key::random();
        let sealed = seal_item(&key, "password", "a1", b"hello");
        assert_eq!(&sealed[..4], b"AVv1");
        assert_eq!(sealed.len(), OVERHEAD + 5);
        assert_eq!(open_item(&key, "password", "a1", &sealed).unwrap().as_slice(), b"hello");
        assert_eq!(open_item(&key, "password", "a2", &sealed), Err(Unopened));
        assert_eq!(open_item(&key, "codes", "a1", &sealed), Err(Unopened));
        assert_eq!(open_item(&Key::random(), "password", "a1", &sealed), Err(Unopened));
    }

    #[test]
    fn every_byte_of_a_file_counts() {
        let key = Key::random();
        let sealed = seal_item(&key, "k", "i", b"plain");
        for at in 0..sealed.len() {
            let mut changed = sealed.clone();
            changed[at] ^= 0x40;
            assert_eq!(open_item(&key, "k", "i", &changed), Err(Unopened), "byte {at}");
        }
        for length in 0..sealed.len() {
            assert_eq!(open_item(&key, "k", "i", &sealed[..length]), Err(Unopened), "{length} bytes");
        }
    }

    #[test]
    fn data_opens_under_its_vault_purpose_and_record_only() {
        let key = Key::random();
        let sealed = seal_data(&key, "aa", "resume", "2026", b"hello");
        assert_eq!(&sealed[..4], b"AVs1");
        assert_eq!(open_data(&key, "aa", "resume", "2026", &sealed).unwrap().as_slice(), b"hello");
        assert_eq!(open_data(&key, "bb", "resume", "2026", &sealed), Err(Unopened));
        assert_eq!(open_data(&key, "aa", "digest", "2026", &sealed), Err(Unopened));
        assert_eq!(open_data(&key, "aa", "resume", "2027", &sealed), Err(Unopened));
        // The two forms are not each other's, whatever the labels
        let mut other = sealed.clone();
        other[..4].copy_from_slice(ITEM_MAGIC);
        assert_eq!(open_data(&key, "aa", "resume", "2026", &other), Err(Unopened));
    }

    #[test]
    fn a_record_can_hold_a_colon_without_meeting_another() {
        let key = Key::random();
        let sealed = seal_data(&key, "aa", "resume", "b:c", b"x");
        assert_eq!(open_data(&key, "aa", "resume", "b", &sealed), Err(Unopened));
        assert_eq!(open_data(&key, "aa", "resume", "b:c", &sealed).unwrap().as_slice(), b"x");
    }

    #[test]
    fn matches_the_cipher_for_a_known_nonce() {
        // Notes' test of its own sealing, under the same key, nonce and label: Node's createCipheriv('aes-256-gcm',
        // Buffer.alloc(32, 1), Buffer.alloc(12, 2)) with the AAD 'amethystora-notes:data', on 'hello', printed the tag
        // c964f997bf588859d760f0689fc0ddfa and the ciphertext 6fb3a52525
        let sealed = seal_with(ITEM_MAGIC, &Key::from_bytes(&[1; 32]), &[2; 12], b"hello", "amethystora-notes:data");
        assert_eq!(hex(&sealed), "41567631020202020202020202020202c964f997bf588859d760f0689fc0ddfa6fb3a52525");
    }

    #[test]
    fn nonces_are_fresh() {
        let key = Key::random();
        assert_ne!(seal_item(&key, "k", "i", b"x")[4..16], seal_item(&key, "k", "i", b"x")[4..16]);
    }

    #[test]
    fn a_key_says_nothing_of_itself() {
        assert_eq!(format!("{:?}", Key::from_bytes(&[7; 32])), "Key(…)");
        let key = Key::from_bytes(&[7; 32]);
        assert_eq!(key.copy().bytes(), key.bytes());
        assert!(Key::from_slice(&[1; 31]).is_none());
        assert_ne!(Key::random().bytes(), Key::random().bytes());
    }

    #[test]
    fn secrets_compare_whole() {
        assert!(same(b"abc", b"abc"));
        assert!(!same(b"abc", b"abd"));
        assert!(!same(b"abc", b"abcd"));
        assert_eq!(unhex(&hex(&[0, 255, 16])), Some(vec![0, 255, 16]));
        assert_eq!(unhex("0g"), None);
        assert_eq!(unhex("ABCD"), None);
        assert_eq!(unhex("abc"), None);
    }
}
