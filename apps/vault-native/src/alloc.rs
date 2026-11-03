//! Every block this program gives back is wiped first, as the service's are (`apps/vault-service/src/alloc.rs`): a password on
//! its way to the browser passes through serde_json's strings and base64's buffers, not only through what is `Zeroizing`.

use std::alloc::{GlobalAlloc, Layout, System};

pub struct Wiping;

// SAFETY: System does the allocating. dealloc only writes the block it was given, which its caller gave up, and realloc is the
// trait's default: a new block, a copy, and the old one through dealloc.
unsafe impl GlobalAlloc for Wiping {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: as System's
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: as System's
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
        // SAFETY: the block is layout.size() bytes and still this allocator's
        unsafe {
            libc::explicit_bzero(block.cast(), layout.size());
            System.dealloc(block, layout);
        }
    }
}
