//! amethystora-vault-service: holds the Vault's keys (plans/done/PLAN-vault.md). Started by `amethystora-vault.socket`, as its own
//! user, with no network: it takes the listening socket from systemd, and exits when nothing is open.

use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use amethystora_vault_core::slots::{SystemdCreds, LOG_N};
use amethystora_vault_service::desktop::Desktop;
use amethystora_vault_service::server::{Server, IDLE_EXIT};
use amethystora_vault_service::service::{Config, Service};

/// The descriptors systemd passes, as sd_listen_fds says: the vault's own socket, and the SSH agent's beside it, told apart by the
/// name of their socket units
fn listening_sockets() -> Option<(UnixListener, Option<UnixListener>)> {
    let pid: u32 = std::env::var("LISTEN_PID").ok()?.parse().ok()?;
    let count: i32 = std::env::var("LISTEN_FDS").ok()?.parse().ok()?;
    if pid != std::process::id() || !(1..=2).contains(&count) {
        return None;
    }
    let names = std::env::var("LISTEN_FDNAMES").unwrap_or_default();
    let (mut vault, mut agent) = (None, None);
    let mut named = names.split(':');
    for fd in 3..3 + count {
        // SAFETY: these are what the socket-activation protocol hands over, to this process alone, and nothing else here owns them
        let listener = unsafe {
            libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
            UnixListener::from_raw_fd(fd)
        };
        if named.next() == Some("amethystora-vault-ssh.socket") { agent = Some(listener) } else { vault = Some(listener) }
    }
    Some((vault?, agent))
}

/// SIGTERM and SIGINT, as a descriptor the loop waits on, so that stopping closes everything as any other closing does
fn signals() -> Option<RawFd> {
    // SAFETY: a signal set this function owns, blocked for every thread started after this
    unsafe {
        let mut set = std::mem::zeroed::<libc::sigset_t>();
        libc::sigemptyset(&raw mut set);
        libc::sigaddset(&raw mut set, libc::SIGTERM);
        libc::sigaddset(&raw mut set, libc::SIGINT);
        libc::sigprocmask(libc::SIG_BLOCK, &raw const set, std::ptr::null_mut());
        let fd = libc::signalfd(-1, &raw const set, libc::SFD_CLOEXEC | libc::SFD_NONBLOCK);
        (fd >= 0).then_some(fd)
    }
}

fn main() -> ExitCode {
    // SAFETY: prctl with these arguments only changes this process's flag; no core file, and no /proc/PID/mem for anyone
    unsafe {
        libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
    }
    // Argon2 for sync is made by a short-lived child of this very program, so that this one's memory never grows by it
    if std::env::args().nth(1).as_deref() == Some(amethystora_vault_service::sync::KDF_CHILD) {
        return amethystora_vault_service::sync::kdf_child();
    }
    // Beside this program, wherever it is installed. A process may still read its own /proc files once not dumpable.
    let worker = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|folder| folder.join("amethystora-vault-work"))).unwrap_or_else(|| PathBuf::from("/usr/libexec/amethystora-vault-work"));
    let Some((listener, agent)) = listening_sockets() else {
        eprintln!("amethystora-vault-service: start it through amethystora-vault.socket");
        return ExitCode::FAILURE;
    };
    let state = std::env::var_os("STATE_DIRECTORY").map_or_else(|| PathBuf::from("/var/lib/amethystora/vault"), PathBuf::from);
    let service = Service::new(Config { state, log_n: LOG_N, worker }, Arc::new(SystemdCreds), Box::new(Desktop::new()));
    match Server::new(listener, signals(), service, IDLE_EXIT) {
        Ok(server) => {
            match agent {
                Some(agent) => server.with_ssh(agent).run(),
                None => server.run(),
            }
            ExitCode::SUCCESS
        }
        Err(_) => ExitCode::FAILURE,
    }
}
