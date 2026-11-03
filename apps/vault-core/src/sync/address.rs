//! Choosing a server (spec 1): from what the person typed, the two bases a connection ever uses, and the origins the network
//! service may reach. No URL a server hands over is ever used.

/// Where a connected account's requests go
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Server {
    /// What the person typed, tidied: what is shown and kept
    pub address: String,
    pub identity: String,
    pub api: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Not an address: a query, a fragment, a user name, a scheme other than https, no host
    NotAnAddress,
    /// Plain http to a host other than this machine
    NotHttps,
    /// Bitwarden's government region: out of scope
    Unsupported,
}

impl Refused {
    pub fn name(self) -> &'static str {
        match self {
            Refused::NotAnAddress => "not-an-address",
            Refused::NotHttps => "not-https",
            Refused::Unsupported => "unsupported-server",
        }
    }
}

/// Whether a host is this machine: the only one plain http may reach
pub fn loopback(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// The scheme, host and port of a URL, and the rest: `https://host:port` and `/path?query`
pub fn split_url(url: &str) -> Option<(String, String, u16, String)> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let default = match scheme.as_str() {
        "https" => 443,
        "http" => 80,
        _ => return None,
    };
    let (authority, path) = match rest.find(['/', '?']) {
        Some(at) => (&rest[..at], &rest[at..]),
        None => (rest, ""),
    };
    if authority.is_empty() || authority.contains('@') || path.contains('#') || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        let (host, after) = inner.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(port) => port.parse().ok()?,
            None if after.is_empty() => default,
            None => return None,
        };
        (format!("[{host}]"), port)
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host.to_string(), port.parse().ok()?),
            None => (authority.to_string(), default),
        }
    };
    let shaped = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']' | ':');
    if host.is_empty() || !host.chars().all(shaped) || port == 0 {
        return None;
    }
    let path = if path.is_empty() { "/".to_string() } else { path.to_string() };
    Some((scheme, host.to_ascii_lowercase(), port, path))
}

/// `scheme://host:port`, the one form origins are compared in
pub fn origin_of(url: &str) -> Option<String> {
    let (scheme, host, port, _) = split_url(url)?;
    Some(format!("{scheme}://{host}:{port}"))
}

impl Server {
    pub fn from_address(typed: &str) -> Result<Server, Refused> {
        let typed = typed.trim();
        // Plain printable ASCII and nothing else: a path may not hold a character that turns the sentence a person is asked around
        // (U+202E and its kind), and a name outside ASCII is typed in its punycode form
        if !typed.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(Refused::NotAnAddress);
        }
        let bare = typed.trim_end_matches('/').to_ascii_lowercase();
        let bare = bare.strip_prefix("https://").unwrap_or(&bare);
        for (names, region) in [(["bitwarden.com", "vault.bitwarden.com"], "com"), (["bitwarden.eu", "vault.bitwarden.eu"], "eu")] {
            if names.contains(&bare) {
                return Ok(Server {
                    address: format!("https://vault.bitwarden.{region}"),
                    identity: format!("https://identity.bitwarden.{region}"),
                    api: format!("https://api.bitwarden.{region}"),
                });
            }
        }
        if bare.contains("bitwarden-gov.com") {
            return Err(Refused::Unsupported);
        }
        let with_scheme = if typed.contains("://") { typed.to_string() } else { format!("https://{typed}") };
        let address = with_scheme.trim_end_matches('/').to_string();
        let (scheme, host, _, path) = split_url(&address).ok_or(Refused::NotAnAddress)?;
        if path.contains('?') {
            return Err(Refused::NotAnAddress);
        }
        if scheme != "https" && !loopback(&host) {
            return Err(Refused::NotHttps);
        }
        Ok(Server { identity: format!("{address}/identity"), api: format!("{address}/api"), address })
    }

    /// The origins of the two bases: all a connection ever reaches
    pub fn origins(&self) -> Vec<String> {
        let mut origins: Vec<String> = [&self.identity, &self.api].iter().filter_map(|base| origin_of(base)).collect();
        origins.dedup();
        origins
    }

    /// Only for tests against a server on a test network, which answers plain http there (plans/vault-sync/IMPLEMENTERS.md 3).
    /// The shipped program has no way to it: no setting and no variable.
    #[cfg(any(test, feature = "plain-http-for-tests"))]
    pub fn plain_for_tests(address: &str) -> Server {
        let address = address.trim_end_matches('/').to_string();
        Server { identity: format!("{address}/identity"), api: format!("{address}/api"), address }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_comes_to_two_bases() {
        let cloud = Server::from_address("bitwarden.com").unwrap();
        assert_eq!((cloud.identity.as_str(), cloud.api.as_str()), ("https://identity.bitwarden.com", "https://api.bitwarden.com"));
        assert_eq!(Server::from_address("https://vault.bitwarden.eu/").unwrap().api, "https://api.bitwarden.eu");
        let own = Server::from_address("https://vault.example.test/bw//").unwrap();
        assert_eq!(own.identity, "https://vault.example.test/bw/identity");
        assert_eq!(own.api, "https://vault.example.test/bw/api");
        assert_eq!(own.origins(), vec!["https://vault.example.test:443".to_string()]);
        assert_eq!(Server::from_address("vault.example.test:8443").unwrap().origins(), vec!["https://vault.example.test:8443".to_string()]);
        assert_eq!(cloud.origins().len(), 2);
    }

    #[test]
    fn what_is_not_an_address_is_refused() {
        assert_eq!(Server::from_address("http://vault.example.test"), Err(Refused::NotHttps));
        assert!(Server::from_address("http://127.0.0.1:8080").is_ok());
        assert!(Server::from_address("http://[::1]:8080").is_ok());
        assert_eq!(Server::from_address("https://user@vault.example.test"), Err(Refused::NotAnAddress));
        assert_eq!(Server::from_address("https://vault.example.test/?x=1"), Err(Refused::NotAnAddress));
        assert_eq!(Server::from_address("https://vault.example.test/#x"), Err(Refused::NotAnAddress));
        assert_eq!(Server::from_address("ftp://vault.example.test"), Err(Refused::NotAnAddress));
        assert_eq!(Server::from_address("https://vault example"), Err(Refused::NotAnAddress));
        // A character that turns a sentence around, or any that is not printable ASCII, in a path or a name (the review's L5)
        assert_eq!(Server::from_address("https://vault.example.test/a\u{202e}b"), Err(Refused::NotAnAddress));
        assert_eq!(Server::from_address("https://vault.example.test/\u{200f}"), Err(Refused::NotAnAddress));
        assert_eq!(Server::from_address("https://v\u{e4}ult.example.test"), Err(Refused::NotAnAddress));
        assert_eq!(Server::from_address("https://bitwarden-gov.com"), Err(Refused::Unsupported));
    }

    #[test]
    fn origins_compare_in_one_form() {
        assert_eq!(origin_of("https://Vault.Example.TEST/api/sync?x=1").as_deref(), Some("https://vault.example.test:443"));
        assert_eq!(origin_of("https://vault.example.test:443/").as_deref(), Some("https://vault.example.test:443"));
        assert_eq!(origin_of("https://a@vault.example.test/"), None);
        assert_eq!(origin_of("https://evil.test\\@vault.example.test/"), None);
    }
}
