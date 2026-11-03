//! Amethystora Vault's core, with no GTK and no socket (plans/PLAN-vault.md). The service, the window and the
//! command line all use it.

pub mod seal;
pub mod slots;
pub mod store;
pub mod vaults;
pub mod moving;
pub mod protocol;
pub mod items;
pub mod generate;
pub mod otp;
pub mod otpauth;
pub mod bmc;
pub mod r#import;
pub mod codes;
pub mod keys;
pub mod pki;
pub mod sync;
