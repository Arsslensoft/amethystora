//! polkit and logind, over the system bus (`bus.rs`). The connection exists only while a vault is open or a person is being
//! asked, and if it ends, every vault closes: nothing would tell it to otherwise.
//!
//! - **Closing:** logind's `PrepareForSleep`, each session's `Lock` and `LockedHint` (seconds apart, and only the second comes when
//!   the shell locks by itself), and `SessionRemoved`, after which the sessions left say whose are over. A delay inhibitor, while a
//!   vault is open, holds the sleep until the key is gone.
//! - **Asking:** polkit's `CheckAuthorization` for the process that asked, after every session of the account has said it is not
//!   locked. The service is the action's `org.freedesktop.policykit.owner`, which lets it ask for another process and pass
//!   `polkit.message`, with the program and the item as details, which polkit fills in as pkexec's `$(program)` is.
//! - **Who said it:** anyone on the system bus can send this connection a signal of its own, and a reply comes only from where
//!   the call went. So a signal counts only from logind's unique name, a reply only from the name the call was for, both as the
//!   bus says they are now (`GetNameOwner`, `NameOwnerChanged`), and a message whose header does not read is passed over.

use std::collections::HashMap;
use std::os::fd::{OwnedFd, RawFd};
use std::path::PathBuf;

use crate::bus::{self, Bus, Message, NotAMessage, Reader, Writer};
use crate::outside::{Ask, CannotWatch, Event, Outside};
use crate::peer::{clean, Peer};

const LOGIN1: &str = "org.freedesktop.login1";
const LOGIN1_PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION: &str = "org.freedesktop.login1.Session";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const POLKIT: &str = "org.freedesktop.PolicyKit1";
const POLKIT_PATH: &str = "/org/freedesktop/PolicyKit1/Authority";
const AUTHORITY: &str = "org.freedesktop.PolicyKit1.Authority";
const DBUS: &str = "org.freedesktop.DBus";
const DBUS_PATH: &str = "/org/freedesktop/DBus";

enum Waiting {
    Approval(u64),
    /// The first step of an approval: which sessions the account has
    SessionsOf(u64),
    /// The second: whether one of them is locked
    LockedOf(u64),
    /// Whose session locked
    SessionUser,
    /// Which sessions are left
    Sessions,
    /// What class of session one of them is, and whose
    ClassOf(u32),
    Inhibitor,
    /// Who has this well-known name now
    Owner(&'static str),
    /// An answer that means nothing to anyone, like a cancellation's or a match's
    Nothing,
}

impl Waiting {
    /// Whose answer this has to be
    fn from(&self) -> &'static str {
        match self {
            Waiting::Approval(_) => POLKIT,
            Waiting::Owner(_) | Waiting::Nothing => DBUS,
            _ => LOGIN1,
        }
    }
}

/// An approval on its way to polkit, until the account's sessions have answered
struct Asking {
    uid: u32,
    /// None when only the lock is asked about, and nobody is prompted
    body: Option<Vec<u8>>,
    pidfd: Option<OwnedFd>,
    sessions_left: usize,
}

/// Working out who is still logged in, once a session has ended. An account's `manager` session outlasts its logout, and lasts for
/// ever with lingering, so it does not count.
struct Census {
    /// The accounts with a session of a person's
    people: Vec<u32>,
    answers_left: usize,
    /// Another session ended meanwhile: ask again after this
    again: bool,
}

/// Where logind keeps what each account has, one file named by the number: world-readable, a `SESSIONS=` line while the
/// account has a session, and `STATE=lingering` while it has none and the owner let its programs go on. A test's private bus
/// has a folder of its own, named by the service's environment, which only whoever starts the service can set.
fn users_folder() -> PathBuf {
    std::env::var_os("AMETHYSTORA_VAULT_LOGIND_USERS").map_or_else(|| PathBuf::from("/run/systemd/users"), PathBuf::from)
}

fn user_state(uid: u32) -> String {
    std::fs::read_to_string(users_folder().join(uid.to_string())).unwrap_or_default()
}

/// And one file for each session, named by its id, with its `CLASS=`, `STATE=` and `UID=`
fn sessions_folder() -> PathBuf {
    std::env::var_os("AMETHYSTORA_VAULT_LOGIND_SESSIONS").map_or_else(|| PathBuf::from("/run/systemd/sessions"), PathBuf::from)
}

/// Whether this session of the account is a person's: of a `user` class (the census above counts the same), and not on its way out. The
/// account's `SESSIONS=` line lists its `manager` session as well, which a lingering account keeps for ever with nobody at the
/// machine, so that line alone says nothing about a person.
fn is_a_persons_session(uid: u32, id: &str) -> bool {
    if id.is_empty() || id.len() > 32 || !id.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return false;
    }
    let Ok(state) = std::fs::read_to_string(sessions_folder().join(id)) else { return false };
    let value = |key: &str| state.lines().find_map(|line| line.strip_prefix(key));
    value("UID=").is_some_and(|owner| owner == uid.to_string()) && value("CLASS=").is_some_and(|class| class.starts_with("user")) && value("STATE=") != Some("closing")
}

#[derive(Default)]
pub struct Desktop {
    bus: Option<Bus>,
    waiting: HashMap<u32, Waiting>,
    asking: HashMap<u64, Asking>,
    /// logind's signals are coming on this connection
    subscribed: bool,
    watching: bool,
    /// Holds a sleep until it is closed
    inhibitor: Option<OwnedFd>,
    /// Approvals not answered yet
    approvals: usize,
    /// Asking which sessions are left and of what class
    census: Option<Census>,
    /// The unique names of logind and polkit, as the bus last said
    owners: HashMap<&'static str, String>,
    events: Vec<Event>,
}

impl Desktop {
    pub fn new() -> Desktop {
        Desktop::default()
    }

    /// The connection, hearing logind: from the first approval or open vault to the last
    fn connected(&mut self) -> Option<&mut Bus> {
        if self.bus.is_none() {
            self.bus = Bus::connect(&bus::system_bus()).ok();
            self.subscribed = false;
            self.owners.clear();
        }
        if !self.subscribed {
            let bus = self.bus.as_mut()?;
            let mut serials = Vec::new();
            let mut owners = Vec::new();
            for name in [LOGIN1, POLKIT] {
                let mut body = Writer::default();
                body.string(&format!("type='signal',sender='{DBUS}',interface='{DBUS}',member='NameOwnerChanged',arg0='{name}'"));
                serials.push(bus.call(DBUS, DBUS_PATH, DBUS, "AddMatch", "s", &body.bytes));
                let mut body = Writer::default();
                body.string(name);
                owners.push((bus.call(DBUS, DBUS_PATH, DBUS, "GetNameOwner", "s", &body.bytes), name));
            }
            for rule in [
                format!("type='signal',sender='{LOGIN1}',interface='{MANAGER}',member='PrepareForSleep'"),
                format!("type='signal',sender='{LOGIN1}',interface='{MANAGER}',member='SessionRemoved'"),
                format!("type='signal',sender='{LOGIN1}',interface='{SESSION}',member='Lock'"),
                format!("type='signal',sender='{LOGIN1}',interface='{PROPERTIES}',member='PropertiesChanged',arg0='{SESSION}'"),
            ] {
                let mut body = Writer::default();
                body.string(&rule);
                serials.push(bus.call(DBUS, DBUS_PATH, DBUS, "AddMatch", "s", &body.bytes));
            }
            for serial in serials {
                self.waiting.insert(serial, Waiting::Nothing);
            }
            for (serial, name) in owners {
                self.waiting.insert(serial, Waiting::Owner(name));
            }
            self.subscribed = true;
        }
        self.bus.as_mut()
    }

    /// The bus went: every approval is a no, and every vault closes
    fn lost(&mut self) {
        self.bus = None;
        self.owners.clear();
        self.census = None;
        self.subscribed = false;
        self.inhibitor = None;
        self.approvals = 0;
        for (_, waiting) in self.waiting.drain() {
            if let Waiting::Approval(token) = waiting {
                self.events.push(Event::Approved(token, false));
            }
        }
        for (token, _) in self.asking.drain() {
            self.events.push(Event::Approved(token, false));
        }
        if self.watching {
            self.watching = false;
            self.events.push(Event::Lost);
        }
    }

    fn flush(&mut self) {
        if let Some(bus) = &mut self.bus
            && bus.flush().is_err()
        {
            self.lost();
        }
    }

    fn drop_bus_if_idle(&mut self) {
        if !self.watching && self.approvals == 0 {
            self.bus = None;
            self.owners.clear();
            self.census = None;
            self.subscribed = false;
            self.waiting.clear();
            self.asking.clear();
        }
    }

    fn signal(&mut self, message: &Message) {
        let mut reader = Reader::new(&message.body);
        if message.sender == DBUS && message.interface == DBUS && message.member == "NameOwnerChanged" && message.signature == "sss" {
            let (Some(name), Some(old), Some(new)) = (reader.string(), reader.string(), reader.string()) else { return };
            let Some(name) = [LOGIN1, POLKIT].into_iter().find(|known| *known == name) else { return };
            if new.is_empty() {
                self.owners.remove(name);
            } else {
                self.owners.insert(name, new.to_string());
            }
            // logind started again has forgotten the inhibitor and may have missed a lock: close, failing safe
            if name == LOGIN1 && !old.is_empty() && self.watching {
                self.watching = false;
                self.inhibitor = None;
                self.events.push(Event::Lost);
            }
            return;
        }
        // Only logind's own: a signal can be sent to this connection by anyone on the bus
        if self.owners.get(LOGIN1) != Some(&message.sender) {
            return;
        }
        match (message.interface.as_str(), message.member.as_str()) {
            (MANAGER, "PrepareForSleep") if message.signature == "b" => {
                if reader.bool() == Some(true) {
                    self.events.push(Event::Sleeping);
                }
            }
            (MANAGER, "SessionRemoved") => {
                match &mut self.census {
                    Some(census) => census.again = true,
                    None => self.start_census(),
                }
            }
            (SESSION, "Lock") => self.ask_whose(&message.path),
            (PROPERTIES, "PropertiesChanged") if message.signature == "sa{sv}as" && locked_hint(&message.body) == Some(true) => self.ask_whose(&message.path),
            _ => {}
        }
    }

    /// Whose session this is that locked. A session of `manager` class never locks, so any one of an account's locking is the
    /// person's: no waiting for the others.
    fn ask_whose(&mut self, path: &str) {
        let mut body = Writer::default();
        body.string(SESSION);
        body.string("User");
        let Some(bus) = self.bus.as_mut() else { return };
        let serial = bus.call(LOGIN1, path, PROPERTIES, "Get", "ss", &body.bytes);
        self.waiting.insert(serial, Waiting::SessionUser);
    }

    fn start_census(&mut self) {
        let Some(bus) = self.bus.as_mut() else { return };
        let serial = bus.call(LOGIN1, LOGIN1_PATH, MANAGER, "ListSessions", "", &[]);
        self.waiting.insert(serial, Waiting::Sessions);
        self.census = Some(Census { people: Vec::new(), answers_left: 1, again: false });
    }

    fn census_answered(&mut self) {
        let Some(census) = &mut self.census else { return };
        census.answers_left = census.answers_left.saturating_sub(1);
        if census.answers_left > 0 {
            return;
        }
        let Some(census) = self.census.take() else { return };
        if census.again {
            self.start_census();
        } else {
            self.events.push(Event::SessionsLeft(census.people));
        }
    }

    /// One approval goes no further: nothing was put to anyone
    fn not_asked(&mut self, token: u64) {
        if self.asking.remove(&token).is_some() {
            self.approvals = self.approvals.saturating_sub(1);
            self.events.push(Event::NotAsked(token));
            self.drop_bus_if_idle();
        }
    }

    /// Every session has said it is not locked: now the person is asked, or, if that was all, the answer is yes
    fn put_to_polkit(&mut self, token: u64) {
        let Some(asking) = self.asking.remove(&token) else { return };
        let Some(body) = asking.body else {
            self.approvals = self.approvals.saturating_sub(1);
            self.events.push(Event::Approved(token, true));
            return self.drop_bus_if_idle();
        };
        let Some(bus) = self.bus.as_mut() else { return };
        let serial = bus.call_with_fd(POLKIT, POLKIT_PATH, AUTHORITY, "CheckAuthorization", "(sa{sv})sa{ss}us", &body, asking.pidfd);
        self.waiting.insert(serial, Waiting::Approval(token));
    }

    /// Asks logind first whether any session of the account is locked, then, with a body, polkit
    fn ask(&mut self, token: u64, uid: u32, body: Option<Vec<u8>>, pidfd: Option<OwnedFd>) {
        let Some(bus) = self.connected() else {
            // No bus: a prompt cannot be put to anyone, which is a no. Whether the screen is locked is not asked either, and the
            // opening that follows finds nothing to tell it when to close, and says so.
            self.events.push(Event::Approved(token, body.is_none()));
            return;
        };
        let serial = bus.call(LOGIN1, LOGIN1_PATH, MANAGER, "ListSessions", "", &[]);
        self.waiting.insert(serial, Waiting::SessionsOf(token));
        self.asking.insert(token, Asking { uid, body, pidfd, sessions_left: 0 });
        self.approvals += 1;
        self.flush();
    }

    fn answer(&mut self, message: Message) {
        let Some(serial) = message.reply_to else { return };
        let Some(from) = self.waiting.get(&serial).map(Waiting::from) else { return };
        // An answer from anyone else is nobody's: the call stays waiting for its own. The bus itself answers with an error for a
        // name that is not there, and nobody else can say they are the bus.
        let from_bus = message.sender == DBUS;
        let sender_ok = if from == DBUS { from_bus } else { self.owners.get(from) == Some(&message.sender) || (from_bus && message.kind == bus::ERROR) };
        if !sender_ok {
            return;
        }
        let Some(waiting) = self.waiting.remove(&serial) else { return };
        let good = message.kind == bus::METHOD_RETURN;
        match waiting {
            Waiting::Owner(name) => {
                match Reader::new(&message.body).string().filter(|_| good && message.signature == "s") {
                    Some(owner) => self.owners.insert(name, owner.to_string()),
                    None => self.owners.remove(name),
                };
            }
            Waiting::Approval(token) => {
                self.approvals = self.approvals.saturating_sub(1);
                let yes = good && message.signature == "(bba{ss})" && {
                    let mut reader = Reader::new(&message.body);
                    reader.struct_start().is_some() && reader.bool() == Some(true)
                };
                self.events.push(Event::Approved(token, yes));
                self.drop_bus_if_idle();
            }
            // Not knowing whose session locked, or who is left, closes everyone's: it fails closed
            Waiting::SessionUser => match session_user(&message).filter(|_| good) {
                Some(uid) => self.events.push(Event::Locked(uid)),
                None => self.events.push(Event::SessionsLeft(Vec::new())),
            },
            Waiting::Sessions => {
                let Some(sessions) = session_list(&message).filter(|_| good) else {
                    self.census = None;
                    return self.events.push(Event::SessionsLeft(Vec::new()));
                };
                if let Some(census) = &mut self.census {
                    census.answers_left += sessions.len();
                }
                for (uid, path) in sessions {
                    let mut body = Writer::default();
                    body.string(SESSION);
                    body.string("Class");
                    let Some(bus) = self.bus.as_mut() else { return };
                    let serial = bus.call(LOGIN1, &path, PROPERTIES, "Get", "ss", &body.bytes);
                    self.waiting.insert(serial, Waiting::ClassOf(uid));
                }
                self.census_answered();
            }
            Waiting::ClassOf(uid) => {
                // A session gone by the time it was asked about is gone, and any class of person's counts
                let person = good && {
                    let mut reader = Reader::new(&message.body);
                    reader.signature() == Some("s") && reader.string().is_some_and(|class| class.starts_with("user"))
                };
                if let (true, Some(census)) = (person, &mut self.census) {
                    census.people.push(uid);
                }
                self.census_answered();
            }
            Waiting::SessionsOf(token) => {
                let Some(uid) = self.asking.get(&token).map(|asking| asking.uid) else { return };
                let Some(sessions) = session_list(&message).filter(|_| good) else { return self.not_asked(token) };
                let mine: Vec<String> = sessions.into_iter().filter(|(owner, _)| *owner == uid).map(|(_, path)| path).collect();
                // A process with no session of its own: polkit knows what to make of that
                if mine.is_empty() {
                    return self.put_to_polkit(token);
                }
                let Some(asking) = self.asking.get_mut(&token) else { return };
                asking.sessions_left = mine.len();
                for path in mine {
                    let mut body = Writer::default();
                    body.string(SESSION);
                    body.string("LockedHint");
                    let Some(bus) = self.bus.as_mut() else { return };
                    let serial = bus.call(LOGIN1, &path, PROPERTIES, "Get", "ss", &body.bytes);
                    self.waiting.insert(serial, Waiting::LockedOf(token));
                }
            }
            Waiting::LockedOf(token) => {
                let unlocked = good && {
                    let mut reader = Reader::new(&message.body);
                    reader.signature() == Some("b") && reader.bool() == Some(false)
                };
                if !unlocked {
                    return self.not_asked(token);
                }
                let Some(asking) = self.asking.get_mut(&token) else { return };
                asking.sessions_left = asking.sessions_left.saturating_sub(1);
                if asking.sessions_left == 0 {
                    self.put_to_polkit(token);
                }
            }
            Waiting::Inhibitor => {
                if self.watching {
                    self.inhibitor = message.fds.into_iter().next();
                }
            }
            Waiting::Nothing => {}
        }
    }
}

/// `LockedHint`'s new value, if this change is of it
fn locked_hint(body: &[u8]) -> Option<bool> {
    let mut reader = Reader::new(body);
    if reader.string()? != SESSION {
        return None;
    }
    let end = reader.array_end(8)?;
    while reader.at() < end {
        reader.struct_start()?;
        let name = reader.string()?;
        let signature = reader.signature()?;
        if name == "LockedHint" && signature == "b" {
            return reader.bool();
        }
        reader.skip(signature, 0)?;
    }
    None
}

/// The answer to Get(Session, "User"): v containing (uo)
fn session_user(message: &Message) -> Option<u32> {
    let mut reader = Reader::new(&message.body);
    if reader.signature()? != "(uo)" {
        return None;
    }
    reader.struct_start()?;
    reader.u32()
}

/// The answer to ListSessions: a(susso), each session's account and its object
fn session_list(message: &Message) -> Option<Vec<(u32, String)>> {
    if message.signature != "a(susso)" {
        return None;
    }
    let mut reader = Reader::new(&message.body);
    let end = reader.array_end(8)?;
    let mut sessions = Vec::new();
    while reader.at() < end {
        reader.struct_start()?;
        reader.string()?;
        let uid = reader.u32()?;
        reader.string()?;
        reader.string()?;
        sessions.push((uid, reader.string()?.to_string()));
    }
    Some(sessions)
}

impl Outside for Desktop {
    fn wait_on(&self) -> Option<(RawFd, i16)> {
        let bus = self.bus.as_ref()?;
        Some((bus.fd(), if bus.wants_write() { libc::POLLIN | libc::POLLOUT } else { libc::POLLIN }))
    }

    fn has_events(&self) -> bool {
        !self.events.is_empty()
    }

    fn ready(&mut self) -> Vec<Event> {
        self.flush();
        let mut gone = match &mut self.bus {
            Some(bus) => bus.read().is_err(),
            None => false,
        };
        // What arrived before the bus went still counts
        while let Some(bus) = &mut self.bus {
            match bus.next_message() {
                // Heard whenever connected: a prompt being asked has to go if the screen locks, with no vault open
                Ok(Some(message)) if message.kind == bus::SIGNAL => self.signal(&message),
                Ok(Some(message)) if message.kind == bus::METHOD_RETURN || message.kind == bus::ERROR => self.answer(message),
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(NotAMessage) => {
                    gone = true;
                    break;
                }
            }
        }
        if gone {
            self.lost();
        }
        self.flush();
        std::mem::take(&mut self.events)
    }

    fn watch(&mut self) -> Result<(), CannotWatch> {
        if self.watching {
            return Ok(());
        }
        let Some(bus) = self.connected() else { return Err(CannotWatch) };
        let mut body = Writer::default();
        for text in ["sleep", "Amethystora Vault", "Closes the vault before the machine sleeps", "delay"] {
            body.string(text);
        }
        let inhibitor = bus.call(LOGIN1, LOGIN1_PATH, MANAGER, "Inhibit", "ssss", &body.bytes);
        self.waiting.insert(inhibitor, Waiting::Inhibitor);
        self.watching = true;
        self.flush();
        if self.watching { Ok(()) } else { Err(CannotWatch) }
    }

    fn unwatch(&mut self) {
        self.watching = false;
        self.inhibitor = None;
        self.drop_bus_if_idle();
    }

    fn approve(&mut self, token: u64, ask: &Ask, peer: &Peer) {
        // The process itself where the kernel gave a pidfd, which a reused number cannot pass for, or its number and start time
        let started = peer.start_time();
        if started.is_none() {
            self.events.push(Event::Approved(token, false));
            return;
        }
        let pidfd = peer.pidfd.as_ref().and_then(|pidfd| pidfd.try_clone().ok());
        let program = peer.program();
        let item = clean(&ask.item, 100);
        let detail = clean(&ask.detail, 100);
        let mut body = Writer::default();
        // The subject: (sa{sv}), the process, with what keeps its number its own
        body.align(8);
        body.string("unix-process");
        body.array(8, |writer| {
            if pidfd.is_some() {
                writer.align(8);
                writer.string("pidfd");
                writer.signature("h");
                writer.u32(0);
            } else {
                writer.align(8);
                writer.string("pid");
                writer.signature("u");
                writer.u32(peer.pid as u32);
                writer.align(8);
                writer.string("start-time");
                writer.signature("t");
                writer.u64(started.unwrap_or(0));
            }
            writer.align(8);
            writer.string("uid");
            writer.signature("i");
            writer.i32(peer.uid as i32);
        });
        body.string(ask.action.id());
        body.string_map(&[("polkit.message", ask.message), ("polkit.gettext_domain", "amethystora"), ("program", &program), ("item", &item), ("detail", &detail)]);
        // AllowUserInteraction
        body.u32(1);
        body.string(&cancellation(token));
        self.ask(token, peer.uid, Some(body.bytes), pidfd);
    }

    fn when_unlocked(&mut self, token: u64, peer: &Peer) {
        self.ask(token, peer.uid, None, None);
    }

    fn has_session(&self, uid: u32) -> bool {
        user_state(uid).lines().filter_map(|line| line.strip_prefix("SESSIONS=")).any(|ids| ids.split_whitespace().any(|id| is_a_persons_session(uid, id)))
    }

    fn lingering(&self, uid: u32) -> bool {
        user_state(uid).lines().any(|line| line == "STATE=lingering")
    }

    fn cancel(&mut self, token: u64) {
        if self.asking.remove(&token).is_some() {
            self.approvals = self.approvals.saturating_sub(1);
            self.drop_bus_if_idle();
            return;
        }
        let mut body = Writer::default();
        body.string(&cancellation(token));
        let at_polkit = self.waiting.iter().find(|(_, waiting)| matches!(waiting, Waiting::Approval(asked) if *asked == token)).map(|(serial, _)| *serial);
        let Some(serial) = at_polkit else { return };
        self.waiting.remove(&serial);
        self.approvals = self.approvals.saturating_sub(1);
        if let Some(bus) = &mut self.bus {
            let serial = bus.call(POLKIT, POLKIT_PATH, AUTHORITY, "CancelCheckAuthorization", "s", &body.bytes);
            self.waiting.insert(serial, Waiting::Nothing);
        }
        self.flush();
        self.drop_bus_if_idle();
    }
}

fn cancellation(token: u64) -> String {
    format!("amethystora-vault-{token}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reads_logind_s_change_of_lock() {
        let body = |locked: bool, interface: &str| {
            let mut body = Writer::default();
            body.string(interface);
            body.array(8, |writer| {
                writer.align(8);
                writer.string("IdleHint");
                writer.signature("b");
                writer.bool(true);
                writer.align(8);
                writer.string("LockedHint");
                writer.signature("b");
                writer.bool(locked);
            });
            body.array(4, |_| {});
            body.bytes
        };
        assert_eq!(locked_hint(&body(true, SESSION)), Some(true));
        assert_eq!(locked_hint(&body(false, SESSION)), Some(false));
        assert_eq!(locked_hint(&body(true, "other")), None);
        assert_eq!(locked_hint(&[]), None);
    }

    #[test]
    fn it_reads_who_a_session_is() {
        let mut body = Writer::default();
        body.signature("(uo)");
        body.align(8);
        body.u32(1000);
        body.string("/org/freedesktop/login1/user/_1000");
        let message = Message { body: body.bytes, ..Message::default() };
        assert_eq!(session_user(&message), Some(1000));
        assert_eq!(session_user(&Message::default()), None);
        let mut body = Writer::default();
        body.array(8, |writer| {
            for uid in [1000, 1001] {
                writer.align(8);
                writer.string("2");
                writer.u32(uid);
                writer.string("me");
                writer.string("seat0");
                writer.string("/org/freedesktop/login1/session/_32");
            }
        });
        let message = Message { signature: "a(susso)".into(), body: body.bytes, ..Message::default() };
        let path = "/org/freedesktop/login1/session/_32".to_string();
        assert_eq!(session_list(&message), Some(vec![(1000, path.clone()), (1001, path)]));
    }

    fn sleeping(sender: &str) -> Message {
        let mut body = Writer::default();
        body.bool(true);
        Message { kind: bus::SIGNAL, sender: sender.into(), path: LOGIN1_PATH.into(), interface: MANAGER.into(), member: "PrepareForSleep".into(), signature: "b".into(), body: body.bytes, ..Message::default() }
    }

    #[test]
    fn a_signal_counts_only_from_logind() {
        let mut desktop = Desktop::new();
        // Nobody known yet: nothing counts
        desktop.signal(&sleeping(":1.5"));
        assert!(desktop.events.is_empty());
        desktop.owners.insert(LOGIN1, ":1.5".to_string());
        // Anyone on the bus can send this connection a signal of its own
        desktop.signal(&sleeping(":1.9"));
        assert!(desktop.events.is_empty());
        let mut lock = sleeping(":1.9");
        (lock.interface, lock.member, lock.signature, lock.body) = (SESSION.into(), "Lock".into(), String::new(), Vec::new());
        desktop.signal(&lock);
        assert!(desktop.events.is_empty() && desktop.waiting.is_empty(), "no question asked of logind either");
        desktop.signal(&sleeping(":1.5"));
        assert_eq!(desktop.events, vec![Event::Sleeping]);
    }

    #[test]
    fn only_the_bus_says_who_owns_a_name() {
        let mut desktop = Desktop::new();
        let changed = |sender: &str, new: &str| {
            let mut body = Writer::default();
            for text in [LOGIN1, "", new] {
                body.string(text);
            }
            Message { kind: bus::SIGNAL, sender: sender.into(), interface: DBUS.into(), member: "NameOwnerChanged".into(), signature: "sss".into(), body: body.bytes, ..Message::default() }
        };
        desktop.signal(&changed(":1.9", ":1.9"));
        assert!(desktop.owners.is_empty(), "a stranger cannot name itself logind");
        desktop.signal(&changed(DBUS, ":1.5"));
        assert_eq!(desktop.owners.get(LOGIN1), Some(&":1.5".to_string()));
    }

    #[test]
    fn a_reply_counts_only_from_whoever_was_asked() {
        let mut desktop = Desktop::new();
        desktop.owners.insert(POLKIT, ":1.3".to_string());
        let yes = |sender: &str, kind: u8| {
            let mut body = Writer::default();
            body.align(8);
            body.bool(true);
            body.bool(false);
            body.array(8, |_| {});
            Message { kind, reply_to: Some(7), sender: sender.into(), signature: "(bba{ss})".into(), body: body.bytes, ..Message::default() }
        };
        desktop.waiting.insert(7, Waiting::Approval(1));
        desktop.approvals = 1;
        desktop.answer(yes(":1.9", bus::METHOD_RETURN));
        desktop.answer(yes(":1.9", bus::ERROR));
        assert!(desktop.events.is_empty() && desktop.waiting.contains_key(&7), "a stranger's yes is nobody's");
        // The bus says no for a name that is not there
        desktop.answer(yes(DBUS, bus::ERROR));
        assert_eq!(desktop.events, vec![Event::Approved(1, false)]);
        desktop.events.clear();
        // (An answered call with nothing else pending lets go of the bus, and of what it knew)
        desktop.owners.insert(POLKIT, ":1.3".to_string());
        desktop.waiting.insert(7, Waiting::Approval(1));
        desktop.approvals = 1;
        desktop.answer(yes(":1.3", bus::METHOD_RETURN));
        assert_eq!(desktop.events, vec![Event::Approved(1, true)]);
    }

    #[test]
    fn an_account_has_a_session_while_logind_says_so() {
        let folder = std::env::temp_dir().join(format!("avs-users-{}", amethystora_vault_core::seal::random_hex(4)));
        std::fs::create_dir_all(&folder).unwrap();
        let sessions = folder.join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::write(folder.join("1000"), "NAME=me\nSESSIONS=2 5\n").unwrap();
        std::fs::write(sessions.join("2"), "UID=1000\nCLASS=user\nSTATE=active\n").unwrap();
        std::fs::write(sessions.join("5"), "UID=1000\nCLASS=manager\nSTATE=online\n").unwrap();
        std::fs::write(folder.join("1001"), "NAME=lingering\nSTATE=lingering\n").unwrap();
        // A lingering account's manager session is listed in SESSIONS= with nobody there
        std::fs::write(folder.join("1003"), "NAME=lingering-with-manager\nSTATE=lingering\nSESSIONS=7\nACTIVE_SESSIONS=\nONLINE_SESSIONS=7\n").unwrap();
        std::fs::write(sessions.join("7"), "UID=1003\nCLASS=manager\nSTATE=online\n").unwrap();
        // Every other way for a session not to be a person's present
        std::fs::write(folder.join("1004"), "NAME=leaving\nSTATE=closing\nSESSIONS=8\n").unwrap();
        std::fs::write(sessions.join("8"), "UID=1004\nCLASS=user\nSTATE=closing\n").unwrap();
        std::fs::write(folder.join("1005"), "NAME=odd\nSESSIONS=9 ../x 10\n").unwrap();
        std::fs::write(sessions.join("9"), "UID=1000\nCLASS=user\nSTATE=active\n").unwrap();
        std::fs::write(sessions.join("10"), "UID=1005\nCLASS=greeter\nSTATE=active\n").unwrap();
        // SAFETY: set before any other thread of this test binary reads it; only this test uses the names
        unsafe {
            std::env::set_var("AMETHYSTORA_VAULT_LOGIND_USERS", &folder);
            std::env::set_var("AMETHYSTORA_VAULT_LOGIND_SESSIONS", &sessions);
        }
        let desktop = Desktop::new();
        assert!(desktop.has_session(1000));
        assert!(!desktop.has_session(1001));
        assert!(!desktop.has_session(1002));
        assert!(!desktop.has_session(1003), "a manager session is nobody");
        assert!(!desktop.has_session(1004), "a closing one neither");
        assert!(!desktop.has_session(1005), "nor another account's file, nor a name that is a path, nor another class");
        assert!(desktop.lingering(1003) && !desktop.lingering(1004));
        // Logged out and lingering is not a session, but is the state a timer's work runs in
        assert!(desktop.lingering(1001) && !desktop.lingering(1000) && !desktop.lingering(1002));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
