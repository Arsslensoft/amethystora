//! The plugin system's programs (plans/PLAN-plugins.md): `amethystora-plugin-root`, the one writer of the store, and
//! `amethystora-plugin-host`, which runs plugins in a sandbox and stands between them and the network. This library is
//! what both and the tests share. No GTK, and nothing here talks to the bus: that is the host's own main.rs.

pub mod audit;
pub mod broker;
pub mod check;
pub mod conf;
pub mod files;
pub mod host;
pub mod manifest;
pub mod output;
pub mod proxy;
pub mod rootcmd;
pub mod sandbox;
pub mod secrets;
pub mod shim;
pub mod store;
pub mod tree;
