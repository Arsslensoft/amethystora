//! Backup codes, service names, coverage and the directory's lookup, with values of this file's own: each line holds a
//! rule of sections 6, 12 and 13.

use amethystora_vault_core::codes::{
    Directory, ServiceIndex, account_service_name, add_codes, coverage, fold, hue, is_low, next_unused, same_service, service_for, service_keys,
};
use amethystora_vault_core::items::{Account, BackupCode, BackupService, Secret};

fn list(codes: &[&str]) -> Vec<String> {
    codes.iter().map(|c| c.to_string()).collect()
}

fn secrets(codes: &[&str]) -> Vec<Secret> {
    codes.iter().map(|c| Secret::new(*c)).collect()
}

fn parse_codes(text: &str) -> Vec<String> {
    amethystora_vault_core::codes::parse_codes(text).codes.iter().map(|c| c.as_str().to_string()).collect()
}

fn parse_document_codes(text: &str) -> Vec<String> {
    amethystora_vault_core::codes::parse_document_codes(text).codes.iter().map(|c| c.as_str().to_string()).collect()
}

#[test]
fn reading_pasted_codes() {
    assert_eq!(parse_codes("qwert-11111\nasdfg-22222\r\nzxcvb-33333"), list(&["qwert-11111", "asdfg-22222", "zxcvb-33333"]));
    // Two short groups are one code
    assert_eq!(parse_codes("9876 5432\n8765 4321"), list(&["9876 5432", "8765 4321"]));
    // Two long ones are two
    assert_eq!(parse_codes("qwert-11111 asdfg-22222\nzxcvb-33333"), list(&["qwert-11111", "asdfg-22222", "zxcvb-33333"]));
    // Columns, by two spaces or a tab or a comma
    assert_eq!(parse_codes("9876 5432    8765 4321\n7654 3210\t6543 2109"), list(&["9876 5432", "8765 4321", "7654 3210", "6543 2109"]));
    assert_eq!(parse_codes("wwwwww, xxxxxx yyyyyy"), list(&["wwwwww", "xxxxxx", "yyyyyy"]));
    // Blank lines, spaces and repeats go, the first place kept
    assert_eq!(parse_codes("\n  code1  \n\ncode1\ncode2\n"), list(&["code1", "code2"]));
    assert_eq!(parse_codes(""), Vec::<String>::new());
}

#[test]
fn numbering_and_bullets_go_first() {
    // Decision 3
    assert_eq!(parse_codes("1. abcd-1234\n2. efgh-5678"), list(&["abcd-1234", "efgh-5678"]));
    assert_eq!(parse_codes("1) abcd-1234\n10) efgh-5678"), list(&["abcd-1234", "efgh-5678"]));
    assert_eq!(parse_codes("• abcd-1234\n- efgh-5678\n* ijkl-9012"), list(&["abcd-1234", "efgh-5678", "ijkl-9012"]));
    assert_eq!(parse_codes("1. 1234"), list(&["1234"]));
    assert_eq!(parse_codes("1. 1234 5678\n2. 2345 6789"), list(&["1234 5678", "2345 6789"]));
    // A column of numbered codes: each piece has its own number
    assert_eq!(parse_codes("1. 1234 5678    6. 8765 4321"), list(&["1234 5678", "8765 4321"]));
    // What only begins like a marker is a code
    assert_eq!(parse_codes("1.5abc\n-abc\n1234.abc"), list(&["1.5abc", "-abc", "1234.abc"]));
    // A marker on its own is nothing
    assert_eq!(parse_codes("1.\n-\nabcde"), list(&["abcde"]));
    // From a document too, where BackMyCodes loses this one
    assert_eq!(parse_document_codes("1. 1234"), list(&["1234"]));
}

#[test]
fn reading_a_document_keeps_what_looks_like_a_code() {
    assert_eq!(parse_document_codes("Your backup codes\n1234 5678  8765 4321\n2468 1357"), list(&["1234 5678", "8765 4321", "2468 1357"]));
    // A hyphen and letters of any script pass, a heading has no digit, a short one is too short
    assert_eq!(parse_document_codes("Код: абвг-1234"), list(&["абвг-1234"]));
    assert_eq!(parse_document_codes("ab1 abcdefg 12"), Vec::<String>::new());
    assert_eq!(parse_document_codes("a!b2c"), Vec::<String>::new());
}

#[test]
fn adding_and_using_codes() {
    let mut service = BackupService { codes: vec![BackupCode::new("one")], ..BackupService::default() };
    let done = add_codes(&mut service, secrets(&["one", "two", "three", "two"]), 5);
    assert_eq!((done.added, done.repeats, done.over_limit), (2, 2, 0));
    assert_eq!(service.codes.len(), 3);
    assert_eq!(service.codes[0].code.as_str(), "one");
    assert_eq!(service.updated_at, Some(5));
    assert!(!is_low(&service));
    assert_eq!(next_unused(&service), Some(0));
    service.codes[0].mark_used(6);
    assert_eq!(next_unused(&service), Some(1));
    assert!(is_low(&service));
    service.codes[1].mark_used(6);
    service.codes[2].mark_used(6);
    assert_eq!(next_unused(&service), None);
    assert!(is_low(&service), "none left is low too");
    assert!(!is_low(&BackupService::default()), "no codes at all is not");
    let plenty = BackupService { codes: vec![BackupCode::new("a"), BackupCode::new("b"), BackupCode::new("c")], ..BackupService::default() };
    assert!(!is_low(&plenty));
}

#[test]
fn the_keys_of_a_name() {
    for (name, keys) in [
        ("Acme, Inc.", &["acmeinc", "acme", "acme"][..]),
        ("Acme Widgets LLC", &["acmewidgetsllc", "acmewidgets", "acmewidgets"]),
        ("Example GmbH, Ltd.", &["examplegmbhltd", "example", "example"]),
        ("Example Login", &["examplelogin", "example", "example"]),
        ("https://login.example.com/path?x=1", &["loginexamplecom", "loginexamplecom", "examplecom", "example"]),
        ("foo.bar.example.co.uk", &["foobarexamplecouk", "foobarexamplecouk", "examplecouk", "example"]),
        ("example.org", &["exampleorg", "exampleorg", "exampleorg", "example"]),
        ("www.Example.net", &["examplenet", "examplenet", "examplenet", "example"]),
        ("example.social", &["examplesocial", "examplesocial", "examplesocial", "example"]),
        ("org.uk", &["orguk", "orguk", "orguk", "org"]),
        (".org", &["org", "org"]),
        ("Wind", &["wind", "wind", "wind"]),
        ("Example (work)", &["example", "example", "example"]),
        ("Crédit Agricole (CA)", &["creditagricole", "creditagricole", "creditagricole"]),
        ("Ünïcödé Bank", &["unicodebank", "unicodebank", "unicodebank"]),
        ("Яндекс (Ya)", &[]),
        ("", &[]),
        ("   ", &[]),
    ] {
        assert_eq!(service_keys(name), list(keys), "{name:?}");
    }
    assert!(service_keys(&"a".repeat(2000)).is_empty());
}

#[test]
fn folding() {
    assert_eq!(fold("  Crédit \u{a0}"), "credit");
    assert_eq!(fold("ÅNGSTRÖM"), "angstrom");
    // Composed and decomposed are one
    assert_eq!(fold("Йод"), fold("И\u{306}од"));
    assert_eq!(fold("İstanbul"), "istanbul");
    // A final sigma stays what JavaScript makes it
    assert_eq!(fold("ΣΑΣ"), "σας");
    // Every kind of mark goes, spacing and enclosing ones too; Hangul decomposes to letters, which stay
    assert_eq!(fold("a\u{903}b\u{20dd}c\u{300}"), "abc");
    assert_eq!(fold("한"), "\u{1112}\u{1161}\u{11ab}");
    assert_eq!(fold("ﬁ Ⅻ"), "ﬁ ⅻ");
}

#[test]
fn which_names_are_the_same_service() {
    for (a, b) in [
        ("Example", "example"),
        ("Example, Inc.", "Example"),
        ("example.com", "Example"),
        ("https://accounts.example.com/", "Example"),
        ("Epic Toys Account", "Epic Toys"),
        // Section 16, point 22: BackMyCodes would not see these
        ("Яндекс", "Яндекс"),
        ("Яндекс", " яндекс "),
    ] {
        assert!(same_service(a, b), "{a} {b}");
    }
    for (a, b) in [("Example", "Exemplar"), ("Example", "Example Cloud"), ("Яндекс", "Google"), ("", ""), ("Яндекс", "Почта")] {
        assert!(!same_service(a, b), "{a} {b}");
    }
}

#[test]
fn hues_are_stable() {
    // h = 7, then h * 31 + each unit, by hand for one short name: "ab" is (7*31+97)*31+98 = 9832, and 9832 mod 360 is 112
    assert_eq!(hue("ab"), 112);
    assert_eq!(hue("  AB "), 112);
    // Worked out in the spec, section 6
    assert_eq!(hue("GitHub"), 138);
    assert_eq!(hue("  github "), 138);
    assert_eq!(hue("Steam"), 65);
    assert_eq!(hue("Café ☕"), 117);
    // Long names wrap at 2^32, and an astral character is two units
    assert!(hue(&"long name ".repeat(1000)) < 360);
    assert_eq!(hue("😀"), ((7u32 * 31 + 0xD83D) * 31 + 0xDE00) % 360);
}

fn account(issuer: &str, label: &str) -> Account {
    Account { issuer: issuer.to_string(), label: label.to_string(), ..Account::default() }
}

fn service(name: &str) -> BackupService {
    BackupService { service_name: name.to_string(), ..BackupService::default() }
}

#[test]
fn an_accounts_service() {
    assert_eq!(account_service_name(&account("Co", "x:y")), "Co");
    assert_eq!(account_service_name(&account(" \u{a0}", "Co:me")), "Co");
    assert_eq!(account_service_name(&account("", ":me")), ":me");
    assert_eq!(account_service_name(&account("", "me")), "me");
}

#[test]
fn coverage_says_which_have_none() {
    let accounts = [
        account("Example, Inc.", "a"),
        account("Zebra", "b"),
        account("Mule", "c"),
        Account { no_backup_codes: Some(true), ..account("Gone", "d") },
        account("Яндекс", "e"),
        account("", ""),
        Account { deleted_at: Some(1), ..account("Removed", "f") },
    ];
    let services = [service("example.com"), service("Яндекс"), BackupService { deleted_at: Some(1), ..service("Zebra") }];
    let found = coverage(&accounts, &services);
    assert_eq!((found.covered, found.total), (2, 5));
    // A to Z by the fold, and an account with no name at all comes first, stably
    assert_eq!(found.missing, [5, 2, 1]);
    let index = ServiceIndex::new(&services);
    assert_eq!(service_for(&accounts[0], &index).unwrap().service_name, "example.com");
    assert!(service_for(&accounts[5], &index).is_none());
    // The first service that is the same, in the list's order
    let two = [service("Other"), service("Example"), service("example.com")];
    assert_eq!(service_for(&account("Example", ""), &ServiceIndex::new(&two)).unwrap().service_name, "Example");
}

const DIRECTORY: &str = r#"{"source":"x","updated":"2026-10-13","license":"MIT","entries":[
    ["Example","example.com","https://example.com/2fa","https://example.com/lost",["example.org","example.net"]],
    ["Second Example","second.example.io","https://second.example.io/2fa",0],
    ["Example Clone","example.net",0,"https://clone.test/lost"],
    ["No links","nolinks.test",0,""],
    ["javascript","javascript.test","javascript:alert(1)",0],
    "not a row",
    [1,2,3,4]
]}"#;

#[test]
fn the_directory() {
    let directory = Directory::parse(DIRECTORY.as_bytes()).unwrap();
    assert_eq!(directory.len(), 5);
    let row = directory.find("Example, Inc.").unwrap();
    assert_eq!((row.name.as_str(), row.docs.as_deref(), row.recovery.as_deref()), ("Example", Some("https://example.com/2fa"), Some("https://example.com/lost")));
    assert_eq!(row.other_domains, ["example.org", "example.net"]);
    // By any of its domains, and a key goes to the first row that has it
    assert_eq!(directory.find("example.org").unwrap().name, "Example");
    assert_eq!(directory.find("https://example.net/x").unwrap().name, "Example");
    assert_eq!(directory.find("second.example.io").unwrap().name, "Second Example");
    assert_eq!(directory.find("Second Example").unwrap().docs.as_deref(), Some("https://second.example.io/2fa"));
    let row = directory.find("nolinks.test").unwrap();
    assert_eq!((row.docs.as_deref(), row.recovery.as_deref()), (None, None));
    // Only a web address is a link
    assert_eq!(directory.find("javascript").unwrap().docs, None);
    assert!(directory.find("Unknown Service").is_none());
    assert!(directory.find("Яндекс").is_none());
    assert!(Directory::parse(b"[]").is_none());
    assert!(Directory::parse(b"not json").is_none());
}
