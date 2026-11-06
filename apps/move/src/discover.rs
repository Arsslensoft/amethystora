//! Finding the new computer: the old one asks on the network (a datagram to everyone) and the new one answers the asker
//! alone, with its name and the port it listens on. Only an address on a network named home is asked or answered
//! (`zone::Home`). The answer is nothing to trust: it is a place to call, and the key and the six digits decide the rest.

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::text::clean;
use crate::zone::Home;

const ASK: &[u8] = b"amethystora-move?";
const ANSWER: &str = "amethystora-move! ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub name: String,
    pub addr: SocketAddr,
}

/// Answers the old computer's question for as long as it lives; stops when dropped
pub struct Answering {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Answering {
    /// `udp` is bound by the caller (the port the move uses); `tcp_port` is what the answer says
    pub fn start(udp: UdpSocket, tcp_port: u16, name: String, home: Arc<dyn Home + Send + Sync>) -> io::Result<Answering> {
        udp.set_read_timeout(Some(Duration::from_millis(300)))?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut buffer = [0u8; 64];
            let answer = format!("{ANSWER}{tcp_port} {name}");
            while !flag.load(Ordering::Relaxed) {
                let Ok((n, from)) = udp.recv_from(&mut buffer) else { continue };
                if &buffer[..n] == ASK && home.is_home(from.ip()) {
                    let _ = udp.send_to(answer.as_bytes(), from);
                }
            }
        });
        Ok(Answering { stop, thread: Some(thread) })
    }
}

impl Drop for Answering {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// What an answer says, or nothing for anything else
pub fn parse_answer(text: &[u8], from: IpAddr) -> Option<Found> {
    let text = std::str::from_utf8(text).ok()?.strip_prefix(ANSWER)?;
    let (port, name) = text.split_once(' ')?;
    let port: u16 = port.parse().ok().filter(|p| *p >= 1024)?;
    Some(Found { name: clean(name), addr: SocketAddr::new(from, port) })
}

/// A socket bound to `port` that others may share, with broadcast on. The answer to a broadcast comes from the other computer's
/// address, which no firewall that tracks connections takes for the reply to a question sent to everyone: it is a new packet, and
/// it is let in only where the port it goes to is open. So the question is asked from the move's own port, which the home zone
/// opens (and not from a port of the system's choosing, which no zone does).
fn socket_on(port: u16) -> io::Result<UdpSocket> {
    use std::os::fd::FromRawFd;
    // SAFETY: plain socket calls; the descriptor is closed by `UdpSocket`'s drop once it is handed over, and on every error here
    unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let on: libc::c_int = 1;
        let size = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
        let mut address: libc::sockaddr_in = std::mem::zeroed();
        address.sin_family = libc::AF_INET as libc::sa_family_t;
        address.sin_port = port.to_be();
        let ok = libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_REUSEADDR, (&raw const on).cast(), size) == 0
            && libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_BROADCAST, (&raw const on).cast(), size) == 0
            && libc::bind(fd, (&raw const address).cast(), std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t) == 0;
        if !ok {
            let error = io::Error::last_os_error();
            libc::close(fd);
            return Err(error);
        }
        Ok(UdpSocket::from_raw_fd(fd))
    }
}

/// Asks on the network and listens for `wait`; `to` is the address to ask (the broadcast address, or one in a test). `from` is the
/// port to ask from: the move's own, or 0 where nothing is between (a test, one machine); when it cannot be had, any port.
pub fn find(to: SocketAddr, wait: Duration, home: &dyn Home, from: u16) -> io::Result<Vec<Found>> {
    let socket = match socket_on(from) {
        Ok(socket) => socket,
        Err(_) => {
            let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
            socket.set_broadcast(true)?;
            socket
        }
    };
    socket.send_to(ASK, to)?;
    let until = Instant::now() + wait;
    let mut found: Vec<Found> = Vec::new();
    let mut buffer = [0u8; 256];
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        socket.set_read_timeout(Some(left))?;
        let Ok((n, from)) = socket.recv_from(&mut buffer) else { break };
        if !home.is_home(from.ip()) {
            continue;
        }
        if let Some(one) = parse_answer(&buffer[..n], from.ip()) {
            if !found.iter().any(|f| f.addr == one.addr) {
                found.push(one);
            }
        }
        if found.len() >= 16 {
            break;
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone::Anywhere;

    #[test]
    fn an_answer_names_a_place_to_call() {
        let from: IpAddr = "192.168.1.9".parse().unwrap();
        assert_eq!(parse_answer(b"amethystora-move! 4793 Desk\x1b[31m", from), Some(Found { name: "Desk[31m".into(), addr: "192.168.1.9:4793".parse().unwrap() }));
        assert_eq!(parse_answer(b"amethystora-move! 80 Desk", from), None, "not a port a program of a person listens on");
        assert_eq!(parse_answer(b"amethystora-move! x Desk", from), None);
        assert_eq!(parse_answer(b"hello", from), None);
        assert_eq!(parse_answer(&[0xff, 0xfe], from), None);
    }

    #[test]
    fn the_old_computer_finds_the_new_one() {
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let to = udp.local_addr().unwrap();
        let answering = Answering::start(udp, 4793, "New desk".into(), Arc::new(Anywhere)).unwrap();
        let found = find(to, Duration::from_millis(600), &Anywhere, 0).unwrap();
        assert_eq!(found, vec![Found { name: "New desk".into(), addr: SocketAddr::new("127.0.0.1".parse().unwrap(), 4793) }]);
        drop(answering);
    }

    #[test]
    fn the_question_is_asked_from_a_port_that_can_be_shared_and_the_answer_goes_back_to_it() {
        // The asker holds a port of its own, as the move's port is held on the old computer, and the answerer replies to where it was asked
        let asked_from = socket_on(0).unwrap();
        let port = asked_from.local_addr().unwrap().port();
        assert!(socket_on(port).is_ok(), "the same port can be asked from by another program of the same machine");
        drop(asked_from);
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let to = udp.local_addr().unwrap();
        let answering = Answering::start(udp, 4793, "New desk".into(), Arc::new(Anywhere)).unwrap();
        let found = find(to, Duration::from_millis(600), &Anywhere, port).unwrap();
        assert_eq!(found.len(), 1, "an answer to the port the question came from is heard");
        drop(answering);
    }

    #[test]
    fn a_network_that_is_not_home_gets_no_answer_and_is_not_believed() {
        struct Nowhere;
        impl Home for Nowhere {
            fn is_home(&self, _: IpAddr) -> bool {
                false
            }
        }
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let to = udp.local_addr().unwrap();
        let answering = Answering::start(udp, 4793, "New desk".into(), Arc::new(Nowhere)).unwrap();
        assert!(find(to, Duration::from_millis(500), &Anywhere, 0).unwrap().is_empty());
        drop(answering);
        // And an answer that comes from a network that is not home is passed over
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let to = udp.local_addr().unwrap();
        let answering = Answering::start(udp, 4793, "New desk".into(), Arc::new(Anywhere)).unwrap();
        assert!(find(to, Duration::from_millis(500), &Nowhere, 0).unwrap().is_empty());
        drop(answering);
    }
}
