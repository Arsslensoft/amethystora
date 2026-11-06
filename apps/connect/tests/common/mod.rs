//! What the integration tests share: a copy of the program that plays a computer or a phone, in one process,
//! and the small helpers to wait and look.
#![allow(dead_code)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use amethystora_connect::ask::AskFirst;
use amethystora_connect::device::Device;
use amethystora_connect::notify::{Notice, Recorder};
use amethystora_connect::packet::Packet;
use amethystora_connect::service::{Config, Service, Settings};
use gio::prelude::*;

pub struct Peer {
    pub service: Rc<Service>,
    pub notices: Rc<Recorder>,
    pub seen: Rc<RefCell<Vec<(String, Packet)>>>,
    pub dir: PathBuf,
}

impl Peer {
    pub fn id(&self) -> String {
        self.service.me.device_id.clone()
    }

    pub fn port(&self) -> u16 {
        self.service.tcp_port.get()
    }

    pub fn device(&self, id: &str) -> Rc<Device> {
        self.service.device(id).unwrap_or_else(|| panic!("{} does not know {id}", self.service.name()))
    }

    pub fn saw(&self, kind: &str) -> Vec<Packet> {
        self.seen.borrow().iter().filter(|(_, p)| p.kind == kind).map(|(_, p)| p.clone()).collect()
    }

    pub fn downloads(&self) -> PathBuf {
        self.dir.join("downloads")
    }

    pub fn pictures(&self) -> PathBuf {
        self.dir.join("pictures")
    }
}

pub fn phone_capabilities() -> (Vec<String>, Vec<String>) {
    let list = |l: &[&str]| l.iter().map(|s| s.to_string()).collect();
    (
        list(&[
            "kdeconnect.ping",
            "kdeconnect.battery.request",
            "kdeconnect.notification.request",
            "kdeconnect.notification.action",
            "kdeconnect.notification.reply",
            "kdeconnect.share.request",
            "kdeconnect.share.request.update",
            "kdeconnect.clipboard",
            "kdeconnect.clipboard.connect",
            "kdeconnect.contacts.request_all_uids_timestamps",
            "kdeconnect.contacts.request_vcards_by_uid",
            "kdeconnect.sms.request",
            "kdeconnect.sms.request_conversation",
            "kdeconnect.sms.request_conversations",
            "kdeconnect.telephony.request_mute",
        ]),
        list(&[
            "kdeconnect.ping",
            "kdeconnect.battery",
            "kdeconnect.notification",
            "kdeconnect.share.request",
            "kdeconnect.clipboard",
            "kdeconnect.clipboard.connect",
            "kdeconnect.contacts.response_uids_timestamps",
            "kdeconnect.contacts.response_vcards",
            "kdeconnect.sms.messages",
            "kdeconnect.telephony",
            "kdeconnect.connectivity_report",
        ]),
    )
}

pub fn make(root: &Path, name: &str, kind: &str, phone: bool, udp: bool) -> Peer {
    make_with(root, name, kind, phone, udp, |_| {})
}

/// The same, with a last word on its configuration
pub fn make_with(root: &Path, name: &str, kind: &str, phone: bool, udp: bool, tweak: impl FnOnce(&mut Config)) -> Peer {
    let (cfg, dir) = config_for(root, name, kind, phone, udp, tweak);
    let notices = Rc::new(Recorder::default());
    // A copy playing a phone talks to computers, as a phone's app does, and takes a file at once (R7-27's card is this computer's); a computer talks to computers only while
    // its switch is on, off here as the image ships
    let service = Service::new(cfg, Box::new(notices.clone()), Settings { name: name.into(), computers: phone, ask_first: if phone { AskFirst::Never } else { AskFirst::default() }, ..Settings::default() }, None).unwrap();
    started(service, notices, dir)
}

/// What a copy of the program is configured with in these tests, and the folder it keeps its things in
pub fn config_for(root: &Path, name: &str, kind: &str, phone: bool, udp: bool, tweak: impl FnOnce(&mut Config)) -> (Config, PathBuf) {
    let dir = root.join(name.replace(' ', "-"));
    let app = dir.join("app.sh");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&app, format!("#!/bin/sh\necho \"$@\" >> {}/app.log\n", dir.display())).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&app, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut cfg = Config::production();
    cfg.config_dir = dir.join("config");
    cfg.state_dir = dir.join("state");
    cfg.download_dir = Some(dir.join("downloads"));
    cfg.pictures_dir = Some(dir.join("pictures"));
    cfg.discovery_port = 0;
    cfg.control_ports = 0..=0;
    cfg.payload_ports = 0..=0;
    cfg.datagram_ports = 1..=65535;
    cfg.allow_loopback = true;
    cfg.home_interfaces = Some(vec!["lo".into()]);
    cfg.listen_udp = udp;
    cfg.app_command = vec![app.to_string_lossy().into_owned()];
    // No systemd here: the programs the service starts are started directly
    cfg.launcher = Vec::new();
    cfg.kind = kind.into();
    cfg.record_signals = true;
    if phone {
        cfg.capabilities = Some(phone_capabilities());
    }
    tweak(&mut cfg);
    (cfg, dir)
}

/// Puts the packet tap on a service and starts it
pub fn started(service: Rc<Service>, notices: Rc<Recorder>, dir: PathBuf) -> Peer {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    *service.tap.borrow_mut() = Some(Rc::new(move |from, p| log.borrow_mut().push((from.to_string(), p.clone()))));
    service.start();
    Peer { service, notices, seen, dir }
}

pub async fn until(what: &str, mut cond: impl FnMut() -> bool) {
    for _ in 0..600 {
        if cond() {
            return;
        }
        glib::timeout_future(Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for {what}");
}

pub async fn pause(ms: u64) {
    glib::timeout_future(Duration::from_millis(ms)).await;
}

pub fn prop<T: glib::variant::FromVariant>(dev: &Rc<Device>, name: &str) -> T {
    dev.properties().into_iter().find(|(n, _)| *n == name).and_then(|(_, v)| v.get::<T>()).unwrap_or_else(|| panic!("property {name}"))
}

pub fn signals(peer: &Peer, name: &str) -> Vec<glib::Variant> {
    peer.service.recorded.borrow().iter().filter(|s| s.name == name).filter_map(|s| s.args.clone()).collect()
}

/// The notification that asks about a pairing with this device: the key ends in the connection it was raised for
pub fn prompt_of(peer: &Peer, id: &str) -> Option<Notice> {
    peer.notices.raised_with_prefix(&format!("pair:{id}:")).pop()
}

pub fn emitted(peer: &Peer, name: &str) -> usize {
    peer.service.recorded.borrow().iter().filter(|s| s.name == name).count()
}

pub fn state_of(peer: &Peer, transfer_state: &str, id: &str) -> bool {
    peer.service.recorded.borrow().iter().any(|s| {
        s.name == "Transfer" && s.args.as_ref().is_some_and(|a| a.child_value(0).str() == Some(id) && a.child_value(5).str() == Some(transfer_state))
    })
}

pub fn file_uri(path: &Path) -> String {
    gio::File::for_path(path).uri().to_string()
}

pub fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 31 + i / 7) as u8).collect()
}

pub fn find_in(dir: &Path, prefix: &str) -> Vec<PathBuf> {
    std::fs::read_dir(dir).map(|r| r.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(prefix)).collect()).unwrap_or_default()
}


/// The computer calls the phone, and the phone asks to pair and is accepted on the computer: both are paired
pub async fn connect_and_pair(computer: &Peer, phone: &Peer) {
    let (cid, pid) = (computer.id(), phone.id());
    computer.service.connect_to(&format!("127.0.0.1:{}", phone.port())).unwrap();
    until("the two to be connected", || computer.service.device(&pid).is_some_and(|d| d.connected()) && phone.service.device(&cid).is_some_and(|d| d.connected())).await;
    phone.service.pair(&cid).unwrap();
    until("the prompt", || prompt_of(computer, &pid).is_some()).await;
    computer.notices.press(&prompt_of(computer, &pid).unwrap().key, "accept");
    until("the pairing", || computer.device(&pid).paired() && phone.device(&cid).paired()).await;
}

/// A session bus of its own for a test, as a program to start and a way to connect to it
pub struct Daemon {
    pub address: String,
    pid: String,
}

impl Daemon {
    pub fn start() -> Option<Daemon> {
        let out = std::process::Command::new("dbus-daemon").args(["--session", "--fork", "--print-address", "--print-pid"]).output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let mut lines = text.lines();
        Some(Daemon { address: lines.next()?.to_string(), pid: lines.next()?.to_string() })
    }

    pub async fn connect(&self) -> gio::DBusConnection {
        gio::DBusConnection::for_address_future(&self.address, gio::DBusConnectionFlags::AUTHENTICATION_CLIENT | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION, None).await.expect("the test bus")
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = std::process::Command::new("kill").arg(&self.pid).status();
    }
}
