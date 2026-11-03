//! L6 of the review (plans/vault/work/review-pki.md): a block Rust frees still holding a key's secret. This binary's allocator
//! looks through every block freed, or left behind by a move, while a check is armed. OpenSSL's own allocations are not seen.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use amethystora_vault_core::keys::{generate, KeyType};
use base64::Engine;

struct Watch;

static ARMED: AtomicBool = AtomicBool::new(false);
static NEEDLE: AtomicUsize = AtomicUsize::new(0);
static NEEDLE_LEN: AtomicUsize = AtomicUsize::new(0);
static FOUND: AtomicUsize = AtomicUsize::new(0);
/// One armed check at a time
static ONE: Mutex<()> = Mutex::new(());

fn look(block: *const u8, size: usize) -> bool {
    let length = NEEDLE_LEN.load(Ordering::Relaxed);
    if !ARMED.load(Ordering::Relaxed) || length == 0 || size < length {
        return false;
    }
    let haystack = unsafe { std::slice::from_raw_parts(block, size) };
    let needle = unsafe { std::slice::from_raw_parts(NEEDLE.load(Ordering::Relaxed) as *const u8, length) };
    haystack.windows(length).any(|window| window == needle)
}

unsafe impl GlobalAlloc for Watch {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
        if look(block, layout.size()) {
            FOUND.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.dealloc(block, layout) }
    }

    unsafe fn realloc(&self, block: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let held = look(block, layout.size());
        let moved = unsafe { System.realloc(block, layout, size) };
        // A block that moved went back to the heap as it was
        if held && moved != block {
            FOUND.fetch_add(1, Ordering::Relaxed);
        }
        moved
    }
}

#[global_allocator]
static WATCH: Watch = Watch;

/// How many blocks went back to the heap holding `needle` while `work` ran
fn freed_holding<R>(needle: &[u8], work: impl FnOnce() -> R) -> (usize, R) {
    let _one = ONE.lock().unwrap();
    let kept: &'static [u8] = Box::leak(needle.to_vec().into_boxed_slice());
    NEEDLE.store(kept.as_ptr() as usize, Ordering::Relaxed);
    NEEDLE_LEN.store(kept.len(), Ordering::Relaxed);
    FOUND.store(0, Ordering::Relaxed);
    ARMED.store(true, Ordering::Relaxed);
    let result = work();
    ARMED.store(false, Ordering::Relaxed);
    (FOUND.load(Ordering::Relaxed), result)
}

#[test]
fn an_aes_keys_plain_export_leaves_nothing_behind() {
    let key = generate(KeyType::Aes256).unwrap();
    let encoded = base64::engine::general_purpose::STANDARD.encode(key.as_bytes());
    let (found, exported) = freed_holding(&encoded.as_bytes()[..40], || key.export_plain().unwrap());
    assert_eq!(found, 0, "the export left its base64 in a freed block");
    assert_eq!(&exported[..44], encoded.as_bytes());
    assert_eq!(exported[44], b'\n');
    // The check sees what it should: a plain Vec of the same, dropped, is found
    let (found, ()) = freed_holding(&encoded.as_bytes()[..40], || drop(encoded.as_bytes().to_vec()));
    assert_eq!(found, 1);
}
