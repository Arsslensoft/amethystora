//! Who a device says it is: the identity packet, in a datagram, in plain text over a new connection, and
//! (protocol 8) again inside TLS (spec-protocol.md 1.3 to 1.5 and 4). What a peer sends is checked here and
//! nowhere else: an id that is not an id, a name with the characters the protocol forbids (removed, not
//! refused), a port or version that is not a number.

use serde_json::{Map, Value, json};

use crate::limits;
use crate::packet::{self, IDENTITY, Packet};
use crate::text;

/// What this program handles, and what it may send (spec-plugins.md 1.2). A capability is a claim, not a
/// promise: a type listed here that nothing sends yet costs the phone nothing.
pub const INCOMING: &[&str] = &[
    "kdeconnect.battery",
    "kdeconnect.battery.request",
    "kdeconnect.clipboard",
    "kdeconnect.clipboard.connect",
    "kdeconnect.connectivity_report",
    "kdeconnect.contacts.response_uids_timestamps",
    "kdeconnect.contacts.response_vcards",
    "kdeconnect.notification",
    "kdeconnect.notification.request",
    "kdeconnect.ping",
    "kdeconnect.share.request",
    "kdeconnect.share.request.update",
    "kdeconnect.sms.messages",
    "kdeconnect.telephony",
];

pub const OUTGOING: &[&str] = &[
    "kdeconnect.battery",
    "kdeconnect.battery.request",
    "kdeconnect.clipboard",
    "kdeconnect.clipboard.connect",
    "kdeconnect.contacts.request_all_uids_timestamps",
    "kdeconnect.contacts.request_vcards_by_uid",
    "kdeconnect.notification",
    "kdeconnect.notification.action",
    "kdeconnect.notification.reply",
    "kdeconnect.notification.request",
    "kdeconnect.ping",
    "kdeconnect.share.request",
    "kdeconnect.sms.request",
    "kdeconnect.sms.request_conversation",
    "kdeconnect.sms.request_conversations",
    "kdeconnect.telephony.request",
    "kdeconnect.telephony.request_mute",
];

/// What this computer advertises now (plans/ui-ideas/spec-phone.md 1): the lists above, ringing the phone always,
/// the media packets both ways while music from either side is on, and the presenter's packet while the clicker is
/// on, and the photo request while Snap with your phone is on. Off, the phone's own app shows none of them for this computer.
pub fn advertised(music: bool, clicker: bool, photo: bool) -> (Vec<String>, Vec<String>) {
    let mut incoming: Vec<String> = INCOMING.iter().map(|s| s.to_string()).collect();
    let mut outgoing: Vec<String> = OUTGOING.iter().map(|s| s.to_string()).collect();
    outgoing.push(crate::media::RING.to_string());
    if music {
        for kind in [crate::media::MPRIS, crate::media::MPRIS_REQUEST] {
            incoming.push(kind.to_string());
            outgoing.push(kind.to_string());
        }
    }
    if clicker {
        incoming.push(crate::media::PRESENTER.to_string());
    }
    // Snap with your phone (photo.rs): listed only while the person has chosen it
    if photo {
        incoming.push(crate::photo::PHOTO.to_string());
        outgoing.push(crate::photo::REQUEST.to_string());
    }
    (incoming, outgoing)
}

#[derive(Debug, PartialEq, Eq)]
pub enum IdentityError {
    /// No `deviceId`, or one that is not 32 to 38 of `A-Za-z0-9_-`
    BadId,
    /// A name with nothing left after the forbidden characters are taken out
    BadName,
    /// `protocolVersion` or `tcpPort` present and not a whole number in range
    BadNumber,
}

/// A device as it introduced itself. Everything but the id may be missing: a datagram carries little, and a
/// plain identity is read for its id and version before the full one arrives inside TLS.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Identity {
    pub device_id: String,
    pub name: Option<String>,
    pub kind: Option<String>,
    pub protocol: Option<u32>,
    pub incoming: Option<Vec<String>>,
    pub outgoing: Option<Vec<String>>,
    pub tcp_port: Option<u16>,
    pub target_id: Option<String>,
    pub target_protocol: Option<u32>,
}

/// 32 to 38 characters from `A-Za-z0-9_-`. Compared as exact, case-sensitive strings. One that starts with a hyphen
/// is not taken: no id a phone makes does, and it would read as an option on a command line.
pub fn valid_device_id(id: &str) -> bool {
    (32..=38).contains(&id.len()) && !id.starts_with('-') && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// The ten the protocol forbids, and the typographic quotes and brackets of other scripts: our prompts put a name
/// between “ and ”, and a name that could close them would be a way to write the rest of the sentence
const FORBIDDEN_IN_NAMES: &[char] = &[
    '"', '\'', ',', ';', ':', '.', '!', '?', '(', ')', '[', ']', '<', '>', '\u{201C}', '\u{201D}', '\u{201E}', '\u{201F}', '\u{2018}', '\u{2019}', '\u{201A}', '\u{201B}', '\u{00AB}',
    '\u{00BB}', '\u{2039}', '\u{203A}', '\u{300C}', '\u{300D}', '\u{300E}', '\u{300F}', '\u{FF02}', '\u{FF07}',
];

/// A device name as the protocol allows it: the ten forbidden characters (and control characters) removed,
/// trimmed, at most 32 UTF-16 units. Empty when nothing is left.
pub fn sanitize_name(name: &str) -> String {
    let cleaned: String = text::clean_line(name).chars().filter(|c| !FORBIDDEN_IN_NAMES.contains(c)).collect();
    let mut units = 0;
    let mut out = String::new();
    for c in cleaned.trim().chars() {
        units += c.len_utf16();
        if units > 32 {
            break;
        }
        out.push(c);
    }
    out.trim().to_string()
}

const DEVICE_TYPES: &[&str] = &["desktop", "laptop", "phone", "tablet", "tv"];

impl Identity {
    pub fn parse(body: &Map<String, Value>) -> Result<Identity, IdentityError> {
        let device_id = packet::str_of(body, "deviceId").filter(|id| valid_device_id(id)).ok_or(IdentityError::BadId)?.to_string();
        let name = match packet::str_of(body, "deviceName") {
            Some(raw) => match sanitize_name(raw) {
                name if name.is_empty() => return Err(IdentityError::BadName),
                name => Some(name),
            },
            None => None,
        };
        // An unknown type is a desktop, not a refusal
        let kind = packet::str_of(body, "deviceType").map(|k| if DEVICE_TYPES.contains(&k) { k.to_string() } else { "desktop".to_string() });
        let protocol = match body.get("protocolVersion") {
            None | Some(Value::Null) => None,
            Some(_) => Some(packet::int_of(body, "protocolVersion").and_then(|v| u32::try_from(v).ok()).ok_or(IdentityError::BadNumber)?),
        };
        let tcp_port = match body.get("tcpPort") {
            None | Some(Value::Null) => None,
            Some(_) => Some(packet::int_of(body, "tcpPort").and_then(|v| u16::try_from(v).ok()).ok_or(IdentityError::BadNumber)?),
        };
        let target_protocol = packet::int_of(body, "targetProtocolVersion").and_then(|v| u32::try_from(v).ok());
        Ok(Identity {
            device_id,
            name,
            kind,
            protocol,
            incoming: capabilities(body, "incomingCapabilities"),
            outgoing: capabilities(body, "outgoingCapabilities"),
            tcp_port,
            target_id: packet::str_of(body, "targetDeviceId").map(str::to_string),
            target_protocol,
        })
    }

    pub fn from_packet(p: &Packet) -> Result<Identity, IdentityError> {
        if p.kind != IDENTITY {
            return Err(IdentityError::BadId);
        }
        Identity::parse(&p.body)
    }
}

/// A capability list: strings only, of a sane length and count
fn capabilities(body: &Map<String, Value>, key: &str) -> Option<Vec<String>> {
    packet::strings_of(body, key).map(|list| list.into_iter().filter(|c| c.len() <= 128).take(256).collect())
}

/// This computer as it announces itself
#[derive(Debug, Clone)]
pub struct Local {
    pub device_id: String,
    pub name: String,
    pub kind: String,
    /// What it handles and may send: `INCOMING` and `OUTGOING`, but for a test that plays a phone
    pub incoming: Vec<String>,
    pub outgoing: Vec<String>,
}

impl Local {
    pub fn new(device_id: &str, name: &str, kind: &str) -> Local {
        let (incoming, outgoing) = advertised(false, false, false);
        Local { device_id: device_id.to_string(), name: name.to_string(), kind: kind.to_string(), incoming, outgoing }
    }

    /// Whether this computer listed the type among those it handles
    pub fn handles(&self, kind: &str) -> bool {
        self.incoming.iter().any(|c| c == kind)
    }

    /// The identity packet: with every capability (inside TLS, and in plain text before it), or trimmed to
    /// fit a datagram. `target` is the id and version of the device being called, from its datagram.
    pub fn packet(&self, tcp_port: u16, capabilities: bool, target: Option<(&str, Option<u32>)>) -> Packet {
        let mut body = json!({
            "deviceId": self.device_id,
            "deviceName": self.name,
            "deviceType": self.kind,
            "protocolVersion": limits::PROTOCOL_VERSION,
            "incomingCapabilities": if capabilities { json!(self.incoming) } else { json!([]) },
            "outgoingCapabilities": if capabilities { json!(self.outgoing) } else { json!([]) },
            "tcpPort": tcp_port,
        });
        if let (Some((id, version)), Some(map)) = (target, body.as_object_mut()) {
            map.insert("targetDeviceId".into(), json!(id));
            if let Some(version) = version {
                map.insert("targetProtocolVersion".into(), json!(version));
            }
        }
        Packet::new(IDENTITY, body)
    }

    /// The same without `tcpPort`: what is sent inside TLS, where the port is no use
    pub fn packet_in_tls(&self) -> Packet {
        let mut p = self.packet(0, true, None);
        p.body.remove("tcpPort");
        p
    }

    /// The datagram: the whole identity if it fits a link's MTU, else without `outgoingCapabilities`, else
    /// without `incomingCapabilities` as well
    pub fn datagram(&self, tcp_port: u16) -> Vec<u8> {
        let mut p = self.packet(tcp_port, true, None);
        let mut line = p.to_line();
        if line.len() > limits::DATAGRAM_PREFERRED {
            p.body.insert("outgoingCapabilities".into(), json!([]));
            line = p.to_line();
        }
        if line.len() > limits::DATAGRAM_PREFERRED {
            p.body.insert("incomingCapabilities".into(), json!([]));
            line = p.to_line();
        }
        line
    }
}

/// Our name for phones to show: the machine's pretty name, else its host name, cleaned to what phones accept
pub fn default_name(configured: &str, pretty: Option<&str>, host: &str) -> String {
    for candidate in [configured, pretty.unwrap_or(""), host] {
        let name = sanitize_name(candidate);
        if !name.is_empty() {
            return name;
        }
    }
    text::tr("Amethystora")
}

/// laptop, tablet or desktop from the SMBIOS chassis type number
pub fn kind_of_chassis(chassis: Option<u32>) -> &'static str {
    match chassis {
        Some(8 | 9 | 10 | 14 | 31) => "laptop",
        Some(30 | 32) => "tablet",
        _ => "desktop",
    }
}

/// Whether a device of this kind is another computer, which the phone link talks to only while the switch
/// `computers` is on. A type the protocol does not know was already read as a desktop, so it is one too.
pub fn is_computer(kind: &str) -> bool {
    !matches!(kind, "phone" | "tablet" | "tv")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PHONE: &str = "4a8f2c7e_91d3_4b5a_8e6f_0c1d2e3f4a5b";
    const US: &str = "c3f1a8d27b5e4096a1d84f7e2b9c60d5";

    fn parse(json: &str) -> Result<Identity, IdentityError> {
        Identity::from_packet(&Packet::parse(json.as_bytes()).unwrap())
    }

    #[test]
    fn ids() {
        assert!(valid_device_id(US));
        assert!(valid_device_id(PHONE));
        assert!(valid_device_id(&"a-b_".repeat(8)));
        assert!(!valid_device_id("short"));
        assert!(!valid_device_id(&"a".repeat(31)));
        assert!(!valid_device_id(&"a".repeat(39)));
        assert!(!valid_device_id(&format!("{}.x", "a".repeat(32))));
        assert!(!valid_device_id(&format!("{} ", "a".repeat(32))));
        assert!(!valid_device_id(&format!("{}/", "a".repeat(32))));
        assert!(!valid_device_id(&format!("-{}", "a".repeat(32))), "an id is not an option");
        assert!(valid_device_id(&format!("a-{}", "a".repeat(32))));
    }

    #[test]
    fn names_lose_the_forbidden_characters() {
        assert_eq!(sanitize_name("Sam's phone"), "Sams phone");
        assert_eq!(sanitize_name("host.local"), "hostlocal");
        assert_eq!(sanitize_name("a\"b'c,d;e:f.g!h?i(j)k[l]m<n>o"), "abcdefghijklmno");
        assert_eq!(sanitize_name("  spaced  "), "spaced");
        assert_eq!(sanitize_name("...!!!"), "");
        assert_eq!(sanitize_name(&"x".repeat(40)).len(), 32);
        // 32 UTF-16 units: an astral character is two
        assert_eq!(sanitize_name(&"\u{1F600}".repeat(20)).chars().count(), 16);
        assert_eq!(sanitize_name("two\nlines"), "two lines");
        // The quotes our prompts use cannot be closed by a name
        assert_eq!(sanitize_name("Pixel\u{201D} (verified"), "Pixel verified");
        assert_eq!(sanitize_name("\u{201C}Sam\u{2019}s\u{00BB} phone\u{2028}"), "Sams phone");
    }

    #[test]
    fn the_spec_datagrams_parse() {
        let full = parse(
            r#"{"id":1792741200123,"type":"kdeconnect.identity","body":{"deviceId":"4a8f2c7e_91d3_4b5a_8e6f_0c1d2e3f4a5b","deviceName":"Sam phone","deviceType":"phone","protocolVersion":8,"incomingCapabilities":["kdeconnect.battery","kdeconnect.ping"],"outgoingCapabilities":["kdeconnect.ping"],"tcpPort":1716}}"#,
        )
        .unwrap();
        assert_eq!(full.device_id, PHONE);
        assert_eq!(full.name.as_deref(), Some("Sam phone"));
        assert_eq!(full.kind.as_deref(), Some("phone"));
        assert_eq!(full.protocol, Some(8));
        assert_eq!(full.tcp_port, Some(1716));
        assert_eq!(full.incoming.as_ref().map(Vec::len), Some(2));

        // The minimal datagram: no name, no type, no lists
        let minimal = parse(r#"{"id":0,"type":"kdeconnect.identity","body":{"deviceId":"4a8f2c7e_91d3_4b5a_8e6f_0c1d2e3f4a5b","protocolVersion":8,"tcpPort":1716}}"#).unwrap();
        assert_eq!(minimal.name, None);
        assert_eq!(minimal.incoming, None);
    }

    #[test]
    fn the_negative_vectors_are_refused() {
        // A string where a number is expected, a short id, a dot in the id, the wrong type
        assert_eq!(parse(&format!(r#"{{"type":"kdeconnect.identity","body":{{"deviceId":"{PHONE}","tcpPort":"1716"}}}}"#)), Err(IdentityError::BadNumber));
        assert_eq!(parse(r#"{"type":"kdeconnect.identity","body":{"deviceId":"short"}}"#), Err(IdentityError::BadId));
        assert_eq!(parse(&format!(r#"{{"type":"kdeconnect.identity","body":{{"deviceId":"{}.x"}}}}"#, "a".repeat(32))), Err(IdentityError::BadId));
        assert_eq!(parse(&format!(r#"{{"type":"kdeconnect.ping","body":{{"deviceId":"{PHONE}"}}}}"#)), Err(IdentityError::BadId));
        assert_eq!(parse(&format!(r#"{{"type":"kdeconnect.identity","body":{{"deviceId":"{PHONE}","deviceName":"..!!"}}}}"#)), Err(IdentityError::BadName));
        assert_eq!(parse(&format!(r#"{{"type":"kdeconnect.identity","body":{{"deviceId":"{PHONE}","protocolVersion":"8"}}}}"#)), Err(IdentityError::BadNumber));
        assert_eq!(parse(&format!(r#"{{"type":"kdeconnect.identity","body":{{"deviceId":"{PHONE}","tcpPort":70000}}}}"#)), Err(IdentityError::BadNumber));
    }

    #[test]
    fn an_unknown_type_is_a_desktop_and_names_are_cleaned() {
        let i = parse(&format!(r#"{{"type":"kdeconnect.identity","body":{{"deviceId":"{PHONE}","deviceName":"Sam's phone","deviceType":"toaster"}}}}"#)).unwrap();
        assert_eq!(i.kind.as_deref(), Some("desktop"));
        assert_eq!(i.name.as_deref(), Some("Sams phone"));
    }

    #[test]
    fn our_identity_has_the_shape_of_the_spec() {
        let local = Local::new(US, "Studio laptop", "laptop");
        let datagram = local.datagram(1716);
        assert!(datagram.len() <= limits::DATAGRAM_PREFERRED, "{} bytes", datagram.len());
        assert_eq!(datagram.last(), Some(&b'\n'));
        let back = Packet::parse(&datagram).unwrap();
        assert_eq!(back.kind, IDENTITY);
        let id = Identity::from_packet(&back).unwrap();
        assert_eq!(id.device_id, US);
        assert_eq!(id.protocol, Some(8));
        assert_eq!(id.tcp_port, Some(1716));
        assert_eq!(id.incoming.unwrap().len(), INCOMING.len());

        let plain = local.packet(1717, true, Some((PHONE, Some(8))));
        assert_eq!(plain.str("targetDeviceId"), Some(PHONE));
        assert_eq!(plain.int("targetProtocolVersion"), Some(8));
        assert_eq!(plain.int("tcpPort"), Some(1717));
        let no_version = local.packet(1717, true, Some((PHONE, None)));
        assert_eq!(no_version.int("targetProtocolVersion"), None);

        let tls = local.packet_in_tls();
        assert!(!tls.body.contains_key("tcpPort"));
        assert!(tls.body.contains_key("incomingCapabilities"));
    }

    #[test]
    fn a_datagram_that_does_not_fit_loses_its_lists() {
        let local = Local::new(US, &"x".repeat(32), "laptop");
        // The real lists fit; a datagram built from absurdly long ones does not, and drops them in order
        let mut p = local.packet(1716, true, None);
        let long: Vec<Value> = (0..200).map(|i| json!(format!("kdeconnect.something.long.{i}"))).collect();
        p.body.insert("incomingCapabilities".into(), json!(long));
        p.body.insert("outgoingCapabilities".into(), json!(long));
        assert!(p.to_line().len() > limits::DATAGRAM_PREFERRED);
    }

    #[test]
    fn names_and_chassis() {
        assert_eq!(default_name("", Some("Studio laptop"), "host.local"), "Studio laptop");
        assert_eq!(default_name("", None, "host.local"), "hostlocal");
        assert_eq!(default_name("Mine", Some("x"), "h"), "Mine");
        assert_eq!(default_name("...", Some("..."), "..."), text::tr("Amethystora"));
        assert_eq!(kind_of_chassis(Some(9)), "laptop");
        assert_eq!(kind_of_chassis(Some(31)), "laptop");
        assert_eq!(kind_of_chassis(Some(32)), "tablet");
        assert_eq!(kind_of_chassis(Some(3)), "desktop");
        assert_eq!(kind_of_chassis(None), "desktop");
        assert!(is_computer("laptop") && is_computer("desktop"));
        assert!(!is_computer("phone") && !is_computer("tablet") && !is_computer("tv"));
    }
}
