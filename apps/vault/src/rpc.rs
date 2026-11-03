//! Speaking to the service, always off the window's thread: one request, one connection, as `exchange` has it. What it answers
//! is wiped when the page has done with it, and so is what was asked. Every operation is by its name in
//! `plans/vault/work/core.md` or `ops.md`, and a page says what it means by a refusal in its own words.

use std::io::{Read, Write};
use std::ops::Deref;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use amethystora_ui::{gio, t};
use amethystora_vault_client::{MAX_LINE, SOCKET};
use amethystora_vault_core::protocol;
use base64::Engine;
use serde_json::{json, Value};
use zeroize::{Zeroize, Zeroizing};

/// A person has five minutes to answer the service's prompt, and it answers for them after that
const PATIENCE: Duration = Duration::from_secs(6 * 60);
/// What never asks anyone is answered at once or not at all
const QUICK: Duration = Duration::from_secs(10);

/// Operations that read what the service holds in its index, or touch no vault: they never prompt, and the service does not count
/// them as use, so they neither wait long nor move the window's close deadline
fn quick(op: &str) -> bool {
    matches!(
        op,
        "status" | "methods" | "list" | "lock" | "key-types" | "key-public" | "cert-profiles" | "cert-list" | "cert-show" | "cert-chain" | "ca-cert" | "machine" | "password-generate" | "password-reused" | "auth-list" | "codes-list" | "codes-coverage" | "sync-status"
    )
}

/// Whether the service counts this as use of the vault, which keeps it open
pub fn is_use(request: &Value) -> bool {
    !request["op"].as_str().is_some_and(quick)
}

/// Why the service said no
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trouble {
    Closed,
    NotSetUp,
    /// The person said no, or it did not match. `wait`: too many wrong passwords, try again in so many seconds; `problem`: a word
    /// from the service saying what to fix (`ops.md`)
    Refused { wait: u64, problem: Option<String> },
    /// A file put back from before
    Stale,
    TooLarge,
    Failed,
}

impl Trouble {
    /// The sentence for one nobody has anything more particular to say about
    pub fn words(&self) -> String {
        match self {
            Trouble::Closed => t!("The vault is closed."),
            Trouble::NotSetUp => t!("There is no vault on this account yet."),
            Trouble::Refused { wait, .. } if *wait > 0 => t!("Too many wrong tries. Try again in {seconds, plural, one {# second} other {# seconds}}.", seconds = *wait),
            Trouble::Refused { problem: Some(problem), .. } => problem_words(problem),
            Trouble::Refused { .. } => t!("That was refused."),
            Trouble::Stale => t!("A file in the vault has been put back from before. Nothing was changed."),
            Trouble::TooLarge => t!("That is too large for the vault."),
            Trouble::Failed => t!("The vault's service could not do that. Try again in a moment."),
        }
    }
}

/// What the service says of a certificate or a key it refused, as `ops.md` lists the words
fn problem_words(problem: &str) -> String {
    match problem {
        "outside-limits" => t!("Only local names and private addresses can go in this certificate."),
        "name-looks-like-host" => t!("That name looks like a host name, which this certificate cannot carry."),
        "not-exportable" => t!("This key was made so that it can never leave the vault."),
        "guest" => t!("A guest account has no certificates."),
        "bad-password" => t!("That password cannot be used."),
        "expired" => t!("This certificate is past its dates."),
        "bad-field" | "missing-field" | "not-in-profile" => t!("Something in the form cannot go in this certificate."),
        _ => t!("That was refused."),
    }
}

/// An answer, wiped when it is dropped: anything in it may be a secret
pub struct Answer(pub Value);

impl Deref for Answer {
    type Target = Value;

    fn deref(&self) -> &Value {
        &self.0
    }
}

impl Drop for Answer {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// Every string in a value overwritten where it is
pub fn wipe(value: &mut Value) {
    match value {
        Value::String(text) => text.zeroize(),
        Value::Array(list) => list.iter_mut().for_each(wipe),
        Value::Object(map) => map.iter_mut().for_each(|(_, value)| wipe(value)),
        _ => {}
    }
}

/// The size `value` takes written out, without writing it anywhere
fn written_size(value: &Value) -> Option<usize> {
    struct Count(usize);
    impl Write for Count {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += bytes.len();
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(0);
    serde_json::to_writer(&mut count, value).ok()?;
    Some(count.0)
}

/// `vault-client`'s `exchange`, but with every buffer one that wipes itself: the request is written into one of the exact size it
/// takes, and the answer is read into one that is never grown, so no earlier copy of either is left behind in the heap.
fn exchange(request: &Value, wait: Duration) -> Option<Value> {
    let size = written_size(request)?;
    let mut line = Zeroizing::new(Vec::with_capacity(size + 1));
    serde_json::to_writer(&mut *line, request).ok()?;
    line.push(b'\n');
    let mut stream = UnixStream::connect(SOCKET).ok()?;
    stream.set_read_timeout(Some(wait)).ok()?;
    stream.set_write_timeout(Some(wait)).ok()?;
    stream.write_all(&line).ok()?;
    drop(line);
    let mut answer: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::with_capacity(MAX_LINE + 1));
    let mut chunk = Zeroizing::new([0u8; 8192]);
    loop {
        let wanted = (MAX_LINE + 1 - answer.len()).min(chunk.len());
        let got = stream.read(&mut chunk[..wanted]).ok()?;
        if got == 0 {
            break;
        }
        answer.extend_from_slice(&chunk[..got]);
        if chunk[..got].contains(&b'\n') || answer.len() > MAX_LINE {
            break;
        }
    }
    let end = answer.iter().position(|byte| *byte == b'\n').unwrap_or(answer.len());
    serde_json::from_slice(&answer[..end]).ok()
}

/// One operation. Off the window's thread, and it wipes the request there.
pub async fn ask(mut request: Value) -> Result<Answer, Trouble> {
    request["v"] = json!(1);
    let wait = if request["op"].as_str().is_some_and(quick) { QUICK } else { PATIENCE };
    let answered = gio::spawn_blocking(move || {
        let answer = exchange(&request, wait);
        wipe(&mut request);
        answer
    })
    .await;
    let answer = Answer(match answered {
        Ok(Some(answer)) => answer,
        _ => return Err(Trouble::Failed),
    });
    if answer["ok"] == json!(true) {
        return Ok(answer);
    }
    Err(match answer["why"].as_str().unwrap_or("failed") {
        "closed" => Trouble::Closed,
        "not-set-up" => Trouble::NotSetUp,
        "too-large" => Trouble::TooLarge,
        "refused" => Trouble::Refused { wait: answer["wait"].as_u64().unwrap_or(0), problem: answer["problem"].as_str().map(str::to_string) },
        _ if answer["stale"] == json!(true) => Trouble::Stale,
        _ => Trouble::Failed,
    })
}

/// A secret in an answer, in base64 on the wire, as text
pub fn secret_of(answer: &Answer, field: &str) -> Option<Zeroizing<String>> {
    let bytes = Zeroizing::new(base64::engine::general_purpose::STANDARD.decode(answer[field].as_str()?).ok()?);
    std::str::from_utf8(&bytes).ok().map(|text| Zeroizing::new(text.to_string()))
}

/// What goes in the request for a secret, as a value to move into it: `json!` copies what it is given, and the copy would never be wiped
pub fn encode(secret: &[u8]) -> Value {
    Value::String(protocol::encode(secret))
}

/// A plain value in base64, which is no secret
pub fn base64_of(bytes: &[u8]) -> String {
    protocol::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_string_in_an_answer_is_overwritten() {
        let mut value = json!({"secret": "hunter2", "list": ["one", {"two": "three"}], "n": 3});
        wipe(&mut value);
        // zeroize clears the text as well as overwriting it
        assert_eq!(value, json!({"secret": "", "list": ["", {"two": ""}], "n": 3}));
    }

    #[test]
    fn only_what_never_asks_is_quick() {
        assert!(!is_use(&json!({"op": "list"})));
        assert!(!is_use(&json!({"op": "status"})));
        assert!(is_use(&json!({"op": "get"})));
        assert!(is_use(&json!({"op": "auth-code"})));
        assert!(is_use(&json!({"op": "unlock"})));
        assert!(quick("list") && !quick("put"));
    }

    #[test]
    fn a_request_is_measured_as_it_is_written() {
        let value = json!({"op": "put", "name": "é"});
        assert_eq!(written_size(&value), Some(serde_json::to_string(&value).unwrap().len()));
    }

    #[test]
    fn a_refusal_has_words() {
        assert!(!Trouble::Refused { wait: 0, problem: Some("outside-limits".into()) }.words().is_empty());
    }
}
