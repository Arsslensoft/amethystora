//! What may be written into a certificate as a name, and the root's limits, in one place: the service's own check and the
//! root's Name Constraints read the same table, so that it refuses exactly what the root would.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::PkiError;

/// X.520's limit for a common, organization or unit name
pub const MAX_NAME: usize = 64;
const MAX_EMAIL: usize = 254;
const MAX_WEB: usize = 512;
const MAX_HOST: usize = 253;

/// The DNS names the root may speak for, and their subdomains
pub(super) const PERMITTED_DNS: [&str; 4] = ["local", "home.arpa", "internal", "localhost"];

/// The addresses it may speak for, each with its mask, in the order the plan lists them
pub(super) const PERMITTED_IP: [(IpAddr, &[u8]); 6] = [
    (IpAddr::V4(Ipv4Addr::new(10, 0, 0, 0)), &[255, 0, 0, 0]),
    (IpAddr::V4(Ipv4Addr::new(172, 16, 0, 0)), &[255, 240, 0, 0]),
    (IpAddr::V4(Ipv4Addr::new(192, 168, 0, 0)), &[255, 255, 0, 0]),
    (IpAddr::V4(Ipv4Addr::new(127, 0, 0, 0)), &[255, 0, 0, 0]),
    (IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 0)), &[0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    (IpAddr::V6(Ipv6Addr::LOCALHOST), &[0xff; 16]),
];

/// What cannot be seen or has no business in a name: the control, format, private-use and unassigned code points (General_Category
/// Cc, Cf, Co, Cn), line and paragraph separators, and the few characters that draw as nothing without being Cf. The joiners
/// stay, since Persian and Indic names need them.
fn invisible(c: char) -> bool {
    use icu_properties::props::GeneralCategory as Category;
    if matches!(c, '\u{200c}' | '\u{200d}') {
        return false;
    }
    if matches!(c, '\u{034f}' | '\u{115f}' | '\u{1160}' | '\u{17b4}' | '\u{17b5}' | '\u{3164}' | '\u{ffa0}') {
        return true;
    }
    matches!(
        icu_properties::CodePointMapData::<Category>::new().get(c),
        Category::Control
            | Category::Format
            | Category::PrivateUse
            | Category::Unassigned
            | Category::Surrogate
            | Category::LineSeparator
            | Category::ParagraphSeparator
    )
}

/// Text a person typed that is shown and stored but is not a Distinguished Name's part: not empty, not too long, not padded,
/// nothing in it that cannot be seen.
pub(super) fn check_text(text: &str, limit: usize) -> bool {
    let count = text.chars().count();
    count != 0 && count <= limit && text.trim() == text && !text.chars().any(invisible)
}

/// Refuses what a person should not be able to put in a name that goes into a certificate: what `check_text` refuses, and the
/// characters that separate a Distinguished Name's parts (RFC 4514), so that a typed name never reads as more fields than it
/// is. `describe` escapes them as well, for a certificate from elsewhere.
pub fn check_name(name: &str, limit: usize) -> bool {
    check_text(name, limit) && !name.starts_with('#') && !name.contains([',', '=', '+', ';', '"', '\\', '<', '>'])
}

/// A subject's Common Name that reads as a host name. With no DNS name beside it, OpenSSL holds such a CN to the root's
/// limits as one, whatever the certificate's purpose, so `j.smith` in a PDF certificate would fail its checks: it is
/// refused when it is typed.
pub(super) fn host_shaped(name: &str) -> bool {
    name.contains('.') && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn valid_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// An address is written into a certificate as IA5String, so only ASCII, and nothing that could read as a second name
pub(super) fn check_email(email: &str) -> Result<(), ()> {
    let (local, domain) = email.split_once('@').ok_or(())?;
    let local_ok = |byte: u8| byte.is_ascii_alphanumeric() || b"!#$%&'*+-/=?^_`{|}~.".contains(&byte);
    if email.len() > MAX_EMAIL
        || local.is_empty()
        || local.len() > 64
        || !local.bytes().all(local_ok)
        || !domain.split('.').all(valid_label)
    {
        return Err(());
    }
    Ok(())
}

pub(super) fn check_web(web: &str) -> Result<(), ()> {
    let rest = web.strip_prefix("https://").or_else(|| web.strip_prefix("http://")).ok_or(())?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if web.len() > MAX_WEB || host.is_empty() || !web.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(());
    }
    Ok(())
}

pub(super) enum LocalName {
    Dns(String),
    Ip(IpAddr),
}

fn within(address: IpAddr, network: IpAddr, mask: &[u8]) -> bool {
    let (address, network): (Vec<u8>, Vec<u8>) = match (address, network) {
        (IpAddr::V4(address), IpAddr::V4(network)) => (address.octets().to_vec(), network.octets().to_vec()),
        (IpAddr::V6(address), IpAddr::V6(network)) => (address.octets().to_vec(), network.octets().to_vec()),
        _ => return false,
    };
    address.iter().zip(&network).zip(mask).all(|((a, n), m)| a & m == n & m)
}

/// A name for local TLS: an address inside the root's limits or a host under them, lowercased, and never a wildcard.
/// Anything else is refused here, before it gets as far as a chain that would fail.
pub(super) fn check_local_name(name: &str) -> Result<LocalName, PkiError> {
    if let Ok(address) = name.parse::<IpAddr>() {
        return if PERMITTED_IP.iter().any(|(network, mask)| within(address, *network, mask)) {
            Ok(LocalName::Ip(address))
        } else {
            Err(PkiError::OutsideLimits)
        };
    }
    if name.len() > MAX_HOST || !name.split('.').all(valid_label) {
        return Err(PkiError::BadField("host name"));
    }
    let lower = name.to_ascii_lowercase();
    if PERMITTED_DNS.iter().any(|allowed| lower == *allowed || lower.ends_with(&format!(".{allowed}"))) {
        Ok(LocalName::Dns(lower))
    } else {
        Err(PkiError::OutsideLimits)
    }
}
