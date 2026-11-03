//! Keeping a feature's data under a key in Amethystora Vault, without ever holding that key: data goes in and comes
//! back, and the key stays in the service (plans/PLAN-vault.md, plans/PLAN-ui-ideas-r7-build.md).
//!
//! The socket is `/run/amethystora/vault.sock`, a Unix stream, and every message either way is one line of JSON, so
//! that GNOME Shell can speak it through Gio without blocking. A connection may carry any number of requests, each
//! answered in turn before the next is read, but one with nothing asked for 5 seconds is closed: a client that keeps one
//! between requests reconnects when it finds it gone.
//!
//! ```text
//! {"v":1,"op":"seal","purpose":"resume","record":"2026","data":"<base64>"}   -> {"ok":true,"data":"<base64>"}
//! {"v":1,"op":"open","purpose":"resume","record":"2026","data":"<base64>"}   -> {"ok":true,"data":"<base64>"}
//! {"v":1,"op":"forget","purpose":"resume"}                                   -> {"ok":true}
//! anything refused                                                           -> {"ok":false,"why":"closed"}
//! ```
//!
//! `why` is one of the names of [`Refusal`]. What is sealed is at most [`MAX_DATA`] bytes before base64, and a line at
//! most [`MAX_LINE`] bytes. A purpose is `[a-z0-9-]{1,40}` and a record at most 128 characters without a control
//! character. The vault's random ID, the purpose and the record are authenticated with the data, so a file cannot pass
//! for another, another purpose's or another account's. The account is the one the kernel says is asking.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use base64::Engine;
use serde_json::{json, Value};

pub const SOCKET: &str = "/run/amethystora/vault.sock";
/// What one seal or open carries, before base64
pub const MAX_DATA: usize = 1024 * 1024;
/// What sealing adds: `AVs1`, the nonce and the tag
pub const OVERHEAD: usize = 32;
/// One line of the protocol: the data in base64 and a little JSON around it
pub const MAX_LINE: usize = MAX_DATA / 3 * 4 + 4096;

/// Why the service said no. A feature keeps nothing in the clear for any of these: it waits, or works from memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The vault is set up and closed. Say "Open Vault"; never ask for it unasked.
    Closed,
    /// No vault on this account. Work from memory, and offer it once.
    NotSetUp,
    /// The person said no, or this may not be asked
    Refused,
    TooLarge,
    BadRequest,
    /// The service could not be reached, or failed. As closed, and said once.
    Failed,
}

impl Refusal {
    pub fn name(self) -> &'static str {
        match self {
            Refusal::Closed => "closed",
            Refusal::NotSetUp => "not-set-up",
            Refusal::Refused => "refused",
            Refusal::TooLarge => "too-large",
            Refusal::BadRequest => "bad-request",
            Refusal::Failed => "failed",
        }
    }

    pub fn from_name(name: &str) -> Refusal {
        match name {
            "closed" => Refusal::Closed,
            "not-set-up" => Refusal::NotSetUp,
            "refused" => Refusal::Refused,
            "too-large" => Refusal::TooLarge,
            "bad-request" => Refusal::BadRequest,
            _ => Refusal::Failed,
        }
    }
}

/// One request and its whole answer, a refusal included, over a connection of its own. For what asks a person, `wait` has to
/// be as long as a person takes.
pub fn exchange(socket: &Path, request: &Value, wait: Duration) -> Result<Value, Refusal> {
    let mut stream = UnixStream::connect(socket).map_err(|_| Refusal::Failed)?;
    stream.set_read_timeout(Some(wait)).map_err(|_| Refusal::Failed)?;
    stream.set_write_timeout(Some(wait)).map_err(|_| Refusal::Failed)?;
    let mut line = request.to_string();
    line.push('\n');
    stream.write_all(line.as_bytes()).map_err(|_| Refusal::Failed)?;
    let mut answer = String::new();
    BufReader::new(stream.take(MAX_LINE as u64)).read_line(&mut answer).map_err(|_| Refusal::Failed)?;
    serde_json::from_str(&answer).map_err(|_| Refusal::Failed)
}

fn ask(socket: &Path, request: &Value) -> Result<Value, Refusal> {
    let answer = exchange(socket, request, Duration::from_secs(10))?;
    if answer["ok"] == Value::Bool(true) {
        Ok(answer)
    } else {
        Err(Refusal::from_name(answer["why"].as_str().unwrap_or("failed")))
    }
}

fn with_data(socket: &Path, op: &str, purpose: &str, record: &str, data: &[u8]) -> Result<Vec<u8>, Refusal> {
    if data.len() > MAX_DATA + OVERHEAD {
        return Err(Refusal::TooLarge);
    }
    let engine = base64::engine::general_purpose::STANDARD;
    let answer = ask(socket, &json!({"v": 1, "op": op, "purpose": purpose, "record": record, "data": engine.encode(data)}))?;
    engine.decode(answer["data"].as_str().ok_or(Refusal::Failed)?).map_err(|_| Refusal::Failed)
}

/// `plain`, sealed under this account's key for `purpose`
pub fn seal(purpose: &str, record: &str, plain: &[u8]) -> Result<Vec<u8>, Refusal> {
    seal_at(Path::new(SOCKET), purpose, record, plain)
}

/// What [`seal`] sealed, given the same purpose and record
pub fn open(purpose: &str, record: &str, sealed: &[u8]) -> Result<Vec<u8>, Refusal> {
    open_at(Path::new(SOCKET), purpose, record, sealed)
}

/// Removes the key for `purpose`, after which nothing sealed under it can be read again
pub fn forget(purpose: &str) -> Result<(), Refusal> {
    forget_at(Path::new(SOCKET), purpose)
}

/// [`seal`], at another socket: a test's
pub fn seal_at(socket: &Path, purpose: &str, record: &str, plain: &[u8]) -> Result<Vec<u8>, Refusal> {
    if plain.len() > MAX_DATA {
        return Err(Refusal::TooLarge);
    }
    with_data(socket, "seal", purpose, record, plain)
}

/// [`open`], at another socket
pub fn open_at(socket: &Path, purpose: &str, record: &str, sealed: &[u8]) -> Result<Vec<u8>, Refusal> {
    with_data(socket, "open", purpose, record, sealed)
}

/// [`forget`], at another socket
pub fn forget_at(socket: &Path, purpose: &str) -> Result<(), Refusal> {
    ask(socket, &json!({"v": 1, "op": "forget", "purpose": purpose})).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_keeps_its_name_on_the_wire() {
        for refusal in [Refusal::Closed, Refusal::NotSetUp, Refusal::Refused, Refusal::TooLarge, Refusal::BadRequest, Refusal::Failed] {
            assert_eq!(Refusal::from_name(refusal.name()), refusal);
        }
        assert_eq!(Refusal::from_name("anything else"), Refusal::Failed);
    }

    #[test]
    fn a_line_holds_the_largest_data_in_base64() {
        assert!(MAX_LINE > (MAX_DATA + OVERHEAD).div_ceil(3) * 4 + 256);
    }
}
