//! amethystora-vault-net: started by `amethystora-vault-net.socket` when the vault's service syncs, as its own user. Serves each
//! connection on a thread of its own, so that one account's slow server never holds up another's (the review's M7), and exits once
//! nobody has connected for a while: it runs only while sync is working.

use std::io::BufReader;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::net::UnixListener;
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use amethystora_vault_net::http::Session;

#[global_allocator]
static WIPING: amethystora_vault_net::alloc::Wiping = amethystora_vault_net::alloc::Wiping;

/// No connection for this long and it goes; the socket stays with systemd
const IDLE: Duration = Duration::from_secs(20);
/// The vault's service is slow to send the next line only while it works on the last answer
const PATIENCE: Duration = Duration::from_secs(120);
/// Connections at once: a run holds one, and a few accounts sync at the same time. A body is 32 MiB at most, so this is what keeps
/// the unit's memory limit meaning something.
const MOST_CONNECTIONS: usize = 4;

fn main() -> ExitCode {
    // SAFETY: only this process's flag: no core file, and no /proc/PID/mem for anyone
    unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) };
    let ours = std::env::var("LISTEN_PID").ok().and_then(|pid| pid.parse::<u32>().ok()) == Some(std::process::id()) && std::env::var("LISTEN_FDS").as_deref() == Ok("1");
    if !ours {
        eprintln!("amethystora-vault-net: start it through amethystora-vault-net.socket");
        return ExitCode::FAILURE;
    }
    // SAFETY: fd 3 is what socket activation hands this process alone
    let listener = unsafe {
        libc::fcntl(3, libc::F_SETFD, libc::FD_CLOEXEC);
        UnixListener::from_raw_fd(3)
    };
    let active = Arc::new(AtomicUsize::new(0));
    loop {
        let mut poll = libc::pollfd { fd: listener.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        // SAFETY: one live pollfd
        let ready = unsafe { libc::poll(&mut poll, 1, IDLE.as_millis() as i32) };
        if ready == 0 {
            if active.load(Ordering::SeqCst) == 0 {
                return ExitCode::SUCCESS;
            }
            continue;
        }
        if ready < 0 {
            continue;
        }
        let Ok((stream, _)) = listener.accept() else { continue };
        if active.load(Ordering::SeqCst) >= MOST_CONNECTIONS || stream.set_read_timeout(Some(PATIENCE)).is_err() || stream.set_write_timeout(Some(PATIENCE)).is_err() {
            continue;
        }
        let Ok(mut writer) = stream.try_clone() else { continue };
        active.fetch_add(1, Ordering::SeqCst);
        let counted = active.clone();
        let spawned = std::thread::Builder::new().stack_size(512 * 1024).spawn(move || {
            let mut reader = BufReader::new(stream);
            amethystora_vault_net::serve(&mut reader, &mut writer, &|origins, ca, pin| Session::new(origins, ca, pin));
            counted.fetch_sub(1, Ordering::SeqCst);
        });
        if spawned.is_err() {
            active.fetch_sub(1, Ordering::SeqCst);
        }
    }
}
