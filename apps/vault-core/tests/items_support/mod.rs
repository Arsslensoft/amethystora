//! What the items' tests share: the counting allocator that holds the parsers to their bounds, a small random source
//! that repeats, and the path of the local copy of BackMyCodes' test data, which never enters the repository.
#![allow(dead_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Counting;

static NOW: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static TOTAL: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    // This thread's alone: the test harness's own thread allocates whenever it likes
    static COUNT: Cell<usize> = const { Cell::new(0) };
}
/// A test binary runs its tests at once, and they share these counters, so a measuring test holds this
pub static MEASURING: Mutex<()> = Mutex::new(());

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let now = NOW.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        TOTAL.fetch_add(layout.size(), Ordering::Relaxed);
        PEAK.fetch_max(now, Ordering::Relaxed);
        COUNT.set(COUNT.get() + 1);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        NOW.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size > layout.size() {
            TOTAL.fetch_add(new_size - layout.size(), Ordering::Relaxed);
            let now = NOW.fetch_add(new_size - layout.size(), Ordering::Relaxed) + new_size - layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        } else {
            NOW.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
        }
        COUNT.set(COUNT.get() + 1);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// Every byte allocated so far, freed or not: the difference over a run is what it asked of the allocator, however little
/// of it was held at once
pub fn allocated_total() -> usize {
    TOTAL.load(Ordering::Relaxed)
}

/// What running this took on the heap, in peak bytes above the start and in allocations
pub fn measured<T>(run: impl FnOnce() -> T) -> (T, usize, usize) {
    let (start, count) = (NOW.load(Ordering::Relaxed), COUNT.get());
    PEAK.store(start, Ordering::Relaxed);
    let result = run();
    (result, PEAK.load(Ordering::Relaxed).saturating_sub(start), COUNT.get() - count)
}

/// xorshift64*: repeatable, so that a failure can be run again
pub struct Random(pub u64);

impl Random {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    pub fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.next() as u8).collect()
    }

    /// `text` with a few bytes changed or taken out
    pub fn mutated(&mut self, text: &[u8]) -> Vec<u8> {
        let mut out = text.to_vec();
        for _ in 0..1 + self.below(4) {
            if out.is_empty() {
                break;
            }
            let at = self.below(out.len());
            match self.below(3) {
                0 => out[at] = self.next() as u8,
                1 => {
                    out.remove(at);
                }
                _ => out.insert(at, b"\"{}[]:,\\0-9eE%+="[self.below(16)]),
            }
        }
        out
    }
}

/// Said where it is seen: straight to the terminal, past the test harness, which keeps a passing test's own output
pub fn say(line: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "{line}");
}

/// Whether a program a test checks against is installed. Without it the test says it skipped, and fails under
/// VAULT_VERIFIERS_REQUIRED, so that a run that claims to have checked something has.
pub fn need(program: &str) -> bool {
    let found = std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()));
    if !found {
        assert!(std::env::var_os("VAULT_VERIFIERS_REQUIRED").is_none(), "{program} is not installed");
        say(&format!("SKIPPED: {program} is not installed, so nothing was checked against it"));
    }
    found
}

pub const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../plans/vault-bmc/vectors.json");

/// The local copy of BackMyCodes' test data, when this checkout has one. It is never in the repository, so a build
/// stage has none: a test without it says that it skipped, where it is seen.
pub fn vectors() -> Option<serde_json::Value> {
    let text = match std::fs::read_to_string(VECTORS) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            say("SKIPPED: plans/vault-bmc/vectors.json is not in this checkout, so BackMyCodes' own cases were not run");
            return None;
        }
        // A copy that is there and cannot be read (not UTF-8, no permission) must not pass as if it were not there
        Err(e) => panic!("vectors.json is there and cannot be read: {e}"),
    };
    // Four of its strings hold a lone surrogate, which no Rust string or UTF-8 input can: U+FFFD stands in for them
    let text = text.replace("\\ud800", "\u{fffd}").replace("\\udc00", "\u{fffd}");
    // A copy that is there and unreadable must not pass as if it were not there
    Some(serde_json::from_str(&text).expect("vectors.json is JSON"))
}

/// The JSON at `key` in the copy's own text, its whitespace taken out and nothing else changed: the bytes as written,
/// key order included, which a parsed Value does not keep
pub fn vector_bytes(key: &str) -> Option<String> {
    let text = std::fs::read_to_string(VECTORS).ok()?;
    let start = text.find(&format!("\"{key}\":"))? + key.len() + 3;
    let (mut out, mut depth, mut in_string, mut escaped) = (String::new(), 0, false, false);
    for c in text[start..].chars() {
        if in_string {
            out.push(c);
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            c if c.is_whitespace() => continue,
            '"' => in_string = true,
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            _ => {}
        }
        out.push(c);
        if depth == 0 {
            return Some(out);
        }
    }
    None
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2).map(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap()).collect()
}

/// An account as the test data writes one, for comparing: only the keys it has
pub fn account_json(account: &amethystora_vault_core::items::Account) -> serde_json::Value {
    use serde_json::json;
    let kind = match account.otp_kind() {
        amethystora_vault_core::items::OtpKind::Totp => "totp",
        amethystora_vault_core::items::OtpKind::Hotp => "hotp",
        amethystora_vault_core::items::OtpKind::Steam => "steam",
    };
    let mut out = json!({
        "type": kind, "issuer": account.issuer, "label": account.label, "secret": account.secret.as_str(),
        "algorithm": account.algorithm, "digits": account.digits, "period": account.period,
    });
    if let Some(counter) = account.counter {
        out["counter"] = json!(counter);
    }
    if let Some(note) = &account.note {
        out["note"] = json!(note.as_str());
    }
    if let Some(pinned) = account.pinned {
        out["pinned"] = json!(pinned);
    }
    if let Some(tags) = &account.tags {
        out["tags"] = json!(tags);
    }
    out
}
