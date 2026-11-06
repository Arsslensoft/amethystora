//! Only a network named home (R7-25, and the rule of the phone link): the new computer listens and the old one calls only
//! where the interface that reaches the other is in firewalld's `amethystora-home` zone. The firewall's own rule is the first
//! lock (the move's port is open in that zone alone); this is the second, and it fails closed: a question that cannot be
//! answered counts as not home. Run as the account: firewalld answers a read to anyone.

use std::net::IpAddr;
use std::process::Command;

/// The zone of the networks the person named home (`ame security home-networks`)
pub const HOME_ZONE: &str = "amethystora-home";

pub trait Home {
    fn is_home(&self, peer: IpAddr) -> bool;
}

/// Asks the machine: which interface reaches the address, and which zone that interface is in
pub struct Firewalld;

impl Home for Firewalld {
    fn is_home(&self, peer: IpAddr) -> bool {
        if peer.is_loopback() || peer.is_unspecified() || peer.is_multicast() {
            return false;
        }
        let Some(interface) = interface_to(peer) else { return false };
        zone_of(&interface).as_deref() == Some(HOME_ZONE)
    }
}

fn interface_to(peer: IpAddr) -> Option<String> {
    let out = Command::new("ip").args(["-o", "route", "get", &peer.to_string()]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    interface_of(&String::from_utf8_lossy(&out.stdout))
}

/// The word after `dev` in the line `ip -o route get` prints, if it is an interface name
pub fn interface_of(line: &str) -> Option<String> {
    let mut words = line.split_whitespace();
    while let Some(word) = words.next() {
        if word == "dev" {
            return words.next().filter(|n| !n.is_empty() && n.len() <= 15 && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '@'))).map(str::to_string);
        }
    }
    None
}

fn zone_of(interface: &str) -> Option<String> {
    let out = Command::new("firewall-cmd").arg(format!("--get-zone-of-interface={interface}")).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Every address is home (the tests, which run on one machine)
pub struct Anywhere;

impl Home for Anywhere {
    fn is_home(&self, _: IpAddr) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_interface_is_read_from_the_route() {
        assert_eq!(interface_of("192.168.1.5 via 10.0.0.1 dev enp0s31f6 src 10.0.0.199 uid 1000 \\    cache"), Some("enp0s31f6".to_string()));
        assert_eq!(interface_of("local 127.0.0.1 dev lo src 127.0.0.1 uid 1000"), Some("lo".to_string()));
        assert_eq!(interface_of("nothing"), None);
        assert_eq!(interface_of("x dev --evil=1"), None);
        assert_eq!(interface_of("x dev"), None);
    }

    #[test]
    fn loopback_is_never_home() {
        assert!(!Firewalld.is_home("127.0.0.1".parse().unwrap()));
        assert!(!Firewalld.is_home("::1".parse().unwrap()));
        assert!(!Firewalld.is_home("0.0.0.0".parse().unwrap()));
    }
}
