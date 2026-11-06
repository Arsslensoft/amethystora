//! L9 of the Vault's key review, alone in its binary: it looks for secret numbers in this whole process's memory, so that another test's
//! allocations (the others of keys_review.rs run beside it, in threads) must not be able to leave one about or move one (K11 of
//! plans/vault/work/verify-pki.md: it failed once in about thirty runs with them beside it).
#![allow(dead_code)]

use amethystora_vault_core::keys::import_openssh;
use base64::Engine;
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
/// Where in this process's writable memory `masked` (a pattern xor `MASK`, so that the pattern is not itself in memory) is
const MASK: u8 = 0x5a;

fn in_memory(masked: &[u8]) -> usize {
    use std::os::unix::fs::FileExt;
    let maps = std::fs::read_to_string("/proc/self/maps").unwrap();
    let mem = std::fs::File::open("/proc/self/mem").unwrap();
    let mut found = 0;
    let mut chunk = vec![0u8; 1 << 20];
    for line in maps.lines() {
        let mut fields = line.split_whitespace();
        let range = fields.next().unwrap();
        if !fields.next().unwrap().starts_with("rw") {
            continue;
        }
        let (start, end) = range.split_once('-').unwrap();
        let (mut at, end) = (u64::from_str_radix(start, 16).unwrap(), u64::from_str_radix(end, 16).unwrap());
        while at < end {
            let length = ((end - at) as usize).min(chunk.len());
            if mem.read_exact_at(&mut chunk[..length], at).is_err() {
                break;
            }
            found += chunk[..length].windows(masked.len()).filter(|window| window.iter().zip(masked).all(|(a, b)| a ^ MASK == *b)).count();
            at += length.saturating_sub(masked.len() - 1).max(1) as u64;
        }
    }
    chunk.fill(0);
    found
}

/// The middle of a number as OpenSSL keeps it, its 64-bit words least significant first (the first words of a freed block are
/// the allocator's), masked
fn masked_limbs(big_endian: &[u8], from: usize, length: usize) -> Vec<u8> {
    let little: Vec<u8> = big_endian.iter().rev().copied().collect();
    little[from..from + length].iter().map(|byte| byte ^ MASK).collect()
}

fn ssh_mpint(out: &mut Vec<u8>, number: &[u8]) {
    let mut signed = if number[0] & 0x80 != 0 { vec![0u8] } else { Vec::new() };
    signed.extend_from_slice(number);
    ssh_string(out, &signed);
}

fn pad_to_block(out: &mut Vec<u8>) {
    let mut pad = 1u8;
    while !out.len().is_multiple_of(8) {
        out.push(pad);
        pad += 1;
    }
}

/// L9: the numbers read from an OpenSSH file are wiped when freed. `p - 1` and `q - 1` give the factors away, and the EC
/// scalar is the key; OpenSSL frees an ordinary big number without clearing it. The test looks for them in the process's memory.
#[test]
fn l9_secret_numbers_are_wiped_when_freed() {
    let rsa = Rsa::generate(2048).unwrap();
    // p and q are odd, so subtracting one needs no borrow
    let minus_one = |number: Vec<u8>| {
        let mut number = number;
        *number.last_mut().unwrap() -= 1;
        number
    };
    let (p1, q1) = (minus_one(rsa.p().unwrap().to_vec()), minus_one(rsa.q().unwrap().to_vec()));
    let patterns = [masked_limbs(&p1, 16, 32), masked_limbs(&q1, 16, 32)];
    let file = v1_file(
        "none",
        "none",
        b"",
        &rsa_public(&rsa),
        &rsa_private([&rsa.n().to_vec(), &rsa.e().to_vec(), &rsa.d().to_vec(), &rsa.iqmp().unwrap().to_vec(), &rsa.p().unwrap().to_vec(), &rsa.q().unwrap().to_vec()]),
        b"",
    );

    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
    // A scalar with a zero in front is a byte short (one in 256), too short for the pattern's middle: draw again
    let ec = std::iter::repeat_with(|| EcKey::generate(&group).unwrap()).find(|ec| ec.private_key().num_bytes() == 32).unwrap();
    let scalar = ec.private_key().to_vec();
    let scalar_pattern = masked_limbs(&scalar, 16, 16);
    let mut context = openssl::bn::BigNumContext::new().unwrap();
    let point = ec.public_key().to_bytes(&group, openssl::ec::PointConversionForm::UNCOMPRESSED, &mut context).unwrap();
    let mut public = Vec::new();
    ssh_string(&mut public, b"ecdsa-sha2-nistp256");
    ssh_string(&mut public, b"nistp256");
    ssh_string(&mut public, &point);
    let mut private = vec![9u8; 8];
    ssh_string(&mut private, b"ecdsa-sha2-nistp256");
    ssh_string(&mut private, b"nistp256");
    ssh_string(&mut private, &point);
    ssh_mpint(&mut private, &scalar);
    ssh_string(&mut private, b"");
    pad_to_block(&mut private);
    let ec_file = v1_file("none", "none", b"", &public, &private, b"");
    drop((rsa, ec, context));

    for pattern in patterns.iter().chain([&scalar_pattern]) {
        assert_eq!(in_memory(pattern), 0, "the test's own setup left the number about");
    }
    import_openssh(&file, b"").unwrap();
    import_openssh(&ec_file, b"").unwrap();
    for (what, pattern) in [("p - 1", &patterns[0]), ("q - 1", &patterns[1]), ("the EC scalar", &scalar_pattern)] {
        assert_eq!(in_memory(pattern), 0, "{what} was freed without being wiped");
    }
}
