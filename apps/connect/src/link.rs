//! A control connection: the handshake that turns a TCP connection into one (spec-protocol.md section 4), and
//! the connection itself, with a queue that writes one packet at a time.

use std::cell::Cell;
use std::net::IpAddr;
use std::rc::Rc;

use futures_channel::mpsc;
use futures_util::StreamExt;
use gio::prelude::*;

use crate::identity::{Identity, Local};
use crate::limits;
use crate::net::{self, Error, LineReader, Result, Trust};
use crate::packet::Packet;
use crate::text;
use crate::x509;

/// What a handshake needs from the program
pub struct Context {
    pub certificate: gio::TlsCertificate,
    pub local: Local,
    pub tcp_port: u16,
    /// The pin of a paired device, by id
    pub pin_of: Rc<dyn Fn(&str) -> Option<Rc<Vec<u8>>>>,
}

impl Context {
    fn trust(&self, id: &str) -> Trust {
        match (self.pin_of)(id) {
            Some(pin) => Trust::Pin(pin),
            None => Trust::FirstUse,
        }
    }
}

/// A connection that passed the handshake: TLS up, the peer's certificate in hand, its identity known
pub struct Opened {
    pub identity: Identity,
    /// Whether the identity was read inside the TLS session, which a pinned certificate vouches for. One taken from
    /// a datagram (protocol 7) or from the plain line before TLS is not.
    pub authenticated: bool,
    pub peer_der: Vec<u8>,
    pub peer: (IpAddr, u16, u32),
    pub caller: bool,
    tls: gio::TlsConnection,
    socket: gio::Socket,
    reader: LineReader,
}

/// A pin that did not match, with the id of the device that claimed it
fn name_the_claim(error: Error, claimed: Option<String>) -> Error {
    match (error, claimed) {
        (Error::PinMismatch, Some(id)) => Error::Mismatch(id),
        (error, _) => error,
    }
}

/// The second identity, inside TLS: both sides send first and then read, so that neither waits for the other
async fn exchange_identities(ctx: &Context, tls: &gio::TlsConnection, reader: &mut LineReader) -> Result<Identity> {
    net::write_all(&tls.output_stream(), ctx.local.packet_in_tls().to_line()).await?;
    let line = net::timed(limits::STEP_TIMEOUT, reader.next(limits::UNPAIRED_LINE)).await?.ok_or(Error::Protocol("closed before its identity"))?;
    let packet = Packet::parse(&line).map_err(|_| Error::Protocol("malformed identity"))?;
    let identity = Identity::from_packet(&packet).map_err(|_| Error::Protocol("bad identity"))?;
    // The one inside TLS names the device; the one before it only decided whether it is paired
    if identity.name.is_none() || identity.kind.is_none() {
        return Err(Error::Protocol("identity without a name or a type"));
    }
    Ok(identity)
}

fn same_device(first: &Identity, second: &Identity) -> bool {
    first.device_id == second.device_id && first.protocol == second.protocol
}

/// We opened the connection (we heard a datagram, or were told an address): our identity in plain text, then
/// TLS as the server. `expect` is the datagram's identity, when there was one.
pub async fn as_caller(ctx: &Context, connection: gio::SocketConnection, expect: Option<Identity>) -> Result<Opened> {
    let peer = net::peer_of(&connection)?;
    let target = expect.as_ref().map(|e| (e.device_id.as_str(), e.protocol));
    net::write_all(&connection.output_stream(), ctx.local.packet(ctx.tcp_port, true, target).to_line()).await?;

    let trust = expect.as_ref().map_or(Trust::FirstUse, |e| ctx.trust(&e.device_id));
    let base: gio::IOStream = connection.clone().upcast();
    let claimed = expect.as_ref().map(|e| e.device_id.clone());
    let secured = net::secure_as_server(&base, &ctx.certificate, trust).await.map_err(|e| name_the_claim(e, claimed))?;
    let mut reader = LineReader::new(secured.tls.input_stream());

    let mut authenticated = true;
    let identity = match expect {
        // A protocol 7 peer sends nothing inside TLS: the datagram is all there is to know of it
        Some(first) if first.protocol == Some(7) => {
            authenticated = false;
            Identity { name: first.name.clone().or_else(|| Some(text::tr("Unknown device"))), kind: first.kind.clone().or_else(|| Some("phone".into())), ..first }
        }
        first => {
            let second = exchange_identities(ctx, &secured.tls, &mut reader).await?;
            match first {
                Some(first) if !same_device(&first, &second) => return Err(Error::Protocol("the identity inside TLS is another device's")),
                // Called by address alone: only a protocol 8 peer sends this, so anything else is not one
                None if second.protocol != Some(8) => return Err(Error::Protocol("a peer called by address must speak protocol 8")),
                _ => {}
            }
            second
        }
    };
    finish(ctx, identity, authenticated, secured, reader, connection.socket(), peer, true)
}

/// A peer opened the connection to us: its identity in plain text, then TLS as the client
pub async fn as_acceptor(ctx: &Context, connection: gio::SocketConnection) -> Result<Opened> {
    let peer = net::peer_of(&connection)?;
    let mut plain = LineReader::new(connection.input_stream());
    let line = net::timed(limits::STEP_TIMEOUT, plain.next(limits::PLAIN_LINE)).await?.ok_or(Error::Protocol("closed before its identity"))?;
    let packet = Packet::parse(&line).map_err(|_| Error::Protocol("malformed identity"))?;
    let first = Identity::from_packet(&packet).map_err(|_| Error::Protocol("bad identity"))?;
    let version = first.protocol.filter(|v| limits::PROTOCOLS_ACCEPTED.contains(v)).ok_or(Error::Protocol("a protocol version we do not speak"))?;
    // A peer that names whom it called must have called us
    if first.target_id.as_deref().is_some_and(|id| id != ctx.local.device_id) || first.target_protocol.is_some_and(|v| v != limits::PROTOCOL_VERSION) {
        return Err(Error::Protocol("called another device"));
    }
    if plain.buffered() != 0 {
        return Err(Error::Protocol("spoke before TLS"));
    }

    let base: gio::IOStream = connection.clone().upcast();
    let secured = net::secure_as_client(&base, &ctx.certificate, ctx.trust(&first.device_id)).await.map_err(|e| name_the_claim(e, Some(first.device_id.clone())))?;
    let mut reader = LineReader::new(secured.tls.input_stream());
    let authenticated = version >= 8;
    let identity = if version >= 8 {
        let second = exchange_identities(ctx, &secured.tls, &mut reader).await?;
        if !same_device(&first, &second) {
            return Err(Error::Protocol("the identity inside TLS is another device's"));
        }
        second
    } else {
        Identity { name: first.name.clone().or_else(|| Some(text::tr("Unknown device"))), kind: first.kind.clone().or_else(|| Some("phone".into())), ..first }
    };
    finish(ctx, identity, authenticated, secured, reader, connection.socket(), peer, false)
}

#[allow(clippy::too_many_arguments)]
fn finish(ctx: &Context, identity: Identity, authenticated: bool, secured: net::Secured, reader: LineReader, socket: gio::Socket, peer: (IpAddr, u16, u32), caller: bool) -> Result<Opened> {
    // Whoever the peer turned out to be: a paired device presents its pin and nothing else. The first check
    // was against the id from the datagram or the plain identity, this one against the final one.
    if let Trust::Pin(pin) = ctx.trust(&identity.device_id) {
        if *pin != secured.peer_der {
            return Err(Error::Mismatch(identity.device_id));
        }
    }
    // The certificate's name is the device's id. The pin, not the name, is what authenticates; a difference
    // is only worth a line in the log
    if let Some(name) = x509::parts(&secured.peer_der).and_then(|p| p.common_name) {
        if name != identity.device_id {
            eprintln!("amethystora-connect: a certificate that names another device than the one it came from");
        }
    }
    Ok(Opened { identity, authenticated, peer_der: secured.peer_der, peer, caller, tls: secured.tls, socket, reader })
}

impl Opened {
    /// Ends a connection that was not wanted
    pub fn close(&self) {
        let _ = self.socket.close();
    }
}

// --- The connection ---------------------------------------------------------------------------------------

pub struct Link {
    pub id: u64,
    pub peer: (IpAddr, u16, u32),
    pub peer_der: Vec<u8>,
    pub peer_spki: Vec<u8>,
    pub caller: bool,
    /// When the connection came up: the oldest unpaired one is the first to go when the slots run out
    pub since: std::time::Instant,
    /// Whether the first packet of an unpaired device has been answered already (spec-protocol.md 5.9)
    pub told_unpaired: Cell<bool>,
    out: mpsc::UnboundedSender<Vec<u8>>,
    socket: gio::Socket,
    tls: gio::TlsConnection,
    closed: Cell<bool>,
}

/// What reads the connection: lines, until it ends
pub struct Reader {
    reader: LineReader,
}

impl Reader {
    pub async fn next(&mut self, max: usize) -> Result<Option<Vec<u8>>> {
        self.reader.next(max).await
    }
}

impl Link {
    /// Wraps a connection that passed the handshake, and starts its writer. The reader is the caller's to run.
    pub fn start(id: u64, opened: Opened) -> (Rc<Link>, Reader) {
        let (out, mut queue) = mpsc::unbounded::<Vec<u8>>();
        let spki = x509::parts(&opened.peer_der).map(|p| p.spki).unwrap_or_default();
        let output = opened.tls.output_stream();
        let socket = opened.socket.clone();
        // One packet at a time, in the order they were queued; a write that fails ends the connection
        glib::spawn_future_local(async move {
            while let Some(line) = queue.next().await {
                if net::write_all(&output, line).await.is_err() {
                    let _ = socket.close();
                    break;
                }
            }
        });
        let link = Rc::new(Link {
            id,
            peer: opened.peer,
            peer_der: opened.peer_der,
            peer_spki: spki,
            caller: opened.caller,
            since: std::time::Instant::now(),
            told_unpaired: Cell::new(false),
            out,
            socket: opened.socket,
            tls: opened.tls,
            closed: Cell::new(false),
        });
        (link, Reader { reader: opened.reader })
    }

    /// Queues a packet. False when the connection is already closed.
    pub fn send(&self, packet: &Packet) -> bool {
        !self.closed.get() && self.out.unbounded_send(packet.to_line()).is_ok()
    }

    pub fn is_closed(&self) -> bool {
        self.closed.get()
    }

    /// Ends the connection at once: what waits on it fails
    pub fn close(&self) {
        if !self.closed.replace(true) {
            let _ = self.socket.close();
        }
    }

    /// Ends it with a TLS close, for the program stopping
    pub async fn close_politely(&self) {
        if !self.closed.replace(true) {
            net::close(&self.tls).await;
            let _ = self.socket.close();
        }
    }

    /// The writer ends when the last sender is dropped, so a link nobody holds any more cleans up after itself
    pub fn ip(&self) -> IpAddr {
        self.peer.0
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        let _ = self.socket.close();
    }
}
