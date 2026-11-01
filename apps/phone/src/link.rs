//! The phone link as this window sees it: the program amethystora-connect on the session bus
//! (connect/dbus/org.amethystora.Connect.xml), watched for coming and going, its phones read and kept up to
//! date from the signals it sends, and the calls the window makes to it. Everything the window shows comes
//! from here, and it works out nothing itself.
//!
//! The window never starts the program: it watches the name without asking the bus to start it, and calls
//! carry NO_AUTO_START, since the program listens only while `ame desktop phone` has it on. Every string and
//! number that comes over the bus is checked for its type and cleaned (model.rs) before it is kept.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::Path;
use std::rc::{Rc, Weak};

use amethystora_ui::gio::prelude::*;
use amethystora_ui::{gio, glib};

use crate::model::{self, Device, Transfer};

/// The program's name on the session bus, and its objects
pub const NAME: &str = "org.amethystora.Connect";
pub const MANAGER_PATH: &str = "/org/amethystora/Connect";
pub const MANAGER: &str = "org.amethystora.Connect";
pub const DEVICE: &str = "org.amethystora.Connect.Device";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const NOT_AVAILABLE: &str = "org.amethystora.Connect.Error.NotAvailable";

/// A call that did not work
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The phone is not connected, or not paired, or does not take that
    NotAvailable,
    /// The program is not running
    Gone,
    /// Anything else, as the bus said it: for a log, never shown as it is
    Other(String),
}

impl From<glib::Error> for Failure {
    fn from(error: glib::Error) -> Self {
        match gio::DBusError::remote_error(&error).as_deref() {
            Some(NOT_AVAILABLE) => Failure::NotAvailable,
            Some("org.freedesktop.DBus.Error.ServiceUnknown" | "org.freedesktop.DBus.Error.NameHasNoOwner") => Failure::Gone,
            _ => Failure::Other(error.message().chars().take(300).collect()),
        }
    }
}

/// What happened that the window shows
#[derive(Debug, Clone)]
pub enum Event {
    /// The program came or went
    Presence,
    /// A phone came, went, or something about one changed
    Devices,
    /// A file's transfer moved on
    Transfer(Transfer),
    /// A phone's list of conversations changed (the device id)
    Conversations(String),
    /// A conversation of a phone has other messages (the device id and thread)
    Messages(String, i64),
}

/// A notification of a phone that can be answered
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repliable {
    pub id: String,
    pub app: String,
    pub title: String,
    pub text: String,
    /// The app's icon as the program kept it from the phone, when it did (model::kept_icon)
    pub icon: Option<std::path::PathBuf>,
}

#[derive(Default)]
struct Inner {
    connection: Option<gio::DBusConnection>,
    /// The unique name of the connection the program amethystora-connect holds its name with, once it is known to be
    /// that program, and which every call and signal is then addressed to; and whether the name is held by another
    owner: String,
    untrusted: bool,
    present: bool,
    subscriptions: Vec<gio::SignalSubscription>,
    /// This computer's name as phones see it, and where files from them go
    name: String,
    receive_folder: String,
    /// Whether other computers pair too (the program's switch `computers`)
    computers: bool,
    devices: Vec<Device>,
    /// Paths being read, and those that changed while they were
    fetching: HashSet<String>,
    dirty: HashSet<String>,
    /// Newest last; at most the last few of each
    transfers: Vec<Transfer>,
    /// Which reading of the program this is, so that an answer to an earlier one is dropped
    generation: u32,
}

/// The program on the bus
pub struct Link {
    me: Weak<Link>,
    on_event: Box<dyn Fn(Event)>,
    inner: RefCell<Inner>,
}

/// How many transfers are remembered
const MOST_TRANSFERS: usize = 64;

impl Link {
    pub fn new(on_event: impl Fn(Event) + 'static) -> Rc<Link> {
        Rc::new_cyclic(|me| Link { me: me.clone(), on_event: Box::new(on_event), inner: RefCell::default() })
    }

    /// Begins to watch for the program, which fires `Presence` once the bus has answered
    pub fn start(self: &Rc<Self>) {
        let (appeared, vanished) = (self.me.clone(), self.me.clone());
        let id = gio::bus_watch_name(
            gio::BusType::Session,
            NAME,
            gio::BusNameWatcherFlags::NONE,
            move |connection, _, owner| {
                if let Some(this) = appeared.upgrade() {
                    this.appeared(connection, owner.to_string());
                }
            },
            move |_, _| {
                if let Some(this) = vanished.upgrade() {
                    this.vanished();
                }
            },
        );
        // The watch lasts as long as the window does, so its id is not kept
        let _ = id;
    }

    fn fire(&self, event: Event) {
        (self.on_event)(event);
    }

    // --- What is known ---------------------------------------------------------------------------------

    pub fn present(&self) -> bool {
        self.inner.borrow().present
    }

    /// The name is held by a program that is not amethystora-connect: nothing is asked of it, and the window shows
    /// the link as off
    pub fn untrusted(&self) -> bool {
        self.inner.borrow().untrusted
    }

    pub fn devices(&self) -> Vec<Device> {
        self.inner.borrow().devices.clone()
    }

    pub fn device(&self, id: &str) -> Option<Device> {
        self.inner.borrow().devices.iter().find(|d| d.id == id).cloned()
    }

    /// This computer's name as the phones see it, and the folder files from them are put in
    pub fn name(&self) -> String {
        self.inner.borrow().name.clone()
    }

    pub fn receive_folder(&self) -> String {
        self.inner.borrow().receive_folder.clone()
    }

    /// Whether other computers pair with this one too
    pub fn computers(&self) -> bool {
        self.inner.borrow().computers
    }

    /// The transfers of a phone still going, and those that ended a moment ago
    pub fn transfers(&self, device: &str) -> Vec<Transfer> {
        self.inner.borrow().transfers.iter().filter(|t| t.device == device).cloned().collect()
    }

    pub fn transfer(&self, id: &str) -> Option<Transfer> {
        self.inner.borrow().transfers.iter().find(|t| t.id == id).cloned()
    }

    // --- Coming and going ------------------------------------------------------------------------------

    /// The name is taken: by the program, which is believed, or by another, which is not. The name is free while the
    /// link is off, so any program of the account can take it, and nothing is read from the owner, sent to it or
    /// shown of it until the bus says which program it is.
    fn appeared(self: &Rc<Self>, connection: gio::DBusConnection, owner: String) {
        let generation = {
            let mut inner = self.inner.borrow_mut();
            inner.generation += 1;
            inner.connection = None;
            inner.owner.clear();
            inner.present = false;
            inner.untrusted = false;
            inner.subscriptions.clear();
            inner.devices.clear();
            inner.fetching.clear();
            inner.dirty.clear();
            inner.transfers.clear();
            inner.generation
        };
        let this = self.clone();
        glib::spawn_future_local(async move {
            let trusted = owner_is_program(&connection, &owner, &program()).await;
            if !this.current(generation) {
                return;
            }
            if !trusted {
                this.inner.borrow_mut().untrusted = true;
                this.fire(Event::Presence);
                return;
            }
            {
                let mut inner = this.inner.borrow_mut();
                inner.connection = Some(connection.clone());
                inner.owner = owner.clone();
                inner.present = true;
            }
            // Subscribed before anything is read, so that no change falls between the reading and the signals, and
            // to the connection that was checked, not to whoever holds the name later
            let weak = this.me.clone();
            let subscription = connection.subscribe_to_signal(Some(&owner), None, None, None, None, gio::DBusSignalFlags::NONE, move |signal| {
                if let Some(this) = weak.upgrade() {
                    this.signal(signal.object_path, signal.interface_name, signal.signal_name, signal.parameters);
                }
            });
            this.inner.borrow_mut().subscriptions.push(subscription);
            this.fire(Event::Presence);
            this.load(generation).await;
        });
    }

    fn vanished(&self) {
        {
            let mut inner = self.inner.borrow_mut();
            inner.generation += 1;
            inner.connection = None;
            inner.owner.clear();
            inner.untrusted = false;
            inner.present = false;
            inner.subscriptions.clear();
            inner.devices.clear();
            inner.fetching.clear();
            inner.dirty.clear();
            inner.transfers.clear();
            inner.name.clear();
            inner.computers = false;
        }
        self.fire(Event::Presence);
    }

    /// Reads the program's phones again, as when the window comes back to the front
    pub fn refresh(self: &Rc<Self>) {
        let generation = self.inner.borrow().generation;
        if self.present() {
            let this = self.clone();
            glib::spawn_future_local(async move { this.load(generation).await });
        }
    }

    fn current(&self, generation: u32) -> bool {
        self.inner.borrow().generation == generation
    }

    /// The program's own properties and every phone's
    async fn load(self: &Rc<Self>, generation: u32) {
        if let Ok(all) = self.call(MANAGER_PATH, PROPERTIES, "GetAll", Some((MANAGER,).to_variant())).await {
            if self.current(generation) {
                let mut inner = self.inner.borrow_mut();
                for (name, value) in model::entries(&all.child_value(0)) {
                    match name.as_str() {
                        "Name" => inner.name = value.str().map(|n| model::clean(n, model::MOST_NAME)).unwrap_or_default(),
                        "ReceiveFolder" => inner.receive_folder = value.str().map(str::to_string).unwrap_or_default(),
                        "Computers" => inner.computers = value.get::<bool>().unwrap_or(false),
                        _ => {}
                    }
                }
            }
        }
        let paths: Vec<String> = match self.call(MANAGER_PATH, MANAGER, "GetDevices", None).await {
            Ok(reply) => reply.child_value(0).get::<Vec<String>>().unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        if !self.current(generation) {
            return;
        }
        // Those that went are gone, whatever the signals said
        self.inner.borrow_mut().devices.retain(|d| paths.contains(&d.path));
        for path in paths.into_iter().filter(|p| valid_path(p)).take(64) {
            self.fetch(&path, generation).await;
        }
        if self.current(generation) {
            self.fire(Event::Devices);
        }
    }

    /// One phone's properties, read again until nothing changed under the reading
    async fn fetch(self: &Rc<Self>, path: &str, generation: u32) {
        if !self.inner.borrow_mut().fetching.insert(path.to_string()) {
            self.inner.borrow_mut().dirty.insert(path.to_string());
            return;
        }
        loop {
            self.inner.borrow_mut().dirty.remove(path);
            let reply = self.call(path, PROPERTIES, "GetAll", Some((DEVICE,).to_variant())).await;
            if !self.current(generation) {
                return;
            }
            if let Ok(all) = reply {
                let device = Device::from_properties(path, &all.child_value(0));
                if !device.id.is_empty() {
                    let mut inner = self.inner.borrow_mut();
                    match inner.devices.iter_mut().find(|d| d.path == path) {
                        Some(known) => *known = device,
                        None => inner.devices.push(device),
                    }
                }
            }
            if !self.inner.borrow().dirty.contains(path) {
                break;
            }
        }
        self.inner.borrow_mut().fetching.remove(path);
    }

    // --- Signals ---------------------------------------------------------------------------------------

    fn signal(self: &Rc<Self>, path: &str, interface: &str, signal: &str, parameters: &glib::Variant) {
        if !valid_path(path) && path != MANAGER_PATH {
            return;
        }
        let generation = self.inner.borrow().generation;
        match (interface, signal) {
            (MANAGER, "DeviceAdded") => {
                if let Some(added) = parameters.child_value(0).str().map(str::to_string).filter(|p| valid_path(p)) {
                    let this = self.clone();
                    glib::spawn_future_local(async move {
                        this.fetch(&added, generation).await;
                        if this.current(generation) {
                            this.fire(Event::Devices);
                        }
                    });
                }
            }
            (MANAGER, "DeviceRemoved") => {
                if let Some(removed) = parameters.child_value(0).str() {
                    self.inner.borrow_mut().devices.retain(|d| d.path != removed);
                    self.fire(Event::Devices);
                }
            }
            (PROPERTIES, "PropertiesChanged") => {
                let which = parameters.child_value(0);
                let changed = parameters.child_value(1);
                if which.str() == Some(DEVICE) && valid_path(path) {
                    let known = self.inner.borrow().devices.iter().any(|d| d.path == path);
                    if !known || self.inner.borrow().fetching.contains(path) {
                        // Not read yet, or being read: read it again once it has been, for it may have changed under it
                        let path = path.to_string();
                        let this = self.clone();
                        glib::spawn_future_local(async move {
                            this.fetch(&path, generation).await;
                            if this.current(generation) {
                                this.fire(Event::Devices);
                            }
                        });
                    } else {
                        if let Some(device) = self.inner.borrow_mut().devices.iter_mut().find(|d| d.path == path) {
                            for (name, value) in model::entries(&changed) {
                                device.set(&name, &value);
                            }
                        }
                        // A property named but not given is read again
                        if parameters.child_value(2).n_children() > 0 {
                            let (path, this) = (path.to_string(), self.clone());
                            glib::spawn_future_local(async move {
                                this.fetch(&path, generation).await;
                                if this.current(generation) {
                                    this.fire(Event::Devices);
                                }
                            });
                        }
                        self.fire(Event::Devices);
                    }
                } else if which.str() == Some(MANAGER) {
                    {
                        let mut inner = self.inner.borrow_mut();
                        for (name, value) in model::entries(&changed) {
                            match name.as_str() {
                                "Name" => inner.name = value.str().map(|n| model::clean(n, model::MOST_NAME)).unwrap_or_default(),
                                "ReceiveFolder" => inner.receive_folder = value.str().map(str::to_string).unwrap_or_default(),
                                "Computers" => inner.computers = value.get::<bool>().unwrap_or(false),
                                _ => {}
                            }
                        }
                    }
                    // The page's heading and words follow the program's own properties too
                    self.fire(Event::Devices);
                }
            }
            (DEVICE, "Transfer") => {
                let Some(device) = self.device_at(path) else { return };
                let id = parameters.child_value(0).str().unwrap_or_default().to_string();
                let transfer = Transfer {
                    id: id.clone(),
                    device: device.id,
                    // A name, never a path: kept to its last part whatever the program sent
                    name: model::clean(parameters.child_value(1).str().unwrap_or_default().rsplit('/').next().unwrap_or_default(), 255),
                    incoming: parameters.child_value(2).get::<bool>().unwrap_or(false),
                    done: parameters.child_value(3).get::<i64>().unwrap_or(0),
                    total: parameters.child_value(4).get::<i64>().unwrap_or(0),
                    state: parameters.child_value(5).str().unwrap_or_default().to_string(),
                };
                if id.is_empty() {
                    return;
                }
                {
                    let mut inner = self.inner.borrow_mut();
                    match inner.transfers.iter_mut().find(|t| t.id == id) {
                        Some(known) => *known = transfer.clone(),
                        None => {
                            inner.transfers.push(transfer.clone());
                            let over = inner.transfers.len().saturating_sub(MOST_TRANSFERS);
                            inner.transfers.drain(..over);
                        }
                    }
                }
                self.fire(Event::Transfer(transfer));
            }
            (DEVICE, "ConversationsChanged") => {
                if let Some(device) = self.device_at(path) {
                    self.fire(Event::Conversations(device.id));
                }
            }
            (DEVICE, "MessagesChanged") => {
                if let Some(device) = self.device_at(path) {
                    self.fire(Event::Messages(device.id, parameters.child_value(0).get::<i64>().unwrap_or(-1)));
                }
            }
            _ => {}
        }
    }

    fn device_at(&self, path: &str) -> Option<Device> {
        self.inner.borrow().devices.iter().find(|d| d.path == path).cloned()
    }

    // --- Calls -----------------------------------------------------------------------------------------

    async fn call(&self, path: &str, interface: &str, method: &str, parameters: Option<glib::Variant>) -> Result<glib::Variant, Failure> {
        let (connection, owner) = {
            let inner = self.inner.borrow();
            (inner.connection.clone(), inner.owner.clone())
        };
        // To the connection that was checked, by its own name: never to the well-known one, which may change hands
        let Some(connection) = connection else { return Err(Failure::Gone) };
        connection
            .call_future(Some(&owner), path, interface, method, parameters.as_ref(), None, gio::DBusCallFlags::NO_AUTO_START, 25_000)
            .await
            .map_err(Failure::from)
    }

    async fn on_device(&self, id: &str, method: &str, parameters: Option<glib::Variant>) -> Result<glib::Variant, Failure> {
        let Some(device) = self.device(id) else { return Err(Failure::NotAvailable) };
        self.call(&device.path, DEVICE, method, parameters).await
    }

    /// Announces this computer now and tries the last address of each paired phone
    pub async fn announce(&self) -> Result<(), Failure> {
        self.call(MANAGER_PATH, MANAGER, "Announce", None).await.map(|_| ())
    }

    pub async fn pair(&self, id: &str) -> Result<(), Failure> {
        self.on_device(id, "Pair", None).await.map(|_| ())
    }

    pub async fn accept_pairing(&self, id: &str) -> Result<(), Failure> {
        self.on_device(id, "AcceptPairing", None).await.map(|_| ())
    }

    pub async fn reject_pairing(&self, id: &str) -> Result<(), Failure> {
        self.on_device(id, "RejectPairing", None).await.map(|_| ())
    }

    pub async fn unpair(&self, id: &str) -> Result<(), Failure> {
        self.on_device(id, "Unpair", None).await.map(|_| ())
    }

    pub async fn ping(&self, id: &str) -> Result<(), Failure> {
        self.on_device(id, "Ping", None).await.map(|_| ())
    }

    /// Makes the phone ring, even when it is silent, until Found it is pressed on it
    pub async fn ring(&self, id: &str) -> Result<(), Failure> {
        self.on_device(id, "Ring", None).await.map(|_| ())
    }

    /// Sends files (file: addresses); the ids of their transfers
    pub async fn send_files(&self, id: &str, uris: &[String]) -> Result<Vec<String>, Failure> {
        let reply = self.on_device(id, "SendFiles", Some((uris.to_vec(),).to_variant())).await?;
        Ok(reply.child_value(0).get::<Vec<String>>().unwrap_or_default())
    }

    pub async fn send_url(&self, id: &str, url: &str) -> Result<(), Failure> {
        self.on_device(id, "SendUrl", Some((url,).to_variant())).await.map(|_| ())
    }

    pub async fn cancel_transfer(&self, id: &str, transfer: &str) -> Result<(), Failure> {
        self.on_device(id, "CancelTransfer", Some((transfer,).to_variant())).await.map(|_| ())
    }

    pub async fn reply_to_notification(&self, id: &str, notification: &str, text: &str) -> Result<(), Failure> {
        self.on_device(id, "ReplyToNotification", Some((notification, text).to_variant())).await.map(|_| ())
    }

    /// The notifications of a phone that can be answered, newest first
    pub async fn repliable(&self, id: &str) -> Result<Vec<Repliable>, Failure> {
        let reply = self.on_device(id, "GetRepliable", None).await?;
        let list = reply.child_value(0);
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| amethystora_ui::palette::home().join(".local/state"));
        Ok((0..list.n_children())
            .filter_map(|at| {
                let mut found = Repliable { id: String::new(), app: String::new(), title: String::new(), text: String::new(), icon: None };
                for (key, value) in model::entries(&list.child_value(at)) {
                    let text = value.str().unwrap_or_default();
                    match key.as_str() {
                        "id" => found.id = text.to_string(),
                        "app" => found.app = model::clean(text, model::MOST_NAME).replace('\n', " "),
                        "title" => found.title = model::clean(text, 256),
                        "text" => found.text = model::clean(text, model::MOST_TEXT),
                        "icon" => found.icon = model::kept_icon(text, &state).filter(|p| p.symlink_metadata().is_ok_and(|m| m.is_file())),
                        _ => {}
                    }
                }
                (!found.id.is_empty()).then_some(found)
            })
            .collect())
    }

    pub async fn request_conversations(&self, id: &str) -> Result<(), Failure> {
        self.on_device(id, "RequestConversations", None).await.map(|_| ())
    }

    pub async fn conversations(&self, id: &str, limit: i32) -> Result<Vec<model::Conversation>, Failure> {
        let reply = self.on_device(id, "ListConversations", Some((limit,).to_variant())).await?;
        Ok(model::conversations(&reply.child_value(0)))
    }

    /// Asks for `count` messages of a thread older than `before` (0: the newest)
    pub async fn request_messages(&self, id: &str, thread: i64, before: i64, count: i32) -> Result<(), Failure> {
        self.on_device(id, "RequestMessages", Some((thread, before, count).to_variant())).await.map(|_| ())
    }

    pub async fn messages(&self, id: &str, thread: i64, limit: i32) -> Result<Vec<model::Message>, Failure> {
        let reply = self.on_device(id, "ListMessages", Some((thread, limit).to_variant())).await?;
        Ok(model::messages(&reply.child_value(0)))
    }

    /// Sends a text to these addresses; thread 0 starts a conversation
    pub async fn send_message(&self, id: &str, addresses: &[String], body: &str, thread: i64) -> Result<(), Failure> {
        self.on_device(id, "SendMessage", Some((addresses.to_vec(), body, thread).to_variant())).await.map(|_| ())
    }
}

/// Where the program amethystora-connect is installed: the one program whose name on the bus is believed
const PROGRAM: &str = "/usr/libexec/amethystora-connect";

/// The program's path. The tests, which run a stand-in for it, say which in AMETHYSTORA_CONNECT_EXE; a build for the
/// image (release) takes no word from the environment.
fn program() -> String {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var("AMETHYSTORA_CONNECT_EXE").ok().filter(|path| !path.is_empty()) {
        return path;
    }
    PROGRAM.to_string()
}

/// Whether a process runs `program`, going by what /proc/PID/exe says. A program replaced by an update while it runs
/// is that path and " (deleted)".
fn is_program(exe: &Path, program: &str) -> bool {
    let exe = exe.as_os_str().as_encoded_bytes();
    exe == program.as_bytes() || exe == format!("{program} (deleted)").as_bytes()
}

/// Whether the connection `owner` is the program, as the bus knows it: the account's own, and running `program`.
/// Anything the bus cannot say is no.
async fn owner_is_program(connection: &gio::DBusConnection, owner: &str, program: &str) -> bool {
    let asked = connection
        .call_future(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetConnectionCredentials",
            Some(&(owner,).to_variant()),
            None,
            gio::DBusCallFlags::NO_AUTO_START,
            5_000,
        )
        .await;
    let Ok(reply) = asked else { return false };
    let credentials = model::entries(&reply.child_value(0));
    let number = |name: &str| credentials.iter().find(|(key, _)| key == name).and_then(|(_, value)| value.get::<u32>());
    let (Some(user), Some(pid)) = (number("UnixUserID"), number("ProcessID")) else { return false };
    // SAFETY: getuid has no failure and no arguments
    if user != unsafe { libc::getuid() } {
        return false;
    }
    std::fs::read_link(format!("/proc/{pid}/exe")).is_ok_and(|exe| is_program(&exe, program))
}

/// An object path of a phone, as the program names them: under its own, and only letters, digits and underscores
fn valid_path(path: &str) -> bool {
    path.strip_prefix("/org/amethystora/Connect/Device/")
        .is_some_and(|id| !id.is_empty() && id.len() <= 128 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_program_s_own_paths_are_taken() {
        assert!(valid_path("/org/amethystora/Connect/Device/abc_123"));
        assert!(!valid_path("/org/amethystora/Connect/Device/"));
        assert!(!valid_path("/org/amethystora/Connect/Device/a/b"));
        assert!(!valid_path("/org/amethystora/Connect"));
        assert!(!valid_path("/org/other/Device/abc"));
        assert!(!valid_path("/org/amethystora/Connect/Device/a-b"));
    }

    #[test]
    fn only_the_installed_program_is_believed() {
        let program = "/usr/libexec/amethystora-connect";
        assert!(is_program(Path::new(program), program));
        // Replaced by an update while it runs
        assert!(is_program(Path::new("/usr/libexec/amethystora-connect (deleted)"), program));
        for other in [
            "/usr/bin/python3.14",
            "/tmp/usr/libexec/amethystora-connect",
            "/usr/libexec/amethystora-connect2",
            "/usr/libexec/amethystora-connect (deleted)x",
            "/home/ada/.local/bin/amethystora-connect",
            "amethystora-connect",
            "",
        ] {
            assert!(!is_program(Path::new(other), program), "{other}");
        }
    }

    #[test]
    fn errors_are_told_apart() {
        let not_available = gio::DBusError::new_for_dbus_error(NOT_AVAILABLE, "the phone is not connected");
        assert_eq!(Failure::from(not_available), Failure::NotAvailable);
        let gone = gio::DBusError::new_for_dbus_error("org.freedesktop.DBus.Error.ServiceUnknown", "no such name");
        assert_eq!(Failure::from(gone), Failure::Gone);
        let other = gio::DBusError::new_for_dbus_error("org.example.Error", "something");
        assert!(matches!(Failure::from(other), Failure::Other(_)));
    }
}
