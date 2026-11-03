//! Who is asking: the kernel's word for the account (`SO_PEERCRED`) and for the process (`SO_PEERPIDFD`). Nothing the client
//! says about itself counts.

use std::fs;
use std::mem;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

// Not in the libc crate for every target it builds for
const SO_PEERPIDFD: libc::c_int = 77;

#[derive(Debug)]
pub struct Peer {
    pub uid: u32,
    pub pid: i32,
    /// Holds the process's number to that process, on kernels that have it (6.5 and later)
    pub pidfd: Option<OwnedFd>,
}

impl Peer {
    /// What the kernel says of the far end of a connected socket
    pub fn of(fd: RawFd) -> Option<Peer> {
        let mut credentials: libc::ucred = unsafe { mem::zeroed() };
        let mut length = mem::size_of::<libc::ucred>() as libc::socklen_t;
        // SAFETY: the kernel writes at most `length` bytes to a value of that size
        let asked = unsafe { libc::getsockopt(fd, libc::SOL_SOCKET, libc::SO_PEERCRED, (&raw mut credentials).cast(), &mut length) };
        if asked != 0 || length as usize != mem::size_of::<libc::ucred>() {
            return None;
        }
        let mut pidfd: libc::c_int = -1;
        let mut length = mem::size_of::<libc::c_int>() as libc::socklen_t;
        // SAFETY: as above; a new descriptor if it answers
        let answered = unsafe { libc::getsockopt(fd, libc::SOL_SOCKET, SO_PEERPIDFD, (&raw mut pidfd).cast(), &mut length) };
        // SAFETY: the descriptor the kernel just made, which nothing else owns
        let pidfd = (answered == 0 && pidfd >= 0).then(|| unsafe { OwnedFd::from_raw_fd(pidfd) });
        Some(Peer { uid: credentials.uid, pid: credentials.pid, pidfd })
    }

    /// False once the process that connected has gone, whatever now has its number. Without a pidfd, the start time polkit
    /// is given does the same.
    pub fn alive(&self) -> bool {
        let Some(pidfd) = &self.pidfd else { return true };
        let mut poll = libc::pollfd { fd: pidfd.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        // SAFETY: one pollfd, not waiting: a pidfd reads as ready when its process has ended
        unsafe { libc::poll(&mut poll, 1, 0) == 0 }
    }

    /// The process's start time, in clock ticks, which polkit takes with its number so that a new process under the same
    /// number is not what gets approved
    pub fn start_time(&self) -> Option<u64> {
        if !self.alive() {
            return None;
        }
        let stat = fs::read_to_string(format!("/proc/{}/stat", self.pid)).ok()?;
        // Its name can hold anything, a parenthesis too, and ends at the last one. Field 22, the 20th after it.
        stat.rsplit_once(')')?.1.split_ascii_whitespace().nth(19)?.parse().ok()
    }

    /// What the program calls itself, and what the approval calls it in those words. `/proc/PID/exe` would be the kernel's
    /// word, and is refused to this service's user for another account's process (the spike, 2026-11-02; `CAP_SYS_PTRACE`
    /// would also read every process's memory). So this proves nothing and never skips an approval. `comm` names a script
    /// by its own name rather than its interpreter's and is cut at 15 bytes, so `cmdline`'s first word finishes it when
    /// it starts the same. Nothing else is taken for a name, not a Flatpak's app ID from its cgroup either: the account chooses
    /// that too, since any program can start itself in a scope called anything. Every prompt says "a program calling itself".
    pub fn program(&self) -> String {
        let comm = fs::read_to_string(format!("/proc/{}/comm", self.pid)).unwrap_or_default();
        let comm = comm.trim_end_matches('\n');
        let first = fs::read(format!("/proc/{}/cmdline", self.pid)).unwrap_or_default();
        let first = first.split(|byte| *byte == 0).next().unwrap_or_default();
        let first = String::from_utf8_lossy(first);
        let base = first.rsplit('/').next().unwrap_or_default();
        let name = if !comm.is_empty() && base.starts_with(comm) { base } else { comm };
        clean(name, 100)
    }
}

/// Punctuation beyond ASCII's: the marks of the scripts people write in, and not the invisible ones (no line or paragraph
/// separator, no bidirectional control, no format character)
fn punctuation(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(c, '\u{a1}' | '\u{a7}' | '\u{ab}' | '\u{b6}' | '\u{b7}' | '\u{bb}' | '\u{bf}' | '\u{60c}' | '\u{61b}' | '\u{61f}')
        || matches!(c, '\u{2010}'..='\u{2027}' | '\u{2030}'..='\u{205e}' | '\u{3001}'..='\u{3003}' | '\u{3008}'..='\u{3011}' | '\u{ff01}'..='\u{ff0f}' | '\u{ff1a}'..='\u{ff20}')
}

/// Text somebody else wrote, made fit to show in a prompt: letters, digits, punctuation and a plain space, which is all a name
/// needs, with runs of spaces made one and the ends trimmed, and not too long. Nothing that draws nothing or moves the text
/// (separators, zero-width and format characters, bidirectional controls, tags) and no symbols or emoji get through.
pub fn clean(text: &str, most: usize) -> String {
    let mut out = String::new();
    let mut count = 0;
    for c in text.chars() {
        if count >= most {
            break;
        }
        if c == ' ' {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
                count += 1;
            }
        } else if c.is_alphanumeric() || punctuation(c) {
            out.push(c);
            count += 1;
        }
    }
    out.truncate(out.trim_end().len());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    #[test]
    fn the_kernel_says_who_is_asking() {
        let (a, _b) = UnixStream::pair().unwrap();
        let peer = Peer::of(a.as_raw_fd()).unwrap();
        // SAFETY: getuid and getpid cannot fail
        assert_eq!(peer.uid, unsafe { libc::getuid() });
        assert_eq!(peer.pid, unsafe { libc::getpid() });
        assert!(peer.alive());
        assert!(peer.start_time().is_some());
        assert!(!peer.program().is_empty());
    }

    #[test]
    fn a_program_goes_on_a_prompt_without_its_tricks() {
        assert_eq!(clean("  ssh\n\u{1b}[31m\u{202e}x ", 100), "ssh[31mx");
        assert_eq!(clean("abcdef", 3), "abc");
    }

    #[test]
    fn only_what_a_name_needs_gets_through() {
        // Separators, zero-width and format characters, the soft hyphen, tags, the isolates, no-break space and an emoji
        for invisible in ["\u{2028}", "\u{2029}", "\u{200b}", "\u{200d}", "\u{feff}", "\u{ad}", "\u{e0041}", "\u{206a}", "\u{fff9}", "\u{2066}", "\u{85}", "\u{a0}", "\u{1f511}"] {
            assert_eq!(clean(&format!("a{invisible}b"), 100), "ab", "{invisible:?}");
        }
        // Letters of any script, digits, punctuation, and a plain space made one
        assert_eq!(clean("  Pa\u{ff}\u{e9}  “Mon\u{e9}” — 日本語, العربية؟  ", 100), "Pa\u{ff}\u{e9} “Mon\u{e9}” — 日本語, العربية؟");
        assert_eq!(clean("a \t\n b", 100), "a b");
        assert_eq!(clean("a      b", 100), "a b");
        // Cut where it is cut, never ending on a space
        assert_eq!(clean("abc def", 4), "abc");
        assert!(clean(&"x ".repeat(500), 100).chars().count() <= 100);
    }
}
