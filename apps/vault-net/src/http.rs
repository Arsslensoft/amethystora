//! HTTP/1.1 over TLS to the one server a session names, and nothing else: https only (plain http only to this machine), no
//! redirect followed (a 3xx goes back as it is, and the client refuses it), every read bounded in size and in time, and no
//! method that destroys an item. Connections are kept for the session, so a run's requests share one handshake.

use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use openssl::hash::MessageDigest;
use openssl::ssl::{ShutdownState, SslConnector, SslMethod, SslStream, SslVerifyMode, SslVersion};
use openssl::x509::verify::X509VerifyFlags;
use openssl::x509::{X509VerifyResult, X509};
use zeroize::Zeroizing;

use amethystora_vault_core::sync::address::{loopback, origin_of, split_url};

/// The most a response may be: a /sync of several thousand items
pub const MOST_BODY: usize = 32 * 1024 * 1024;
const MOST_HEAD: usize = 64 * 1024;
const CONNECT: Duration = Duration::from_secs(10);
/// One request, from connecting to its last byte
const WHOLE: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Unreachable,
    Untrusted,
    Timeout,
    TooLarge,
    NotAllowed,
}

impl Failure {
    pub fn name(self) -> &'static str {
        match self {
            Failure::Unreachable => "unreachable",
            Failure::Untrusted => "untrusted",
            Failure::Timeout => "timeout",
            Failure::TooLarge => "too-large",
            Failure::NotAllowed => "not-allowed",
        }
    }
}

enum Stream {
    Plain(TcpStream),
    Tls(Box<SslStream<TcpStream>>),
}

impl Stream {
    fn tcp(&self) -> &TcpStream {
        match self {
            Stream::Plain(tcp) => tcp,
            Stream::Tls(tls) => tls.get_ref(),
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(tcp) => tcp.read(buf),
            Stream::Tls(tls) => {
                let count = tls.read(buf)?;
                // The library reports a connection that ended with no close_notify as an end of file, as it does a clean close. Anyone on the
                // network can end one so, and a body that is read to its end by that is cut short at a place of their choosing (the review's
                // L2): an end of the stream is an end only when the server said it.
                if count == 0 && !buf.is_empty() && !tls.get_shutdown().contains(ShutdownState::RECEIVED) {
                    return Err(std::io::Error::from(ErrorKind::UnexpectedEof));
                }
                Ok(count)
            }
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(tcp) => tcp.write(buf),
            Stream::Tls(tls) => tls.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Stream::Plain(tcp) => tcp.flush(),
            Stream::Tls(tls) => tls.flush(),
        }
    }
}

/// What a session may reach and trust
pub struct Session {
    origins: Vec<String>,
    connector: SslConnector,
    plain_anywhere: bool,
    open: HashMap<String, Stream>,
    /// The fingerprint the person compared, when they trusted a certificate for this connection
    pin: Option<String>,
}

/// The SHA-256 of a certificate's DER, in lowercase hex: what a person compares
pub fn fingerprint(cert: &openssl::x509::X509Ref) -> String {
    cert.digest(MessageDigest::sha256()).map(|digest| digest.iter().map(|b| format!("{b:02x}")).collect()).unwrap_or_default()
}

fn connector(ca: Option<&X509>, verify: bool) -> Result<SslConnector, Failure> {
    let mut builder = SslConnector::builder(SslMethod::tls_client()).map_err(|_| Failure::Unreachable)?;
    builder.set_min_proto_version(Some(SslVersion::TLS1_2)).map_err(|_| Failure::Unreachable)?;
    if let Some(ca) = ca {
        // This connection's own CA, beside the machine's: a server certificate it issued, or the one pinned itself
        builder.cert_store_mut().add_cert(ca.clone()).map_err(|_| Failure::Untrusted)?;
        builder.verify_param_mut().set_flags(X509VerifyFlags::PARTIAL_CHAIN).map_err(|_| Failure::Untrusted)?;
    }
    if !verify {
        builder.set_verify(SslVerifyMode::NONE);
    }
    Ok(builder.build())
}

impl Session {
    /// `ca` is a PEM the person was shown, and `pin` its SHA-256 as they saw it: both, or neither
    pub fn new(origins: Vec<String>, ca: Option<&str>, pin: Option<&str>) -> Result<Session, Failure> {
        if origins.is_empty() || origins.len() > 2 || origins.iter().any(|origin| origin_of(origin).as_deref() != Some(origin.as_str())) {
            return Err(Failure::NotAllowed);
        }
        let ca = match (ca, pin) {
            (Some(pem), Some(pin)) => {
                let cert = X509::from_pem(pem.as_bytes()).map_err(|_| Failure::Untrusted)?;
                if fingerprint(&cert) != pin.to_ascii_lowercase() {
                    return Err(Failure::Untrusted);
                }
                Some(cert)
            }
            (None, None) => None,
            _ => return Err(Failure::Untrusted),
        };
        let pin = ca.as_ref().map(|cert| fingerprint(cert));
        Ok(Session { origins, connector: connector(ca.as_ref(), true)?, plain_anywhere: false, open: HashMap::new(), pin })
    }

    /// Plain http to any host: for the tests against a server on a test network, and nothing shipped
    #[cfg(feature = "plain-http-for-tests")]
    pub fn plain_for_tests(origins: Vec<String>) -> Session {
        Session { origins, connector: connector(None, true).expect("a connector"), plain_anywhere: true, open: HashMap::new(), pin: None }
    }

    pub fn request(&mut self, method: &str, url: &str, headers: &[(String, String)], body: &[u8]) -> Result<(u16, Zeroizing<Vec<u8>>), Failure> {
        let (scheme, host, port, path) = split_url(url).ok_or(Failure::NotAllowed)?;
        let origin = format!("{scheme}://{host}:{port}");
        if !self.origins.contains(&origin) || path.contains('#') {
            return Err(Failure::NotAllowed);
        }
        if scheme == "http" && !loopback(&host) && !self.plain_anywhere {
            return Err(Failure::NotAllowed);
        }
        // Only what the protocol's trash, restore and writes use. DELETE, and POST to an item's `/delete`, destroy it for good.
        let bare = path.split('?').next().unwrap_or("").trim_end_matches('/').to_ascii_lowercase();
        match method {
            "GET" | "PUT" => {}
            "POST" if !bare.ends_with("/delete") => {}
            _ => return Err(Failure::NotAllowed),
        }
        let deadline = Instant::now() + WHOLE;
        let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: keep-alive\r\nContent-Length: {}\r\n", host_header(&host, port, &scheme), body.len());
        for (name, value) in headers {
            let forbidden = ["host", "content-length", "connection", "transfer-encoding", "expect"];
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') || value.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0) || forbidden.contains(&name.to_ascii_lowercase().as_str()) {
                return Err(Failure::NotAllowed);
            }
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str("\r\n");
        let mut message = Zeroizing::new(head.into_bytes());
        message.extend_from_slice(body);

        // A connection kept from before, unless the server has closed it meanwhile
        let mut stream = match self.open.remove(&origin) {
            Some(stream) if !closed(stream.tcp()) => stream,
            _ => self.connect(&scheme, &host, port, deadline)?,
        };
        set_deadline(&stream, deadline)?;
        stream.write_all(&message).map_err(io_failure)?;
        stream.flush().map_err(io_failure)?;
        let (status, body, keep) = read_response(&mut stream, deadline)?;
        if keep {
            self.open.insert(origin, stream);
        }
        Ok((status, body))
    }

    fn connect(&self, scheme: &str, host: &str, port: u16, deadline: Instant) -> Result<Stream, Failure> {
        let tcp = tcp_to(host, port)?;
        set_tcp_deadline(&tcp, deadline)?;
        if scheme == "http" {
            return Ok(Stream::Plain(tcp));
        }
        let name = host.trim_start_matches('[').trim_end_matches(']');
        let failure = |error: openssl::ssl::HandshakeError<TcpStream>| match error {
            openssl::ssl::HandshakeError::Failure(mid) if mid.ssl().verify_result() != X509VerifyResult::OK => Failure::Untrusted,
            openssl::ssl::HandshakeError::WouldBlock(_) => Failure::Timeout,
            _ => Failure::Unreachable,
        };
        match self.connector.connect(name, tcp).map_err(failure) {
            Ok(tls) => Ok(Stream::Tls(Box::new(tls))),
            // The very certificate the person compared whole is the server's: a server at an address its certificate does not
            // name (CN=localhost reached at 10.0.0.111) is still the one they trusted. Its chain is checked as ever; only the name
            // is not, and only for that certificate: one a pinned CA issued must still name the host
            // Whatever stops that second try, the answer is the first's
            Err(Failure::Untrusted) if self.pin.is_some() => {
                let again = || -> Option<Stream> {
                    let tcp = tcp_to(host, port).ok()?;
                    set_tcp_deadline(&tcp, deadline).ok()?;
                    let mut configured = self.connector.configure().ok()?;
                    configured.set_verify_hostname(false);
                    let tls = configured.connect(name, tcp).ok()?;
                    let presented = tls.ssl().peer_certificate().map(|cert| fingerprint(&cert));
                    (presented.is_some() && presented == self.pin).then(|| Stream::Tls(Box::new(tls)))
                };
                again().ok_or(Failure::Untrusted)
            }
            Err(failure) => Err(failure),
        }
    }
}

fn host_header(host: &str, port: u16, scheme: &str) -> String {
    let default = if scheme == "https" { 443 } else { 80 };
    if port == default { host.to_string() } else { format!("{host}:{port}") }
}

fn tcp_to(host: &str, port: u16) -> Result<TcpStream, Failure> {
    let name = host.trim_start_matches('[').trim_end_matches(']');
    let addresses = (name, port).to_socket_addrs().map_err(|_| Failure::Unreachable)?;
    let mut last = Failure::Unreachable;
    for address in addresses.take(8) {
        match TcpStream::connect_timeout(&address, CONNECT) {
            Ok(tcp) => return Ok(tcp),
            Err(error) if error.kind() == ErrorKind::TimedOut => last = Failure::Timeout,
            Err(_) => {}
        }
    }
    Err(last)
}

fn set_tcp_deadline(tcp: &TcpStream, deadline: Instant) -> Result<(), Failure> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(Failure::Timeout);
    }
    tcp.set_read_timeout(Some(left)).and_then(|()| tcp.set_write_timeout(Some(left))).map_err(|_| Failure::Unreachable)
}

fn set_deadline(stream: &Stream, deadline: Instant) -> Result<(), Failure> {
    set_tcp_deadline(stream.tcp(), deadline)
}

/// A kept connection the server has closed reads as ready: end of file, or something nobody asked for
fn closed(tcp: &TcpStream) -> bool {
    let mut poll = libc::pollfd { fd: std::os::fd::AsRawFd::as_raw_fd(tcp), events: libc::POLLIN, revents: 0 };
    // SAFETY: one pollfd, not waiting
    unsafe { libc::poll(&mut poll, 1, 0) != 0 }
}

fn io_failure(error: std::io::Error) -> Failure {
    match error.kind() {
        ErrorKind::WouldBlock | ErrorKind::TimedOut => Failure::Timeout,
        _ => Failure::Unreachable,
    }
}

/// Reads into `buffer` until it holds a whole head, within the limits
struct Reader<'a> {
    stream: &'a mut Stream,
    buffer: Zeroizing<Vec<u8>>,
    deadline: Instant,
}

impl Reader<'_> {
    fn more(&mut self) -> Result<usize, Failure> {
        set_deadline(self.stream, self.deadline)?;
        let mut chunk = Zeroizing::new([0u8; 16 * 1024]);
        let count = self.stream.read(&mut chunk[..]).map_err(io_failure)?;
        self.buffer.extend_from_slice(&chunk[..count]);
        Ok(count)
    }

    /// The bytes up to and including the first `\r\n` at or after `from`, taken out of the buffer
    fn line(&mut self) -> Result<Zeroizing<Vec<u8>>, Failure> {
        loop {
            if let Some(at) = self.buffer.windows(2).position(|pair| pair == b"\r\n") {
                let line: Vec<u8> = self.buffer.drain(..at + 2).collect();
                return Ok(Zeroizing::new(line[..at].to_vec()));
            }
            if self.buffer.len() > MOST_HEAD {
                return Err(Failure::TooLarge);
            }
            if self.more()? == 0 {
                return Err(Failure::Unreachable);
            }
        }
    }

    fn exactly(&mut self, count: usize, into: &mut Zeroizing<Vec<u8>>) -> Result<(), Failure> {
        // Not `into.len() + count`: a chunk's size is the server's, and a sum of that wraps
        if count > MOST_BODY.saturating_sub(into.len()) {
            return Err(Failure::TooLarge);
        }
        while self.buffer.len() < count {
            if self.more()? == 0 {
                return Err(Failure::Unreachable);
            }
        }
        into.extend(self.buffer.drain(..count));
        Ok(())
    }
}

/// The status, the body, and whether the connection may be used again
fn read_response(stream: &mut Stream, deadline: Instant) -> Result<(u16, Zeroizing<Vec<u8>>, bool), Failure> {
    let mut reader = Reader { stream, buffer: Zeroizing::new(Vec::new()), deadline };
    loop {
        let status_line = reader.line()?;
        let text = std::str::from_utf8(&status_line).map_err(|_| Failure::Unreachable)?;
        let mut words = text.split(' ');
        let version = words.next().unwrap_or("");
        let status: u16 = words.next().and_then(|code| code.parse().ok()).filter(|code| (100..600).contains(code)).ok_or(Failure::Unreachable)?;
        if !version.starts_with("HTTP/1.") {
            return Err(Failure::Unreachable);
        }
        let (mut length, mut chunked, mut keep) = (None, false, version == "HTTP/1.1");
        let mut count = 0;
        loop {
            let line = reader.line()?;
            if line.is_empty() {
                break;
            }
            count += 1;
            if count > 128 {
                return Err(Failure::TooLarge);
            }
            let line = String::from_utf8_lossy(&line).to_string();
            let Some((name, value)) = line.split_once(':') else { return Err(Failure::Unreachable) };
            let (name, value) = (name.trim().to_ascii_lowercase(), value.trim().to_ascii_lowercase());
            match name.as_str() {
                "content-length" => {
                    let parsed: usize = value.parse().map_err(|_| Failure::Unreachable)?;
                    if length.is_some_and(|known| known != parsed) {
                        return Err(Failure::Unreachable);
                    }
                    length = Some(parsed);
                }
                "transfer-encoding" => chunked = value.split(',').any(|coding| coding.trim() == "chunked"),
                "connection" if value.contains("close") => keep = false,
                _ => {}
            }
        }
        // A 1xx is not the answer: the real one follows
        if (100..200).contains(&status) {
            continue;
        }
        let mut body = Zeroizing::new(Vec::new());
        if chunked {
            loop {
                let size_line = reader.line()?;
                let size_text = String::from_utf8_lossy(&size_line);
                let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16).map_err(|_| Failure::Unreachable)?;
                if size == 0 {
                    // Trailers, then the empty line
                    while !reader.line()?.is_empty() {}
                    break;
                }
                reader.exactly(size, &mut body)?;
                if !reader.line()?.is_empty() {
                    return Err(Failure::Unreachable);
                }
            }
        } else if let Some(length) = length {
            if length > MOST_BODY {
                return Err(Failure::TooLarge);
            }
            reader.exactly(length, &mut body)?;
        } else if status != 204 && status != 304 {
            // Until the server closes
            keep = false;
            body.extend(reader.buffer.drain(..));
            while reader.more()? > 0 {
                if reader.buffer.len() + body.len() > MOST_BODY {
                    return Err(Failure::TooLarge);
                }
                body.extend(reader.buffer.drain(..));
            }
        }
        // Anything left over is a server ahead of itself: never used for the next request
        if !reader.buffer.is_empty() {
            keep = false;
        }
        return Ok((status, body, keep));
    }
}

/// A server's certificate chain, as the person is shown it before trusting a private CA
pub struct Seen {
    pub trusted: bool,
    pub chain: Vec<(String, String, String, String, bool)>,
}

fn names(name: &openssl::x509::X509NameRef) -> String {
    name.entries()
        .filter_map(|entry| Some(format!("{}={}", entry.object().nid().short_name().ok()?, String::from_utf8_lossy(entry.data().as_slice()).chars().filter(|c| !c.is_control()).collect::<String>())))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Connects without trusting, and says whether the machine's own trust would have, and what the server showed:
/// subject, issuer, SHA-256, PEM and whether it signed itself, the server's certificate first
pub fn probe(origin: &str) -> Result<Seen, Failure> {
    let (scheme, host, port, _) = split_url(origin).ok_or(Failure::NotAllowed)?;
    if scheme != "https" {
        return Ok(Seen { trusted: loopback(&host), chain: Vec::new() });
    }
    let tcp = tcp_to(&host, port)?;
    set_tcp_deadline(&tcp, Instant::now() + Duration::from_secs(20))?;
    let connector = connector(None, false)?;
    let name = host.trim_start_matches('[').trim_end_matches(']');
    let tls = connector.connect(name, tcp).map_err(|_| Failure::Unreachable)?;
    let trusted = tls.ssl().verify_result() == X509VerifyResult::OK;
    let mut chain = Vec::new();
    for cert in tls.ssl().peer_cert_chain().into_iter().flatten().take(10) {
        let pem = String::from_utf8(cert.to_pem().unwrap_or_default()).unwrap_or_default();
        let self_signed = cert.public_key().is_ok_and(|key| cert.verify(&key).unwrap_or(false));
        chain.push((names(cert.subject_name()), names(cert.issuer_name()), fingerprint(cert), pem, self_signed));
    }
    Ok(Seen { trusted, chain })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn serve_once(answer: &'static [u8]) -> (String, std::thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut got = vec![0u8; 4096];
            let count = socket.read(&mut got).unwrap();
            socket.write_all(answer).unwrap();
            got.truncate(count);
            got
        });
        (address, thread)
    }

    fn session(address: &str) -> Session {
        Session::new(vec![origin_of(address).unwrap()], None, None).unwrap()
    }

    #[test]
    fn a_plain_answer_and_a_chunked_one() {
        let (address, thread) = serve_once(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        let (status, body) = session(&address).request("GET", &format!("{address}/api/sync"), &[("Accept".into(), "application/json".into())], b"").unwrap();
        assert_eq!((status, &body[..]), (200, &b"{}"[..]));
        let sent = String::from_utf8(thread.join().unwrap()).unwrap();
        assert!(sent.starts_with("GET /api/sync HTTP/1.1\r\nHost: 127.0.0.1:"));
        let (address, _) = serve_once(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n");
        assert_eq!(&session(&address).request("GET", &format!("{address}/x"), &[], b"").unwrap().1[..], b"abcde");
    }

    #[test]
    fn a_redirect_is_not_followed() {
        let (address, _) = serve_once(b"HTTP/1.1 302 Found\r\nLocation: https://evil.test/\r\nContent-Length: 0\r\n\r\n");
        assert_eq!(session(&address).request("GET", &format!("{address}/x"), &[], b"").unwrap().0, 302);
    }

    #[test]
    fn nothing_but_the_sessions_server() {
        let mut session = session("http://127.0.0.1:9");
        assert_eq!(session.request("GET", "http://127.0.0.2:9/x", &[], b"").err(), Some(Failure::NotAllowed));
        assert_eq!(session.request("GET", "https://127.0.0.1:9/x", &[], b"").err(), Some(Failure::NotAllowed));
        assert_eq!(session.request("GET", "http://user@127.0.0.1:9/x", &[], b"").err(), Some(Failure::NotAllowed));
        assert_eq!(session.request("DELETE", "http://127.0.0.1:9/api/ciphers/x", &[], b"").err(), Some(Failure::NotAllowed));
        assert_eq!(session.request("POST", "http://127.0.0.1:9/api/ciphers/x/delete", &[], b"").err(), Some(Failure::NotAllowed));
        assert_eq!(session.request("GET", "http://127.0.0.1:9/x", &[("X".into(), "a\r\nHost: evil".into())], b"").err(), Some(Failure::NotAllowed));
        // Plain http is for this machine only
        assert!(Session::new(vec!["http://vault.example.test:80".into()], None, None).is_ok());
        let mut far = Session::new(vec!["http://vault.example.test:80".into()], None, None).unwrap();
        assert_eq!(far.request("GET", "http://vault.example.test/x", &[], b"").err(), Some(Failure::NotAllowed));
        // A pin that is not the CA's, or one without the other
        assert!(Session::new(vec!["https://a.test:443".into()], None, Some("00")).is_err());
    }

    #[test]
    fn a_body_past_the_limit_is_refused_before_it_is_read() {
        let (address, _) = serve_once(b"HTTP/1.1 200 OK\r\nContent-Length: 999999999\r\n\r\n");
        assert_eq!(session(&address).request("GET", &format!("{address}/x"), &[], b"").err(), Some(Failure::TooLarge));
        let (address, _) = serve_once(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\nab");
        assert_eq!(session(&address).request("GET", &format!("{address}/x"), &[], b"").err(), Some(Failure::Unreachable));
    }
}
