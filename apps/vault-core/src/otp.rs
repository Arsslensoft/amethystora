//! One-time codes: HOTP (RFC 4226), TOTP (RFC 6238) and Steam Guard, and the Base32 their secrets are written in
//! (plans/vault-bmc/spec-formats.md, sections 3 and 4). A code is asked for once a second per account on screen, so
//! making one allocates nothing: the key in a buffer on the stack that wipes itself, the hashes on the stack.

use std::fmt;

use sha1::Digest;
use zeroize::Zeroizing;

use crate::items::{Account, MAX_SECRET_CHARS, OtpKind, Secret, is_js_space};

/// What a secret of the longest allowed text decodes to
const MAX_KEY: usize = MAX_SECRET_CHARS * 5 / 8;
/// Longest code: a 31-bit value has ten digits, so more could only be zeros
const MAX_DIGITS: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
}

impl Algorithm {
    /// The name a stored account holds, as section 3 reads it. SHA-3's names are known and make no code either.
    pub fn from_stored(name: &str) -> Option<Algorithm> {
        const NAMES: [(&str, Algorithm); 15] = [
            ("SHA1", Algorithm::Sha1),
            ("SHA-1", Algorithm::Sha1),
            ("SSL3-SHA1", Algorithm::Sha1),
            ("SHA224", Algorithm::Sha224),
            ("SHA-224", Algorithm::Sha224),
            ("SHA2-224", Algorithm::Sha224),
            ("SHA256", Algorithm::Sha256),
            ("SHA-256", Algorithm::Sha256),
            ("SHA2-256", Algorithm::Sha256),
            ("SHA384", Algorithm::Sha384),
            ("SHA-384", Algorithm::Sha384),
            ("SHA2-384", Algorithm::Sha384),
            ("SHA512", Algorithm::Sha512),
            ("SHA-512", Algorithm::Sha512),
            ("SHA2-512", Algorithm::Sha512),
        ];
        NAMES.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, a)| *a)
    }

    pub fn name(self) -> &'static str {
        match self {
            Algorithm::Sha1 => "SHA1",
            Algorithm::Sha224 => "SHA224",
            Algorithm::Sha256 => "SHA256",
            Algorithm::Sha384 => "SHA384",
            Algorithm::Sha512 => "SHA512",
        }
    }
}

/// The largest HMAC here, on the stack and wiped
struct Mac {
    bytes: Zeroizing<[u8; 64]>,
    len: usize,
}

/// What HMAC needs of a hash. A code is made in the process that holds every vault's key, which never loads libcrypto, so
/// these are the `sha1` and `sha2` crates', each on the stack.
trait Hash {
    const BLOCK: usize;
    const OUT: usize;
    fn start() -> Self;
    fn add(&mut self, bytes: &[u8]);
    fn end(self, out: &mut [u8]);
}

macro_rules! digest_hash {
    ($name:path, $block:literal, $out:literal) => {
        impl Hash for $name {
            const BLOCK: usize = $block;
            const OUT: usize = $out;
            fn start() -> Self {
                <Self as Digest>::new()
            }
            fn add(&mut self, bytes: &[u8]) {
                Digest::update(self, bytes);
            }
            fn end(self, out: &mut [u8]) {
                out[..$out].copy_from_slice(&self.finalize());
            }
        }
    };
}

digest_hash!(sha1::Sha1, 64, 20);
digest_hash!(sha2::Sha224, 64, 28);
digest_hash!(sha2::Sha256, 64, 32);
digest_hash!(sha2::Sha384, 128, 48);
digest_hash!(sha2::Sha512, 128, 64);

fn hmac_with<H: Hash>(key: &[u8], message: &[u8]) -> Mac {
    let mut held = Zeroizing::new([0u8; 128]);
    let pad = &mut held[..H::BLOCK];
    if key.len() > H::BLOCK {
        let mut hash = H::start();
        hash.add(key);
        hash.end(pad);
    } else {
        pad[..key.len()].copy_from_slice(key);
    }
    pad.iter_mut().for_each(|b| *b ^= 0x36);
    let mut hash = H::start();
    hash.add(pad);
    hash.add(message);
    let mut inner = Zeroizing::new([0u8; 64]);
    hash.end(&mut inner[..]);
    pad.iter_mut().for_each(|b| *b ^= 0x36 ^ 0x5c);
    let mut hash = H::start();
    hash.add(pad);
    hash.add(&inner[..H::OUT]);
    let mut bytes = Zeroizing::new([0u8; 64]);
    hash.end(&mut bytes[..]);
    Mac { bytes, len: H::OUT }
}

fn hmac(algorithm: Algorithm, key: &[u8], message: &[u8]) -> Mac {
    match algorithm {
        Algorithm::Sha1 => hmac_with::<sha1::Sha1>(key, message),
        Algorithm::Sha224 => hmac_with::<sha2::Sha224>(key, message),
        Algorithm::Sha256 => hmac_with::<sha2::Sha256>(key, message),
        Algorithm::Sha384 => hmac_with::<sha2::Sha384>(key, message),
        Algorithm::Sha512 => hmac_with::<sha2::Sha512>(key, message),
    }
}

/// A code in a small buffer of its own. It prints nothing by accident.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Code {
    text: [u8; MAX_DIGITS],
    len: u8,
}

impl Code {
    fn dashes(len: usize) -> Code {
        Code { text: [b'-'; MAX_DIGITS], len: len.min(MAX_DIGITS) as u8 }
    }

    pub fn as_str(&self) -> &str {
        // Only ASCII is ever written into it
        std::str::from_utf8(&self.text[..self.len as usize]).unwrap_or("")
    }

    /// Whether the account could make it: an unusable account gets dashes as long as its code
    pub fn is_usable(&self) -> bool {
        self.len > 0 && self.text[0] != b'-'
    }

    pub fn grouped(&self) -> String {
        group_code(self.as_str())
    }
}

impl fmt::Debug for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[code]")
    }
}

const STEAM_ALPHABET: &[u8; 26] = b"23456789BCDFGHJKMNPQRTVWXY";

/// The code for an account at a time in Unix seconds, `offset` windows on. It never fails: an account that cannot make
/// one gets dashes, and a period of 0 or below, or a secret of no bytes, which the doors refuse, count as that too.
pub fn code_at(account: &Account, unix_seconds: i64, offset: i64) -> Code {
    let kind = account.otp_kind();
    // Digits that make no code (0, or past what 31 bits hold) are refused at the doors, and show as six dashes
    let width = match kind {
        OtpKind::Steam => 5,
        _ if (1..=MAX_DIGITS as i64).contains(&account.digits) => account.digits as usize,
        _ => return Code::dashes(6),
    };
    let counter = match kind {
        OtpKind::Hotp => account.counter.unwrap_or(0),
        _ if account.period <= 0 => return Code::dashes(width),
        _ => unix_seconds.div_euclid(account.period),
    }
    .wrapping_add(offset);
    let algorithm = match kind {
        OtpKind::Steam => Algorithm::Sha1,
        _ => match Algorithm::from_stored(&account.algorithm) {
            Some(a) => a,
            None => return Code::dashes(width),
        },
    };
    let mut key = Zeroizing::new([0u8; MAX_KEY]);
    let len = match decode_into(account.secret.as_str(), &mut key[..]) {
        Some(len) if len > 0 => len,
        _ => return Code::dashes(width),
    };
    let mac = hmac(algorithm, &key[..len], &counter.to_be_bytes());
    let value = truncate(&mac.bytes[..mac.len]);
    if kind == OtpKind::Steam { steam_text(value) } else { digits_text(value, width) }
}

fn truncate(mac: &[u8]) -> u32 {
    let at = (mac[mac.len() - 1] & 0x0f) as usize;
    u32::from_be_bytes([mac[at], mac[at + 1], mac[at + 2], mac[at + 3]]) & 0x7fff_ffff
}

fn digits_text(value: u32, width: usize) -> Code {
    let mut code = Code { text: [b'0'; MAX_DIGITS], len: width as u8 };
    let mut rest = value as u64 % 10u64.pow(width as u32);
    for at in (0..width).rev() {
        code.text[at] = b'0' + (rest % 10) as u8;
        rest /= 10;
    }
    code
}

fn steam_text(mut value: u32) -> Code {
    let mut code = Code { text: [0; MAX_DIGITS], len: 5 };
    for at in 0..5 {
        code.text[at] = STEAM_ALPHABET[(value % 26) as usize];
        value /= 26;
    }
    code
}

/// A code of exactly 6 or 8 characters in two halves, `287 082`; any other length whole
pub fn group_code(code: &str) -> String {
    let count = code.chars().count();
    if count == 6 || count == 8 {
        let half: String = code.chars().take(count / 2).collect();
        let rest: String = code.chars().skip(count / 2).collect();
        format!("{half} {rest}")
    } else {
        code.to_string()
    }
}

/// Seconds until the code changes, from 1 to the period
pub fn seconds_left(period: i64, unix_ms: i64) -> i64 {
    if period <= 0 {
        return 0;
    }
    period - unix_ms.div_euclid(1000).rem_euclid(period)
}

/// A secret for typing, in fours with a space, no space at the end. A line break starts a new count.
pub fn group_secret(secret: &str) -> Secret {
    let mut out = Zeroizing::new(String::with_capacity(secret.len() + secret.len() / 4));
    let mut units = 0;
    for c in secret.chars() {
        out.push(c);
        if matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
            units = 0;
            continue;
        }
        units += c.len_utf16();
        if units >= 4 {
            out.push(' ');
            units = 0;
        }
    }
    // Trimmed in place, so that the grouped secret is never copied
    let end = out.trim_end_matches(is_js_space).len();
    out.truncate(end);
    let start = out.len() - out.trim_start_matches(is_js_space).len();
    out.drain(..start);
    Secret::from(out)
}

fn base32_value(c: char) -> Option<u8> {
    match c {
        'A'..='Z' => Some(c as u8 - b'A'),
        '2'..='7' => Some(c as u8 - b'2' + 26),
        _ => None,
    }
}

/// Section 4's reading, each output byte to `put`: spaces go, full Unicode upper case, `=` only at the end, and bits
/// that do not fill a byte are dropped even when they are not zero. False on anything else.
fn read_base32(text: &str, mut put: impl FnMut(u8)) -> bool {
    let (mut bits, mut have, mut padding) = (0u32, 0u32, false);
    for c in text.chars().filter(|c| *c != ' ') {
        for upper in c.to_uppercase() {
            if upper == '=' {
                padding = true;
                continue;
            }
            let Some(value) = base32_value(upper) else { return false };
            if padding {
                return false;
            }
            bits = (bits << 5 | value as u32) & 0xfff;
            have += 5;
            if have >= 8 {
                have -= 8;
                put((bits >> have) as u8);
            }
        }
    }
    true
}

/// Into a buffer of the caller's, for code-making. `None` on a bad secret or one too long for it.
fn decode_into(text: &str, out: &mut [u8]) -> Option<usize> {
    let (mut len, mut fits) = (0, true);
    let good = read_base32(text, |b| {
        if len < out.len() {
            out[len] = b;
            len += 1;
        } else {
            fits = false;
        }
    });
    (good && fits).then_some(len)
}

/// The bytes of a Base32 secret. Empty for `` and `A`, as section 4 has it: the doors refuse a secret of no bytes.
pub fn base32_decode(text: &str) -> Option<Zeroizing<Vec<u8>>> {
    let mut count = 0;
    if !read_base32(text, |_| count += 1) {
        return None;
    }
    let mut out = Zeroizing::new(Vec::with_capacity(count));
    read_base32(text, |b| out.push(b));
    Some(out)
}

/// RFC 4648's alphabet, upper case, no padding, a last partial group left-aligned with zero bits
pub fn base32_encode(bytes: &[u8]) -> Secret {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = Zeroizing::new(String::with_capacity((bytes.len() * 8).div_ceil(5)));
    let (mut bits, mut have) = (0u32, 0u32);
    for &b in bytes {
        bits = (bits << 8 | b as u32) & 0xffff;
        have += 8;
        while have >= 5 {
            have -= 5;
            out.push(ALPHABET[(bits >> have) as usize & 31] as char);
        }
    }
    if have > 0 {
        out.push(ALPHABET[(bits << (5 - have)) as usize & 31] as char);
    }
    Secret::from(out)
}

/// A secret decoded and written again: upper case, no padding, no leftover bits
pub fn canonical_secret(text: &str) -> Option<Secret> {
    base32_decode(text).map(|bytes| base32_encode(&bytes))
}

/// What a person typed, as they will see it: full Unicode upper case, then every space and `-` gone
pub fn normalize_typed(text: &str) -> Zeroizing<String> {
    let kept = || text.chars().flat_map(char::to_uppercase).filter(|c| !is_js_space(*c) && *c != '-');
    // Measured first, so that the buffer never grows and leaves a copy behind
    let mut out = Zeroizing::new(String::with_capacity(kept().map(char::len_utf8).sum()));
    out.extend(kept());
    out
}

/// `^[A-Z2-7]+=*$`, and how many characters come before the padding
fn plain_base32_length(text: &str) -> Option<usize> {
    let body = text.trim_end_matches('=');
    let good = !body.is_empty() && body.bytes().all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b));
    good.then_some(body.len())
}

/// A normalized typed secret is good when it decodes to at least 10 bytes
pub fn typed_secret_is_valid(normalized: &str) -> bool {
    normalized.len() <= MAX_SECRET_CHARS && plain_base32_length(normalized).is_some_and(|len| len * 5 / 8 >= 10)
}

/// A secret from a structured import: far more lenient than typing, since someone already has that account, but it
/// must hold a byte. Kept as this text, not canonicalized.
pub fn structured_secret(text: &str) -> Option<Secret> {
    if text.len() > MAX_SECRET_CHARS * 4 {
        return None;
    }
    let mut normal = normalize_typed(text);
    let end = normal.trim_end_matches('=').len();
    normal.truncate(end);
    let good = normal.len() <= MAX_SECRET_CHARS && plain_base32_length(&normal).is_some_and(|len| len * 5 / 8 >= 1);
    good.then(|| Secret::from(normal))
}

/// Whether a secret decodes and holds no byte at all, which every door refuses (section 16, point 18)
pub fn holds_no_bytes(secret: &str) -> bool {
    base32_decode(secret).is_some_and(|bytes| bytes.is_empty())
}
