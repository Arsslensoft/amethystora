//! The way out of a sandbox with no network of its own (see Cargo.toml): `proxy`, held outside the sandbox, and `shim`, the
//! forwarder inside it. The names a proxy may be told are held to their shape here, for whoever writes its list.

pub mod proxy;
pub mod shim;

/// What a host may send in an hour before it is blocked, unless its list says more
pub const DEFAULT_LIMIT: u64 = 1 << 20;

/// A named host: letters, digits, hyphens and dots, one `*.` in front at most, never an address
pub fn valid_host(host: &str) -> bool {
    let name = host.strip_prefix("*.").unwrap_or(host);
    let labels: Vec<&str> = name.split('.').collect();
    host.len() <= 253
        && labels.len() >= 2
        && labels.iter().all(|l| {
            !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && l.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
        // 1.2.3.4 is an address, not a name; the last label of a name is never all digits
        && !labels.last().is_some_and(|l| l.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether `host` (lower case) is covered by an allowed entry: the name itself, or a subdomain of `*.name`
pub fn host_matches(entry: &str, host: &str) -> bool {
    match entry.strip_prefix("*.") {
        Some(base) => host.len() > base.len() + 1 && host.ends_with(base) && host.as_bytes()[host.len() - base.len() - 1] == b'.',
        None => entry == host,
    }
}
