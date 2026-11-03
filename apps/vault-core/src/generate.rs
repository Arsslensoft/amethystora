//! Passwords and words, from the kernel's random source, and how strong each is: real bits for what was generated, and
//! a figure marked as an estimate for what was typed. Nothing checks a breach list: that would send something.

use zeroize::Zeroizing;

use crate::items::Secret;

pub const MIN_LENGTH: usize = 8;
pub const MAX_LENGTH: usize = 128;
pub const DEFAULT_LENGTH: usize = 20;
pub const DEFAULT_WORDS: usize = 6;
pub const MIN_WORDS: usize = 3;
pub const MAX_WORDS: usize = 20;

/// EFF's large list, as EFF publishes it (CC-BY 3.0 US, named in `NOTICE`): a line is five dice, a tab and the word
const WORD_LIST: &str = include_str!("../assets/eff_large_wordlist.txt");
/// The longest word of the list in bytes (a test holds it to the list)
pub const LONGEST_WORD: usize = 9;
pub const WORD_COUNT: usize = 7776;

const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
/// No quote, backslash, backtick or space: each breaks somewhere a password gets pasted
const SYMBOLS: &str = "!#$%&()*+,-./:;<=>?@[]^_{|}~";

/// Which characters a password may hold. Each chosen set appears at least once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Charsets {
    pub lower: bool,
    pub upper: bool,
    pub digits: bool,
    pub symbols: bool,
}

impl Default for Charsets {
    fn default() -> Charsets {
        Charsets { lower: true, upper: true, digits: true, symbols: true }
    }
}

impl Charsets {
    fn chosen(self) -> Vec<&'static str> {
        [(self.lower, LOWER), (self.upper, UPPER), (self.digits, DIGITS), (self.symbols, SYMBOLS)]
            .into_iter()
            .filter_map(|(on, set)| on.then_some(set))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerateError {
    /// A length outside 8 to 128, a number of words outside 3 to 20, or no character set
    OutOfRange,
    /// The kernel's random source failed
    Random,
}

impl From<getrandom::Error> for GenerateError {
    fn from(_: getrandom::Error) -> GenerateError {
        GenerateError::Random
    }
}

/// What came out, and the real number of bits: how many choices there were
#[derive(Clone, Debug)]
pub struct Generated {
    pub password: Secret,
    pub bits: f64,
}

/// A uniform number below `n`, from a pool of random bytes: a value past the last whole multiple of `n` is thrown away,
/// so that no number is likelier than another
struct Dice {
    pool: Zeroizing<[u8; 256]>,
    at: usize,
}

impl Dice {
    fn new() -> Dice {
        Dice { pool: Zeroizing::new([0; 256]), at: 256 }
    }

    fn byte(&mut self) -> Result<u8, getrandom::Error> {
        if self.at == self.pool.len() {
            getrandom::fill(&mut self.pool[..])?;
            self.at = 0;
        }
        self.at += 1;
        Ok(self.pool[self.at - 1])
    }

    fn below(&mut self, n: u32) -> Result<u32, getrandom::Error> {
        debug_assert!((1..=65536).contains(&n));
        let limit = 65536 - 65536 % n;
        loop {
            let value = u32::from(self.byte()?) << 8 | u32::from(self.byte()?);
            if value < limit {
                return Ok(value % n);
            }
        }
    }
}

/// A random password of `length` characters from the chosen sets, uniform over every such password holding at least
/// one of each: one that lacks a set is thrown away whole, so that the bits below are the truth.
pub fn password(length: usize, sets: Charsets) -> Result<Generated, GenerateError> {
    let chosen = sets.chosen();
    if !(MIN_LENGTH..=MAX_LENGTH).contains(&length) || chosen.is_empty() {
        return Err(GenerateError::OutOfRange);
    }
    let alphabet: Vec<u8> = chosen.iter().flat_map(|set| set.bytes()).collect();
    let mut dice = Dice::new();
    loop {
        let mut out = Zeroizing::new(String::with_capacity(length));
        for _ in 0..length {
            out.push(alphabet[dice.below(alphabet.len() as u32)? as usize] as char);
        }
        if chosen.iter().all(|set| out.bytes().any(|b| set.as_bytes().contains(&b))) {
            return Ok(Generated { password: Secret::from(out), bits: password_bits(length, sets) });
        }
    }
}

/// log2 of how many passwords of this length hold every chosen set, by inclusion and exclusion over the sets
pub fn password_bits(length: usize, sets: Charsets) -> f64 {
    let sizes: Vec<f64> = sets.chosen().iter().map(|s| s.len() as f64).collect();
    let mut count = 0.0;
    for mask in 1u32..(1 << sizes.len()) {
        let size: f64 = (0..sizes.len()).filter(|i| mask & (1 << i) != 0).map(|i| sizes[i]).sum();
        let sign = if (sizes.len() as u32 - mask.count_ones()) & 1 == 0 { 1.0 } else { -1.0 };
        count += sign * size.powi(length as i32);
    }
    count.log2()
}

fn word(index: usize) -> &'static str {
    WORD_LIST.lines().nth(index).and_then(|line| line.split('\t').nth(1)).unwrap_or("")
}

/// Words from the list, each a uniform choice, joined by a separator: the words are all that is secret
pub fn words(count: usize, separator: char) -> Result<Generated, GenerateError> {
    if !(MIN_WORDS..=MAX_WORDS).contains(&count) {
        return Err(GenerateError::OutOfRange);
    }
    let mut dice = Dice::new();
    // The words chosen first, so that the one buffer they go into is the right size and never grows
    let mut picked = Zeroizing::new([0u16; MAX_WORDS]);
    for slot in picked.iter_mut().take(count) {
        *slot = dice.below(WORD_COUNT as u32)? as u16;
    }
    // The longest word and a separator for each, so that the buffer is never grown and nothing else names the words
    let mut out = Zeroizing::new(String::with_capacity(count * (LONGEST_WORD + separator.len_utf8())));
    for (i, &at) in picked[..count].iter().enumerate() {
        if i > 0 {
            out.push(separator);
        }
        out.push_str(word(at as usize));
    }
    Ok(Generated { password: Secret::from(out), bits: words_bits(count) })
}

pub fn words_bits(count: usize) -> f64 {
    count as f64 * (WORD_COUNT as f64).log2()
}

/// A rough figure for a typed password. It is an estimate, and has to be shown as one: nothing can know how someone
/// chose a password. It takes the pool of characters used, charges little for what repeats or runs on from the
/// character before, and takes a list word as what it is.
pub fn estimate_bits(password: &str) -> f64 {
    // The list is ASCII, so this needs no lower-case copy of the password
    if WORD_LIST.lines().filter_map(|line| line.split('\t').nth(1)).any(|word| word.eq_ignore_ascii_case(password)) {
        return (WORD_COUNT as f64).log2();
    }
    let (mut l, mut u, mut d, mut s, mut other) = (false, false, false, false, false);
    for c in password.chars() {
        match c {
            'a'..='z' => l = true,
            'A'..='Z' => u = true,
            '0'..='9' => d = true,
            _ if c.is_ascii() => s = true,
            _ => other = true,
        }
    }
    let pool = [(l, 26), (u, 26), (d, 10), (s, 33), (other, 100)].iter().filter(|(on, _)| *on).map(|(_, n)| n).sum::<u32>();
    if pool == 0 {
        return 0.0;
    }
    let each = f64::from(pool).log2();
    let (mut bits, mut last) = (0.0, None::<char>);
    for c in password.chars() {
        let close = last.is_some_and(|p| (p as i64 - c as i64).abs() <= 1);
        bits += if close { 1.0 } else { each };
        last = Some(c);
    }
    bits
}
