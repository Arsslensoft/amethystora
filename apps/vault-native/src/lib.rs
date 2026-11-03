//! The host behind the Vault's browser extension. `frame` is the browser's framing, `site` which items a page may be offered,
//! `host` a message's way to the service and back, and `install` the manifests that let a browser start this at all.

pub mod frame;
pub mod host;
pub mod install;
pub mod site;
