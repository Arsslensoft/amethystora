//! Keeping one account on a Vaultwarden or Bitwarden server in step with the first vault (plans/done/PLAN-vault.md, "Sync with
//! Vaultwarden and Bitwarden"). Written from plans/vault-sync/spec-protocol.md alone, under the clean-room rule of
//! plans/vault-sync/IMPLEMENTERS.md: wire names are the protocol's, everything else is ours.
//!
//! Nothing here touches a socket or a file. The service gives a run what it needs (the account, the vault's items, an `Http`
//! that reaches the network service) and writes back what the run says.
//!
//! What a Bitwarden release may change is kept in this file, so that following one is one small change.

pub mod address;
pub mod api;
pub mod attach;
pub mod cipher;
pub mod convert;
pub mod crypto;
#[cfg(any(test, feature = "fake-server"))]
pub mod fake;
pub mod json;
pub mod map;
pub mod merge;
pub mod plain;
#[cfg(feature = "fake-server")]
pub mod register;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod kinds_tests;
#[cfg(test)]
mod kinds_review_tests;

/// `Bitwarden-Client-Version` (spec 3.1). What the client says it understands: at or above 2024.12.0 a server sends SSH key
/// items, which are then counted and left alone; below 2026.2.0 Bitwarden's leaves out its types 6 to 8, which this client
/// cannot read; at or above 2025.11.0 nothing more is gated that this client uses. Tested on Vaultwarden 1.37.5 (2026-11-02).
/// Raise it only once the interop tests pass on the new number, and never above what this client handles.
pub const CLIENT_VERSION: &str = "2025.12.0";
/// `client_id` of a password login and `Bitwarden-Client-Name` (spec 3.2): Bitwarden's server takes only its own list, and
/// there is no way to register one's own. The maintainer may rule on it.
pub const CLIENT_ID: &str = "cli";
/// LinuxDesktop
pub const DEVICE_TYPE: &str = "8";
/// What the person sees in the account's list of devices and in any login email: what this is, honestly
pub const DEVICE_NAME: &str = "Amethystora Vault";
pub const USER_AGENT: &str = concat!("AmethystoraVault/", env!("CARGO_PKG_VERSION"));

/// The KDF settings this client computes at all (spec 2.1)
pub mod bounds {
    pub const PBKDF2_FLOOR: u32 = 100_000;
    /// Below this, and at or above the floor, the person is shown the number and confirms
    pub const PBKDF2_CONFIRM_BELOW: u32 = 600_000;
    pub const PBKDF2_MOST: u32 = 2_000_000;
    pub const ARGON2_PASSES: (u32, u32) = (2, 10);
    pub const ARGON2_MIB: (u32, u32) = (16, 1024);
    /// Above this much memory the person confirms first: it is spent in a child process of its own, and one that a server asked for
    pub const ARGON2_CONFIRM_ABOVE_MIB: u32 = 256;
    pub const ARGON2_LANES: (u32, u32) = (1, 16);
}

/// The largest EncString, in characters, each field may be on Bitwarden's server, which this client keeps to on every server
/// so that an item stays valid if the account moves (spec 7)
pub mod lengths {
    pub const NOTES: usize = 10_000;
    pub const URI: usize = 10_000;
    pub const PASSWORD: usize = 5_000;
    pub const NAME: usize = 1_000;
    /// The characters of a type 2 EncString of `n` plaintext bytes (spec 4.1)
    pub const fn encstring(n: usize) -> usize {
        let ct = 16 * (n / 16 + 1);
        72 + 4 * ct.div_ceil(3)
    }
}

/// What a person may decide an item does in sync, and the kinds that never go at all, whatever anyone says. A kind has one choice,
/// `off`, `on` (Plain: Bitwarden's own encryption alone, which every Bitwarden app opens) or `sealed` (a note only this vault opens),
/// and an item may have a choice of its own, which wins.
pub mod kinds {
    pub const PASSWORD: &str = crate::items::KIND_PASSWORD;
    pub const ACCOUNT: &str = crate::items::KIND_ACCOUNT;
    pub const BACKUP_CODES: &str = crate::items::KIND_BACKUP_CODES;
    pub const KEY: &str = crate::protocol::KIND_KEY;
    pub const CERTIFICATE: &str = crate::protocol::KIND_CERTIFICATE;
    pub const PROTECTED: &str = "protected";

    /// Synced, plain, unless the person says not to
    pub const ON_BY_DEFAULT: &[&str] = &[PASSWORD, ACCOUNT, BACKUP_CODES];
    /// Off until the person chooses
    pub const OFF_BY_DEFAULT: &[&str] = &[KEY, CERTIFICATE, PROTECTED];
    /// Every kind that can go, in the order the window shows them, with the word a person types for each
    pub const ALL: &[(&str, &str)] = &[("passwords", PASSWORD), ("codes", ACCOUNT), ("backup-codes", BACKUP_CODES), ("keys", KEY), ("certificates", CERTIFICATE), ("files", PROTECTED)];

    pub fn may_sync(kind: &str) -> bool {
        ON_BY_DEFAULT.contains(&kind) || OFF_BY_DEFAULT.contains(&kind)
    }

    /// An item no kind covers: it goes by its own word alone. The identity of this machine (its device certificate, whose issue asked an
    /// administrator) is one: the switch for all certificates is not what lets its private key leave the vault.
    pub fn by_item_only(kind: &str, meta: &serde_json::Value) -> bool {
        kind == CERTIFICATE && meta.get("profile").and_then(serde_json::Value::as_str) == Some("device-identity")
    }

    /// What a kind is called where a person reads it, in the plural
    pub fn plural(kind: &str) -> &'static str {
        match kind {
            PASSWORD => "passwords",
            ACCOUNT => "authenticator codes",
            BACKUP_CODES => "backup codes",
            KEY => "keys",
            CERTIFICATE => "certificates",
            _ => "protected files",
        }
    }

    /// The kind a person's word names (`keys`), or the kind itself
    pub fn of_word(word: &str) -> Option<&'static str> {
        ALL.iter().find(|(name, kind)| *name == word || *kind == word).map(|(_, kind)| *kind)
    }

    /// What a kind does until it is chosen
    pub fn default_choice(kind: &str) -> &'static str {
        if ON_BY_DEFAULT.contains(&kind) { "on" } else { "off" }
    }
}
