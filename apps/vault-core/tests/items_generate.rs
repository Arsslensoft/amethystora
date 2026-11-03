//! The generators: uniform, within their ranges, and honest about their bits.

use amethystora_vault_core::generate::{
    Charsets, DEFAULT_LENGTH, DEFAULT_WORDS, GenerateError, WORD_COUNT, estimate_bits, password, password_bits, words, words_bits,
};
use std::collections::HashMap;

const ONLY_DIGITS: Charsets = Charsets { lower: false, upper: false, digits: true, symbols: false };

#[test]
fn the_list_is_efs_large_one() {
    let list = include_str!("../assets/eff_large_wordlist.txt");
    let lines: Vec<&str> = list.lines().collect();
    assert_eq!(lines.len(), WORD_COUNT);
    let mut seen = std::collections::HashSet::new();
    for line in &lines {
        let (dice, word) = line.split_once('\t').unwrap();
        assert_eq!(dice.len(), 5);
        assert!(dice.bytes().all(|b| (b'1'..=b'6').contains(&b)));
        assert!(!word.is_empty() && seen.insert(word));
    }
    assert_eq!(lines[0], "11111\tabacus");
    assert_eq!(lines[WORD_COUNT - 1], "66666\tzoom");
}

#[test]
fn passwords_have_the_length_and_the_sets() {
    for length in [8, 20, 128] {
        for _ in 0..200 {
            let made = password(length, Charsets::default()).unwrap();
            let text = made.password.as_str();
            assert_eq!(text.len(), length);
            assert!(text.bytes().any(|b| b.is_ascii_lowercase()));
            assert!(text.bytes().any(|b| b.is_ascii_uppercase()));
            assert!(text.bytes().any(|b| b.is_ascii_digit()));
            assert!(text.bytes().any(|b| !b.is_ascii_alphanumeric()));
            assert!(!text.contains(['"', '\'', '\\', '`', ' ']));
        }
    }
    let only = password(DEFAULT_LENGTH, ONLY_DIGITS).unwrap();
    assert!(only.password.as_str().bytes().all(|b| b.is_ascii_digit()));
}

#[test]
fn out_of_range_is_refused() {
    for length in [0, 7, 129, usize::MAX] {
        assert_eq!(password(length, Charsets::default()).unwrap_err(), GenerateError::OutOfRange);
    }
    assert_eq!(password(20, Charsets { lower: false, upper: false, digits: false, symbols: false }).unwrap_err(), GenerateError::OutOfRange);
    assert_eq!(words(2, '-').unwrap_err(), GenerateError::OutOfRange);
    assert_eq!(words(21, '-').unwrap_err(), GenerateError::OutOfRange);
}

#[test]
fn the_bits_are_the_real_ones() {
    // One set: log2(10) a digit, exactly
    assert!((password(8, ONLY_DIGITS).unwrap().bits - 8.0 * 10f64.log2()).abs() < 1e-9);
    // Two sets of a short length, against counting. Lower and digits, 8 characters: 36^8 - 26^8 - 10^8
    let sets = Charsets { lower: true, upper: false, digits: true, symbols: false };
    let want = (36f64.powi(8) - 26f64.powi(8) - 10f64.powi(8)).log2();
    assert!((password_bits(8, sets) - want).abs() < 1e-9);
    // All four take a little off log2(90^len) for the passwords that lack a set
    let bits = password_bits(20, Charsets::default());
    let all = 20.0 * 90f64.log2();
    assert!(bits < all && all - bits < 0.5, "{bits} {all}");
    // The longest still fits a double
    assert!(password_bits(128, Charsets::default()).is_finite());
    // Six words, the default
    assert!((words(DEFAULT_WORDS, '-').unwrap().bits - 6.0 * 7776f64.log2()).abs() < 1e-9);
    assert!((words_bits(6) - 77.549).abs() < 0.01);
}

#[test]
fn words_come_from_the_list() {
    let list: std::collections::HashSet<&str> = include_str!("../assets/eff_large_wordlist.txt").lines().map(|l| l.split('\t').nth(1).unwrap()).collect();
    for _ in 0..200 {
        let made = words(6, ' ').unwrap();
        let parts: Vec<&str> = made.password.as_str().split(' ').collect();
        assert_eq!(parts.len(), 6);
        assert!(parts.iter().all(|p| list.contains(p)));
    }
    assert!(words(3, '-').unwrap().password.as_str().matches('-').count() >= 2);
}

/// The digits of 2,000 passwords of 128: 256,000 draws from ten, within what chance allows. A modulo on a byte would
/// make 0 to 5 likelier by about one part in 25, far past this.
#[test]
fn no_character_is_likelier_than_another() {
    let mut counts: HashMap<u8, u64> = HashMap::new();
    for _ in 0..2_000 {
        for b in password(128, ONLY_DIGITS).unwrap().password.as_str().bytes() {
            *counts.entry(b).or_default() += 1;
        }
    }
    let total: u64 = counts.values().sum();
    let expected = total as f64 / 10.0;
    assert_eq!(counts.len(), 10);
    let chi: f64 = counts.values().map(|c| (*c as f64 - expected).powi(2) / expected).sum();
    // 9 degrees of freedom: 1 in 100,000 runs passes 39
    assert!(chi < 39.0, "chi squared {chi}, {counts:?}");
}

#[test]
fn a_typed_password_gets_an_estimate() {
    assert_eq!(estimate_bits(""), 0.0);
    assert!(estimate_bits("zoom") < 13.0);
    assert!(estimate_bits("aaaaaaaaaaaaaaaaaaaa") < 26.0);
    assert!(estimate_bits("abcdefghijklmnopqrstuvwxyz") < 32.0);
    let strong = estimate_bits("Tr7#kQ9!mZ2@vX4$pL6%");
    assert!(strong > 100.0, "{strong}");
    assert!(estimate_bits("correct-horse-battery") > estimate_bits("correct"));
    assert!(estimate_bits("пароль-длинный") > 0.0);
}
