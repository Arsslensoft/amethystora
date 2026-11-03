//! Test code, built only with `fake-server`: makes an account on a Vaultwarden in a container, as the web vault would, so that
//! the tests against it have something to log in to (plans/vault-sync/IMPLEMENTERS.md 3). The program never makes keys for an
//! account (spec 10.3), and nothing here is in it.

use base64::Engine;
use openssl::pkey::PKey;
use openssl::rsa::Rsa;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use super::api::{Http, Request};
use super::crypto::{self, EncString, Kdf, SymKey};
use super::json;
use super::{CLIENT_VERSION, DEVICE_TYPE};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn headers() -> Vec<(&'static str, String)> {
    vec![
        ("Accept", "application/json".to_string()),
        ("Content-Type", "application/json".to_string()),
        ("Bitwarden-Client-Version", CLIENT_VERSION.to_string()),
        ("Device-Type", DEVICE_TYPE.to_string()),
    ]
}

/// One JSON call, unauthenticated or with a token: the status and what the body parsed as
pub fn call(http: &mut dyn Http, method: &'static str, url: &str, access: Option<&str>, body: Option<&Value>) -> (u16, Value) {
    let mut headers = headers();
    if let Some(access) = access {
        headers.push(("Authorization", format!("Bearer {access}")));
    }
    let body = Zeroizing::new(body.map(|body| serde_json::to_vec(body).unwrap()).unwrap_or_default());
    let response = http.call(Request { method, url: url.to_string(), headers, body }).expect("the server answered");
    let parsed = serde_json::from_slice(&response.body).unwrap_or(Value::Null);
    (response.status, parsed)
}

/// Registers `email` with this master password and KDF, with a user key of 64 random bytes and an RSA key pair, which is what
/// a web vault does and this program never does. `identity` is the server's `<address>/identity`.
pub fn register(http: &mut dyn Http, identity: &str, email: &str, password: &str, kdf: &Kdf) -> Result<(), String> {
    let (status, token) = call(http, "POST", &format!("{identity}/accounts/register/send-verification-email"), None, Some(&json!({"email": email, "name": "Test"})));
    if status != 200 {
        return Err(format!("send-verification-email: {status} {token}"));
    }
    let token = token.as_str().ok_or("no token")?.to_string();
    let master = crypto::master_key(password.as_bytes(), email, kdf).map_err(|why| format!("{why:?}"))?;
    let stretched = crypto::stretch(&master);
    let hash = crypto::master_hash(&master, password.as_bytes());
    let user = SymKey::random();
    let rsa = PKey::from_rsa(Rsa::generate(2048).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let public = rsa.public_key_to_der().map_err(|e| e.to_string())?;
    let private = Zeroizing::new(rsa.private_key_to_pkcs8().map_err(|e| e.to_string())?);
    let body = json!({
        "email": email,
        "emailVerificationToken": token,
        "masterPasswordHash": hash.as_str(),
        "masterPasswordHint": null,
        "userSymmetricKey": EncString::seal(&stretched, user.bytes()).text(),
        "kdf": kdf.kind,
        "kdfIterations": kdf.iterations,
        "kdfMemory": kdf.memory,
        "kdfParallelism": kdf.parallelism,
        "userAsymmetricKeys": {"publicKey": B64.encode(public), "encryptedPrivateKey": EncString::seal(&user, &private).text()},
    });
    let (status, answer) = call(http, "POST", &format!("{identity}/accounts/register/finish"), None, Some(&body));
    if status == 200 { Ok(()) } else { Err(format!("register/finish: {status} {answer}")) }
}

/// The personal API key's secret (`POST /api/accounts/api-key`, after a password login); its `client_id` is `user.` and the account's id
pub fn api_key_secret(http: &mut dyn Http, api: &str, access: &str, email: &str, password: &str, kdf: &Kdf) -> Result<String, String> {
    let master = crypto::master_key(password.as_bytes(), email, kdf).map_err(|why| format!("{why:?}"))?;
    let hash = crypto::master_hash(&master, password.as_bytes());
    let (status, answer) = call(http, "POST", &format!("{api}/accounts/api-key"), Some(access), Some(&json!({"masterPasswordHash": hash.as_str()})));
    if status != 200 {
        return Err(format!("api-key: {status} {answer}"));
    }
    json::text(&answer, "apiKey").map(str::to_string).ok_or_else(|| "no apiKey".to_string())
}
