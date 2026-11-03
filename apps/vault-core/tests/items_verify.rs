//! What the independent check of the review fixes found (plans/vault/work/verify-items.md, N1 to N8), each held by a test
//! that failed before its fix.

mod items_support;
use items_support::*;

use amethystora_vault_client::MAX_DATA;
use amethystora_vault_core::bmc::{Added, Collector, read_export};
use amethystora_vault_core::codes::{add_codes, parse_codes, parse_document_codes};
use amethystora_vault_core::generate::{LONGEST_WORD, words};
use amethystora_vault_core::import::read_foreign;
use amethystora_vault_core::items::{BackupService, MAX_CODES, MAX_ITEM, MAX_NAME, MAX_TAGS, Secret, json_size};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};
use std::time::Instant;

#[global_allocator]
static COUNTING: Counting = Counting;

const KEY: &str = "ORSXG5DTMVRXEZLU";
const MIB: usize = 1024 * 1024;

fn aegis(groups: Value, entries: Value) -> Vec<u8> {
    json!({"header": {}, "db": {"groups": groups, "entries": entries}}).to_string().into_bytes()
}

fn two_fas(groups: Value, services: Value) -> Vec<u8> {
    json!({"schemaVersion": 4, "groups": groups, "services": services}).to_string().into_bytes()
}

fn bitwarden(folders: Value, items: Value) -> Vec<u8> {
    json!({"encrypted": false, "folders": folders, "items": items}).to_string().into_bytes()
}

fn plain_export(accounts: Value, services: Value) -> Vec<u8> {
    json!({"app": "BackMyCodes", "scheme": "plaintext", "accounts": accounts, "backupServices": services}).to_string().into_bytes()
}

// N1: one group named by every entry

#[test]
fn a_group_name_past_its_bound_is_never_copied_for_an_entry() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    // The review's input in kind: every one of 65,536 entries names a group of 10 MiB, in a file under the 16 MiB bound
    let name = "x".repeat(10 * MIB);
    let count = 65_536;
    let files = [
        aegis(json!([{"uuid": "g", "name": name}]), json!((0..count).map(|_| json!({"type": "totp", "info": {"secret": KEY}, "groups": ["g"]})).collect::<Vec<_>>())),
        two_fas(json!([{"id": "g", "name": name}]), json!((0..count).map(|_| json!({"name": "n", "secret": KEY, "groupId": "g"})).collect::<Vec<_>>())),
        bitwarden(json!([{"id": "g", "name": name}]), json!((0..count).map(|_| json!({"name": "n", "folderId": "g", "login": {"totp": KEY}})).collect::<Vec<_>>())),
    ];
    for file in &files {
        let (start, before) = (Instant::now(), allocated_total());
        let read = read_foreign(file, "").unwrap();
        let asked = allocated_total() - before;
        // The refused entries are counted, and no one of them copied the name (65,536 copies would be 640 GiB)
        assert_eq!((read.accounts.len(), read.skipped), (0, count), "{}", String::from_utf8_lossy(&file[..40]));
        assert!(asked < 400 * MIB, "{asked} bytes asked for");
        assert!(start.elapsed().as_secs_f64() < 8.0, "{:?}", start.elapsed());
    }
}

// N2: a tag compared before its length is bounded, and a name met again and again

#[test]
fn a_repeated_group_of_long_names_costs_nothing() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    // 64 groups of 190 KB that differ only at their end, one entry naming them, then the last again and again
    let groups: Vec<Value> = (0..64).map(|i| json!({"uuid": format!("g{i}"), "name": format!("{}{i}", "x".repeat(190_000))})).collect();
    let mut ids: Vec<Value> = (0..64).map(|i| json!(format!("g{i}"))).collect();
    // The last one: each mention was compared with the 63 before it, scanning each to its end
    ids.extend((0..3000).map(|_| json!("g63")));
    let file = aegis(json!(groups), json!([{"type": "totp", "info": {"secret": KEY}, "groups": ids}]));
    let start = Instant::now();
    let read = read_foreign(&file, "").unwrap();
    assert_eq!((read.accounts.len(), read.skipped), (0, 1));
    assert!(start.elapsed().as_secs_f64() < 2.0, "{:?}", start.elapsed());
}

#[test]
fn a_repeated_group_at_the_bound_costs_little() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    // The names at the bound and different at their end, 900,000 mentions of one: all bounds met, the file is 4.6 MB
    let groups: Vec<Value> = (0..64).map(|i| json!({"uuid": format!("g{i}"), "name": format!("{}{i:02}", "x".repeat(MAX_NAME - 2))})).collect();
    let mut ids: Vec<Value> = (0..64).map(|i| json!(format!("g{i}"))).collect();
    ids.extend((0..900_000).map(|_| json!("g63")));
    let file = aegis(json!(groups), json!([{"type": "totp", "info": {"secret": KEY}, "groups": ids}]));
    let start = Instant::now();
    let read = read_foreign(&file, "").unwrap();
    assert_eq!((read.accounts.len(), read.skipped), (1, 0));
    assert_eq!(read.accounts[0].tags.as_ref().unwrap().len(), MAX_TAGS);
    assert!(start.elapsed().as_secs_f64() < 3.0, "{:?}", start.elapsed());
}

#[test]
fn many_groups_with_one_name_are_one_tag() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let groups: Vec<Value> = (0..200).map(|i| json!({"uuid": format!("g{i}"), "name": "Work"})).collect();
    let ids: Vec<Value> = (0..200).map(|i| json!(format!("g{i}"))).collect();
    let file = aegis(json!(groups), json!([{"type": "totp", "info": {"secret": KEY}, "groups": ids, "group": "Work"}]));
    let read = read_foreign(&file, "").unwrap();
    assert_eq!(read.accounts[0].tags.as_deref(), Some(&["Work".to_string()][..]));
}

// N3: a number out of range is not a missing one

#[test]
fn a_number_out_of_range_refuses_the_entry_at_every_field() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let big = json!(u64::MAX);
    for (account, why) in [
        (json!({"type": "hotp", "secret": KEY, "counter": big}), "counter u64::MAX"),
        (json!({"type": "hotp", "secret": KEY, "counter": 1e300}), "counter 1e300"),
        (json!({"type": "hotp", "secret": KEY, "counter": -1e300}), "counter -1e300"),
        (json!({"secret": KEY, "digits": 1e300}), "digits 1e300"),
        (json!({"secret": KEY, "digits": -1e300}), "digits -1e300"),
        (json!({"secret": KEY, "period": -1e300}), "period -1e300"),
        (json!({"secret": KEY, "period": big}), "period u64::MAX"),
        (json!({"secret": KEY, "addedAt": 1e300}), "addedAt"),
        (json!({"secret": KEY, "updatedAt": 1e300}), "updatedAt"),
        (json!({"secret": KEY, "order": -1e300}), "order"),
    ] {
        let read = read_export(&plain_export(json!([account]), json!([])), "").unwrap();
        assert_eq!((read.accounts.len(), read.skipped), (0, 1), "{why}");
    }
    let service = json!({"serviceName": "s", "codes": [{"code": "abc", "used": true, "usedAt": 1e300}]});
    let read = read_export(&plain_export(json!([]), json!([service])), "").unwrap();
    assert_eq!((read.services.len(), read.skipped), (0, 1), "usedAt");
    let service = json!({"serviceName": "s", "addedAt": u64::MAX});
    let read = read_export(&plain_export(json!([]), json!([service])), "").unwrap();
    assert_eq!((read.services.len(), read.skipped), (0, 1), "a service's addedAt");

    // A missing field or one of another type is still its default
    let read = read_export(&plain_export(json!([{"secret": KEY, "digits": "x", "period": null}]), json!([])), "").unwrap();
    assert_eq!((read.accounts.len(), read.skipped, read.accounts[0].digits, read.accounts[0].period), (1, 0, 6, 30));
}

#[test]
fn a_transfer_number_out_of_range_refuses_the_entry() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    for field in [r#""c":1e300"#, r#""c":-1e300"#, r#""d":1e300"#, r#""p":-1e300"#, r#""c":18446744073709551615"#] {
        let payload = format!(r#"{{"a":[{{"s":"{KEY}","t":"hotp",{field}}}]}}"#);
        let mut collector = Collector::new();
        assert_eq!(collector.add(&format!("BMCP:01/01:{}", URL_SAFE_NO_PAD.encode(&payload))), Added::New);
        let read = collector.read("", 1).unwrap();
        assert_eq!((read.accounts.len(), read.skipped), (0, 1), "{field}");
    }
}

// N4: adding codes stops where the item does

#[test]
fn adding_codes_stops_at_the_limit_of_one_item() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let codes: Vec<Secret> = (0..4000).map(|i| Secret::new(format!("{i:0>255}"))).collect();
    let mut service = BackupService::default();
    let done = add_codes(&mut service, codes, 1);
    assert!(done.added > 0 && done.added < 4000, "{done:?}");
    assert_eq!(done.added + done.over_limit, 4000);
    assert_eq!(done.repeats, 0);
    assert_eq!(service.codes.len(), done.added);
    assert!(service.size_with_room() <= MAX_ITEM);
    // Every code may be used, and the item still goes over the socket
    for code in &mut service.codes {
        code.mark_used(i64::MIN);
    }
    assert!(json_size(&service) <= MAX_DATA, "{}", json_size(&service));
    // Full: nothing more is added, and each is counted
    let more = add_codes(&mut service, vec![Secret::new("one more")], 2);
    assert_eq!((more.added, more.over_limit), (0, 1));
}

#[test]
fn a_service_at_the_limit_of_the_door_keeps_room_for_used_at() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let service = |count: usize| {
        let codes: Vec<Value> = (0..count).map(|i| json!({"code": format!("{i:0>256}"), "used": false})).collect();
        read_export(&plain_export(json!([]), json!([{"serviceName": "s", "codes": codes}])), "").unwrap()
    };
    // The most codes the door takes
    let (mut low, mut high) = (1000, MAX_CODES);
    assert!(service(low).services.len() == 1 && service(high).services.is_empty());
    while high - low > 1 {
        let middle = (low + high) / 2;
        if service(middle).services.len() == 1 { low = middle } else { high = middle }
    }
    let count = low;
    let mut held = service(count).services.pop().unwrap();
    assert_eq!(held.codes.len(), count);
    for code in &mut held.codes {
        code.mark_used(i64::MAX);
    }
    held.updated_at = Some(i64::MAX);
    assert!(json_size(&held) <= MAX_DATA, "{} against {MAX_DATA}", json_size(&held));
}

// N5: repeats are found by their hash

#[test]
fn repeats_of_the_last_code_cost_one_comparison_each() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let mut text = String::new();
    for i in 0..4095 {
        text.push_str(&format!("c{i:04}\n"));
    }
    text.push_str(&"last1\n".repeat((MIB - text.len()) / 6));
    assert!(text.len() <= MIB);
    let start = Instant::now();
    let parsed = parse_codes(&text);
    assert_eq!((parsed.codes.len(), parsed.dropped, parsed.text_too_large), (4096, 0, false));
    assert!(start.elapsed().as_secs_f64() < 1.0, "{:?}", start.elapsed());
}

#[test]
fn parsing_counts_what_it_drops() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let long = "c".repeat(300);
    let parsed = parse_codes(&format!("ok1234\n{long}\nok1234\nnext567\n"));
    assert_eq!((parsed.codes.len(), parsed.dropped), (2, 1), "a repeat is no loss, a long code is one");
    let many: String = (0..MAX_CODES + 3).map(|i| format!("code{i:05}\n")).collect();
    let parsed = parse_codes(&many);
    assert_eq!((parsed.codes.len(), parsed.dropped), (MAX_CODES, 3));
    // The document path counts the same, and a heading is no code
    let parsed = parse_document_codes(&format!("Your codes\nabcd-1234\n{long}\n"));
    assert_eq!((parsed.codes.len(), parsed.dropped), (1, 1));
    let parsed = parse_codes(&"a ".repeat(MIB));
    assert!(parsed.text_too_large && parsed.codes.is_empty());
}

// N7: the words chosen are named by nothing else

#[test]
fn words_make_one_buffer_of_the_right_size() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let list = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/eff_large_wordlist.txt")).unwrap();
    let longest = list.lines().filter_map(|l| l.split('\t').nth(1)).map(str::len).max().unwrap();
    assert_eq!(LONGEST_WORD, longest);
    for separator in ['-', 'é', '😀'] {
        let (made, _, allocations) = measured(|| words(20, separator).unwrap());
        assert_eq!(made.password.as_str().matches(separator).count(), 19);
        // A vector of the chosen words beside the buffer was a second one, and left their places unwiped
        assert_eq!(allocations, 1, "{separator}");
    }
}

// N8: markers one after the other

#[test]
fn markers_one_after_another_all_go() {
    let _guard = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let text = |parsed: amethystora_vault_core::codes::ParsedCodes| parsed.codes.iter().map(|c| c.as_str().to_string()).collect::<Vec<_>>();
    for pasted in ["• 1. abcd-1234", "1. • abcd-1234", "1) - abcd-1234", "* • 2. abcd-1234", "  • 1.   2) abcd-1234"] {
        assert_eq!(text(parse_codes(pasted)), ["abcd-1234"], "{pasted}");
        assert_eq!(text(parse_document_codes(pasted)), ["abcd-1234"], "{pasted}");
    }
    // A lot of them stay linear
    let start = Instant::now();
    assert_eq!(text(parse_codes(&format!("{}abcd-1234", "• ".repeat(150_000)))), ["abcd-1234"]);
    assert!(start.elapsed().as_secs_f64() < 1.0, "{:?}", start.elapsed());
    // These keep what they start with, as ruled
    assert_eq!(text(parse_codes("1.5678-abcd\n-abc\n1.5abc")), ["1.5678-abcd", "-abc", "1.5abc"]);
}
