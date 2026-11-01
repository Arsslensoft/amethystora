//! Every number the protocol and the plugins fix, in one place (spec-protocol.md 7.7, spec-plugins.md 1.10).
//! The wire facts are the ports and the version; the rest are Amethystora's choices and can be changed here.

use std::ops::RangeInclusive;
use std::time::Duration;

/// The UDP port identity datagrams go to and come from
pub const DISCOVERY_PORT: u16 = 1716;
/// The control connection: the first free port of this range is the one announced as `tcpPort`
pub const CONTROL_PORTS: RangeInclusive<u16> = 1716..=1764;
/// A payload a peer downloads from us: the first free port of this range
pub const PAYLOAD_PORTS: RangeInclusive<u16> = 1739..=1764;
/// The `tcpPort` of a datagram we accept: the range the firewall service opens
pub const DATAGRAM_PORTS: RangeInclusive<u16> = 1714..=1764;
/// The payload port of a packet we accept, on the address of the control connection
pub const PAYLOAD_PORTS_ACCEPTED: RangeInclusive<u16> = 1024..=65535;

/// What we announce, and the only peer versions we talk to
pub const PROTOCOL_VERSION: u32 = 8;
pub const PROTOCOLS_ACCEPTED: RangeInclusive<u32> = 7..=8;

pub const DATAGRAM_PREFERRED: usize = 1472;
pub const DATAGRAM_READ: usize = 8192;
/// The plain identity before TLS
pub const PLAIN_LINE: usize = 8192;
/// A connection to a device we do not hold as paired: it may only pair, and pair packets are tiny
pub const UNPAIRED_LINE: usize = 64 * 1024;
/// A conversation list with thumbnails is the largest thing a plugin sends
pub const CONTROL_LINE: usize = 16 * 1024 * 1024;

pub const PAIR_WAIT_SECS: i64 = 30;
pub const PAIR_FRESH_SECS: i64 = 1800;
pub const PAIR_REQUESTS_PER_MINUTE: usize = 3;
/// Prompts waiting for the person at once, over every device
pub const PAIR_PROMPTS_AT_ONCE: usize = 3;

/// TCP keepalive on every control and payload connection: a dead peer is noticed in about 25 seconds
pub const KEEPALIVE_IDLE: i32 = 10;
pub const KEEPALIVE_INTERVAL: i32 = 5;
pub const KEEPALIVE_COUNT: i32 = 3;
pub const USER_TIMEOUT_MS: i32 = 30_000;

pub const STEP_TIMEOUT: Duration = Duration::from_secs(10);
/// From accept (or connect) to the end of the second identity
pub const HANDSHAKE_TOTAL: Duration = Duration::from_secs(15);
pub const HANDSHAKES_AT_ONCE: usize = 8;
pub const HANDSHAKES_PER_SOURCE: u32 = 2;
/// At most one attempt to connect to one source address in this time, and this many in progress at once
pub const ATTEMPT_SPACING: Duration = Duration::from_secs(2);
pub const ATTEMPTS_AT_ONCE: usize = 4;

/// Connections kept for one paired device: the oldest is closed beyond this
pub const LINKS_PER_DEVICE: usize = 3;
/// Devices that are connected and not paired, at any moment
pub const UNPAIRED_DEVICES: usize = 8;

pub const PAYLOAD_ACCEPT_WAIT: Duration = Duration::from_secs(60);
pub const PAYLOAD_IDLE: Duration = Duration::from_secs(60);
pub const DOWNLOADS_PER_DEVICE: usize = 4;
pub const DOWNLOADS_IN_ALL: usize = 8;
/// A phone has been seen to send more than it announced: this much extra is kept, more fails the transfer
pub const PAYLOAD_EXCESS: u64 = 1024 * 1024;

pub const REACH_FIRST: Duration = Duration::from_secs(5);
pub const REACH_FIRST_MINUTE: Duration = Duration::from_secs(60);
pub const REACH_LATER: Duration = Duration::from_secs(30);

// Text from a phone, as it is shown (spec-plugins.md 1.10)
pub const NOTIFICATION_TITLE: usize = 256;
pub const NOTIFICATION_BODY: usize = 4096;
pub const NOTIFICATION_BUTTONS: usize = 4;
pub const PING_TEXT: usize = 256;
pub const SHARED_TEXT: usize = 64 * 1024;
pub const SHARED_LINK: usize = 8 * 1024;
pub const NAME_SHOWN: usize = 128;
pub const FILE_NAME: usize = 255;
pub const CLIPBOARD_TEXT: usize = 1024 * 1024;
pub const CLIPBOARD_PER_SECOND: usize = 5;

pub const ICON_BYTES: u64 = 512 * 1024;
pub const ICON_EDGE: i32 = 256;
pub const ICON_WAIT: Duration = Duration::from_secs(2);
pub const ICONS_KEPT: usize = 256;
pub const ICONS_BYTES: u64 = 32 * 1024 * 1024;
pub const ICON_DOWNLOADS: usize = 4;
pub const PHOTO_TEXT: usize = 256 * 1024;

pub const CONTACT_CARDS_PER_REQUEST: usize = 50;
pub const CONTACT_CARD: usize = 256 * 1024;
pub const CONTACT_PROPERTIES: usize = 64;
pub const CONTACTS_KEPT: usize = 5000;
pub const CONTACT_NUMBERS: usize = 20;

pub const THREADS_KEPT: usize = 1000;
pub const MESSAGES_PER_THREAD: usize = 500;
pub const MESSAGE_BODY: usize = 16 * 1024;
pub const REQUESTS_OUTSTANDING: usize = 2;

pub const BATCH_QUIET: Duration = Duration::from_secs(60);

/// The phone as a slide clicker: at most one click in this long (plans/ui-ideas/spec-phone.md 4)
pub const CLICK_SPACING: Duration = Duration::from_millis(150);
