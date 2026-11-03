//! The SSH agent: the agent protocol on `/run/amethystora/vault-ssh.sock`, which the same loop serves beside the vault's own
//! socket, with the account from `SO_PEERCRED` as there (plans/PLAN-vault.md, Using a secret). It lists the public keys of what
//! is open and signs. A key never leaves the service, and nothing that adds, removes or locks is ever honoured: the vault holds
//! the keys, and this is only a way to use them.
//!
//! Frames are a four-byte length and then a message, bounded as OpenSSH's own agent bounds them. Anything else, whatever it is,
//! is the protocol's one word for no. The signature itself is a job for `amethystora-vault-work`, like everything that takes libcrypto,
//! which the service never loads.

use std::rc::Rc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};

use amethystora_vault_core::protocol::{encode, Refusal};

use crate::outside::{say, Action};
use crate::peer::{clean, Peer};
use crate::plugs::{keys, work};
use crate::service::{Next, Reuse, Service};

/// OpenSSH's `AGENT_MAX_LEN`: nothing a client has a reason to send is longer
pub const MOST: usize = 256 * 1024;
/// A key on `While open` asks once for this long, rather than until the vault closes: a person who said yes to one `git pull`
/// has not said yes for the whole day
pub const WINDOW: Duration = Duration::from_secs(15 * 60);
/// What `ssh` itself will take of a list (`MAX_AGENT_IDENTITIES`), and a limit on the answer
const MOST_KEYS: usize = 2048;

const FAILURE: u8 = 5;
const SUCCESS: u8 = 6;
const EXTENSION: u8 = 27;
/// OpenSSH's name for telling the agent which server a connection is for, and whether the agent is forwarded to it
const SESSION_BIND: &[u8] = b"session-bind@openssh.com";
/// How many binds a connection may make (OpenSSH's own agent allows 16), and how long any part of one may be
const MOST_BINDS: u8 = 16;
const MOST_BIND_PART: usize = 16 * 1024;
/// A list that would not fit what `ssh-add -l` reads is cut short, with room to spare
const MOST_LIST: usize = MOST / 2;
const COMMENT: usize = 100;

/// What one connection has told the agent about itself
#[derive(Default)]
pub struct Conn {
    /// Set once a bind says the connection is forwarded from another computer, and then for good: nothing a client says takes it back.
    /// The host's key, as its fingerprint, since the protocol never says the host's name.
    forwarded: Option<String>,
    binds: u8,
}
const REQUEST_IDENTITIES: u8 = 11;
const IDENTITIES_ANSWER: u8 = 12;
const SIGN_REQUEST: u8 = 13;
const SIGN_RESPONSE: u8 = 14;
const RSA_SHA2_256: u32 = 2;
const RSA_SHA2_512: u32 = 4;

/// The kind of item the agent uses. Its listed `meta` says which are SSH keys, and holds their public line.
const KIND: &str = "key";

pub enum Frame {
    /// Not all of it has arrived, and none was taken
    Partial,
    /// It says it is longer than anything allowed
    TooLarge,
    Whole(Vec<u8>),
}

fn length_of(input: &[u8]) -> Option<usize> {
    let bytes: [u8; 4] = input.get(..4)?.try_into().ok()?;
    Some(u32::from_be_bytes(bytes) as usize)
}

/// Whether there is something for `take` to say: a whole message, or a length that will never be one
pub fn ready(input: &[u8]) -> bool {
    length_of(input).is_some_and(|length| length > MOST || input.len() >= 4 + length)
}

/// The next message, out of what has arrived
pub fn take(input: &mut Vec<u8>) -> Frame {
    let Some(length) = length_of(input) else { return Frame::Partial };
    if length > MOST {
        return Frame::TooLarge;
    }
    if input.len() < 4 + length {
        return Frame::Partial;
    }
    let frame = input[4..4 + length].to_vec();
    input.drain(..4 + length);
    Frame::Whole(frame)
}

/// An answer the service worked out, as the frame that goes back: a message if it was granted, and anything else, a person's no
/// or a closed vault included, is the same one byte
pub fn reply(answer: &Value) -> Vec<u8> {
    let message = answer["ssh"].as_str().and_then(|message| STANDARD.decode(message).ok()).unwrap_or_else(|| vec![FAILURE]);
    let mut frame = (message.len() as u32).to_be_bytes().to_vec();
    frame.extend_from_slice(&message);
    frame
}

fn message(bytes: &[u8]) -> Next {
    Next::Answer(json!({"ok": true, "ssh": STANDARD.encode(bytes)}))
}

fn string(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
}

/// A walk through a message, every length checked
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn u32(&mut self) -> Option<u32> {
        let (head, tail) = self.0.split_first_chunk::<4>()?;
        self.0 = tail;
        Some(u32::from_be_bytes(*head))
    }

    fn byte(&mut self) -> Option<u8> {
        let (head, tail) = self.0.split_first()?;
        self.0 = tail;
        Some(*head)
    }

    fn string(&mut self) -> Option<&'a [u8]> {
        let length = self.u32()? as usize;
        if length > self.0.len() {
            return None;
        }
        let (head, tail) = self.0.split_at(length);
        self.0 = tail;
        Some(head)
    }
}

/// An SSH key's wire form, from the line its listed `meta` holds
fn blob_of(meta: &Value) -> Option<Vec<u8>> {
    if meta["key_kind"] != json!("ssh") {
        return None;
    }
    STANDARD.decode(meta["openssh"].as_str()?.split(' ').nth(1)?).ok()
}

/// One message from a client. Nothing a client sends can make this do more than answer no.
pub fn request(service: &mut Service, peer: &Rc<Peer>, conn: &mut Conn, frame: &[u8]) -> Next {
    match frame.split_first() {
        Some((&REQUEST_IDENTITIES, [])) => identities(service, peer.uid, conn),
        Some((&EXTENSION, rest)) => bind(conn, rest),
        Some((&SIGN_REQUEST, rest)) => {
            let mut reader = Reader(rest);
            match (reader.string(), reader.string(), reader.u32()) {
                (Some(blob), Some(data), Some(flags)) if reader.0.is_empty() => sign(service, peer.uid, conn.forwarded.clone(), blob.to_vec(), data.to_vec(), flags),
                _ => Next::refuse(Refusal::BadRequest),
            }
        }
        // Adding, removing, locking and every other extension
        _ => Next::refuse(Refusal::Refused),
    }
}

/// With nothing open, an empty list: it is true, and `ssh` then goes on to whatever else it may try
fn identities(service: &Service, uid: u32, conn: &Conn) -> Next {
    let mut listed = Vec::new();
    let mut count = 0u32;
    for entry in service.listed(uid, KIND) {
        if let Some(blob) = blob_of(&entry.meta) {
            // Names are the account's own words, but they reach a program on another computer when the agent is forwarded: there they
            // are left out, and here they are shown as text and cut, so that a list is always one `ssh-add -l` can read
            let comment = if conn.forwarded.is_some() { String::new() } else { clean(&entry.name, COMMENT) };
            if listed.len() + blob.len() + comment.len() + 8 > MOST_LIST {
                break;
            }
            string(&mut listed, &blob);
            string(&mut listed, comment.as_bytes());
            count += 1;
            if count as usize == MOST_KEYS {
                break;
            }
        }
    }
    let mut out = vec![IDENTITIES_ANSWER];
    out.extend_from_slice(&count.to_be_bytes());
    out.extend_from_slice(&listed);
    message(&out)
}

/// `session-bind@openssh.com`: which server this connection is for, and whether the agent is forwarded to it. It only ever makes
/// the agent stricter, so the signature the server's key made over the session is not checked here.
fn bind(conn: &mut Conn, rest: &[u8]) -> Next {
    let mut reader = Reader(rest);
    if reader.string() != Some(SESSION_BIND) {
        return Next::refuse(Refusal::Refused);
    }
    let parts = (reader.string(), reader.string(), reader.string(), reader.0);
    let (Some(host_key), Some(session), Some(signature), [forwarding]) = parts else { return Next::refuse(Refusal::BadRequest) };
    if host_key.is_empty() || host_key.len() > MOST_BIND_PART || session.len() > MOST_BIND_PART || signature.len() > MOST_BIND_PART || *forwarding > 1 || conn.binds >= MOST_BINDS {
        return Next::refuse(Refusal::BadRequest);
    }
    conn.binds += 1;
    if *forwarding == 1 && conn.forwarded.is_none() {
        conn.forwarded = Some(format!("SHA256:{}", STANDARD.encode(Sha256::digest(host_key)).trim_end_matches('=')));
    }
    message(&[SUCCESS])
}

/// What a request to sign is for, from the data itself
enum Purpose {
    /// A login to a server, which says which key it is for
    Login,
    /// `ssh-keygen -Y sign`, git's signed commits: a message, in a namespace
    Message(String),
    Other,
}

fn purpose(data: &[u8], blob: &[u8]) -> Purpose {
    if let Some(rest) = data.strip_prefix(b"SSHSIG") {
        let namespace = Reader(rest).string().map(|text| clean(&String::from_utf8_lossy(text), 40)).unwrap_or_default();
        return Purpose::Message(namespace);
    }
    // The login's own shape (RFC 4252): the session, the request's number, who, the service, how, whether it has a signature, the
    // algorithm and the key, and for a hostbound login the server's key after it
    let mut reader = Reader(data);
    let shaped = reader.string().is_some_and(|session| (16..=64).contains(&session.len()))
        && reader.byte() == Some(50)
        && reader.string().is_some()
        && reader.string() == Some(b"ssh-connection")
        && matches!(reader.string(), Some(b"publickey" | b"publickey-hostbound-v00@openssh.com"))
        && reader.byte() == Some(1)
        && reader.string().is_some()
        && reader.string() == Some(blob)
        // The hostbound form (OpenSSH 8.9 and later) ends with the server's key
        && (reader.0.is_empty() || (reader.string().is_some() && reader.0.is_empty()));
    if shaped { Purpose::Login } else { Purpose::Other }
}

fn sign(service: &mut Service, uid: u32, forwarded: Option<String>, blob: Vec<u8>, data: Vec<u8>, flags: u32) -> Next {
    let found = service.listed(uid, KIND).into_iter().find(|entry| blob_of(&entry.meta).as_deref() == Some(&blob[..]));
    let Some((id, key_type)) = found.map(|entry| (entry.id.clone(), entry.meta["type"].as_str().unwrap_or_default().to_string())) else {
        return Next::refuse(Refusal::Refused);
    };
    // Before anybody is asked: `ssh-rsa` is SHA-1, and a request for it gets no prompt
    let Some((format, digest)) = scheme(&key_type, flags) else { return Next::refuse(Refusal::Refused) };
    let item = id.clone();
    // A login from here may be remembered for the window. Anything else asks every time: a message is signed for whoever asks, and a
    // forwarded connection is another computer's.
    let (message, detail, reuse) = match (forwarded, purpose(&data, &blob)) {
        (Some(host), _) => (say::SIGN_FORWARDED, host, Reuse::Never),
        (None, Purpose::Login) => (say::SIGN, String::new(), Reuse::For(WINDOW)),
        (None, Purpose::Message(namespace)) => (say::SIGN_MESSAGE, namespace, Reuse::Never),
        (None, Purpose::Other) => (say::SIGN_OTHER, String::new(), Reuse::Never),
    };
    service.with_window(uid, KIND, &item, Action::Use, message, &detail, reuse, move |service, vault| {
        let held = match keys::read_key(service, uid, vault, &id) {
            Ok(held) => held,
            Err(next) => return next,
        };
        let program = service.config.worker.clone();
        let request = json!({"job": "sign-message", "type": key_type, "key": *held.key, "digest": digest, "data": encode(&data)});
        Next::work(move || work::run(&program, &request), move |_, _, answer: Value| signed_answer(format, &answer))
    })
}

/// How this type of key signs: the signature's name in SSH, and the digest OpenSSL is told to use
fn scheme(key_type: &str, flags: u32) -> Option<(&'static str, &'static str)> {
    match key_type {
        "ed25519" => Some(("ssh-ed25519", "none")),
        "ecdsa-p256" => Some(("ecdsa-sha2-nistp256", "sha256")),
        "ecdsa-p384" => Some(("ecdsa-sha2-nistp384", "sha384")),
        "ecdsa-p521" => Some(("ecdsa-sha2-nistp521", "sha512")),
        "rsa-3072" | "rsa-4096" | "rsa" if flags & RSA_SHA2_256 != 0 => Some(("rsa-sha2-256", "sha256")),
        "rsa-3072" | "rsa-4096" | "rsa" if flags & RSA_SHA2_512 != 0 => Some(("rsa-sha2-512", "sha512")),
        _ => None,
    }
}

fn signed_answer(format: &str, answer: &Value) -> Next {
    let raw = answer["signature"].as_str().and_then(|signature| STANDARD.decode(signature).ok());
    let Some(blob) = raw.and_then(|raw| if format.starts_with("ecdsa") { from_der(&raw) } else { Some(raw) }) else { return Next::refuse(Refusal::Refused) };
    let mut signature = Vec::new();
    string(&mut signature, format.as_bytes());
    string(&mut signature, &blob);
    let mut out = vec![SIGN_RESPONSE];
    string(&mut out, &signature);
    message(&out)
}

/// A DER tag and length, in the short or two-byte forms a signature of any curve here needs, and what is left after it
fn der_item(bytes: &[u8], tag: u8) -> Option<(&[u8], &[u8])> {
    let (&found, rest) = bytes.split_first()?;
    let (&first, rest) = rest.split_first()?;
    let (length, rest) = match first {
        0..=0x7f => (usize::from(first), rest),
        0x81 => {
            let (&length, rest) = rest.split_first()?;
            (usize::from(length), rest)
        }
        _ => return None,
    };
    if found != tag || length > rest.len() {
        return None;
    }
    Some(rest.split_at(length))
}

/// `ECDSA-Sig-Value` as SSH wants it: the two numbers as mpints, which is how DER already writes an integer
fn from_der(der: &[u8]) -> Option<Vec<u8>> {
    let (inner, rest) = der_item(der, 0x30)?;
    let (r, inner) = der_item(inner, 0x02)?;
    let (s, inner) = der_item(inner, 0x02)?;
    if !rest.is_empty() || !inner.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    string(&mut out, r);
    string(&mut out, s);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_taken_only_when_it_has_all_arrived() {
        let mut input = vec![0, 0, 0, 2, 11];
        assert!(!ready(&input));
        assert!(matches!(take(&mut input), Frame::Partial));
        input.extend_from_slice(&[12, 0, 0, 0, 1]);
        assert!(ready(&input));
        assert!(matches!(take(&mut input), Frame::Whole(frame) if frame == [11, 12]));
        assert_eq!(input, [0, 0, 0, 1]);
    }

    #[test]
    fn a_length_past_the_limit_is_refused_before_anything_waits_for_it() {
        for mut input in [vec![255, 255, 255, 255], vec![0, 4, 0, 1]] {
            assert!(ready(&input));
            assert!(matches!(take(&mut input), Frame::TooLarge));
        }
        let mut most = (MOST as u32).to_be_bytes().to_vec();
        assert!(!ready(&most));
        assert!(matches!(take(&mut most), Frame::Partial));
    }

    #[test]
    fn a_no_of_any_kind_is_one_byte() {
        for answer in [json!({"ok": false, "why": "refused"}), json!({"ok": true}), json!({"ssh": "*"}), json!(null)] {
            assert_eq!(reply(&answer), [0, 0, 0, 1, FAILURE]);
        }
    }

    #[test]
    fn rsa_never_signs_with_sha1() {
        for key_type in ["rsa-3072", "rsa-4096", "rsa"] {
            assert_eq!(scheme(key_type, 0), None);
            assert_eq!(scheme(key_type, 1), None);
            assert_eq!(scheme(key_type, RSA_SHA2_256), Some(("rsa-sha2-256", "sha256")));
            assert_eq!(scheme(key_type, RSA_SHA2_512), Some(("rsa-sha2-512", "sha512")));
        }
    }

    #[test]
    fn only_what_ssh_has_a_form_for_signs() {
        for key_type in ["x25519", "aes-256", "ml-dsa-65", "ml-kem-768", ""] {
            assert_eq!(scheme(key_type, RSA_SHA2_256), None);
        }
    }

    #[test]
    fn a_der_signature_becomes_two_mpints_and_anything_else_nothing() {
        let der = [0x30, 8, 2, 2, 0, 0x80, 2, 2, 1, 2];
        assert_eq!(from_der(&der).unwrap(), [0, 0, 0, 2, 0, 0x80, 0, 0, 0, 2, 1, 2]);
        for bad in [&[][..], &[0x30], &[0x30, 200], &[0x30, 2, 2, 9], &[0x31, 0], &[0x30, 0x84, 255, 255, 255, 255], &[0x30, 8, 2, 2, 0, 0x80, 2, 2, 1, 2, 0]] {
            assert_eq!(from_der(bad), None, "{bad:?}");
        }
    }
}
