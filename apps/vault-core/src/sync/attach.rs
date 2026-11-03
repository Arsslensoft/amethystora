//! Attachments (plans/vault-sync/spec-attachments.md): a file as an item of ours carries it, written once and never changed. The
//! file is encrypted in memory before any request, sent through the v2 flow to the connected server's own address (never to a
//! host a response names), read back whole and checked before anything is decrypted. The one delete there is, is of a slot this
//! client opened a moment ago and could not fill.

use serde_json::{json, Value};
use zeroize::Zeroizing;

use super::api::{failure, headers, Api, Expired, NetFailure, Problem, Request};
use super::address::origin_of;
use super::crypto::{self, EncString, SymKey, Unreadable};
use super::json;

/// The most a file of ours may be: far above anything a vault item can hold (`MAX_DATA`), and below every server's own limit (4.5)
pub const MOST_FILE: usize = 64 * 1024 * 1024;
/// What both servers allow a file to differ from the size announced (spec-attachments 1.2)
const ALLOWANCE: usize = 1024 * 1024;
/// A file name's most bytes as plaintext, so that its EncString stays within the 1,000 characters Bitwarden gives `attachments2` (3.3)
const MOST_NAME: usize = 687;
/// The name every sealed blob goes by: it must say nothing of its kind (3.3)
pub const SEALED_NAME: &str = "amethystora-sealed.bin";
/// A token's most characters, whichever shape (2.2)
const MOST_TOKEN: usize = 4096;

/// Why a file did not go or come, each a stable word the window says in a sentence of its own
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fail {
    /// The account has no premium, which Bitwarden's server asks for attachments
    Premium,
    /// Bitwarden's own servers, whose storage host is never contacted
    Cloud,
    /// The server said no: attachments are off, or the account's room for them is spent
    Refused,
    /// The files are kept at another host, which the Vault never contacts
    Elsewhere,
    /// The server has not the file any more, or not what was sent
    Lost,
    /// More than a file of ours may be
    TooBig,
    /// Not what the protocol says a response is
    Bad,
}

impl Fail {
    pub fn word(self) -> &'static str {
        match self {
            Fail::Premium => "attachments-premium",
            Fail::Cloud => "attachments-cloud",
            Fail::Refused => "attachments-refused",
            Fail::Elsewhere => "attachments-elsewhere",
            Fail::Lost => "attachment-lost",
            Fail::TooBig => "file-too-large",
            Fail::Bad => "attachments-bad-response",
        }
    }

    /// The reason a word says, as a connection keeps it: only the ones that hold for the server
    pub fn from_word(word: &str) -> Option<Fail> {
        [Fail::Premium, Fail::Cloud, Fail::Refused, Fail::Elsewhere].into_iter().find(|fail| fail.word() == word)
    }

    /// A reason that holds for every file on this server, and so for as long as nothing changes there: a run does not ask again
    pub fn for_the_server(self) -> bool {
        matches!(self, Fail::Premium | Fail::Cloud | Fail::Refused | Fail::Elsewhere)
    }
}

/// A file ready to send: its key and name wrapped under the item's key, and its bytes in the form the server stores
pub struct Prepared {
    pub key: String,
    pub name: String,
    pub bytes: Zeroizing<Vec<u8>>,
}

impl Prepared {
    /// The announced size: the encrypted bytes, the type byte included (3.4)
    pub fn size(&self) -> usize {
        self.bytes.len()
    }
}

/// A name that fits: at most `MOST_NAME` bytes, cut at a character
pub fn short_name(name: &str) -> String {
    let mut name: String = name.chars().filter(|c| !c.is_control() && *c != '/').collect();
    while name.len() > MOST_NAME {
        name.pop();
    }
    if name.is_empty() { "file".to_string() } else { name }
}

/// Everything encrypted, in memory, before any request: a fresh 64-byte key for this file, wrapped under the cipher's own key
pub fn prepare(item: &SymKey, name: &str, plain: &[u8]) -> Result<Prepared, Fail> {
    if plain.len() > MOST_FILE {
        return Err(Fail::TooBig);
    }
    let key = SymKey::random();
    Ok(Prepared { key: EncString::seal(item, key.bytes()).text(), name: crypto::seal_text(item, &short_name(name)), bytes: Zeroizing::new(EncString::seal(&key, plain).binary()) })
}

/// A file opened: its key out of the record (type 2, the cipher's key, exactly 64 bytes), then the file's shape, its tag, and only then AES
pub fn open(item: &SymKey, record: &Value, file: &[u8]) -> Result<Zeroizing<Vec<u8>>, Unreadable> {
    let wrapped = json::text(record, "key").ok_or(Unreadable)?;
    let key = crypto::open_key(wrapped, item)?;
    EncString::from_binary(file)?.open(&key)
}

/// A boundary no encrypted file could hold by chance, and one no server could have chosen
fn boundary() -> String {
    format!("amethystora-{}", crate::seal::random_hex(16))
}

/// The one part of the upload: `data`, with a filename, and nothing before it (spec-attachments 1.2)
pub fn multipart(boundary: &str, file: &[u8]) -> Zeroizing<Vec<u8>> {
    let mut body = Zeroizing::new(Vec::with_capacity(file.len() + 256));
    body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"data\"; filename=\"attachment\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
    body.extend_from_slice(file);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

/// An address for a download, built here from the connected address and the ids asked for, and only one thing taken from the one a
/// server gave: its token, which has to have the shape of either server's (spec-attachments 2.2). None for anything else.
pub fn download_address(address: &str, api: &str, given: &str, cipher: &str, attachment: &str) -> Option<String> {
    let (scheme, rest) = given.split_once("://")?;
    // https always; plain http only where the connection itself is plain http (a loopback test), never as a downgrade
    let plain = scheme.eq_ignore_ascii_case("http") && address.starts_with("http://");
    if !scheme.eq_ignore_ascii_case("https") && !plain {
        return None;
    }
    let (before, query) = rest.split_once('?')?;
    // No other part of it, and the query is the one parameter
    if before.contains('#') || query.contains('#') || query.contains('&') {
        return None;
    }
    let token = query.strip_prefix("token=")?;
    if token.is_empty() || token.len() > MOST_TOKEN {
        return None;
    }
    let path = before.find('/').map_or("", |at| &before[at..]);
    let ours = format!("/attachments/{}/{attachment}", cipher.to_ascii_lowercase());
    if path.to_ascii_lowercase().ends_with(&ours) && token.split('.').count() == 3 && token.split('.').all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')) {
        return Some(format!("{address}/attachments/{cipher}/{attachment}?token={token}"));
    }
    if path.ends_with("/ciphers/attachment/download") && token.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'%' | b'-')) {
        return Some(format!("{api}/ciphers/attachment/download?token={token}"));
    }
    None
}

/// A slot opened by call 1
pub struct Slot {
    pub id: String,
    /// The bytes go to this same server (`fileUploadType` 0)
    pub direct: bool,
}

impl Api<'_> {
    /// Call 1: a slot for the file. The slot is already on the cipher when this answers, with no file in it.
    pub fn open_slot(&mut self, cipher: &str, file: &Prepared, revision: &str) -> Result<Result<Slot, Fail>, Expired> {
        let body = json!({"key": file.key, "fileName": file.name, "fileSize": file.size(), "adminRequest": false, "lastKnownRevisionDate": revision});
        let response = self.call("POST", &format!("/ciphers/{cipher}/attachment/v2"), Some(&body))?;
        match response.status {
            200 => {}
            401 => return Err(Expired::Token),
            // What a server says no with (Vaultwarden's 400 for attachments off or no room, Bitwarden's for no premium); a 404 or a 413 from
            // a proxy in front is that file's trouble, and not the server's refusal of files
            400 | 402 | 403 => return Ok(Err(Fail::Refused)),
            400..=499 => return Ok(Err(Fail::Bad)),
            _ => return Err(Expired::Problem(failure(&response))),
        }
        let answer = super::api::parse(&response)?;
        let Some(id) = json::text(&answer, "attachmentId").filter(|id| super::cipher::attachment_id_ok(id)) else { return Ok(Err(Fail::Bad)) };
        Ok(Ok(Slot { id: id.to_string(), direct: json::number(&answer, "fileUploadType") == Some(0) }))
    }

    /// Call 2: the bytes, to the address built from the ids. Nothing the response of call 1 said about where is read.
    pub fn upload(&mut self, cipher: &str, slot: &str, file: &[u8]) -> Result<Result<(), Fail>, Expired> {
        let boundary = boundary();
        let mut headers = headers(None);
        headers.push(("Content-Type", format!("multipart/form-data; boundary={boundary}")));
        let body = multipart(&boundary, file);
        let response = self.raw("POST", &format!("{}/ciphers/{cipher}/attachment/{slot}", self.server.api), headers, body, true)?;
        match response.status {
            200 => Ok(Ok(())),
            401 => Err(Expired::Token),
            400 | 402 | 403 => Ok(Err(Fail::Refused)),
            400..=499 => Ok(Err(Fail::Bad)),
            _ => Err(Expired::Problem(failure(&response))),
        }
    }

    /// The one delete there is: a slot this client opened in this very operation and never saw filled (spec-attachments 5.4)
    pub fn drop_slot(&mut self, cipher: &str, slot: &str) -> Result<(), Expired> {
        if !super::api::is_uuid(cipher) || !super::cipher::attachment_id_ok(slot) {
            return Err(Expired::Problem(Problem::BadResponse));
        }
        let response = self.call("DELETE", &format!("/ciphers/{cipher}/attachment/{slot}"), None)?;
        if response.status == 401 { Err(Expired::Token) } else { Ok(()) }
    }

    /// A file of a cipher, downloaded and not yet decrypted: the address asked for, then the bytes from the address built here with no
    /// credential, and never more than the file's size and an allowance, nor more than a file of ours may be
    pub fn download(&mut self, cipher: &str, attachment: &str, size: u64) -> Result<Result<Zeroizing<Vec<u8>>, Fail>, Expired> {
        if !super::api::is_uuid(cipher) || !super::cipher::attachment_id_ok(attachment) {
            return Err(Expired::Problem(Problem::BadResponse));
        }
        let response = self.call("GET", &format!("/ciphers/{cipher}/attachment/{attachment}"), None)?;
        match response.status {
            200 => {}
            401 => return Err(Expired::Token),
            400 | 404 => return Ok(Err(Fail::Lost)),
            _ => return Err(Expired::Problem(failure(&response))),
        }
        let answer = super::api::parse(&response)?;
        if json::text(&answer, "id").is_none_or(|id| id != attachment) {
            return Ok(Err(Fail::Bad));
        }
        let Some(url) = json::text(&answer, "url").and_then(|given| download_address(&self.server.address, &self.server.api, given, cipher, attachment)) else {
            return Ok(Err(Fail::Elsewhere));
        };
        if origin_of(&url).is_none_or(|origin| !self.server.origins().contains(&origin)) {
            return Ok(Err(Fail::Elsewhere));
        }
        let mut head = headers(None);
        head.retain(|(name, _)| *name != "Accept");
        head.push(("Accept", "*/*".to_string()));
        let fetched = self.raw("GET", &url, head, Zeroizing::new(Vec::new()), false)?;
        match fetched.status {
            200 => {}
            // A token that ran out, or none that fits: the file is not there for this client
            300..=499 => return Ok(Err(Fail::Lost)),
            _ => return Err(Expired::Problem(failure(&fetched))),
        }
        let most = usize::try_from(size).unwrap_or(usize::MAX).saturating_add(ALLOWANCE).min(MOST_FILE + 49 + 16);
        if fetched.body.len() > most {
            return Ok(Err(Fail::Bad));
        }
        Ok(Ok(fetched.body))
    }

    /// A request with a body of bytes, to an address built here: the connected server's origin or nothing. The access token goes
    /// only where `auth` says, which no download does.
    pub(super) fn raw(&mut self, method: &'static str, url: &str, mut headers: Vec<(&'static str, String)>, body: Zeroizing<Vec<u8>>, auth: bool) -> Result<super::api::Response, Problem> {
        if auth {
            headers.push(("Authorization", format!("Bearer {}", self.access)));
        }
        if origin_of(url).is_none_or(|origin| !self.server.origins().contains(&origin)) {
            return Err(Problem::Net(NetFailure::NotAllowed));
        }
        self.http.call(Request { method, url: url.to_string(), headers, body }).map_err(Problem::Net)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "3f2504e0-4f89-11d3-9a0c-0305e82c3301";

    fn item_key() -> SymKey {
        SymKey::from_slice(&(0x40u8..0x80).collect::<Vec<_>>()).unwrap()
    }

    /// Vector 8 of spec-attachments 3.5: the file as uploaded, its wrapped key and name, byte for byte
    #[test]
    fn the_vector_of_the_spec() {
        let item = item_key();
        let key = SymKey::from_slice(&(0x80u8..0xc0).collect::<Vec<_>>()).unwrap();
        let hex = |bytes: &[u8]| crate::seal::hex(bytes);
        let file = EncString::seal_with_iv(&key, "attachment test file \u{2713}\n".as_bytes(), <[u8; 16]>::try_from(crate::seal::unhex("3132333435363738393a3b3c3d3e3f40").unwrap()).unwrap());
        assert_eq!(hex(&file.binary()), "023132333435363738393a3b3c3d3e3f4078de68d5fc41d86841d54477c45bf674be486342df917a700abe83b9ed3ea0878d5548c35cb58ad1c732e86dd03292ef271f7cefe30f29f9cf5bb9a111e12862");
        assert_eq!(file.binary().len(), 81);
        let wrapped = EncString::seal_with_iv(&item, key.bytes(), <[u8; 16]>::try_from(crate::seal::unhex("0102030405060708090a0b0c0d0e0f10").unwrap()).unwrap()).text();
        assert_eq!(wrapped, "2.AQIDBAUGBwgJCgsMDQ4PEA==|Xb0894ro0S9uRadVHhT8RptMo/bxBJ01Jc9oIYmURZH3HOYXsD+Lvuift8KunDC/dPfRuxQInuQmjV0pfQ/4tGvna69SdDGZiaIeXVz2sj0=|DaWl3jyqXlewvphOlcDQb7cDf0pg11dHrLnHCLnA4LI=");
        let name = EncString::seal_with_iv(&item, b"key.pem", <[u8; 16]>::try_from(crate::seal::unhex("2122232425262728292a2b2c2d2e2f30").unwrap()).unwrap()).text();
        assert_eq!(name, "2.ISIjJCUmJygpKissLS4vMA==|5g3e6vRQhJ6YwsNcLp19+w==|vqJDzLKiGruC5tX2vJYakeM4EspSp+mLkyJNcMUoMEM=");
        // It opens through the record of an attachment, and an empty file is 65 bytes
        let record = json!({"id": "abc", "key": wrapped, "fileName": name, "size": "81"});
        assert_eq!(open(&item, &record, &file.binary()).unwrap().as_slice(), "attachment test file \u{2713}\n".as_bytes());
        assert_eq!(prepare(&item, "x", b"").unwrap().size(), 65);
    }

    #[test]
    fn what_must_be_refused_is() {
        let item = item_key();
        let key = SymKey::from_slice(&(0x80u8..0xc0).collect::<Vec<_>>()).unwrap();
        let wrapped = EncString::seal(&item, key.bytes()).text();
        let record = json!({"key": wrapped});
        let good = EncString::seal(&key, b"hello there, this is a file").binary();
        assert!(open(&item, &record, &good).is_ok());
        let mut flipped_tag = good.clone();
        flipped_tag[17] ^= 1;
        let mut flipped_last = good.clone();
        *flipped_last.last_mut().unwrap() ^= 1;
        let mut type_zero = good.clone();
        type_zero[0] = 0;
        for bad in [flipped_tag, flipped_last, type_zero, good[..64].to_vec(), good[..good.len() - 1].to_vec()] {
            assert!(open(&item, &record, &bad).is_err());
        }
        // Under the item's key instead of the attachment's: the tag does not match, which is how a swapped key shows
        assert!(EncString::from_binary(&EncString::seal(&item, b"x").binary()).unwrap().open(&key).is_err());
        // A key wrapped under another cipher's item key does not unwrap, and no key at all is no file
        let other = EncString::seal(&SymKey::random(), key.bytes()).text();
        assert!(open(&item, &json!({"key": other}), &good).is_err());
        assert!(open(&item, &json!({"key": null}), &good).is_err());
        // A key of 32 bytes under a good MAC is refused (spec 4.4)
        assert!(open(&item, &json!({"key": EncString::seal(&item, &[7u8; 32]).text()}), &good).is_err());
    }

    #[test]
    fn a_download_address_is_built_here_from_a_token_alone() {
        let (address, api) = ("https://vault.example.test", "https://vault.example.test/api");
        let token = "aaa.bbb_-.ccc";
        let wanted = format!("{address}/attachments/{ID}/abcdef0123?token={token}");
        // Vaultwarden's, whatever host it says (its DOMAIN may be wrong)
        assert_eq!(download_address(address, api, &format!("https://wrong.test:1/attachments/{ID}/abcdef0123?token={token}"), ID, "abcdef0123"), Some(wanted.clone()));
        // Never plain http for a connection that is https (a downgrade), but for one that is plain itself (a test on a loopback address)
        assert_eq!(download_address(address, api, &format!("http://wrong.test:1/attachments/{ID}/abcdef0123?token={token}"), ID, "abcdef0123"), None);
        assert_eq!(
            download_address("http://127.0.0.1:8080", "http://127.0.0.1:8080/api", &format!("http://elsewhere.test/attachments/{ID}/abcdef0123?token={token}"), ID, "abcdef0123"),
            Some(format!("http://127.0.0.1:8080/attachments/{ID}/abcdef0123?token={token}"))
        );
        assert_eq!(download_address(address, api, &format!("https://vault.example.test/attachments/{ID}/abcdef0123?token={token}"), ID, "abcdef0123"), Some(wanted));
        // Bitwarden's self-hosted form
        assert_eq!(download_address(address, api, "https://x.test/api/ciphers/attachment/download?token=AbC%2B.d-e_f~", ID, "a1"), Some(format!("{api}/ciphers/attachment/download?token=AbC%2B.d-e_f~")));
        // Everything else is refused
        for bad in [
            format!("https://bucket.s3.example.test/{ID}/abcdef0123?X-Amz-Signature=1"),
            format!("https://x.test/attachments/{ID}/abcdef0123"),
            format!("https://x.test/attachments/{ID}/abcdef0123?token={token}&x=1"),
            format!("https://x.test/attachments/{ID}/another?token={token}"),
            format!("https://x.test/attachments/{ID}/abcdef0123?token=one-part-only"),
            format!("https://x.test/attachments/{ID}/abcdef0123?token=a.b.c.d"),
            format!("https://x.test/attachments/{ID}/abcdef0123?token=a.b.c d"),
            format!("file:///attachments/{ID}/abcdef0123?token={token}"),
            format!("https://x.test/attachments/00000000-0000-4000-8000-000000000000/abcdef0123?token={token}"),
            "https://x.test/api/ciphers/attachment/download?token=a/b".to_string(),
            "https://blob.core.windows.net/x?sv=1&sig=2".to_string(),
        ] {
            assert_eq!(download_address(address, api, &bad, ID, "abcdef0123"), None, "{bad}");
        }
        assert_eq!(download_address(address, api, &format!("https://x.test/attachments/{ID}/abcdef0123?token={}", "a".repeat(5000)), ID, "abcdef0123"), None);
    }

    #[test]
    fn the_upload_is_one_part_called_data_with_a_filename() {
        let body = multipart("B", b"\x02bytes");
        let text = String::from_utf8_lossy(&body).to_string();
        assert!(text.starts_with("--B\r\nContent-Disposition: form-data; name=\"data\"; filename=\"attachment\"\r\n"));
        assert_eq!(text.matches("--B").count(), 2);
        assert!(text.ends_with("\r\n--B--\r\n"));
    }

    #[test]
    fn names_are_cut_and_files_are_capped() {
        assert_eq!(short_name("a/b\nc.pem"), "abc.pem");
        assert_eq!(short_name(&"é".repeat(500)).len(), 686);
        assert_eq!(short_name(""), "file");
        assert!(matches!(prepare(&item_key(), "x", &vec![0u8; MOST_FILE + 1]), Err(Fail::TooBig)));
    }
}
