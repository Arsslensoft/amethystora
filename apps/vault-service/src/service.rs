//! What the service does with a request, apart from sockets and polling (`server.rs`), so that all of it can be tested
//! through a fake of the outside. It answers at once, or says what it is waiting for and what follows: an approval, which a
//! person gives, or work off the loop, such as scrypt.
//!
//! The account is always the kernel's word (`Peer`), and picks the only vaults a request can touch.

use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use amethystora_vault_core::items::KIND_PASSWORD;
use amethystora_vault_core::protocol::{self, service_only, Approval, Method, Refusal, Request, Secret};
use amethystora_vault_core::seal::{self, Key};
use amethystora_vault_core::slots::{self, Chip, Found, Kdf, RecoveryKey, Slot, SlotKind, Table};
use amethystora_vault_core::store::{Entry, Store, Trouble, DATA_KEY};
use amethystora_vault_core::sync::kinds;
use amethystora_vault_core::vaults::{self, Account, Background, Draft, MAX_SLOTS};

use crate::outside::{say, Action, Ask, Event, Outside};
use crate::peer::Peer;

/// A setup's recovery key can wait this long to be typed back
const DRAFT_PATIENCE: Duration = Duration::from_secs(15 * 60);
/// A backup brought in or read out goes chunk after chunk, with nothing asked in between: what waits longer than this was left
pub(crate) const STAGED_PATIENCE: Duration = Duration::from_secs(60);
/// Room left in the account's quota for the index and the meta, which a write takes again
const SPARE: u64 = 64 * 1024;
/// The purpose whose key is also on the chip, released while the vault is closed
pub const BACKGROUND: &str = "background";

pub type Then<T> = Box<dyn FnOnce(&mut Service, &Rc<Peer>, T) -> Next>;

/// What a request comes to
pub enum Next {
    Answer(Value),
    Approve { ask: Ask, then: Then<bool> },
    /// Goes on only if none of the account's sessions is locked: nothing is put to anyone
    WhenUnlocked { then: Then<bool> },
    Work { job: Box<dyn FnOnce() -> Box<dyn Any + Send> + Send>, then: Then<Box<dyn Any + Send>> },
    /// Work that may take minutes, a server being on the other end: a thread of its own, so that it never holds up the one
    /// work thread, which everyone's scrypt is waiting for (`sync.rs`)
    Slow { job: Box<dyn FnOnce() -> Box<dyn Any + Send> + Send>, then: Then<Box<dyn Any + Send>> },
}

impl Next {
    pub fn ok() -> Next {
        Next::Answer(json!({"ok": true}))
    }

    pub fn refuse(why: Refusal) -> Next {
        Next::Answer(protocol::refusal(why))
    }

    pub fn trouble(trouble: Trouble) -> Next {
        match trouble {
            // Telling these apart would say more than the person asking needs, and might say it to a program
            Trouble::NotFound | Trouble::Damaged => Next::refuse(Refusal::Refused),
            Trouble::Stale => Next::Answer(json!({"ok": false, "why": Refusal::Failed.name(), "stale": true})),
            Trouble::Disk => Next::refuse(Refusal::Failed),
            Trouble::Full => Next::refuse(Refusal::TooLarge),
        }
    }

    /// Goes on to `then` once none of the account's sessions is locked: an opening answered behind the lock screen would be
    /// the screen's to keep out
    pub fn when_unlocked(then: impl FnOnce(&mut Service, &Rc<Peer>) -> Next + 'static) -> Next {
        Next::WhenUnlocked { then: Box::new(move |service, peer, yes| if yes { then(service, peer) } else { Next::refuse(Refusal::Closed) }) }
    }

    pub fn work<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static, then: impl FnOnce(&mut Service, &Rc<Peer>, T) -> Next + 'static) -> Next {
        Next::Work {
            job: Box::new(move || Box::new(job()) as Box<dyn Any + Send>),
            then: Box::new(move |service, peer, done| match done.downcast::<T>() {
                Ok(done) => then(service, peer, *done),
                Err(_) => Next::refuse(Refusal::Failed),
            }),
        }
    }

    pub fn slow<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static, then: impl FnOnce(&mut Service, &Rc<Peer>, T) -> Next + 'static) -> Next {
        match Next::work(job, then) {
            Next::Work { job, then } => Next::Slow { job, then },
            other => other,
        }
    }

    /// Asks the person, and goes on only on a yes
    pub fn approve(action: Action, message: &'static str, item: &str, then: impl FnOnce(&mut Service, &Rc<Peer>) -> Next + 'static) -> Next {
        Next::approve_with(action, message, item, "", then)
    }

    /// `approve` for a sentence with a `$(detail)` in it
    pub fn approve_with(action: Action, message: &'static str, item: &str, detail: &str, then: impl FnOnce(&mut Service, &Rc<Peer>) -> Next + 'static) -> Next {
        Next::Approve {
            ask: Ask { action, message, item: item.to_string(), detail: detail.to_string() },
            then: Box::new(move |service, peer, yes| if yes { then(service, peer) } else { Next::refuse(Refusal::Refused) }),
        }
    }
}

pub struct Config {
    /// /var/lib/amethystora/vault, which only the service's user reaches
    pub state: PathBuf,
    /// scrypt's cost for a new account: a test's is lower
    pub log_n: u8,
    /// amethystora-vault-work, which does everything that takes libcrypto, one short-lived process at a time
    pub worker: PathBuf,
}

/// How long a yes to a signature lasts, for `with_window`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reuse {
    For(Duration),
    /// Not at all: a `While open` key asks every time this one is asked
    Never,
}

/// What is open for one account
struct Session {
    /// In the order they were opened: a name is looked for from the last
    vaults: Vec<Store>,
    /// Items of `While open` asked about since opening, with when that yes runs out if it has a window of its own
    approved: HashMap<String, Option<Instant>>,
    last_use: Instant,
    idle: Duration,
    /// What the account's folder takes on the disk, counted when it was first opened and kept up to date by what is written
    used: u64,
}

pub struct Service {
    pub config: Config,
    pub chip: Arc<dyn Chip>,
    pub outside: Box<dyn Outside>,
    sessions: HashMap<u32, Session>,
    drafts: HashMap<u32, (Draft, Instant)>,
    /// A backup being brought in, a chunk at a time, until a restore uses it or it is not wanted (`plugs/moving.rs`)
    pub(crate) staged: HashMap<u32, (Vec<u8>, Instant)>,
    /// A backup being read out, a chunk at a time: made once at its first chunk and let go after its last (`plugs/moving.rs`)
    pub(crate) made: HashMap<u32, (Vec<u8>, Instant)>,
    /// Counts closings, each account's and everyone's, so that an opening that took its time does not undo one that came
    /// meanwhile. An account's own, or another's lock would stop this one's opening.
    closes: HashMap<u32, u64>,
    closes_of_all: u64,
    /// Which accounts closed since the server last looked (None: everyone), so that it takes back what it was asking them
    closed: Vec<Option<u32>>,
    /// Sync with a Vaultwarden or Bitwarden server: when each account's next run is due, and the runs waiting for a thread
    pub sync: crate::sync::Hub,
}

fn no_secret<T>(_: T) -> Next {
    Next::refuse(Refusal::Refused)
}

impl Service {
    pub fn new(config: Config, chip: Arc<dyn Chip>, outside: Box<dyn Outside>) -> Service {
        vaults::sweep_forgotten(&config.state);
        Service { config, chip, outside, sessions: HashMap::new(), drafts: HashMap::new(), staged: HashMap::new(), made: HashMap::new(), closes: HashMap::new(), closes_of_all: 0, closed: Vec::new(), sync: crate::sync::Hub::default() }
    }

    /// Whether the account is served at all: it has a login session, or it has none and lingers. The second is served the
    /// `background` key and nothing else (`request`), since nobody is there to approve anything.
    pub fn serves(&self, uid: u32) -> bool {
        self.outside.has_session(uid) || self.outside.lingering(uid)
    }

    /// Whether the account has a login session
    pub fn has_session(&self, uid: u32) -> bool {
        self.outside.has_session(uid)
    }

    /// Who closed since the last call: the server takes back the prompts it has up for them
    pub fn take_closed(&mut self) -> Vec<Option<u32>> {
        std::mem::take(&mut self.closed)
    }

    /// The person is using the vault: only what succeeded counts, so that asking for what is not there, or being refused, never
    /// keeps it open
    pub(crate) fn touch(&mut self, uid: u32) {
        if let Some(session) = self.sessions.get_mut(&uid) {
            session.last_use = Instant::now();
        }
    }

    /// Whether anything keeps the service from going away
    pub fn is_quiet(&self) -> bool {
        self.sessions.is_empty() && self.drafts.is_empty() && self.staged.is_empty() && self.made.is_empty()
    }

    pub fn is_open(&self, uid: u32) -> bool {
        self.sessions.contains_key(&uid)
    }

    /// When something next has to close by itself: one deadline, and no polling
    pub fn deadline(&self) -> Option<Instant> {
        let idle = self.sessions.values().map(|session| session.last_use + session.idle);
        let drafts = self.drafts.values().map(|(_, since)| *since + DRAFT_PATIENCE);
        let staged = self.staged.values().chain(self.made.values()).map(|(_, since)| *since + STAGED_PATIENCE);
        idle.chain(drafts).chain(staged).chain(self.sync.deadline()).min()
    }

    pub fn tick(&mut self, now: Instant) {
        crate::sync::tick(self, now);
        let idle: Vec<u32> = self.sessions.iter().filter(|(_, session)| session.last_use + session.idle <= now).map(|(uid, _)| *uid).collect();
        for uid in idle {
            self.close(uid);
        }
        self.drafts.retain(|_, (_, since)| *since + DRAFT_PATIENCE > now);
        self.staged.retain(|_, (_, since)| *since + STAGED_PATIENCE > now);
        self.made.retain(|_, (_, since)| *since + STAGED_PATIENCE > now);
    }

    /// Closes what this account has open: the keys are wiped as they drop
    pub fn close(&mut self, uid: u32) {
        *self.closes.entry(uid).or_default() += 1;
        self.closed.push(Some(uid));
        self.sessions.remove(&uid);
        self.drafts.remove(&uid);
        self.staged.remove(&uid);
        self.sync.closed(uid);
        if self.sessions.is_empty() {
            self.outside.unwatch();
        }
    }

    pub fn close_all(&mut self) {
        self.closes_of_all += 1;
        self.closed.push(None);
        self.sessions.clear();
        self.drafts.clear();
        self.staged.clear();
        self.sync.closed_all();
        self.outside.unwatch();
    }

    /// What the outside said. An approval's answer is the server's to route.
    pub fn event(&mut self, event: &Event) {
        match event {
            Event::Locked(uid) => self.close(*uid),
            Event::Sleeping | Event::Lost => self.close_all(),
            Event::SessionsLeft(left) => {
                let gone: Vec<u32> = self.sessions.keys().filter(|uid| !left.contains(uid)).copied().collect();
                for uid in gone {
                    self.close(uid);
                }
            }
            Event::Approved(..) | Event::NotAsked(_) => {}
        }
    }

    pub fn closes_so_far(&self, uid: u32) -> u64 {
        self.closes_of_all + self.closes.get(&uid).copied().unwrap_or(0)
    }

    pub(crate) fn account(&self, uid: u32) -> Result<Account, Refusal> {
        match Account::load(&self.config.state, uid) {
            Ok(Some(account)) => Ok(account),
            Ok(None) => Err(Refusal::NotSetUp),
            Err(_) => Err(Refusal::Failed),
        }
    }

    pub fn request(&mut self, peer: &Rc<Peer>, line: &[u8]) -> Next {
        let request = match protocol::parse(line) {
            Ok(request) => request,
            Err(why) => return Next::refuse(why),
        };
        let uid = peer.uid;
        // Logged out and lingering (a night's backup, a routine): the key for such work, which needs no vault open and no one, and no other request
        if !self.outside.has_session(uid) && !matches!(&request, Request::Seal { purpose, .. } | Request::Open { purpose, .. } if purpose == BACKGROUND) {
            return Next::refuse(Refusal::Closed);
        }
        if crate::sync::plug_changes(self, uid, &request) {
            self.sync.changed(uid);
        }
        match request {
            Request::Seal { purpose, record, data } => self.seal(uid, purpose, record, data),
            Request::Open { purpose, record, data } => self.unseal(uid, purpose, record, data),
            Request::Forget { purpose } => self.forget(uid, &purpose),
            Request::Status => self.status(uid),
            Request::Setup { name, method, secret } => self.setup(uid, name, method, secret),
            // Opening and keeping a vault wait for the screen to be unlocked, whichever way in: nothing opens behind the lock
            Request::Confirm { recovery } => Next::when_unlocked(move |service, peer| service.confirm(peer.uid, &recovery)),
            Request::New { name, hidden, secret } => self.new_vault(uid, name, hidden, secret),
            Request::Unlock { method, slot, vault, secret } => Next::when_unlocked(move |service, peer| service.unlock(peer.uid, method, slot, vault, secret)),
            Request::Lock => {
                self.close(uid);
                Next::ok()
            }
            Request::Methods => self.methods(uid),
            Request::AddSecurityKey { name, credential, salt, secret, vault } => self.add_security_key(uid, name, credential, salt, secret, vault),
            Request::AddChipSlot { kind, vault } => self.add_chip_slot(uid, kind, vault),
            Request::RemoveSlot { slot } => self.remove_slot(slot),
            Request::Background { on } => self.background(uid, on),
            Request::ChangePassword { vault, current, secret } => self.change_password(uid, vault, current, secret),
            Request::SetIdle { minutes } => self.set_idle(uid, minutes),
            Request::Rename { vault, name } => self.rename(uid, vault.as_deref(), name),
            Request::RemoveThis { vault } => self.remove_this(uid, vault.as_deref()),
            Request::ForgetEverything => self.forget_everything(),
            Request::Put { kind, id, name, meta, approval, value, vault } => self.put(uid, kind, id, name, meta, approval, value, vault.as_deref()),
            Request::Get { kind, id } => self.get(uid, kind, id),
            Request::List { kind } => self.list(uid, kind.as_deref()),
            Request::Remove { kind, id } => self.remove(uid, kind, id),
            Request::SetApproval { kind, id, approval } => self.set_approval(uid, kind, id, approval),
            Request::Plug { op, body } => match crate::plugs::find(&op) {
                Some(plug) => plug(self, peer, &body).unwrap_or_else(Next::refuse),
                None => Next::refuse(Refusal::BadRequest),
            },
        }
    }

    // --- What other programs use: no prompt, ever, and nothing counts as the person's use -------------------------------

    /// The first vault, if it is open: the only one that holds data keys, so that nothing the desktop relies on depends on a
    /// vault it cannot see
    pub(crate) fn first_open(&mut self, uid: u32) -> Option<&mut Store> {
        self.sessions.get_mut(&uid)?.vaults.iter_mut().find(|store| store.meta.first)
    }

    /// How many vaults of this account are open
    pub(crate) fn open_count(&self, uid: u32) -> usize {
        self.sessions.get(&uid).map_or(0, |session| session.vaults.len())
    }

    fn seal(&mut self, uid: u32, purpose: String, record: String, data: Zeroizing<Vec<u8>>) -> Next {
        if let Some(store) = self.first_open(uid) {
            let key = match store.data_key(&purpose, true) {
                Ok(Some(key)) => key,
                Ok(None) => return Next::refuse(Refusal::Refused),
                Err(trouble) => return Next::trouble(trouble),
            };
            let sealed = seal::seal_data(&key, store.id(), &purpose, &record, &data);
            return Next::Answer(json!({"ok": true, "data": protocol::encode(&sealed)}));
        }
        self.while_closed(uid, purpose, move |key, vault, purpose| Ok(seal::seal_data(key, vault, purpose, &record, &data)))
    }

    fn unseal(&mut self, uid: u32, purpose: String, record: String, data: Vec<u8>) -> Next {
        if let Some(store) = self.first_open(uid) {
            let key = match store.data_key(&purpose, false) {
                Ok(Some(key)) => key,
                Ok(None) => return Next::refuse(Refusal::Refused),
                Err(trouble) => return Next::trouble(trouble),
            };
            return match seal::open_data(&key, store.id(), &purpose, &record, &data) {
                Ok(plain) => Next::Answer(json!({"ok": true, "data": protocol::encode(&plain)})),
                Err(_) => Next::refuse(Refusal::Refused),
            };
        }
        self.while_closed(uid, purpose, move |key, vault, purpose| seal::open_data(key, vault, purpose, &record, &data).map(|plain| plain.to_vec()))
    }

    /// Only the `background` key is had with the vault closed, from the chip, off the loop: anything else waits
    fn while_closed(&mut self, uid: u32, purpose: String, make: impl FnOnce(&Key, &str, &str) -> Result<Vec<u8>, seal::Unopened> + Send + 'static) -> Next {
        let account = match self.account(uid) {
            Ok(account) => account,
            Err(why) => return Next::refuse(why),
        };
        let (Some(background), Some(first), true) = (account.file.background.clone(), account.first().map(|first| first.id.clone()), purpose == BACKGROUND) else {
            return Next::refuse(Refusal::Closed);
        };
        let chip = self.chip.clone();
        Next::work(
            move || {
                let sealed = base64::engine::general_purpose::STANDARD.decode(&background.sealed).map_err(|_| seal::Unopened)?;
                let plain = chip.open(&slots::chip_name(&background.id), &sealed)?;
                let key = Key::from_slice(&plain).ok_or(seal::Unopened)?;
                make(&key, &first, &purpose).map(Zeroizing::new)
            },
            |_, _, done: Result<Zeroizing<Vec<u8>>, seal::Unopened>| match done {
                Ok(out) => Next::Answer(json!({"ok": true, "data": protocol::encode(&out)})),
                Err(_) => Next::refuse(Refusal::Refused),
            },
        )
    }

    fn forget(&mut self, uid: u32, purpose: &str) -> Next {
        let Some(store) = self.first_open(uid) else {
            return match self.account(uid) {
                Ok(_) => Next::refuse(Refusal::Closed),
                Err(why) => Next::refuse(why),
            };
        };
        // Nothing to lose, nothing to ask
        if store.entry(DATA_KEY, purpose).is_none() {
            if purpose == BACKGROUND {
                self.drop_background(uid);
            }
            return Next::ok();
        }
        // The key is what every file of a feature is sealed under: forgetting it loses them for good, so a person says so
        let purpose = purpose.to_string();
        Next::approve(Action::Use, say::REMOVE_ITEM, &purpose.clone(), move |service, peer| {
            let uid = peer.uid;
            let Some(store) = service.first_open(uid) else { return Next::refuse(Refusal::Closed) };
            match store.remove(DATA_KEY, &purpose) {
                Ok(()) | Err(Trouble::NotFound) => {}
                Err(trouble) => return Next::trouble(trouble),
            }
            if purpose == BACKGROUND {
                service.drop_background(uid);
            }
            service.touch(uid);
            Next::ok()
        })
    }

    fn drop_background(&mut self, uid: u32) {
        if let Ok(mut account) = self.account(uid)
            && account.file.background.take().is_some()
        {
            let _ = account.save();
        }
    }

    // --- Saying what there is --------------------------------------------------------------------------------------------

    fn status(&self, uid: u32) -> Next {
        let account = match self.account(uid) {
            Ok(account) => account,
            Err(Refusal::NotSetUp) => return Next::Answer(json!({"ok": true, "set_up": false, "tpm": self.chip.present(), "vaults": []})),
            Err(why) => return Next::refuse(why),
        };
        let open: Vec<&Store> = self.sessions.get(&uid).map(|session| session.vaults.iter().collect()).unwrap_or_default();
        Next::Answer(json!({
            "ok": true,
            "set_up": true,
            "tpm": self.chip.present(),
            "idle_minutes": account.file.idle_minutes,
            "background": account.file.background.is_some(),
            "vaults": vaults::describe(&account, &open),
        }))
    }

    /// How each vault opens: a security key's credential and salt are what its client needs to ask it for the secret
    fn methods(&self, uid: u32) -> Next {
        let account = match self.account(uid) {
            Ok(account) => account,
            Err(why) => return Next::refuse(why),
        };
        let session = self.sessions.get(&uid);
        let name_of = |id: &str| session.and_then(|session| session.vaults.iter().find(|store| store.id() == id)).map(|store| store.meta.name.clone());
        let mut list: Vec<Value> = account
            .file
            .listed
            .iter()
            .map(|vault| {
                let slots: Vec<Value> = vault
                    .slots
                    .iter()
                    .map(|slot| json!({"id": slot.id, "kind": slot.kind.name(), "name": slot.name, "credential": slot.credential, "salt": slot.salt, "chip": slot.chip}))
                    .collect();
                json!({"id": vault.id, "name": name_of(&vault.id), "first": vault.first, "password": vault.password, "slots": slots})
            })
            .collect();
        for store in session.iter().flat_map(|session| session.vaults.iter()).filter(|store| store.meta.hidden) {
            list.push(json!({"id": store.id(), "name": store.meta.name, "first": false, "hidden": true, "password": store.meta.password.is_some(), "slots": []}));
        }
        Next::Answer(json!({"ok": true, "tpm": self.chip.present(), "background": account.file.background.is_some(), "vaults": list}))
    }

    // --- Setting up ----------------------------------------------------------------------------------------------------

    fn setup(&mut self, uid: u32, name: String, method: Method, secret: Option<Secret>) -> Next {
        match self.account(uid) {
            Err(Refusal::NotSetUp) => {}
            Ok(_) => return Next::refuse(Refusal::Refused),
            Err(why) => return Next::refuse(why),
        }
        let slot = match (method, &secret) {
            (Method::Password, Some(secret)) if slots::usable(secret) => None,
            (Method::Password, _) => return Next::refuse(Refusal::Refused),
            (Method::Approval, None) => Some(SlotKind::Approval),
            (Method::NoPrompt, None) if self.chip.present() => Some(SlotKind::NoPrompt),
            (Method::NoPrompt, None) => return Next::refuse(Refusal::Refused),
            _ => return Next::refuse(Refusal::BadRequest),
        };
        let name = if name.is_empty() { "Vault".to_string() } else { name };
        Next::approve(Action::Open, say::SET_UP, "", move |service, peer| {
            // Another setup of this account may have been approved and kept while this one waited
            if !matches!(service.account(peer.uid), Err(Refusal::NotSetUp)) {
                return Next::refuse(Refusal::Refused);
            }
            let (chip, kdf) = (service.chip.clone(), Kdf::fresh(service.config.log_n));
            Next::work(
                move || {
                    let under = match &secret {
                        Some(secret) => Some(slots::derive(secret, &kdf)?),
                        None => None,
                    };
                    let mut draft = Draft::new(&name, false, true, kdf, under.as_ref())?;
                    if let Some(kind) = slot {
                        draft.add_chip_slot(kind, chip.as_ref())?;
                    }
                    Ok(draft)
                },
                move |service, peer, draft: Result<Draft, seal::Unopened>| service.keep_draft(peer.uid, draft),
            )
        })
    }

    fn keep_draft(&mut self, uid: u32, draft: Result<Draft, seal::Unopened>) -> Next {
        match draft {
            Ok(draft) => {
                let text = draft.recovery.text();
                self.drafts.insert(uid, (draft, Instant::now()));
                Next::Answer(json!({"ok": true, "recovery": *text}))
            }
            Err(_) => Next::refuse(Refusal::Refused),
        }
    }

    /// The recovery key, typed back: only now is the vault kept, and open
    fn confirm(&mut self, uid: u32, typed: &[u8]) -> Next {
        // A lock, a sleep or the time it waited dropped the draft: whatever is typed now, nothing is there to keep, which is not a wrong key
        let Some((draft, _)) = self.drafts.get(&uid) else { return Next::Answer(json!({"ok": false, "why": "refused", "problem": "no-draft"})) };
        let typed = std::str::from_utf8(typed).ok().and_then(RecoveryKey::parse);
        let matches = typed.is_some_and(|typed| seal::same(typed.key().bytes(), draft.recovery.key().bytes()));
        if !matches {
            return Next::refuse(Refusal::Refused);
        }
        if self.outside.watch().is_err() {
            return Next::refuse(Refusal::Failed);
        }
        let Some((draft, _)) = self.drafts.remove(&uid) else { return Next::refuse(Refusal::Refused) };
        match draft.commit(&self.config.state, uid) {
            Ok(store) => {
                let answer = json!({"ok": true, "vault": store.id()});
                self.add_store(uid, store);
                Next::Answer(answer)
            }
            Err(trouble) => {
                self.unwatch_if_nothing();
                Next::trouble(trouble)
            }
        }
    }

    fn new_vault(&mut self, uid: u32, name: String, hidden: bool, secret: Secret) -> Next {
        let account = match self.account(uid) {
            Ok(account) => account,
            Err(why) => return Next::refuse(why),
        };
        if !slots::usable(&secret) {
            return no_secret(secret);
        }
        Next::approve(Action::Open, say::NEW_VAULT, "", move |_, _| {
            if !account.has_room() {
                return Next::refuse(Refusal::Refused);
            }
            let (kdf, records) = (account.file.kdf.clone(), account.records(Table::Password));
            Next::work(
                move || {
                    let under = slots::derive(&secret, &kdf)?;
                    // A password that opens another vault would shadow it: refused like any other unusable one
                    if !slots::search(&under, &records, Table::Password).is_empty() {
                        return Err(seal::Unopened);
                    }
                    Draft::new(&name, hidden, false, kdf, Some(&under))
                },
                move |service, peer, draft: Result<Draft, seal::Unopened>| service.keep_draft(peer.uid, draft),
            )
        })
    }

    // --- Opening -------------------------------------------------------------------------------------------------------

    fn unlock(&mut self, uid: u32, method: Method, slot: Option<String>, vault: Option<String>, secret: Option<Secret>) -> Next {
        let account = match self.account(uid) {
            Ok(account) => account,
            Err(why) => return Next::refuse(why),
        };
        let closes = self.closes_so_far(uid);
        match method {
            Method::Password => {
                let Some(secret) = secret else { return Next::refuse(Refusal::BadRequest) };
                // No password that can be set looks like this one, so there is nothing to guess and nothing to count
                if !slots::usable(&secret) {
                    return Next::refuse(Refusal::Refused);
                }
                // After a few wrong ones, each further try is put to the person first, which a program guessing cannot answer
                let gated = account.needs_approval();
                let (kdf, records) = (account.file.kdf.clone(), account.records(Table::Password));
                let try_it = move || {
                    // One derivation and every record, whatever there is to find
                    Next::work(
                        move || slots::derive(&secret, &kdf).map(|under| slots::search(&under, &records, Table::Password)).unwrap_or_default(),
                        move |service, peer, found: Vec<Found>| {
                            let account = match service.account(peer.uid) {
                                Ok(account) => account,
                                Err(why) => return Next::refuse(why),
                            };
                            if found.is_empty() {
                                account.note_failure();
                                return Next::refuse(Refusal::Refused);
                            }
                            // Only a right password the person approved gives the free tries back: a program with a vault of
                            // its own could otherwise open that one every fifth try
                            if gated {
                                account.note_success();
                            }
                            service.open_found(peer.uid, found.into_iter().map(|found| (found.vault, found.key)).collect(), closes)
                        },
                    )
                };
                if gated { Next::approve(Action::Open, say::OPEN, "", move |_, _| try_it()) } else { try_it() }
            }
            Method::Recovery => {
                let Some(secret) = secret else { return Next::refuse(Refusal::BadRequest) };
                // Not a guess worth counting: 256 random bits, and the way back in when guesses have run out
                let Some(typed) = std::str::from_utf8(&secret).ok().and_then(RecoveryKey::parse) else { return Next::refuse(Refusal::Refused) };
                let found = slots::search(&typed.key(), &account.records(Table::Recovery), Table::Recovery);
                if found.is_empty() {
                    return Next::refuse(Refusal::Refused);
                }
                account.note_success();
                self.open_found(uid, found.into_iter().map(|found| (found.vault, found.key)).collect(), closes)
            }
            Method::SecurityKey => {
                let (Some(slot), Some(secret)) = (slot, secret) else { return Next::refuse(Refusal::BadRequest) };
                let found = account.file.listed.iter().find_map(|vault| vault.slots.iter().find(|one| one.id == slot && one.kind == SlotKind::SecurityKey).map(|one| (vault.id.clone(), one)));
                let Some((vault, slot)) = found else { return Next::refuse(Refusal::Refused) };
                match slot.open_with_secret(&secret) {
                    Ok(key) => self.open_found(uid, vec![(vault, key)], closes),
                    Err(_) => Next::refuse(Refusal::Refused),
                }
            }
            Method::Approval | Method::NoPrompt => {
                let kind = if method == Method::Approval { SlotKind::Approval } else { SlotKind::NoPrompt };
                let found = account
                    .file
                    .listed
                    .iter()
                    .filter(|listed| vault.as_ref().is_none_or(|vault| *vault == listed.id))
                    .find_map(|listed| listed.slots.iter().find(|slot| slot.kind == kind).map(|slot| (listed.id.clone(), slot.clone())));
                let Some((vault, slot)) = found else { return Next::refuse(Refusal::Refused) };
                let release = move |service: &mut Service| {
                    let chip = service.chip.clone();
                    let closes = service.closes_so_far(uid);
                    Next::work(
                        move || slot.open_on_chip(chip.as_ref()),
                        move |service, peer, key: Result<Key, seal::Unopened>| match key {
                            Ok(key) => service.open_found(peer.uid, vec![(vault, key)], closes),
                            Err(_) => Next::refuse(Refusal::Refused),
                        },
                    )
                };
                if kind == SlotKind::NoPrompt { release(self) } else { Next::approve(Action::Open, say::OPEN, "", move |service, _| release(service)) }
            }
        }
    }

    fn add_store(&mut self, uid: u32, store: Store) {
        let account = self.account(uid).ok();
        let idle = account.as_ref().map_or(vaults::IDLE_MINUTES, |account| account.file.idle_minutes);
        let session = self.sessions.entry(uid).or_insert_with(|| Session {
            vaults: Vec::new(),
            approved: HashMap::new(),
            last_use: Instant::now(),
            idle: Duration::from_secs(u64::from(idle) * 60),
            used: account.as_ref().map_or(0, Account::disk_use),
        });
        session.vaults.retain(|open| open.id() != store.id());
        if crate::sync::is_connected(&store) {
            self.sync.opened(uid);
        }
        session.vaults.push(store);
        session.last_use = Instant::now();
    }

    fn unwatch_if_nothing(&mut self) {
        if self.sessions.is_empty() {
            self.outside.unwatch();
        }
    }

    /// Opens what a slot gave. Nothing opens if the screen locked or the machine slept while it was being worked out, or if
    /// nothing would say when to close it.
    fn open_found(&mut self, uid: u32, found: Vec<(String, Key)>, closes: u64) -> Next {
        if closes != self.closes_so_far(uid) {
            return Next::refuse(Refusal::Closed);
        }
        let account = match self.account(uid) {
            Ok(account) => account,
            Err(why) => return Next::refuse(why),
        };
        if self.outside.watch().is_err() {
            return Next::refuse(Refusal::Failed);
        }
        let mut opened = Vec::new();
        for (id, key) in found {
            match Store::open(&account.folder_of(&id), &id, key) {
                Ok(store) => {
                    opened.push(json!({"id": store.id(), "name": store.meta.name, "first": store.meta.first, "hidden": store.meta.hidden}));
                    self.add_store(uid, store);
                }
                Err(trouble) => {
                    self.unwatch_if_nothing();
                    return Next::trouble(trouble);
                }
            }
        }
        Next::Answer(json!({"ok": true, "opened": opened}))
    }

    // --- The ways in ---------------------------------------------------------------------------------------------------

    pub(crate) fn open_store(&mut self, uid: u32, vault: Option<&str>) -> Result<&mut Store, Refusal> {
        let session = self.sessions.get_mut(&uid).ok_or(Refusal::Closed)?;
        match vault {
            Some(vault) => session.vaults.iter_mut().find(|store| store.id() == vault).ok_or(Refusal::Refused),
            None => session.vaults.last_mut().ok_or(Refusal::Closed),
        }
    }

    /// The listed vault this slot is for, open, with its key to seal the slot with
    fn listed_key(&mut self, uid: u32, vault: Option<&str>) -> Result<(String, Key), Refusal> {
        let store = self.open_store(uid, vault)?;
        if store.meta.hidden {
            // It opens by password only: any other slot would need the service to know which vault to offer
            return Err(Refusal::Refused);
        }
        Ok((store.id().to_string(), store.key().copy()))
    }

    fn save_slot(&mut self, uid: u32, vault: &str, slot: Slot) -> Next {
        // The slot is sealed with a key taken when it was asked for: if the vault has closed since, that key is not to be used
        if !self.sessions.get(&uid).is_some_and(|session| session.vaults.iter().any(|store| store.id() == vault)) {
            return Next::refuse(Refusal::Closed);
        }
        let mut account = match self.account(uid) {
            Ok(account) => account,
            Err(why) => return Next::refuse(why),
        };
        let Some(listed) = account.listed_mut(vault) else { return Next::refuse(Refusal::Refused) };
        // One of each kind on the chip: a new one takes the old one's place
        if slot.kind != SlotKind::SecurityKey {
            listed.slots.retain(|old| old.kind != slot.kind);
        }
        if listed.slots.len() >= MAX_SLOTS {
            return Next::refuse(Refusal::Refused);
        }
        let id = slot.id.clone();
        listed.slots.push(slot);
        match account.save() {
            Ok(()) => {
                self.touch(uid);
                Next::Answer(json!({"ok": true, "slot": id}))
            }
            Err(trouble) => Next::trouble(trouble),
        }
    }

    fn add_security_key(&mut self, uid: u32, name: String, credential: String, salt: String, secret: Secret, vault: Option<String>) -> Next {
        let (vault, key) = match self.listed_key(uid, vault.as_deref()) {
            Ok(found) => found,
            Err(why) => return Next::refuse(why),
        };
        Next::approve(Action::Open, say::CHANGE_METHODS, "", move |service, peer| match Slot::for_security_key(&name, &credential, &salt, &secret, &key) {
            Ok(slot) => service.save_slot(peer.uid, &vault, slot),
            Err(_) => Next::refuse(Refusal::BadRequest),
        })
    }

    fn add_chip_slot(&mut self, uid: u32, kind: Method, vault: Option<String>) -> Next {
        let kind = match kind {
            Method::Approval => SlotKind::Approval,
            Method::NoPrompt => SlotKind::NoPrompt,
            _ => return Next::refuse(Refusal::BadRequest),
        };
        let (vault, key) = match self.listed_key(uid, vault.as_deref()) {
            Ok(found) => found,
            Err(why) => return Next::refuse(why),
        };
        Next::approve(Action::Open, say::CHANGE_METHODS, "", move |service, _| {
            let chip = service.chip.clone();
            Next::work(
                move || Slot::for_chip(kind, chip.as_ref(), &key),
                move |service, peer, slot: Result<Slot, seal::Unopened>| match slot {
                    Ok(slot) => service.save_slot(peer.uid, &vault, slot),
                    Err(_) => Next::refuse(Refusal::Refused),
                },
            )
        })
    }

    fn remove_slot(&mut self, slot: String) -> Next {
        Next::approve(Action::Open, say::CHANGE_METHODS, "", move |service, peer| {
            let mut account = match service.account(peer.uid) {
                Ok(account) => account,
                Err(why) => return Next::refuse(why),
            };
            let Some(listed) = account.file.listed.iter_mut().find(|listed| listed.slots.iter().any(|one| one.id == slot)) else { return Next::refuse(Refusal::Refused) };
            // The recovery key is not a way anyone means to use every day
            if !listed.password && listed.slots.len() == 1 {
                return Next::refuse(Refusal::Refused);
            }
            listed.slots.retain(|one| one.id != slot);
            match account.save() {
                Ok(()) => Next::ok(),
                Err(trouble) => Next::trouble(trouble),
            }
        })
    }

    fn background(&mut self, uid: u32, on: bool) -> Next {
        if !on {
            return Next::approve(Action::Open, say::CHANGE_METHODS, "", |service, peer| {
                service.drop_background(peer.uid);
                Next::ok()
            });
        }
        if !self.chip.present() {
            return Next::refuse(Refusal::Refused);
        }
        if self.first_open(uid).is_none() {
            return Next::refuse(Refusal::Closed);
        }
        Next::approve(Action::Open, say::CHANGE_METHODS, "", |service, peer| {
            let Some(store) = service.first_open(peer.uid) else { return Next::refuse(Refusal::Closed) };
            let key = match store.data_key(BACKGROUND, true) {
                Ok(Some(key)) => key,
                Ok(None) => return Next::refuse(Refusal::Refused),
                Err(trouble) => return Next::trouble(trouble),
            };
            let chip = service.chip.clone();
            let id = seal::random_hex(16);
            Next::work(
                move || {
                    let sealed = chip.seal(&slots::chip_name(&id), key.bytes())?;
                    Ok(Background { id, sealed: protocol::encode(&sealed) })
                },
                |service, peer, background: Result<Background, seal::Unopened>| {
                    let (Ok(background), Ok(mut account)) = (background, service.account(peer.uid)) else { return Next::refuse(Refusal::Refused) };
                    account.file.background = Some(background);
                    match account.save() {
                        Ok(()) => Next::ok(),
                        Err(trouble) => Next::trouble(trouble),
                    }
                },
            )
        })
    }

    fn change_password(&mut self, uid: u32, vault: Option<String>, current: Option<Secret>, secret: Secret) -> Next {
        let account = match self.account(uid) {
            Ok(account) => account,
            Err(why) => return Next::refuse(why),
        };
        let (id, key, had) = match self.open_store(uid, vault.as_deref()) {
            Ok(store) => (store.id().to_string(), store.key().copy(), store.meta.password.is_some()),
            Err(why) => return Next::refuse(why),
        };
        if !slots::usable(&secret) {
            return no_secret(secret);
        }
        if had && current.is_none() {
            return Next::refuse(Refusal::BadRequest);
        }
        // The one salt stays: a second would let the time a password takes say how many vaults there are
        let (kdf, records) = (account.file.kdf.clone(), account.records(Table::Password));
        let work_id = id.clone();
        let change = move || -> Result<(Option<String>, slots::Record), Refused> {
            let old = match &current {
                Some(current) => {
                    let under = slots::derive(current, &kdf).map_err(|_| Refused::Current)?;
                    let found = slots::search(&under, &records, Table::Password);
                    Some(found.into_iter().find(|found| found.vault == work_id).ok_or(Refused::Current)?.record)
                }
                None => None,
            };
            let under = slots::derive(&secret, &kdf).map_err(|_| Refused::New)?;
            if slots::search(&under, &records, Table::Password).iter().any(|found| found.vault != work_id) {
                return Err(Refused::New);
            }
            let record = slots::seal_record(&under, &work_id, &key, Table::Password).ok_or(Refused::New)?;
            Ok((old, record))
        };
        let apply = move |service: &mut Service, peer: &Rc<Peer>, done: Result<(Option<String>, slots::Record), Refused>| {
            // The vault is still open, or nothing is written: a lock that came meanwhile has closed it
            if service.open_store(peer.uid, Some(&id)).is_err() {
                return Next::refuse(Refusal::Closed);
            }
            let mut account = match service.account(peer.uid) {
                Ok(account) => account,
                Err(why) => return Next::refuse(why),
            };
            let (old, record) = match done {
                Ok(done) => done,
                Err(Refused::Current) => {
                    account.note_failure();
                    return Next::refuse(Refusal::Refused);
                }
                Err(Refused::New) => {
                    account.note_failure();
                    return Next::refuse(Refusal::Refused);
                }
            };
            let nonce = account.set_password_record(old.as_deref(), &record);
            if let Some(listed) = account.listed_mut(&id) {
                listed.password = true;
            }
            if let Err(trouble) = account.save() {
                return Next::trouble(trouble);
            }
            account.note_success();
            let Ok(store) = service.open_store(peer.uid, Some(&id)) else { return Next::refuse(Refusal::Closed) };
            store.meta.password = Some(nonce);
            match store.write_meta() {
                Ok(()) => {
                    service.touch(peer.uid);
                    Next::ok()
                }
                Err(trouble) => Next::trouble(trouble),
            }
        };
        // Always put to the person, a current password typed or not: it is a way to ask whether a password opens another vault,
        // and a program can type anything. The person is asked once, before any derivation.
        Next::approve(Action::Open, say::CHANGE_METHODS, "", move |_, _| Next::work(change, apply))
    }

    fn set_idle(&mut self, uid: u32, minutes: u32) -> Next {
        let account = match self.account(uid) {
            Ok(account) => account,
            Err(why) => return Next::refuse(why),
        };
        // Lowering it is always safe; keeping the key in memory for longer is the person's to say
        if minutes > account.file.idle_minutes {
            return Next::approve(Action::Open, say::IDLE_LONGER, "", move |service, peer| service.apply_idle(peer.uid, minutes));
        }
        self.apply_idle(uid, minutes)
    }

    fn apply_idle(&mut self, uid: u32, minutes: u32) -> Next {
        let mut account = match self.account(uid) {
            Ok(account) => account,
            Err(why) => return Next::refuse(why),
        };
        account.file.idle_minutes = minutes;
        if let Err(trouble) = account.save() {
            return Next::trouble(trouble);
        }
        if let Some(session) = self.sessions.get_mut(&uid) {
            session.idle = Duration::from_secs(u64::from(minutes) * 60);
        }
        self.touch(uid);
        Next::ok()
    }

    // --- Vaults --------------------------------------------------------------------------------------------------------

    fn rename(&mut self, uid: u32, vault: Option<&str>, name: String) -> Next {
        let store = match self.open_store(uid, vault) {
            Ok(store) => store,
            Err(why) => return Next::refuse(why),
        };
        store.meta.name = name;
        match store.write_meta() {
            Ok(()) => {
                self.touch(uid);
                Next::ok()
            }
            Err(trouble) => Next::trouble(trouble),
        }
    }

    /// Removes only the one that is open. The first holds what the desktop relies on, and goes only with `forget-everything`.
    fn remove_this(&mut self, uid: u32, vault: Option<&str>) -> Next {
        let (id, name, first) = match self.open_store(uid, vault) {
            Ok(store) => (store.id().to_string(), store.meta.name.clone(), store.meta.first),
            Err(why) => return Next::refuse(why),
        };
        if first {
            return Next::refuse(Refusal::Refused);
        }
        Next::approve(Action::Use, say::REMOVE_VAULT, &name, move |service, peer| {
            let uid = peer.uid;
            let Some(session) = service.sessions.get_mut(&uid) else { return Next::refuse(Refusal::Closed) };
            let Some(at) = session.vaults.iter().position(|store| store.id() == id) else { return Next::refuse(Refusal::Closed) };
            let store = session.vaults.remove(at);
            // What was approved in it goes with it
            let prefix = format!("{id}/");
            session.approved.retain(|approved, _| !approved.starts_with(&prefix));
            let empty = session.vaults.is_empty();
            if empty {
                service.close(uid);
            }
            let mut account = match service.account(uid) {
                Ok(account) => account,
                Err(why) => return Next::refuse(why),
            };
            match account.remove_vault(&store.meta, &id) {
                Ok(()) => Next::ok(),
                Err(trouble) => Next::trouble(trouble),
            }
        })
    }

    /// Every vault of the account, including any nobody can see. Works closed: it is what a person does who lost every way in.
    fn forget_everything(&mut self) -> Next {
        Next::approve(Action::Use, say::FORGET_ALL, "", |service, peer| {
            service.close(peer.uid);
            match Account::forget_everything(&service.config.state, peer.uid) {
                Ok(()) => Next::ok(),
                Err(trouble) => Next::trouble(trouble),
            }
        })
    }

    // --- Items ---------------------------------------------------------------------------------------------------------

    /// Every open vault of this account, the one opened last first
    pub(crate) fn open_stores(&mut self, uid: u32) -> impl Iterator<Item = &mut Store> {
        self.sessions.get_mut(&uid).into_iter().flat_map(|session| session.vaults.iter_mut().rev())
    }

    /// Where an item is, from the vault opened last (or only in `within`): a name is looked for in the vaults open now
    pub(crate) fn locate(&self, uid: u32, kind: &str, id: &str, within: Option<&str>) -> Result<(String, Approval, String), Refusal> {
        let session = self.sessions.get(&uid).ok_or(Refusal::Closed)?;
        session
            .vaults
            .iter()
            .rev()
            .filter(|store| within.is_none_or(|within| store.id() == within))
            .find_map(|store| store.entry(kind, id).map(|entry| (store.id().to_string(), entry.approval, entry.name.clone())))
            .ok_or(Refusal::Refused)
    }

    pub fn store_of(&mut self, uid: u32, vault: &str) -> Result<&mut Store, Refusal> {
        self.sessions.get_mut(&uid).ok_or(Refusal::Closed)?.vaults.iter_mut().find(|store| store.id() == vault).ok_or(Refusal::Closed)
    }

    /// Writes through the account's quota, for whoever writes an item: the disk is shared, and a program of one account must not
    /// fill it. `change` does the write in the store; what the files took before and after is what the account has used, and
    /// `extra` is about what a new value will take.
    pub fn write_in<T>(&mut self, uid: u32, vault: &str, kind: &str, id: &str, extra: usize, change: impl FnOnce(&mut Store) -> Result<T, Trouble>) -> Result<T, Trouble> {
        self.write_many_in(uid, vault, &[(kind, id)], extra, change)
    }

    /// The same for a write of several items at once (an import, a replace): the files of all of them, and the index and the
    /// meta once
    pub fn write_many_in<T>(&mut self, uid: u32, vault: &str, items: &[(&str, &str)], extra: usize, change: impl FnOnce(&mut Store) -> Result<T, Trouble>) -> Result<T, Trouble> {
        let session = self.sessions.get_mut(&uid).ok_or(Trouble::NotFound)?;
        let store = session.vaults.iter_mut().find(|store| store.id() == vault).ok_or(Trouble::NotFound)?;
        let before = store.footprint_of(items);
        // The index and the meta are written again too, and may have grown: room is left for them
        if session.used.saturating_sub(before) + extra as u64 + SPARE > vaults::QUOTA {
            return Err(Trouble::Full);
        }
        let done = change(store)?;
        session.used = session.used.saturating_sub(before) + store.footprint_of(items);
        // Sync hears of every write to the first vault that it carries, from a program's `put` as from a plug (authenticator, codes,
        // keys, certificates), and of no other vault's
        if store.meta.first && items.iter().any(|(kind, _)| kinds::may_sync(kind)) {
            self.sync.changed(uid);
        }
        Ok(done)
    }

    /// An approval was for that item: one taken away is not to be remembered for a new one by the same name
    pub(crate) fn forget_approved(&mut self, uid: u32, vault: &str, kind: &str, id: &str) {
        if let Some(session) = self.sessions.get_mut(&uid) {
            session.approved.remove(&format!("{vault}/{kind}.{id}"));
            // And the yes with a window of its own that `with_window` keeps apart
            session.approved.remove(&format!("{vault}/{kind}.{id}#window"));
        }
    }

    /// Goes on to `then` once the item's setting is satisfied: `force` asks whatever the setting says. For the `items`,
    /// `keys` and `pki` modules too (`plugs.rs`).
    #[allow(clippy::too_many_arguments)]
    pub fn with_approval(&mut self, uid: u32, kind: &str, id: &str, action: Action, message: &'static str, force: bool, then: impl FnOnce(&mut Service, &str) -> Next + 'static) -> Next {
        self.asking(uid, None, kind, id, action, message, force, "", None, then)
    }

    /// `with_approval` for what a yes should not last until the vault closes: an SSH key's `While open` lasts a `Reuse::For` of its own,
    /// kept apart from every other yes to the same item, or does not last at all. `detail` is the sentence's `$(detail)`.
    #[allow(clippy::too_many_arguments)]
    pub fn with_window(&mut self, uid: u32, kind: &str, id: &str, action: Action, message: &'static str, detail: &str, reuse: Reuse, then: impl FnOnce(&mut Service, &str) -> Next + 'static) -> Next {
        self.asking(uid, None, kind, id, action, message, false, detail, Some(reuse), then)
    }

    #[allow(clippy::too_many_arguments)]
    /// `within` limits the item to one vault
    fn asking(&mut self, uid: u32, within: Option<&str>, kind: &str, id: &str, action: Action, message: &'static str, force: bool, detail: &str, reuse: Option<Reuse>, then: impl FnOnce(&mut Service, &str) -> Next + 'static) -> Next {
        let (vault, approval, name) = match self.locate(uid, kind, id, within) {
            Ok(found) => found,
            Err(why) => return Next::refuse(why),
        };
        // A yes with a window is kept under a key of its own: a yes to anything else on the item must not end it, nor outlast it
        let remembered = if reuse.is_some() { format!("{vault}/{kind}.{id}#window") } else { format!("{vault}/{kind}.{id}") };
        let now = Instant::now();
        let approved = !matches!(reuse, Some(Reuse::Never)) && self.sessions.get(&uid).and_then(|session| session.approved.get(&remembered)).is_some_and(|until| until.is_none_or(|until| now < until));
        let ask = force
            || match approval {
                Approval::Ask => true,
                Approval::WhileOpen => !approved,
                Approval::Never => false,
            };
        if !ask {
            return then(self, &vault);
        }
        Next::approve_with(action, message, &name, detail, move |service, peer| {
            // What was asked by force is a yes to that, and to nothing after it
            if approval == Approval::WhileOpen
                && !force
                && !matches!(reuse, Some(Reuse::Never))
                && let Some(session) = service.sessions.get_mut(&peer.uid)
            {
                let until = match reuse {
                    Some(Reuse::For(window)) => Some(Instant::now() + window),
                    _ => None,
                };
                session.approved.insert(remembered, until);
            }
            // The person answered: that is use
            service.touch(peer.uid);
            then(service, &vault)
        })
    }

    /// What the open vaults list of this kind, last opened first, for what speaks for the service rather than on its socket: no
    /// approval, and not a use, so that asking cannot keep a vault from closing
    pub fn listed(&self, uid: u32, kind: &str) -> Vec<&Entry> {
        let Some(session) = self.sessions.get(&uid) else { return Vec::new() };
        session.vaults.iter().rev().flat_map(|store| store.entries().filter(|entry| entry.kind == kind)).collect()
    }

    /// The `(vault, id)` of every item of this kind that a program of the account could read now with no question: set to never ask,
    /// or asked once while the vault is open and said yes to. What compares only these tells a program nothing it could not read.
    pub(crate) fn readable_unasked(&self, uid: u32, kind: &str) -> Vec<(String, String)> {
        let Some(session) = self.sessions.get(&uid) else { return Vec::new() };
        let now = Instant::now();
        let mut found = Vec::new();
        for store in &session.vaults {
            for entry in store.entries().filter(|entry| entry.kind == kind) {
                let remembered = || session.approved.get(&format!("{}/{kind}.{}", store.id(), entry.id)).is_some_and(|until| until.is_none_or(|until| now < until));
                if entry.approval == Approval::Never || (entry.approval == Approval::WhileOpen && remembered()) {
                    found.push((store.id().to_string(), entry.id.clone()));
                }
            }
        }
        found
    }

    /// The first vault, if it is open
    pub fn first_vault(&mut self, uid: u32) -> Option<String> {
        self.first_open(uid).map(|store| store.id().to_string())
    }

    #[allow(clippy::too_many_arguments)]
    fn put(&mut self, uid: u32, kind: String, id: Option<String>, name: String, meta: Value, approval: Option<Approval>, value: Value, vault: Option<&str>) -> Next {
        if service_only(&kind) {
            return Next::refuse(Refusal::Refused);
        }
        // A logo is set and read without a question, so it holds what the window keeps of one and nothing else: any program could
        // otherwise keep whatever it liked in the vault unasked
        if kind == protocol::KIND_LOGO && !logo_shaped(&name, &meta, &value) {
            return Next::refuse(Refusal::BadRequest);
        }
        let target = match self.open_store(uid, vault) {
            Ok(store) => store.id().to_string(),
            Err(why) => return Next::refuse(why),
        };
        let existing = match &id {
            Some(id) => match self.locate(uid, &kind, id, Some(&target)) {
                Ok((_, old, _)) => Some(old),
                // Not in this vault but in another: writing here would shadow it for `get`, and write over its setting
                Err(_) if self.locate(uid, &kind, id, None).is_ok() => return Next::refuse(Refusal::Refused),
                Err(_) => None,
            },
            None => None,
        };
        let id = id.unwrap_or_else(|| seal::random_hex(8));
        // A logo is a picture of a public site: it is kept at Never, which asks nothing to set and nothing to read, whatever was said
        let logo = kind == protocol::KIND_LOGO;
        let setting = if logo { Approval::Never } else { approval.or(existing).unwrap_or(Approval::WhileOpen) };
        let keep = move |service: &mut Service, uid: u32, target: &str, kind: &str, id: &str, name: &str, meta: &Value, value: &Value| -> Next {
            let extra = value.to_string().len() + name.len() + meta.to_string().len();
            match service.write_in(uid, target, kind, id, extra, |store| store.put(kind, id, name, meta, setting, value)) {
                Ok(entry) => {
                    service.touch(uid);
                    Next::Answer(json!({"ok": true, "kind": entry.kind, "id": entry.id, "vault": target}))
                }
                Err(trouble) => Next::trouble(trouble),
            }
        };
        let lowers = |old: Approval| rank(setting) < rank(old);
        match existing {
            // A new item that never asks is only set after the warning, and a person says so
            None if setting == Approval::Never && !logo => {
                let shown = name.clone();
                Next::approve(Action::Use, say::LOOSEN_ITEM, &shown, move |service, peer| keep(service, peer.uid, &target, &kind, &id, &name, &meta, &value))
            }
            None => keep(self, uid, &target, &kind, &id, &name, &meta, &value),
            // Changing an item is using it; and lowering how often it asks always asks
            Some(old) => {
                let force = lowers(old);
                let message = if force { say::LOOSEN_ITEM } else { say::CHANGE_ITEM };
                let (kind_there, id_there, within) = (kind.clone(), id.clone(), target.clone());
                self.asking(uid, Some(&within), &kind_there, &id_there, Action::Use, message, force, "", None, move |service, _| keep(service, uid, &target, &kind, &id, &name, &meta, &value))
            }
        }
    }

    fn get(&mut self, uid: u32, kind: String, id: String) -> Next {
        if service_only(&kind) {
            return if self.sessions.contains_key(&uid) { Next::refuse(Refusal::Refused) } else { self.closed_or_not_set_up(uid) };
        }
        if !self.sessions.contains_key(&uid) {
            return self.closed_or_not_set_up(uid);
        }
        let (kind_there, id_there) = (kind.clone(), id.clone());
        self.with_approval(uid, &kind_there, &id_there, Action::Use, say::USE_ITEM, false, move |service, vault| match service.store_of(uid, vault) {
            Ok(store) => match store.get(&kind, &id) {
                Ok(value) => {
                    service.touch(uid);
                    if kind == KIND_PASSWORD {
                        return Next::Answer(json!({"ok": true, "kind": kind, "id": id, "vault": vault, "value": value, "copy": "secret"}));
                    }
                    Next::Answer(json!({"ok": true, "kind": kind, "id": id, "vault": vault, "value": value}))
                }
                Err(trouble) => Next::trouble(trouble),
            },
            Err(why) => Next::refuse(why),
        })
    }

    fn closed_or_not_set_up(&self, uid: u32) -> Next {
        match self.account(uid) {
            Ok(_) => Next::refuse(Refusal::Closed),
            Err(why) => Next::refuse(why),
        }
    }

    /// Reading the list of names needs an open vault and no approval
    pub(crate) fn list(&mut self, uid: u32, kind: Option<&str>) -> Next {
        let Some(session) = self.sessions.get(&uid) else { return self.closed_or_not_set_up(uid) };
        let mut items = Vec::new();
        for store in session.vaults.iter().rev() {
            for entry in store.entries().filter(|entry| kind.is_none_or(|kind| kind == entry.kind)) {
                items.push(json!({"vault": store.id(), "kind": entry.kind, "id": entry.id, "name": entry.name, "meta": entry.meta, "approval": entry.approval.name(), "changed": entry.changed}));
            }
        }
        self.touch(uid);
        Next::Answer(json!({"ok": true, "items": items}))
    }

    /// Removing always asks, whatever the setting: nothing else can lose it for good
    fn remove(&mut self, uid: u32, kind: String, id: String) -> Next {
        // What the service keeps for itself is not removed by a program: the connection to a server and its links would go with one
        // ordinary approval that does not say so
        if crate::sync::owns(&kind) {
            return if self.sessions.contains_key(&uid) { Next::refuse(Refusal::Refused) } else { self.closed_or_not_set_up(uid) };
        }
        if !self.sessions.contains_key(&uid) {
            return self.closed_or_not_set_up(uid);
        }
        let (kind_there, id_there) = (kind.clone(), id.clone());
        self.with_approval(uid, &kind_there, &id_there, Action::Use, say::REMOVE_ITEM, true, move |service, vault| match service.write_in(uid, vault, &kind, &id, 0, |store| store.remove(&kind, &id)) {
            Ok(()) => {
                // An approval was for that item: a new one by the same name is another
                service.forget_approved(uid, vault, &kind, &id);
                if kind == DATA_KEY && id == BACKGROUND {
                    service.drop_background(uid);
                }
                service.touch(uid);
                Next::ok()
            }
            Err(trouble) => Next::trouble(trouble),
        })
    }

    fn set_approval(&mut self, uid: u32, kind: String, id: String, approval: Approval) -> Next {
        if protocol::NO_SETTING.contains(&kind.as_str()) {
            return Next::refuse(Refusal::Refused);
        }
        if !self.sessions.contains_key(&uid) {
            return self.closed_or_not_set_up(uid);
        }
        let (kind_there, id_there) = (kind.clone(), id.clone());
        self.with_approval(uid, &kind_there, &id_there, Action::Use, say::LOOSEN_ITEM, true, move |service, vault| match service.write_in(uid, vault, &kind, &id, 0, |store| store.set_approval(&kind, &id, approval)) {
            Ok(()) => {
                service.forget_approved(uid, vault, &kind, &id);
                service.touch(uid);
                Next::ok()
            }
            Err(trouble) => Next::trouble(trouble),
        })
    }
}

fn rank(approval: Approval) -> u8 {
    match approval {
        Approval::Never => 0,
        Approval::WhileOpen => 1,
        Approval::Ask => 2,
    }
}

/// Which password of a change was no good: only a wrong current one counts as a guess
enum Refused {
    Current,
    New,
}

/// The biggest picture the window keeps of a site, a PNG of 64 pixels a side, with room to spare
const MOST_LOGO: usize = 64 * 1024;

/// What the window keeps of a site's logo (`apps/vault/src/logos.rs`): the host as its name, whether there is a picture, and the
/// picture as a PNG, or nothing for a site that had none
fn logo_shaped(name: &str, meta: &Value, value: &Value) -> bool {
    let host = !name.is_empty()
        && name.len() <= 253
        && name.split('.').all(|part| !part.is_empty() && part.len() <= 63 && part.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'));
    let Some(meta) = meta.as_object() else { return false };
    let Some(value) = value.as_object() else { return false };
    let said = meta.get("picture").and_then(Value::as_bool).filter(|_| meta.len() == 1);
    let picture = match (said, value.len(), value.get("picture")) {
        (Some(false), 0, None) => true,
        (Some(true), 1, Some(Value::String(text))) => base64::engine::general_purpose::STANDARD
            .decode(text)
            .is_ok_and(|png| png.len() <= MOST_LOGO && png.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])),
        _ => false,
    };
    host && picture
}
