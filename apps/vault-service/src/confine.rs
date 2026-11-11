//! What `amethystora-vault-work` shuts itself off from before it reads its job. The child shares the service's user, and so its
//! state folder (every account's sealed vaults) and the right to ask systemd-creds for the chip's secrets: it has no need of
//! either, since a job arrives on standard input and its answer leaves on standard output, and what parses a stranger's key,
//! certificate or sync answer in libcrypto must not be able to reach them if it goes wrong.
//!
//! Two things, both with raw system calls (no crate): Landlock lets it read only `/usr`, `/etc`, `/dev`, `/proc` and `/sys`
//! (libcrypto's libraries, providers and configuration, the kernel's random numbers) and write nothing, so `/var/lib` (the state),
//! `/run` (the sockets, systemd-creds') and `/tmp` are closed to it; and a seccomp filter takes away the calls that would reach
//! the chip or the service by another road: sockets, running programs, ptrace, other processes' memory and descriptors. Each is
//! best effort in itself (a kernel without Landlock, or an architecture the filter is not written for, goes without that part)
//! and `Confined` says which held. The service's own unit already has `NoNewPrivileges`.

use std::ffi::CString;

/// Which parts took hold
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Confined {
    pub files: bool,
    pub calls: bool,
}

/// Landlock's first ABI: every file access it knows
const HANDLED: u64 = (1 << 13) - 1;
/// `LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_READ_DIR`
const READ: u64 = (1 << 2) | (1 << 3);
/// What libcrypto and the dynamic loader read
const READABLE: [&str; 5] = ["/usr", "/etc", "/dev", "/proc", "/sys"];

const SYS_LANDLOCK_CREATE_RULESET: libc::c_long = 444;
const SYS_LANDLOCK_ADD_RULE: libc::c_long = 445;
const SYS_LANDLOCK_RESTRICT_SELF: libc::c_long = 446;
const LANDLOCK_RULE_PATH_BENEATH: libc::c_long = 1;

#[repr(C)]
struct RulesetAttr {
    handled_access_fs: u64,
}

#[repr(C, packed)]
struct PathBeneath {
    allowed_access: u64,
    parent_fd: i32,
}

/// One instruction of a classic BPF program (`struct sock_filter`)
#[repr(C)]
#[derive(Clone, Copy)]
struct Instruction {
    code: u16,
    jump_true: u8,
    jump_false: u8,
    constant: u32,
}

/// `struct sock_fprog`
#[repr(C)]
struct Program {
    length: u16,
    filter: *const Instruction,
}

const LOAD_WORD: u16 = 0x20;
const JUMP_EQUAL: u16 = 0x15;
const JUMP_SET: u16 = 0x45;
const RETURN: u16 = 0x06;
const ALLOW: u32 = 0x7fff_0000;
const KILL: u32 = 0x8000_0000;
const FAIL_WITH_PERMISSION: u32 = 0x0005_0000 | 1;
const X32_BIT: u32 = 0x4000_0000;

#[cfg(target_arch = "x86_64")]
const ARCHITECTURE: Option<u32> = Some(0xC000_003E);
#[cfg(target_arch = "aarch64")]
const ARCHITECTURE: Option<u32> = Some(0xC000_00B7);
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
const ARCHITECTURE: Option<u32> = None;

/// The calls a job never makes, which each failed with EPERM: a way to the chip's secrets or the service, or out of the job
fn denied() -> Vec<u32> {
    [
        libc::SYS_socket,
        libc::SYS_socketpair,
        libc::SYS_connect,
        libc::SYS_bind,
        libc::SYS_execve,
        libc::SYS_execveat,
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_pidfd_open,
        libc::SYS_pidfd_getfd,
        libc::SYS_io_uring_setup,
        libc::SYS_open_by_handle_at,
        libc::SYS_keyctl,
        libc::SYS_add_key,
        libc::SYS_request_key,
    ]
    .iter()
    .map(|number| *number as u32)
    .collect()
}

/// The filter's program: the wrong architecture and x32 are killed, the calls above fail, the rest pass
fn program(architecture: u32, denied: &[u32]) -> Vec<Instruction> {
    let step = |code, jump_true, jump_false, constant| Instruction { code, jump_true, jump_false, constant };
    // `seccomp_data`: the call's number at 0 and the architecture at 4
    let mut filter = vec![step(LOAD_WORD, 0, 0, 4), step(JUMP_EQUAL, 1, 0, architecture), step(RETURN, 0, 0, KILL), step(LOAD_WORD, 0, 0, 0)];
    if cfg!(target_arch = "x86_64") {
        filter.push(step(JUMP_SET, 0, 1, X32_BIT));
        filter.push(step(RETURN, 0, 0, KILL));
    }
    for number in denied {
        filter.push(step(JUMP_EQUAL, 0, 1, *number));
        filter.push(step(RETURN, 0, 0, FAIL_WITH_PERMISSION));
    }
    filter.push(step(RETURN, 0, 0, ALLOW));
    filter
}

fn seccomp() -> bool {
    let Some(architecture) = ARCHITECTURE else { return false };
    let filter = program(architecture, &denied());
    let program = Program { length: filter.len() as u16, filter: filter.as_ptr() };
    // SAFETY: prctl with a filter that lives through the call; it only restricts this process and what it starts
    unsafe { libc::prctl(libc::PR_SET_SECCOMP, 2, &raw const program, 0, 0) == 0 }
}

fn landlock() -> bool {
    let attr = RulesetAttr { handled_access_fs: HANDLED };
    // SAFETY: the call reads `attr`, which is the size it is told; what it returns is a descriptor or an error
    let ruleset = unsafe { libc::syscall(SYS_LANDLOCK_CREATE_RULESET, &raw const attr, std::mem::size_of::<RulesetAttr>(), 0) };
    if ruleset < 0 {
        return false;
    }
    let ruleset = ruleset as libc::c_int;
    for path in READABLE {
        let Ok(path) = CString::new(path) else { continue };
        // SAFETY: a plain open of a path, whose descriptor is closed below
        let parent = unsafe { libc::open(path.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
        if parent < 0 {
            continue;
        }
        let rule = PathBeneath { allowed_access: READ, parent_fd: parent };
        // SAFETY: `rule` is the struct the call reads for a rule of this kind
        unsafe {
            libc::syscall(SYS_LANDLOCK_ADD_RULE, ruleset, LANDLOCK_RULE_PATH_BENEATH, &raw const rule, 0);
            libc::close(parent);
        }
    }
    // SAFETY: restricts this process alone, from a ruleset it holds
    let held = unsafe { libc::syscall(SYS_LANDLOCK_RESTRICT_SELF, ruleset, 0) } == 0;
    // SAFETY: the ruleset's descriptor is ours and no longer needed
    unsafe { libc::close(ruleset) };
    held
}

/// Takes the job's process off the state, the sockets and every other program. Call it first, before libcrypto has read anything.
pub fn confine() -> Confined {
    // SAFETY: no new privileges for this process and what it starts, which both parts need and the unit asks for anyway
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Confined { files: false, calls: false };
    }
    Confined { files: landlock(), calls: seccomp() }
}
