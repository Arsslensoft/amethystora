//! The service: the devices, their connections, discovery, and pairing. What the plugins do with a paired
//! device's packets is in `plugins.rs`, the file transfers in `transfers.rs`, and how all of it looks on the
//! session bus in `dbus.rs`. Everything runs on the main loop of one thread.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use gio::prelude::*;
use serde_json::json;

use crate::clipboard::Clipboard;
use crate::device::{self, Device};
use crate::identity::{self, Identity, Local};
use crate::limits;
use crate::link::{self, Context, Link, Opened};
use crate::net::{self, Error};
use crate::netinfo::{self, HomeZone};
use crate::notify::{Event, Notice, Notifier};
use crate::packet::{self, Packet};
use crate::pairing::{self, Input, Out, Refusal};
use crate::store::{self, Devices, Material, Paired};
use crate::text::{self, tr, trf};

pub struct Config {
    pub config_dir: PathBuf,
    pub state_dir: PathBuf,
    /// Where the account keeps downloads; the XDG folder when `None`
    pub download_dir: Option<PathBuf>,
    /// Where the account keeps pictures; the XDG folder when `None`
    pub pictures_dir: Option<PathBuf>,
    pub discovery_port: u16,
    pub control_ports: RangeInclusive<u16>,
    pub payload_ports: RangeInclusive<u16>,
    pub datagram_ports: RangeInclusive<u16>,
    /// Loopback is a place to talk to only for the tests
    pub allow_loopback: bool,
    /// The interfaces to announce on without asking firewalld: for the tests, and for a lab with no firewall
    pub home_interfaces: Option<Vec<String>>,
    pub listen_udp: bool,
    /// The program the notifications start, with its arguments to come
    pub app_command: Vec<String>,
    /// What goes in front of every program the service starts (the Phone app, `gio open`): `systemd-run --user`,
    /// which makes it a unit of the user's manager of its own, outside this service's cgroup and sandbox, so that
    /// the window outlives the service and is not shut in as the service is. Empty to start them directly.
    pub launcher: Vec<String>,
    /// Opens an address or a file in the program the person has for it
    pub open_command: String,
    pub kind: String,
    /// The files of the programs that may call the service on the bus (see `dbus::APP_EXE`)
    pub app_exe: String,
    pub module_exe: String,
    /// What this announces it handles and sends, in place of the program's own lists: for a test that plays a phone
    pub capabilities: Option<(Vec<String>, Vec<String>)>,
    pub verbose: bool,
    /// Show the phone's notifications that it marks silent (it had them before the connection) or ongoing (it will not let
    /// them be cleared). Off, as the image ships.
    pub show_silent: bool,
    pub show_ongoing: bool,
    pub record_signals: bool,
    /// The bus firewalld is asked on: the system bus when `None`
    pub firewall_bus: Option<gio::DBusConnection>,
}

impl Config {
    pub fn production() -> Config {
        let home = text::home();
        let config = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(".config"));
        let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(".local/state"));
        Config {
            config_dir: config.join("amethystora/connect"),
            state_dir: state.join("amethystora/connect"),
            download_dir: None,
            pictures_dir: None,
            discovery_port: limits::DISCOVERY_PORT,
            control_ports: limits::CONTROL_PORTS,
            payload_ports: limits::PAYLOAD_PORTS,
            datagram_ports: limits::DATAGRAM_PORTS,
            allow_loopback: false,
            home_interfaces: None,
            listen_udp: true,
            // An absolute path: a program of that name earlier in PATH (~/.local/bin) is not started instead
            app_command: vec!["/usr/bin/amethystora-phone".to_string()],
            launcher: ["/usr/bin/systemd-run", "--user", "--collect", "--quiet"].map(String::from).to_vec(),
            open_command: "/usr/bin/gio".to_string(),
            kind: "desktop".to_string(),
            app_exe: crate::dbus::APP_EXE.to_string(),
            module_exe: crate::dbus::MODULE_EXE.to_string(),
            capabilities: None,
            verbose: false,
            show_silent: false,
            show_ongoing: false,
            record_signals: false,
            firewall_bus: None,
        }
    }
}

/// What the account's settings say
#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub name: String,
    pub clipboard_send: bool,
    pub clipboard_receive: bool,
    pub photo_folder: PhotoFolder,
    /// Other computers pair too (the GSettings key computers), off as the image ships
    pub computers: bool,
    /// Music from either side and the phone as a slide clicker (the keys music and clicker), off as the image ships
    pub music: bool,
    pub clicker: bool,
    /// A code from your phone (the key codes): each new text is looked at for one, off as the image ships
    pub codes: bool,
    /// Snap with your phone (the key photo), off as the image ships
    pub photo: bool,
    /// Whose files wait for Accept (the key ask-first)
    pub ask_first: crate::ask::AskFirst,
}

/// Where pictures from a phone go (the GSettings key photo-folder)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PhotoFolder {
    /// Pictures/Phone, as the image ships
    #[default]
    Pictures,
    /// With the phone's other files, in Downloads/Phone/NAME
    Downloads,
}

impl PhotoFolder {
    /// The key's value; anything but `downloads` is the default
    pub fn from_key(value: &str) -> PhotoFolder {
        if value == "downloads" { PhotoFolder::Downloads } else { PhotoFolder::Pictures }
    }
}

/// What was sent on the bus, kept when the tests ask
#[derive(Debug, Clone)]
pub struct Signal {
    pub device: Option<String>,
    pub name: String,
    pub args: Option<glib::Variant>,
}

pub struct Transfer {
    pub device: String,
    pub abort: futures_util::future::AbortHandle,
    pub name: String,
    pub incoming: bool,
    pub total: u64,
}

#[derive(Debug)]
pub enum ApiError {
    NotAvailable(String),
    InvalidArgs(String),
}

impl ApiError {
    pub fn unavailable(why: &str) -> ApiError {
        ApiError::NotAvailable(why.to_string())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

pub struct Service {
    pub cfg: Config,
    pub me: Material,
    pub local: RefCell<Local>,
    pub tcp_port: Cell<u16>,
    pub devices: RefCell<BTreeMap<String, Rc<Device>>>,
    pub store: RefCell<Devices>,
    pub notifier: Box<dyn Notifier>,
    pub bus: RefCell<Option<gio::DBusConnection>>,
    pub clip: RefCell<Clipboard>,
    pub clip_send: Cell<bool>,
    pub clip_receive: Cell<bool>,
    pub photo_folder: Cell<PhotoFolder>,
    /// Whether other computers are talked to (identity::is_computer); phones and tablets always are
    pub computers: Cell<bool>,
    /// Music from either side, and the phone as a slide clicker (plans/ui-ideas/spec-phone.md)
    pub music: Cell<bool>,
    pub clicker: Cell<bool>,
    /// Whether a new text is looked at for a one-time code (R7-23); only its digits are ever sent to the module
    pub codes: Cell<bool>,
    /// Whether this computer lists the photo request and offers Snap with your phone (photo.rs)
    pub photo: Cell<bool>,
    /// Whose files wait for the person's Accept (ask.rs)
    pub ask_first: Cell<crate::ask::AskFirst>,
    /// This computer's players while a phone is shown them (players.rs)
    pub players: RefCell<Option<crate::players::Watcher>>,
    /// The module's connections that said they are presenting (Presenting): the clicker's keys reach the module only
    /// while one of them is a client and has not said otherwise
    pub presenters: RefCell<HashSet<String>>,
    pub home: HomeZone,
    pub next: Cell<u64>,
    handshakes: Cell<usize>,
    per_source: RefCell<HashMap<IpAddr, u32>>,
    attempts: RefCell<HashMap<IpAddr, Instant>>,
    attempts_active: Cell<usize>,
    connecting: RefCell<HashSet<(String, IpAddr)>>,
    pub downloads_active: Cell<usize>,
    pub icons_active: Cell<usize>,
    pub transfers: RefCell<HashMap<String, Transfer>>,
    /// Files that came, by transfer id, for the buttons of their notifications
    pub received: RefCell<VecDeque<(String, PathBuf)>>,
    /// When the zones were last asked for because an address was not known to be home
    zone_asked: Cell<Option<Instant>>,
    zone_tick: Cell<u64>,
    /// When pairing requests came, by the address they came from
    pair_sources: RefCell<HashMap<IpAddr, VecDeque<Instant>>>,
    /// Links and texts that came, by number, for the buttons of theirs
    pub shared: RefCell<VecDeque<(String, String, bool)>>,
    /// Who called the service on the bus, as the bus says (a unique name's role)
    pub callers: RefCell<HashMap<String, crate::dbus::Role>>,
    /// The connections of the app and the module that have called: the signals of the service are sent to them, one
    /// by one, and to no other connection on the bus
    pub clients: RefCell<HashMap<String, crate::dbus::Role>>,
    /// The module's connections that have said they show the links phones share (TakeLinks): while one of them is a
    /// client, a web link goes to it as UrlReceived and the program raises no notification of its own for it
    pub link_takers: RefCell<HashSet<String>>,
    /// The activation token of the button just pressed, for the program it starts
    activation: RefCell<Option<String>>,
    /// The sizes of the files being fetched, which the free space is not to be promised twice
    pub reserved: Cell<u64>,
    udp: RefCell<Vec<gio::Socket>>,
    listener: RefCell<Option<gio::SocketListener>>,
    reach_epoch: Cell<Instant>,
    reach_tick: Cell<u64>,
    tasks: RefCell<Vec<glib::JoinHandle<()>>>,
    pub recorded: RefCell<Vec<Signal>>,
    pub root_published: RefCell<HashMap<&'static str, glib::Variant>>,
    pub stopping: Cell<bool>,
    /// Sees every packet that arrives, before anything is done with it: for the tests
    pub tap: RefCell<Option<Rc<dyn Fn(&str, &Packet)>>>,
    pub weak: Weak<Service>,
}

impl Service {
    pub fn new(cfg: Config, notifier: Box<dyn Notifier>, settings: Settings, pretty_host: Option<String>) -> Result<Rc<Service>, String> {
        let (me, created) = store::load_material(&cfg.config_dir, packet::now_secs())?;
        if created {
            eprintln!("amethystora-connect: made this computer's certificate; phones will have to be paired again");
        }
        store::private_dir(&cfg.state_dir).map_err(|e| format!("{}: {e}", cfg.state_dir.display()))?;
        let name = identity::default_name(&settings.name, pretty_host.as_deref(), &glib::host_name());
        let mut local = Local::new(&me.device_id, &name, &cfg.kind);
        (local.incoming, local.outgoing) = identity::advertised(settings.music, settings.clicker, settings.photo);
        if let Some((incoming, outgoing)) = &cfg.capabilities {
            local.incoming = incoming.clone();
            local.outgoing = outgoing.clone();
        }
        let home = match &cfg.home_interfaces {
            Some(names) => HomeZone::fixed(&names.iter().map(String::as_str).collect::<Vec<_>>()),
            None => match cfg.firewall_bus.clone().map(Ok).unwrap_or_else(|| gio::bus_get_sync(gio::BusType::System, None::<&gio::Cancellable>)) {
                Ok(system) => HomeZone::firewalld(system),
                Err(_) => HomeZone::fixed(&[]),
            },
        };
        let devices = Devices::load(&cfg.config_dir)?;
        let service = Rc::new_cyclic(|weak| Service {
            me,
            local: RefCell::new(local),
            tcp_port: Cell::new(0),
            devices: RefCell::default(),
            store: RefCell::new(devices),
            notifier,
            bus: RefCell::new(None),
            clip: RefCell::default(),
            clip_send: Cell::new(settings.clipboard_send),
            clip_receive: Cell::new(settings.clipboard_receive),
            photo_folder: Cell::new(settings.photo_folder),
            computers: Cell::new(settings.computers),
            music: Cell::new(settings.music),
            clicker: Cell::new(settings.clicker),
            codes: Cell::new(settings.codes),
            photo: Cell::new(settings.photo),
            ask_first: Cell::new(settings.ask_first),
            players: RefCell::default(),
            presenters: RefCell::default(),
            home,
            next: Cell::new(1),
            handshakes: Cell::new(0),
            per_source: RefCell::default(),
            attempts: RefCell::default(),
            attempts_active: Cell::new(0),
            connecting: RefCell::default(),
            downloads_active: Cell::new(0),
            icons_active: Cell::new(0),
            transfers: RefCell::default(),
            received: RefCell::default(),
            zone_asked: Cell::new(None),
            zone_tick: Cell::new(0),
            pair_sources: RefCell::default(),
            shared: RefCell::default(),
            callers: RefCell::default(),
            activation: RefCell::default(),
            clients: RefCell::default(),
            link_takers: RefCell::default(),
            reserved: Cell::new(0),
            udp: RefCell::default(),
            listener: RefCell::default(),
            reach_epoch: Cell::new(Instant::now()),
            reach_tick: Cell::new(0),
            tasks: RefCell::default(),
            recorded: RefCell::default(),
            root_published: RefCell::default(),
            stopping: Cell::new(false),
            tap: RefCell::default(),
            weak: weak.clone(),
            cfg,
        });
        // The phones paired before are known from the start, disconnected
        service.restore_paired();
        let weak = Rc::downgrade(&service);
        service.notifier.set_handler(Rc::new(move |event| {
            if let Some(service) = weak.upgrade() {
                service.on_notification_event(event);
            }
        }));
        Ok(service)
    }

    pub fn log(&self, what: &str) {
        if self.cfg.verbose {
            eprintln!("amethystora-connect: {what}");
        }
    }

    pub fn next_id(&self) -> u64 {
        let n = self.next.get();
        self.next.set(n + 1);
        n
    }

    pub fn contacts_path(&self, id: &str) -> PathBuf {
        self.cfg.state_dir.join(format!("contacts-{id}.json"))
    }

    pub fn icons_dir(&self, id: &str) -> PathBuf {
        self.cfg.state_dir.join("icons").join(id)
    }

    pub fn receive_root(&self) -> PathBuf {
        let download = self.cfg.download_dir.clone().or_else(|| glib::user_special_dir(glib::UserDirectory::Downloads));
        crate::share::receive_root(download, &text::home())
    }

    pub fn photo_root(&self) -> PathBuf {
        let pictures = self.cfg.pictures_dir.clone().or_else(|| glib::user_special_dir(glib::UserDirectory::Pictures));
        crate::share::photo_root(pictures, &text::home())
    }

    /// The folder the setting gives pictures: Pictures/Phone, or `None` while they go with the phone's other files
    pub fn photo_folder_path(&self) -> Option<PathBuf> {
        (self.photo_folder.get() == PhotoFolder::Pictures).then(|| self.photo_root())
    }

    /// The folder a file from a phone is put in, made if missing (0700): a picture in Pictures/Phone while the setting
    /// says so, anything else in the phone's own folder under the receive root
    pub fn folder_for(&self, dev: &Device, name: &str) -> std::io::Result<PathBuf> {
        match self.photo_folder_path() {
            Some(folder) if crate::share::is_picture(name) => {
                if let Some(parent) = folder.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                store::private_dir(&folder)?;
                Ok(folder)
            }
            _ => crate::share::device_folder(&self.receive_root(), &dev.name()),
        }
    }

    pub fn device(&self, id: &str) -> Option<Rc<Device>> {
        self.devices.borrow().get(id).cloned()
    }

    pub fn name(&self) -> String {
        self.local.borrow().name.clone()
    }

    pub fn pin_of(&self, id: &str) -> Option<Rc<Vec<u8>>> {
        self.store.borrow().paired.get(id).map(|p| Rc::new(p.pin.clone()))
    }

    // --- Devices ------------------------------------------------------------------------------------------

    /// A device in the registry, on the bus when there is one
    fn make_device(&self, id: &str, name: &str, kind: &str, protocol: u32) -> Rc<Device> {
        let mut path = device::path_of(id);
        {
            // Two ids that differ only in characters the path cannot hold get paths that differ all the same
            let devices = self.devices.borrow();
            let mut n = 1;
            while devices.values().any(|d| d.path == path) {
                path = format!("{}_{n}", device::path_of(id));
                n += 1;
            }
        }
        let dev = Device::with_policy(id, path, name, kind, protocol, self.cfg.show_silent, self.cfg.show_ongoing);
        dev.st.borrow_mut().offers = device::Offers { music: self.music.get(), clicker: self.clicker.get(), photo: self.photo.get() };
        self.devices.borrow_mut().insert(id.to_string(), dev.clone());
        crate::dbus::register_device(&self.weak.upgrade().expect("the service is alive"), &dev);
        dev
    }

    fn remove_device(&self, dev: &Rc<Device>) {
        crate::dbus::unregister_device(&self.weak.upgrade().expect("the service is alive"), dev);
        self.devices.borrow_mut().remove(&dev.id);
    }

    /// Whether a device of this kind is talked to now: a phone or a tablet always, another computer only while the
    /// switch is on
    pub fn talks_to(&self, kind: &str) -> bool {
        self.computers.get() || !identity::is_computer(kind)
    }

    /// The paired devices of the store that are not in the registry yet, and are talked to now, made known,
    /// disconnected; returns those it made
    fn restore_paired(&self) -> Vec<Rc<Device>> {
        let paired: Vec<(String, Paired)> = self.store.borrow().paired.iter().map(|(id, p)| (id.clone(), p.clone())).collect();
        let mut made = Vec::new();
        for (id, p) in paired {
            if self.device(&id).is_some() || !self.talks_to(&p.kind) {
                continue;
            }
            let dev = self.make_device(&id, &p.name, &p.kind, p.protocol);
            {
                let mut st = dev.st.borrow_mut();
                st.paired = true;
                st.incoming = p.incoming.clone();
                st.outgoing = p.outgoing.clone();
                st.address = p.address;
                st.contacts = crate::contacts::Book::load(&self.contacts_path(&id));
            }
            made.push(dev);
        }
        made
    }

    /// The switch `computers`. Off, every other computer is let go: its connections closed and its object taken off
    /// the bus, a pairing kept in the store (Unpair alone forgets one). On, the paired ones are known again and this
    /// computer announces itself, so that the others find it at once.
    pub fn set_computers(self: &Rc<Self>, on: bool) {
        if self.computers.replace(on) == on {
            return;
        }
        if on {
            for dev in self.restore_paired() {
                crate::dbus::device_added(self, &dev);
            }
            self.announce_burst();
        } else {
            let gone: Vec<Rc<Device>> = self.devices.borrow().values().filter(|d| identity::is_computer(&d.st.borrow().kind)).cloned().collect();
            for dev in gone {
                let (links, waiting) = {
                    let mut st = dev.st.borrow_mut();
                    (std::mem::take(&mut st.links), st.pairing_link.take())
                };
                if let Some(link) = waiting {
                    self.notifier.close(&pair_key(&dev.id, link));
                }
                // A file on its way either way stops too, while the device is still on the bus to say so
                let running: Vec<String> = self.transfers.borrow().iter().filter(|(_, t)| t.device == dev.id).map(|(id, _)| id.clone()).collect();
                for id in running {
                    self.cancel_transfer_by_id(&id);
                }
                self.remove_device(&dev);
                crate::dbus::device_removed(self, &dev);
                for link in links {
                    link.close();
                }
            }
        }
        crate::dbus::root_props_changed(self);
    }

    // --- Starting and stopping ---------------------------------------------------------------------------

    /// Listens: the control port, discovery, and the timers. A port that cannot be had is told, and the program
    /// goes on with what it can do (it can still call phones).
    pub fn start(self: &Rc<Self>) {
        let listener = gio::SocketListener::new();
        let mut bound = false;
        for port in self.cfg.control_ports.clone() {
            let port = if *self.cfg.control_ports.start() == 0 { 0 } else { port };
            if port == 0 {
                if let Ok(p) = listener.add_any_inet_port(None::<&glib::Object>) {
                    self.tcp_port.set(p);
                    bound = true;
                    break;
                }
            } else if listener.add_inet_port(port, None::<&glib::Object>).is_ok() {
                self.tcp_port.set(port);
                bound = true;
                break;
            }
        }
        if bound {
            self.log(&format!("listening on TCP {}", self.tcp_port.get()));
            *self.listener.borrow_mut() = Some(listener.clone());
            let service = self.clone();
            self.spawn_task(async move {
                loop {
                    match listener.accept_future().await {
                        Ok((connection, _)) => service.on_incoming(connection),
                        Err(_) => break,
                    }
                }
            });
        } else {
            eprintln!("amethystora-connect: no port from {} to {} is free; phones cannot connect to this computer", self.cfg.control_ports.start(), self.cfg.control_ports.end());
            self.raise(Notice {
                key: "error:port".into(),
                summary: tr("The phone link cannot listen"),
                body: tr("Another program is using the ports the phone link needs. Phones can still be reached from here."),
                icon: Some("dialog-warning-symbolic".into()),
                ..Notice::default()
            });
        }
        if self.cfg.listen_udp {
            self.bind_udp();
        }

        // The clock: pairing deadlines, and the timers that retry
        let weak = self.weak.clone();
        glib::timeout_add_seconds_local(1, move || {
            let Some(service) = weak.upgrade() else { return glib::ControlFlow::Break };
            if service.stopping.get() {
                return glib::ControlFlow::Break;
            }
            service.tick();
            glib::ControlFlow::Continue
        });
        // The zones: when firewalld reloads, or an interface changes zone, or firewalld itself comes or goes, the map of
        // which interfaces are home is asked for again. A map that was left as it was would let a network that has
        // stopped being home go on being talked to, or the reverse.
        if let Some(bus) = self.home.bus().cloned() {
            let pending = Rc::new(Cell::new(false));
            let ask = {
                let weak = self.weak.clone();
                move || {
                    let Some(service) = weak.upgrade() else { return };
                    if pending.replace(true) {
                        return;
                    }
                    let (pending, weak) = (pending.clone(), service.weak.clone());
                    glib::timeout_add_local_once(Duration::from_millis(300), move || {
                        pending.set(false);
                        if let Some(service) = weak.upgrade() {
                            let again = service.clone();
                            service.spawn_task(async move { again.home.refresh().await });
                        }
                    });
                }
            };
            let ask = Rc::new(ask);
            let (on_signal, on_owner, ask_connection) = (ask.clone(), ask.clone(), ask);
            #[allow(deprecated)] // the stream form would need the program to poll it for callbacks this small
            {
                // NetworkManager: a connection's settings changed, which is what `nmcli connection modify` does when a network
                // is named home, and nothing else says
                let on_connection = ask_connection.clone();
                let _ = bus.signal_subscribe(Some("org.freedesktop.NetworkManager"), Some("org.freedesktop.NetworkManager.Settings.Connection"), Some("Updated"), None, None, gio::DBusSignalFlags::NONE, move |_, _, _, _, _, _| on_connection());
                // These last as long as the connection: their ids are not needed
                let _ = bus.signal_subscribe(Some("org.fedoraproject.FirewallD1"), None, None, None, None, gio::DBusSignalFlags::NONE, move |_, _, _, _, _, _| on_signal());
                let _ = bus.signal_subscribe(Some("org.freedesktop.DBus"), Some("org.freedesktop.DBus"), Some("NameOwnerChanged"), Some("/org/freedesktop/DBus"), Some("org.fedoraproject.FirewallD1"), gio::DBusSignalFlags::NONE, move |_, _, _, _, _, _| on_owner());
            }
        }
        // The network: a change is a reason to announce again
        let monitor = gio::NetworkMonitor::default();
        let weak = self.weak.clone();
        let pending = Rc::new(Cell::new(false));
        monitor.connect_network_changed(move |_, _| {
            let Some(service) = weak.upgrade() else { return };
            if pending.replace(true) {
                return;
            }
            let pending = pending.clone();
            let weak = service.weak.clone();
            glib::timeout_add_seconds_local_once(1, move || {
                pending.set(false);
                if let Some(service) = weak.upgrade() {
                    service.network_changed();
                }
            });
        });
        self.announce_burst();
    }

    pub fn spawn_task(self: &Rc<Self>, task: impl std::future::Future<Output = ()> + 'static) {
        let handle = glib::spawn_future_local(task);
        let mut tasks = self.tasks.borrow_mut();
        tasks.retain(|t| !t.is_terminated_());
        tasks.push(handle);
    }

    /// Ends everything: listeners first, then every connection with a proper TLS close
    pub async fn stop(self: &Rc<Self>) {
        self.stopping.set(true);
        if let Some(listener) = self.listener.borrow_mut().take() {
            listener.close();
        }
        for socket in self.udp.borrow_mut().drain(..) {
            let _ = socket.close();
        }
        for task in self.tasks.borrow_mut().drain(..) {
            task.abort();
        }
        let links: Vec<Rc<Link>> = self.devices.borrow().values().flat_map(|d| d.st.borrow().links.clone()).collect();
        for link in links {
            link.close_politely().await;
        }
    }

    fn tick(self: &Rc<Self>) {
        let now = packet::now_secs();
        let devices: Vec<Rc<Device>> = self.devices.borrow().values().cloned().collect();
        for dev in &devices {
            let idle = dev.st.borrow().machine.is_idle();
            if !idle {
                let paired = dev.paired();
                let outs = dev.st.borrow_mut().machine.step(paired, Input::Tick, now).unwrap_or_default();
                if !outs.is_empty() {
                    self.run_outs(dev, None, outs);
                }
            }
            self.batch_quiet(dev);
        }
        // The zones are asked again every minute, in case a word of their change was missed
        let zone_tick = self.zone_tick.get() + 1;
        self.zone_tick.set(zone_tick);
        if zone_tick % 60 == 0 && self.home.bus().is_some() {
            let service = self.clone();
            self.spawn_task(async move { service.home.refresh().await });
        }
        // Paired phones that are away are called at their last address: every five seconds for a minute after
        // an event, then every thirty
        let tick = self.reach_tick.get() + 1;
        self.reach_tick.set(tick);
        let recent = self.reach_epoch.get().elapsed() < limits::REACH_FIRST_MINUTE;
        if (recent && tick % limits::REACH_FIRST.as_secs() == 0) || (!recent && tick % limits::REACH_LATER.as_secs() == 0) {
            self.reach_known();
        }
    }

    fn network_changed(self: &Rc<Self>) {
        self.reach_epoch.set(Instant::now());
        self.reach_tick.set(0);
        self.announce_burst();
    }

    // --- Discovery ---------------------------------------------------------------------------------------

    fn bind_udp(self: &Rc<Self>) {
        for family in [gio::SocketFamily::Ipv4, gio::SocketFamily::Ipv6] {
            let Ok(socket) = gio::Socket::new(family, gio::SocketType::Datagram, gio::SocketProtocol::Default) else { continue };
            if family == gio::SocketFamily::Ipv6 {
                // A socket of its own for IPv6, so that the IPv4 one is the only one that hears broadcasts
                let _ = socket.set_option(libc::IPPROTO_IPV6, libc::IPV6_V6ONLY, 1);
            } else {
                socket.set_broadcast(true);
            }
            let any = gio::InetAddress::new_any(family);
            let address = gio::InetSocketAddress::new(&any, self.cfg.discovery_port);
            // Address reuse, as another program that speaks the protocol may be on the machine
            if socket.bind(&address, true).is_err() {
                eprintln!("amethystora-connect: cannot listen on UDP {} ({family:?})", self.cfg.discovery_port);
                continue;
            }
            socket.set_blocking(false);
            self.udp.borrow_mut().push(socket.clone());
            let service = self.clone();
            self.spawn_task(async move {
                let mut buffer = vec![0u8; limits::DATAGRAM_READ];
                loop {
                    gio::prelude::SocketExtManual::create_source_future(&socket, glib::IOCondition::IN, None::<&gio::Cancellable>, glib::Priority::DEFAULT).await;
                    loop {
                        match socket.receive_from(&mut buffer[..], None::<&gio::Cancellable>) {
                            Ok((n, from)) => {
                                if let Some((ip, _, scope)) = net::from_gio(&from) {
                                    service.on_datagram(&buffer[..n], ip, scope);
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    if socket.is_closed() {
                        break;
                    }
                }
            });
        }
    }

    fn udp_socket(&self, v6: bool) -> Option<gio::Socket> {
        self.udp.borrow().iter().find(|s| (s.family() == gio::SocketFamily::Ipv6) == v6).cloned()
    }

    fn send_datagram(&self, to: IpAddr, scope: u32, bytes: &[u8]) {
        let Some(socket) = self.udp_socket(to.is_ipv6()) else { return };
        let address = net::socket_address(to, self.cfg.discovery_port, scope);
        let _ = socket.send_to(Some(&address), bytes, None::<&gio::Cancellable>);
    }

    /// The identity, on the networks of the home zone: once now, and again after one and after three seconds,
    /// so that a phone that was asleep for the first is awake for the next
    pub fn announce_burst(self: &Rc<Self>) {
        let service = self.clone();
        self.spawn_task(async move {
            service.home.refresh().await;
            for wait in [0, 1, 2] {
                if wait > 0 {
                    glib::timeout_future_seconds(wait).await;
                }
                if service.stopping.get() {
                    return;
                }
                service.announce_now();
            }
        });
    }

    fn announce_now(&self) {
        let datagram = self.local.borrow().datagram(self.tcp_port.get());
        for iface in netinfo::interfaces() {
            if !iface.up || !iface.broadcast || !self.home.contains(&iface.name) || (iface.loopback && !self.cfg.allow_loopback) {
                continue;
            }
            for network in &iface.v4 {
                self.send_datagram(IpAddr::V4(network.broadcast()), 0, &datagram);
            }
        }
    }

    /// The identity to one address, and the phone's reply is a connection from it
    pub fn announce_to(&self, ip: IpAddr, scope: u32) {
        self.announce_to_port(ip, scope, self.cfg.discovery_port);
    }

    pub fn announce_to_port(&self, ip: IpAddr, scope: u32, port: u16) {
        let datagram = self.local.borrow().datagram(self.tcp_port.get());
        let Some(socket) = self.udp_socket(ip.is_ipv6()) else { return };
        let _ = socket.send_to(Some(&net::socket_address(ip, port, scope)), &datagram, None::<&gio::Cancellable>);
    }

    /// The port the IPv4 discovery socket is bound to
    pub fn udp_port(&self) -> Option<u16> {
        let address = self.udp_socket(false)?.local_address().ok()?;
        net::from_gio(&address).map(|(_, port, _)| port)
    }

    /// The identity to the last address of each paired phone that is not connected
    fn reach_known(self: &Rc<Self>) {
        let targets: Vec<(IpAddr, u32)> = self
            .devices
            .borrow()
            .values()
            .filter_map(|d| {
                let st = d.st.borrow();
                (st.paired && !st.links.iter().any(|l| !l.is_closed())).then_some(st.address).flatten()
            })
            .map(|(ip, _)| (ip, 0))
            .collect();
        let interfaces = netinfo::interfaces();
        for (ip, scope) in targets {
            if netinfo::interface_of(&interfaces, ip, scope).is_some_and(|i| self.home.contains(&i.name)) {
                self.announce_to(ip, scope);
            }
        }
    }

    /// "Look again": announce now, and call the paired phones at their last address
    pub fn look_again(self: &Rc<Self>) {
        self.reach_epoch.set(Instant::now());
        self.reach_tick.set(0);
        self.announce_burst();
        self.reach_known();
    }

    fn on_datagram(self: &Rc<Self>, bytes: &[u8], ip: IpAddr, scope: u32) {
        if self.stopping.get() {
            return;
        }
        let ip = netinfo::unmap(ip);
        let Ok(packet) = Packet::parse(bytes.strip_suffix(b"\n").unwrap_or(bytes)) else { return };
        let Ok(found) = Identity::from_packet(&packet) else { return };
        // Our own broadcast coming back
        if found.device_id == self.me.device_id {
            return;
        }
        let Some(port) = found.tcp_port.filter(|p| self.cfg.datagram_ports.contains(p)) else { return };
        if found.protocol.is_some_and(|v| !limits::PROTOCOLS_ACCEPTED.contains(&v)) {
            return;
        }
        // Only to a neighbour on a network of the home zone, never to an address a stranger could forge from afar
        if !netinfo::local_enough(ip, self.cfg.allow_loopback) {
            return;
        }
        if self.on_home_network(ip, scope) {
            self.datagram_found(found, ip, port, scope);
        } else if self.zone_ask_due() {
            // Not home as far as the last answer goes. Networks are named home with a command that changes a
            // connection's zone and sends no word of it: ask again, and decide again. The phone repeats itself.
            let service = self.clone();
            self.spawn_task(async move {
                service.home.refresh().await;
                if service.on_home_network(ip, scope) {
                    service.datagram_found(found, ip, port, scope);
                }
            });
        }
    }

    fn datagram_found(self: &Rc<Self>, found: Identity, ip: IpAddr, port: u16, scope: u32) {
        // Another computer is not called while the switch is off (link_up decides again on the identity that counts)
        if found.kind.as_deref().is_some_and(|kind| !self.talks_to(kind)) {
            return;
        }
        let known = self.device(&found.device_id);
        if known.is_none() && self.unpaired_count() >= limits::UNPAIRED_DEVICES {
            return;
        }
        if let Some(dev) = &known {
            if dev.st.borrow().links.iter().any(|l| !l.is_closed() && l.peer.0 == ip) {
                return;
            }
        }
        self.connect_out(ip, port, scope, Some(found));
    }

    /// Whether firewalld may be asked about the zones again now: not more than once in two seconds, so that a stream of
    /// connections from a network that is not home cannot make the program ask all the time
    fn zone_ask_due(&self) -> bool {
        let due = self.zone_asked.get().is_none_or(|t| t.elapsed() >= Duration::from_secs(2));
        if due {
            self.zone_asked.set(Some(Instant::now()));
        }
        due
    }

    /// Whether an address is on a network of the home zone: on a subnet of an interface that firewalld puts in
    /// `amethystora-home`. Tailscale, Docker, Waydroid and a machine with its firewall off have addresses that
    /// pass for private ones and are on no such interface; a firewall that cannot be asked has none either.
    pub fn on_home_network(&self, ip: IpAddr, scope: u32) -> bool {
        let interfaces = netinfo::interfaces();
        netinfo::interface_of(&interfaces, ip, scope).is_some_and(|iface| self.home.contains(&iface.name))
    }

    fn unpaired_count(&self) -> usize {
        self.devices.borrow().values().filter(|d| !d.paired()).count()
    }

    /// Calls a peer: the TCP connection, our identity in plain text, TLS as the server
    pub fn connect_out(self: &Rc<Self>, ip: IpAddr, port: u16, scope: u32, expect: Option<Identity>) {
        let id = expect.as_ref().map(|e| e.device_id.clone()).unwrap_or_default();
        if self.attempts_active.get() >= limits::ATTEMPTS_AT_ONCE {
            return;
        }
        {
            let mut attempts = self.attempts.borrow_mut();
            if attempts.get(&ip).is_some_and(|t| t.elapsed() < limits::ATTEMPT_SPACING) {
                return;
            }
            attempts.insert(ip, Instant::now());
            attempts.retain(|_, t| t.elapsed() < Duration::from_secs(60));
        }
        if !self.connecting.borrow_mut().insert((id.clone(), ip)) {
            return;
        }
        self.attempts_active.set(self.attempts_active.get() + 1);
        let service = self.clone();
        self.spawn_task(async move {
            let ctx = service.context();
            let result = net::timed(limits::HANDSHAKE_TOTAL, async {
                let connection = net::connect(SocketAddr::new(ip, port), scope).await?;
                link::as_caller(&ctx, connection, expect).await
            })
            .await;
            service.attempts_active.set(service.attempts_active.get().saturating_sub(1));
            service.connecting.borrow_mut().remove(&(id, ip));
            service.handshake_done(result, ip);
        });
    }

    fn context(self: &Rc<Self>) -> Context {
        let weak = self.weak.clone();
        Context {
            certificate: self.me.tls.clone(),
            local: self.local.borrow().clone(),
            tcp_port: self.tcp_port.get(),
            pin_of: Rc::new(move |id| weak.upgrade().and_then(|s| s.pin_of(id))),
        }
    }

    fn on_incoming(self: &Rc<Self>, connection: gio::SocketConnection) {
        if self.stopping.get() {
            return;
        }
        let Ok((ip, _, _)) = net::peer_of(&connection) else { return };
        // Whatever the firewall lets in, a stranger from afar is not talked to, and neither is one on a network that
        // is not home (the firewall opens more than the home zone: Tailscale's interface is trusted, for one), which
        // is decided in the task, since the zones may have to be asked again first
        let Ok((_, _, scope)) = net::peer_of(&connection) else { return };
        if !netinfo::local_enough(ip, self.cfg.allow_loopback) {
            return;
        }
        if self.handshakes.get() >= limits::HANDSHAKES_AT_ONCE {
            return;
        }
        {
            let mut per = self.per_source.borrow_mut();
            let count = per.entry(ip).or_insert(0);
            if *count >= limits::HANDSHAKES_PER_SOURCE {
                return;
            }
            *count += 1;
        }
        self.handshakes.set(self.handshakes.get() + 1);
        net::tune(&connection.socket());
        let service = self.clone();
        self.spawn_task(async move {
            let mut home = service.on_home_network(ip, scope);
            if !home && service.zone_ask_due() {
                service.home.refresh().await;
                home = service.on_home_network(ip, scope);
            }
            let result = if home {
                let ctx = service.context();
                net::timed(limits::HANDSHAKE_TOTAL, link::as_acceptor(&ctx, connection)).await
            } else {
                Err(Error::Protocol("not a network of the home zone"))
            };
            service.handshakes.set(service.handshakes.get().saturating_sub(1));
            {
                let mut per = service.per_source.borrow_mut();
                if let Some(count) = per.get_mut(&ip) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        per.remove(&ip);
                    }
                }
            }
            service.handshake_done(result, ip);
        });
    }

    fn handshake_done(self: &Rc<Self>, result: net::Result<Opened>, from: IpAddr) {
        match result {
            Ok(opened) => self.link_up(opened),
            Err(Error::Mismatch(id)) => self.certificate_mismatch(&id),
            Err(e) => self.log(&format!("a connection with {from} did not come up: {e}")),
        }
    }

    /// A paired phone's id with another certificate: refused, the pairing kept, the person told (not often)
    fn certificate_mismatch(self: &Rc<Self>, id: &str) {
        let Some(dev) = self.device(id) else { return };
        let told = {
            let mut st = dev.st.borrow_mut();
            let recent = st.mismatch_told.is_some_and(|t| t.elapsed() < Duration::from_secs(600));
            if !recent {
                st.mismatch_told = Some(Instant::now());
            }
            recent
        };
        eprintln!("amethystora-connect: something claiming to be a paired phone presented another certificate and was refused");
        if !told {
            self.raise(Notice {
                key: format!("info:{id}"),
                summary: trf("Something claimed to be “{name}”", &[("name", &dev.name())]),
                body: tr("It did not have the certificate that phone was paired with, so it was refused. If you reinstalled the app on the phone, remove the phone here and pair it again."),
                icon: Some("dialog-warning-symbolic".into()),
                urgent: true,
                ..Notice::default()
            });
        }
    }

    // --- Connections --------------------------------------------------------------------------------------

    fn link_up(self: &Rc<Self>, opened: Opened) {
        if self.stopping.get() {
            return;
        }
        let identity = opened.identity.clone();
        let id = identity.device_id.clone();
        if id == self.me.device_id {
            return;
        }
        let protocol = identity.protocol.unwrap_or(8);
        let name = identity.name.clone().unwrap_or_else(|| tr("Unknown device"));
        let kind = identity.kind.clone().unwrap_or_else(|| "phone".into());
        let stored_kind = self.store.borrow().paired.get(&id).map(|p| p.kind.clone());
        let paired = stored_kind.is_some();
        // Another computer, by what it says now or by what it was paired as, is let go while the switch is off: a
        // paired computer that calls itself a phone stays a computer
        if !self.talks_to(&kind) || stored_kind.is_some_and(|k| !self.talks_to(&k)) {
            self.log(&format!("{id} is another computer, and computers are off; let go"));
            opened.close();
            return;
        }

        let (dev, is_new) = match self.device(&id) {
            Some(dev) => (dev, false),
            None => {
                if self.unpaired_count() >= limits::UNPAIRED_DEVICES {
                    return;
                }
                (self.make_device(&id, &name, &kind, protocol), true)
            }
        };
        // A pairing that is waiting stays with its connection (spec 5.10). Another connection with the same id is taken
        // (two devices that find each other at once each call the other, and a phone that closes the first when the
        // second comes would otherwise lose its request), but it can take nothing over: the request, its prompt, the
        // phone's name and what it can do stay as the first connection made them, and a pairing packet on the
        // second is ignored until the first has ended. A host that read the phone's id from its broadcast could
        // otherwise take the person's prompt over while they compare the key.
        let pending = !paired && {
            let st = dev.st.borrow();
            !st.machine.is_idle() && st.links.iter().any(|l| !l.is_closed())
        };
        // Whether this connection's identity was read inside the TLS session the pin vouches for. A paired
        // phone's record is kept up to date from such an identity alone: a datagram anyone can forge says nothing.
        let authenticated = opened.authenticated;
        if paired {
            let stored = self.store.borrow().paired.get(&id).map(|p| p.protocol).unwrap_or(protocol);
            if protocol < stored {
                self.log(&format!("{id} came back with an older protocol than it paired with; refused"));
                opened.close();
                return;
            }
        }
        let was_connected = dev.connected();
        let (link, reader) = Link::start(self.next_id(), opened);
        let address = (link.peer.0, link.peer.1);
        {
            let mut st = dev.st.borrow_mut();
            // The newest connection is written to; of the others, the oldest is closed only when there are too many, and
            // the one a pairing request is on is never the one that goes. A healthy connection is not closed because a
            // newer one came: two devices that find each other at once each call the other, and Fedora's own KDE Connect
            // takes the closing of the older of two connections for the device leaving, and forgets it.
            st.links.retain(|l| !l.is_closed());
            let waiting = st.pairing_link;
            while st.links.len() >= limits::LINKS_PER_DEVICE {
                let Some(at) = st.links.iter().position(|l| Some(l.id) != waiting) else { break };
                st.links.remove(at).close();
            }
            if !paired && !pending {
                st.machine = pairing::Machine::new(protocol);
                st.pairing_link = None;
                st.pairing_key.clear();
            }
            if (paired && authenticated) || (!paired && !pending) {
                st.name = name;
                // A paired computer stays one whatever it says on a later connection: what waits for Accept (ask.rs) is decided
                // by what it was, and a device's word is not a reason to let its files arrive by themselves
                if !(paired && identity::is_computer(&st.kind) && !identity::is_computer(&kind)) {
                    st.kind = kind;
                }
                st.protocol = protocol;
                st.incoming = identity.incoming.clone().unwrap_or_default();
                st.outgoing = identity.outgoing.clone().unwrap_or_default();
                // The port a phone connected from is not where to call it: its own is in the identity
                st.address = Some((address.0, identity.tcp_port.unwrap_or(limits::DISCOVERY_PORT)));
            } else if paired {
                st.address = Some((address.0, identity.tcp_port.unwrap_or(limits::DISCOVERY_PORT)));
            }
            st.links.push(link.clone());
            st.unpaired_since_connect = false;
        }
        if paired && authenticated {
            self.update_paired_record(&dev);
        }
        self.log(&format!("{} connected from {}", dev.id, address.0));
        crate::dbus::props_changed(self, &dev);
        if is_new {
            crate::dbus::device_added(self, &dev);
        }
        if paired && !was_connected {
            self.on_connected(&dev);
        }
        let service = self.clone();
        self.spawn_task(async move { service.read_loop(dev, link, reader).await });
    }

    /// What is kept of a paired phone is kept up to date: its name, what it can do, where it was
    fn update_paired_record(&self, dev: &Rc<Device>) {
        let st = dev.st.borrow();
        let mut store = self.store.borrow_mut();
        let Some(record) = store.paired.get_mut(&dev.id) else { return };
        let changed = record.name != st.name || record.kind != st.kind || record.protocol != st.protocol || record.incoming != st.incoming || record.outgoing != st.outgoing || record.address != st.address;
        if changed {
            record.name = st.name.clone();
            record.kind = st.kind.clone();
            record.protocol = st.protocol;
            record.incoming = st.incoming.clone();
            record.outgoing = st.outgoing.clone();
            record.address = st.address;
            if let Err(e) = store.save() {
                eprintln!("amethystora-connect: cannot keep the paired phones: {e}");
            }
        }
    }

    async fn read_loop(self: Rc<Self>, dev: Rc<Device>, link: Rc<Link>, mut reader: link::Reader) {
        loop {
            let max = if dev.paired() { limits::CONTROL_LINE } else { limits::UNPAIRED_LINE };
            let line = match reader.next(max).await {
                Ok(Some(line)) => line,
                Ok(None) => break,
                Err(e) => {
                    self.log(&format!("{}: {e}", dev.id));
                    break;
                }
            };
            match Packet::parse(&line) {
                Ok(packet) => self.on_packet(&dev, &link, packet),
                Err(_) => break,
            }
        }
        link.close();
        self.link_closed(&dev, &link);
    }

    fn link_closed(self: &Rc<Self>, dev: &Rc<Device>, link: &Rc<Link>) {
        let (gone, paired, withdraw_pairing, carried_on) = {
            let mut st = dev.st.borrow_mut();
            st.links.retain(|l| l.id != link.id);
            let withdraw = st.pairing_link == Some(link.id);
            let mut carried_on = None;
            if withdraw {
                // A request goes on over another connection of the same device (the one with the same certificate): a
                // phone that is called twice closes one of the two, and it may be the one the request went over
                let other = st.links.iter().find(|l| !l.is_closed() && l.peer_der == link.peer_der).cloned();
                match (other, st.machine.is_idle()) {
                    (Some(other), false) => {
                        st.pairing_link = Some(other.id);
                        carried_on = Some(other.id);
                    }
                    _ => {
                        // The key belongs to the connection it was made on
                        st.machine = pairing::Machine::new(st.protocol);
                        st.pairing_link = None;
                        st.pairing_key.clear();
                    }
                }
            }
            (!st.links.iter().any(|l| !l.is_closed()), st.paired, withdraw, carried_on)
        };
        self.log(&format!("{} disconnected", dev.id));
        if withdraw_pairing {
            self.notifier.close(&pair_key(&dev.id, link.id));
            if carried_on.is_some() {
                self.raise_pairing_prompt_again(dev);
            }
        }
        // A computer the switch let go is off the bus already, and its id may be a new object's by now
        let registered = self.device(&dev.id).is_some_and(|d| Rc::ptr_eq(&d, dev));
        if gone {
            self.on_disconnected(dev);
            if !registered {
                return;
            }
            if !paired && !self.stopping.get() {
                self.remove_device(dev);
                crate::dbus::device_removed(self, dev);
                return;
            }
        }
        if registered {
            crate::dbus::props_changed(self, dev);
        }
    }

    // --- Pairing -----------------------------------------------------------------------------------------

    /// A `kdeconnect.pair` packet
    pub fn on_pair_packet(self: &Rc<Self>, dev: &Rc<Device>, link: &Rc<Link>, packet: &Packet) {
        // A boolean, and nothing else: "false" as a string is not false
        let Some(pair) = packet.bool("pair") else { return };
        let timestamp = packet::int_or_text_of(&packet.body, "timestamp");
        let paired = dev.paired();
        if pair && !paired {
            // A request from a device that is not paired is a prompt for the person: so many from one address in
            // a minute, and so many prompts at a time, and no more. Reconnecting does not start the count again.
            let starts_a_prompt = dev.st.borrow().machine.is_idle();
            if !self.pair_request_allowed(link.ip(), starts_a_prompt) {
                return;
            }
        }
        {
            let st = dev.st.borrow();
            // One pairing at a time, on the connection it began on or on another of the same device: one that
            // presents the same certificate (a phone that is called twice may answer on either of its connections),
            // and not one that only claims the same id
            let on_pairing_connection = |waiting: u64| waiting == link.id || st.links.iter().any(|l| l.id == waiting && l.peer_der == link.peer_der);
            if !paired && !st.machine.is_idle() && st.pairing_link.is_some_and(|waiting| !on_pairing_connection(waiting)) {
                return;
            }
        }
        let outs = {
            let mut st = dev.st.borrow_mut();
            match st.machine.step(paired, Input::Peer { pair, timestamp }, packet::now_secs()) {
                Ok(outs) => outs,
                Err(_) => return,
            }
        };
        self.run_outs(dev, Some(link), outs);
    }

    /// Counts a pairing request against its source address, and a new prompt against the prompts already showing
    fn pair_request_allowed(&self, from: IpAddr, starts_a_prompt: bool) -> bool {
        let now = Instant::now();
        let mut sources = self.pair_sources.borrow_mut();
        sources.retain(|_, times| {
            times.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
            !times.is_empty()
        });
        let times = sources.entry(from).or_default();
        if times.len() >= limits::PAIR_REQUESTS_PER_MINUTE {
            return false;
        }
        if starts_a_prompt {
            let showing = self.devices.borrow().values().filter(|d| !d.st.borrow().machine.is_idle()).count();
            if showing >= limits::PAIR_PROMPTS_AT_ONCE {
                return false;
            }
        }
        times.push_back(now);
        true
    }

    /// Something other than a pairing packet from a device that is not paired: told once, on this connection,
    /// when no pairing is going on, that it is not paired. Dropped.
    pub fn unpaired_packet(self: &Rc<Self>, dev: &Rc<Device>, link: &Rc<Link>) {
        if link.told_unpaired.replace(true) {
            return;
        }
        if dev.st.borrow().machine.is_idle() {
            link.send(&Packet::new(packet::PAIR, json!({ "pair": false })));
        }
    }

    /// The person asks the phone to pair
    pub fn pair(self: &Rc<Self>, id: &str) -> ApiResult<()> {
        let dev = self.device(id).ok_or_else(|| ApiError::unavailable("no such device"))?;
        let link = dev.link().ok_or_else(|| ApiError::unavailable("the phone is not connected"))?;
        if dev.paired() {
            return Err(ApiError::unavailable("already paired"));
        }
        let outs = {
            let mut st = dev.st.borrow_mut();
            st.pairing_link = Some(link.id);
            let outs = st.machine.step(false, Input::Ask, packet::now_secs());
            if outs.is_err() {
                st.pairing_link = None;
            }
            outs.map_err(|r| ApiError::unavailable(&format!("{r:?}")))?
        };
        self.run_outs(&dev, Some(&link), outs);
        Ok(())
    }

    /// An answer to the request that is waiting. `only_link` is the connection a notification's button belongs
    /// to: an answer for any other connection than the one the request is on is not taken.
    fn answer(self: &Rc<Self>, id: &str, input: Input, only_link: Option<u64>) -> ApiResult<()> {
        let dev = self.device(id).ok_or_else(|| ApiError::unavailable("no such device"))?;
        let paired = dev.paired();
        let link = {
            let st = dev.st.borrow();
            if only_link.is_some() && st.pairing_link != only_link {
                return Err(ApiError::unavailable("that request is no longer the one waiting"));
            }
            st.pairing_link.and_then(|lid| st.links.iter().find(|l| l.id == lid).cloned())
        }
        .or_else(|| dev.link());
        let outs = dev.st.borrow_mut().machine.step(paired, input, packet::now_secs()).map_err(|r: Refusal| ApiError::unavailable(&format!("{r:?}")))?;
        self.run_outs(&dev, link.as_ref(), outs);
        Ok(())
    }

    pub fn accept_pairing(self: &Rc<Self>, id: &str) -> ApiResult<()> {
        self.answer(id, Input::Accept, None)
    }

    pub fn reject_pairing(self: &Rc<Self>, id: &str) -> ApiResult<()> {
        self.answer(id, Input::Reject, None)
    }

    /// Ends a pairing, or cancels a request that is waiting; a phone that is not connected is forgotten here only
    pub fn unpair(self: &Rc<Self>, id: &str) -> ApiResult<()> {
        let dev = self.device(id).ok_or_else(|| ApiError::unavailable("no such device"))?;
        if dev.paired() && dev.link().is_none() {
            self.forget_pairing(&dev, false);
            return Ok(());
        }
        self.answer(id, Input::Cancel, None)
    }

    /// Does what the pairing machine says
    pub fn run_outs(self: &Rc<Self>, dev: &Rc<Device>, link: Option<&Rc<Link>>, outs: Vec<Out>) {
        let link = link.cloned().or_else(|| dev.link());
        let mut failed = false;
        for out in outs {
            match out {
                Out::Send { pair, timestamp } => {
                    if failed && pair {
                        continue;
                    }
                    let mut body = json!({ "pair": pair });
                    if let (Some(map), Some(t)) = (body.as_object_mut(), timestamp) {
                        map.insert("timestamp".into(), t.into());
                    }
                    if let Some(link) = &link {
                        link.send(&Packet::new(packet::PAIR, body));
                    }
                }
                Out::Prompt { outgoing, key_timestamp, deadline } => {
                    let Some(link) = &link else { continue };
                    let key = pairing::verification_key(&self.me.spki, &link.peer_spki, key_timestamp);
                    {
                        let mut st = dev.st.borrow_mut();
                        st.pairing_link = Some(link.id);
                        st.pairing_key = key.clone();
                    }
                    self.pairing_prompt(dev, &key, outgoing, deadline);
                    crate::dbus::props_changed(self, dev);
                }
                Out::Withdraw => {
                    let waiting = {
                        let mut st = dev.st.borrow_mut();
                        st.pairing_key.clear();
                        st.pairing_link.take()
                    };
                    if let Some(lid) = waiting {
                        self.notifier.close(&pair_key(&dev.id, lid));
                    }
                    crate::dbus::props_changed(self, dev);
                }
                Out::StorePin => {
                    let Some(link) = &link else {
                        failed = true;
                        continue;
                    };
                    if let Err(e) = self.store_pin(dev, link) {
                        eprintln!("amethystora-connect: cannot keep the pairing: {e}");
                        failed = true;
                        link.send(&Packet::new(packet::PAIR, json!({ "pair": false })));
                        self.raise(Notice {
                            key: format!("info:{}", dev.id),
                            summary: tr("Pairing failed"),
                            body: tr("The pairing could not be saved on this computer."),
                            icon: Some("dialog-error-symbolic".into()),
                            ..Notice::default()
                        });
                    }
                }
                Out::DropPin | Out::Unpaired => {
                    // DropPin comes before Unpaired; the work is done once
                    if matches!(out, Out::Unpaired) {
                        self.forget_pairing(dev, true);
                    }
                }
                Out::Paired => {
                    if failed {
                        continue;
                    }
                    let waiting = {
                        let mut st = dev.st.borrow_mut();
                        st.paired = true;
                        st.pairing_key.clear();
                        st.pairing_link.take()
                    };
                    if let Some(lid) = waiting {
                        self.notifier.close(&pair_key(&dev.id, lid));
                    }
                    self.log(&format!("{} paired", dev.id));
                    crate::dbus::props_changed(self, dev);
                    self.on_connected(dev);
                }
                Out::Rejected => {
                    self.raise(Notice {
                        key: format!("info:{}", dev.id),
                        summary: trf("“{name}” did not accept the pairing", &[("name", &dev.name())]),
                        icon: Some("dialog-information-symbolic".into()),
                        ..Notice::default()
                    });
                }
                Out::TimedOut | Out::Confirmed => {}
                Out::ClockSkew { .. } => {
                    self.raise(Notice {
                        key: format!("info:{}", dev.id),
                        summary: tr("Pairing failed"),
                        body: tr("The clocks of this computer and the phone differ by more than half an hour. Set both to the right time and try again."),
                        icon: Some("dialog-warning-symbolic".into()),
                        urgent: true,
                        ..Notice::default()
                    });
                }
            }
        }
    }

    fn store_pin(&self, dev: &Rc<Device>, link: &Rc<Link>) -> std::io::Result<()> {
        let st = dev.st.borrow();
        let mut store = self.store.borrow_mut();
        store.paired.insert(
            dev.id.clone(),
            Paired {
                name: st.name.clone(),
                kind: st.kind.clone(),
                protocol: st.protocol,
                incoming: st.incoming.clone(),
                outgoing: st.outgoing.clone(),
                pin: link.peer_der.clone(),
                address: st.address,
                paired_at: packet::now_secs(),
            },
        );
        let saved = store.save();
        if saved.is_err() {
            store.paired.remove(&dev.id);
        }
        saved
    }

    /// A pairing is over: the pin goes, and so does everything kept for the phone
    pub fn forget_pairing(self: &Rc<Self>, dev: &Rc<Device>, told_by_phone: bool) {
        {
            let mut store = self.store.borrow_mut();
            store.paired.remove(&dev.id);
            if let Err(e) = store.save() {
                eprintln!("amethystora-connect: cannot keep the paired phones: {e}");
            }
        }
        let name = dev.name();
        self.clear_device_state(dev);
        let _ = std::fs::remove_file(self.contacts_path(&dev.id));
        let _ = std::fs::remove_dir_all(self.icons_dir(&dev.id));
        self.clip.borrow_mut().forget(&dev.id);
        let aborted: Vec<String> = self.transfers.borrow().iter().filter(|(_, t)| t.device == dev.id).map(|(id, _)| id.clone()).collect();
        for id in aborted {
            self.cancel_transfer_by_id(&id);
        }
        let (connected, waiting) = {
            let mut st = dev.st.borrow_mut();
            st.paired = false;
            st.machine = pairing::Machine::new(st.protocol);
            st.pairing_key.clear();
            (st.links.iter().any(|l| !l.is_closed()), st.pairing_link.take())
        };
        if let Some(lid) = waiting {
            self.notifier.close(&pair_key(&dev.id, lid));
        }
        self.log(&format!("{} unpaired", dev.id));
        if told_by_phone {
            self.raise(Notice {
                key: format!("info:{}", dev.id),
                summary: trf("“{name}” was unpaired", &[("name", &name)]),
                body: tr("The phone ended the pairing."),
                icon: Some("dialog-information-symbolic".into()),
                ..Notice::default()
            });
        }
        crate::dbus::props_changed(self, dev);
        if !connected {
            self.remove_device(dev);
            crate::dbus::device_removed(self, dev);
        }
    }

    /// The prompt of a request that has moved to another connection, with the time it has left
    fn raise_pairing_prompt_again(&self, dev: &Rc<Device>) {
        let (key, phase) = {
            let st = dev.st.borrow();
            (st.pairing_key.clone(), st.machine.phase())
        };
        match phase {
            pairing::Phase::Incoming { deadline, .. } => self.pairing_prompt(dev, &key, false, deadline),
            pairing::Phase::Outgoing { deadline, .. } => self.pairing_prompt(dev, &key, true, deadline),
            pairing::Phase::Idle => {}
        }
    }

    fn pairing_prompt(&self, dev: &Rc<Device>, key: &str, outgoing: bool, deadline: i64) {
        let name = dev.name();
        let shown = format!("{} {}", &key[..4], &key[4..]);
        let older = dev.st.borrow().protocol < 8;
        let computer = identity::is_computer(&dev.st.borrow().kind);
        let mut body = match (outgoing, computer) {
            (true, false) => trf("Key {key}. Compare it with the key on the phone, and cancel if they differ.", &[("key", &shown)]),
            (false, false) => trf("Key {key}. Compare it with the key on the phone, and accept only if they are the same.", &[("key", &shown)]),
            (true, true) => trf("Key {key}. Compare it with the key on the other computer, and cancel if they differ.", &[("key", &shown)]),
            (false, true) => trf("Key {key}. Compare it with the key on the other computer, and accept only if they are the same.", &[("key", &shown)]),
        };
        if older {
            body.push('\n');
            body.push_str(&tr("This phone uses an older version of the protocol, which protects the pairing less well."));
        }
        let actions = if outgoing {
            vec![("cancel".to_string(), tr("Cancel"))]
        } else {
            vec![("accept".to_string(), tr("Accept")), ("reject".to_string(), tr("Reject"))]
        };
        let wait = (deadline - packet::now_secs()).clamp(1, limits::PAIR_WAIT_SECS) as i32 * 1000;
        let Some(link_id) = dev.st.borrow().pairing_link else { return };
        self.raise(Notice {
            key: pair_key(&dev.id, link_id),
            summary: if outgoing { trf("Pairing with “{name}”", &[("name", &name)]) } else { trf("“{name}” wants to pair", &[("name", &name)]) },
            body,
            icon: Some("security-medium-symbolic".into()),
            actions,
            // An ordinary notification: a critical one goes through Do Not Disturb and stays, which is what a
            // flood of requests would want. The Phones page of the app shows the request as well.
            urgent: false,
            timeout_ms: wait,
            sound: None,
        });
    }

    // --- Notifications -----------------------------------------------------------------------------------

    pub fn raise(&self, notice: Notice) {
        self.notifier.notify(notice);
    }

    /// The person did something with a notification
    fn on_notification_event(self: &Rc<Self>, event: Event) {
        let (key, action) = match &event {
            Event::Action { key, action, token } => {
                *self.activation.borrow_mut() = token.clone();
                (key.as_str(), Some(action.as_str()))
            }
            Event::Dismissed { key } => (key.as_str(), None),
        };
        let mut parts = key.splitn(3, ':');
        let (kind, id, rest) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let Some(dev) = self.device(id) else { return };
        match (kind, action) {
            // The key of a prompt ends in the connection it was raised for: only that connection's request is answered
            ("pair", Some(what)) => {
                let Ok(link_id) = rest.parse::<u64>() else { return };
                let input = match what {
                    "accept" => Input::Accept,
                    "reject" => Input::Reject,
                    "cancel" => Input::Cancel,
                    _ => return,
                };
                let _ = self.answer(id, input, Some(link_id));
            }
            ("n", _) => self.on_phone_notification_event(&dev, rest, action),
            ("call", Some("mute")) => {
                let _ = self.mute_ringer(id);
            }
            ("got", Some(what)) | ("batch", Some(what)) => self.on_received_event(rest, what),
            ("photo", Some("cancel")) => self.cancel_photo(&dev, rest),
            // Dismissing the card is a no, as Decline is
            ("ask", what) => self.on_ask_event(&dev, rest, what.unwrap_or("decline")),
            ("text", Some(what)) => self.on_text_event(&dev, rest, what),
            ("xfer", Some("cancel")) => self.cancel_transfer_by_id(rest),
            _ => {}
        }
    }

    pub fn launch_app(&self, args: &[&str]) {
        let mut argv: Vec<&str> = self.cfg.app_command.iter().map(String::as_str).collect();
        argv.extend_from_slice(args);
        self.launch(&argv);
    }

    /// Opens an address or a file with whatever the person has for it. Only ever called for a button the person pressed.
    pub fn open_uri(&self, uri: &str) {
        self.launch(&[self.cfg.open_command.as_str(), "open", uri]);
    }

    /// Starts a program as an argument vector (never through a shell), outside this service: in a unit of the user's
    /// manager if there is a launcher, so that it outlives the service and does not run in its sandbox. The
    /// shell's activation token of the button just pressed goes with it, so that its window comes to the front.
    fn launch(&self, command: &[&str]) {
        let token = self.activation.borrow_mut().take().filter(|t| t.len() <= 256 && !t.chars().any(char::is_control));
        let mut argv: Vec<String> = self.cfg.launcher.clone();
        if let (false, Some(token)) = (argv.is_empty(), &token) {
            argv.push(format!("--setenv=XDG_ACTIVATION_TOKEN={token}"));
        }
        if !argv.is_empty() {
            argv.push("--".to_string());
        }
        argv.extend(command.iter().map(|s| s.to_string()));
        let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
        if let Some(token) = &token {
            launcher.setenv("XDG_ACTIVATION_TOKEN", token, true);
        }
        let refs: Vec<&std::ffi::OsStr> = argv.iter().map(|a| std::ffi::OsStr::new(a.as_str())).collect();
        if let Err(e) = launcher.spawn(&refs) {
            eprintln!("amethystora-connect: cannot start {}: {}", argv[0], e.message());
        }
    }

    // --- The clipboard -----------------------------------------------------------------------------------

    /// The module says this computer's clipboard changed
    pub fn push_clipboard(self: &Rc<Self>, text: &str) {
        if !self.clip_send.get() {
            return;
        }
        let targets: Vec<String> = self.devices.borrow().values().filter(|d| d.paired() && d.connected() && d.takes("kdeconnect.clipboard")).map(|d| d.id.clone()).collect();
        let send = self.clip.borrow_mut().local_changed(text, packet::now_ms(), &targets);
        for id in send {
            if let Some(dev) = self.device(&id) {
                self.send_to(&dev, "kdeconnect.clipboard", json!({ "content": text }));
            }
        }
    }

    pub fn set_clipboard_switches(self: &Rc<Self>, send: bool, receive: bool) {
        self.clip_send.set(send);
        self.clip_receive.set(receive);
        crate::dbus::root_props_changed(self);
    }

    /// The switches music and clicker. What this computer lists changes with them, and a phone reads the list only
    /// when it connects: each connected phone is let go and called again, and comes back within seconds with the
    /// features as they are now. A test that gave the program lists of its own keeps them.
    pub fn set_media_switches(self: &Rc<Self>, music: bool, clicker: bool) {
        if self.music.get() == music && self.clicker.get() == clicker {
            return;
        }
        self.music.set(music);
        self.clicker.set(clicker);
        if self.cfg.capabilities.is_none() {
            let mut local = self.local.borrow_mut();
            (local.incoming, local.outgoing) = identity::advertised(music, clicker, self.photo.get());
        }
        let devices: Vec<Rc<Device>> = self.devices.borrow().values().cloned().collect();
        let mut reconnect = Vec::new();
        for dev in &devices {
            {
                let mut st = dev.st.borrow_mut();
                st.offers = device::Offers { music, clicker, photo: self.photo.get() };
                if !music {
                    st.players.clear();
                }
            }
            if dev.paired() && dev.connected() && self.cfg.capabilities.is_none() {
                reconnect.extend(dev.st.borrow().links.iter().filter(|l| !l.is_closed()).cloned());
            }
            crate::dbus::props_changed(self, dev);
        }
        self.sync_players();
        // Each connection's reading ends with it, and says the phone went; the phone is then called at once
        if !reconnect.is_empty() {
            for link in &reconnect {
                link.close();
            }
            self.look_again();
        }
    }

    /// The switch photo: this computer lists the photo request, and a phone that takes it is offered Snap with your phone. Like
    /// music, what is listed changes, so each connected phone is let go and called again.
    pub fn set_photo(self: &Rc<Self>, on: bool) {
        if self.photo.get() == on {
            return;
        }
        self.photo.set(on);
        if self.cfg.capabilities.is_none() {
            let mut local = self.local.borrow_mut();
            (local.incoming, local.outgoing) = identity::advertised(self.music.get(), self.clicker.get(), on);
        }
        let devices: Vec<Rc<Device>> = self.devices.borrow().values().cloned().collect();
        let mut reconnect = Vec::new();
        for dev in &devices {
            {
                let mut st = dev.st.borrow_mut();
                st.offers.photo = on;
                if !on {
                    st.photo = Default::default();
                }
            }
            if dev.paired() && dev.connected() && self.cfg.capabilities.is_none() {
                reconnect.extend(dev.st.borrow().links.iter().filter(|l| !l.is_closed()).cloned());
            }
            crate::dbus::props_changed(self, dev);
        }
        if !reconnect.is_empty() {
            for link in &reconnect {
                link.close();
            }
            self.look_again();
        }
    }

    pub fn set_photo_folder(self: &Rc<Self>, folder: PhotoFolder) {
        self.photo_folder.set(folder);
        crate::dbus::root_props_changed(self);
    }

    pub fn set_name(self: &Rc<Self>, configured: &str, pretty_host: Option<&str>) {
        let name = identity::default_name(configured, pretty_host, &glib::host_name());
        if self.local.borrow().name != name {
            self.local.borrow_mut().name = name;
            crate::dbus::root_props_changed(self);
            self.announce_burst();
        }
    }

    // --- Sending -----------------------------------------------------------------------------------------

    /// A packet to a paired, connected phone that lists the type among those it takes
    pub fn send_to(&self, dev: &Rc<Device>, kind: &str, body: serde_json::Value) -> bool {
        if !dev.paired() || !dev.takes(kind) {
            return false;
        }
        match dev.link() {
            Some(link) => link.send(&Packet::new(kind, body)),
            None => false,
        }
    }

    /// Same, as an API call: a phone that cannot take it is an error to the caller
    pub fn require_send(&self, id: &str, kind: &str, body: serde_json::Value) -> ApiResult<()> {
        let dev = self.device(id).ok_or_else(|| ApiError::unavailable("no such device"))?;
        if !dev.paired() {
            return Err(ApiError::unavailable("the phone is not paired"));
        }
        if !dev.connected() {
            return Err(ApiError::unavailable("the phone is not connected"));
        }
        if !dev.takes(kind) {
            return Err(ApiError::unavailable("the phone does not take this"));
        }
        if self.send_to(&dev, kind, body) { Ok(()) } else { Err(ApiError::unavailable("the connection is closed")) }
    }
}

/// The task handles carry a way to ask whether they have ended
trait Ended {
    fn is_terminated_(&self) -> bool;
}

impl Ended for glib::JoinHandle<()> {
    fn is_terminated_(&self) -> bool {
        futures_util::future::FusedFuture::is_terminated(self)
    }
}

/// The key of the notification that asks the person about a pairing: one for each connection, so that a prompt
/// belongs to the connection that made the request, and goes when it goes
pub fn pair_key(device: &str, link: u64) -> String {
    format!("pair:{device}:{link}")
}
