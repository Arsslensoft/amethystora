//! Backup codes: reading them from text, which accounts have none, the keys a service's name goes by, its stable hue, and
//! 2fa.directory's list (plans/vault-bmc/spec-formats.md, sections 6, 12 and 13, and the Coordinator's decisions).

use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};

use icu_normalizer::DecomposingNormalizerBorrowed;
use icu_properties::CodePointMapData;
use icu_properties::props::GeneralCategory;
use serde_json::Value;
use zeroize::Zeroizing;

use crate::items::{
    Account, BackupCode, BackupService, MAX_CODE, MAX_CODES, MAX_ITEM, MAX_NAME, Secret, USED_AT_ROOM, is_js_space, js_trim, json_size,
};

/// A pasted text or a document's text: longer than this is not a list of codes
pub const MAX_TEXT: usize = 1024 * 1024;
const LOW_AT: usize = 2;

// Reading codes

/// A marker at the start of a piece of text, which goes before anything else (decision 3; BackMyCodes does not do this).
/// A bullet always goes. A hyphen goes only before a space, since `-abc` may be a code. `1)` goes before anything, and
/// `1.` before a space or a letter, since `1.5abc` may be a code; more than three digits is a code, not a number.
fn strip_marker(piece: &str) -> &str {
    let text = piece.trim_start_matches(is_js_space);
    if let Some(rest) = text.strip_prefix(['•', '·', '‣', '◦', '–', '—', '*']) {
        return rest;
    }
    if let Some(rest) = text.strip_prefix('-') {
        return if rest.is_empty() || rest.starts_with(is_js_space) { rest } else { piece };
    }
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    if !(1..=3).contains(&digits) {
        return piece;
    }
    let after = text.get(digits + 1..).unwrap_or("");
    let ends = after.is_empty() || after.starts_with(is_js_space);
    match text.as_bytes().get(digits) {
        Some(b')') if ends || after.starts_with(char::is_alphanumeric) => after,
        Some(b'.') if ends || after.starts_with(char::is_alphabetic) => after,
        _ => piece,
    }
}

/// `[,\t]+` or `\s{2,}` end a piece
fn pieces(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut chars) = (0, line.char_indices().peekable());
    while let Some((at, c)) = chars.next() {
        let mut end = at + c.len_utf8();
        let splits = if c == ',' || c == '\t' {
            true
        } else if is_js_space(c) {
            chars.peek().is_some_and(|(_, next)| is_js_space(*next))
        } else {
            false
        };
        if !splits {
            continue;
        }
        // The whole run of what splits: commas and tabs with spaces after them leave those to the word split
        while let Some(&(next_at, next)) = chars.peek() {
            let same_run = if c == ',' || c == '\t' { next == ',' || next == '\t' } else { is_js_space(next) };
            if !same_run {
                break;
            }
            end = next_at + next.len_utf8();
            chars.next();
        }
        out.push(&line[start..at]);
        start = end;
    }
    out.push(&line[start..]);
    out
}

/// What reading a text of codes made, and what it did not take: every code left out is counted, so that a person is told
/// of it rather than finding a list short
#[derive(Debug, Default)]
pub struct ParsedCodes {
    pub codes: Vec<Secret>,
    /// Codes past [`MAX_CODE`] characters, and codes past [`MAX_CODES`] in the list
    pub dropped: usize,
    /// The text is past [`MAX_TEXT`]: it is not a list of codes, and none was read
    pub text_too_large: bool,
}

/// Codes from pasted text: numbering and bullets go first, however many are nested; a piece of exactly two parts of at
/// most 5 UTF-16 units each is one code (`1234 5678`), and otherwise each part is one; repeats are dropped, keeping the
/// first place (they are no loss, and are not counted).
///
/// Each code is a [`Secret`], and nothing keeps another copy of one: repeats are found by a keyed hash and the place
/// of the first with it, then compared.
pub fn parse_codes(text: &str) -> ParsedCodes {
    let mut parsed = ParsedCodes::default();
    if text.len() > MAX_TEXT {
        parsed.text_too_large = true;
        return parsed;
    }
    let hashing = RandomState::new();
    let mut seen: HashMap<u64, usize> = HashMap::new();
    let mut add = |code: Secret| {
        if code.as_str().len() > MAX_CODE {
            parsed.dropped += 1;
            return;
        }
        let hash = hashing.hash_one(code.as_str());
        let repeat = match seen.get(&hash) {
            None => false,
            Some(&place) if parsed.codes[place].as_str() == code.as_str() => true,
            // The same 64 bits for another code: not a thing to expect, and not to be wrong about
            Some(_) => parsed.codes.iter().any(|held| held.as_str() == code.as_str()),
        };
        if repeat {
            return;
        }
        if parsed.codes.len() >= MAX_CODES {
            parsed.dropped += 1;
            return;
        }
        seen.entry(hash).or_insert(parsed.codes.len());
        parsed.codes.push(code);
    };
    for line in text.split(['\r', '\n']).filter(|l| !l.is_empty()) {
        for piece in pieces(line) {
            let parts: Vec<&str> = without_markers(piece).split(is_js_space).filter(|p| !p.is_empty()).collect();
            if let [a, b] = parts[..]
                && a.encode_utf16().count() <= 5
                && b.encode_utf16().count() <= 5
            {
                let mut code = Zeroizing::new(String::with_capacity(a.len() + 1 + b.len()));
                code.push_str(a);
                code.push(' ');
                code.push_str(b);
                add(Secret::from(code));
            } else {
                parts.into_iter().for_each(|part| add(Secret::new(part)));
            }
        }
    }
    parsed
}

/// Markers one after another (`• 1. abcd`), each gone, as many as there are
fn without_markers(piece: &str) -> &str {
    let mut piece = piece;
    loop {
        let next = strip_marker(piece);
        if next.len() == piece.len() {
            return piece;
        }
        piece = next;
    }
}

/// A PDF's or a picture's text keeps only what looks like a code, since a document also has headings
pub fn parse_document_codes(text: &str) -> ParsedCodes {
    let mut parsed = parse_codes(text);
    parsed.codes.retain(|code| looks_like_code(code.as_str()));
    parsed
}

fn looks_like_code(code: &str) -> bool {
    let bare = || code.chars().filter(|c| !is_js_space(*c) && *c != '-');
    bare().map(char::len_utf16).sum::<usize>() >= 4 && bare().any(|c| c.is_ascii_digit()) && bare().all(char::is_alphanumeric)
}

/// What [`add_codes`] did with each code it was given
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AddedCodes {
    pub added: usize,
    /// Already in the service: no loss
    pub repeats: usize,
    /// Not added, because the service holds as many as one item may, in count or in bytes (with room kept for each
    /// code to be marked used): the person is told
    pub over_limit: usize,
}

/// Codes not already there are added after those that are, exact match, each unused, until the item is full: the rest
/// are counted in `over_limit`, never lost quietly
pub fn add_codes(service: &mut BackupService, codes: Vec<Secret>, now_ms: i64) -> AddedCodes {
    let hashing = RandomState::new();
    let mut held: HashMap<u64, usize> = HashMap::new();
    for (place, code) in service.codes.iter().enumerate() {
        held.entry(hashing.hash_one(code.code.as_str())).or_insert(place);
    }
    let mut size = service.size_with_room();
    let mut done = AddedCodes::default();
    let mut codes = codes.into_iter();
    while let Some(code) = codes.next() {
        let hash = hashing.hash_one(code.as_str());
        let repeat = match held.get(&hash) {
            None => false,
            Some(&place) if service.codes[place].code.as_str() == code.as_str() => true,
            Some(_) => service.codes.iter().any(|c| c.code.as_str() == code.as_str()),
        };
        if repeat {
            done.repeats += 1;
            continue;
        }
        let entry = BackupCode { code, ..BackupCode::default() };
        // The code's JSON, the comma before it and the room its `usedAt` will need
        let cost = json_size(&entry) + 1 + USED_AT_ROOM;
        if service.codes.len() >= MAX_CODES || size + cost > MAX_ITEM {
            done.over_limit += 1 + codes.count();
            break;
        }
        size += cost;
        held.entry(hash).or_insert(service.codes.len());
        service.codes.push(entry);
        done.added += 1;
    }
    if done.added > 0 {
        service.updated_at = Some(now_ms);
    }
    done
}

/// Has codes and 2 or fewer unused: a service with none left is low too, one with no codes at all is not
pub fn is_low(service: &BackupService) -> bool {
    !service.codes.is_empty() && service.codes.iter().filter(|c| !c.used).count() <= LOW_AT
}

/// The first unused code, which "Next code" reveals
pub fn next_unused(service: &BackupService) -> Option<usize> {
    service.codes.iter().position(|c| !c.used)
}

// Service names

/// NFD, every combining mark gone, lower case as JavaScript does it, trimmed
pub fn fold(name: &str) -> String {
    let categories = CodePointMapData::<GeneralCategory>::new();
    let is_mark = |c: &char| {
        matches!(categories.get(*c), GeneralCategory::NonspacingMark | GeneralCategory::SpacingMark | GeneralCategory::EnclosingMark)
    };
    let bare: String = DecomposingNormalizerBorrowed::new_nfd().normalize(name).chars().filter(|c| !is_mark(c)).collect();
    js_trim(&bare.to_lowercase()).to_string()
}

fn squash(text: &str) -> String {
    text.chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).collect()
}

const COMPANY_WORDS: [&str; 25] = [
    "inc", "llc", "ltd", "limited", "corp", "corporation", "gmbh", "ggmbh", "ag", "sa", "sas", "sarl", "plc", "bv", "ab", "oy", "pty",
    "srl", "account", "accounts", "id", "login", "2fa", "mfa", "sso",
];

/// Longer first, since a leftmost match at the end of the name finds these before their own last part
const SUFFIXES: [&str; 36] = [
    ".co.uk", ".org.uk", ".com.au", ".co.jp", ".com.br", ".co.nz", ".com", ".net", ".org", ".io", ".co", ".me", ".app", ".dev", ".ai", ".gg",
    ".tv", ".us", ".eu", ".uk", ".de", ".fr", ".nl", ".be", ".ch", ".at", ".it", ".es", ".se", ".pl", ".ca", ".au", ".jp", ".info", ".biz",
    ".xyz",
];

fn known_suffix(s: &str) -> Option<&'static str> {
    SUFFIXES.iter().copied().find(|suffix| s.ends_with(suffix))
}

/// `^[a-z][a-z0-9+.-]*://([^/?#]*).*$`: a URL counts as its host
fn url_host(s: &str) -> Option<&str> {
    let (scheme, rest) = s.split_once("://")?;
    let first = scheme.chars().next()?;
    let good = first.is_ascii_lowercase()
        && scheme.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '+' | '.' | '-'));
    if !good {
        return None;
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    // `.` stops at a line break, so what comes after the host may hold none
    if rest[end..].contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return None;
    }
    Some(&rest[..end])
}

/// Remove a final `\s*\([^()]*\)`
fn without_final_brackets(s: &str) -> &str {
    if !s.ends_with(')') {
        return s;
    }
    match s[..s.len() - 1].rfind(['(', ')']) {
        Some(open) if s.as_bytes()[open] == b'(' => s[..open].trim_end_matches(is_js_space),
        _ => s,
    }
}

/// Remove from the end, once, words like `Inc.` or `Account` that follow a comma or a space, any number in a row
fn without_company_words(mut s: &str) -> &str {
    loop {
        let body = s.strip_suffix('.').unwrap_or(s);
        let start = body.rfind(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit())).map_or(0, |at| at + body[at..].chars().next().map_or(1, char::len_utf8));
        let word = &body[start..];
        if !COMPANY_WORDS.contains(&word) {
            return s;
        }
        let before = body[..start].trim_end_matches(|c: char| is_js_space(c) || c == ',');
        if before.len() == start {
            return s;
        }
        s = before;
    }
}

fn is_domain(s: &str) -> bool {
    let good = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    s.contains('.') && s.split('.').all(good)
}

/// The keys of a name, most specific first. A name with no ASCII letter or digit has none.
pub fn service_keys(name: &str) -> Vec<String> {
    if name.len() > MAX_NAME {
        return Vec::new();
    }
    let folded = fold(name);
    let mut s = url_host(&folded).unwrap_or(&folded);
    s = without_final_brackets(s);
    s = s.strip_prefix("www.").unwrap_or(s);
    let mut keys = vec![squash(s)];
    s = without_company_words(s);
    keys.push(squash(s));
    if is_domain(s) {
        let suffix = known_suffix(s).unwrap_or_else(|| &s[s.rfind('.').unwrap_or(0)..]);
        let head = &s[..s.len() - suffix.len()];
        let label = head.rsplit('.').next().unwrap_or(head);
        keys.push(squash(&format!("{label}{suffix}")));
        keys.push(squash(label));
    } else {
        let stripped = known_suffix(s).map_or(s, |suffix| &s[..s.len() - suffix.len()]);
        keys.push(squash(stripped));
    }
    keys.retain(|key| !key.is_empty());
    keys
}

/// Whether two names are one service: a shared key, or equal folds that are not empty, so that `Яндекс` is itself
/// though it has no keys (section 16, point 22)
pub fn same_service(a: &str, b: &str) -> bool {
    let (fold_a, fold_b) = (fold(a), fold(b));
    if !fold_a.is_empty() && fold_a == fold_b {
        return true;
    }
    let keys = service_keys(b);
    service_keys(a).iter().any(|key| keys.contains(key))
}

/// Services by what their names go by, built once for a list: the window asks it for each row, so that a long list of
/// accounts costs no pairs of comparisons
pub struct ServiceIndex<'a> {
    services: &'a [BackupService],
    first: HashMap<String, usize>,
}

impl<'a> ServiceIndex<'a> {
    pub fn new(services: &'a [BackupService]) -> ServiceIndex<'a> {
        let mut first = HashMap::new();
        for (at, service) in services.iter().enumerate().filter(|(_, s)| s.deleted_at.is_none()) {
            let folded = fold(&service.service_name);
            if !folded.is_empty() {
                first.entry(format!("={folded}")).or_insert(at);
            }
            for key in service_keys(&service.service_name) {
                first.entry(key).or_insert(at);
            }
        }
        ServiceIndex { services, first }
    }

    /// The first service that is the same as this account's
    pub fn find(&self, account: &Account) -> Option<&'a BackupService> {
        let name = account_service_name(account);
        if !name.chars().any(|c| !is_js_space(c)) {
            return None;
        }
        let folded = format!("={}", fold(name));
        let keys = service_keys(name);
        let at = keys.iter().chain([&folded]).filter_map(|key| self.first.get(key)).min()?;
        self.services.get(*at)
    }
}

/// A stable colour for a name's letter: from its text, trimmed and in lower case, not its accents
pub fn hue(name: &str) -> u32 {
    js_trim(name).to_lowercase().encode_utf16().fold(7u32, |h, unit| h.wrapping_mul(31).wrapping_add(unit as u32)) % 360
}

// Which accounts have none

/// The service an account belongs to: its issuer, else the part of its label before a colon, else its label
pub fn account_service_name(account: &Account) -> &str {
    if account.issuer.chars().any(|c| !is_js_space(c)) {
        return &account.issuer;
    }
    match account.label.find(':') {
        Some(at) if at > 0 => &account.label[..at],
        _ => &account.label,
    }
}

/// The first service holding codes for an account, if any, from an index built once for the list
pub fn service_for<'a>(account: &Account, services: &ServiceIndex<'a>) -> Option<&'a BackupService> {
    services.find(account)
}

/// For the coverage card
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Coverage {
    pub covered: usize,
    /// Accounts counted: those marked as having no codes are left out of the total too
    pub total: usize,
    /// Indices into the accounts given, A to Z by the fold of their service's name
    pub missing: Vec<usize>,
}

pub fn coverage(accounts: &[Account], services: &[BackupService]) -> Coverage {
    let index_of = ServiceIndex::new(services);
    let mut result = Coverage::default();
    let mut missing = Vec::new();
    for (index, account) in accounts.iter().enumerate() {
        if account.deleted_at.is_some() || account.no_backup_codes == Some(true) {
            continue;
        }
        result.total += 1;
        if index_of.find(account).is_some() {
            result.covered += 1;
        } else {
            missing.push((fold(account_service_name(account)), index));
        }
    }
    missing.sort_by(|a, b| a.0.cmp(&b.0));
    result.missing = missing.into_iter().map(|(_, index)| index).collect();
    result
}

// 2fa.directory

/// One service in the bundled list: `[name, domain, docs or 0, recovery or 0, [other domains]?]`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryRow {
    pub name: String,
    pub domain: String,
    /// `Get new codes`: how to set up two-factor, which is where new codes come from
    pub docs: Option<String>,
    /// `Lost access?`
    pub recovery: Option<String>,
    pub other_domains: Vec<String>,
}

/// The bundled copy, read on first need, with the index section 13 lays out
#[derive(Debug, Default)]
pub struct Directory {
    rows: Vec<DirectoryRow>,
    index: HashMap<String, u32>,
}

const MAX_DIRECTORY: usize = 4 * 1024 * 1024;
const MAX_ROWS: usize = 20_000;

fn link(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).filter(|s| s.starts_with("https://") || s.starts_with("http://")).map(str::to_string)
}

fn read_row(row: &Value) -> Option<DirectoryRow> {
    let row = row.as_array()?;
    let other_domains = row.get(4).and_then(Value::as_array).map_or(Vec::new(), |list| {
        list.iter().filter_map(Value::as_str).map(str::to_string).collect()
    });
    Some(DirectoryRow {
        name: row.first()?.as_str()?.to_string(),
        domain: row.get(1)?.as_str()?.to_string(),
        docs: link(row.get(2)),
        recovery: link(row.get(3)),
        other_domains,
    })
}

impl Directory {
    /// From the file `{"entries":[ROW,...]}`. A row of the wrong shape is left out; `None` when the file is not one.
    pub fn parse(json: &[u8]) -> Option<Directory> {
        if json.len() > MAX_DIRECTORY {
            return None;
        }
        let file: Value = serde_json::from_slice(json).ok()?;
        let entries = file.get("entries")?.as_array()?;
        if entries.len() > MAX_ROWS {
            return None;
        }
        let rows: Vec<DirectoryRow> = entries.iter().filter_map(read_row).collect();
        let mut index = HashMap::new();
        for (at, row) in rows.iter().enumerate() {
            let names = [&row.name, &row.domain].into_iter().chain(row.other_domains.iter());
            for key in names.flat_map(|name| service_keys(name)) {
                // A key goes to the first row that has it
                index.entry(key).or_insert(at as u32);
            }
        }
        Some(Directory { rows, index })
    }

    /// For each key of a name in order, the first that is in the index
    pub fn find(&self, service: &str) -> Option<&DirectoryRow> {
        let at = service_keys(service).iter().find_map(|key| self.index.get(key))?;
        self.rows.get(*at as usize)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
