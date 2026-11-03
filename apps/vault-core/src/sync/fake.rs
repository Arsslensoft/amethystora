//! A server of our own that answers the protocol as spec-protocol.md describes it, for tests: honest by default, and hostile
//! when told, each attack of the spec's section 10.2 a knob. It keeps every request, so that a test can say what left the
//! machine. Built only for tests (`fake-server`); nothing shipped has it.

use std::collections::HashMap;

use base64::Engine;
use serde_json::{json, Map, Value};
use zeroize::Zeroizing;

use super::api::{Http, NetFailure, Request, Response};
use super::crypto::{self, EncString, Kdf, SymKey};

pub const USER_ID: &str = "0b1c2d3e-4f50-6172-8394-a5b6c7d8e9f0";
pub const EMAIL: &str = "person@example.test";
pub const PASSWORD: &str = "a master password for tests";
pub const API_ID: &str = "user.0b1c2d3e-4f50-6172-8394-a5b6c7d8e9f0";
pub const API_SECRET: &str = "an-api-key-secret";
pub const BASE: &str = "https://vault.example.test";

/// One request as it reached the server
#[derive(Clone, Debug)]
pub struct Seen {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

pub type Hook = Box<dyn FnMut(&mut Value) + Send>;

pub struct Fake {
    pub kdf: Kdf,
    pub user: SymKey,
    pub protected: String,
    pub hash: String,
    pub ciphers: Vec<Value>,
    pub seen: Vec<Seen>,
    /// Ticks once a write, so that every revision date is new
    pub clock: i64,
    pub two_factor: bool,
    pub refresh_count: usize,
    /// Changes what a response says before it goes, by route
    pub hooks: HashMap<&'static str, Hook>,
    /// Answers a route with this status and an empty body
    pub status: HashMap<&'static str, u16>,
    pub revision: u64,
    /// Where this server says it is: requests to any other address are refused
    pub base: String,
    /// Files uploaded, by cipher and attachment id
    pub files: HashMap<(String, String), Vec<u8>>,
    /// Slots dropped by the one delete there is
    pub dropped: Vec<(String, String)>,
    pub premium: bool,
    /// What the slot call answers to a refusal: a status (400, with Vaultwarden's words), none to accept
    pub slot_status: Option<u16>,
    /// The `fileUploadType` the slot call answers with: 0 is direct
    pub upload_type: i64,
    /// The host a download address names, where it is not the server's own (a wrong `DOMAIN`)
    pub download_host: Option<String>,
    /// An address of another shape, for a server that keeps its files elsewhere
    pub download_url: Option<String>,
    /// Uploads that answer 200 and keep nothing
    pub swallow_uploads: bool,
    /// Bitwarden moves a cipher's date when an attachment comes or goes; Vaultwarden does not
    pub bump_on_attachment: bool,
    /// A server that does not keep an SSH key item: it answers `sshKey: null`
    pub no_ssh: bool,
}

fn time_text(at: i64) -> String {
    // Seven fractional digits, as Bitwarden's server writes them
    let seconds = 1_790_000_000 + at;
    let days = seconds / 86_400;
    let rest = seconds % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.1234567Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

fn uuid(n: usize) -> String {
    format!("{:08x}-0000-4000-8000-{:012x}", n, n)
}

impl Default for Fake {
    fn default() -> Fake {
        Fake::new(Kdf { kind: 0, iterations: 600_000, memory: None, parallelism: None })
    }
}

impl Fake {
    pub fn new(kdf: Kdf) -> Fake {
        // Settings the client refuses to compute (a hostile server's) get a stand-in: no login is ever made with them
        let master = crypto::master_key(PASSWORD.as_bytes(), EMAIL, &kdf).unwrap_or_else(|_| Zeroizing::new([0u8; 32]));
        let stretched = crypto::stretch(&master);
        let user = SymKey::random();
        let protected = EncString::seal(&stretched, user.bytes()).text();
        let hash = crypto::master_hash(&master, PASSWORD.as_bytes()).to_string();
        Fake { kdf, user, protected, hash, ciphers: Vec::new(), seen: Vec::new(), clock: 0, two_factor: false, refresh_count: 0, hooks: HashMap::new(), status: HashMap::new(), revision: 1, base: BASE.to_string(), files: HashMap::new(), dropped: Vec::new(), premium: true, slot_status: None, upload_type: 0, download_host: None, download_url: None, swallow_uploads: false, bump_on_attachment: false, no_ssh: false }
    }

    pub fn now(&mut self) -> String {
        self.clock += 2;
        self.revision += 1;
        time_text(self.clock)
    }

    pub fn access(&self) -> String {
        let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json!({"sub": USER_ID}).to_string());
        format!("header.{claims}.signature")
    }

    pub fn find(&self, id: &str) -> Option<&Value> {
        self.ciphers.iter().find(|raw| raw["id"] == json!(id))
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut Value> {
        self.ciphers.iter_mut().find(|raw| raw["id"] == json!(id))
    }

    /// A second client's write: what another app does to an item, encrypted as it would
    pub fn store(&mut self, mut cipher: Value) -> String {
        let id = uuid(self.ciphers.len() + 1000);
        let now = self.now();
        cipher["id"] = json!(id);
        cipher["object"] = json!("cipherDetails");
        cipher["creationDate"] = json!(now);
        cipher["revisionDate"] = json!(now);
        cipher.as_object_mut().map(|o| o.entry("deletedDate").or_insert(Value::Null));
        cipher.as_object_mut().map(|o| o.remove("encryptedFor"));
        cipher.as_object_mut().map(|o| o.remove("lastKnownRevisionDate"));
        cipher.as_object_mut().map(|o| o.entry("attachments").or_insert(Value::Null));
        if self.no_ssh && cipher["type"] == json!(5) {
            cipher["sshKey"] = Value::Null;
        }
        self.ciphers.push(cipher);
        id
    }

    /// Marks an item changed by another client
    pub fn touch(&mut self, id: &str) {
        let now = self.now();
        if let Some(raw) = self.find_mut(id) {
            raw["revisionDate"] = json!(now);
        }
    }

    /// What another client does: a file on a cipher, with its key and name made as an app makes them (the cipher's key wraps the file's)
    pub fn attach(&mut self, cipher: &str, name: &str, plain: &[u8]) -> String {
        let raw = self.find(cipher).cloned().expect("a cipher");
        let item = super::cipher::key_of(&raw, &self.user).expect("its key");
        let prepared = super::attach::prepare(&item, name, plain).expect("a file");
        let id = format!("{:010x}", self.files.len() + 1);
        self.files.insert((cipher.to_string(), id.clone()), prepared.bytes.to_vec());
        self.record(cipher, &id, &prepared.key, &prepared.name, prepared.size());
        id
    }

    fn record(&mut self, cipher: &str, id: &str, key: &str, name: &str, size: usize) {
        let bump = self.bump_on_attachment;
        let now = self.now();
        let Some(raw) = self.find_mut(cipher) else { return };
        let mut list = raw["attachments"].as_array().cloned().unwrap_or_default();
        list.push(json!({"object": "attachment", "id": id, "url": "unused", "fileName": name, "key": key, "size": size.to_string(), "sizeName": format!("{size} Bytes")}));
        raw["attachments"] = Value::Array(list);
        if bump {
            raw["revisionDate"] = json!(now);
        }
    }

    /// The calls about files: a slot, the bytes, what a file is, its bytes, and the one delete
    fn files(&mut self, route: &str, path: &str, headers: &[(&'static str, String)], body: &[u8]) -> (u16, Vec<u8>) {
        let json_of = |status: u16, value: Value| (status, serde_json::to_vec(&value).unwrap_or_default());
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        match (route, parts.as_slice()) {
            ("slot", ["api", "ciphers", id, "attachment", "v2"]) => {
                if let Some(status) = self.slot_status {
                    return json_of(status, json!({"message": "Attachments are disabled"}));
                }
                let Some(cipher) = self.find(id).cloned() else { return json_of(400, json!({"message": "Cipher doesn't exist"})) };
                let request: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
                let Some(size) = request["fileSize"].as_u64() else { return json_of(400, json!({"message": "fileSize"})) };
                let attachment = format!("{:010x}", self.files.len() + 1000 + cipher["attachments"].as_array().map_or(0, Vec::len));
                self.record(id, &attachment, request["key"].as_str().unwrap_or(""), request["fileName"].as_str().unwrap_or(""), size as usize);
                // The record is there before any file, as on both servers
                let now = self.find(id).cloned().unwrap_or_default();
                json_of(200, json!({"object": "attachment-fileUpload", "attachmentId": attachment, "url": format!("/ciphers/{id}/attachment/{attachment}"), "fileUploadType": self.upload_type, "cipherResponse": now}))
            }
            ("upload", ["api", "ciphers", id, "attachment", attachment]) => {
                let boundary = headers.iter().find(|(name, _)| name.eq_ignore_ascii_case("content-type")).and_then(|(_, value)| value.split("boundary=").nth(1)).map(str::to_string);
                let Some(boundary) = boundary else { return json_of(400, json!({"message": "Invalid content"})) };
                let start = format!("--{boundary}\r\n");
                let text_start = body.windows(4).position(|window| window == b"\r\n\r\n");
                let end_marker = format!("\r\n--{boundary}--\r\n");
                let end = body.windows(end_marker.len()).rposition(|window| window == end_marker.as_bytes());
                let (Some(from), Some(to)) = (text_start, end) else { return json_of(400, json!({"message": "Invalid content"})) };
                if !body.starts_with(start.as_bytes()) || to < from + 4 || !String::from_utf8_lossy(&body[..from]).contains("name=\"data\"; filename=") {
                    return json_of(400, json!({"message": "Invalid content"}));
                }
                let data = body[from + 4..to].to_vec();
                if self.swallow_uploads {
                    return (200, Vec::new());
                }
                let announced: Option<usize> = self.find(id).and_then(|raw| raw["attachments"].as_array().and_then(|list| list.iter().find(|one| one["id"] == json!(attachment)).and_then(|one| one["size"].as_str().and_then(|size| size.parse().ok()))));
                if announced.is_none_or(|announced| announced.abs_diff(data.len()) > 1024 * 1024) {
                    return json_of(400, json!({"message": "Attachment size mismatch"}));
                }
                self.files.insert((id.to_string(), attachment.to_string()), data.clone());
                // The size is what arrived
                if let Some(raw) = self.find_mut(id)
                    && let Some(list) = raw["attachments"].as_array_mut()
                    && let Some(one) = list.iter_mut().find(|one| one["id"] == json!(attachment))
                {
                    one["size"] = json!(data.len().to_string());
                }
                (200, Vec::new())
            }
            ("info", ["api", "ciphers", id, "attachment", attachment]) => {
                let Some(one) = self.find(id).and_then(|raw| raw["attachments"].as_array().and_then(|list| list.iter().find(|one| one["id"] == json!(attachment)).cloned())) else {
                    return json_of(400, json!({"message": "Attachment doesn't exist"}));
                };
                let host = self.download_host.clone().unwrap_or_else(|| self.base.clone());
                let url = self.download_url.clone().unwrap_or_else(|| format!("{host}/attachments/{id}/{attachment}?token=aaaa.bbbb.cccc"));
                let mut answer = one;
                answer["url"] = json!(url);
                json_of(200, answer)
            }
            ("bytes", ["attachments", id, attachment]) => match self.files.get(&(id.to_string(), attachment.to_string())) {
                Some(data) => (200, data.clone()),
                None => json_of(404, json!({})),
            },
            ("drop", ["api", "ciphers", id, "attachment", attachment]) => {
                self.dropped.push((id.to_string(), attachment.to_string()));
                self.files.remove(&(id.to_string(), attachment.to_string()));
                if let Some(raw) = self.find_mut(id)
                    && let Some(list) = raw["attachments"].as_array_mut()
                {
                    list.retain(|one| one["id"] != json!(attachment));
                }
                json_of(200, json!({"cipher": {}}))
            }
            _ => json_of(404, json!({})),
        }
    }

    fn prelogin(&self) -> Value {
        json!({"kdf": self.kdf.kind, "kdfIterations": self.kdf.iterations, "kdfMemory": self.kdf.memory, "kdfParallelism": self.kdf.parallelism,
               "kdfSettings": {"kdfType": self.kdf.kind, "iterations": self.kdf.iterations, "memory": self.kdf.memory, "parallelism": self.kdf.parallelism}, "salt": null})
    }

    fn token(&mut self, form: &HashMap<String, String>) -> (u16, Value) {
        let grant = form.get("grant_type").map(String::as_str);
        let ok = match grant {
            Some("password") => form.get("password") == Some(&self.hash) && form.get("username").map(String::as_str) == Some(EMAIL),
            Some("client_credentials") => form.get("client_id").map(String::as_str) == Some(API_ID) && form.get("client_secret").map(String::as_str) == Some(API_SECRET),
            Some("refresh_token") => {
                self.refresh_count += 1;
                form.get("refresh_token").map(String::as_str) == Some("refresh-1")
            }
            _ => false,
        };
        if !ok {
            return (400, json!({"error": "invalid_grant", "error_description": "invalid_username_or_password", "ErrorModel": {"Message": "Username or password is incorrect. Try again."}}));
        }
        if grant == Some("password") && self.two_factor && form.get("twoFactorToken").map(String::as_str) != Some("123456") {
            return (400, json!({"error": "invalid_grant", "TwoFactorProviders": ["0"], "TwoFactorProviders2": {"0": null}}));
        }
        let mut answer = json!({"access_token": self.access(), "token_type": "Bearer", "expires_in": 7200, "scope": "api offline_access"});
        if grant != Some("refresh_token") {
            answer["Key"] = json!(self.protected);
            answer["PrivateKey"] = json!("2.AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=");
            answer["Kdf"] = json!(self.kdf.kind);
            answer["KdfIterations"] = json!(self.kdf.iterations);
            answer["KdfMemory"] = json!(self.kdf.memory);
            answer["KdfParallelism"] = json!(self.kdf.parallelism);
            answer["UserDecryptionOptions"] = json!({"HasMasterPassword": true, "MasterPasswordUnlock": {"Kdf": {"KdfType": self.kdf.kind, "Iterations": self.kdf.iterations, "Memory": self.kdf.memory, "Parallelism": self.kdf.parallelism}, "MasterKeyEncryptedUserKey": self.protected, "Salt": EMAIL}});
            answer["AccountKeys"] = json!({"publicKeyEncryptionKeyPair": {"publicKey": "AAAA"}, "signatureKeyPair": null, "securityState": null});
        }
        if grant != Some("client_credentials") {
            answer["refresh_token"] = json!("refresh-1");
        }
        (200, answer)
    }

    fn sync(&self) -> Value {
        json!({
            "object": "sync",
            "profile": {"id": USER_ID, "email": EMAIL, "premium": self.premium, "key": self.protected, "privateKey": "2.AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=", "securityStamp": "x", "organizations": []},
            "folders": [],
            "collections": [],
            "ciphers": self.ciphers,
            "userDecryption": {"masterPasswordUnlock": {"kdf": {"kdfType": self.kdf.kind, "iterations": self.kdf.iterations, "memory": self.kdf.memory, "parallelism": self.kdf.parallelism}, "masterKeyEncryptedUserKey": self.protected, "salt": EMAIL}},
        })
    }

    fn write(&mut self, method: &str, path: &str, body: &Value) -> (u16, Value) {
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        match (method, parts.as_slice()) {
            ("POST", ["ciphers"]) => {
                if body.get("encryptedFor") != Some(&json!(USER_ID)) {
                    return (422, json!({"message": "encryptedFor"}));
                }
                let id = self.store(body.clone());
                (200, self.find(&id).cloned().unwrap_or_default())
            }
            ("PUT", ["ciphers", id]) => {
                let now = self.now();
                let Some(stored) = self.find(id).cloned() else { return (404, json!({})) };
                let then = stored["revisionDate"].as_str().and_then(super::cipher::parse_time).unwrap_or(0);
                let sent = body["lastKnownRevisionDate"].as_str().and_then(super::cipher::parse_time);
                if sent.is_some_and(|sent| (then - sent).abs() > 1_000_000_000) {
                    return (400, json!({"message": "The client copy of this cipher is out of date. Resync the client and try again."}));
                }
                let mut new: Map<String, Value> = body.as_object().cloned().unwrap_or_default();
                new.remove("encryptedFor");
                new.remove("lastKnownRevisionDate");
                for name in ["id", "object", "creationDate", "deletedDate", "attachments"] {
                    new.insert(name.into(), stored.get(name).cloned().unwrap_or(Value::Null));
                }
                new.insert("revisionDate".into(), json!(now));
                let new = Value::Object(new);
                *self.find_mut(id).expect("there") = new.clone();
                (200, new)
            }
            ("PUT", ["ciphers", id, "delete"]) => {
                let now = self.now();
                let Some(stored) = self.find_mut(id) else { return (404, json!({})) };
                stored["deletedDate"] = json!(now);
                stored["revisionDate"] = json!(now);
                (200, Value::Null)
            }
            ("PUT", ["ciphers", id, "restore"]) => {
                let now = self.now();
                let Some(stored) = self.find_mut(id) else { return (404, json!({})) };
                stored["deletedDate"] = Value::Null;
                stored["revisionDate"] = json!(now);
                (200, stored.clone())
            }
            _ => (404, json!({})),
        }
    }
}

impl Http for Fake {
    fn call(&mut self, request: Request) -> Result<Response, NetFailure> {
        let body_text = String::from_utf8_lossy(&request.body).to_string();
        self.seen.push(Seen { method: request.method.to_string(), url: request.url.clone(), headers: request.headers.iter().map(|(n, v)| (n.to_string(), v.clone())).collect(), body: body_text.clone() });
        let Some(path) = request.url.clone().strip_prefix(self.base.as_str()).map(str::to_string) else { return Err(NetFailure::NotAllowed) };
        let route: &'static str = match (request.method, path.split('?').next().unwrap_or("")) {
            ("POST", "/identity/accounts/prelogin/password") => "prelogin",
            ("POST", "/identity/connect/token") => "token",
            ("GET", "/api/sync") => "sync",
            ("GET", "/api/accounts/revision-date") => "revision",
            ("GET", "/api/config") => "config",
            ("POST", p) if p.starts_with("/api/ciphers/") && p.ends_with("/attachment/v2") => "slot",
            ("POST", p) if p.starts_with("/api/ciphers/") && p.contains("/attachment/") => "upload",
            ("GET", p) if p.starts_with("/api/ciphers/") && p.contains("/attachment/") => "info",
            ("GET", p) if p.starts_with("/attachments/") => "bytes",
            ("DELETE", p) if p.starts_with("/api/ciphers/") && p.contains("/attachment/") => "drop",
            ("GET", p) if p.starts_with("/api/ciphers/") => "fetch",
            ("DELETE", _) => "destroy",
            ("POST", p) if p.ends_with("/delete") => "destroy",
            ("PUT", p) if p.ends_with("/delete") => "trash",
            _ => "write",
        };
        if let Some(status) = self.status.get(route) {
            return Ok(Response { status: *status, body: Zeroizing::new(Vec::new()) });
        }
        let (status, mut answer) = match route {
            "prelogin" => (200, self.prelogin()),
            "token" => {
                let form: HashMap<String, String> = body_text.split('&').filter_map(|pair| pair.split_once('=')).map(|(k, v)| (decode(k), decode(v))).collect();
                self.token(&form)
            }
            "sync" => (200, self.sync()),
            "revision" => (200, json!(self.revision)),
            "config" => (200, json!({"version": "2026.6.0", "server": {"name": "Fake"}, "environment": {"api": "https://evil.test/api", "identity": "https://evil.test/identity"}})),
            "fetch" => {
                let id = path.trim_start_matches("/api/ciphers/");
                match self.find(id) {
                    Some(raw) => (200, raw.clone()),
                    None => (404, json!({})),
                }
            }
            "slot" | "upload" | "info" | "bytes" | "drop" => {
                let (status, body) = self.files(route, path.split('?').next().unwrap_or(""), &request.headers, &request.body);
                return Ok(Response { status, body: Zeroizing::new(body) });
            }
            "destroy" => panic!("a permanent delete was asked for: {} {}", request.method, request.url),
            _ => {
                let body: Value = serde_json::from_str(&body_text).unwrap_or(Value::Null);
                let path = path.trim_start_matches("/api").to_string();
                self.write(request.method, &path, &body)
            }
        };
        if let Some(hook) = self.hooks.get_mut(route) {
            hook(&mut answer);
        }
        let body = if answer.is_null() { Vec::new() } else { serde_json::to_vec(&answer).unwrap_or_default() };
        Ok(Response { status, body: Zeroizing::new(body) })
    }
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && at + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(std::str::from_utf8(&bytes[at + 1..at + 3]).unwrap_or(""), 16)
        {
            out.push(byte);
            at += 3;
            continue;
        }
        out.push(bytes[at]);
        at += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}
