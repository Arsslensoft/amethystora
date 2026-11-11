//! The service that holds Amethystora Vault's keys, and what its command line shares with it (plans/done/PLAN-vault.md). No GTK
//! and no GLib: it speaks to the system bus itself (`bus.rs`), and its memory budget is 8 MB.

pub mod alloc;
pub mod bus;
pub mod confine;
pub mod desktop;
pub mod outside;
pub mod peer;
pub mod plugs;
pub mod server;
pub mod service;
pub mod sync;
pub mod ssh;
pub mod protect;
pub mod credentials;

#[global_allocator]
static WIPING: alloc::Wiping = alloc::Wiping;
