//! One-time codes against the RFCs' own vectors and values worked out from the rules, and Base32, typed secrets and links
//! with values of this file's own.

use amethystora_vault_core::items::{Account, OtpKind, Secret};
use amethystora_vault_core::otp::{self, code_at};
use amethystora_vault_core::otpauth::{link_for_qr, read_link, write_link};

// The RFCs' shared secrets: "12345678901234567890" and its longer forms for SHA-256 and SHA-512
const RFC_SHA1: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
const RFC_SHA256: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZA";
const RFC_SHA512: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNA";

fn totp(secret: &str, algorithm: &str, digits: i64, period: i64) -> Account {
    Account { secret: Secret::new(secret), algorithm: algorithm.to_string(), digits, period, ..Account::default() }
}

#[test]
fn rfc_4226_appendix_d() {
    let want = ["755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583", "399871", "520489"];
    for (count, code) in want.iter().enumerate() {
        let account = Account {
            kind: Some(OtpKind::Hotp),
            secret: Secret::new(RFC_SHA1),
            counter: Some(count as i64),
            ..Account::default()
        };
        // The clock does not come into it
        assert_eq!(code_at(&account, 999_999_999, 0).as_str(), *code);
        if count > 0 {
            let before = Account { counter: Some(count as i64 - 1), ..account };
            assert_eq!(code_at(&before, 0, 1).as_str(), *code, "the next code is the counter's");
        }
    }
}

#[test]
fn rfc_6238_appendix_b() {
    let table: [(i64, &str, &str, &str); 6] = [
        (59, "94287082", "46119246", "90693936"),
        (1111111109, "07081804", "68084774", "25091201"),
        (1111111111, "14050471", "67062674", "99943326"),
        (1234567890, "89005924", "91819424", "93441116"),
        (2000000000, "69279037", "90698825", "38618901"),
        (20000000000, "65353130", "77737706", "47863826"),
    ];
    for (t, sha1, sha256, sha512) in table {
        assert_eq!(code_at(&totp(RFC_SHA1, "SHA1", 8, 30), t, 0).as_str(), sha1);
        assert_eq!(code_at(&totp(RFC_SHA256, "SHA256", 8, 30), t, 0).as_str(), sha256);
        assert_eq!(code_at(&totp(RFC_SHA512, "SHA512", 8, 30), t, 0).as_str(), sha512);
    }
}

#[test]
fn six_digits_are_the_last_six() {
    assert_eq!(code_at(&totp(RFC_SHA1, "SHA1", 6, 30), 59, 0).as_str(), "287082");
    assert_eq!(code_at(&totp(RFC_SHA1, "SHA1", 6, 30), 59, 0).grouped(), "287 082");
    assert_eq!(code_at(&totp(RFC_SHA1, "SHA1", 8, 30), 59, 0).grouped(), "9428 7082");
    // The next window, at the same moment
    assert_eq!(code_at(&totp(RFC_SHA1, "SHA1", 6, 30), 29, 1).as_str(), "287082");
}

#[test]
fn hmac_truncation_uses_the_hash_in_use() {
    // Worked out with Node's crypto while writing the spec, section 3
    assert_eq!(code_at(&totp("JBSWY3DPEHPK3PXP", "SHA256", 8, 60), 1700000000, 0).as_str(), "71205722");
    // The other two hashes, from openssl's own HMAC, so that the two by hand here are not only self-consistent
    for (algorithm, digest) in [("SHA224", openssl::hash::MessageDigest::sha224()), ("SHA384", openssl::hash::MessageDigest::sha384())] {
        let key = otp::base32_decode("JBSWY3DPEHPK3PXP").unwrap();
        let pkey = openssl::pkey::PKey::hmac(&key).unwrap();
        let mut signer = openssl::sign::Signer::new(digest, &pkey).unwrap();
        signer.update(&5i64.to_be_bytes()).unwrap();
        let mac = signer.sign_to_vec().unwrap();
        let at = (mac[mac.len() - 1] & 15) as usize;
        let value = u32::from_be_bytes([mac[at], mac[at + 1], mac[at + 2], mac[at + 3]]) & 0x7fff_ffff;
        let want = format!("{:06}", value % 1_000_000);
        assert_eq!(code_at(&totp("JBSWY3DPEHPK3PXP", algorithm, 6, 30), 150, 0).as_str(), want, "{algorithm}");
    }
}

#[test]
fn steam_guard() {
    // Worked out in the spec, section 3: the remainders in turn, least significant first
    let steam = |period| Account { kind: Some(OtpKind::Steam), secret: Secret::new("JBSWY3DPEHPK3PXP"), period, ..Account::default() };
    assert_eq!(code_at(&steam(30), 0, 0).as_str(), "VH8YJ");
    assert_eq!(code_at(&steam(30), 1700000000, 0).as_str(), "2KM2P");
    // Algorithm and digits mean nothing to it
    let odd = Account { algorithm: "SHA512".to_string(), digits: 8, ..steam(30) };
    assert_eq!(code_at(&odd, 1700000000, 0).as_str(), "2KM2P");
    assert_eq!(code_at(&odd, 1700000000, 0).grouped(), "2KM2P");
}

#[test]
fn an_unusable_account_gets_dashes() {
    for (account, dashes) in [
        (totp("not base32!", "SHA1", 6, 30), "------"),
        (totp(RFC_SHA1, "MD5", 8, 30), "--------"),
        (totp(RFC_SHA1, "SHA3-256", 6, 30), "------"),
        (totp(RFC_SHA1, "SHA1", 6, 0), "------"),
        (totp(RFC_SHA1, "SHA1", 6, -30), "------"),
        (totp("", "SHA1", 6, 30), "------"),
        (totp("A", "SHA1", 6, 30), "------"),
        (Account { kind: Some(OtpKind::Steam), secret: Secret::new("1"), ..Account::default() }, "-----"),
    ] {
        let code = code_at(&account, 59, 0);
        assert_eq!(code.as_str(), dashes);
        assert!(!code.is_usable());
    }
    assert_eq!(otp::group_code("------"), "--- ---");
    // A long secret is not a reason to make a buffer: past what the doors take, it makes no code
    assert_eq!(code_at(&totp(&"A".repeat(2000), "SHA1", 6, 30), 59, 0).as_str(), "------");
}

#[test]
fn nothing_prints_a_secret_or_a_code() {
    let account = totp("JBSWY3DPEHPK3PXP", "SHA1", 6, 30);
    let printed = format!("{account:?} {:?}", code_at(&account, 59, 0));
    assert!(!printed.contains("JBSW"), "{printed}");
    assert!(!printed.contains(code_at(&account, 59, 0).as_str()));
}

#[test]
fn seconds_left() {
    assert_eq!(otp::seconds_left(30, 0), 30);
    assert_eq!(otp::seconds_left(30, 29_999), 1);
    assert_eq!(otp::seconds_left(30, 30_000), 30);
    assert_eq!(otp::seconds_left(0, 30_000), 0);
}

#[test]
fn base32_reading() {
    // RFC 4648's own, then section 4's rules on them
    for (text, bytes) in [
        ("", ""),
        ("MY", "f"),
        ("MZXQ", "fo"),
        ("MZXW6", "foo"),
        ("MZXW6YQ", "foob"),
        ("MZXW6YTB", "fooba"),
        ("MZXW6YTBOI", "foobar"),
        ("MZXW6===", "foo"),
        ("mzxw6", "foo"),
        ("  MZX W6  ", "foo"),
    ] {
        assert_eq!(otp::base32_decode(text).as_deref().map(|b| b.as_slice()), Some(bytes.as_bytes()), "{text}");
    }
    // Bits past the last whole byte go, even when they are not zero
    assert_eq!(otp::base32_decode("MZXW7").as_deref().map(|b| b.as_slice()), Some(&b"foo"[..]));
    assert_eq!(otp::base32_decode("A").unwrap().len(), 0);
    // Full Unicode upper case: a long s is an S, and a sharp s two of them
    assert_eq!(otp::base32_decode("ſſ").unwrap().as_slice(), otp::base32_decode("SS").unwrap().as_slice());
    assert_eq!(otp::base32_decode("ßA").unwrap().as_slice(), otp::base32_decode("SSA").unwrap().as_slice());
    // Errors
    for bad in ["M=ZXW6", "MZXW1", "MZ\tXW6", "MZ-XW6", "é", "\u{212A}A"] {
        assert!(otp::base32_decode(bad).is_none(), "{bad}");
    }
}

#[test]
fn base32_writing() {
    assert_eq!(otp::base32_encode(b"").as_str(), "");
    assert_eq!(otp::base32_encode(b"f").as_str(), "MY");
    assert_eq!(otp::base32_encode(b"foob").as_str(), "MZXW6YQ");
    assert_eq!(otp::base32_encode(b"foobar").as_str(), "MZXW6YTBOI");
    assert_eq!(otp::canonical_secret("mzxw7==").as_ref().map(Secret::as_str), Some("MZXW6"));
}

#[test]
fn typed_secrets() {
    let normal = otp::normalize_typed("mzxw 6ytb-oizz\u{a0}zz\u{3000}zz");
    assert_eq!(normal.as_str(), "MZXW6YTBOIZZZZZZ");
    assert!(otp::typed_secret_is_valid(&normal));
    assert!(otp::typed_secret_is_valid("MZXW6YTBOIZZZZZZ=="));
    // 15 characters are 9 bytes
    assert!(!otp::typed_secret_is_valid("MZXW6YTBOIZZZZZ"));
    assert!(!otp::typed_secret_is_valid("MZXW6YTBOIZZZZZZ=A"));
    assert!(!otp::typed_secret_is_valid(""));
    assert_eq!(otp::normalize_typed("--- -").as_str(), "");
    // U+0085 is no JavaScript space, so it stays and spoils the secret
    assert!(!otp::typed_secret_is_valid(&otp::normalize_typed("MZXW6YTB\u{85}OIZZZZZZ")));
    assert_eq!(otp::group_secret("MZXW6YTBOIZZZZZZ").as_str(), "MZXW 6YTB OIZZ ZZZZ");
    assert_eq!(otp::group_secret("MZXW6").as_str(), "MZXW 6");
    // Every character counts towards a group, as BackMyCodes counts them, and the ends are trimmed
    assert_eq!(otp::group_secret("  MZXW6 \u{a0}").as_str(), "MZ XW6");
}

#[test]
fn structured_secrets_are_more_lenient() {
    assert_eq!(otp::structured_secret("mz xw====").as_ref().map(Secret::as_str), Some("MZXW"));
    assert_eq!(otp::structured_secret("MY").as_ref().map(Secret::as_str), Some("MY"));
    // A character alone holds no byte
    assert!(otp::structured_secret("M").is_none());
    assert!(otp::structured_secret("").is_none());
    assert!(otp::structured_secret("MZ1").is_none());
}

// Links. The names here are this file's own; each line holds one rule of section 5.

const KEY: &str = "ORSXG5DTMVRXEZLU";

fn steam_link(link: &str) -> Account {
    let account = read_link(link).unwrap_or_else(|| panic!("refused: {link}"));
    assert_eq!(account.kind, Some(OtpKind::Steam), "{link}");
    assert_eq!((account.algorithm.as_str(), account.digits), ("SHA1", 6), "{link}");
    account
}

#[test]
fn every_steam_spelling() {
    // Bitwarden's
    let a = steam_link(&format!("steam://{}", KEY.to_lowercase()));
    assert_eq!((a.issuer.as_str(), a.label.as_str(), a.secret.as_str()), ("Steam", "", KEY));
    assert_eq!(steam_link(&format!("STEAM://{KEY}==")).secret.as_str(), KEY);
    for bad in ["steam://", "steam://ab1", "steam://ab=c", "steam://A"] {
        assert!(read_link(bad).is_none(), "{bad}");
    }
    // Aegis's
    let a = steam_link(&format!("otpauth://steam/Valve:player?secret={KEY}&issuer=Valve&digits=5"));
    assert_eq!((a.issuer.as_str(), a.label.as_str()), ("Valve", "player"));
    let a = steam_link(&format!("otpauth://STEAM/player?secret={KEY}&period=45&algorithm=SHA256&digits=8"));
    assert_eq!(a.period, 45);
    assert!(read_link(&format!("otpauth://steam/x?secret={KEY}&digits=0")).is_none());
    assert!(read_link(&format!("otpauth://steam/x?secret={KEY}&algorithm=SHA3-256")).is_none());
    // KeePassXC's: the name exactly, the value in any letter case
    steam_link(&format!("otpauth://totp/x?secret={KEY}&encoder=steam"));
    steam_link(&format!("otpauth://totp/x?secret={KEY}&encoder=STEAM"));
    steam_link(&format!("otpauth://totp/x?encoder=st%65am&secret={KEY}&digits=7"));
    assert_eq!(read_link(&format!("otpauth://totp/x?secret={KEY}&Encoder=steam")).unwrap().kind, Some(OtpKind::Totp));
    assert_eq!(read_link(&format!("otpauth://totp/x?secret={KEY}&encoder=+steam")).unwrap().kind, Some(OtpKind::Totp));
    // A 5-digit code from Steam: the issuer, or with none the label
    steam_link(&format!("otpauth://totp/x?secret={KEY}&issuer=STEAM&digits=5"));
    let a = steam_link(&format!("otpauth://totp/steam?secret={KEY}&digits=5&period=20"));
    assert_eq!((a.issuer.as_str(), a.label.as_str(), a.period), ("", "steam", 20));
    assert!(read_link(&format!("otpauth://totp/Other:steam?secret={KEY}&digits=5")).is_none());
    // HOTP never is
    assert!(read_link(&format!("otpauth://hotp/Steam:x?secret={KEY}&counter=1&digits=5")).is_none());
    assert_eq!(read_link(&format!("otpauth://HOTP/x?secret={KEY}&counter=3&encoder=steam")).unwrap().kind, Some(OtpKind::Hotp));
}

#[test]
fn reading_links() {
    let a = read_link(&format!("  OTPAUTH://TOTP/My%20Co:me@example.org?SECRET={}&ALGORITHM=sha512&Digits=8&issuer=My%20Co\n", KEY.to_lowercase())).unwrap();
    assert_eq!(a.kind, Some(OtpKind::Totp));
    assert_eq!((a.issuer.as_str(), a.label.as_str(), a.secret.as_str(), a.algorithm.as_str(), a.digits, a.period), ("My Co", "me@example.org", KEY, "SHA512", 8, 30));
    // The issuer parameter wins over the prefix unless it is empty; literal spaces after the colon go, escaped ones stay
    assert_eq!(read_link(&format!("otpauth://totp/P:  n?secret={KEY}&issuer=I")).unwrap().issuer, "I");
    let a = read_link(&format!("otpauth://totp/P:  n?secret={KEY}&issuer=")).unwrap();
    assert_eq!((a.issuer.as_str(), a.label.as_str()), ("P", "n"));
    assert_eq!(read_link(&format!("otpauth://totp/P:%20n?secret={KEY}")).unwrap().label, " n");
    // SHA-224 and SHA-384 are the plan's; SHA-3 is known and makes no code
    assert_eq!(read_link(&format!("otpauth://totp/x?secret={KEY}&algorithm=SHA224")).unwrap().algorithm, "SHA224");
    assert_eq!(read_link(&format!("otpauth://totp/x?secret={KEY}&algorithm=sha384")).unwrap().algorithm, "SHA384");
    // The secret is written canonically
    assert_eq!(read_link("otpauth://totp/x?secret=mzxw7===").unwrap().secret.as_str(), "MZXW6");
    // A label with `+` keeps it; a `#` is part of a value
    assert_eq!(read_link(&format!("otpauth://totp/a+b%23?secret={KEY}")).unwrap().label, "a+b#");
    // HOTP reads a counter, and ignores period; TOTP ignores counter
    let a = read_link(&format!("otpauth://hotp/x?secret={KEY}&counter=+7&period=abc")).unwrap();
    assert_eq!((a.counter, a.period), (Some(7), 30));
    assert_eq!(read_link(&format!("otpauth://totp/x?secret={KEY}&counter=abc")).unwrap().counter, None);
    // A parameter later wins
    assert_eq!(read_link(&format!("otpauth://totp/x?digits=6&secret={KEY}&digits=8")).unwrap().digits, 8);
}

#[test]
fn refused_links() {
    for bad in [
        String::new(),
        "not a link".to_string(),
        "https://example.org/".to_string(),
        format!("otpauth://totp/?secret={KEY}"),
        format!("otpauth://totp/a\nb?secret={KEY}"),
        format!("otpauth://totp/%zz?secret={KEY}"),
        format!("otpauth://totp/%C3?secret={KEY}"),
        format!("otpauth://totp/x?secret={KEY}&"),
        format!("otpauth://totp/x?&secret={KEY}"),
        format!("otpauth://totp/x?secret={KEY}&&digits=6"),
        format!("otpauth://totp/x?secret={KEY}&novalue"),
        format!("otpauth://totp/x?secret={KEY}&image=https://x/a.png?size=2"),
        format!("otpauth://totp/x?secret={KEY}&other=%zz"),
        "otpauth://totp/x?digits=6".to_string(),
        "otpauth://totp/x?secret=".to_string(),
        "otpauth://totp/x?secret=M".to_string(),
        "otpauth://totp/x?secret=MZ1".to_string(),
        format!("otpauth://totp/x?secret={KEY}&algorithm=MD5"),
        format!("otpauth://totp/x?secret={KEY}&algorithm=SHA-256"),
        format!("otpauth://totp/x?secret={KEY}&algorithm=SHA3-512"),
        format!("otpauth://totp/x?secret={KEY}&digits=7"),
        format!("otpauth://totp/x?secret={KEY}&digits=07"),
        format!("otpauth://totp/x?secret={KEY}&period=0"),
        format!("otpauth://totp/x?secret={KEY}&period=-30"),
        format!("otpauth://totp/x?secret={KEY}&period=99999999999999999999"),
        format!("otpauth://hotp/x?secret={KEY}"),
        format!("otpauth://hotp/x?secret={KEY}&counter=abc"),
        format!("otpauth://hotp/x?secret={KEY}&counter=-5"),
        format!("otpauth://hotp/x?secret={KEY}&counter=9007199254740992"),
        format!("otpauth://hotp/x?secret={KEY}&counter=1&digits=7"),
        format!("otpauth://yotp/x?secret={KEY}"),
        format!("otpauth://totp/{}?secret={KEY}", "x".repeat(9000)),
    ] {
        assert!(read_link(&bad).is_none(), "{bad}");
    }
}

fn account(kind: Option<OtpKind>, issuer: &str, label: &str, secret: &str, algorithm: &str, digits: i64, period: i64) -> Account {
    Account {
        kind,
        issuer: issuer.to_string(),
        label: label.to_string(),
        secret: Secret::new(secret),
        algorithm: algorithm.to_string(),
        digits,
        period,
        ..Account::default()
    }
}

#[test]
fn writing_links_byte_for_byte() {
    let written = |a: &Account| write_link(a).unwrap().as_str().to_string();
    assert_eq!(
        written(&account(None, "Co", "me", KEY, "SHA1", 6, 30)),
        format!("otpauth://totp/Co:me?issuer=Co&secret={KEY}&algorithm=SHA1&digits=6&period=30")
    );
    assert_eq!(written(&account(None, "", "me", KEY, "SHA1", 6, 30)), format!("otpauth://totp/me?secret={KEY}&algorithm=SHA1&digits=6&period=30"));
    // encodeURIComponent, and what it leaves alone
    assert_eq!(
        written(&account(None, "é:☕", "a b&?=#/%+@", KEY, "sha-256", 8, 60)),
        format!("otpauth://totp/%C3%A9%3A%E2%98%95:a%20b%26%3F%3D%23%2F%25%2B%40?issuer=%C3%A9%3A%E2%98%95&secret={KEY}&algorithm=SHA256&digits=8&period=60")
    );
    assert_eq!(
        written(&account(None, "a!b~c*d'e(f)g", "x-y_z.w", KEY, "SHA512", 8, 30)),
        format!("otpauth://totp/a!b~c*d'e(f)g:x-y_z.w?issuer=a!b~c*d'e(f)g&secret={KEY}&algorithm=SHA512&digits=8&period=30")
    );
    let mut hotp = account(Some(OtpKind::Hotp), "Co", "me", KEY, "SHA1", 6, 30);
    assert_eq!(written(&hotp), format!("otpauth://hotp/Co:me?issuer=Co&secret={KEY}&algorithm=SHA1&digits=6&counter=0"));
    hotp.counter = Some(7);
    assert_eq!(written(&hotp), format!("otpauth://hotp/Co:me?issuer=Co&secret={KEY}&algorithm=SHA1&digits=6&counter=7"));
    // Steam: always 5, whatever it held
    assert_eq!(
        written(&account(Some(OtpKind::Steam), "", "me", KEY, "SHA256", 8, 60)),
        format!("otpauth://steam/me?secret={KEY}&algorithm=SHA256&digits=5&period=60")
    );
    // The secret canonical, and what cannot be written refused
    assert_eq!(written(&account(None, "", "me", "mzxw7==", "SHA1", 6, 30)), "otpauth://totp/me?secret=MZXW6&algorithm=SHA1&digits=6&period=30");
    assert!(write_link(&account(None, "", "me", "not base32!", "SHA1", 6, 30)).is_none());
    assert!(write_link(&account(None, "", "me", KEY, "MD5", 6, 30)).is_none());
}

#[test]
fn writing_and_reading_again() {
    let cases = [
        account(Some(OtpKind::Totp), "Co Ltd", "me@example.org", KEY, "SHA256", 8, 60),
        account(Some(OtpKind::Totp), "", "only me", KEY, "SHA1", 6, 30),
        account(Some(OtpKind::Steam), "Steam", "player", KEY, "SHA1", 6, 30),
        Account { counter: Some(41), ..account(Some(OtpKind::Hotp), "Co", "me", KEY, "SHA512", 6, 30) },
        // SHA-224 goes through this reader, unlike BackMyCodes'
        account(Some(OtpKind::Totp), "Co", "me", KEY, "SHA224", 6, 30),
    ];
    for original in cases {
        let link = write_link(&original).unwrap();
        let back = read_link(link.as_str()).unwrap();
        assert_eq!(back.kind, original.kind);
        assert_eq!(back.label, original.label);
        assert_eq!(back.secret.as_str(), original.secret.as_str());
        assert_eq!((back.algorithm.as_str(), back.digits, back.period, back.counter), (original.algorithm.as_str(), original.digits, original.period, original.counter));
    }
}

#[test]
fn an_empty_label_comes_back_empty() {
    // The writer stays byte for byte, and reading fixes it (decision 5)
    let a = account(None, "Co", "", KEY, "SHA1", 6, 30);
    let link = write_link(&a).unwrap();
    assert_eq!(link.as_str(), format!("otpauth://totp/Co:?issuer=Co&secret={KEY}&algorithm=SHA1&digits=6&period=30"));
    let back = read_link(link.as_str()).unwrap();
    assert_eq!((back.issuer.as_str(), back.label.as_str()), ("Co", ""));
    // A bare steam:// account, the same way
    let steam = read_link(&format!("steam://{KEY}")).unwrap();
    let back = read_link(write_link(&steam).unwrap().as_str()).unwrap();
    assert_eq!((back.kind, back.issuer.as_str(), back.label.as_str()), (Some(OtpKind::Steam), "Steam", ""));
    // A name that only looks like that stays, since its own issuer is another
    assert_eq!(read_link(&format!("otpauth://totp/Co:?secret={KEY}&issuer=Other")).unwrap().label, "Co:");
    assert_eq!(read_link(&format!("otpauth://totp/Co:?secret={KEY}")).unwrap().label, "Co:");
    // No names at all: the writer writes what nothing reads, so no QR is drawn of it
    let nameless = account(None, "", "", KEY, "SHA1", 6, 30);
    assert!(write_link(&nameless).is_some());
    assert!(link_for_qr(&nameless).is_none());
    assert!(link_for_qr(&a).is_some());
}

#[test]
fn the_manual_form() {
    use amethystora_vault_core::otp::Algorithm;
    use amethystora_vault_core::otpauth::{ManualEntry, ManualError, manual_account};
    let good = ManualEntry { kind: OtpKind::Totp, issuer: " Co ", label: "me", key: "orsx g5dt-mvrx ezlu", algorithm: Algorithm::Sha256, digits: 8, period: 60, counter: 0 };
    let a = manual_account(&good).unwrap();
    assert_eq!((a.kind, a.issuer.as_str(), a.secret.as_str(), a.algorithm.as_str(), a.digits, a.period, a.counter), (Some(OtpKind::Totp), "Co", KEY, "SHA256", 8, 60, None));
    let hotp = manual_account(&ManualEntry { kind: OtpKind::Hotp, counter: 5, period: 0, ..good_entry() }).unwrap();
    assert_eq!((hotp.counter, hotp.period), (Some(5), 30));
    // Steam takes only the key and the names, whatever else the form held
    let steam = manual_account(&ManualEntry { kind: OtpKind::Steam, digits: 7, period: 0, algorithm: Algorithm::Sha512, ..good_entry() }).unwrap();
    assert_eq!((steam.kind, steam.algorithm.as_str(), steam.digits, steam.period), (Some(OtpKind::Steam), "SHA1", 6, 30));
    // Each says what it says, in this order
    let refusals = [
        (ManualEntry { key: " - ", issuer: "", label: "", ..good_entry() }, ManualError::KeyRequired),
        (ManualEntry { key: "ORSXG5DT", issuer: "", label: "", ..good_entry() }, ManualError::KeyNotBase32),
        (ManualEntry { issuer: " ", label: "\u{a0}", period: 0, ..good_entry() }, ManualError::NameRequired),
        (ManualEntry { period: 14, ..good_entry() }, ManualError::PeriodOutOfRange),
        (ManualEntry { period: 301, ..good_entry() }, ManualError::PeriodOutOfRange),
        (ManualEntry { kind: OtpKind::Hotp, counter: -1, ..good_entry() }, ManualError::CounterOutOfRange),
        (ManualEntry { kind: OtpKind::Hotp, counter: 1 << 53, ..good_entry() }, ManualError::CounterOutOfRange),
        (ManualEntry { digits: 7, ..good_entry() }, ManualError::DigitsOutOfRange),
    ];
    for (entry, why) in refusals {
        assert_eq!(manual_account(&entry).unwrap_err(), why);
    }
}

fn good_entry() -> amethystora_vault_core::otpauth::ManualEntry<'static> {
    amethystora_vault_core::otpauth::ManualEntry {
        kind: OtpKind::Totp,
        issuer: "Co",
        label: "me",
        key: KEY,
        algorithm: amethystora_vault_core::otp::Algorithm::Sha1,
        digits: 6,
        period: 30,
        counter: 0,
    }
}
