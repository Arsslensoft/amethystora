//! amethystora-agent-root: runs the AI agent's root commands, started by the agent's tool server with pkexec in
//! the mode the administrator chose (/etc/amethystora/agent.conf's ROOT), after an administrator's password.
//!
//!   pkexec /usr/libexec/amethystora-agent-root session|minutes|each
//!
//! It trusts nothing its caller says. It reads agent.conf itself, and runs only for the tool server
//! (/usr/libexec/amethystora-agent-tools) of an account the lock allows to act and that turned changes on,
//! under an agent whose own checks are on. It talks only over the socket pair on its stdin and stdout, one
//! JSON request a line ({id, argv, reason}) and one answer a line ({id, status, output} or {id, refused}):
//! a socket cannot be opened through /proc/PID/fd by another process of the account, as a pipe could. Each
//! command is an argument vector, never a shell, run with `env -i`, in /, for ten minutes at most, its output
//! capped; the never list is refused; and each one is said in a notification to the account and a journal
//! line (identifier amethystora-agent-root) before it runs.
//!
//! In the `session` mode it serves until its socket closes (the agent ended), the account's screen locks, an
//! hour passes without a request, or agent.conf changes. In the others it serves one request.

use std::ffi::CString;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use amethystora_agent_tools::policy::{self, Conf, Level, RootMode};
use amethystora_agent_tools::{checks, history, never, run, text};
use serde_json::{Value, json};

const TOOLS: &str = "/usr/libexec/amethystora-agent-tools";
const NOTIFY: &str = "/usr/libexec/amethystora-notify-users";
const PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin";
/// The most a command may take, and the most of what it prints that is kept and given back
const LIMIT: Duration = Duration::from_secs(600);
const KEPT: usize = 8192;
const GIVEN: usize = 4096;
const IDLE: Duration = Duration::from_secs(3600);
const MAX_LINE: usize = 1 << 16;

fn journal(message: &str) {
    static IDENT: &[u8] = b"amethystora-agent-root\0";
    let message = CString::new(text::field(message, 2000)).unwrap_or_default();
    unsafe {
        libc::openlog(IDENT.as_ptr().cast(), libc::LOG_PID, libc::LOG_AUTHPRIV);
        libc::syslog(libc::LOG_NOTICE, c"%s".as_ptr(), message.as_ptr());
    }
}

fn refuse(why: &str) -> ExitCode {
    eprintln!("amethystora-agent-root: {why}");
    journal(&format!("refused to start: {why}"));
    ExitCode::from(1)
}

/// The account's home, as the system has it
fn home_of(uid: u32) -> Option<PathBuf> {
    let mut buffer = vec![0 as libc::c_char; 16384];
    let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result: *mut libc::passwd = std::ptr::null_mut();
    let found = unsafe { libc::getpwuid_r(uid, &mut entry, buffer.as_mut_ptr(), buffer.len(), &mut result) };
    if found != 0 || result.is_null() {
        return None;
    }
    Some(PathBuf::from(unsafe { std::ffi::CStr::from_ptr(entry.pw_dir) }.to_string_lossy().into_owned()))
}

/// agent.conf's text, only when root alone can write it; None for a file anyone else could have changed
fn conf_text() -> Result<String, String> {
    let path = Path::new(policy::CONF);
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.uid() != 0 || meta.mode() & 0o022 != 0 || !meta.file_type().is_file() => {
            Err(format!("{} is not root's alone", policy::CONF))
        }
        Ok(_) => std::fs::read_to_string(path).map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.to_string()),
    }
}

/// Whether one of the account's desktop sessions is locked (GNOME tells logind, LockedHint)
fn locked(uid: u32) -> bool {
    let sessions = run::run(&["loginctl", "list-sessions", "--no-legend"], Duration::from_secs(10)).stdout;
    for line in sessions.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 2 || fields[1] != uid.to_string() {
            continue;
        }
        let shown = run::run(&["loginctl", "show-session", fields[0], "-p", "Type", "-p", "LockedHint"], Duration::from_secs(10)).stdout;
        let graphical = shown.lines().any(|line| line == "Type=wayland" || line == "Type=x11");
        if graphical && shown.lines().any(|line| line == "LockedHint=yes") {
            return true;
        }
    }
    false
}

/// Whether stdin is a socket: the pair the tool server made, and nothing a terminal or a pipe could be
fn stdin_is_socket() -> bool {
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    unsafe { libc::fstat(0, &mut stat) == 0 && stat.st_mode & libc::S_IFMT == libc::S_IFSOCK }
}

/// One request's argument vector and reason, as far as they are well made
fn request(line: &str) -> Result<(String, Vec<String>, String), String> {
    let value: Value = serde_json::from_str(line).map_err(|_| "not JSON".to_string())?;
    let id = value["id"].as_str().filter(|id| !id.is_empty() && id.len() <= 16 && id.chars().all(|c| c.is_ascii_hexdigit())).ok_or("no id")?;
    let argv: Vec<String> = value["argv"]
        .as_array()
        .filter(|list| !list.is_empty() && list.len() <= 64)
        .ok_or("argv is 1 to 64 words")?
        .iter()
        .map(|word| word.as_str().filter(|word| word.len() <= 4096 && !word.contains('\0')).map(str::to_string))
        .collect::<Option<_>>()
        .ok_or("each word of argv is text of at most 4096 characters")?;
    let reason = value["reason"].as_str().map(|reason| text::field(reason, 600)).unwrap_or_default();
    Ok((id.to_string(), argv, reason))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(mode) = args.first().and_then(|word| RootMode::parse(word)).filter(|mode| *mode != RootMode::Terminal) else {
        return refuse("usage: pkexec amethystora-agent-root session|minutes|each");
    };
    if args.len() != 1 {
        return refuse("usage: pkexec amethystora-agent-root session|minutes|each");
    }
    if unsafe { libc::geteuid() } != 0 {
        return refuse("not root: it is started with pkexec");
    }
    // pkexec says who asked; started any other way, nobody did
    let Some(uid) = std::env::var("PKEXEC_UID").ok().and_then(|uid| uid.parse::<u32>().ok()) else {
        return refuse("not started by pkexec");
    };
    if !stdin_is_socket() {
        return refuse("it talks only over the tool server's socket");
    }
    // Only the agent's tool server, of the same account, asks it
    let parent = unsafe { libc::getppid() } as u32;
    let server = std::fs::read_link(format!("/proc/{parent}/exe")).unwrap_or_default();
    let server_uid = std::fs::metadata(format!("/proc/{parent}")).map(|meta| meta.uid()).unwrap_or(u32::MAX);
    if server != Path::new(TOOLS) || server_uid != uid {
        return refuse("only the AI agent's tool server starts it");
    }
    let first = match conf_text() {
        Ok(text) => text,
        Err(error) => return refuse(&error),
    };
    let conf = Conf::parse(&first);
    let account = policy::account_of(uid).unwrap_or_else(|| uid.to_string());
    if conf.root != mode {
        return refuse(&format!("the administrator chose the {} mode for root commands, not {}", conf.root.word(), mode.word()));
    }
    if conf.ceiling(&account) < Level::Act {
        return refuse("an administrator does not let the agent make changes for this account");
    }
    let Some(home) = home_of(uid) else { return refuse("the account has no home") };
    let config = home.join(".config/amethystora");
    if std::fs::symlink_metadata(config.join("agentic")).is_err() || std::fs::symlink_metadata(config.join("agent-tools")).is_err() {
        return refuse("the account has not let the agent make changes");
    }
    // Only a tool server that an agent started through the launcher is served, not one a program started itself
    if let Err(why) = checks::launched_by_launcher(parent, checks::environment_of(parent).as_deref()) {
        return refuse(&why);
    }
    // The agent above the tool server, with its own checks off, is refused everything
    let flags = std::fs::read_to_string(checks::FLAGS).unwrap_or_default();
    if let Some((agent, Some(flag))) = checks::agent_above(parent, &flags) {
        return refuse(&format!("{agent} runs with {flag}, which turns its own checks off"));
    }

    // SAFETY: fd 0 and 1 are the socket pair, which nothing else here uses
    let input = unsafe { std::fs::File::from_raw_fd(0) };
    let mut output = unsafe { std::fs::File::from_raw_fd(1) };
    let mut reader = BufReader::new(input);
    let mut last = Instant::now();
    let reply = |output: &mut std::fs::File, value: Value| -> bool { writeln!(output, "{value}").and_then(|()| output.flush()).is_ok() };
    loop {
        // Waits for a request a few seconds at a time, and in the session mode ends its session in between
        let mut ready = libc::pollfd { fd: reader.get_ref().as_raw_fd(), events: libc::POLLIN, revents: 0 };
        let polled = if reader.buffer().is_empty() { unsafe { libc::poll(&mut ready, 1, 5000) } } else { 1 };
        if polled == 0 {
            if mode == RootMode::Session {
                if last.elapsed() > IDLE {
                    journal(&format!("{account}'s root session ended after an hour without a command"));
                    return ExitCode::SUCCESS;
                }
                if locked(uid) {
                    journal(&format!("{account}'s root session ended: the screen locked"));
                    return ExitCode::SUCCESS;
                }
                if conf_text().ok().as_deref() != Some(first.as_str()) {
                    journal(&format!("{account}'s root session ended: agent.conf changed"));
                    return ExitCode::SUCCESS;
                }
            }
            continue;
        }
        if polled < 0 {
            return ExitCode::FAILURE;
        }
        let mut line = String::new();
        match (&mut reader).take(MAX_LINE as u64).read_line(&mut line) {
            Ok(0) | Err(_) => return ExitCode::SUCCESS,
            Ok(_) => {}
        }
        last = Instant::now();
        let (id, argv, reason) = match request(&line) {
            Ok(request) => request,
            Err(why) => {
                if !reply(&mut output, json!({"refused": why})) {
                    return ExitCode::FAILURE;
                }
                continue;
            }
        };
        let answer = if conf_text().ok().as_deref() != Some(first.as_str()) {
            json!({"id": id, "refused": "agent.conf changed since this helper started: ask again"})
        } else if let Some(why) = never::refusal(&argv) {
            journal(&format!("{account} asked for {} as root, refused: {why}", history::words(&json!(argv))));
            json!({"id": id, "refused": why})
        } else {
            let shown = history::words(&json!(argv));
            journal(&format!("{account}'s AI agent ran as root: {shown} (its reason: {reason})"));
            let _ = run::run(
                &[NOTIFY, "--user", &account, "The AI agent ran a command as root", &format!("{shown}\nIts reason: {reason}")],
                Duration::from_secs(15),
            );
            let ran = run::run_clean(&argv, &[("PATH", PATH), ("LANG", "C.UTF-8"), ("HOME", "/root")], LIMIT, KEPT);
            journal(&format!(
                "{account}'s root command ended: {}",
                match (ran.timed_out, ran.code) {
                    (true, _) => "stopped after ten minutes".to_string(),
                    (_, Some(code)) => format!("exit {code}"),
                    _ => "could not start".to_string(),
                }
            ));
            let given = run::tail(&ran.stdout, GIVEN).to_string();
            json!({"id": id, "status": ran.code, "timed_out": ran.timed_out, "output": given})
        };
        if !reply(&mut output, answer) {
            return ExitCode::FAILURE;
        }
        if mode != RootMode::Session {
            return ExitCode::SUCCESS;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_checked() {
        assert!(request(r#"{"id": "ab12", "argv": ["rpm-ostree", "status"], "reason": "why"}"#).is_ok());
        assert!(request(r#"{"id": "ab12", "argv": [], "reason": ""}"#).is_err());
        assert!(request(r#"{"id": "../x", "argv": ["a"]}"#).is_err());
        assert!(request(r#"{"id": "ab", "argv": ["a", 1]}"#).is_err());
        assert!(request("rm -rf /").is_err());
        let (_, _, reason) = request(r#"{"id": "ab", "argv": ["a"], "reason": "x\u001b[2Jy"}"#).unwrap();
        assert_eq!(reason, "x[2Jy");
    }
}
