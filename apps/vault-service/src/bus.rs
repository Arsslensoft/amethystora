//! Just enough of D-Bus to ask polkit and to hear logind, over the system bus's socket: the service otherwise links neither
//! GLib nor a bus crate, and its memory budget is 8 MB. Little-endian only, and every length is checked before it is used: a
//! message that does not parse ends the connection, which closes every vault (`desktop.rs`).

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::mem;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

const MOST_MESSAGE: usize = 1024 * 1024;

pub const METHOD_CALL: u8 = 1;
pub const METHOD_RETURN: u8 = 2;
pub const ERROR: u8 = 3;
pub const SIGNAL: u8 = 4;

/// Writes a message body by D-Bus's alignment rules
#[derive(Default)]
pub struct Writer {
    pub bytes: Vec<u8>,
}

impl Writer {
    pub fn align(&mut self, to: usize) {
        while !self.bytes.len().is_multiple_of(to) {
            self.bytes.push(0);
        }
    }

    pub fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn u32(&mut self, value: u32) {
        self.align(4);
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn i32(&mut self, value: i32) {
        self.align(4);
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn u64(&mut self, value: u64) {
        self.align(8);
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn bool(&mut self, value: bool) {
        self.u32(u32::from(value));
    }

    /// A string or an object path
    pub fn string(&mut self, value: &str) {
        self.u32(value.len() as u32);
        self.bytes.extend_from_slice(value.as_bytes());
        self.bytes.push(0);
    }

    pub fn signature(&mut self, value: &str) {
        self.bytes.push(value.len() as u8);
        self.bytes.extend_from_slice(value.as_bytes());
        self.bytes.push(0);
    }

    /// An array of elements aligned to `element`: its length does not count the padding before the first
    pub fn array(&mut self, element: usize, fill: impl FnOnce(&mut Writer)) {
        self.align(4);
        let at = self.bytes.len();
        self.bytes.extend_from_slice(&[0; 4]);
        self.align(element);
        let start = self.bytes.len();
        fill(self);
        let length = (self.bytes.len() - start) as u32;
        self.bytes[at..at + 4].copy_from_slice(&length.to_le_bytes());
    }

    /// A string to string dictionary, `a{ss}`
    pub fn string_map(&mut self, entries: &[(&str, &str)]) {
        self.array(8, |writer| {
            for (key, value) in entries {
                writer.align(8);
                writer.string(key);
                writer.string(value);
            }
        });
    }
}

/// Reads a message body, every read bounded
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    /// A big-endian message's header: read for its length and its descriptors, and then passed over
    big: bool,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes, at: 0, big: false }
    }

    fn align(&mut self, to: usize) -> Option<()> {
        let at = self.at.checked_next_multiple_of(to)?;
        if at > self.bytes.len() {
            return None;
        }
        self.at = at;
        Some(())
    }

    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(count)?;
        let taken = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(taken)
    }

    pub fn byte(&mut self) -> Option<u8> {
        self.take(1).map(|taken| taken[0])
    }

    pub fn u32(&mut self) -> Option<u32> {
        self.align(4)?;
        let bytes: [u8; 4] = self.take(4)?.try_into().ok()?;
        Some(if self.big { u32::from_be_bytes(bytes) } else { u32::from_le_bytes(bytes) })
    }

    pub fn bool(&mut self) -> Option<bool> {
        Some(self.u32()? != 0)
    }

    pub fn string(&mut self) -> Option<&'a str> {
        let length = self.u32()? as usize;
        let text = std::str::from_utf8(self.take(length)?).ok()?;
        (self.byte()? == 0).then_some(text)
    }

    pub fn signature(&mut self) -> Option<&'a str> {
        let length = self.byte()? as usize;
        let text = std::str::from_utf8(self.take(length)?).ok()?;
        (self.byte()? == 0).then_some(text)
    }

    pub fn struct_start(&mut self) -> Option<()> {
        self.align(8)
    }

    /// An array's end, with its elements' alignment passed over
    pub fn array_end(&mut self, element: usize) -> Option<usize> {
        let length = self.u32()? as usize;
        self.align(element)?;
        let end = self.at.checked_add(length)?;
        (end <= self.bytes.len()).then_some(end)
    }

    pub fn at(&self) -> usize {
        self.at
    }

    /// Passes over one value of the type at the start of `signature`, and says what is left of the signature
    pub fn skip<'s>(&mut self, signature: &'s str, depth: u32) -> Option<&'s str> {
        if depth > 32 {
            return None;
        }
        let code = *signature.as_bytes().first()?;
        let rest = signature.get(1..)?;
        match code {
            b'y' => {
                self.take(1)?;
                Some(rest)
            }
            b'n' | b'q' => {
                self.align(2)?;
                self.take(2)?;
                Some(rest)
            }
            b'b' | b'i' | b'u' | b'h' => {
                self.align(4)?;
                self.take(4)?;
                Some(rest)
            }
            b'x' | b't' | b'd' => {
                self.align(8)?;
                self.take(8)?;
                Some(rest)
            }
            b's' | b'o' => {
                self.string()?;
                Some(rest)
            }
            b'g' => {
                self.signature()?;
                Some(rest)
            }
            b'v' => {
                let inner = self.signature()?;
                let left = self.skip(inner, depth + 1)?;
                left.is_empty().then_some(rest)
            }
            b'a' => {
                let after = single_type(rest)?;
                let element = &rest[..rest.len() - after.len()];
                let end = self.array_end(alignment(element)?)?;
                while self.at < end {
                    let before = self.at;
                    self.skip(element, depth + 1)?;
                    // An element of nothing (an empty struct, which D-Bus does not allow) would go round for ever
                    if self.at == before {
                        return None;
                    }
                }
                (self.at == end).then_some(after)
            }
            b'(' | b'{' => {
                self.align(8)?;
                let close = if code == b'(' { b')' } else { b'}' };
                let mut inner = rest;
                if *inner.as_bytes().first()? == close {
                    return None;
                }
                while *inner.as_bytes().first()? != close {
                    inner = self.skip(inner, depth + 1)?;
                }
                inner.get(1..)
            }
            _ => None,
        }
    }
}

fn alignment(signature: &str) -> Option<usize> {
    Some(match *signature.as_bytes().first()? {
        b'y' | b'g' | b'v' => 1,
        b'n' | b'q' => 2,
        b'b' | b'i' | b'u' | b'h' | b's' | b'o' | b'a' => 4,
        b'x' | b't' | b'd' | b'(' | b'{' => 8,
        _ => return None,
    })
}

/// What follows the first complete type in a signature
fn single_type(signature: &str) -> Option<&str> {
    let mut depth = 0i32;
    for (at, code) in signature.bytes().enumerate() {
        match code {
            b'a' => continue,
            b'(' | b'{' => depth += 1,
            b')' | b'}' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return None;
        }
        if depth == 0 {
            return signature.get(at + 1..);
        }
    }
    None
}

#[derive(Debug, Default)]
pub struct Message {
    pub kind: u8,
    pub serial: u32,
    pub reply_to: Option<u32>,
    pub path: String,
    pub interface: String,
    pub member: String,
    pub error: String,
    pub sender: String,
    pub destination: String,
    pub signature: String,
    pub body: Vec<u8>,
    pub fds: Vec<OwnedFd>,
}

/// A message's kind when it is passed over: a header that does not read, or a big-endian message, neither of which logind or
/// polkit sends here. Its length is known, so the next message is found all the same.
pub const IGNORED: u8 = 0;

/// A message out of what has arrived: Ok(None) if it is not all here, an error only if the stream cannot be followed any more.
/// Its descriptors' count is None when its header could not be read.
fn parse(input: &[u8]) -> Result<Option<(Message, usize, Option<u32>)>, ()> {
    if input.len() < 16 {
        return Ok(None);
    }
    let big = match input[0] {
        b'l' => false,
        b'B' => true,
        _ => return Err(()),
    };
    if input[3] != 1 {
        return Err(());
    }
    let number = |at: usize| {
        let bytes = [input[at], input[at + 1], input[at + 2], input[at + 3]];
        (if big { u32::from_be_bytes(bytes) } else { u32::from_le_bytes(bytes) }) as usize
    };
    let (body_length, fields_length) = (number(4), number(12));
    let fields_end = 16usize.checked_add(fields_length).ok_or(())?;
    let total = fields_end.checked_next_multiple_of(8).and_then(|end| end.checked_add(body_length)).ok_or(())?;
    if total > MOST_MESSAGE {
        return Err(());
    }
    if input.len() < total {
        return Ok(None);
    }
    // Alignment within the header counts from the message's start
    let mut fields = Reader { bytes: &input[..fields_end], at: 16, big };
    let Some((mut message, fds)) = header(&mut fields, fields_end) else {
        return Ok(Some((Message { kind: IGNORED, ..Message::default() }, total, None)));
    };
    message.serial = number(8) as u32;
    message.kind = if big { IGNORED } else { input[1] };
    message.body = input[total - body_length..total].to_vec();
    Ok(Some((message, total, Some(fds))))
}

fn header(fields: &mut Reader, end: usize) -> Option<(Message, u32)> {
    let mut message = Message::default();
    let mut fds = 0u32;
    while fields.at() < end {
        fields.struct_start()?;
        let code = fields.byte()?;
        let signature = fields.signature()?;
        match (code, signature) {
            (1, "o") => message.path = fields.string()?.to_string(),
            (2, "s") => message.interface = fields.string()?.to_string(),
            (3, "s") => message.member = fields.string()?.to_string(),
            (4, "s") => message.error = fields.string()?.to_string(),
            (5, "u") => message.reply_to = Some(fields.u32()?),
            (6, "s") => message.destination = fields.string()?.to_string(),
            (7, "s") => message.sender = fields.string()?.to_string(),
            (8, "g") => message.signature = fields.signature()?.to_string(),
            (9, "u") => fds = fields.u32()?,
            _ => {
                if !fields.skip(signature, 0)?.is_empty() {
                    return None;
                }
            }
        }
    }
    Some((message, fds))
}

/// Bytes on the bus that can never be a message
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotAMessage;

/// One connection to a bus
pub struct Bus {
    stream: UnixStream,
    input: Vec<u8>,
    output: Vec<u8>,
    fds: VecDeque<OwnedFd>,
    /// Descriptors going out, each with how many bytes of `output` come before the message it goes with
    sending: VecDeque<(usize, OwnedFd)>,
    serial: u32,
    /// From Hello's answer
    name: Option<String>,
    hello: u32,
}

/// Where the system bus is: what systemd's and GLib's own clients read, for a test's private bus
pub fn system_bus() -> std::path::PathBuf {
    std::env::var("DBUS_SYSTEM_BUS_ADDRESS")
        .ok()
        .and_then(|address| address.strip_prefix("unix:path=").map(|path| path.split(',').next().unwrap_or(path).to_string()))
        .unwrap_or_else(|| "/run/dbus/system_bus_socket".to_string())
        .into()
}

impl Bus {
    /// Connects and says who this is: the only step that blocks, for a second at most, on a local socket
    pub fn connect(path: &Path) -> io::Result<Bus> {
        let mut stream = UnixStream::connect(path)?;
        let patience = Some(Duration::from_secs(1));
        stream.set_read_timeout(patience)?;
        stream.set_write_timeout(patience)?;
        // SAFETY: geteuid cannot fail
        let uid: String = unsafe { libc::geteuid() }.to_string().bytes().map(|byte| format!("{byte:02x}")).collect();
        stream.write_all(format!("\0AUTH EXTERNAL {uid}\r\n").as_bytes())?;
        let said = read_auth_line(&mut stream)?;
        if !said.starts_with("OK ") {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        stream.write_all(b"NEGOTIATE_UNIX_FD\r\n")?;
        let said = read_auth_line(&mut stream)?;
        if !said.starts_with("AGREE_UNIX_FD") {
            return Err(io::ErrorKind::Unsupported.into());
        }
        stream.write_all(b"BEGIN\r\n")?;
        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
        stream.set_nonblocking(true)?;
        let mut bus = Bus { stream, input: Vec::new(), output: Vec::new(), fds: VecDeque::new(), sending: VecDeque::new(), serial: 0, name: None, hello: 0 };
        let hello = bus.call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "Hello", "", &[]);
        bus.flush()?;
        bus.hello = hello;
        Ok(bus)
    }

    pub fn fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    pub fn wants_write(&self) -> bool {
        !self.output.is_empty()
    }

    fn send(&mut self, kind: u8, fields: &[(u8, &str, &str)], signature: &str, body: &[u8]) -> u32 {
        self.serial = self.serial.wrapping_add(1).max(1);
        let mut header = Writer::default();
        header.byte(b'l');
        header.byte(kind);
        header.byte(0);
        header.byte(1);
        header.u32(body.len() as u32);
        header.u32(self.serial);
        header.array(8, |writer| {
            for (code, kind_of, value) in fields {
                writer.align(8);
                writer.byte(*code);
                writer.signature(kind_of);
                match *kind_of {
                    "g" => writer.signature(value),
                    "u" => writer.u32(value.parse().unwrap_or(0)),
                    _ => writer.string(value),
                }
            }
            if !signature.is_empty() {
                writer.align(8);
                writer.byte(8);
                writer.signature("g");
                writer.signature(signature);
            }
        });
        header.align(8);
        self.output.extend_from_slice(&header.bytes);
        self.output.extend_from_slice(body);
        self.serial
    }

    pub fn call(&mut self, destination: &str, path: &str, interface: &str, member: &str, signature: &str, body: &[u8]) -> u32 {
        self.send(METHOD_CALL, &[(1, "o", path), (2, "s", interface), (3, "s", member), (6, "s", destination)], signature, body)
    }

    /// A call carrying one descriptor at most, the body's `h` number 0
    #[allow(clippy::too_many_arguments)]
    pub fn call_with_fd(&mut self, destination: &str, path: &str, interface: &str, member: &str, signature: &str, body: &[u8], fd: Option<OwnedFd>) -> u32 {
        let Some(fd) = fd else { return self.call(destination, path, interface, member, signature, body) };
        self.sending.push_back((self.output.len(), fd));
        let fields = [(1, "o", path), (2, "s", interface), (3, "s", member), (6, "s", destination), (9, "u", "1")];
        self.send(METHOD_CALL, &fields, signature, body)
    }

    /// For tests' stand-ins for logind
    pub fn signal(&mut self, path: &str, interface: &str, member: &str, signature: &str, body: &[u8]) -> u32 {
        self.send(SIGNAL, &[(1, "o", path), (2, "s", interface), (3, "s", member)], signature, body)
    }

    /// A signal to one connection, as anyone on the bus can send: for tests of what the service makes of a stranger's
    pub fn signal_to(&mut self, destination: &str, path: &str, interface: &str, member: &str, signature: &str, body: &[u8]) -> u32 {
        self.send(SIGNAL, &[(1, "o", path), (2, "s", interface), (3, "s", member), (6, "s", destination)], signature, body)
    }

    /// Bytes as they are, for tests that send what no writer here would
    pub fn send_raw(&mut self, bytes: &[u8]) {
        self.output.extend_from_slice(bytes);
    }

    /// The unique name the bus gave this connection, once Hello is answered
    pub fn unique_name(&mut self) -> Option<String> {
        if self.name.is_none() {
            let _ = self.flush();
            let _ = self.read();
        }
        self.name.clone()
    }

    /// For tests' stand-ins: an answer to a call, with a descriptor if it has one
    pub fn reply(&mut self, to: &Message, signature: &str, body: &[u8], fd: Option<OwnedFd>) -> u32 {
        let serial = to.serial.to_string();
        let count = fd.is_some();
        if let Some(fd) = fd {
            self.sending.push_back((self.output.len(), fd));
        }
        let mut fields = vec![(6, "s", to.sender.as_str()), (5, "u", serial.as_str())];
        if count {
            fields.push((9, "u", "1"));
        }
        self.send(METHOD_RETURN, &fields, signature, body)
    }

    /// Writes what it can without waiting
    pub fn flush(&mut self) -> io::Result<()> {
        while !self.output.is_empty() {
            let written = match self.sending.front() {
                Some((0, fd)) => send_with_fd(&self.stream, &self.output, fd),
                Some((before, _)) => self.stream.write(&self.output[..*before]),
                None => self.stream.write(&self.output),
            };
            match written {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(count) => {
                    self.output.drain(..count);
                    if self.sending.front().is_some_and(|(before, _)| *before == 0) {
                        self.sending.pop_front();
                    }
                    for (before, _) in &mut self.sending {
                        *before = before.saturating_sub(count);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// Reads what is there, keeping any descriptors sent with it. An error once the bus is gone.
    pub fn read(&mut self) -> io::Result<()> {
        loop {
            let mut buffer = [0u8; 16 * 1024];
            let mut part = libc::iovec { iov_base: buffer.as_mut_ptr().cast(), iov_len: buffer.len() };
            // Room for sixteen descriptors, aligned as cmsghdr wants
            let mut control = [0u64; 16];
            // SAFETY: an all-zero msghdr is valid, and the pointers set below stay alive through the call
            let mut header: libc::msghdr = unsafe { mem::zeroed() };
            header.msg_iov = &raw mut part;
            header.msg_iovlen = 1;
            header.msg_control = control.as_mut_ptr().cast();
            header.msg_controllen = mem::size_of_val(&control) as _;
            // SAFETY: as above
            let count = unsafe { libc::recvmsg(self.stream.as_raw_fd(), &raw mut header, libc::MSG_CMSG_CLOEXEC | libc::MSG_DONTWAIT) };
            if count < 0 {
                let error = io::Error::last_os_error();
                return if error.kind() == io::ErrorKind::WouldBlock { Ok(()) } else { Err(error) };
            }
            // SAFETY: walks the control messages the kernel just wrote, within the length it gave
            unsafe {
                let mut entry = libc::CMSG_FIRSTHDR(&raw const header);
                while !entry.is_null() {
                    if (*entry).cmsg_level == libc::SOL_SOCKET && (*entry).cmsg_type == libc::SCM_RIGHTS {
                        let room = ((*entry).cmsg_len as usize - libc::CMSG_LEN(0) as usize) / mem::size_of::<RawFd>();
                        let first = libc::CMSG_DATA(entry).cast::<RawFd>();
                        for at in 0..room {
                            self.fds.push_back(OwnedFd::from_raw_fd(first.add(at).read_unaligned()));
                        }
                    }
                    entry = libc::CMSG_NXTHDR(&raw const header, entry);
                }
            }
            if count == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            self.input.extend_from_slice(&buffer[..count as usize]);
        }
    }

    /// The next whole message that has arrived. An error for bytes that are not one.
    pub fn next_message(&mut self) -> Result<Option<Message>, NotAMessage> {
        let Some((mut message, length, fds)) = parse(&self.input).map_err(|()| NotAMessage)? else {
            return if self.input.len() > MOST_MESSAGE { Err(NotAMessage) } else { Ok(None) };
        };
        self.input.drain(..length);
        match fds {
            // Descriptors no message here asked for may come with it: missing ones are only missing
            Some(count) => {
                for _ in 0..count {
                    if let Some(fd) = self.fds.pop_front() {
                        message.fds.push(fd);
                    }
                }
            }
            // A header that does not read says nothing of what came with it: none of what is held can be matched any more
            None => self.fds.clear(),
        }
        if self.hello != 0 && message.reply_to == Some(self.hello) && message.kind == METHOD_RETURN {
            self.name = Reader::new(&message.body).string().map(str::to_string);
        }
        Ok(Some(message))
    }
}

/// The first byte of what is sent carries the descriptor
fn send_with_fd(stream: &UnixStream, bytes: &[u8], fd: &OwnedFd) -> io::Result<usize> {
    let mut part = libc::iovec { iov_base: bytes.as_ptr().cast_mut().cast(), iov_len: bytes.len() };
    let mut control = [0u64; 4];
    // SAFETY: an all-zero msghdr is valid, the pointers set here stay alive through the call, and the control buffer has room
    // for one descriptor's message
    let count = unsafe {
        let mut header: libc::msghdr = mem::zeroed();
        header.msg_iov = &raw mut part;
        header.msg_iovlen = 1;
        header.msg_control = control.as_mut_ptr().cast();
        header.msg_controllen = libc::CMSG_SPACE(mem::size_of::<RawFd>() as u32) as _;
        let entry = libc::CMSG_FIRSTHDR(&raw const header);
        (*entry).cmsg_level = libc::SOL_SOCKET;
        (*entry).cmsg_type = libc::SCM_RIGHTS;
        (*entry).cmsg_len = libc::CMSG_LEN(mem::size_of::<RawFd>() as u32) as _;
        libc::CMSG_DATA(entry).cast::<RawFd>().write_unaligned(fd.as_raw_fd());
        libc::sendmsg(stream.as_raw_fd(), &raw const header, libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL)
    };
    if count < 0 { Err(io::Error::last_os_error()) } else { Ok(count as usize) }
}

fn read_auth_line(stream: &mut UnixStream) -> io::Result<String> {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while line.len() < 512 {
        if stream.read(&mut byte)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        if byte[0] == b'\n' {
            return Ok(String::from_utf8_lossy(&line).trim_end().to_string());
        }
        line.push(byte[0]);
    }
    Err(io::ErrorKind::InvalidData.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message_of(kind: u8, signature: &str, body: &[u8]) -> Vec<u8> {
        let (ours, theirs) = UnixStream::pair().unwrap();
        ours.set_nonblocking(true).unwrap();
        let mut bus = Bus { stream: ours, input: Vec::new(), output: Vec::new(), fds: VecDeque::new(), sending: VecDeque::new(), serial: 41, name: None, hello: 0 };
        bus.send(kind, &[(1, "o", "/a/b"), (3, "s", "Member")], signature, body);
        drop(theirs);
        bus.output.clone()
    }

    #[test]
    fn what_it_writes_it_reads_back() {
        let mut body = Writer::default();
        body.string("hello");
        body.bool(true);
        let bytes = message_of(METHOD_CALL, "sb", &body.bytes);
        let (message, length, fds) = parse(&bytes).unwrap().unwrap();
        assert_eq!(length, bytes.len());
        assert_eq!(fds, Some(0));
        assert_eq!(message.kind, METHOD_CALL);
        assert_eq!(message.serial, 42);
        assert_eq!(message.path, "/a/b");
        assert_eq!(message.member, "Member");
        assert_eq!(message.signature, "sb");
        let mut reader = Reader::new(&message.body);
        assert_eq!(reader.string(), Some("hello"));
        assert_eq!(reader.bool(), Some(true));
    }

    #[test]
    fn a_short_message_waits_and_a_false_one_ends_it() {
        let bytes = message_of(SIGNAL, "", &[]);
        for length in 0..bytes.len() {
            assert!(matches!(parse(&bytes[..length]), Ok(None)), "{length}");
        }
        let mut not_a_bus = bytes.clone();
        not_a_bus[0] = b'x';
        assert!(parse(&not_a_bus).is_err());
        let mut huge = bytes.clone();
        huge[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&huge).is_err());
        let mut fields = bytes.clone();
        fields[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&fields).is_err());
        let mut version = bytes.clone();
        version[3] = 2;
        assert!(parse(&version).is_err());
    }

    #[test]
    fn a_big_endian_message_is_passed_over_and_the_next_one_read() {
        // As a stranger on the bus may send one: logind and polkit never do, and the stream goes on after it
        let (ours, theirs) = UnixStream::pair().unwrap();
        ours.set_nonblocking(true).unwrap();
        let mut bus = Bus { stream: ours, input: Vec::new(), output: Vec::new(), fds: VecDeque::new(), sending: VecDeque::new(), serial: 1, name: None, hello: 0 };
        let mut big = Vec::new();
        big.extend_from_slice(b"B\x04\x00\x01");
        big.extend_from_slice(&0u32.to_be_bytes());
        big.extend_from_slice(&7u32.to_be_bytes());
        big.extend_from_slice(&0u32.to_be_bytes());
        bus.input.extend_from_slice(&big);
        bus.input.extend_from_slice(&message_of(SIGNAL, "", &[]));
        let first = bus.next_message().unwrap().unwrap();
        assert_eq!(first.kind, IGNORED);
        let second = bus.next_message().unwrap().unwrap();
        assert_eq!(second.kind, SIGNAL);
        assert_eq!(second.member, "Member");
        drop(theirs);
    }

    #[test]
    fn a_header_that_does_not_read_is_passed_over() {
        let mut bytes = message_of(SIGNAL, "", &[]);
        // The first field's code says a path follows, and its signature says something else entirely
        bytes[17] = 1;
        bytes[18] = b'z';
        let (message, length, fds) = parse(&bytes).unwrap().unwrap();
        assert_eq!((message.kind, length, fds), (IGNORED, bytes.len(), None));
    }

    #[test]
    fn an_array_of_nothing_ends_rather_than_going_round() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&[0; 12]);
        assert_eq!(Reader::new(&bytes).skip("a()", 0), None);
        assert_eq!(Reader::new(&bytes).skip("a{}", 0), None);
        assert_eq!(Reader::new(&bytes).skip("()", 0), None);
    }

    #[test]
    fn nonsense_never_panics() {
        // The fixed header of a valid message followed by every mangling of its fields
        let bytes = message_of(SIGNAL, "s", &[]);
        for at in 16..bytes.len() {
            for value in [0u8, 1, 0x7f, 0xff] {
                let mut changed = bytes.clone();
                changed[at] = value;
                let _ = parse(&changed);
            }
        }
    }

    #[test]
    fn it_passes_over_what_it_does_not_want() {
        // PropertiesChanged's own body: s a{sv} as
        let mut body = Writer::default();
        body.string("org.freedesktop.login1.Session");
        body.array(8, |writer| {
            writer.align(8);
            writer.string("IdleHint");
            writer.signature("b");
            writer.bool(false);
            writer.align(8);
            writer.string("Display");
            writer.signature("(so)");
            writer.align(8);
            writer.string("x");
            writer.string("/o");
        });
        body.array(4, |writer| writer.string("Other"));
        let mut reader = Reader::new(&body.bytes);
        assert_eq!(reader.skip("sa{sv}as", 0).and_then(|left| reader.skip(left, 0)).and_then(|left| reader.skip(left, 0)), Some(""));
        assert_eq!(reader.at(), body.bytes.len());
    }

    #[test]
    fn a_bad_signature_is_refused() {
        let mut reader = Reader::new(&[0u8; 64]);
        assert_eq!(reader.skip("", 0), None);
        assert_eq!(reader.skip("z", 0), None);
        assert_eq!(reader.skip("a", 0), None);
        assert_eq!(reader.skip("(i", 0), None);
        let deep = format!("{}i{}", "(".repeat(40), ")".repeat(40));
        assert_eq!(reader.skip(&deep, 0), None);
    }
}
