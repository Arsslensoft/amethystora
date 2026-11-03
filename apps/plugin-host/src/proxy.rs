//! The one way out of a sandbox (plans/PLAN-plugins.md, "Blocking what a plugin sends"). A plugin's network namespace
//! has only loopback; what it may reach is an HTTPS proxy on a unix socket the host holds for it, which takes `CONNECT
//! host:443` for the hosts it was granted and nothing else. The proxy cannot read inside HTTPS: what it acts on is the
//! name (resolved here, and refused if any address is the machine's own or a private network's, so that DNS cannot
//! point it at the router), the person's blocks, and how many bytes go out to each host in an hour.
//!
//! TODO(phase 0): the default of 1 MiB an hour counts each connection's TLS handshake as sent; measure it before the
//! number is final.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, TcpStream};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::manifest::{DEFAULT_LIMIT, host_matches, valid_host};

/// Whether an address is one a plugin may never be taken to: this machine's, a private network's, a reserved range
pub fn forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => forbidden_v4(a),
        IpAddr::V6(a) => forbidden_v6(a),
    }
}

fn forbidden_v4(a: Ipv4Addr) -> bool {
    let o = a.octets();
    a.is_private()
        || a.is_loopback()
        || a.is_link_local()
        || a.is_unspecified()
        || a.is_broadcast()
        || a.is_multicast()
        || a.is_documentation()
        || o[0] == 0
        || (o[0] == 100 && (64..128).contains(&o[1]))
        || (o[0] == 192 && o[1] == 0 && o[2] == 0)
        || (o[0] == 198 && (o[1] == 18 || o[1] == 19))
        || o[0] >= 240
}

fn forbidden_v6(a: Ipv6Addr) -> bool {
    let s = a.segments();
    let o = a.octets();
    // Addresses that carry an IPv4 one: judged by the one they carry
    let embedded = |b: [u8; 4]| forbidden_v4(Ipv4Addr::from(b));
    a.is_loopback()
        || a.is_unspecified()
        || a.is_multicast()
        || (s[0] & 0xfe00) == 0xfc00
        || (s[0] & 0xffc0) == 0xfe80
        || (s[0] & 0xffc0) == 0xfec0
        || (s[0] == 0x2001 && s[1] == 0x0db8)
        // ::ffff:a.b.c.d and the deprecated ::a.b.c.d
        || (s[..5] == [0; 5] && (s[5] == 0xffff || s[5] == 0) && embedded([o[12], o[13], o[14], o[15]]))
        // NAT64's 64:ff9b::/96 and 6to4's 2002::/16
        || (s[0] == 0x64 && s[1] == 0xff9b && s[2..6] == [0; 4] && embedded([o[12], o[13], o[14], o[15]]))
        || (s[0] == 0x2002 && embedded([o[2], o[3], o[4], o[5]]))
}

/// What the person and the machine allow one plugin to reach, read again whenever it is asked
#[derive(Clone, Debug, Default)]
pub struct Policy {
    pub allowed: Vec<String>,
    pub blocked_all: bool,
    pub blocked_hosts: Vec<String>,
    pub limits: BTreeMap<String, u64>,
    /// `BLOCK_NETWORK=yes` in plugins.conf: no plugin on the machine reaches the network
    pub machine_off: bool,
}

impl Policy {
    /// The allowed entry that covers `host`, if the plugin may reach it at all
    pub fn entry_for(&self, host: &str) -> Option<&String> {
        if self.machine_off || self.blocked_all || self.blocked_hosts.iter().any(|b| host_matches(b, host) || b == host) {
            return None;
        }
        self.allowed.iter().find(|e| host_matches(e, host))
    }

    pub fn allows(&self, host: &str, port: u16) -> bool {
        port == 443 && valid_host(host) && !host.starts_with("*.") && self.entry_for(host).is_some()
    }

    pub fn limit_for(&self, host: &str) -> u64 {
        self.allowed.iter().filter(|e| host_matches(e, host)).find_map(|e| self.limits.get(e)).copied().unwrap_or(DEFAULT_LIMIT)
    }
}

/// `CONNECT host:port HTTP/1.1` and its headers, as (host, port). Anything else is no request this proxy answers
pub fn parse_connect(head: &str) -> Option<(String, u16)> {
    let line = head.lines().next()?;
    let mut words = line.split(' ');
    if words.next()? != "CONNECT" {
        return None;
    }
    let target = words.next()?;
    if !words.next()?.starts_with("HTTP/1.") || words.next().is_some() {
        return None;
    }
    let (host, port) = target.rsplit_once(':')?;
    // An address in brackets or a bare one is not a name
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    Some((host, port.parse().ok()?))
}

type Resolver = Box<dyn Fn(&str, u16) -> std::io::Result<Vec<IpAddr>> + Send + Sync>;
type Dialer = Box<dyn Fn(&[IpAddr], u16) -> std::io::Result<TcpStream> + Send + Sync>;
type Loader = Box<dyn Fn() -> Policy + Send + Sync>;
type Told = Box<dyn Fn(&str) + Send + Sync>;

pub struct Proxy {
    load: Loader,
    resolve: Resolver,
    dial: Dialer,
    /// Told once when a host passes its limit and is blocked
    over_limit: Told,
    sent: Mutex<HashMap<String, (Instant, u64)>>,
    over: Mutex<HashSet<String>>,
    /// The last few refusals, for a log: a plugin that is refused over and over must not grow the host
    pub refused: Mutex<Vec<String>>,
    active: AtomicUsize,
}

/// Connections open at once for one plugin: more are refused, never queued, so that threads and descriptors stay within the unit's limits
const MAX_CONNECTIONS: usize = 16;
const MAX_REFUSED: usize = 64;

struct Conn {
    done: AtomicBool,
}

impl Proxy {
    pub fn new(load: Loader, resolve: Resolver, dial: Dialer, over_limit: Told) -> Arc<Proxy> {
        Arc::new(Proxy { load, resolve, dial, over_limit, sent: Mutex::new(HashMap::new()), over: Mutex::new(HashSet::new()), refused: Mutex::new(Vec::new()), active: AtomicUsize::new(0) })
    }

    /// The real one: the system's resolver (which refuses nothing itself) and a connection with a 10 second limit
    pub fn system(load: Loader, over_limit: Told) -> Arc<Proxy> {
        use std::net::ToSocketAddrs;
        Proxy::new(
            load,
            Box::new(|host, port| Ok((host, port).to_socket_addrs()?.map(|a| a.ip()).collect())),
            Box::new(|ips, port| {
                let mut last = std::io::Error::other("no address");
                for ip in ips {
                    match TcpStream::connect_timeout(&(*ip, port).into(), Duration::from_secs(10)) {
                        Ok(stream) => return Ok(stream),
                        Err(e) => last = e,
                    }
                }
                Err(last)
            }),
            over_limit,
        )
    }

    /// Counts what went out to the allowed entry `entry` (a wildcard's hosts share one allowance); true when this passes the hour's limit
    fn count(&self, host: &str, limit: u64, bytes: u64) -> bool {
        let mut sent = self.sent.lock().unwrap();
        let entry = sent.entry(host.to_string()).or_insert((Instant::now(), 0));
        if entry.0.elapsed() >= Duration::from_secs(3600) {
            *entry = (Instant::now(), 0);
        }
        entry.1 += bytes;
        entry.1 > limit
    }

    pub fn bytes_sent(&self) -> BTreeMap<String, u64> {
        self.sent.lock().unwrap().iter().map(|(h, (_, n))| (h.clone(), *n)).collect()
    }

    fn blocked_now(&self, entry: &str, host: &str) -> bool {
        self.over.lock().unwrap().contains(entry) || !(self.load)().allows(host, 443)
    }

    /// Answers one connection from the sandbox's forwarder, to the end
    pub fn handle(self: &Arc<Self>, mut client: UnixStream) {
        if self.active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            self.active.fetch_sub(1, Ordering::SeqCst);
            self.note_refused("too many connections".into());
            let _ = client.write_all(b"HTTP/1.1 403 Forbidden
Connection: close

");
            return;
        }
        self.serve_one(client);
        self.active.fetch_sub(1, Ordering::SeqCst);
    }

    fn note_refused(&self, why: String) {
        let mut refused = self.refused.lock().unwrap();
        if refused.len() >= MAX_REFUSED {
            refused.remove(0);
        }
        refused.push(why);
    }

    fn serve_one(self: &Arc<Self>, mut client: UnixStream) {
        let _ = client.set_read_timeout(Some(Duration::from_secs(10)));
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while head.len() < 8192 && !head.ends_with(b"\r\n\r\n") {
            match client.read(&mut byte) {
                Ok(1) => head.push(byte[0]),
                _ => return,
            }
        }
        let refuse = |client: &mut UnixStream, why: String| {
            self.note_refused(why);
            // Every refusal is told the same, so that a plugin learns nothing from which rule it hit
            let _ = client.write_all(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n");
        };
        let Some((host, port)) = std::str::from_utf8(&head).ok().and_then(parse_connect) else {
            return refuse(&mut client, "not a CONNECT".into());
        };
        let policy = (self.load)();
        let entry = policy.entry_for(&host).cloned().unwrap_or_default();
        if !policy.allows(&host, port) || self.over.lock().unwrap().contains(&entry) {
            return refuse(&mut client, format!("{host}:{port} is not allowed"));
        }
        // Resolved here, once, and the connection goes to what was checked: a host that answers with a private address is refused
        let ips = match (self.resolve)(&host, port) {
            Ok(ips) if !ips.is_empty() && !ips.iter().any(|ip| forbidden_ip(*ip)) => ips,
            _ => return refuse(&mut client, format!("{host} does not resolve to a public address")),
        };
        let Ok(upstream) = (self.dial)(&ips, port) else {
            return refuse(&mut client, format!("{host} did not answer"));
        };
        if client.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").is_err() {
            return;
        }
        let _ = client.set_read_timeout(None);
        let limit = policy.limit_for(&host);
        let conn = Arc::new(Conn { done: AtomicBool::new(false) });
        let (Ok(client_out), Ok(upstream_in)) = (client.try_clone(), upstream.try_clone()) else { return };
        // Down: the host's answers to the plugin
        let Ok(down) = std::thread::Builder::new().spawn({
            let (mut from, mut to) = (upstream_in, client_out);
            move || {
                let _ = std::io::copy(&mut from, &mut to);
                let _ = to.shutdown(Shutdown::Write);
            }
        }) else { return };
        // Watch: a block, or the limit, cuts what is open at once
        let watch = std::thread::Builder::new().spawn({
            let (proxy, conn, host, entry) = (self.clone(), conn.clone(), host.clone(), entry.clone());
            let (client, upstream) = (client.try_clone(), upstream.try_clone());
            move || {
                while !conn.done.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(250));
                    if proxy.blocked_now(&entry, &host) {
                        if let (Ok(c), Ok(u)) = (&client, &upstream) {
                            let _ = c.shutdown(Shutdown::Both);
                            let _ = u.shutdown(Shutdown::Both);
                        }
                        break;
                    }
                }
            }
        });
        let watch = watch.ok();
        // Up: what the plugin sends, counted
        let mut buffer = [0u8; 16384];
        let mut upstream_out = upstream;
        loop {
            match client.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if self.count(&entry, limit, n as u64) {
                        if self.over.lock().unwrap().insert(entry.clone()) {
                            (self.over_limit)(&entry);
                        }
                        let _ = client.shutdown(Shutdown::Both);
                        let _ = upstream_out.shutdown(Shutdown::Both);
                        break;
                    }
                    if upstream_out.write_all(&buffer[..n]).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = upstream_out.shutdown(Shutdown::Both);
        let _ = down.join();
        conn.done.store(true, Ordering::Relaxed);
        if let Some(watch) = watch {
            let _ = watch.join();
        }
    }

    /// Serves a listener until the process ends: one thread for each connection
    pub fn serve(self: &Arc<Self>, listener: UnixListener) {
        for client in listener.incoming().flatten() {
            let proxy = self.clone();
            // A thread that cannot be made refuses the connection by dropping it, and the host goes on
            let _ = std::thread::Builder::new().spawn(move || proxy.handle(client));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn address_rules() {
        for bad in [
            "127.0.0.1", "10.0.0.5", "172.16.3.4", "192.168.1.1", "169.254.1.1", "0.0.0.0", "100.64.0.1", "224.0.0.1", "255.255.255.255", "198.18.0.1", "240.0.0.1",
            "::1", "::", "fe80::1", "fc00::1", "fd12::1", "ff02::1", "::ffff:192.168.1.1", "::ffff:127.0.0.1", "64:ff9b::a00:1", "2002:c0a8:101::1", "2001:db8::1", "fec0::1",
        ] {
            assert!(forbidden_ip(bad.parse().unwrap()), "{bad}");
        }
        for good in ["8.8.8.8", "140.82.112.5", "2606:4700::1111", "::ffff:8.8.8.8", "2001:4860:4860::8888"] {
            assert!(!forbidden_ip(good.parse().unwrap()), "{good}");
        }
    }

    #[test]
    fn the_request_and_the_policy() {
        assert_eq!(parse_connect("CONNECT API.github.com.:443 HTTP/1.1\r\nHost: x\r\n\r\n"), Some(("api.github.com".into(), 443)));
        for bad in ["GET / HTTP/1.1\r\n\r\n", "CONNECT a.com:443 SPDY/3\r\n\r\n", "CONNECT a.com HTTP/1.1\r\n\r\n", "CONNECT a.com:x HTTP/1.1\r\n\r\n", "", "CONNECT a.com:443 HTTP/1.1 extra\r\n\r\n"] {
            assert!(parse_connect(bad).is_none(), "{bad:?}");
        }
        let mut p = Policy { allowed: vec!["api.github.com".into(), "*.example.org".into()], ..Policy::default() };
        assert!(p.allows("api.github.com", 443) && p.allows("a.example.org", 443));
        assert!(!p.allows("api.github.com", 22) && !p.allows("github.com", 443) && !p.allows("example.org", 443));
        assert!(!p.allows("192.168.1.1", 443) && !p.allows("localhost", 443));
        p.blocked_hosts.push("api.github.com".into());
        assert!(!p.allows("api.github.com", 443) && p.allows("a.example.org", 443));
        p.blocked_all = true;
        assert!(!p.allows("a.example.org", 443));
        let mut q = Policy { allowed: vec!["a.example.com".into()], machine_off: true, ..Policy::default() };
        assert!(!q.allows("a.example.com", 443));
        q.machine_off = false;
        assert_eq!(q.limit_for("a.example.com"), DEFAULT_LIMIT);
        q.limits.insert("a.example.com".into(), 5);
        assert_eq!(q.limit_for("a.example.com"), 5);
    }

    /// A proxy whose upstream is a local echo server, whatever name is asked, with the resolver saying `answer`
    fn rig(policy: Policy, answer: &'static str) -> (Arc<Proxy>, Arc<Mutex<Vec<String>>>) {
        let echo = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = echo.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for mut s in echo.incoming().flatten() {
                std::thread::spawn(move || {
                    let mut b = [0u8; 4096];
                    while let Ok(n) = s.read(&mut b) {
                        if n == 0 || s.write_all(&b[..n]).is_err() {
                            break;
                        }
                    }
                });
            }
        });
        let told = Arc::new(Mutex::new(Vec::new()));
        let t = told.clone();
        let shared = Arc::new(Mutex::new(policy));
        let proxy = Proxy::new(
            Box::new(move || shared.lock().unwrap().clone()),
            Box::new(move |_, _| Ok(vec![answer.parse().unwrap()])),
            Box::new(move |_, _| TcpStream::connect(("127.0.0.1", port))),
            Box::new(move |h| t.lock().unwrap().push(h.to_string())),
        );
        (proxy, told)
    }

    fn ask(proxy: &Arc<Proxy>, request: &str) -> (UnixStream, String) {
        let (mine, theirs) = UnixStream::pair().unwrap();
        let p = proxy.clone();
        std::thread::spawn(move || p.handle(theirs));
        (&mine).write_all(request.as_bytes()).unwrap();
        let mut first = Vec::new();
        let mut b = [0u8; 1];
        while !first.ends_with(b"\r\n\r\n") && (&mine).read(&mut b).unwrap_or(0) == 1 {
            first.push(b[0]);
        }
        (mine, String::from_utf8_lossy(&first).into_owned())
    }

    fn allowing(host: &str) -> Policy {
        Policy { allowed: vec![host.into()], ..Policy::default() }
    }

    #[test]
    fn refuses_port_22_a_rebinding_host_and_a_stranger() {
        let (proxy, _) = rig(allowing("a.example.com"), "8.8.8.8");
        assert!(ask(&proxy, "CONNECT a.example.com:22 HTTP/1.1\r\n\r\n").1.starts_with("HTTP/1.1 403"));
        assert!(ask(&proxy, "CONNECT b.example.com:443 HTTP/1.1\r\n\r\n").1.starts_with("HTTP/1.1 403"));
        assert!(ask(&proxy, "CONNECT 192.168.1.1:443 HTTP/1.1\r\n\r\n").1.starts_with("HTTP/1.1 403"));
        assert!(ask(&proxy, "GET http://a.example.com/ HTTP/1.1\r\n\r\n").1.starts_with("HTTP/1.1 403"));
        let (rebinding, _) = rig(allowing("a.example.com"), "192.168.1.7");
        assert!(ask(&rebinding, "CONNECT a.example.com:443 HTTP/1.1\r\n\r\n").1.starts_with("HTTP/1.1 403"), "DNS pointing at the router");
        let (v6, _) = rig(allowing("a.example.com"), "::ffff:10.0.0.1");
        assert!(ask(&v6, "CONNECT a.example.com:443 HTTP/1.1\r\n\r\n").1.starts_with("HTTP/1.1 403"));
    }

    #[test]
    fn carries_bytes_and_counts_them() {
        let (proxy, _) = rig(allowing("a.example.com"), "8.8.8.8");
        let (mut c, head) = ask(&proxy, "CONNECT a.example.com:443 HTTP/1.1\r\n\r\n");
        assert!(head.starts_with("HTTP/1.1 200"));
        c.write_all(b"hello").unwrap();
        let mut back = [0u8; 5];
        c.read_exact(&mut back).unwrap();
        assert_eq!(&back, b"hello");
        assert_eq!(proxy.bytes_sent()["a.example.com"], 5);
    }

    #[test]
    fn sending_past_the_limit_blocks_the_host_and_cuts_the_connection() {
        let mut policy = allowing("a.example.com");
        policy.limits.insert("a.example.com".into(), 100);
        let (proxy, told) = rig(policy, "8.8.8.8");
        let (mut c, head) = ask(&proxy, "CONNECT a.example.com:443 HTTP/1.1\r\n\r\n");
        assert!(head.starts_with("HTTP/1.1 200"));
        c.write_all(&[7u8; 4000]).unwrap();
        // The connection ends: the proxy cut it, and a new one is refused
        let mut sink = Vec::new();
        let _ = c.read_to_end(&mut sink);
        assert_eq!(told.lock().unwrap().as_slice(), ["a.example.com"]);
        assert!(ask(&proxy, "CONNECT a.example.com:443 HTTP/1.1\r\n\r\n").1.starts_with("HTTP/1.1 403"));
    }


    #[test]
    fn a_wildcard_shares_one_allowance() {
        let mut policy = allowing("*.example.org");
        policy.limits.insert("*.example.org".into(), 100);
        let (proxy, told) = rig(policy, "8.8.8.8");
        for host in ["a1.example.org", "a2.example.org"] {
            let (mut c, head) = ask(&proxy, &format!("CONNECT {host}:443 HTTP/1.1\r\n\r\n"));
            assert!(head.starts_with("HTTP/1.1 200"), "{host}");
            c.write_all(&[7u8; 80]).unwrap();
            let mut sink = Vec::new();
            let _ = c.set_read_timeout(Some(Duration::from_secs(3)));
            let _ = c.read_to_end(&mut sink);
        }
        assert_eq!(told.lock().unwrap().as_slice(), ["*.example.org"]);
        assert!(ask(&proxy, "CONNECT a3.example.org:443 HTTP/1.1\r\n\r\n").1.starts_with("HTTP/1.1 403"));
    }

    #[test]
    fn a_block_cuts_a_connection_that_is_open() {
        let shared = Arc::new(Mutex::new(allowing("a.example.com")));
        let echo = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = echo.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for s in echo.incoming().flatten() {
                std::mem::forget(s);
            }
        });
        let load = shared.clone();
        let proxy = Proxy::new(
            Box::new(move || load.lock().unwrap().clone()),
            Box::new(|_, _| Ok(vec!["8.8.8.8".parse().unwrap()])),
            Box::new(move |_, _| TcpStream::connect(("127.0.0.1", port))),
            Box::new(|_| {}),
        );
        let (mut c, head) = ask(&proxy, "CONNECT a.example.com:443 HTTP/1.1\r\n\r\n");
        assert!(head.starts_with("HTTP/1.1 200"));
        shared.lock().unwrap().blocked_hosts.push("a.example.com".into());
        c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut b = [0u8; 1];
        // End of file, not a timeout: the proxy shut the connection by itself
        assert_eq!(c.read(&mut b).unwrap_or(0), 0);
    }
}
