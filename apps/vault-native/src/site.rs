//! Which items a page may be offered. A page's origin is the browser's word, handed on by the extension's background script.
//!
//! Two hosts are the same site when they have the same registrable domain, by the public suffix list on the machine. That list
//! is libpsl's file, which every image has since curl needs it. Without it nothing can be told apart, so nothing fills.
//!
//! - An item's address saying nothing about its scheme, or saying https, fills only over https. Saying http fills over either.
//! - A host with no registrable domain (an IP address, `localhost`, a public suffix itself, a name with one label) matches only
//!   itself. A page on a port other than its scheme's matches only an address naming that port.
//! - A frame fills only if it is the same site as the top-level page.
//! - An authenticator entry has no address. It matches when its issuer names this domain, is the name of a login that matches
//!   here, or is a service whose 2fa.directory row lists this domain. Never by a name resembling the domain's first label:
//!   `github.example` is not GitHub. Anything else the person has to pick, and sees the domain first.

use std::path::Path;

/// `publicsuffix-list-dafsa`, which libpsl requires
pub const SUFFIX_LIST: &str = "/usr/share/publicsuffix/public_suffix_list.dafsa";
/// A trimmed copy of 2fa.directory (MIT, named in NOTICE). Without it, codes match only by the two other ways.
pub const DIRECTORY: &str = "/usr/share/amethystora/vault/2fa-directory.json";

const MAX_LIST: u64 = 16 * 1024 * 1024;
const MAX_DIRECTORY: u64 = 4 * 1024 * 1024;
const HEADER: &[u8] = b".DAFSA@PSL_";
const HEADER_LENGTH: usize = 16;
const EXCEPTION: u8 = 1;
const WILDCARD: u8 = 2;

/// The public suffix list, as libpsl keeps it: a graph of every rule, each stored in its `xn--` form as well, which is the one
/// a browser's origin is in. Looked up, never parsed, so a short-lived process pays for no table.
pub struct Suffixes {
    graph: Vec<u8>,
}

impl Suffixes {
    /// `None` when it is missing, or anything but a list with rules everybody has in it
    pub fn load(path: &Path) -> Option<Suffixes> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path).ok()?.take(MAX_LIST).read_to_end(&mut bytes).ok()?;
        Suffixes::from_bytes(bytes)
    }

    pub fn from_bytes(mut bytes: Vec<u8>) -> Option<Suffixes> {
        if bytes.len() < HEADER_LENGTH || !bytes.starts_with(HEADER) || bytes[HEADER_LENGTH - 1] != b'\n' {
            return None;
        }
        bytes.drain(..HEADER_LENGTH);
        let list = Suffixes { graph: bytes };
        (list.lookup(b"com").is_some() && list.lookup(b"co.uk").is_some()).then_some(list)
    }

    /// What the graph holds for exactly this name. A node is its label's characters, the last with its high bit set, then the
    /// offsets of its children, or a value alone. A damaged file comes to `None`: every step reads through `get`, and each
    /// time it goes down a level, a byte of the name has been used up.
    fn lookup(&self, name: &[u8]) -> Option<u8> {
        let graph = &self.graph;
        let (mut list, mut child, mut used) = (0usize, 0usize, 0usize);
        loop {
            let first = *graph.get(list)?;
            let (step, width) = match first & 0x60 {
                0x60 => ((usize::from(first & 0x1f) << 16) | (usize::from(*graph.get(list + 1)?) << 8) | usize::from(*graph.get(list + 2)?), 3),
                0x40 => ((usize::from(first & 0x1f) << 8) | usize::from(*graph.get(list + 1)?), 2),
                _ => (usize::from(first & 0x3f), 1),
            };
            child += step;
            let (mut at, mut read) = (child, used);
            let mut same = true;
            while same && *graph.get(at)? < 0x80 {
                same = name.get(read) == Some(&graph[at]);
                at += 1;
                read += 1;
            }
            if same {
                let end = graph[at];
                if end & 0xe0 == 0x80 {
                    if read == name.len() {
                        return Some(end & 0x1f);
                    }
                } else if name.get(read) == Some(&(end & 0x7f)) {
                    (list, child, used) = (at + 1, at + 1, read + 1);
                    continue;
                }
            }
            if first & 0x80 != 0 {
                return None;
            }
            list += width;
        }
    }

    /// The part of `host` a site owns: one label more than its public suffix. `None` when it has no such part.
    pub fn registrable<'a>(&self, host: &'a str) -> Option<&'a str> {
        if is_address(host) {
            return None;
        }
        let dots: Vec<usize> = host.match_indices('.').map(|(at, _)| at).collect();
        // Where each candidate suffix starts, from the longest
        let starts: Vec<usize> = std::iter::once(0).chain(dots.iter().map(|at| at + 1)).collect();
        for (index, start) in starts.iter().copied().enumerate() {
            let flags = self.lookup(&host.as_bytes()[start..]);
            if flags.is_some_and(|flags| flags & EXCEPTION != 0) {
                return Some(&host[start..]);
            }
            let parent = starts.get(index + 1).and_then(|next| self.lookup(&host.as_bytes()[*next..]));
            let wild = parent.is_some_and(|flags| flags & WILDCARD != 0);
            // An unknown last label is a suffix too, the list's implied `*`
            if flags.is_some() || wild || index + 1 == starts.len() {
                return (index > 0).then(|| &host[starts[index - 1]..]);
            }
        }
        None
    }

    /// Whether two hosts are one site. A host with no registrable domain is only itself.
    pub fn same_site(&self, one: &str, other: &str) -> bool {
        one == other || matches!((self.registrable(one), self.registrable(other)), (Some(a), Some(b)) if a == b)
    }
}

/// An IPv4 address in the form an origin has it, or an IPv6 one in brackets
fn is_address(host: &str) -> bool {
    host.starts_with('[') || host.parse::<std::net::Ipv4Addr>().is_ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    Https,
    Http,
}

/// A page's origin, exactly as a browser writes one: `https://host` or `https://host:port`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    pub scheme: Scheme,
    pub host: String,
    pub port: Option<u16>,
}

fn split_port(authority: &str) -> Option<(&str, Option<u16>)> {
    // The colon of a port, not one inside an IPv6 address's brackets
    let colon = authority.rfind(':').filter(|at| !authority[*at..].contains(']'));
    match colon {
        Some(at) => {
            let port = &authority[at + 1..];
            let digits = !port.is_empty() && port.len() <= 5 && port.bytes().all(|byte| byte.is_ascii_digit());
            Some((&authority[..at], Some(port.parse().ok().filter(|_| digits).filter(|port| *port != 0)?)))
        }
        None => Some((authority, None)),
    }
}

/// A host as an origin holds it: lowercase, in its `xn--` form, with no empty label
fn host_shape(host: &str) -> bool {
    if host.starts_with('[') {
        return host.ends_with(']') && host.len() <= 47 && host[1..host.len() - 1].parse::<std::net::Ipv6Addr>().is_ok();
    }
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| !label.is_empty() && label.len() <= 63)
        && host.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'.' || byte == b'_')
}

impl Origin {
    /// Refuses anything a browser would not write as the origin of an http or https page: `null`, `about:blank`, an extension's,
    /// a path, a login in the address, capitals, a name not in its `xn--` form, a port that is the scheme's own.
    pub fn parse(text: &str) -> Option<Origin> {
        let (scheme, rest) = if let Some(rest) = text.strip_prefix("https://") {
            (Scheme::Https, rest)
        } else {
            (Scheme::Http, text.strip_prefix("http://")?)
        };
        let (host, port) = split_port(rest)?;
        let default = if scheme == Scheme::Https { 443 } else { 80 };
        (host_shape(host) && port != Some(default)).then(|| Origin { scheme, host: host.to_string(), port })
    }
}

/// How well an item's address fits a page
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    No,
    /// The same site, on another of its hosts: the person sees which before it fills
    SameSite,
    Exact,
}

/// An address as a person wrote it: a scheme or not, a path or not, capitals, a name in its own script
struct Address {
    scheme: Option<Scheme>,
    host: String,
    port: Option<u16>,
}

fn read_address(text: &str) -> Option<Address> {
    let text = text.trim();
    let (scheme, rest) = match text.split_once("://") {
        Some((scheme, rest)) => match scheme.to_ascii_lowercase().as_str() {
            "https" => (Some(Scheme::Https), rest),
            "http" => (Some(Scheme::Http), rest),
            _ => return None,
        },
        None => (None, text),
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    // A login in an address is how `https://github.com@evil.example` reads as GitHub
    if authority.contains('@') {
        return None;
    }
    let (host, port) = split_port(authority)?;
    let host = host.strip_suffix('.').unwrap_or(host);
    let host = ascii_host(host)?;
    host_shape(&host).then_some(Address { scheme, host, port })
}

/// A host in its `xn--` form. Lowercased but not otherwise mapped: a name that needed more than that only fails to match.
pub fn ascii_host(host: &str) -> Option<String> {
    if host.is_ascii() {
        return Some(host.to_ascii_lowercase());
    }
    let labels: Option<Vec<String>> = host.to_lowercase().split('.').map(|label| if label.is_ascii() { Some(label.to_string()) } else { punycode(label).map(|code| format!("xn--{code}")) }).collect();
    Some(labels?.join("."))
}

/// RFC 3492's encoding of a label
fn punycode(label: &str) -> Option<String> {
    let (base, tmin, tmax, skew, damp) = (36u32, 1u32, 26u32, 38u32, 700u32);
    let digit = |value: u32| char::from(if value < 26 { b'a' + value as u8 } else { b'0' + (value - 26) as u8 });
    let adapt = |mut delta: u32, points: u32, first: bool| {
        delta = if first { delta / damp } else { delta / 2 };
        delta += delta / points;
        let mut k = 0;
        while delta > ((base - tmin) * tmax) / 2 {
            delta /= base - tmin;
            k += base;
        }
        k + (((base - tmin + 1) * delta) / (delta + skew))
    };
    let input: Vec<u32> = label.chars().map(u32::from).collect();
    let mut output: String = label.chars().filter(char::is_ascii).collect();
    let basic = output.len() as u32;
    if basic > 0 {
        output.push('-');
    }
    let (mut n, mut delta, mut bias, mut handled) = (128u32, 0u32, 72u32, basic);
    while (handled as usize) < input.len() {
        let next = input.iter().copied().filter(|point| *point >= n).min()?;
        delta = delta.checked_add((next - n).checked_mul(handled + 1)?)?;
        n = next;
        for point in &input {
            if *point < n {
                delta = delta.checked_add(1)?;
            }
            if *point == n {
                let mut q = delta;
                let mut k = base;
                loop {
                    let t = if k <= bias { tmin } else if k >= bias + tmax { tmax } else { k - bias };
                    if q < t {
                        break;
                    }
                    output.push(digit(t + (q - t) % (base - t)));
                    q = (q - t) / (base - t);
                    k += base;
                }
                output.push(digit(q));
                bias = adapt(delta, handled + 1, handled == basic);
                delta = 0;
                handled += 1;
            }
        }
        delta = delta.checked_add(1)?;
        n = n.checked_add(1)?;
    }
    Some(output)
}

/// How an item's address fits a page. An address that cannot be read fits nothing.
pub fn fit(list: &Suffixes, page: &Origin, address: &str) -> Fit {
    let Some(item) = read_address(address) else { return Fit::No };
    let scheme_allows = match item.scheme {
        Some(Scheme::Http) => true,
        Some(Scheme::Https) | None => page.scheme == Scheme::Https,
    };
    let default = if item.scheme == Some(Scheme::Http) { Some(80) } else { Some(443) };
    let item_port = item.port.filter(|port| Some(*port) != default);
    if !scheme_allows || item_port != page.port {
        return Fit::No;
    }
    if item.host == page.host {
        Fit::Exact
    } else if list.same_site(&item.host, &page.host) {
        Fit::SameSite
    } else {
        Fit::No
    }
}

/// How issuers and service names compare: letters and digits alone, whatever their case
fn fold(name: &str) -> String {
    name.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// The bundled 2fa.directory rows, `[name, domain, docs, recovery, [other domains]?]`. Read only when a page has authenticator
/// entries to sort, and gone with the message.
pub struct Directory {
    rows: Vec<(String, Vec<String>)>,
}

impl Directory {
    pub fn load(path: &Path) -> Option<Directory> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path).ok()?.take(MAX_DIRECTORY + 1).read_to_end(&mut bytes).ok()?;
        if bytes.len() as u64 > MAX_DIRECTORY {
            return None;
        }
        Directory::parse(&bytes)
    }

    pub fn parse(bytes: &[u8]) -> Option<Directory> {
        let file: serde_json::Value = serde_json::from_slice(bytes).ok()?;
        let rows = file.get("entries")?.as_array()?.iter().filter_map(|row| {
            let row = row.as_array()?;
            let mut domains = vec![row.get(1)?.as_str()?.to_string()];
            domains.extend(row.get(4).and_then(|list| list.as_array()).into_iter().flatten().filter_map(|domain| domain.as_str().map(str::to_string)));
            Some((fold(row.first()?.as_str()?), domains))
        });
        Some(Directory { rows: rows.collect() })
    }

    fn lists(&self, list: &Suffixes, issuer: &str, host: &str) -> bool {
        let issuer = fold(issuer);
        !issuer.is_empty() && self.rows.iter().filter(|(name, _)| *name == issuer).flat_map(|(_, domains)| domains).any(|domain| list.same_site(domain, host))
    }
}

/// Whether an authenticator entry belongs to this page without the person having to say so. `logins` are the names of the
/// logins that fit it.
pub fn code_fits(list: &Suffixes, directory: Option<&Directory>, page: &Origin, issuer: &str, logins: &[&str]) -> bool {
    let named = issuer.trim().to_ascii_lowercase();
    if !named.is_empty() && (named == page.host || list.registrable(&page.host) == Some(named.as_str())) {
        return true;
    }
    // A login's name keeps its dots: Save names a login after the site it came from, and `ste.am` is not Steam. A name that is the
    // page's own address is not a name anyone chose.
    let spelt = |name: &str| name.chars().filter(|c| c.is_alphanumeric() || *c == '.').flat_map(char::to_lowercase).collect::<String>();
    let folded = spelt(issuer);
    if !folded.is_empty() && logins.iter().any(|login| spelt(login) == folded && folded != page.host && list.registrable(&page.host) != Some(folded.as_str())) {
        return true;
    }
    directory.is_some_and(|directory| directory.lists(list, issuer, &page.host))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> Option<Suffixes> {
        let list = Suffixes::load(Path::new(SUFFIX_LIST));
        if list.is_none() {
            eprintln!("skipped: no {SUFFIX_LIST} here");
        }
        list
    }

    fn page(text: &str) -> Origin {
        Origin::parse(text).unwrap()
    }

    #[test]
    fn registrable_domains_are_the_lists() {
        let Some(list) = list() else { return };
        for (host, want) in [
            ("github.com", Some("github.com")),
            ("www.github.com", Some("github.com")),
            ("a.b.github.com", Some("github.com")),
            ("bbc.co.uk", Some("bbc.co.uk")),
            ("www.bbc.co.uk", Some("bbc.co.uk")),
            ("co.uk", None),
            ("com", None),
            // Private suffixes count: one person's page must never be another's site
            ("github.io", None),
            ("me.github.io", Some("me.github.io")),
            ("x.me.github.io", Some("me.github.io")),
            // Wildcards and their exceptions
            ("a.ck", None),
            ("b.a.ck", Some("b.a.ck")),
            ("www.ck", Some("www.ck")),
            ("x.www.ck", Some("www.ck")),
            // Names the list stores in their xn-- form
            ("xn--55qx5d.cn", None),
            ("foo.xn--55qx5d.cn", Some("foo.xn--55qx5d.cn")),
            // No rule: the last label is the suffix
            ("localhost", None),
            ("foo.unknown-tld", Some("foo.unknown-tld")),
            ("127.0.0.1", None),
            ("[::1]", None),
        ] {
            assert_eq!(list.registrable(host), want, "{host}");
        }
    }

    #[test]
    fn origins_are_only_what_a_browser_writes() {
        assert_eq!(page("https://github.com"), Origin { scheme: Scheme::Https, host: "github.com".into(), port: None });
        assert_eq!(page("http://nas.home.arpa:5000").port, Some(5000));
        assert_eq!(page("https://[::1]:8443").host, "[::1]");
        for bad in [
            "",
            "null",
            "about:blank",
            "file:///etc/passwd",
            "moz-extension://abc",
            "chrome-extension://abcdefghijklmnop",
            "https://",
            "https://github.com/",
            "https://github.com/login",
            "https://github.com?x",
            "https://user@github.com",
            "https://GitHub.com",
            "https://github.com:443",
            "http://github.com:80",
            "https://github.com:0",
            "https://github.com:99999",
            "https://github.com:",
            "https://github..com",
            "https://.github.com",
            "https://github.com.",
            "https://gïthub.com",
            "https://git hub.com",
            "https://[nonsense]",
            "https://[::1",
            " https://github.com",
        ] {
            assert!(Origin::parse(bad).is_none(), "{bad}");
        }
        assert!(Origin::parse(&format!("https://{}.com", "a".repeat(300))).is_none());
    }

    #[test]
    fn an_item_fills_on_its_own_site_alone() {
        let Some(list) = list() else { return };
        let at = |item: &str, on: &str| fit(&list, &page(on), item);
        assert_eq!(at("https://github.com", "https://github.com"), Fit::Exact);
        assert_eq!(at("github.com", "https://github.com"), Fit::Exact);
        assert_eq!(at("HTTPS://GitHub.com/login?x=1#y", "https://github.com"), Fit::Exact);
        assert_eq!(at("https://github.com.", "https://github.com"), Fit::Exact);
        assert_eq!(at("https://github.com", "https://gist.github.com"), Fit::SameSite);
        for look_alike in [
            "https://github.com.evil.example",
            "https://github-com.example",
            "https://githab.com",
            "https://github.co",
            "https://github.com.co.uk",
            "https://xn--gthub-n4a.com",
            "https://evil.example",
        ] {
            assert_eq!(at("https://github.com", look_alike), Fit::No, "{look_alike}");
        }
        // One person's pages are not another's, nor one suffix's sites each other's
        assert_eq!(at("https://me.github.io", "https://you.github.io"), Fit::No);
        assert_eq!(at("https://a.co.uk", "https://b.co.uk"), Fit::No);
        // Addresses that only read as the site
        assert_eq!(at("https://github.com@evil.example", "https://evil.example"), Fit::No);
        assert_eq!(at("https://github.com@evil.example", "https://github.com"), Fit::No);
        assert_eq!(at("javascript:alert(1)", "https://github.com"), Fit::No);
        assert_eq!(at("", "https://github.com"), Fit::No);
    }

    #[test]
    fn plain_http_needs_the_item_to_say_so() {
        let Some(list) = list() else { return };
        let at = |item: &str, on: &str| fit(&list, &page(on), item);
        assert_eq!(at("https://example.com", "http://example.com"), Fit::No);
        assert_eq!(at("example.com", "http://example.com"), Fit::No);
        assert_eq!(at("http://example.com", "http://example.com"), Fit::Exact);
        assert_eq!(at("http://example.com", "https://example.com"), Fit::Exact);
    }

    #[test]
    fn a_host_with_no_site_is_only_itself_and_a_port_counts() {
        let Some(list) = list() else { return };
        let at = |item: &str, on: &str| fit(&list, &page(on), item);
        assert_eq!(at("http://192.168.1.5", "http://192.168.1.5"), Fit::Exact);
        assert_eq!(at("http://192.168.1.5", "http://192.168.1.6"), Fit::No);
        assert_eq!(at("https://[::1]", "https://[::1]"), Fit::Exact);
        assert_eq!(at("https://[::1]", "https://[::2]"), Fit::No);
        assert_eq!(at("http://localhost", "http://localhost"), Fit::Exact);
        assert_eq!(at("https://github.io", "https://github.io"), Fit::Exact);
        assert_eq!(at("https://github.io", "https://me.github.io"), Fit::No);
        assert_eq!(at("https://nas.home.arpa:5001", "https://nas.home.arpa:5001"), Fit::Exact);
        assert_eq!(at("https://nas.home.arpa:5001", "https://nas.home.arpa"), Fit::No);
        assert_eq!(at("https://nas.home.arpa", "https://nas.home.arpa:5001"), Fit::No);
        assert_eq!(at("https://example.com:443", "https://example.com"), Fit::Exact);
    }

    #[test]
    fn a_name_in_its_own_script_finds_its_xn_form() {
        assert_eq!(punycode("bücher").as_deref(), Some("bcher-kva"));
        assert_eq!(punycode("münchen").as_deref(), Some("mnchen-3ya"));
        assert_eq!(punycode("公司").as_deref(), Some("55qx5d"));
        assert_eq!(ascii_host("Bücher.Example").as_deref(), Some("xn--bcher-kva.example"));
        let Some(list) = list() else { return };
        assert_eq!(fit(&list, &page("https://xn--bcher-kva.de"), "https://bücher.de"), Fit::Exact);
    }

    #[test]
    fn a_code_goes_by_the_directory_never_by_a_first_label() {
        let Some(list) = list() else { return };
        let directory = Directory::parse(br#"{"entries":[["GitHub","github.com",0,0],["Google","google.com",0,0,["youtube.com"]],["Bad row"]]}"#).unwrap();
        let fits = |on: &str, issuer: &str, logins: &[&str]| code_fits(&list, Some(&directory), &page(on), issuer, logins);
        assert!(fits("https://github.com", "GitHub", &[]));
        assert!(fits("https://gist.github.com", "github", &[]));
        assert!(fits("https://www.youtube.com", "Google", &[]));
        assert!(fits("https://example.com", "example.com", &[]));
        assert!(fits("https://acme.example", "ACME Corp", &["Acme Corp"]));
        for (on, issuer) in [("https://github.xyz", "GitHub"), ("https://github.com.evil.example", "GitHub"), ("https://evil.example", "GitHub"), ("https://acme.example", "Acme"), ("https://example.com", ""), ("https://github.com", "Google")] {
            assert!(!fits(on, issuer, &[]), "{on} {issuer}");
        }
        assert!(!code_fits(&list, None, &page("https://github.com"), "GitHub", &[]));
    }

    #[test]
    fn a_login_named_after_a_domain_never_stands_for_an_issuer() {
        let Some(list) = list() else { return };
        // The names Save gives logins on look-alike sites, which fold to the real service's name when the dot is dropped
        for (on, issuer) in [("https://ste.am", "Steam"), ("https://twit.ch", "Twitch"), ("https://coinba.se", "Coinbase"), ("https://redd.it", "Reddit")] {
            let named = on.trim_start_matches("https://");
            assert!(!code_fits(&list, None, &page(on), issuer, &[named]), "{on} {issuer}");
        }
        // A name the person chose still counts
        assert!(code_fits(&list, None, &page("https://steampowered.com"), "Steam", &["Steam"]));
    }

    #[test]
    fn what_is_not_a_list_is_none() {
        assert!(Suffixes::from_bytes(Vec::new()).is_none());
        assert!(Suffixes::from_bytes(b"com\nco.uk\n".to_vec()).is_none());
        assert!(Suffixes::from_bytes(b".DAFSA@PSL_0   \n".to_vec()).is_none());
        assert!(Suffixes::from_bytes(b".DAFSA@PSL_0   \n\x00\x00\x00\x00".to_vec()).is_none());
        assert!(Suffixes::load(Path::new("/nonexistent/list")).is_none());
    }

    #[test]
    fn a_damaged_list_only_finds_less() {
        let Ok(bytes) = std::fs::read(SUFFIX_LIST) else { return };
        // Cut anywhere, or with bytes flipped through it, it never panics or hangs
        for cut in (HEADER_LENGTH..bytes.len()).step_by(997) {
            let Some(list) = Suffixes::from_bytes(bytes[..cut].to_vec()) else { continue };
            let _ = list.registrable("www.github.com");
        }
        for seed in 1..64usize {
            let mut damaged = bytes.clone();
            for at in (HEADER_LENGTH..damaged.len()).step_by(seed * 37) {
                damaged[at] ^= 0x55;
            }
            let Some(list) = Suffixes::from_bytes(damaged) else { continue };
            let _ = list.registrable("a.b.c.www.github.com");
        }
    }
}
