//! What the service and its clients say to each other on the socket: the framing and the refusals are
//! `apps/vault-client`'s, and this is the requests, each checked for its shape before anything is done with it. A
//! client's id never reaches a path until it has passed here: no `..`, no separators, a fixed length.

use base64::Engine;
use serde_json::{json, Value};
use zeroize::Zeroizing;

pub use amethystora_vault_client::{Refusal, MAX_DATA, MAX_LINE};

use crate::slots::MAX_PASSWORD;

/// A purpose: `[a-z0-9-]{1,40}`
pub fn valid_purpose(text: &str) -> bool {
    (1..=40).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A record: at most 128 characters, none a control character
pub fn valid_record(text: &str) -> bool {
    text.chars().count() <= 128 && !text.chars().any(char::is_control)
}

/// Labels the vault's own files are sealed under, as `amethystora-vault:<label>:<vault id>`: an item of such a kind under the
/// vault's id would be sealed exactly as they are
const RESERVED_KINDS: &[&str] = &["meta", "index", "data", "slot", "chip"];

/// A kind of item: `[a-z][a-z0-9-]{0,31}`, and none of the vault's own
pub fn valid_kind(text: &str) -> bool {
    (1..=32).contains(&text.len())
        && text.as_bytes()[0].is_ascii_lowercase()
        && text.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !RESERVED_KINDS.contains(&text)
}

/// An item's id, which becomes part of a file name: `[A-Za-z0-9_-]{1,64}`, so never a dot or a separator
pub fn valid_id(text: &str) -> bool {
    (1..=64).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// A vault's or a slot's id: 32 lowercase hex digits
pub fn valid_hex_id(text: &str) -> bool {
    text.len() == 32 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// What a person calls something: 1 to 256 characters, none a control character. It is only ever data.
pub fn valid_name(text: &str) -> bool {
    !text.is_empty() && text.chars().count() <= 256 && !text.chars().any(char::is_control)
}

/// How often using an item asks the person. `WhileOpen` asks once for each item each time the vault is opened, and
/// `Never` not at all, which is only set after a warning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Approval {
    Ask,
    WhileOpen,
    Never,
}

impl Approval {
    pub fn from_name(name: &str) -> Option<Approval> {
        match name {
            "ask" => Some(Approval::Ask),
            "while-open" => Some(Approval::WhileOpen),
            "never" => Some(Approval::Never),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Approval::Ask => "ask",
            Approval::WhileOpen => "while-open",
            Approval::Never => "never",
        }
    }
}

/// How a vault is being opened
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Password,
    Recovery,
    SecurityKey,
    Approval,
    NoPrompt,
}

impl Method {
    fn from_name(name: &str) -> Option<Method> {
        match name {
            "password" => Some(Method::Password),
            "recovery" => Some(Method::Recovery),
            "security-key" => Some(Method::SecurityKey),
            "approval" => Some(Method::Approval),
            "no-prompt" => Some(Method::NoPrompt),
            _ => None,
        }
    }
}

/// A service's logo, a picture of a public site: the one kind the service keeps at `Never` without the loosening question, and no other
pub const KIND_LOGO: &str = "logo";

/// A key, a certificate with its key, and an account's CA: kinds of item only the service reads
pub const KIND_KEY: &str = "key";
pub const KIND_CERTIFICATE: &str = "certificate";
pub const KIND_ACCOUNT_CA: &str = "account-ca";

/// What a client never gets by `get` and never sets by `put`: the service holds these and hands back what is made with them.
/// A password stays out, since handing it over once approved is what it is for. Backup codes are here so that one at a time is
/// `codes-next`, and showing them all takes a question of its own.
pub const SERVICE_ONLY: &[&str] = &[crate::store::DATA_KEY, crate::items::KIND_ACCOUNT, crate::items::KIND_BACKUP_CODES, KIND_KEY, KIND_CERTIFICATE, KIND_ACCOUNT_CA, crate::sync::merge::KIND_CONNECTION, crate::sync::merge::KIND_LINK];

/// Of those, the ones with no setting a person chooses: nothing but the service ever uses them
pub const NO_SETTING: &[&str] = &[crate::store::DATA_KEY, KIND_ACCOUNT_CA];

pub fn service_only(kind: &str) -> bool {
    SERVICE_ONLY.contains(&kind)
}

/// A secret from a client, in base64 on the wire: a password, a recovery key's text or a security key's secret
pub type Secret = Zeroizing<Vec<u8>>;

pub enum Request {
    // What other programs use (apps/vault-client)
    Seal { purpose: String, record: String, data: Zeroizing<Vec<u8>> },
    Open { purpose: String, record: String, data: Vec<u8> },
    Forget { purpose: String },
    // Opening and closing
    Status,
    Setup { name: String, method: Method, secret: Option<Secret> },
    Confirm { recovery: Secret },
    New { name: String, hidden: bool, secret: Secret },
    Unlock { method: Method, slot: Option<String>, vault: Option<String>, secret: Option<Secret> },
    Lock,
    // The ways in
    Methods,
    AddSecurityKey { name: String, credential: String, salt: String, secret: Secret, vault: Option<String> },
    AddChipSlot { kind: Method, vault: Option<String> },
    RemoveSlot { slot: String },
    Background { on: bool },
    ChangePassword { vault: Option<String>, current: Option<Secret>, secret: Secret },
    SetIdle { minutes: u32 },
    // Vaults
    Rename { vault: Option<String>, name: String },
    RemoveThis { vault: Option<String> },
    ForgetEverything,
    // Items, whose value the service does not read
    Put { kind: String, id: Option<String>, name: String, meta: Value, approval: Option<Approval>, value: Value, vault: Option<String> },
    Get { kind: String, id: String },
    List { kind: Option<String> },
    Remove { kind: String, id: String },
    SetApproval { kind: String, id: String, approval: Approval },
    /// An operation of the `items`, `keys` or `pki` modules, which the service's table looks up
    Plug { op: String, body: Value },
}

fn text(body: &Value, name: &str) -> Result<String, Refusal> {
    body.get(name).and_then(Value::as_str).map(str::to_string).ok_or(Refusal::BadRequest)
}

fn optional_text(body: &Value, name: &str) -> Result<Option<String>, Refusal> {
    match body.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(Refusal::BadRequest),
    }
}

fn checked(value: String, shape: fn(&str) -> bool) -> Result<String, Refusal> {
    if shape(&value) { Ok(value) } else { Err(Refusal::BadRequest) }
}

fn vault_id(body: &Value) -> Result<Option<String>, Refusal> {
    match optional_text(body, "vault")? {
        Some(id) => checked(id, valid_hex_id).map(Some),
        None => Ok(None),
    }
}

/// Base64, of at most `limit` bytes. Its length is checked before it is decoded, and a refusal is too-large only for
/// what might fit a larger call.
fn base64_of(body: &Value, name: &str, limit: usize) -> Result<Zeroizing<Vec<u8>>, Refusal> {
    let encoded = body.get(name).and_then(Value::as_str).ok_or(Refusal::BadRequest)?;
    if encoded.len() > limit.div_ceil(3) * 4 {
        return Err(Refusal::TooLarge);
    }
    let decoded = base64::engine::general_purpose::STANDARD.decode(encoded).map(Zeroizing::new).map_err(|_| Refusal::BadRequest)?;
    if decoded.len() > limit { Err(Refusal::TooLarge) } else { Ok(decoded) }
}

/// A secret too long for any password is refused as an unusable or a wrong one is, not as a malformed request
fn secret(body: &Value, name: &str) -> Result<Secret, Refusal> {
    base64_of(body, name, MAX_PASSWORD).map_err(|why| if why == Refusal::TooLarge { Refusal::Refused } else { why })
}

fn optional_secret(body: &Value, name: &str) -> Result<Option<Secret>, Refusal> {
    match body.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => secret(body, name).map(Some),
    }
}

fn method(body: &Value, name: &str) -> Result<Method, Refusal> {
    Method::from_name(&text(body, name)?).ok_or(Refusal::BadRequest)
}

/// One line from a client, whatever it holds, or the refusal to say
pub fn parse(line: &[u8]) -> Result<Request, Refusal> {
    if line.len() > MAX_LINE {
        return Err(Refusal::TooLarge);
    }
    let body: Zeroizing<Vec<u8>> = Zeroizing::new(line.to_vec());
    let body: Value = serde_json::from_slice(&body).map_err(|_| Refusal::BadRequest)?;
    if !body.is_object() || body.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(Refusal::BadRequest);
    }
    let op = text(&body, "op")?;
    let request = match op.as_str() {
        "seal" => Request::Seal { purpose: checked(text(&body, "purpose")?, valid_purpose)?, record: checked(text(&body, "record")?, valid_record)?, data: base64_of(&body, "data", MAX_DATA)? },
        "open" => Request::Open {
            purpose: checked(text(&body, "purpose")?, valid_purpose)?,
            record: checked(text(&body, "record")?, valid_record)?,
            data: base64_of(&body, "data", MAX_DATA + crate::seal::OVERHEAD)?.to_vec(),
        },
        "forget" => Request::Forget { purpose: checked(text(&body, "purpose")?, valid_purpose)? },
        "status" => Request::Status,
        "setup" => Request::Setup { name: optional_name(&body)?.unwrap_or_default(), method: method(&body, "method")?, secret: optional_secret(&body, "secret")? },
        "confirm" => Request::Confirm { recovery: secret(&body, "recovery")? },
        "new" => Request::New { name: checked(text(&body, "name")?, valid_name)?, hidden: body.get("hidden").and_then(Value::as_bool).unwrap_or(false), secret: secret(&body, "secret")? },
        "unlock" => Request::Unlock {
            method: method(&body, "method")?,
            slot: optional_text(&body, "slot")?.map(|slot| checked(slot, valid_hex_id)).transpose()?,
            vault: vault_id(&body)?,
            secret: optional_secret(&body, "secret")?,
        },
        "lock" => Request::Lock,
        "methods" => Request::Methods,
        "add-security-key" => Request::AddSecurityKey {
            name: checked(text(&body, "name")?, valid_name)?,
            credential: checked(text(&body, "credential")?, short_base64)?,
            salt: checked(text(&body, "salt")?, short_base64)?,
            secret: secret(&body, "secret")?,
            vault: vault_id(&body)?,
        },
        "add-slot" => Request::AddChipSlot { kind: method(&body, "kind")?, vault: vault_id(&body)? },
        "remove-slot" => Request::RemoveSlot { slot: checked(text(&body, "slot")?, valid_hex_id)? },
        "background" => Request::Background { on: body.get("on").and_then(Value::as_bool).ok_or(Refusal::BadRequest)? },
        "change-password" => Request::ChangePassword { vault: vault_id(&body)?, current: optional_secret(&body, "current")?, secret: secret(&body, "secret")? },
        "set-idle" => {
            let minutes = body.get("minutes").and_then(Value::as_u64).filter(|minutes| (1..=1440).contains(minutes)).ok_or(Refusal::BadRequest)?;
            Request::SetIdle { minutes: minutes as u32 }
        }
        "rename" => Request::Rename { vault: vault_id(&body)?, name: checked(text(&body, "name")?, valid_name)? },
        "remove-this" => Request::RemoveThis { vault: vault_id(&body)? },
        "forget-everything" => Request::ForgetEverything,
        "put" => {
            let kind = checked(text(&body, "kind")?, valid_kind)?;
            let id = optional_text(&body, "id")?.map(|id| checked(id, valid_id)).transpose()?;
            let name = checked(text(&body, "name")?, valid_name)?;
            let meta = body.get("meta").cloned().unwrap_or(Value::Null);
            // What is listed without approval: small, and the caller's word that it holds no secret
            if meta.to_string().len() > crate::store::MAX_META {
                return Err(Refusal::TooLarge);
            }
            let approval = match optional_text(&body, "approval")? {
                Some(name) => Some(Approval::from_name(&name).ok_or(Refusal::BadRequest)?),
                None => None,
            };
            let value = body.get("value").cloned().ok_or(Refusal::BadRequest)?;
            if value.to_string().len() > MAX_DATA {
                return Err(Refusal::TooLarge);
            }
            Request::Put { kind, id, name, meta, approval, value, vault: vault_id(&body)? }
        }
        "get" => Request::Get { kind: checked(text(&body, "kind")?, valid_kind)?, id: checked(text(&body, "id")?, valid_id)? },
        "list" => Request::List { kind: optional_text(&body, "kind")?.map(|kind| checked(kind, valid_kind)).transpose()? },
        "remove" => Request::Remove { kind: checked(text(&body, "kind")?, valid_kind)?, id: checked(text(&body, "id")?, valid_id)? },
        "set-approval" => Request::SetApproval {
            kind: checked(text(&body, "kind")?, valid_kind)?,
            id: checked(text(&body, "id")?, valid_id)?,
            approval: Approval::from_name(&text(&body, "approval")?).ok_or(Refusal::BadRequest)?,
        },
        _ if valid_kind(&op) => Request::Plug { op, body },
        _ => return Err(Refusal::BadRequest),
    };
    Ok(request)
}

fn optional_name(body: &Value) -> Result<Option<String>, Refusal> {
    optional_text(body, "name")?.map(|name| checked(name, valid_name)).transpose()
}

fn short_base64(text: &str) -> bool {
    (1..=1024).contains(&text.len()) && base64::engine::general_purpose::STANDARD.decode(text).is_ok()
}

/// One line of answer, with its newline
pub fn line(answer: &Value) -> Vec<u8> {
    let mut out = answer.to_string().into_bytes();
    out.push(b'\n');
    out
}

/// `{"ok":false,"why":...}`
pub fn refusal(why: Refusal) -> Value {
    json!({"ok": false, "why": why.name()})
}

pub fn encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is(line: &str, why: Refusal) {
        match parse(line.as_bytes()) {
            Err(got) => assert_eq!(got, why, "{line}"),
            Ok(_) => panic!("{line} passed"),
        }
    }

    #[test]
    fn a_purpose_keeps_to_its_shape() {
        assert!(valid_purpose("resume"));
        assert!(valid_purpose("a-1"));
        assert!(valid_purpose(&"a".repeat(40)));
        for bad in ["", "A", "a b", "a/b", "..", "a.b", "é", "a\n"] {
            assert!(!valid_purpose(bad), "{bad:?}");
        }
        assert!(!valid_purpose(&"a".repeat(41)));
    }

    #[test]
    fn a_record_keeps_to_its_shape() {
        assert!(valid_record(""));
        assert!(valid_record("2026/11:é ../"));
        assert!(valid_record(&"é".repeat(128)));
        assert!(!valid_record(&"é".repeat(129)));
        assert!(!valid_record("a\nb"));
        assert!(!valid_record("a\u{0}b"));
        assert!(!valid_record("a\u{7f}b"));
    }

    #[test]
    fn nothing_that_could_leave_a_folder_is_an_id() {
        for bad in ["", "..", ".", "../x", "a/b", "a\\b", "a.b", "a\0b", "é", " ", "a b", "%2e%2e"] {
            assert!(!valid_id(bad), "{bad:?}");
            assert!(!valid_kind(bad), "{bad:?}");
            assert!(!valid_hex_id(bad), "{bad:?}");
        }
        assert!(valid_id("Abc_1-2"));
        assert!(!valid_id(&"a".repeat(65)));
        assert!(valid_kind("data-key"));
        for reserved in ["meta", "index", "data", "slot", "chip"] {
            assert!(!valid_kind(reserved), "{reserved}");
        }
        assert!(!valid_kind("Data"));
        assert!(!valid_kind("1a"));
        assert!(valid_hex_id(&"0a".repeat(16)));
        assert!(!valid_hex_id(&"0A".repeat(16)));
    }

    #[test]
    fn the_contract_parses() {
        assert!(matches!(
            parse(br#"{"v":1,"op":"seal","purpose":"resume","record":"2026","data":"aGVsbG8="}"#),
            Ok(Request::Seal { purpose, record, data }) if purpose == "resume" && record == "2026" && data.as_slice() == b"hello"
        ));
        assert!(matches!(parse(br#"{"v":1,"op":"open","purpose":"r","record":"","data":""}"#), Ok(Request::Open { .. })));
        assert!(matches!(parse(br#"{"v":1,"op":"forget","purpose":"r"}"#), Ok(Request::Forget { .. })));
        assert!(matches!(parse(br#"{"v":1,"op":"status"}"#), Ok(Request::Status)));
    }

    #[test]
    fn what_is_not_a_request_is_refused_and_never_panics() {
        for bad in [
            "",
            " ",
            "{",
            "null",
            "1",
            "[]",
            "\"seal\"",
            "{}",
            r#"{"v":2,"op":"status"}"#,
            r#"{"v":"1","op":"status"}"#,
            r#"{"v":1.5,"op":"status"}"#,
            r#"{"v":1}"#,
            r#"{"v":1,"op":1}"#,
            r#"{"v":1,"op":"BAD OP"}"#,
            r#"{"v":1,"op":"seal"}"#,
            r#"{"v":1,"op":"seal","purpose":1,"record":"","data":""}"#,
            r#"{"v":1,"op":"seal","purpose":"../x","record":"","data":""}"#,
            r#"{"v":1,"op":"seal","purpose":"r","record":"a\nb","data":""}"#,
            r#"{"v":1,"op":"seal","purpose":"r","record":"","data":"!!!"}"#,
            r#"{"v":1,"op":"seal","purpose":"r","record":"","data":7}"#,
            r#"{"v":1,"op":"forget","purpose":"a/b"}"#,
            r#"{"v":1,"op":"get","kind":"password","id":".."}"#,
            r#"{"v":1,"op":"get","kind":"..","id":"a"}"#,
            r#"{"v":1,"op":"put","kind":"k","id":"../x","name":"n","value":1}"#,
            r#"{"v":1,"op":"put","kind":"k","name":"","value":1}"#,
            r#"{"v":1,"op":"put","kind":"k","name":"n"}"#,
            r#"{"v":1,"op":"put","kind":"k","name":"n","value":1,"approval":"maybe"}"#,
            r#"{"v":1,"op":"unlock","method":"magic"}"#,
            r#"{"v":1,"op":"unlock","method":"password","vault":"../x"}"#,
            r#"{"v":1,"op":"unlock","method":"password","secret":5}"#,
            r#"{"v":1,"op":"set-idle","minutes":0}"#,
            r#"{"v":1,"op":"set-idle","minutes":-1}"#,
            r#"{"v":1,"op":"set-idle","minutes":1e30}"#,
            r#"{"v":1,"op":"set-approval","kind":"k","id":"i","approval":"sometimes"}"#,
        ] {
            is(bad, Refusal::BadRequest);
        }
        // Bytes that are not text, and JSON nested far past what serde_json will follow
        assert!(parse(&[0xff, 0xfe, 0x00]).is_err());
        let deep = "[".repeat(100_000);
        assert!(parse(deep.as_bytes()).is_err());
    }

    #[test]
    fn too_much_is_too_large() {
        let data = "A".repeat((MAX_DATA + 3) / 3 * 4);
        is(&format!(r#"{{"v":1,"op":"seal","purpose":"r","record":"","data":"{data}"}}"#), Refusal::TooLarge);
        let fits = "A".repeat(MAX_DATA / 3 * 4);
        assert!(parse(format!(r#"{{"v":1,"op":"seal","purpose":"r","record":"","data":"{fits}"}}"#).as_bytes()).is_ok());
        assert!(matches!(parse(&vec![b' '; MAX_LINE + 1]), Err(Refusal::TooLarge)));
        let meta = "x".repeat(crate::store::MAX_META + 1);
        is(&format!(r#"{{"v":1,"op":"put","kind":"k","name":"n","value":1,"meta":"{meta}"}}"#), Refusal::TooLarge);
    }

    #[test]
    fn a_secret_too_long_is_refused_as_a_wrong_one_is() {
        let long = "A".repeat(4096);
        is(&format!(r#"{{"v":1,"op":"unlock","method":"password","secret":"{long}"}}"#), Refusal::Refused);
        is(&format!(r#"{{"v":1,"op":"new","name":"n","secret":"{long}"}}"#), Refusal::Refused);
    }

    #[test]
    fn what_no_table_has_comes_through_as_a_plug() {
        assert!(matches!(parse(br#"{"v":1,"op":"password-generate","length":20}"#), Ok(Request::Plug { op, .. }) if op == "password-generate"));
    }
}
