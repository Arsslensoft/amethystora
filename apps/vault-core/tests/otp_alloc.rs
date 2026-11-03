//! A code is asked for once a second per account on screen, so making one allocates nothing. Checked against the tools
//! the image's own testing has, where they are installed.

mod items_support;
use items_support::*;

use amethystora_vault_core::items::{Account, OtpKind, Secret};
use amethystora_vault_core::otp::code_at;
use amethystora_vault_core::otpauth::read_link;
use std::process::Command;

#[global_allocator]
static COUNTING: Counting = Counting;

#[test]
fn a_code_allocates_nothing() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let accounts = [
        Account { secret: Secret::new("JBSWY3DPEHPK3PXP"), ..Account::default() },
        Account { secret: Secret::new("jbsw y3dp ehpk 3pxp"), algorithm: "SHA512".to_string(), digits: 8, ..Account::default() },
        Account { kind: Some(OtpKind::Steam), secret: Secret::new("JBSWY3DPEHPK3PXP"), ..Account::default() },
        Account { kind: Some(OtpKind::Hotp), secret: Secret::new("ſtraße"), counter: Some(3), ..Account::default() },
    ];
    for account in &accounts {
        let (code, _, allocations) = measured(|| code_at(account, 1_700_000_000, 0));
        assert!(code.is_usable());
        assert_eq!(allocations, 0);
    }
}

#[test]
fn oathtool_agrees() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let run = |args: &[&str]| Command::new("oathtool").args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    if !need("oathtool") {
        return;
    }
    let secret = "ORSXG5DTMVRXEZLUMZXW6YTBOI";
    for (hash, name) in [("sha1", "SHA1"), ("sha256", "SHA256"), ("sha512", "SHA512")] {
        for digits in [6, 8] {
            for (t, stamp) in [(59, "1970-01-01 00:00:59 UTC"), (1_700_000_000, "2023-11-14 22:13:20 UTC")] {
                for period in [30, 60] {
                    let want = run(&[
                        &format!("--totp={hash}"),
                        "-b",
                        secret,
                        &format!("--now={stamp}"),
                        "-d",
                        &digits.to_string(),
                        "-s",
                        &period.to_string(),
                    ])
                    .unwrap();
                    let account = Account { secret: Secret::new(secret), algorithm: name.to_string(), digits, period, ..Account::default() };
                    assert_eq!(code_at(&account, t, 0).as_str(), want, "{name} {digits} {t} {period}");
                }
            }
        }
    }
    for counter in [0, 1, 41, 1 << 33] {
        let want = run(&["--hotp", "-b", secret, "-c", &counter.to_string()]).unwrap();
        let account = Account { kind: Some(OtpKind::Hotp), secret: Secret::new(secret), counter: Some(counter), ..Account::default() };
        assert_eq!(code_at(&account, 0, 0).as_str(), want, "{counter}");
    }
}

#[test]
fn links_fit_a_qr_code_at_level_m() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    use std::io::Write;
    let draws = |text: &str| {
        let Ok(mut child) = Command::new("qrencode").args(["-l", "M", "-8", "-o", "/dev/null"]).stdin(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn() else {
            return None;
        };
        child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
        Some(child.wait().unwrap().success())
    };
    let link = amethystora_vault_core::otpauth::write_link(&read_link("otpauth://totp/Co:me?secret=ORSXG5DTMVRXEZLU&issuer=Co").unwrap()).unwrap();
    if need("qrencode") && let Some(drawn) = draws(link.as_str()) {
        assert!(drawn);
    }
}
