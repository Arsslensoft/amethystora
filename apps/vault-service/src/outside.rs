//! What the service needs from the rest of the machine: polkit to ask the person, and logind to say when to close. A trait,
//! so that tests run without either. `desktop.rs` is the real one.

use std::os::fd::RawFd;

use crate::peer::Peer;

/// Which polkit action asks: `org.amethystora.vault.policy`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    Use,
    Export,
    /// An administrator of the machine: the device identity certificate, and a new root
    Admin,
}

impl Action {
    pub fn id(self) -> &'static str {
        match self {
            Action::Open => "org.amethystora.vault.open",
            Action::Use => "org.amethystora.vault.use",
            Action::Export => "org.amethystora.vault.export",
            Action::Admin => "org.amethystora.vault.admin",
        }
    }
}

/// What the person is asked. `message` is a fixed sentence with `$(program)` and `$(item)` in it, which polkit fills in
/// and translates through the text domain, so a name somebody else chose can never change its words.
#[derive(Clone, Debug)]
pub struct Ask {
    pub action: Action,
    pub message: &'static str,
    pub item: String,
    /// One more thing a sentence may say, as `$(detail)`: where a forwarded connection comes from, or what a message is for
    pub detail: String,
}

/// The sentences of an approval, each listed for the translators in vault.just
pub mod say {
    pub const SET_UP: &str = "Set up Amethystora Vault. A program calling itself $(program) asks.";
    pub const OPEN: &str = "Open Amethystora Vault. A program calling itself $(program) asks.";
    pub const NEW_VAULT: &str = "Make a new vault. A program calling itself $(program) asks.";
    pub const CHANGE_METHODS: &str = "Change how Amethystora Vault opens. A program calling itself $(program) asks.";
    pub const USE_ITEM: &str = "A program calling itself $(program) asks to use “$(item)” from Amethystora Vault.";
    pub const CHANGE_ITEM: &str = "A program calling itself $(program) asks to change “$(item)” in Amethystora Vault.";
    pub const REMOVE_ITEM: &str = "A program calling itself $(program) asks to remove “$(item)” from Amethystora Vault.";
    pub const LOOSEN_ITEM: &str = "A program calling itself $(program) asks to change how often “$(item)” asks you in Amethystora Vault.";
    pub const REMOVE_VAULT: &str = "A program calling itself $(program) asks to remove the vault “$(item)” and everything in it.";
    pub const FORGET_ALL: &str = "A program calling itself $(program) asks to remove every vault of this account and everything in them.";
    pub const IDLE_LONGER: &str = "A program calling itself $(program) asks to keep Amethystora Vault open for longer before it closes by itself.";
    pub const EXPORT_ITEM: &str = "A program calling itself $(program) asks to take “$(item)” out of Amethystora Vault.";
    pub const SHOW_QR: &str = "A program calling itself $(program) asks to show the key of “$(item)”. Anyone who sees it can make your codes.";
    pub const SHOW_CODES: &str = "A program calling itself $(program) asks to show every backup code of “$(item)”.";
    pub const EXPORT_ALL: &str = "A program calling itself $(program) asks to take every authenticator account and backup code out of Amethystora Vault.";
    pub const REPLACE_ITEMS: &str = "A program calling itself $(program) asks to replace your authenticator accounts in Amethystora Vault with an import.";
    pub const ISSUE_CERT: &str = "A program calling itself $(program) asks Amethystora Vault to issue the certificate “$(item)”.";
    pub const DEVICE_IDENTITY: &str = "A program calling itself $(program) asks Amethystora Vault to issue the identity certificate of this machine, “$(item)”.";
    pub const ROTATE_ROOT: &str = "A program calling itself $(program) asks to replace the root certificate of this machine. Everyone who trusted the old one has to stop by hand.";
    pub const SIGN: &str = "A program calling itself $(program) asks to sign with “$(item)” from Amethystora Vault.";
    pub const SIGN_MESSAGE: &str = "A program calling itself $(program) asks to sign a message for “$(detail)”, not a login, with “$(item)” from Amethystora Vault.";
    pub const SIGN_OTHER: &str = "A program calling itself $(program) asks to sign something that is not a login with “$(item)” from Amethystora Vault.";
    pub const SIGN_FORWARDED: &str = "A program calling itself $(program), on a connection forwarded from another computer (host key $(detail)), asks to sign with “$(item)” from Amethystora Vault.";
    pub const SYNC_CONNECT: &str = "A program calling itself $(program) asks to connect Amethystora Vault to the server and account “$(item)”. Whatever in the vault syncs is sent to that account, and whoever opens the vault can reach all of it.";
    pub const SYNC_CONNECT_CA: &str = "A program calling itself $(program) asks to connect Amethystora Vault to “$(item)”, trusting the certificate with that fingerprint for this connection. Whatever in the vault syncs is sent to that account, and whoever opens the vault can reach all of it.";
    pub const SYNC_OFF: &str = "A program calling itself $(program) asks to stop syncing Amethystora Vault with its server.";
    pub const SYNC_ITEM: &str = "A program calling itself $(program) asks to send “$(item)” to the server Amethystora Vault syncs with.";
    pub const SYNC_ITEM_OFF: &str = "A program calling itself $(program) asks to stop syncing “$(item)”, which moves its copy on the server to the trash of the server.";
    pub const SYNC_ACCEPT: &str = "A program calling itself $(program) asks to let Amethystora Vault take, on its next sync, the items that went from the server or came from it.";
}

/// Who an account is, as the machine says, for a certificate's name: never what a client says
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    pub login: String,
    /// The first part of its full name, if it has one
    pub full_name: Option<String>,
}

/// The account's own line in the password database, or its domain's: domain accounts have uids past 65534, and count
pub fn person_of(uid: u32) -> Option<Person> {
    let mut buffer = vec![0u8; 16 * 1024];
    // SAFETY: a zeroed passwd for getpwuid_r to fill, with strings in `buffer`, which outlives their use below
    unsafe {
        let mut entry = std::mem::zeroed::<libc::passwd>();
        let mut found = std::ptr::null_mut();
        if libc::getpwuid_r(uid, &raw mut entry, buffer.as_mut_ptr().cast(), buffer.len(), &raw mut found) != 0 || found.is_null() {
            return None;
        }
        let text = |pointer: *const libc::c_char| (!pointer.is_null()).then(|| std::ffi::CStr::from_ptr(pointer).to_string_lossy().into_owned());
        let login = text(entry.pw_name)?;
        let full_name = text(entry.pw_gecos).and_then(|gecos| gecos.split(',').next().map(|name| name.trim().to_string())).filter(|name| !name.is_empty());
        Some(Person { login, full_name })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The person's answer to the approval with this token. Anything but a clear yes is a no.
    Approved(u64, bool),
    /// That approval was never put to anyone: the account's session is locked, and GNOME Shell would show the prompt, and the
    /// item's name in it, over the lock screen for anyone at the machine (the spike, 2026-11-02)
    NotAsked(u64),
    /// That account's session locked
    Locked(u32),
    /// The machine is about to sleep
    Sleeping,
    /// A session ended: these are the accounts that still have one
    SessionsLeft(Vec<u32>),
    /// logind or the bus went away: nothing is watching any more, so nothing stays open
    Lost,
}

/// Nothing can say when to close a vault, so none opens
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CannotWatch;

pub trait Outside {
    /// A descriptor to wait on, and what for, if there is one now
    fn wait_on(&self) -> Option<(RawFd, i16)>;
    /// It is ready, or `has_events` says so: what happened
    fn ready(&mut self) -> Vec<Event>;
    /// Something is already there to say, with nothing to wait on: no bus, so a no
    fn has_events(&self) -> bool {
        false
    }
    /// Vaults are about to be open: say when to close them. An error means nothing would, so none opens.
    fn watch(&mut self) -> Result<(), CannotWatch>;
    /// No vault is open any more
    fn unwatch(&mut self);
    /// Asks the person, answering with `Event::Approved(token, ...)` later, or `NotAsked` if the session is locked. Never blocks.
    fn approve(&mut self, token: u64, ask: &Ask, peer: &Peer);
    /// Asks only whether the account's sessions are unlocked: `Approved(token, true)` if so, `NotAsked` if not
    fn when_unlocked(&mut self, token: u64, peer: &Peer);
    /// Whoever asked has gone
    fn cancel(&mut self, token: u64);
    /// Whether this account has a login session at the machine. One that has none has nobody to approve anything, so it is
    /// served nothing but the `background` key (see `lingering`).
    fn has_session(&self, uid: u32) -> bool;
    /// Whether this account has no session but its programs go on, because its owner let them (lingering): the state a timer
    /// of the account's own, a night's backup, runs in. The only account with no session that is served at all.
    fn lingering(&self, _uid: u32) -> bool {
        false
    }
    /// Who this account is. A trait method so that a test can be a guest.
    fn person(&self, uid: u32) -> Option<Person> {
        person_of(uid)
    }
}
