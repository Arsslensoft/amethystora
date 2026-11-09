//! What the review of 2026-11-02 found in the import of keys (plans/vault/work/review-pki.md: M3, M6, M7, L3, L5), each held
//! to its refusal. Files are made here, by hand, as an attacker would; nothing here needs another program.

use std::time::{Duration, Instant};

use amethystora_vault_core::keys::{import_openssh, KeyError};
use amethystora_vault_core::pki::der;
use base64::Engine;
use openssl::bn::{BigNum, MsbOption};
use openssl::ec::{EcGroup, EcKey};
use openssl::nid::Nid;
use openssl::rsa::Rsa;

fn ssh_string(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
}

fn armour(label: &str, binary: &[u8], width: usize) -> Vec<u8> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(binary);
    let mut text = format!("-----BEGIN {label}-----\n");
    for line in encoded.as_bytes().chunks(width) {
        text.push_str(std::str::from_utf8(line).unwrap());
        text.push('\n');
    }
    text.push_str(&format!("-----END {label}-----\n"));
    text.into_bytes()
}

/// An `openssh-key-v1` file, with `after` written past the sealed part
fn v1_file(cipher: &str, kdf: &str, options: &[u8], public: &[u8], private: &[u8], after: &[u8]) -> Vec<u8> {
    let mut binary = b"openssh-key-v1\0".to_vec();
    ssh_string(&mut binary, cipher.as_bytes());
    ssh_string(&mut binary, kdf.as_bytes());
    ssh_string(&mut binary, options);
    binary.extend_from_slice(&1u32.to_be_bytes());
    ssh_string(&mut binary, public);
    ssh_string(&mut binary, private);
    binary.extend_from_slice(after);
    armour("OPENSSH PRIVATE KEY", &binary, 70)
}

/// The private part of an unencrypted RSA file, the numbers as given
fn rsa_private(numbers: [&[u8]; 6]) -> Vec<u8> {
    let mut out = vec![9, 9, 9, 9, 9, 9, 9, 9];
    ssh_string(&mut out, b"ssh-rsa");
    for number in numbers {
        let mut signed = vec![0u8];
        signed.extend_from_slice(number);
        ssh_string(&mut out, &signed);
    }
    ssh_string(&mut out, b"");
    // The padding to the cipher's block, 8 for "none"
    let mut pad = 1u8;
    while !out.len().is_multiple_of(8) {
        out.push(pad);
        pad += 1;
    }
    out
}

fn rsa_public(rsa: &Rsa<openssl::pkey::Private>) -> Vec<u8> {
    let mut out = Vec::new();
    ssh_string(&mut out, b"ssh-rsa");
    for number in [rsa.e().to_vec(), rsa.n().to_vec()] {
        // An mpint: a zero in front only where the top bit would read as a sign
        let mut signed = if number[0] & 0x80 != 0 { vec![0u8] } else { Vec::new() };
        signed.extend_from_slice(&number);
        ssh_string(&mut out, &signed);
    }
    out
}

/// Refused, and quickly: the service runs an import off its main loop, but it must not hold it
fn refused_within(file: &[u8], passphrase: &[u8], limit: Duration) -> KeyError {
    let start = Instant::now();
    let result = import_openssh(file, passphrase);
    let took = start.elapsed();
    assert!(took < limit, "took {took:?}");
    result.map(|_| ()).unwrap_err()
}

/// M3: a PEM key whose numbers do not agree is refused, so that what it shows is the key that signs
#[test]
fn m3_pem_with_numbers_that_disagree_is_refused() {
    let rsa = Rsa::generate(2048).unwrap();
    let wrong_d = &rsa.d().to_owned().unwrap() + &BigNum::from_u32(2).unwrap();
    let parts = |d: BigNum, p: BigNum| {
        Rsa::from_private_components(
            rsa.n().to_owned().unwrap(),
            rsa.e().to_owned().unwrap(),
            d,
            p,
            rsa.q().unwrap().to_owned().unwrap(),
            rsa.dmp1().unwrap().to_owned().unwrap(),
            rsa.dmq1().unwrap().to_owned().unwrap(),
            rsa.iqmp().unwrap().to_owned().unwrap(),
        )
        .unwrap()
    };
    let bad_d = parts(wrong_d, rsa.p().unwrap().to_owned().unwrap());
    assert!(import_openssh(&bad_d.private_key_to_pem().unwrap(), b"").is_err(), "RSA with a wrong d");
    let other = Rsa::generate(2048).unwrap();
    let bad_p = parts(rsa.d().to_owned().unwrap(), other.p().unwrap().to_owned().unwrap());
    assert!(import_openssh(&bad_p.private_key_to_pem().unwrap(), b"").is_err(), "RSA with a p that is not a factor");

    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
    let (a, b) = (EcKey::generate(&group).unwrap(), EcKey::generate(&group).unwrap());
    let mixed = EcKey::from_private_components(&group, a.private_key(), b.public_key()).unwrap();
    assert!(import_openssh(&mixed.private_key_to_pem().unwrap(), b"").is_err(), "EC with another key's public point");
    // And the same keys whole still import
    assert!(import_openssh(&rsa.private_key_to_pem().unwrap(), b"").is_ok());
    assert!(import_openssh(&a.private_key_to_pem().unwrap(), b"").is_ok());
}

fn encrypted_pkcs8(kdf: &[u8]) -> Vec<u8> {
    let cipher = der::seq(&[&der::oid("2.16.840.1.101.3.4.1.42"), &der::octets(&[2u8; 16])]);
    let algorithm = der::seq(&[&der::oid("1.2.840.113549.1.5.13"), &der::seq(&[kdf, &cipher])]);
    armour("ENCRYPTED PRIVATE KEY", &der::seq(&[&algorithm, &der::octets(&[3u8; 64])]), 64)
}

fn pbkdf2(iterations: u32) -> Vec<u8> {
    der::seq(&[
        &der::oid("1.2.840.113549.1.5.12"),
        &der::seq(&[&der::octets(&[1u8; 16]), &der::uint(&iterations.to_be_bytes()), &der::seq(&[&der::oid("1.2.840.113549.2.9"), &der::null()])]),
    ])
}

/// M6: an encrypted PKCS#8 file sets its own PBKDF2 count, read and bounded before OpenSSL sees the file
#[test]
fn m6_a_pkcs8_files_own_count_is_bounded() {
    for iterations in [10_000_001u32, 20_000_000, 2_147_483_647] {
        let error = refused_within(&encrypted_pkcs8(&pbkdf2(iterations)), b"x", Duration::from_millis(500));
        assert_eq!(error, KeyError::TooCostly, "{iterations}");
    }
    // Within the bound it is only a wrong passphrase, the data being nonsense
    assert_eq!(import_openssh(&encrypted_pkcs8(&pbkdf2(1000)), b"x").map(|_| ()).unwrap_err(), KeyError::WrongPassphrase);
    // scrypt, whose cost a file sets too, and PBES1, whose count is elsewhere: refused, not read
    let scrypt = der::seq(&[&der::oid("1.3.6.1.4.1.11591.4.11"), &der::seq(&[&der::octets(&[1u8; 16]), &der::uint(&[0x40, 0, 0, 0]), &der::uint(&[8]), &der::uint(&[1])])]);
    assert_eq!(refused_within(&encrypted_pkcs8(&scrypt), b"x", Duration::from_millis(500)), KeyError::Unsupported);
    let pbes1 = der::seq(&[&der::oid("1.2.840.113549.1.5.10"), &der::seq(&[&der::octets(&[1u8; 8]), &der::uint(&[0x7f, 0xff, 0xff, 0xff])])]);
    let file = armour("ENCRYPTED PRIVATE KEY", &der::seq(&[&pbes1, &der::octets(&[3u8; 64])]), 64);
    assert_eq!(refused_within(&file, b"x", Duration::from_millis(500)), KeyError::Unsupported);
    // Two keys in one file: which one OpenSSL would read is not this code's guess
    let mut two = encrypted_pkcs8(&pbkdf2(1000));
    two.extend(encrypted_pkcs8(&pbkdf2(2_000_000_000)));
    assert!(refused_within(&two, b"x", Duration::from_millis(500)) != KeyError::WrongPassphrase);
}

/// A number with no small factor, so that a primality test has to work for it
fn rough(bits: i32) -> BigNum {
    let mut number = BigNum::new().unwrap();
    number.pseudo_rand(bits, MsbOption::MAYBE_ZERO, true).unwrap();
    let two = BigNum::from_u32(2).unwrap();
    while [3u32, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47].iter().any(|prime| number.mod_word(*prime).unwrap() == 0) {
        number = &number + &two;
    }
    number
}

/// M7: every number of an RSA key is bounded by its modulus before any arithmetic, in both forms
#[test]
fn m7_an_rsa_files_numbers_are_bounded() {
    let rsa = Rsa::generate(2048).unwrap();
    let (n, e, d, iqmp, p, q) =
        (rsa.n().to_vec(), rsa.e().to_vec(), rsa.d().to_vec(), rsa.iqmp().unwrap().to_vec(), rsa.p().unwrap().to_vec(), rsa.q().unwrap().to_vec());
    let huge = rough(32768).to_vec();
    let long_e = rough(80).to_vec();
    let long_d = rough(2100).to_vec();
    for (what, numbers) in [
        ("p", [&n[..], &e, &d, &iqmp, &huge, &q]),
        ("q", [&n[..], &e, &d, &iqmp, &p, &huge]),
        ("d", [&n[..], &e, &long_d, &iqmp, &p, &q]),
        ("iqmp", [&n[..], &e, &d, &long_d, &p, &q]),
        ("e", [&n[..], &long_e, &d, &iqmp, &p, &q]),
    ] {
        let file = v1_file("none", "none", b"", &rsa_public(&rsa), &rsa_private(numbers), b"");
        refused_within(&file, b"", Duration::from_secs(1));
        eprintln!("{what} refused");
    }
    // The PEM form, through OpenSSL's own reader: the same huge p
    let pem = Rsa::from_private_components(
        rsa.n().to_owned().unwrap(),
        rsa.e().to_owned().unwrap(),
        rsa.d().to_owned().unwrap(),
        BigNum::from_slice(&huge).unwrap(),
        rsa.q().unwrap().to_owned().unwrap(),
        rsa.dmp1().unwrap().to_owned().unwrap(),
        rsa.dmq1().unwrap().to_owned().unwrap(),
        rsa.iqmp().unwrap().to_owned().unwrap(),
    )
    .unwrap()
    .private_key_to_pem()
    .unwrap();
    refused_within(&pem, b"", Duration::from_secs(1));
    // The whole key is fine
    let good = v1_file("none", "none", b"", &rsa_public(&rsa), &rsa_private([&n, &e, &d, &iqmp, &p, &q]), b"");
    assert!(import_openssh(&good, b"").is_ok());
}

/// M6a: an encrypted PKCS#8 key under the plain label is decrypted by OpenSSL all the same, so its count is bounded too
#[test]
fn m6a_an_encrypted_key_under_the_plain_label_is_bounded() {
    let encrypted = |iterations: u32| {
        let cipher = der::seq(&[&der::oid("2.16.840.1.101.3.4.1.42"), &der::octets(&[2u8; 16])]);
        let algorithm = der::seq(&[&der::oid("1.2.840.113549.1.5.13"), &der::seq(&[&pbkdf2(iterations), &cipher])]);
        armour("PRIVATE KEY", &der::seq(&[&algorithm, &der::octets(&[3u8; 64])]), 64)
    };
    assert_eq!(refused_within(&encrypted(2_147_483_647), b"", Duration::from_millis(500)), KeyError::TooCostly);
    assert!(refused_within(&encrypted(1000), b"", Duration::from_millis(500)) != KeyError::TooCostly);
}

/// M7a: a key of three primes, the third of 65536 bits, which nothing else bounds, is refused before OpenSSL tests it
#[test]
fn m7a_an_rsa_key_of_more_than_two_primes_is_refused() {
    let rsa = Rsa::generate(2048).unwrap();
    let int = |number: &openssl::bn::BigNumRef| der::uint(&number.to_vec());
    let r = rough(65536);
    let other = der::seq(&[&int(&r), &int(&r), &int(rsa.q().unwrap())]);
    let key = der::seq(&[
        &der::uint(&[1]),
        &int(rsa.n()),
        &int(rsa.e()),
        &int(rsa.d()),
        &int(rsa.p().unwrap()),
        &int(rsa.q().unwrap()),
        &int(rsa.dmp1().unwrap()),
        &int(rsa.dmq1().unwrap()),
        &int(rsa.iqmp().unwrap()),
        &der::seq(&[&other]),
    ]);
    assert_eq!(refused_within(&armour("RSA PRIVATE KEY", &key, 64), b"", Duration::from_secs(2)), KeyError::Unsupported);
}

/// M7b: OpenSSL works out a DSA key's public number while it reads the file, from sizes the file sets: such a key is refused by
/// its type before it is read, and so is one in the old form
#[test]
fn m7b_a_dsa_key_is_refused_before_it_is_read() {
    let (p, x) = (rough(65536).to_vec(), rough(65536).to_vec());
    let parameters = der::seq(&[&der::uint(&p), &der::uint(&[0x7f; 20]), &der::uint(&[2])]);
    let algorithm = der::seq(&[&der::oid("1.2.840.10040.4.1"), &parameters]);
    let pkcs8 = der::seq(&[&der::uint(&[0]), &algorithm, &der::octets(&der::uint(&x))]);
    assert_eq!(refused_within(&armour("PRIVATE KEY", &pkcs8, 64), b"", Duration::from_millis(500)), KeyError::Unsupported);
    let old = der::seq(&[&der::uint(&[0]), &der::uint(&p), &der::uint(&[0x7f; 20]), &der::uint(&[2]), &der::uint(&x), &der::uint(&x)]);
    assert_eq!(refused_within(&armour("DSA PRIVATE KEY", &old, 64), b"", Duration::from_millis(500)), KeyError::Unsupported);
}

/// K12: a certificate beside one encrypted key, as `openssl pkcs12` writes them, imports; two keys still do not
#[test]
fn k12_a_certificate_beside_an_encrypted_key_is_passed_over() {
    let key = openssl::pkey::PKey::from_ec_key(EcKey::generate(&EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap()).unwrap()).unwrap();
    let encrypted = key.private_key_to_pem_pkcs8_passphrase(openssl::symm::Cipher::aes_256_cbc(), b"pass phrase").unwrap();
    let mut file = armour("CERTIFICATE", &der::seq(&[&der::null()]), 64);
    file.extend(&encrypted);
    assert!(import_openssh(&file, b"pass phrase").is_ok());
    file.extend(key.private_key_to_pem_pkcs8().unwrap());
    assert_eq!(import_openssh(&file, b"pass phrase").map(|_| ()).unwrap_err(), KeyError::Malformed);
}

/// L3: bytes after the sealed part, or no END line, are refused rather than read past
#[test]
fn l3_a_file_must_end_where_it_says() {
    let rsa = Rsa::generate(2048).unwrap();
    let numbers = [rsa.n().to_vec(), rsa.e().to_vec(), rsa.d().to_vec(), rsa.iqmp().unwrap().to_vec(), rsa.p().unwrap().to_vec(), rsa.q().unwrap().to_vec()];
    let private = rsa_private([&numbers[0], &numbers[1], &numbers[2], &numbers[3], &numbers[4], &numbers[5]]);
    let good = v1_file("none", "none", b"", &rsa_public(&rsa), &private, b"");
    assert!(import_openssh(&good, b"").is_ok());
    let trailing = v1_file("none", "none", b"", &rsa_public(&rsa), &private, b"more");
    assert!(import_openssh(&trailing, b"").is_err());
    let text = String::from_utf8(good).unwrap();
    let cut = text.replace("-----END OPENSSH PRIVATE KEY-----\n", "");
    assert!(import_openssh(cut.as_bytes(), b"").is_err());
}

/// K8: an OpenSSH file is one key, its BEGIN line first and its END line last: text around it, a second key or an END of another
/// name is refused rather than passed over
#[test]
fn k8_an_openssh_file_is_its_key_and_nothing_else() {
    let rsa = Rsa::generate(2048).unwrap();
    let numbers = [rsa.n().to_vec(), rsa.e().to_vec(), rsa.d().to_vec(), rsa.iqmp().unwrap().to_vec(), rsa.p().unwrap().to_vec(), rsa.q().unwrap().to_vec()];
    let private = rsa_private([&numbers[0], &numbers[1], &numbers[2], &numbers[3], &numbers[4], &numbers[5]]);
    let good = String::from_utf8(v1_file("none", "none", b"", &rsa_public(&rsa), &private, b"")).unwrap();
    // Blank lines around it and Windows's line ends are still the file ssh-keygen wrote
    for same in [good.clone(), format!("\n\n{good}\n\n"), good.replace('\n', "\r\n"), good.trim_end().to_string()] {
        assert!(import_openssh(same.as_bytes(), b"").is_ok(), "{same:?}");
    }
    for odd in [
        format!("Here is my key:\n{good}"),
        format!("{good}and some more text\n"),
        format!("{good}{good}"),
        good.replace("-----END OPENSSH PRIVATE KEY-----", "-----END OPENSSH PRIVATE KEYS-----"),
        good.replace("-----BEGIN OPENSSH PRIVATE KEY-----", "-----BEGIN OPENSSH PRIVATE KEY----- x"),
    ] {
        assert_eq!(import_openssh(odd.as_bytes(), b"").map(|_| ()).unwrap_err(), KeyError::Malformed, "{odd:?}");
    }
}

/// L5: a file asking for more bcrypt rounds than 256 is refused at once, and says why
#[test]
fn l5_bcrypt_rounds_are_bounded_at_256() {
    for rounds in [257u32, 4096] {
        let mut options = Vec::new();
        ssh_string(&mut options, &[7u8; 16]);
        options.extend_from_slice(&rounds.to_be_bytes());
        let file = v1_file("aes256-ctr", "bcrypt", &options, b"public", &[0u8; 32], b"");
        assert_eq!(refused_within(&file, b"x", Duration::from_millis(200)), KeyError::TooCostly, "{rounds}");
    }
}
