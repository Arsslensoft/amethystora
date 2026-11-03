//! The items as they are sealed: their field names are stored forever, a secret wipes and prints nothing, and an account
//! is exactly the JSON BackMyCodes' files hold.

use amethystora_vault_core::items::{
    Account, BackupCode, BackupService, Item, KIND_ACCOUNT, KIND_BACKUP_CODES, KIND_PASSWORD, OtpKind, PASSWORD_HISTORY, Password, Secret, is_id, is_js_space,
    js_trim, new_id, to_json_wiping,
};
use serde_json::{Value, json};

#[test]
fn an_account_is_written_in_its_stored_key_order() {
    let account = Account {
        id: "acc".to_string(),
        kind: Some(OtpKind::Hotp),
        issuer: "Café ☕".to_string(),
        label: "me".to_string(),
        secret: Secret::new("MZXW6YTBOI"),
        algorithm: "SHA256".to_string(),
        digits: 8,
        period: 30,
        counter: Some(3),
        pinned: Some(true),
        tags: Some(vec!["a".to_string()]),
        order: Some(2),
        note: Some(Secret::new("n")),
        no_backup_codes: Some(true),
        space_id: Some("s".to_string()),
        added_at: 5,
        updated_at: Some(6),
        deleted_at: Some(7),
        unknown: serde_json::from_value(json!({"zz": 1})).unwrap(),
    };
    let text = String::from_utf8(to_json_wiping(&account).unwrap().to_vec()).unwrap();
    assert_eq!(
        text,
        r#"{"id":"acc","type":"hotp","issuer":"Café ☕","label":"me","secret":"MZXW6YTBOI","algorithm":"SHA256","digits":8,"period":30,"counter":3,"pinned":true,"tags":["a"],"order":2,"note":"n","noBackupCodes":true,"spaceId":"s","addedAt":5,"updatedAt":6,"deletedAt":7,"zz":1}"#
    );
    let back: Account = serde_json::from_str(&text).unwrap();
    assert_eq!(serde_json::to_string(&back).unwrap(), text);
}

#[test]
fn what_is_absent_stays_absent() {
    let account = Account { id: "a".to_string(), secret: Secret::new("MZXW6"), added_at: 1, ..Account::default() };
    assert_eq!(
        serde_json::to_string(&account).unwrap(),
        r#"{"id":"a","issuer":"","label":"","secret":"MZXW6","algorithm":"SHA1","digits":6,"period":30,"addedAt":1}"#
    );
    let back: Account = serde_json::from_str(r#"{"secret":"MZXW6"}"#).unwrap();
    assert_eq!((back.otp_kind(), back.algorithm.as_str(), back.digits, back.period), (OtpKind::Totp, "SHA1", 6, 30));
    assert!(serde_json::from_str::<Account>(r#"{"id":"no secret"}"#).is_err());
}

#[test]
fn a_service_and_its_codes() {
    let mut service = BackupService {
        id: "s".to_string(),
        service_name: "Example".to_string(),
        codes: vec![BackupCode::new("one"), BackupCode::new("two")],
        added_at: 3,
        account: Some("me".to_string()),
        tags: Some(vec!["t".to_string()]),
        order: Some(1),
        ..BackupService::default()
    };
    service.codes[1].mark_used(9);
    assert_eq!(
        serde_json::to_string(&service).unwrap(),
        r#"{"id":"s","serviceName":"Example","codes":[{"code":"one","used":false},{"code":"two","used":true,"usedAt":9}],"addedAt":3,"account":"me","tags":["t"],"order":1}"#
    );
    service.codes[1].mark_unused();
    assert_eq!(serde_json::to_string(&service.codes[1]).unwrap(), r#"{"code":"two","used":false}"#);
}

#[test]
fn a_code_keeps_what_it_does_not_know_through_being_used() {
    let mut code: BackupCode = serde_json::from_str(r#"{"code":"c","used":false,"later":{"a":1}}"#).unwrap();
    code.mark_used(4);
    code.mark_unused();
    assert_eq!(serde_json::to_string(&code).unwrap(), r#"{"code":"c","used":false,"later":{"a":1}}"#);
}

#[test]
fn a_password_keeps_the_last_five() {
    let mut item = Password { id: "p".to_string(), password: Secret::new("first"), ..Password::default() };
    for i in 0..7 {
        item.set_password(Secret::new(format!("next {i}")), i);
    }
    assert_eq!(item.password.as_str(), "next 6");
    assert_eq!(item.history.len(), PASSWORD_HISTORY);
    assert_eq!((item.history[0].password.as_str(), item.history[0].changed_at), ("next 5", 6));
    assert_eq!(item.history[4].password.as_str(), "next 1");
    assert_eq!(item.updated_at, Some(6));
    // Setting it again to itself keeps no copy, and an empty one before it is no history
    item.set_password(Secret::new("next 6"), 10);
    assert_eq!((item.history[0].password.as_str(), item.updated_at), ("next 5", Some(10)));
    let mut empty = Password::default();
    empty.set_password(Secret::new("x"), 1);
    assert!(empty.history.is_empty());
    let text = serde_json::to_string(&Password { history: item.history[..1].to_vec(), ..Password::default() }).unwrap();
    assert!(text.contains(r#""history":[{"password":"next 5","changedAt":6}]"#), "{text}");
}

#[test]
fn every_kind_goes_through_json_by_its_name() {
    let items = [
        Item::Password(Password { id: "a".to_string(), password: Secret::new("p"), added_at: 1, ..Password::default() }),
        Item::Account(Account { id: "b".to_string(), secret: Secret::new("MZXW6"), added_at: 1, updated_at: Some(2), ..Account::default() }),
        Item::BackupCodes(BackupService { id: "c".to_string(), added_at: 1, ..BackupService::default() }),
    ];
    for (item, kind) in items.iter().zip([KIND_PASSWORD, KIND_ACCOUNT, KIND_BACKUP_CODES]) {
        assert_eq!(item.kind(), kind);
        let json = item.to_json().unwrap();
        let back = Item::from_json(kind, &json).unwrap();
        assert_eq!(back.id(), item.id());
        assert_eq!(back.changed_at(), item.changed_at());
        assert_eq!(back.deleted_at(), None);
    }
    assert_eq!(items[1].changed_at(), 2);
    assert!(Item::from_json("anything", b"{}").is_none());
    assert!(Item::from_json(KIND_ACCOUNT, b"not json").is_none());
    let mut gone = items[0].clone();
    gone.mark_deleted(9);
    assert_eq!((gone.deleted_at(), gone.changed_at()), (Some(9), 9));
    let back = Item::from_json(KIND_PASSWORD, &gone.to_json().unwrap()).unwrap();
    assert_eq!(back.deleted_at(), Some(9));
}

#[test]
fn nothing_prints_a_secret() {
    let account = Account {
        secret: Secret::new("TOPSECRETVALUE"),
        note: Some(Secret::new("TOPSECRETNOTE")),
        unknown: serde_json::from_value(json!({"k": "TOPSECRETOTHER"})).unwrap(),
        ..Account::default()
    };
    let password = Password {
        password: Secret::new("TOPSECRETPASSWORD"),
        history: vec![amethystora_vault_core::items::OldPassword { password: Secret::new("TOPSECRETOLD"), changed_at: 1 }],
        ..Password::default()
    };
    let service = BackupService { codes: vec![BackupCode::new("TOPSECRETCODE")], ..BackupService::default() };
    for printed in [format!("{account:?}"), format!("{password:?}"), format!("{service:?}"), format!("{:?}", Item::Account(account.clone()))] {
        assert!(!printed.contains("TOPSECRET"), "{printed}");
        assert!(printed.contains("[secret]") || printed.contains("unknown"), "{printed}");
    }
}

#[test]
fn ids_have_one_shape() {
    let id = new_id().unwrap();
    assert!(is_id(&id), "{id}");
    assert_eq!(&id[14..15], "4");
    assert_ne!(id, new_id().unwrap());
    for bad in ["", "acc-1", "../../etc/passwd", "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA", "00000000-0000-4000-8000-00000000000/", "00000000x0000-4000-8000-000000000000"] {
        assert!(!is_id(bad), "{bad}");
    }
}

#[test]
fn javascript_spaces() {
    assert_eq!(js_trim("\u{a0}\u{feff} a b\u{2028}\u{3000}\t"), "a b");
    // str::trim takes U+0085 and leaves U+FEFF; this does the other
    assert_eq!(js_trim("\u{85}a"), "\u{85}a");
    assert!(is_js_space('\u{feff}'));
    assert!(!is_js_space('\u{85}'));
    assert!(!is_js_space('\u{200b}'));
}

#[test]
fn a_wiping_json_is_plain_json() {
    let value: Value = json!({"long": "x".repeat(10_000), "n": [1, 2, 3]});
    let written = to_json_wiping(&value).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&written).unwrap(), value);
}
