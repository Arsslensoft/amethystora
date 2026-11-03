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
        Fake { kdf, user, protected, hash, ciphers: Vec::new(), seen: Vec::new(), clock: 0, two_factor: false, refresh_count: 0, hooks: HashMap::new(), status: HashMap::new(), revision: 1, base: BASE.to_string() }
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
            "profile": {"id": USER_ID, "email": EMAIL, "key": self.protected, "privateKey": "2.AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAA==|AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=", "securityStamp": "x", "organizations": []},
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
            ("GET", p) if p.starts_with("/api/ciphers/") => "fetch",
            ("DELETE", _) => "destroy",
            ("POST", p) if p.ends_with("/delete") => "destroy",
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
