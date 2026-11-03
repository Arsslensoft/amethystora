//! otpauth:// links, read and written, every Steam spelling included (plans/vault-bmc/spec-formats.md, section 5, and
//! the Coordinator's decisions at its end). A link often holds a working secret, so a refusal says nothing about it.

use zeroize::Zeroizing;

use crate::items::{Account, MAX_NAME, MAX_SECRET_CHARS, OtpKind, Secret, js_trim};
use crate::otp::{self, Algorithm};

/// Longer than any QR code holds, and than any real link
pub const MAX_LINK: usize = 8 * 1024;
/// The structured readers' limit too: what a JavaScript number holds exactly
pub const MAX_COUNTER: i64 = (1 << 53) - 1;

fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn strip_prefix_ascii_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &text[prefix.len()..])
}

/// `%XX` as UTF-8: a truncated or bad escape, or bytes that are not UTF-8, refuse. `+` stays `+`.
fn percent_decode(text: &str) -> Option<Zeroizing<String>> {
    let bytes = text.as_bytes();
    let mut out = Zeroizing::new(Vec::with_capacity(bytes.len()));
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let high = (*bytes.get(at + 1)? as char).to_digit(16)?;
            let low = (*bytes.get(at + 2)? as char).to_digit(16)?;
            out.push((high * 16 + low) as u8);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    std::str::from_utf8(&out).ok()?;
    // Checked above, so this moves the bytes and copies nothing
    String::from_utf8(std::mem::take(&mut *out)).ok().map(Zeroizing::new)
}

/// The path of a link: the first `:` or `%3A` with something after it (literal spaces apart) splits a prefix from a
/// name; a colon with nothing after it does not count
fn split_path(path: &str) -> Option<(Option<String>, String)> {
    let mut at = 0;
    while at < path.len() {
        let rest = &path[at..];
        let sep = if rest.starts_with(':') {
            1
        } else if rest.get(..3).is_some_and(|head| head.eq_ignore_ascii_case("%3A")) {
            3
        } else {
            0
        };
        if sep > 0 && at + sep < path.len() {
            let after = &path[at + sep..];
            let name = after.trim_start_matches(' ');
            // All spaces: the last of them is the "at least one character"
            let name = if name.is_empty() { &after[after.len() - 1..] } else { name };
            return Some((Some(taken(percent_decode(&path[..at])?)), taken(percent_decode(name)?)));
        }
        at += rest.chars().next().map_or(1, char::len_utf8);
    }
    Some((None, taken(percent_decode(path)?)))
}

/// A name, out of its wiping buffer: names are shown, so they are kept as plain text
fn taken(mut text: Zeroizing<String>) -> String {
    std::mem::take(&mut *text)
}

struct Parameters {
    secret: Option<Zeroizing<String>>,
    issuer: Option<Zeroizing<String>>,
    algorithm: Option<Zeroizing<String>>,
    digits: Option<Zeroizing<String>>,
    period: Option<Zeroizing<String>>,
    counter: Option<Zeroizing<String>>,
}

fn read_parameters(query: &str) -> Option<Parameters> {
    let mut p = Parameters { secret: None, issuer: None, algorithm: None, digits: None, period: None, counter: None };
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=')?;
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'~' | b'_' | b'-')) {
            return None;
        }
        let value = value.split(is_line_break).next().unwrap_or("");
        let value = percent_decode(value)?;
        let slot = match name.to_ascii_lowercase().as_str() {
            "secret" => &mut p.secret,
            "issuer" => &mut p.issuer,
            "algorithm" => &mut p.algorithm,
            "digits" => &mut p.digits,
            "period" => &mut p.period,
            "counter" => &mut p.counter,
            _ => continue,
        };
        *slot = Some(value);
    }
    Some(p)
}

/// `^\+?[1-9]\d*$`, which must fit a signed 64-bit integer
fn positive_number(text: &str) -> Option<i64> {
    let digits = text.strip_prefix('+').unwrap_or(text);
    if !digits.bytes().next().is_some_and(|b| (b'1'..=b'9').contains(&b)) || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// `^[+-]?\d+$`, refused below 0 and past what the structured readers take (section 16, point 19)
fn counter_number(text: &str) -> Option<i64> {
    let (negative, digits) = match text.as_bytes().first()? {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value: i64 = digits.parse().ok()?;
    (value <= MAX_COUNTER && (!negative || value == 0)).then_some(value)
}

/// `^SHA(1|224|256|384|512|3-224|3-256|3-384|3-512)$` in any letter case: the known names a link may carry. SHA-3 is
/// known and makes no code, so it is refused later than a name that is not known.
fn link_algorithm(text: &str) -> Option<Option<Algorithm>> {
    const SHA3: [&str; 4] = ["SHA3-224", "SHA3-256", "SHA3-384", "SHA3-512"];
    if SHA3.iter().any(|n| n.eq_ignore_ascii_case(text)) {
        return Some(None);
    }
    const NAMES: [Algorithm; 5] = [Algorithm::Sha1, Algorithm::Sha224, Algorithm::Sha256, Algorithm::Sha384, Algorithm::Sha512];
    NAMES.iter().find(|a| a.name().eq_ignore_ascii_case(text)).map(|a| Some(*a))
}

/// `encoder`, as a URL's query reader would give it: from the first `?` up to any `#`, form decoded, the name compared
/// exactly and the first match taken
fn encoder_is_steam(link: &str) -> bool {
    let mut url = Zeroizing::new(String::with_capacity(link.len()));
    url.extend(link.chars().filter(|c| !matches!(c, '\t' | '\n' | '\r')));
    let url = url.trim_matches(|c: char| c <= ' ');
    let Some(start) = url.find('?') else { return false };
    if url.find('#').is_some_and(|hash| hash < start) {
        return false;
    }
    let query = &url[start + 1..];
    let query = query.split('#').next().unwrap_or("");
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if form_decode(name).as_slice() == b"encoder" {
            return form_decode(value).eq_ignore_ascii_case(b"steam");
        }
    }
    false
}

/// `+` is a space and a `%` that is not an escape stays as it is
pub(crate) fn form_decode(text: &str) -> Zeroizing<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut out = Zeroizing::new(Vec::with_capacity(bytes.len()));
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'+' => out.push(b' '),
            b'%' => {
                let hex = bytes.get(at + 1).zip(bytes.get(at + 2)).and_then(|(h, l)| {
                    Some((*h as char).to_digit(16)? * 16 + (*l as char).to_digit(16)?)
                });
                match hex {
                    Some(byte) => {
                        out.push(byte as u8);
                        at += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        at += 1;
    }
    out
}

/// Read a link. `None` when refused, and a refusal never repeats anything of the link: it may hold a working secret.
pub fn read_link(input: &str) -> Option<Account> {
    if input.len() > MAX_LINK {
        return None;
    }
    let link = js_trim(input);

    // A. Bare Steam
    if let Some(rest) = strip_prefix_ascii_case(link, "steam://") {
        let body = rest.trim_end_matches('=');
        if body.is_empty() || !body.bytes().all(|b| b.is_ascii_alphabetic() || (b'2'..=b'7').contains(&b)) {
            return None;
        }
        // Its length first, so that a secret too long to keep is never copied; and the copy wipes
        if body.len() > MAX_SECRET_CHARS {
            return None;
        }
        let mut secret = Zeroizing::new(String::with_capacity(body.len()));
        secret.push_str(body);
        secret.make_ascii_uppercase();
        if otp::holds_no_bytes(&secret) {
            return None;
        }
        return Some(Account {
            kind: Some(OtpKind::Steam),
            issuer: "Steam".to_string(),
            secret: Secret::from(secret),
            ..Account::default()
        });
    }

    // B and C. The scheme, then the last `?`, which is the only one a value's `[^?&]*` leaves
    let rest = strip_prefix_ascii_case(link, "otpauth://")?;
    let (kind_name, rest) = rest.split_at_checked(rest.find('/')?)?;
    let rest = &rest[1..];
    let (hotp, steam_scheme) = match kind_name.to_ascii_lowercase().as_str() {
        "hotp" => (true, false),
        "totp" => (false, false),
        "steam" => (false, true),
        _ => return None,
    };
    let (path, query) = rest.rsplit_once('?')?;
    if path.is_empty() || path.contains(is_line_break) {
        return None;
    }

    // D and E
    let (prefix, name) = split_path(path)?;
    let p = read_parameters(query)?;

    // F
    let counter = match (hotp, p.counter.as_deref()) {
        (true, Some(text)) => Some(counter_number(text)?),
        (true, None) => return None,
        _ => None,
    };
    let period = match (hotp, p.period.as_deref()) {
        (true, _) | (false, None) => 30,
        (false, Some(text)) => positive_number(text)?,
    };
    let issuer = match p.issuer {
        Some(issuer) if !issuer.is_empty() => taken(issuer),
        _ => prefix.unwrap_or_default(),
    };
    let secret_text = p.secret.as_ref()?;
    let body = secret_text.trim_end_matches('=');
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_alphabetic() || (b'2'..=b'7').contains(&b)) || body.len() > MAX_SECRET_CHARS {
        return None;
    }
    let secret = otp::canonical_secret(body)?;
    if secret.is_empty() {
        return None;
    }
    let algorithm = match p.algorithm.as_deref() {
        Some(text) => link_algorithm(text)?,
        None => Some(Algorithm::Sha1),
    };
    let digits = match p.digits.as_deref() {
        Some(text) => positive_number(text)?,
        None => 6,
    };

    // G. A SHA-3 name is refused here, also for Steam
    let algorithm = algorithm?;
    // The writer gives an account with no label `ISSUER:`, so reading makes that empty again (decision 5)
    let label = if !issuer.is_empty() && name.strip_suffix(':') == Some(issuer.as_str()) { String::new() } else { name };
    if issuer.len() > MAX_NAME || label.len() > MAX_NAME {
        return None;
    }
    let mut account = Account { issuer, label, secret, ..Account::default() };
    if hotp {
        if digits != 6 && digits != 8 {
            return None;
        }
        account.kind = Some(OtpKind::Hotp);
        account.algorithm = algorithm.name().to_string();
        account.digits = digits;
        account.counter = counter;
        return Some(account);
    }
    let names = if account.issuer.is_empty() { &account.label } else { &account.issuer };
    let steam = steam_scheme || encoder_is_steam(link) || (digits == 5 && names.eq_ignore_ascii_case("steam"));
    account.period = period;
    if steam {
        account.kind = Some(OtpKind::Steam);
        return Some(account);
    }
    if digits != 6 && digits != 8 {
        return None;
    }
    account.kind = Some(OtpKind::Totp);
    account.algorithm = algorithm.name().to_string();
    account.digits = digits;
    Some(account)
}

/// JavaScript's `encodeURIComponent`
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for &b in text.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(char::from_digit((b >> 4) as u32, 16).unwrap_or('0').to_ascii_uppercase());
            out.push(char::from_digit((b & 15) as u32, 16).unwrap_or('0').to_ascii_uppercase());
        }
    }
    out
}

/// The canonical name a link is written with: section 3's names, SHA-3's among them
fn written_algorithm(name: &str) -> Option<String> {
    if let Some(known) = Algorithm::from_stored(name) {
        return Some(known.name().to_string());
    }
    const SHA3: [&str; 4] = ["SHA3-224", "SHA3-256", "SHA3-384", "SHA3-512"];
    SHA3.iter().find(|n| n.eq_ignore_ascii_case(name)).map(|n| n.to_string())
}

/// The link, byte for byte what BackMyCodes writes. `None` for an undecodable secret or an unknown algorithm. A Steam
/// account is written `otpauth://steam/`, so that an app without Steam refuses it rather than showing wrong codes.
pub fn write_link(account: &Account) -> Option<Secret> {
    let secret = otp::canonical_secret(account.secret.as_str())?;
    let algorithm = written_algorithm(&account.algorithm)?;
    let (scheme, digits, last) = match account.otp_kind() {
        OtpKind::Totp => ("totp", account.digits, format!("period={}", account.period)),
        OtpKind::Hotp => ("hotp", account.digits, format!("counter={}", account.counter.unwrap_or(0))),
        OtpKind::Steam => ("steam", 5, format!("period={}", account.period)),
    };
    let label = encode(&account.label);
    let (name, issuer) = if account.issuer.is_empty() {
        (label, String::new())
    } else {
        let issuer = encode(&account.issuer);
        (format!("{issuer}:{label}"), format!("issuer={issuer}&"))
    };
    // Base32 and an algorithm's name are all characters encodeURIComponent leaves, so they go in as they are, into one
    // buffer of the right size: a link holds the secret, and nothing may grow and leave a copy of it
    let digits = digits.to_string();
    let parts = [
        "otpauth://", scheme, "/", name.as_str(), "?", issuer.as_str(), "secret=", secret.as_str(), "&algorithm=", algorithm.as_str(), "&digits=", digits.as_str(), "&", last.as_str(),
    ];
    let mut link = Zeroizing::new(String::with_capacity(parts.iter().map(|p| p.len()).sum()));
    parts.iter().for_each(|part| link.push_str(part));
    Some(Secret::from(link))
}

/// The link to draw as a QR code: none for an account with no names at all, since nothing can read what that writes
pub fn link_for_qr(account: &Account) -> Option<Secret> {
    if account.issuer.is_empty() && account.label.is_empty() {
        return None;
    }
    write_link(account)
}

/// What the add form was given. The form's limits are stricter than a link's.
pub struct ManualEntry<'a> {
    pub kind: OtpKind,
    pub issuer: &'a str,
    pub label: &'a str,
    pub key: &'a str,
    pub algorithm: Algorithm,
    pub digits: i64,
    pub period: i64,
    pub counter: i64,
}

/// What the form says, one at a time, in this order
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManualError {
    KeyRequired,
    KeyNotBase32,
    NameRequired,
    PeriodOutOfRange,
    CounterOutOfRange,
    DigitsOutOfRange,
}

/// The form's door (section 14). The key is kept as normalized, and Steam takes only the key and the names.
pub fn manual_account(entry: &ManualEntry<'_>) -> Result<Account, ManualError> {
    let key = otp::normalize_typed(entry.key);
    if key.is_empty() {
        return Err(ManualError::KeyRequired);
    }
    if !otp::typed_secret_is_valid(&key) {
        return Err(ManualError::KeyNotBase32);
    }
    let (issuer, label) = (js_trim(entry.issuer), js_trim(entry.label));
    if (issuer.is_empty() && label.is_empty()) || issuer.len() > MAX_NAME || label.len() > MAX_NAME {
        return Err(ManualError::NameRequired);
    }
    let mut account = Account {
        kind: Some(entry.kind),
        issuer: issuer.to_string(),
        label: label.to_string(),
        secret: Secret::from(key),
        ..Account::default()
    };
    if entry.kind == OtpKind::Totp && !(15..=300).contains(&entry.period) {
        return Err(ManualError::PeriodOutOfRange);
    }
    if entry.kind == OtpKind::Hotp && !(0..=MAX_COUNTER).contains(&entry.counter) {
        return Err(ManualError::CounterOutOfRange);
    }
    if entry.kind != OtpKind::Steam {
        if entry.digits != 6 && entry.digits != 8 {
            return Err(ManualError::DigitsOutOfRange);
        }
        account.algorithm = entry.algorithm.name().to_string();
        account.digits = entry.digits;
    }
    match entry.kind {
        OtpKind::Totp => account.period = entry.period,
        OtpKind::Hotp => account.counter = Some(entry.counter),
        OtpKind::Steam => {}
    }
    Ok(account)
}
