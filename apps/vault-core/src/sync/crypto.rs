//! The account's cryptography (spec 2 and 4): the KDFs within bounds, the stretch, the master password hash, the 64-byte
//! keys and type 2 EncStrings. Nothing else: no RSA, no type 0, no key is ever made for an account here.

use std::fmt;

use aes::cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use aes::Aes256;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

use super::bounds;

type HmacSha256 = Hmac<Sha256>;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// A symmetric key of the protocol: the first 32 bytes encrypt, the last 32 authenticate. Always exactly 64 bytes.
#[derive(Clone)]
pub struct SymKey(Zeroizing<[u8; 64]>);

impl SymKey {
    /// Exactly 64 bytes, or none: 32 is the old unauthenticated form, more is the newer format (spec 4.3)
    pub fn from_slice(bytes: &[u8]) -> Option<SymKey> {
        let bytes: &[u8; 64] = bytes.try_into().ok()?;
        Some(SymKey(Zeroizing::new(*bytes)))
    }

    pub fn random() -> SymKey {
        let mut key = Zeroizing::new([0u8; 64]);
        crate::seal::random(&mut key[..]);
        SymKey(key)
    }

    pub fn bytes(&self) -> &[u8; 64] {
        &self.0
    }

    fn enc(&self) -> &[u8] {
        &self.0[..32]
    }

    fn mac(&self) -> &[u8] {
        &self.0[32..]
    }
}

impl fmt::Debug for SymKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[key]")
    }
}

// --- KDF settings ---------------------------------------------------------------------------------------------------

/// The account's KDF as the server describes it: `kind` 0 PBKDF2-SHA256, 1 Argon2id; memory in MiB
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kdf {
    pub kind: u8,
    pub iterations: u32,
    #[serde(default)]
    pub memory: Option<u32>,
    #[serde(default)]
    pub parallelism: Option<u32>,
}

/// Why settings are not computed
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KdfProblem {
    /// A KDF this client does not know
    Unknown,
    /// Less work than the floor: cheap to attack offline
    TooWeak,
    /// More than a machine should spend: a server could use up its memory or an hour of its processor
    TooStrong,
    /// Less than the last login used, or another KDF: a server lowering the work (BW07)
    BelowPin,
}

impl KdfProblem {
    pub fn name(self) -> &'static str {
        match self {
            KdfProblem::Unknown => "kdf-unknown",
            KdfProblem::TooWeak => "kdf-too-weak",
            KdfProblem::TooStrong => "kdf-too-strong",
            KdfProblem::BelowPin => "kdf-lowered",
        }
    }
}

fn within(value: Option<u32>, (low, high): (u32, u32)) -> Result<u32, KdfProblem> {
    match value {
        Some(value) if value < low => Err(KdfProblem::TooWeak),
        Some(value) if value > high => Err(KdfProblem::TooStrong),
        Some(value) => Ok(value),
        None => Err(KdfProblem::Unknown),
    }
}

impl Kdf {
    /// Within the bounds of spec 2.1. Ok(true) when the person must confirm first (PBKDF2 between the floor and 600,000).
    pub fn check(&self) -> Result<bool, KdfProblem> {
        match self.kind {
            0 => {
                if self.memory.is_some_and(|m| m != 0) || self.parallelism.is_some_and(|p| p != 0) {
                    return Err(KdfProblem::Unknown);
                }
                match self.iterations {
                    n if n < bounds::PBKDF2_FLOOR => Err(KdfProblem::TooWeak),
                    n if n > bounds::PBKDF2_MOST => Err(KdfProblem::TooStrong),
                    n => Ok(n < bounds::PBKDF2_CONFIRM_BELOW),
                }
            }
            1 => {
                within(Some(self.iterations), bounds::ARGON2_PASSES)?;
                within(self.memory, bounds::ARGON2_MIB)?;
                within(self.parallelism, bounds::ARGON2_LANES)?;
                Ok(false)
            }
            _ => Err(KdfProblem::Unknown),
        }
    }

    /// Asks for less work than `pinned` in any parameter, or is another KDF. A raise is taken.
    pub fn below(&self, pinned: &Kdf) -> bool {
        if self.kind != pinned.kind {
            return true;
        }
        let less = |a: Option<u32>, b: Option<u32>| a.unwrap_or(0) < b.unwrap_or(0);
        self.iterations < pinned.iterations || less(self.memory, pinned.memory) || less(self.parallelism, pinned.parallelism)
    }

    /// The same settings, with PBKDF2's absent memory and lanes taken as equal to zero ones
    pub fn same(&self, other: &Kdf) -> bool {
        let norm = |kdf: &Kdf| if kdf.kind == 0 { (0, kdf.iterations, 0, 0) } else { (kdf.kind, kdf.iterations, kdf.memory.unwrap_or(0), kdf.parallelism.unwrap_or(0)) };
        norm(self) == norm(other)
    }
}

/// The salt: the email trimmed of white space at both ends and lowercased (spec 2.1)
pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

/// A salt the server sends, if any, has to be ours after the same normalizing
pub fn salt_is_ours(server: Option<&str>, email: &str) -> bool {
    server.is_none_or(|salt| normalize_email(salt) == normalize_email(email))
}

/// The master key (spec 2.2). The settings are checked first, so nothing unbounded is ever computed.
pub fn master_key(password: &[u8], email: &str, kdf: &Kdf) -> Result<Zeroizing<[u8; 32]>, KdfProblem> {
    kdf.check()?;
    let salt = normalize_email(email);
    let mut out = Zeroizing::new([0u8; 32]);
    match kdf.kind {
        0 => pbkdf2::pbkdf2_hmac::<Sha256>(password, salt.as_bytes(), kdf.iterations, &mut out[..]),
        _ => {
            let memory = kdf.memory.unwrap_or(0) * 1024;
            let params = argon2::Params::new(memory, kdf.iterations, kdf.parallelism.unwrap_or(0), Some(32)).map_err(|_| KdfProblem::Unknown)?;
            let argon = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
            let salt = Sha256::digest(salt.as_bytes());
            argon.hash_password_into(password, &salt, &mut out[..]).map_err(|_| KdfProblem::Unknown)?;
        }
    }
    Ok(out)
}

fn hmac(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut mac = <HmacSha256 as hmac::KeyInit>::new_from_slice(key).expect("HMAC takes a key of any length");
    for part in parts {
        mac.update(part);
    }
    mac.finalize().into_bytes().into()
}

/// The stretched master key: HKDF-Expand alone, the master key as PRK, info `enc` and `mac`, 32 bytes each
pub fn stretch(master: &[u8; 32]) -> SymKey {
    let mut both = Zeroizing::new([0u8; 64]);
    both[..32].copy_from_slice(&hmac(master, &[b"enc", &[1]]));
    both[32..].copy_from_slice(&hmac(master, &[b"mac", &[1]]));
    SymKey(both)
}

/// The master password hash a password login sends: PBKDF2-SHA256 of the master key, salted with the password, one round
pub fn master_hash(master: &[u8; 32], password: &[u8]) -> Zeroizing<String> {
    let mut out = Zeroizing::new([0u8; 32]);
    pbkdf2::pbkdf2_hmac::<Sha256>(master, password, 1, &mut out[..]);
    Zeroizing::new(B64.encode(&out[..]))
}

/// The base64 of SHA-256 of a URI: what a `uriChecksum` holds once opened
pub fn uri_checksum(uri: &str) -> String {
    B64.encode(Sha256::digest(uri.as_bytes()))
}

// --- EncString ------------------------------------------------------------------------------------------------------

/// A string that did not read: whichever step it was, it is the same failure, and nothing of it goes back to the server
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unreadable;

/// The longest EncString read at all: Vaultwarden's largest notes, and a little more
pub const MOST_CHARS: usize = 100_100;

/// A type 2 EncString: AES-256-CBC with HMAC-SHA256 over the IV and the ciphertext
pub struct EncString {
    iv: [u8; 16],
    ct: Vec<u8>,
    mac: [u8; 32],
}

/// Whether a string is shaped like a type 2 EncString at all, without a key
pub fn is_type2(text: &str) -> bool {
    EncString::parse(text).is_ok()
}

impl EncString {
    /// Exactly `2.`, three pieces, standard base64, a 16-byte IV, a 32-byte MAC and a non-empty ciphertext of whole blocks
    pub fn parse(text: &str) -> Result<EncString, Unreadable> {
        if text.len() > MOST_CHARS {
            return Err(Unreadable);
        }
        let body = text.strip_prefix("2.").ok_or(Unreadable)?;
        let mut pieces = body.split('|');
        let (Some(iv), Some(ct), Some(mac), None) = (pieces.next(), pieces.next(), pieces.next(), pieces.next()) else {
            return Err(Unreadable);
        };
        let iv: [u8; 16] = B64.decode(iv).map_err(|_| Unreadable)?.try_into().map_err(|_| Unreadable)?;
        let mac: [u8; 32] = B64.decode(mac).map_err(|_| Unreadable)?.try_into().map_err(|_| Unreadable)?;
        let ct = B64.decode(ct).map_err(|_| Unreadable)?;
        if ct.is_empty() || !ct.len().is_multiple_of(16) {
            return Err(Unreadable);
        }
        Ok(EncString { iv, ct, mac })
    }

    /// The MAC first, in constant time, then AES-CBC, then the padding
    pub fn open(&self, key: &SymKey) -> Result<Zeroizing<Vec<u8>>, Unreadable> {
        let mut mac = <HmacSha256 as hmac::KeyInit>::new_from_slice(key.mac()).map_err(|_| Unreadable)?;
        mac.update(&self.iv);
        mac.update(&self.ct);
        mac.verify_slice(&self.mac).map_err(|_| Unreadable)?;
        let aes = Aes256::new_from_slice(key.enc()).map_err(|_| Unreadable)?;
        let mut plain = Zeroizing::new(self.ct.clone());
        let mut previous = self.iv;
        for block in plain.as_chunks_mut::<16>().0 {
            let this: [u8; 16] = *block;
            let mut work = aes::Block::from(this);
            aes.decrypt_block(&mut work);
            for (byte, (out, chain)) in block.iter_mut().zip(work.iter().zip(previous)) {
                *byte = out ^ chain;
            }
            previous = this;
            work.zeroize();
        }
        let pad = *plain.last().ok_or(Unreadable)? as usize;
        let length = plain.len();
        // PKCS#7, every padding byte checked. A server knows nothing of which step failed: the MAC passed already.
        if pad == 0 || pad > 16 || !plain[length - pad..].iter().all(|byte| *byte as usize == pad) {
            return Err(Unreadable);
        }
        plain.truncate(length - pad);
        Ok(plain)
    }

    pub fn open_text(&self, key: &SymKey) -> Result<Zeroizing<String>, Unreadable> {
        let bytes = self.open(key)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| Unreadable)?;
        Ok(Zeroizing::new(text.to_string()))
    }

    /// A fresh IV from the kernel for every string, never reused
    pub fn seal(key: &SymKey, plain: &[u8]) -> EncString {
        let mut iv = [0u8; 16];
        crate::seal::random(&mut iv);
        EncString::seal_with_iv(key, plain, iv)
    }

    fn seal_with_iv(key: &SymKey, plain: &[u8], iv: [u8; 16]) -> EncString {
        let aes = Aes256::new_from_slice(key.enc()).expect("a 32-byte key");
        let pad = 16 - plain.len() % 16;
        let mut ct = Zeroizing::new(Vec::with_capacity(plain.len() + pad));
        ct.extend_from_slice(plain);
        ct.extend(std::iter::repeat_n(pad as u8, pad));
        let mut previous = iv;
        for block in ct.as_chunks_mut::<16>().0 {
            for (byte, chain) in block.iter_mut().zip(previous) {
                *byte ^= chain;
            }
            let mut work = aes::Block::from(*block);
            aes.encrypt_block(&mut work);
            block.copy_from_slice(&work);
            previous = *block;
        }
        let mac = hmac(key.mac(), &[&iv, &ct]);
        EncString { iv, ct: ct.to_vec(), mac }
    }

    pub fn text(&self) -> String {
        format!("2.{}|{}|{}", B64.encode(self.iv), B64.encode(&self.ct), B64.encode(self.mac))
    }
}

/// A key wrapped as a type 2 EncString under another, opened: exactly 64 bytes, or refused (spec 4.3, 4.4)
pub fn open_key(wrapped: &str, under: &SymKey) -> Result<SymKey, Unreadable> {
    let plain = EncString::parse(wrapped)?.open(under)?;
    SymKey::from_slice(&plain).ok_or(Unreadable)
}

/// The plaintext length of a wrapped key, to tell the newer format (longer than 64) from a wrong key
pub fn wrapped_length(wrapped: &str, under: &SymKey) -> Option<usize> {
    EncString::parse(wrapped).ok()?.open(under).ok().map(|plain| plain.len())
}

pub fn seal_text(key: &SymKey, text: &str) -> String {
    EncString::seal(key, text.as_bytes()).text()
}

pub fn open_text(text: &str, key: &SymKey) -> Result<Zeroizing<String>, Unreadable> {
    EncString::parse(text)?.open_text(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(text: &str) -> Vec<u8> {
        crate::seal::unhex(text).unwrap()
    }

    const PASSWORD: &str = "correct horse battery staple é✓";
    const EMAIL: &str = "  Person.Example@Amethystora.TEST ";

    fn user_key() -> SymKey {
        SymKey::from_slice(&(0u8..64).collect::<Vec<_>>()).unwrap()
    }

    fn vector1_stretched() -> SymKey {
        let master: [u8; 32] = unhex("088585f32855255992919c03a92bb85e6e72a649eda55604d731da55b0d4015f").try_into().unwrap();
        stretch(&master)
    }

    #[test]
    fn the_password_and_email_are_as_the_vectors_say() {
        assert_eq!(hex(PASSWORD.as_bytes()), "636f727265637420686f727365206261747465727920737461706c6520c3a9e29c93");
        assert_eq!(normalize_email(EMAIL), "person.example@amethystora.test");
    }

    fn hex(bytes: &[u8]) -> String {
        crate::seal::hex(bytes)
    }

    #[test]
    fn vector_1_pbkdf2() {
        let kdf = Kdf { kind: 0, iterations: 600_000, memory: None, parallelism: None };
        let master = master_key(PASSWORD.as_bytes(), EMAIL, &kdf).unwrap();
        assert_eq!(hex(&master[..]), "088585f32855255992919c03a92bb85e6e72a649eda55604d731da55b0d4015f");
        assert_eq!(B64.encode(&master[..]), "CIWF8yhVJVmSkZwDqSu4Xm5ypkntpVYE1zHaVbDUAV8=");
        assert_eq!(master_hash(&master, PASSWORD.as_bytes()).as_str(), "okQXY0zc+5PTQyw9t5hA8/DOEkCcS6g5wjga53BUo4c=");
        let stretched = stretch(&master);
        assert_eq!(hex(stretched.enc()), "148e1f78d171e669370e6673c23cffd68a1d0a7557e4d9c0869230b78b859e2d");
        assert_eq!(hex(stretched.mac()), "1a0c2476968f64080c86c9e0fe24c9555b8259588d3c39dab9de9fb4003a9144");
    }

    #[test]
    fn vector_2_argon2id() {
        let kdf = Kdf { kind: 1, iterations: 3, memory: Some(64), parallelism: Some(4) };
        assert_eq!(hex(&Sha256::digest(normalize_email(EMAIL).as_bytes())), "763f1efb422057aa9d14b0c28c8e00de8a26bc083a61dd5bcc8553d28a961644");
        let master = master_key(PASSWORD.as_bytes(), EMAIL, &kdf).unwrap();
        assert_eq!(hex(&master[..]), "43b05326feac27c5dd9c364618daf2558dc7f27c84cee6fcc31949c7706ef6cd");
        assert_eq!(master_hash(&master, PASSWORD.as_bytes()).as_str(), "mJMQd//6SRwfcGFGdD9GwZ2/I5SrmHe9cTHzAxoVk9U=");
        let stretched = stretch(&master);
        assert_eq!(hex(stretched.enc()), "cfb9244d999248b1b5b381a91a3e64f314703edacaadf6dfe69f3f1b0b066427");
        assert_eq!(hex(stretched.mac()), "eec2c4c155b50c7359169a6021c3360717a5312789df296fd8e4c36468ca711d");
    }

    const VECTOR3: &str = "2.oKGio6SlpqeoqaqrrK2urw==|x0aUEHmhwY/5srCpXnAac90yp7puDnUZ702oADiY9gkTZKONsg7857LaHClYehfITqFRl+lL5z6YgsPFzB5TuryhPI+3McbYkZeqqvsAEPo=|Eft0auj9Ifs1MvhUezN9uhLdQFgTihg2xr7U4BT33Jk=";

    #[test]
    fn vector_3_the_protected_user_key() {
        let stretched = vector1_stretched();
        let opened = open_key(VECTOR3, &stretched).unwrap();
        assert_eq!(opened.bytes()[..], user_key().bytes()[..]);
        let sealed = EncString::seal_with_iv(&stretched, user_key().bytes(), unhex("a0a1a2a3a4a5a6a7a8a9aaabacadaeaf").try_into().unwrap());
        assert_eq!(sealed.text(), VECTOR3);
    }

    const VECTOR4: &str = "2.sLGys7S1tre4ubq7vL2+vw==|M3zqN2ch0xEveb6Pl2M8YA==|w7DHZ7Z4q7hM+vD0MpCx0F0efz4dSHRZ4efUfmD9XwY=";

    #[test]
    fn vector_4_a_field() {
        let sealed = EncString::seal_with_iv(&user_key(), "hunter2 ✓".as_bytes(), unhex("b0b1b2b3b4b5b6b7b8b9babbbcbdbebf").try_into().unwrap());
        assert_eq!(hex(&sealed.ct), "337cea376721d3112f79be8f97633c60");
        assert_eq!(hex(&sealed.mac), "c3b0c767b678abb84cfaf0f43290b1d05d1e7f3e1d487459e1e7d47e60fd5f06");
        assert_eq!(sealed.text(), VECTOR4);
        assert_eq!(sealed.text().len(), 96);
        assert_eq!(open_text(VECTOR4, &user_key()).unwrap().as_str(), "hunter2 ✓");
    }

    #[test]
    fn vector_5_an_item_key_and_a_login() {
        let item = SymKey::from_slice(&(0x40u8..0x80).collect::<Vec<_>>()).unwrap();
        let iv = |first: u8| -> [u8; 16] { std::array::from_fn(|i| first + i as u8) };
        let key = "2.wMHCw8TFxsfIycrLzM3Ozw==|msD84qQB+YKCn9unug7f8/eO05RNSAwl+rpdDP+d9TCpztnyas1mXenAwRfYaXPbrBB7nUI69hdSIYz5pCpfdghz0jAyvuYhRZrdFcE5rlk=|G2BUIyOgSwV6vAxX29kNbpofzXd7oNl+3fWBfMuenRg=";
        assert_eq!(EncString::seal_with_iv(&user_key(), item.bytes(), iv(0xc0)).text(), key);
        assert_eq!(open_key(key, &user_key()).unwrap().bytes()[..], item.bytes()[..]);
        let name = "2.0NHS09TV1tfY2drb3N3e3w==|KtekZK6/1gWUJ9nSiFXDXg==|Ngx+2vGvW8NNKWckL7ubfciUU7NfpoRx2r+bmROJCq8=";
        assert_eq!(EncString::seal_with_iv(&item, b"Example login", iv(0xd0)).text(), name);
        let uri = "2.4OHi4+Tl5ufo6err7O3u7w==|p++JkpMwrJe3yEuu99nxbssrh/t73XzYocfgr+0iyiI=|STPk7TbHiAdfdwbu2MnAv7J2WApL7BexlYDHUZbwJO4=";
        assert_eq!(EncString::seal_with_iv(&item, b"https://example.test/login", iv(0xe0)).text(), uri);
        assert_eq!(uri_checksum("https://example.test/login"), "c5BbtAfsH99Uli1wQKGKCWswbG4gBySJ6ko0l6bA22w=");
        let checksum = "2.8PHy8/T19vf4+fr7/P3+/w==|fi6x6S3qj0KqjXj+NBPUDs0ePZlVyTm8NYOFAqGecNIszJzHWtrK9ULEq4PAlKgb|oNHx1+vG7GHurPopRT7aO9kIIvXvOd9k1lzHBE55MTY=";
        assert_eq!(EncString::seal_with_iv(&item, uri_checksum("https://example.test/login").as_bytes(), iv(0xf0)).text(), checksum);
        assert_eq!(open_text(checksum, &item).unwrap().as_str(), "c5BbtAfsH99Uli1wQKGKCWswbG4gBySJ6ko0l6bA22w=");
    }

    #[test]
    fn vector_6_every_tampered_string_is_refused() {
        let key = user_key();
        for bad in [
            "2.sLGys7S1tre4ubq7vL2+vw==|M3zqN2ch0xEveb6Pl2M8YA==|w7DHZ7Z4q7hM+vD0MpCx0F0efz4dSHRZ4efUfmD9Xwc=",
            "2.sLGys7S1tre4ubq7vL2+vw==|M3zqN2ch0xEveb6Pl2M8YQ==|w7DHZ7Z4q7hM+vD0MpCx0F0efz4dSHRZ4efUfmD9XwY=",
            "2.sLGys7S1tre4ubq7vL2+vg==|M3zqN2ch0xEveb6Pl2M8YA==|w7DHZ7Z4q7hM+vD0MpCx0F0efz4dSHRZ4efUfmD9XwY=",
            "0.sLGys7S1tre4ubq7vL2+vw==|M3zqN2ch0xEveb6Pl2M8YA==",
            // A fourth piece, IVs of 15 and 17 bytes, a MAC of 31, an empty ciphertext, one of 17 bytes, no type, type 7
            "2.sLGys7S1tre4ubq7vL2+vw==|M3zqN2ch0xEveb6Pl2M8YA==|w7DHZ7Z4q7hM+vD0MpCx0F0efz4dSHRZ4efUfmD9XwY=|AA==",
            "2.sLGys7S1tre4ubq7vL2+|M3zqN2ch0xEveb6Pl2M8YA==|w7DHZ7Z4q7hM+vD0MpCx0F0efz4dSHRZ4efUfmD9XwY=",
            "2.sLGys7S1tre4ubq7vL2+v78A|M3zqN2ch0xEveb6Pl2M8YA==|w7DHZ7Z4q7hM+vD0MpCx0F0efz4dSHRZ4efUfmD9XwY=",
            "2.sLGys7S1tre4ubq7vL2+vw==|M3zqN2ch0xEveb6Pl2M8YA==|w7DHZ7Z4q7hM+vD0MpCx0F0efz4dSHRZ4efUfmD9Xw==",
            "2.sLGys7S1tre4ubq7vL2+vw==||w7DHZ7Z4q7hM+vD0MpCx0F0efz4dSHRZ4efUfmD9XwY=",
            "2.sLGys7S1tre4ubq7vL2+vw==|M3zqN2ch0xEveb6Pl2M8YAA=|w7DHZ7Z4q7hM+vD0MpCx0F0efz4dSHRZ4efUfmD9XwY=",
            "sLGys7S1tre4ubq7vL2+vw==|M3zqN2ch0xEveb6Pl2M8YA==|w7DHZ7Z4q7hM+vD0MpCx0F0efz4dSHRZ4efUfmD9XwY=",
            "7.0oRDQAAAWBgNQ==",
        ] {
            assert_eq!(open_text(bad, &key).err(), Some(Unreadable), "{bad}");
        }
        // Well formed, a MAC that verifies, and 32 bytes inside: never a user key (BW12)
        let short = "2.kJGSk5SVlpeYmZqbnJ2enw==|hD88Rc65KsK1Yn87VbU1+0AiuGqzC224dWy6A2sAYIYeDRcbgLbKCZ6ARy4+V6jL|HmQ95/+1ep3RwvAgRla8jiaUS3Fe/vVolMAcnwiJ1Ds=";
        assert_eq!(wrapped_length(short, &vector1_stretched()), Some(32));
        assert!(open_key(short, &vector1_stretched()).is_err());
    }

    #[test]
    fn vector_7_the_length_formula() {
        for (plain, chars) in [(11, 96), (687, 992), (3695, 5000), (7439, 9992), (74_943, 100_000)] {
            assert_eq!(crate::sync::lengths::encstring(plain), chars);
            assert_eq!(EncString::seal(&user_key(), &vec![b'x'; plain]).text().len(), chars);
        }
        assert!(crate::sync::lengths::encstring(688) > 1000);
    }

    #[test]
    fn the_bounds_and_the_pin() {
        let pbkdf2 = |n| Kdf { kind: 0, iterations: n, memory: None, parallelism: None };
        let argon = |t, m, p| Kdf { kind: 1, iterations: t, memory: Some(m), parallelism: Some(p) };
        assert_eq!(pbkdf2(99_999).check(), Err(KdfProblem::TooWeak));
        assert_eq!(pbkdf2(5000).check(), Err(KdfProblem::TooWeak));
        assert_eq!(pbkdf2(100_000).check(), Ok(true));
        assert_eq!(pbkdf2(599_999).check(), Ok(true));
        assert_eq!(pbkdf2(600_000).check(), Ok(false));
        assert_eq!(pbkdf2(2_000_000).check(), Ok(false));
        assert_eq!(pbkdf2(2_000_001).check(), Err(KdfProblem::TooStrong));
        assert_eq!(argon(1, 64, 4).check(), Err(KdfProblem::TooWeak));
        assert_eq!(argon(2, 15, 4).check(), Err(KdfProblem::TooWeak));
        assert_eq!(argon(2, 16, 1).check(), Ok(false));
        assert_eq!(argon(10, 1024, 16).check(), Ok(false));
        assert_eq!(argon(11, 64, 4).check(), Err(KdfProblem::TooStrong));
        assert_eq!(argon(3, 1025, 4).check(), Err(KdfProblem::TooStrong));
        assert_eq!(argon(3, 64, 17).check(), Err(KdfProblem::TooStrong));
        assert_eq!(argon(3, 64, 0).check(), Err(KdfProblem::TooWeak));
        assert_eq!(Kdf { kind: 1, iterations: 3, memory: None, parallelism: Some(4) }.check(), Err(KdfProblem::Unknown));
        assert_eq!(Kdf { kind: 2, iterations: 3, memory: None, parallelism: None }.check(), Err(KdfProblem::Unknown));
        // Nothing out of bounds is ever computed
        assert_eq!(master_key(b"x", "a@b", &pbkdf2(5000)).err(), Some(KdfProblem::TooWeak));
        // The pin: less in any parameter, or another KDF, is below; more is a raise
        assert!(pbkdf2(600_000).below(&pbkdf2(600_001)));
        assert!(!pbkdf2(700_000).below(&pbkdf2(600_000)));
        assert!(argon(3, 32, 4).below(&argon(3, 64, 4)));
        assert!(argon(3, 64, 2).below(&argon(3, 64, 4)));
        assert!(argon(2, 64, 4).below(&argon(3, 64, 4)));
        assert!(!argon(3, 128, 4).below(&argon(3, 64, 4)));
        assert!(pbkdf2(2_000_000).below(&argon(2, 16, 1)));
        assert!(argon(10, 1024, 16).below(&pbkdf2(100_000)));
    }

    #[test]
    fn the_salt_is_the_clients() {
        assert!(salt_is_ours(None, EMAIL));
        assert!(salt_is_ours(Some("person.example@amethystora.test"), EMAIL));
        assert!(salt_is_ours(Some(" PERSON.example@amethystora.test"), EMAIL));
        assert!(!salt_is_ours(Some("someone.else@amethystora.test"), EMAIL));
        assert!(!salt_is_ours(Some(""), EMAIL));
    }
}
