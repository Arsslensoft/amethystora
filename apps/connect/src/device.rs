//! One phone as the program holds it: what it said it is, its connections, the state of the pairing and of each
//! plugin, and how it looks on the bus. All of it is behind one `RefCell`, borrowed for as short a time as the
//! work allows and never across a call into the bus or a notification.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use glib::variant::ToVariant;

use crate::battery::Battery;
use crate::connectivity::Signal;
use crate::contacts::Book;
use crate::link::Link;
use crate::media;
use crate::notifications::Table;
use crate::pairing::{Machine, Phase};
use crate::sms::Sms;
use crate::telephony;

pub const DEVICE_INTERFACE: &str = "org.amethystora.Connect.Device";
pub const DEVICE_PATH: &str = "/org/amethystora/Connect/Device/";

/// What the phone is sent or asks us for, by the packet types both sides listed
pub fn features(incoming: &[String], outgoing: &[String]) -> Vec<&'static str> {
    let takes = |t: &str| incoming.iter().any(|c| c == t);
    let sends = |t: &str| outgoing.iter().any(|c| c == t);
    let mut out = Vec::new();
    if takes("kdeconnect.ping") {
        out.push("ping");
    }
    if sends("kdeconnect.battery") {
        out.push("battery");
    }
    if sends("kdeconnect.notification") {
        out.push("notifications");
    }
    if takes("kdeconnect.share.request") {
        out.push("share");
    }
    if takes("kdeconnect.clipboard") || sends("kdeconnect.clipboard") {
        out.push("clipboard");
    }
    if takes("kdeconnect.sms.request_conversations") && takes("kdeconnect.sms.request_conversation") && sends("kdeconnect.sms.messages") {
        out.push("sms");
    }
    if sends("kdeconnect.telephony") {
        out.push("telephony");
    }
    if sends("kdeconnect.connectivity_report") {
        out.push("connectivity");
    }
    if takes(media::RING) {
        out.push("ring");
    }
    out
}

/// The features that are there only while the person has chosen them (plans/ui-ideas/spec-phone.md 3 and 4)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Offers {
    pub music: bool,
    pub clicker: bool,
}

fn offered(offers: Offers, incoming: &[String], outgoing: &[String]) -> Vec<&'static str> {
    let takes = |t: &str| incoming.iter().any(|c| c == t);
    let sends = |t: &str| outgoing.iter().any(|c| c == t);
    let mut out = Vec::new();
    if offers.music && (sends(media::MPRIS) || takes(media::MPRIS)) {
        out.push("music");
    }
    if offers.clicker && (sends(media::KEYS) || sends(media::PRESENTER)) {
        out.push("clicker");
    }
    out
}

/// Files that arrive as one batch are told of once, when they are all in
#[derive(Debug)]
pub struct BatchState {
    pub files: u32,
    pub bytes: u64,
    pub done: u32,
    pub failed: u32,
    pub last: Option<PathBuf>,
    pub started: Instant,
}

pub struct State {
    pub name: String,
    pub kind: String,
    pub protocol: u32,
    pub incoming: Vec<String>,
    pub outgoing: Vec<String>,
    pub links: Vec<Rc<Link>>,
    pub paired: bool,
    pub machine: Machine,
    pub pairing_link: Option<u64>,
    pub pairing_key: String,
    pub address: Option<(IpAddr, u16)>,
    pub battery: Battery,
    /// The mobile signal of its last connectivity report, forgotten when it goes
    pub signal: Option<Signal>,
    pub notifications: Table,
    pub contacts: Book,
    pub contacts_queue: VecDeque<String>,
    pub sms: Sms,
    pub call: Option<telephony::Event>,
    pub last_ping: Option<Instant>,
    pub batch: Option<BatchState>,
    pub downloads: usize,
    /// Files announced and not yet done with, waiting for their turn or on the way
    pub queued: usize,
    pub registration: Option<gio::RegistrationId>,
    pub published: HashMap<&'static str, glib::Variant>,
    /// When we last told the person about a certificate that did not match, so that a phone that keeps trying
    /// does not fill the screen
    pub mismatch_told: Option<Instant>,
    pub unpaired_since_connect: bool,
    /// Music from either side and the clicker, as the person has them now
    pub offers: Offers,
    /// The phone's players, while music from either side is on and it is connected
    pub players: media::Phone,
    /// When the phone last made a click of the clicker, which comes no faster than one in `limits::CLICK_SPACING`
    pub last_click: Option<Instant>,
    /// Files offered and not yet accepted, and how many more the person's Accept covers (ask.rs)
    pub asking: crate::ask::Asking,
}

pub struct Device {
    pub id: String,
    pub path: String,
    pub st: RefCell<State>,
}

impl Device {
    pub fn new(id: &str, path: String, name: &str, kind: &str, protocol: u32) -> Rc<Device> {
        Device::with_policy(id, path, name, kind, protocol, false, false)
    }

    /// A device whose notifications show the silent and the ongoing ones of the phone, if told to
    pub fn with_policy(id: &str, path: String, name: &str, kind: &str, protocol: u32, show_silent: bool, show_ongoing: bool) -> Rc<Device> {
        Rc::new(Device {
            id: id.to_string(),
            path,
            st: RefCell::new(State {
                name: name.to_string(),
                kind: kind.to_string(),
                protocol,
                incoming: Vec::new(),
                outgoing: Vec::new(),
                links: Vec::new(),
                paired: false,
                machine: Machine::new(protocol),
                pairing_link: None,
                pairing_key: String::new(),
                address: None,
                battery: Battery::default(),
                signal: None,
                notifications: Table::with_policy(show_silent, show_ongoing),
                contacts: Book::default(),
                contacts_queue: VecDeque::new(),
                sms: Sms::default(),
                call: None,
                last_ping: None,
                batch: None,
                downloads: 0,
                queued: 0,
                registration: None,
                published: HashMap::new(),
                mismatch_told: None,
                unpaired_since_connect: false,
                offers: Offers::default(),
                players: media::Phone::default(),
                last_click: None,
                asking: Default::default(),
            }),
        })
    }

    pub fn name(&self) -> String {
        self.st.borrow().name.clone()
    }

    pub fn paired(&self) -> bool {
        self.st.borrow().paired
    }

    pub fn connected(&self) -> bool {
        self.st.borrow().links.iter().any(|l| !l.is_closed())
    }

    /// The connection to write to: the newest that is still open
    pub fn link(&self) -> Option<Rc<Link>> {
        self.st.borrow().links.iter().rev().find(|l| !l.is_closed()).cloned()
    }

    /// Whether the phone listed this packet type among the ones it takes
    pub fn takes(&self, kind: &str) -> bool {
        self.st.borrow().incoming.iter().any(|c| c == kind)
    }

    pub fn sends(&self, kind: &str) -> bool {
        self.st.borrow().outgoing.iter().any(|c| c == kind)
    }

    pub fn features(&self) -> Vec<&'static str> {
        let st = self.st.borrow();
        if st.paired && st.links.iter().any(|l| !l.is_closed()) { st.features() } else { Vec::new() }
    }

    /// Every property of the device interface as it stands
    pub fn properties(&self) -> Vec<(&'static str, glib::Variant)> {
        let st = self.st.borrow();
        let connected = st.links.iter().any(|l| !l.is_closed());
        let (pairing, deadline) = match st.machine.phase() {
            Phase::Idle => ("none", 0),
            Phase::Incoming { deadline, .. } => ("incoming", deadline),
            Phase::Outgoing { deadline, .. } => ("outgoing", deadline),
        };
        let address = st
            .links
            .iter()
            .rev()
            .find(|l| !l.is_closed())
            .map(|l| match l.peer.0 {
                IpAddr::V4(ip) => format!("{ip}:{}", l.peer.1),
                IpAddr::V6(ip) => format!("[{ip}]:{}", l.peer.1),
            })
            .unwrap_or_default();
        let features: Vec<&str> = if st.paired && connected { st.features() } else { Vec::new() };
        // Like the features, the signal and the players are the phone's only while it is here
        let signal = st.signal.as_ref().filter(|_| st.paired && connected);
        let players: Vec<HashMap<String, glib::Variant>> =
            if st.paired && connected && st.offers.music { st.players.players().iter().map(media::Remote::to_dict).collect() } else { Vec::new() };
        vec![
            ("Id", self.id.to_variant()),
            ("Name", st.name.to_variant()),
            ("Type", st.kind.to_variant()),
            ("Paired", st.paired.to_variant()),
            ("Connected", connected.to_variant()),
            ("Pairing", pairing.to_variant()),
            ("PairingKey", (if pairing == "none" { String::new() } else { st.pairing_key.clone() }).to_variant()),
            ("PairingDeadline", deadline.to_variant()),
            ("Address", address.to_variant()),
            ("BatteryLevel", st.battery.level.to_variant()),
            ("BatteryCharging", st.battery.charging.to_variant()),
            ("SignalStrength", signal.map_or(-1, |s| s.strength).to_variant()),
            ("NetworkType", signal.map(|s| s.network.clone()).unwrap_or_default().to_variant()),
            ("Features", features.to_variant()),
            ("Players", players.to_variant()),
        ]
    }
}

impl State {
    /// What can be done with the phone, from what both sides listed and what the person chose
    fn features(&self) -> Vec<&'static str> {
        let mut out = features(&self.incoming, &self.outgoing);
        out.extend(offered(self.offers, &self.incoming, &self.outgoing));
        out
    }
}

/// The object path of a device: its id with every character that is not a letter or digit as an underscore
pub fn path_of(id: &str) -> String {
    format!("{DEVICE_PATH}{}", id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_phone_that_does_everything() {
        let incoming = caps(&[
            "kdeconnect.ping",
            "kdeconnect.battery.request",
            "kdeconnect.notification.request",
            "kdeconnect.share.request",
            "kdeconnect.clipboard",
            "kdeconnect.sms.request_conversations",
            "kdeconnect.sms.request_conversation",
            "kdeconnect.sms.request",
            "kdeconnect.telephony.request_mute",
        ]);
        let outgoing = caps(&["kdeconnect.ping", "kdeconnect.battery", "kdeconnect.notification", "kdeconnect.share.request", "kdeconnect.clipboard", "kdeconnect.sms.messages", "kdeconnect.telephony", "kdeconnect.connectivity_report"]);
        assert_eq!(features(&incoming, &outgoing), vec!["ping", "battery", "notifications", "share", "clipboard", "sms", "telephony", "connectivity"]);
    }

    #[test]
    fn what_the_phone_leaves_out_is_not_offered() {
        let out = features(&caps(&["kdeconnect.ping"]), &caps(&["kdeconnect.battery"]));
        assert_eq!(out, vec!["ping", "battery"]);
        assert!(features(&[], &[]).is_empty());
        // Texts need all three
        assert!(!features(&caps(&["kdeconnect.sms.request_conversations"]), &caps(&["kdeconnect.sms.messages"])).contains(&"sms"));
        // Clipboard either way
        assert_eq!(features(&[], &caps(&["kdeconnect.clipboard"])), vec!["clipboard"]);
        assert_eq!(features(&caps(&["kdeconnect.clipboard"]), &[]), vec!["clipboard"]);
    }

    #[test]
    fn ringing_and_the_chosen_features() {
        // Ringing needs only the phone to take the request
        assert_eq!(features(&caps(&[media::RING]), &[]), vec!["ring"]);
        let incoming = caps(&[media::MPRIS, media::MPRIS_REQUEST]);
        let outgoing = caps(&[media::MPRIS, media::KEYS, media::PRESENTER]);
        // Music and the clicker only while chosen
        assert!(offered(Offers::default(), &incoming, &outgoing).is_empty());
        assert_eq!(offered(Offers { music: true, clicker: false }, &incoming, &outgoing), vec!["music"]);
        assert_eq!(offered(Offers { music: true, clicker: true }, &incoming, &outgoing), vec!["music", "clicker"]);
        // A phone that sends no keys and no presenter is no clicker
        assert!(offered(Offers { music: false, clicker: true }, &incoming, &caps(&[media::MPRIS])).is_empty());
    }

    #[test]
    fn object_paths() {
        assert_eq!(path_of("4a8f2c7e_91d3_4b5a_8e6f_0c1d2e3f4a5b"), "/org/amethystora/Connect/Device/4a8f2c7e_91d3_4b5a_8e6f_0c1d2e3f4a5b");
        assert_eq!(path_of("a-b"), "/org/amethystora/Connect/Device/a_b");
    }

    #[test]
    fn a_new_device_is_unpaired_and_disconnected() {
        let d = Device::new("c3f1a8d27b5e4096a1d84f7e2b9c60d5", path_of("c3f1a8d27b5e4096a1d84f7e2b9c60d5"), "Sam phone", "phone", 8);
        assert!(!d.paired() && !d.connected());
        let props: HashMap<_, _> = d.properties().into_iter().collect();
        assert_eq!(props["Pairing"].get::<String>().as_deref(), Some("none"));
        assert_eq!(props["PairingKey"].get::<String>().as_deref(), Some(""));
        assert_eq!(props["PairingDeadline"].get::<i64>(), Some(0));
        assert_eq!(props["BatteryLevel"].get::<i32>(), Some(-1));
        assert_eq!(props["Features"].get::<Vec<String>>(), Some(vec![]));
        assert_eq!(props["SignalStrength"].get::<i32>(), Some(-1));
        assert_eq!(props["NetworkType"].get::<String>().as_deref(), Some(""));
        assert_eq!(props.len(), 15);
        assert_eq!(props["Players"].n_children(), 0);
        // A signal kept from before shows only while the phone is paired and here
        d.st.borrow_mut().signal = Some(Signal { strength: 3, network: "LTE".into() });
        let props: HashMap<_, _> = d.properties().into_iter().collect();
        assert_eq!(props["SignalStrength"].get::<i32>(), Some(-1));
    }
}
