//! What the job process shuts itself off from (`confine.rs`): the test starts itself again as the probe, which confines itself and
//! then tries what a compromised parser would, so that the filter and the file rules are proved by what they stop.

use std::ffi::CString;
use std::process::Command;

use amethystora_vault_service::confine::confine;

const PROBE: &str = "AMETHYSTORA_CONFINE_PROBE";

fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

#[test]
fn a_job_cannot_reach_the_state_a_socket_or_another_program() {
    if let Ok(state) = std::env::var(PROBE) {
        probe(&state);
        return;
    }
    let state = std::env::temp_dir().join(format!("amethystora-confine-{}", std::process::id()));
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("account"), b"sealed").unwrap();
    let me = std::env::current_exe().unwrap();
    let out = Command::new(me).args(["--exact", "a_job_cannot_reach_the_state_a_socket_or_another_program", "--nocapture", "--test-threads=1"]).env(PROBE, &state).output().unwrap();
    let _ = std::fs::remove_dir_all(&state);
    assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

fn probe(state: &str) {
    let held = confine();
    // Where the checks are required (the apps layer sets it, as for the verifiers), a part that did not take hold is a failure
    // and not a skip: the test would otherwise pass on a kernel or sandbox with neither
    if std::env::var_os("VAULT_VERIFIERS_REQUIRED").is_some() {
        assert!(held.files && held.calls, "the job process was not confined: {held:?}");
    }
    // SAFETY: plain calls with valid arguments, made to see them refused
    unsafe {
        if held.calls {
            assert_eq!(libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0), -1);
            assert_eq!(errno(), libc::EPERM, "a socket");
            let shell = CString::new("/bin/true").unwrap();
            let argv = [shell.as_ptr(), std::ptr::null()];
            assert_eq!(libc::execv(shell.as_ptr(), argv.as_ptr()), -1);
            assert_eq!(errno(), libc::EPERM, "another program");
            assert_eq!(libc::ptrace(libc::PTRACE_ATTACH, libc::getppid(), 0, 0), -1);
            assert_eq!(errno(), libc::EPERM, "ptrace");
        }
        if held.files {
            let file = CString::new(format!("{state}/account")).unwrap();
            assert_eq!(libc::open(file.as_ptr(), libc::O_RDONLY), -1);
            assert_eq!(errno(), libc::EACCES, "the state");
            let write = CString::new("/etc/amethystora-confine-test").unwrap();
            assert_eq!(libc::open(write.as_ptr(), libc::O_WRONLY | libc::O_CREAT, 0o600), -1);
            let libraries = CString::new("/usr").unwrap();
            let fd = libc::open(libraries.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY);
            assert!(fd >= 0, "/usr is read");
            libc::close(fd);
        }
    }
    println!("confined: files {} calls {}", held.files, held.calls);
}
