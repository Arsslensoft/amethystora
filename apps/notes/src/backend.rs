//! What main.js did for the page: the notes and tasks, kept on this computer and nowhere else, sealed when
//! they are encrypted. Nothing is synced, and nothing here opens a connection.
//!
//! Everything lives in $XDG_DATA_HOME/amethystora-notes, readable by this account only:
//!
//!   vault.json    whether the notes are encrypted and, when they are, their key, sealed
//!   data.bin      the notes and tasks, one sealed JSON document (data.json when not encrypted)
//!   *.bak         the version before the last save, read when the newest cannot be
//!   resources/    the attachments, one sealed file each
//!   next/         everything written anew while encryption is turned on or off or the key changes
//!
//! vault.rs seals and unseals; this keeps the vault, the key while the notes are open, and the store, and
//! writes them where main.js did, the way it did, so that a vault moves between the Electron window and this
//! one unchanged. A passkey opens the notes as well, never instead (passkeys.rs).

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use serde_json::{json, Map, Value};

use amethystora_ui::{gio, glib, t};

use crate::data::{self, Fields, Kind, Store};
use crate::sync::Gate;
use crate::vault::{self, Key};

/// An attachment nothing refers to is kept this long, for the note that is about to
pub const GRACE: f64 = 10.0 * 60000.0;
pub const ATTACHMENT_LIMIT: u64 = 200 * 1024 * 1024;

/// Where the notes are
#[derive(Debug, Clone)]
pub struct Paths {
    pub dir: PathBuf,
    pub vault: PathBuf,
    pub resources: PathBuf,
    pub plain: PathBuf,
    pub sealed: PathBuf,
    pub next: PathBuf,
}

impl Paths {
    pub fn new(dir: PathBuf) -> Paths {
        Paths {
            vault: dir.join("vault.json"),
            resources: dir.join("resources"),
            plain: dir.join("data.json"),
            sealed: dir.join("data.bin"),
            next: dir.join("next"),
            dir,
        }
    }

    /// $XDG_DATA_HOME/amethystora-notes, or ~/.local/share/amethystora-notes
    pub fn standard() -> Paths {
        let data = std::env::var_os("XDG_DATA_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| amethystora_ui::palette::home().join(".local/share"));
        Paths::new(data.join("amethystora-notes"))
    }

    pub fn resource(&self, id: &str) -> PathBuf {
        self.resources.join(id)
    }

    fn bak(file: &Path) -> PathBuf {
        let mut name = file.as_os_str().to_owned();
        name.push(".bak");
        PathBuf::from(name)
    }
}

/// What the page hears from here, as main.js sent it: something could not be saved, the notes locked
#[derive(Default, Clone)]
pub struct Hooks {
    pub problem: Option<Rc<dyn Fn(String)>>,
    pub locked: Option<Rc<dyn Fn()>>,
    /// The notes are open (at start when not encrypted, or unlocked)
    pub opened: Option<Rc<dyn Fn()>>,
    /// The page hands over what it has not sent yet (requestFlush)
    pub flush_page: Option<Rc<dyn Fn()>>,
}

/// An upcoming reminder, kept while locked too so that it still goes off with what it is about
#[derive(Debug, Clone, PartialEq)]
pub struct Reminder {
    pub id: String,
    pub title: String,
    /// Milliseconds
    pub due: f64,
    pub at: f64,
}

pub struct Backend {
    pub paths: Paths,
    /// vault.json as read, None before the first run
    pub vault: Option<Value>,
    /// The data key while the notes are open and encrypted
    pub key: Option<Key>,
    /// The notes and tasks while they are open
    pub store: Option<Store>,
    pub dirty: bool,
    /// Set while encryption is turned on or off or the key changes, when nothing else is saved
    pub changing: bool,
    pub locking: bool,
    pub reminders: Vec<Reminder>,
    pub last_check: f64,
    pub last_active: f64,
    pub shown: HashSet<String>,
    /// The backup the page is asking the passphrase of
    pub pending_backup: Option<Value>,
    /// Cancels the security key's tool that waits to be touched, while a passkey is used or added
    pub touching: Option<gio::Cancellable>,
    /// Where fido2-token and the others are, for the tests' stand-ins; /usr/bin otherwise
    pub tools: Option<PathBuf>,
    pub hooks: Hooks,
    save_timer: Option<glib::SourceId>,
}

pub type Shared = Rc<RefCell<Backend>>;

/// Every save in order, and the change of key while nothing else is saved
#[derive(Clone, Default)]
pub struct Gates {
    pub saving: Gate,
    pub locking: Gate,
}

thread_local! {
    static GATES: Gates = Gates::default();
}

fn gates() -> Gates {
    GATES.with(Clone::clone)
}

impl Backend {
    pub fn new(paths: Paths) -> Backend {
        let now = data::now_ms();
        Backend {
            paths,
            vault: None,
            key: None,
            store: None,
            dirty: false,
            changing: false,
            locking: false,
            reminders: Vec::new(),
            last_check: now,
            last_active: now,
            shown: HashSet::new(),
            pending_backup: None,
            touching: None,
            tools: None,
            hooks: Hooks::default(),
            save_timer: None,
        }
    }

    pub fn encrypted(&self) -> bool {
        self.vault.as_ref().is_some_and(|vault| data::truthy(vault.get("encrypted")))
    }

    /// new, open or locked
    pub fn status(&self) -> &'static str {
        match (&self.vault, &self.store) {
            (None, _) => "new",
            (_, Some(_)) => "open",
            _ => "locked",
        }
    }

    /// The passkeys as the page lists them, never their keys: (id, name, created, protected)
    pub fn passkeys(&self) -> Vec<Passkey> {
        let Some(list) = self.vault.as_ref().and_then(|vault| vault.get("passkeys")).and_then(Value::as_array) else { return Vec::new() };
        list.iter()
            .map(|passkey| Passkey {
                id: passkey.str("id").to_string(),
                name: data::text(passkey.get("name")),
                created: passkey.num_or_zero("created"),
                protected: passkey.num("protect") == Some(3.0),
            })
            .collect()
    }

    /// The key to seal with, or None for notes that are not encrypted. Encrypted but locked, what was
    /// under way when the notes locked (an attachment, an import) stops here rather than write in the clear.
    pub fn sealing(&self) -> Result<Option<&Key>, String> {
        if self.encrypted() && self.key.is_none() {
            return Err(t!("Unlock your notes first."));
        }
        Ok(self.key.as_ref())
    }

    fn problem(this: &Shared, message: String) {
        let hook = this.borrow().hooks.problem.clone();
        if let Some(hook) = hook {
            hook(message);
        }
    }

    // --- Reading -------------------------------------------------------------------------------------------

    pub fn read_vault(&mut self) -> Result<(), String> {
        match std::fs::read_to_string(&self.paths.vault) {
            Ok(text) => match data::parse_json(&text) {
                Ok(value) => {
                    self.vault = Some(value);
                    Ok(())
                }
                Err(error) => {
                    self.vault = None;
                    Err(t!("{file} could not be read: {error}", file = self.paths.vault.display().to_string(), error = error.to_string()))
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.vault = None;
                Ok(())
            }
            Err(error) => {
                self.vault = None;
                Err(t!("{file} could not be read: {error}", file = self.paths.vault.display().to_string(), error = error.to_string()))
            }
        }
    }

    /// The newest readable copy. An encrypted vault reads only sealed files, so that nobody can slip it
    /// notes in the clear.
    pub fn read_data(&mut self) -> Result<Store, String> {
        let first = if self.key.is_some() { self.paths.sealed.clone() } else { self.paths.plain.clone() };
        let candidates = [first.clone(), Paths::bak(&first)];
        let mut found = false;
        for (at, file) in candidates.iter().enumerate() {
            let Ok(bytes) = std::fs::read(file) else { continue };
            found = true;
            let text = match &self.key {
                Some(key) => match vault::unseal(key, &bytes, "data") {
                    Ok(plain) => plain,
                    // Damaged: try the one before it
                    Err(()) => continue,
                },
                None => bytes,
            };
            if let Ok(value) = data::parse_json(&String::from_utf8_lossy(&text)) {
                if at > 0 {
                    self.dirty = true;
                }
                return Ok(Store::normalise(&value));
            }
        }
        if found {
            return Err(t!("Your notes could not be read. Nothing has been changed in {folder}.", folder = self.paths.dir.display().to_string()));
        }
        Ok(Store::empty())
    }

    /// Fails on an attachment in the clear while the notes are encrypted
    pub fn read_resource(&self, id: &str) -> Result<Vec<u8>, ()> {
        read_resource(&self.paths, self.key.as_ref(), id)
    }

    // --- Saving --------------------------------------------------------------------------------------------

    /// Saves in 400 ms, unless something else changes before then
    pub fn schedule_save(this: &Shared) {
        let mut backend = this.borrow_mut();
        backend.dirty = true;
        if let Some(timer) = backend.save_timer.take() {
            timer.remove();
        }
        let weak = Rc::downgrade(this);
        backend.save_timer = Some(glib::timeout_add_local_once(Duration::from_millis(400), move || {
            if let Some(this) = weak.upgrade() {
                this.borrow_mut().save_timer = None;
                glib::spawn_future_local(async move {
                    flush(&this).await;
                });
            }
        }));
    }

    pub fn save_pending(&self) -> bool {
        self.save_timer.is_some()
    }

    // --- Changes from the page -----------------------------------------------------------------------------

    /// The settings the page may change, each checked; the others are left as they are
    pub fn set_settings(this: &Shared, values: &Map<String, Value>) {
        {
            let mut backend = this.borrow_mut();
            let Some(store) = backend.store.as_mut() else { return };
            for (name, value) in values {
                if data::valid_setting(name, value) {
                    store.settings.insert(name.clone(), value.clone());
                }
            }
        }
        Backend::schedule_save(this);
    }

    /// Items written or taken away by the page: saved, and the reminders and attachments brought up to date
    pub fn changed(this: &Shared, kinds: &[Kind], dropped: bool) {
        if this.borrow().store.is_none() {
            return;
        }
        Backend::schedule_save(this);
        if kinds.contains(&Kind::Tasks) {
            this.borrow_mut().compute_reminders();
        }
        if dropped && (kinds.contains(&Kind::Notes) || kinds.contains(&Kind::Revisions)) {
            collect_resources(this);
        }
    }

    // --- Reminders -----------------------------------------------------------------------------------------

    /// Titles and times only, kept in memory while locked so that a reminder still says what it is about
    pub fn compute_reminders(&mut self) {
        let Some(store) = &self.store else { return };
        let now = data::now_ms();
        self.reminders = store
            .values(Kind::Tasks)
            .filter(|task| {
                !task.is("done")
                    && crate::dates::is_iso(&data::text(task.get("due")))
                    && crate::dates::is_time(&data::text(task.get("time")))
                    && task.num("reminder").is_some_and(|n| n >= 0.0 && n.fract() == 0.0)
            })
            .filter_map(|task| {
                let due = crate::dates::due_ms(task.str("due"), Some(task.str("time")))?;
                let title = task.opt("title").map(str::to_string).unwrap_or_else(|| t!("A task"));
                Some(Reminder { id: task.id().to_string(), title, due, at: due - task.num_or_zero("reminder") * 60000.0 })
            })
            .filter(|reminder| reminder.at > now - 60000.0)
            .collect();
    }

    pub fn upcoming(&self) -> bool {
        let now = data::now_ms();
        self.reminders.iter().any(|reminder| reminder.at > now)
    }

    /// The reminders that are due since the last look, each once
    pub fn due_reminders(&mut self) -> Vec<Reminder> {
        let now = data::now_ms();
        let mut due = Vec::new();
        for reminder in &self.reminders {
            let tag = format!("{}:{}", reminder.id, data::js_number(reminder.at));
            if reminder.at > self.last_check && reminder.at <= now && !self.shown.contains(&tag) {
                self.shown.insert(tag);
                due.push(reminder.clone());
            }
        }
        self.last_check = now;
        due
    }
}

/// A passkey as the page lists it
#[derive(Debug, Clone, PartialEq)]
pub struct Passkey {
    pub id: String,
    pub name: String,
    pub created: f64,
    /// Made to need the PIN or a fingerprint every time (protect: 3)
    pub protected: bool,
}

pub fn read_resource(paths: &Paths, key: Option<&Key>, id: &str) -> Result<Vec<u8>, ()> {
    let bytes = std::fs::read(paths.resource(id)).map_err(|_| ())?;
    match key {
        Some(key) => vault::unseal(key, &bytes, &format!("resource:{id}")),
        None => Ok(bytes),
    }
}

/// An attachment written in place, sealed under the key when there is one
pub fn write_resource(paths: &Paths, key: Option<&Key>, id: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(&paths.resources)?;
    set_mode(&paths.resources, 0o700);
    let sealed;
    let bytes = match key {
        Some(key) => {
            sealed = vault::seal(key, bytes, &format!("resource:{id}"));
            &sealed[..]
        }
        None => bytes,
    };
    vault::write_atomic(&paths.resource(id), bytes, false)
}

fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
}

/// The data written where it goes, and whatever the other form left behind taken away: a plain copy must
/// not outlive turning encryption on
fn write_data(paths: &Paths, bytes: &[u8], sealed: bool) -> std::io::Result<()> {
    let (file, other) = if sealed { (&paths.sealed, &paths.plain) } else { (&paths.plain, &paths.sealed) };
    vault::write_atomic(file, bytes, true)?;
    vault::remove_quiet(&[other, &Paths::bak(other)]);
    Ok(())
}

/// Everything changed so far on its way to disk, in order: written away from the window's thread
pub async fn flush(this: &Shared) {
    let _held = gates().saving.enter().await;
    let job = {
        let mut backend = this.borrow_mut();
        if let Some(timer) = backend.save_timer.take() {
            timer.remove();
        }
        if backend.store.is_none() || !backend.dirty || backend.changing {
            return;
        }
        backend.dirty = false;
        let json = backend.store.as_ref().map(Store::to_json).unwrap_or_default();
        let bytes = match &backend.key {
            Some(key) => vault::seal(key, json.as_bytes(), "data"),
            None => json.into_bytes(),
        };
        (backend.paths.clone(), bytes, backend.key.is_some())
    };
    let (paths, bytes, sealed) = job;
    let result = gio::spawn_blocking(move || write_data(&paths, &bytes, sealed).map_err(|e| e.to_string())).await;
    let error = match result {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => Some(String::from("?")),
    };
    if let Some(error) = error {
        this.borrow_mut().dirty = true;
        Backend::problem(this, t!("Your last changes could not be saved: {error}", error = error));
    }
}

// --- Next: turning encryption on or off, and a new key -------------------------------------------------------

/// Whether the key opens a file, or, with no key, whether the file is not sealed
fn fits(key: Option<&Key>, bytes: &[u8], label: &str) -> bool {
    match key {
        None => !vault::is_sealed(bytes),
        Some(key) => vault::unseal(key, bytes, label).is_ok(),
    }
}

/// Moves a file from next/ into place if the vault's key fits it, unless the one in place fits already:
/// that one was saved since, and is newer
fn settle(key: Option<&Key>, from: &Path, to: &Path, label: &str) -> bool {
    let Ok(staged) = std::fs::read(from) else { return false };
    let current = std::fs::read(to).ok();
    if !fits(key, &staged, label) || current.is_some_and(|current| fits(key, &current, label)) {
        return false;
    }
    std::fs::rename(from, to).is_ok()
}

/// Turning encryption on or off and changing the key write everything anew into next/, then vault.json,
/// then move next/ into place. Cut short before vault.json, next/ holds files that the vault's key does
/// not fit, and after it files that it does: only those are moved, and the rest is thrown away. True when
/// the data was moved, which the next save then follows up.
pub fn commit_next(paths: &Paths, key: Option<&Key>) -> bool {
    let staged = paths.next.join("resources");
    if let Ok(entries) = std::fs::read_dir(&staged) {
        let mut names: Vec<String> = entries.flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        for name in names.into_iter().filter(|name| data::is_id(name)) {
            settle(key, &staged.join(&name), &paths.resource(&name), &format!("resource:{name}"));
        }
    }
    let target = if key.is_some() { &paths.sealed } else { &paths.plain };
    let moved = settle(key, &paths.next.join("data"), target, "data");
    let _ = std::fs::remove_dir_all(&paths.next);
    moved
}

/// Everything written anew for the next vault, sealed under its key or, without one, in the clear; then
/// vault.json, then next/ moved into place. Runs away from the window's thread.
fn rewrite(paths: &Paths, old: Option<&Key>, next_key: Option<&Key>, next_vault: &Value, store_json: &str, resources: &[String]) -> Result<bool, String> {
    let io = |error: std::io::Error| error.to_string();
    let _ = std::fs::remove_dir_all(&paths.next);
    std::fs::create_dir_all(paths.next.join("resources")).map_err(io)?;
    set_mode(&paths.next, 0o700);
    set_mode(&paths.next.join("resources"), 0o700);
    for id in resources {
        if let Ok(bytes) = read_resource(paths, old, id) {
            let out = match next_key {
                Some(key) => vault::seal(key, &bytes, &format!("resource:{id}")),
                None => bytes,
            };
            vault::write_atomic(&paths.next.join("resources").join(id), &out, false).map_err(io)?;
        }
    }
    let json = store_json.as_bytes();
    let out = match next_key {
        Some(key) => vault::seal(key, json, "data"),
        None => json.to_vec(),
    };
    vault::write_atomic(&paths.next.join("data"), &out, false).map_err(io)?;
    vault::write_atomic(&paths.vault, vault::pretty(next_vault).as_bytes(), false).map_err(io)?;
    Ok(commit_next(paths, next_key))
}

// --- The vault -----------------------------------------------------------------------------------------------

/// The notes are open: remember what they need reminding of and tidy up after any interrupted change
fn opened(this: &Shared) {
    let dirty = {
        let mut backend = this.borrow_mut();
        let now = data::now_ms();
        backend.last_active = now;
        backend.last_check = now;
        backend.compute_reminders();
        backend.dirty
    };
    let weak = Rc::downgrade(this);
    glib::timeout_add_local_once(Duration::from_secs(5), move || {
        if let Some(this) = weak.upgrade() {
            collect_resources(&this);
        }
    });
    if dirty {
        let this = this.clone();
        glib::spawn_future_local(async move { flush(&this).await });
    }
    let hook = this.borrow().hooks.opened.clone();
    if let Some(hook) = hook {
        hook();
    }
}

/// At start: the vault, and the notes themselves when they are not encrypted
pub fn start(this: &Shared) -> Result<(), String> {
    let mut backend = this.borrow_mut();
    backend.read_vault()?;
    if backend.vault.is_some() && !backend.encrypted() {
        let paths = backend.paths.clone();
        if commit_next(&paths, None) {
            backend.dirty = true;
        }
        let store = backend.read_data()?;
        backend.store = Some(store);
        drop(backend);
        opened(this);
    }
    Ok(())
}

/// The first run: a vault, encrypted with this passphrase or not at all, and a first notebook
pub async fn create(this: &Shared, passphrase: Option<String>) -> Result<(), String> {
    if this.borrow().vault.is_some() {
        return Err(t!("Notes are already set up here."));
    }
    let paths = this.borrow().paths.clone();
    let (secret, next) = gio::spawn_blocking(move || {
        let secret = passphrase.as_ref().map(|_| Key::random());
        let next = match (&passphrase, &secret) {
            (Some(passphrase), Some(secret)) => vault::wrap(passphrase, secret),
            _ => json!({ "format": 1, "encrypted": false }),
        };
        (secret.map(|s| s.copy()), next)
    })
    .await
    .map_err(|_| String::from("?"))?;
    std::fs::create_dir_all(&paths.resources).map_err(|e| e.to_string())?;
    set_mode(&paths.resources, 0o700);
    set_mode(&paths.dir, 0o700);
    vault::write_atomic(&paths.vault, vault::pretty(&next).as_bytes(), false).map_err(|e| e.to_string())?;
    {
        let mut backend = this.borrow_mut();
        backend.vault = Some(next);
        backend.key = secret;
        let mut store = Store::empty();
        let notebook = data::new_id();
        let now = data::ms(data::now_ms());
        // The first notebook, named in the language of the session it was made in
        store.put(Kind::Notebooks, json!({ "id": notebook, "title": t!("Notes"), "parent": null, "created": now, "updated": now }));
        backend.store = Some(store);
        backend.dirty = true;
    }
    flush(this).await;
    opened(this);
    Ok(())
}

/// Opens the notes with their data key
fn open_with(this: &Shared, secret: Key) -> Result<(), String> {
    let mut backend = this.borrow_mut();
    let paths = backend.paths.clone();
    if commit_next(&paths, Some(&secret)) {
        backend.dirty = true;
    }
    backend.key = Some(secret);
    match backend.read_data() {
        Ok(store) => backend.store = Some(store),
        Err(error) => {
            backend.key = None;
            return Err(error);
        }
    }
    drop(backend);
    opened(this);
    Ok(())
}

/// scrypt on the passphrase, away from the window's thread: None for a wrong one
pub async fn unwrap(vault: Value, passphrase: String) -> Result<Option<Key>, String> {
    gio::spawn_blocking(move || vault::unwrap(&passphrase, &vault)).await.map_err(|_| String::from("?"))?
}

pub async fn unlock(this: &Shared, passphrase: String) -> Result<(), String> {
    if this.borrow().store.is_some() {
        return Ok(());
    }
    let Some(sealed) = this.borrow().vault.clone() else { return Err(t!("Unlock your notes first.")) };
    match unwrap(sealed, passphrase).await? {
        Some(secret) => open_with(this, secret),
        None => Err(t!("That is not the passphrase.")),
    }
}

/// Opens the notes with a key a passkey unsealed
pub fn open_with_key(this: &Shared, secret: Key) -> Result<(), String> {
    if this.borrow().store.is_some() {
        return Ok(());
    }
    open_with(this, secret)
}

/// Asks the page to hand over what it has not sent yet: here that is at once, the page being this program
fn request_flush(this: &Shared) {
    let hook = this.borrow().hooks.flush_page.clone();
    if let Some(hook) = hook {
        hook();
    }
}

/// The notes saved and closed, the key wiped: only for encrypted notes, which then need the passphrase
pub async fn lock(this: &Shared) {
    if !this.borrow().encrypted() || this.borrow().store.is_none() {
        return;
    }
    let _held = gates().locking.enter().await;
    if this.borrow().store.is_none() {
        return;
    }
    this.borrow_mut().locking = true;
    // A change of key under way finishes first: it holds the saves
    drop(gates().saving.enter().await);
    request_flush(this);
    flush(this).await;
    {
        let mut backend = this.borrow_mut();
        backend.compute_reminders();
        backend.store = None;
        backend.key = None;
        backend.locking = false;
    }
    let hook = this.borrow().hooks.locked.clone();
    if let Some(hook) = hook {
        hook();
    }
}

/// What the page asks of the encryption: turned on, off, or a new passphrase with or without a new key
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Change {
    Enable,
    Disable,
    Passphrase { new_key: bool },
}

pub async fn set_encryption(this: &Shared, change: Change, current: String, next: String) -> Result<(), String> {
    if this.borrow().store.is_none() {
        return Err(t!("Unlock your notes first."));
    }
    let encrypted = this.borrow().encrypted();
    let sealed = this.borrow().vault.clone().unwrap_or(Value::Null);
    if encrypted && unwrap(sealed.clone(), current).await?.is_none() {
        return Err(t!("That is not the current passphrase."));
    }
    if let (Change::Passphrase { new_key: false }, true, false) = (change, encrypted, next.is_empty()) {
        // The passkeys seal the same key, so they stay. A copy of it, which locking cannot wipe part way.
        let Some(copy) = this.borrow().key.as_ref().map(Key::copy) else { return Err(t!("Unlock your notes first.")) };
        let wrapped = gio::spawn_blocking(move || vault::wrap(&next, &copy)).await.map_err(|_| String::from("?"))?;
        let mut changed = wrapped;
        if let Some(passkeys) = sealed.get("passkeys") {
            data::set(&mut changed, "passkeys", passkeys.clone());
        }
        let paths = this.borrow().paths.clone();
        vault::write_atomic(&paths.vault, vault::pretty(&changed).as_bytes(), false).map_err(|e| e.to_string())?;
        this.borrow_mut().vault = Some(changed);
        return Ok(());
    }
    let (next_key, next_vault) = match change {
        // A new key, so that an old vault.json, in a snapshot or a backup, opens nothing written from now
        // on. The passkeys sealed the old one, and go with it.
        Change::Passphrase { .. } if encrypted && !next.is_empty() => new_vault(next).await?,
        Change::Enable if !encrypted && !next.is_empty() => new_vault(next).await?,
        Change::Disable if encrypted => (None, json!({ "format": 1, "encrypted": false })),
        _ => return Err(t!("Nothing to change.")),
    };
    flush(this).await;
    // Locking would take the key away part way, so neither starts while the other runs
    if this.borrow().store.is_none() || this.borrow().locking || this.borrow().changing {
        return Err(t!("Unlock your notes first."));
    }
    {
        let held = gates().saving.enter().await;
        let job = {
            let mut backend = this.borrow_mut();
            backend.changing = true;
            let store = backend.store.as_ref().expect("checked above");
            let resources: Vec<String> = store.resources.keys().cloned().collect();
            (backend.paths.clone(), backend.key.as_ref().map(Key::copy), next_key.as_ref().map(Key::copy), next_vault.clone(), store.to_json(), resources)
        };
        let (paths, old, next_copy, vault_copy, json, resources) = job;
        let result = gio::spawn_blocking(move || rewrite(&paths, old.as_ref(), next_copy.as_ref(), &vault_copy, &json, &resources))
            .await
            .unwrap_or_else(|_| Err(String::from("?")));
        let mut backend = this.borrow_mut();
        backend.changing = false;
        drop(held);
        // vault.json is the last thing written that can fail: until it is, the old vault and key stand
        result?;
        backend.vault = Some(next_vault);
        backend.key = next_key;
        backend.dirty = true;
    }
    flush(this).await;
    Ok(())
}

async fn new_vault(passphrase: String) -> Result<(Option<Key>, Value), String> {
    gio::spawn_blocking(move || {
        let key = Key::random();
        let wrapped = vault::wrap(&passphrase, &key);
        (Some(key), wrapped)
    })
    .await
    .map_err(|_| String::from("?"))
}

// --- Attachments -------------------------------------------------------------------------------------------

/// The types an attachment's name gives it (TYPES in main.js)
pub const TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("svg", "image/svg+xml"),
    ("avif", "image/avif"),
    ("bmp", "image/bmp"),
    ("pdf", "application/pdf"),
    ("txt", "text/plain"),
    ("md", "text/markdown"),
    ("zip", "application/zip"),
    ("mp3", "audio/mpeg"),
    ("ogg", "audio/ogg"),
    ("mp4", "video/mp4"),
    ("webm", "video/webm"),
    ("odt", "application/vnd.oasis.opendocument.text"),
    ("json", "application/json"),
];

/// path.extname(name).slice(1): what follows the last dot, unless the name starts with it
pub fn extension(name: &str) -> &str {
    let base = name.rsplit('/').next().unwrap_or(name);
    match base.rfind('.') {
        Some(at) if at > 0 && base[..at].chars().any(|c| c != '.') => &base[at + 1..],
        _ => "",
    }
}

pub fn type_of(name: &str) -> String {
    let own = extension(name).to_lowercase();
    TYPES.iter().find(|(ext, _)| *ext == own).map(|(_, mime)| mime.to_string()).unwrap_or_else(|| "application/octet-stream".into())
}

pub fn extension_of(mime: &str, name: &str) -> String {
    let own = extension(name).to_lowercase();
    if !own.is_empty() && own.len() <= 8 && own.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) {
        return own;
    }
    TYPES.iter().find(|(_, kind)| *kind == mime).map(|(ext, _)| ext.to_string()).unwrap_or_else(|| "bin".into())
}

/// The record of an attachment as main.js kept it in store.resources
pub fn resource_record(id: &str, name: &str, mime: &str, size: usize) -> Value {
    let name = if name.is_empty() { "Attachment" } else { name };
    json!({ "id": id, "name": data::js_slice(name, 200), "mime": mime, "size": size, "created": data::ms(data::now_ms()) })
}

/// An attachment added: sealed and written away from the window's thread, then in the store
pub async fn add_resource(this: &Shared, name: String, mime: String, bytes: Vec<u8>, id: Option<String>) -> Result<Value, String> {
    let id = id.unwrap_or_else(data::new_id);
    let (paths, key) = {
        let backend = this.borrow();
        let key = backend.sealing()?.map(Key::copy);
        (backend.paths.clone(), key)
    };
    let size = bytes.len();
    let target = id.clone();
    gio::spawn_blocking(move || write_resource(&paths, key.as_ref(), &target, &bytes).map_err(|e| e.to_string()))
        .await
        .unwrap_or_else(|_| Err(String::from("?")))?;
    let mime = if mime.is_empty() { type_of(&name) } else { mime };
    let resource = resource_record(&id, &name, &mime, size);
    {
        let mut backend = this.borrow_mut();
        let Some(store) = backend.store.as_mut() else { return Err(t!("Unlock your notes first.")) };
        store.resources.insert(id, resource.clone());
    }
    Backend::schedule_save(this);
    Ok(resource)
}

/// Files the person chose: each read, unless it is larger than 200 MB, and attached
pub async fn attach_files(this: &Shared, files: Vec<PathBuf>) -> Vec<Value> {
    let mut added = Vec::new();
    for file in files {
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        if size > ATTACHMENT_LIMIT {
            Backend::problem(this, t!("{file} is larger than 200 MB and was not attached.", file = name.clone()));
            continue;
        }
        let path = file.clone();
        let Ok(Ok(bytes)) = gio::spawn_blocking(move || std::fs::read(path)).await else { continue };
        match add_resource(this, name.clone(), type_of(&file.to_string_lossy()), bytes, None).await {
            Ok(resource) => added.push(resource),
            Err(error) => Backend::problem(this, error),
        }
    }
    added
}

/// What was pasted or dropped: a type of the form type/subtype, or the one its name gives it, and a name
/// of its own when it came with none
pub async fn attach_data(this: &Shared, name: String, mime: String, bytes: Vec<u8>) -> Result<Value, String> {
    if this.borrow().store.is_none() || bytes.len() as u64 > ATTACHMENT_LIMIT {
        return Err(String::new());
    }
    let well_formed = |mime: &str| {
        let Some((kind, sub)) = mime.split_once('/') else { return false };
        let part = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_' || matches!(c, '.' | '+' | '-'));
        part(kind) && part(sub)
    };
    let kind = if well_formed(&mime) { mime } else { type_of(&name) };
    let name = if name.is_empty() {
        format!("Pasted {}.{}", utc_date(data::now_ms()), extension_of(&kind, ""))
    } else {
        name
    };
    add_resource(this, name, kind, bytes, None).await
}

/// new Date().toISOString().slice(0, 10)
fn utc_date(ms: f64) -> String {
    let days = (ms / 86400000.0).floor() as i64;
    let (year, month, day) = crate::dates::civil_from_days(days);
    crate::dates::iso(year, month, day)
}

/// /:\/([0-9a-f]{32})/g: the attachments a text refers to
pub fn resource_links(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;
    while at + 34 <= bytes.len() {
        if bytes[at] == b':' && bytes[at + 1] == b'/' && data::is_id(&text[at + 2..at + 34]) {
            found.push(text[at + 2..at + 34].to_string());
            at += 34;
        } else {
            at += 1;
        }
    }
    found
}

/// Removes the attachments that no note, version or task refers to any more
pub fn collect_resources(this: &Shared) {
    let removed = {
        let mut backend = this.borrow_mut();
        let paths = backend.paths.clone();
        let Some(store) = backend.store.as_mut() else { return };
        let mut referenced: HashSet<String> = HashSet::new();
        for note in store.values(Kind::Notes) {
            referenced.extend(resource_links(&data::text(note.get("body"))));
        }
        for revision in store.values(Kind::Revisions) {
            referenced.extend(resource_links(&data::text(revision.get("body"))));
        }
        for task in store.values(Kind::Tasks) {
            referenced.extend(resource_links(&data::text(task.get("notes"))));
        }
        let now = data::now_ms();
        let stale: Vec<String> = store
            .resources
            .iter()
            .filter(|(id, resource)| !referenced.contains(*id) && now - resource.num_or_zero("created") > GRACE)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &stale {
            store.resources.remove(id);
            let _ = std::fs::remove_file(paths.resource(id));
        }
        // Files a crash left without an entry
        if let Ok(entries) = std::fs::read_dir(&paths.resources) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let orphan = if data::is_id(&name) { !store.resources.contains_key(&name) } else { name.ends_with(".tmp") };
                if orphan {
                    let old = entry
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                        .is_some_and(|m| now - m.as_millis() as f64 > GRACE);
                    if old {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
        }
        if !stale.is_empty() {
            backend.dirty = true;
        }
        backend.dirty
    };
    if removed {
        Backend::schedule_save(this);
    }
}

/// The attachment written where the person chose, as it is, unsealed
pub async fn save_resource(this: &Shared, id: &str, file: PathBuf) -> Result<(), String> {
    let (paths, key) = {
        let backend = this.borrow();
        if !backend.store.as_ref().is_some_and(|store| store.resources.contains_key(id)) {
            return Err(t!("That attachment is not here any more."));
        }
        (backend.paths.clone(), backend.key.as_ref().map(Key::copy))
    };
    let id = id.to_string();
    gio::spawn_blocking(move || {
        let bytes = read_resource(&paths, key.as_ref(), &id).map_err(|_| String::from("Unsupported state or unable to authenticate data"))?;
        std::fs::write(&file, bytes).map_err(|e| e.to_string())
    })
    .await
    .unwrap_or_else(|_| Err(String::from("?")))
}

// --- Export, backup and import ------------------------------------------------------------------------------

pub async fn export_markdown(this: &Shared, into: PathBuf) -> Result<crate::export::Exported, String> {
    let (store, paths, key) = {
        let backend = this.borrow();
        let Some(store) = backend.store.clone() else { return Err(t!("Unlock your notes first.")) };
        (store, backend.paths.clone(), backend.key.as_ref().map(Key::copy))
    };
    gio::spawn_blocking(move || {
        crate::export::export_markdown(&store, &into, &|id| read_resource(&paths, key.as_ref(), id).ok()).map_err(|e| e.to_string())
    })
    .await
    .unwrap_or_else(|_| Err(String::from("?")))
}

/// One file with everything, attachments included, sealed with the notes' own key and carrying it wrapped
/// under the current passphrase: it opens with the passphrase it was made with, on any machine. The
/// passkeys are left out of it. True when it is encrypted.
pub async fn backup(this: &Shared, file: PathBuf) -> Result<bool, String> {
    let (store, paths, key, vault_value) = {
        let backend = this.borrow();
        let Some(store) = backend.store.clone() else { return Err(t!("Unlock your notes first.")) };
        // Locked while the attachments were read: nothing of an encrypted vault is written in the clear
        let key = backend.sealing()?.map(Key::copy);
        (store, backend.paths.clone(), key, backend.vault.clone().unwrap_or(Value::Null))
    };
    gio::spawn_blocking(move || {
        let mut resources = Map::new();
        for id in store.resources.keys() {
            if let Ok(bytes) = read_resource(&paths, key.as_ref(), id) {
                resources.insert(id.clone(), Value::String(vault::base64(&bytes)));
            }
        }
        let text = crate::import::backup_file(&store, resources, key.as_ref().map(|key| (key, &vault_value)));
        vault::write_atomic(&file, text.as_bytes(), false).map_err(|e| e.to_string())?;
        Ok(key.is_some())
    })
    .await
    .unwrap_or_else(|_| Err(String::from("?")))
}

/// What an import did: what it brought in, the files it left out and why, and the name of an encrypted
/// backup whose passphrase the page is to ask for
#[derive(Debug, Default)]
pub struct Imported {
    pub counts: crate::import::Counts,
    pub skipped: Vec<String>,
    pub passphrase: Option<String>,
}

/// Where an import's attachments go: written in place, sealed if the notes are, and listed in the store
fn resource_sink(paths: Paths, key: Option<Key>) -> impl FnMut(&mut Store, &str, &str, &[u8], &str) -> Result<(), String> {
    move |store, name, mime, bytes, id| {
        write_resource(&paths, key.as_ref(), id, bytes).map_err(|e| e.to_string())?;
        crate::import::record(store, name, mime, bytes.len(), id);
        Ok(())
    }
}

fn base_name(file: &Path) -> String {
    file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

pub async fn import_files(this: &Shared, files: Vec<PathBuf>) -> Result<Imported, String> {
    use crate::import::{self, Counts};
    if this.borrow().store.is_none() {
        return Err(t!("Unlock your notes first."));
    }
    let mut imported = Imported { counts: Counts::default(), ..Default::default() };
    for file in files {
        let extension = format!(".{}", extension(&file.to_string_lossy())).to_lowercase();
        // Read away from the window's thread; a Joplin export can be large
        let path = file.clone();
        let bytes = gio::spawn_blocking(move || std::fs::read(path)).await.unwrap_or_else(|_| Err(std::io::Error::other("?")));
        let outcome: Result<(), String> = (|| {
            let bytes = bytes.map_err(|e| e.to_string())?;
            let mut backend = this.borrow_mut();
            let (paths, key) = (backend.paths.clone(), backend.sealing()?.map(Key::copy));
            let mut add = resource_sink(paths, key);
            let store = backend.store.as_mut().ok_or_else(|| t!("Unlock your notes first."))?;
            match extension.as_str() {
                ".jex" => import::import_jex(store, &bytes, &mut imported.counts, &mut add),
                ".amnotes" => {
                    let saved = data::parse_json(&String::from_utf8_lossy(&bytes)).map_err(|e| e.to_string())?;
                    if saved.get("format").and_then(Value::as_str) != Some("amethystora-notes-backup") {
                        return Err(t!("it is not a Notes backup"));
                    }
                    if saved.is("encrypted") {
                        // The passphrase is asked for by the page, and the file stays here
                        imported.passphrase = Some(base_name(&file));
                        backend.pending_backup = Some(saved);
                        return Ok(());
                    }
                    let unpacked = import::open_backup(&saved, None)?;
                    import::import_backup(store, &unpacked, &mut imported.counts, &mut add)
                }
                ".json" => {
                    let planify = data::parse_json(&String::from_utf8_lossy(&bytes)).map_err(|e| e.to_string())?;
                    if !planify.get("items").is_some_and(Value::is_array) || !planify.get("projects").is_some_and(Value::is_array) {
                        return Err(t!("it is not a Planify backup"));
                    }
                    import::import_planify(store, &planify, &mut imported.counts);
                    Ok(())
                }
                _ => {
                    import::import_markdown(store, &file.to_string_lossy(), &String::from_utf8_lossy(&bytes), &mut imported.counts);
                    Ok(())
                }
            }
        })();
        if imported.passphrase.is_some() {
            Backend::schedule_save(this);
            return Ok(Imported { skipped: Vec::new(), ..imported });
        }
        if let Err(reason) = outcome {
            imported.skipped.push(t!("{file} was not imported: {reason}", file = base_name(&file), reason = reason));
        }
    }
    finish_import(this);
    Ok(imported)
}

pub async fn import_pending(this: &Shared, passphrase: String) -> Result<crate::import::Counts, String> {
    use crate::import;
    let Some(saved) = this.borrow().pending_backup.clone().filter(|_| this.borrow().store.is_some()) else {
        return Err(t!("Choose the backup again."));
    };
    let mut counts = import::Counts::default();
    let backup_key = if saved.is("encrypted") {
        match unwrap(saved.clone(), passphrase).await? {
            Some(key) => Some(key),
            None => return Err(t!("That is not the passphrase this backup was made with.")),
        }
    } else {
        None
    };
    let unpacked = gio::spawn_blocking(move || import::open_backup(&saved, backup_key.as_ref())).await.unwrap_or_else(|_| Err(String::from("?")))?;
    {
        let mut backend = this.borrow_mut();
        let (paths, key) = (backend.paths.clone(), backend.sealing()?.map(Key::copy));
        let mut add = resource_sink(paths, key);
        let store = backend.store.as_mut().ok_or_else(|| t!("Choose the backup again."))?;
        import::import_backup(store, &unpacked, &mut counts, &mut add)?;
        backend.pending_backup = None;
    }
    finish_import(this);
    Ok(counts)
}

fn finish_import(this: &Shared) {
    Backend::schedule_save(this);
    this.borrow_mut().compute_reminders();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_as_main_js_had_them() {
        assert_eq!(type_of("Photo.JPG"), "image/jpeg");
        assert_eq!(type_of(".bashrc"), "application/octet-stream");
        assert_eq!(type_of("archive"), "application/octet-stream");
        assert_eq!(extension_of("image/png", ""), "png");
        assert_eq!(extension_of("image/png", "x.jpeg"), "jpeg");
        assert_eq!(extension_of("x/y", "x.verylongext"), "bin");
        assert_eq!(resource_links("a ![x](:/0123456789abcdef0123456789abcdef) b :/xyz [y](:/fedcba9876543210fedcba9876543210)"),
            vec!["0123456789abcdef0123456789abcdef", "fedcba9876543210fedcba9876543210"]);
        assert_eq!(utc_date(0.0), "1970-01-01");
    }
}
