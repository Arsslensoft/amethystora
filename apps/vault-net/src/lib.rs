//! amethystora-vault-net, the Vault's only way out (plans/done/PLAN-vault.md, "Sync with Vaultwarden and Bitwarden"). The vault's
//! service, which has no network, hands it one request at a time over a socket only that service can open, and it hands
//! back what the server said. It holds no key and keeps nothing: Bitwarden's protocol encrypts on the client, so what passes
//! through here is ciphertext, a token and at a password login the master password's hash, each for the one request.
//!
//! One line of JSON each way, as the Vault's own socket:
//!
//! ```text
//! {"v":1,"op":"session","origins":["https://vault.example.test:443"],"ca":PEM?,"pin":HEX?}   -> {"ok":true}
//! {"v":1,"op":"request","method":"GET","url":"...","headers":[["Name","value"]],"body":B64} -> {"ok":true,"status":200,"body":B64}
//! {"v":1,"op":"probe","origin":"https://vault.example.test:443"}                             -> {"ok":true,"trusted":bool,"chain":[...]}
//! anything refused                                                                          -> {"ok":false,"why":"not-allowed"}
//! ```
//!
//! A request needs a session first, on the same connection, and reaches only its origins.

pub mod alloc;
pub mod http;

use std::io::{BufRead, Read, Write};

use base64::Engine;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use http::{Failure, Session};

pub const SOCKET: &str = "/run/amethystora/vault-net.sock";
/// A request's line: the largest body the Vault sends is one item, far under this
pub const MOST_REQUEST: usize = 4 * 1024 * 1024;
/// An answer's line: the largest body, in base64, and a little JSON
pub const MOST_ANSWER: usize = http::MOST_BODY / 3 * 4 + 4096;

/// Makes a session from the origins, the CA and the pin a `session` line named
pub type MakeSession = dyn Fn(Vec<String>, Option<&str>, Option<&str>) -> Result<Session, Failure>;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

fn refusal(why: Failure) -> Value {
    json!({"ok": false, "why": why.name()})
}

/// One connection's lines, each answered before the next is read, until the other end closes
pub fn serve(reader: &mut dyn BufRead, writer: &mut dyn Write, make_session: &MakeSession) {
    let mut session: Option<Session> = None;
    loop {
        let mut line = Zeroizing::new(Vec::new());
        match (&mut *reader).take(MOST_REQUEST as u64 + 1).read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if line.len() > MOST_REQUEST || line.last() != Some(&b'\n') {
            let _ = writeln!(writer, "{}", refusal(Failure::TooLarge));
            return;
        }
        let answer = answer(&line, &mut session, make_session);
        let text = Zeroizing::new(answer.to_string());
        if writer.write_all(text.as_bytes()).and_then(|()| writer.write_all(b"\n")).and_then(|()| writer.flush()).is_err() {
            return;
        }
    }
}

fn answer(line: &[u8], session: &mut Option<Session>, make_session: &MakeSession) -> Value {
    let Ok(body) = serde_json::from_slice::<Value>(line) else { return refusal(Failure::NotAllowed) };
    if body.get("v").and_then(Value::as_u64) != Some(1) {
        return refusal(Failure::NotAllowed);
    }
    match body.get("op").and_then(Value::as_str) {
        Some("session") => {
            let origins: Vec<String> = body.get("origins").and_then(Value::as_array).into_iter().flatten().filter_map(|o| o.as_str().map(str::to_string)).collect();
            let text = |name: &str| body.get(name).and_then(Value::as_str);
            match make_session(origins, text("ca"), text("pin")) {
                Ok(made) => {
                    *session = Some(made);
                    json!({"ok": true})
                }
                Err(why) => refusal(why),
            }
        }
        Some("request") => {
            let Some(session) = session.as_mut() else { return refusal(Failure::NotAllowed) };
            let (Some(method), Some(url)) = (body.get("method").and_then(Value::as_str), body.get("url").and_then(Value::as_str)) else { return refusal(Failure::NotAllowed) };
            let headers: Vec<(String, String)> = body
                .get("headers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|pair| Some((pair.get(0)?.as_str()?.to_string(), pair.get(1)?.as_str()?.to_string())))
                .take(32)
                .collect();
            let Ok(sent) = B64.decode(body.get("body").and_then(Value::as_str).unwrap_or("")).map(Zeroizing::new) else { return refusal(Failure::NotAllowed) };
            match session.request(method, url, &headers, &sent) {
                Ok((status, received)) => json!({"ok": true, "status": status, "body": B64.encode(&received[..])}),
                Err(why) => refusal(why),
            }
        }
        Some("probe") => match body.get("origin").and_then(Value::as_str).map(http::probe) {
            Some(Ok(seen)) => {
                let chain: Vec<Value> = seen.chain.into_iter().map(|(subject, issuer, sha256, pem, self_signed)| json!({"subject": subject, "issuer": issuer, "sha256": sha256, "pem": pem, "self_signed": self_signed})).collect();
                json!({"ok": true, "trusted": seen.trusted, "chain": chain})
            }
            Some(Err(why)) => refusal(why),
            None => refusal(Failure::NotAllowed),
        },
        _ => refusal(Failure::NotAllowed),
    }
}
