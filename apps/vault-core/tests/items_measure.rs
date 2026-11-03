//! What each of the budgets costs, printed. Not a test of anything by itself: run it in release, one at a time:
//! `cargo test --release -p amethystora-vault-core --test items_measure -- --ignored --nocapture --test-threads=1`

mod items_support;
use items_support::*;

use amethystora_vault_core::bmc::{Added, Collector, read_export, write_bmc1, write_export};
use amethystora_vault_core::generate::{Charsets, estimate_bits, password, words};
use amethystora_vault_core::items::{Account, BackupCode, BackupService, OtpKind, Secret};
use amethystora_vault_core::otp::code_at;
use std::time::Instant;

#[global_allocator]
static COUNTING: Counting = Counting;

fn high_water() -> String {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    status.lines().find(|l| l.starts_with("VmHWM")).unwrap_or("").split_whitespace().nth(1).unwrap_or("?").to_string()
}

fn report<T>(what: &str, rounds: usize, mut run: impl FnMut() -> T) {
    let start = Instant::now();
    let mut peak = 0;
    let mut allocations = 0;
    for _ in 0..rounds {
        let (_, p, a) = measured(&mut run);
        peak = peak.max(p);
        allocations = a;
    }
    let each = start.elapsed() / rounds as u32;
    println!("{what:<44} {each:>12.3?} each, heap peak {peak:>10} bytes, {allocations} allocations, VmHWM {} kB", high_water());
}

fn accounts(n: usize) -> Vec<Account> {
    (0..n)
        .map(|i| Account {
            id: format!("{i:08}-1111-4111-8111-111111111111"),
            kind: Some(OtpKind::Totp),
            issuer: format!("Service number {i}"),
            label: format!("me{i}@example.org"),
            secret: Secret::new("ORSXG5DTMVRXEZLUMZXW6YTBOI"),
            added_at: 1_700_000_000_000,
            ..Account::default()
        })
        .collect()
}

#[test]
#[ignore]
fn budgets() {
    let totp = Account { secret: Secret::new("ORSXG5DTMVRXEZLU"), ..Account::default() };
    let sha512 = Account { secret: Secret::new("ORSXG5DTMVRXEZLU"), algorithm: "SHA512".to_string(), digits: 8, ..Account::default() };
    let steam = Account { kind: Some(OtpKind::Steam), secret: Secret::new("ORSXG5DTMVRXEZLU"), ..Account::default() };
    report("one code, TOTP SHA-1", 100_000, || code_at(&totp, 1_700_000_000, 0));
    report("one code, TOTP SHA-512", 100_000, || code_at(&sha512, 1_700_000_000, 0));
    report("one code, Steam", 100_000, || code_at(&steam, 1_700_000_000, 0));
    report("a password of 20", 10_000, || password(20, Charsets::default()).unwrap());
    report("a password of 128", 1_000, || password(128, Charsets::default()).unwrap());
    report("six words", 1_000, || words(6, '-').unwrap());
    report("an estimate for a typed password", 1_000, || estimate_bits("Tr7#kQ9!mZ2@vX4$pL6%"));

    let list = accounts(100);
    let services = [BackupService { service_name: "S".to_string(), codes: vec![BackupCode::new("abcd-1234"); 10], ..BackupService::default() }];
    let password_text = "a long enough password";
    let chunks = write_bmc1(&list, &services, password_text).unwrap();
    println!("100 accounts make {} chunks of BMC1", chunks.len());
    let mut collector = Collector::new();
    for chunk in &chunks {
        assert_eq!(collector.add(chunk.as_str()), Added::New);
    }
    report("opening a BMC1 of 100 accounts", 3, || collector.read(password_text, 1).unwrap());
    report("writing a BMC1 of 100 accounts", 3, || write_bmc1(&list, &services, password_text).unwrap());
    let file = write_export(&list, &services, password_text, 1).unwrap();
    println!("an export of 100 accounts is {} bytes", file.len());
    report("opening an export of 100 accounts", 3, || read_export(file.as_bytes(), password_text).unwrap());
    report("writing an export of 100 accounts", 3, || write_export(&list, &services, password_text, 1).unwrap());
}
