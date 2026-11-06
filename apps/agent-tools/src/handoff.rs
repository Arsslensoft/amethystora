//! The hand-off to a terminal: a command the agent cannot run itself (one that asks questions, or a root
//! command in the `terminal` mode) opens in a terminal for the person at the machine, through
//! amethystora-in-terminal --agent --report, which shows it and the agent's reason and runs it only after a
//! yes. How it ended comes back through a folder of the account's own: the call returns at once with an id,
//! and `history` with that id waits for the result.
//!
//! The yes is asked in the live pill first, when the desktop has one (R7-12): the shell's `org.amethystora.Agent.Ask` on the session
//! bus, which answers with the person's click on Allow or Not now. The question is a file of this folder (`ask.json`), written
//! here, so that what the person reads is the command that runs; the answer is the bus's reply and nothing a file can say, so no
//! program of the account, this agent included, can answer for the person. After an Allow the terminal opens with `--approved`
//! and runs the command (a root command's password is still asked by sudo there). With no live pill, or a command too long to
//! read in a card, the question is the terminal's, as it was.

use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::history::Jobs;
use crate::{run, text};

pub const IN_TERMINAL: &str = "/usr/libexec/amethystora-in-terminal";
const GDBUS: &str = "/usr/bin/gdbus";
/// How long the person has to answer the pill's question
const ASK_PATIENCE: u64 = 3600;
/// A reply sooner than this that is not a yes or a no means nobody is asking in a pill: the terminal asks
const UNAVAILABLE_WITHIN: Duration = Duration::from_secs(10);
/// How long a hand-off waits for the person at the machine
const PATIENCE: Duration = Duration::from_secs(3600);

/// The folder a hand-off's result comes back through
pub fn dir(id: &str) -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })));
    runtime.join("amethystora/agent-tools").join(id)
}

/// Whether there is a desktop to open a terminal on
pub fn graphical() -> bool {
    ["WAYLAND_DISPLAY", "DISPLAY"].iter().any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
}

/// How a finished hand-off ended, in words for the agent, or None while it still waits
pub fn result(id: &str) -> Option<String> {
    let dir = dir(id);
    let status = std::fs::read_to_string(dir.join("status")).ok()?;
    let status = status.trim();
    if status == "declined" {
        return Some("The user declined: it was not run.".into());
    }
    // The question was up and the person did not answer it (the screen locked, the features went off): not a no, and not run
    if status == "unanswered" {
        return Some("Nobody answered the question in the desktop's pill (it was closed, the screen locked or it timed out): it was not run.".into());
    }
    let output = std::fs::read(dir.join("output")).unwrap_or_default();
    let output = run::strip_ansi(&String::from_utf8_lossy(&output));
    let lines: Vec<String> = run::tail(&output, 4096).lines().map(str::to_string).collect();
    let mut said = format!("It ran in the terminal and exited with {status}.");
    if !lines.is_empty() {
        said.push('\n');
        said.push_str(&text::data_block("What it printed, last 4 KB", &lines));
    }
    Some(said)
}

/// What the pill's question came to
#[derive(Debug, PartialEq)]
pub enum Pill {
    Allowed,
    Declined,
    /// Nobody asks in a pill here (no extension, the key off, a command too long): the terminal asks
    Unavailable,
    /// The question was up and nobody answered it: a lock, the features turned off, the hour running out. Not a no
    Unanswered,
}

/// What the pill is asked: written to `ask.json` for the shell to read, so that the command a person reads is the one that runs
pub struct Question {
    /// `run` for a command, `change` for a setting that asks something
    pub kind: &'static str,
    /// The command as words, without the `sudo --` a root command is shown with
    pub command: Vec<String>,
    pub root: bool,
    /// A setting's name for a change
    pub title: Option<String>,
}

/// The SHA-256 of `bytes` in hex: what the shell is told to find in the question file, so that what a person reads is what was written
fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// How the shell's reply reads: a yes, a no, or neither
pub fn reply(stdout: &str) -> Option<bool> {
    match stdout.trim() {
        "(true,)" => Some(true),
        "(false,)" => Some(false),
        _ => None,
    }
}

/// Puts the question to the pill and waits for the person. The `gdbus` call dies with this thread (PDEATHSIG), so that a tool
/// server that ends does not leave a question behind that nobody can answer.
fn ask_pill(id: &str, dir: &std::path::Path, question: &Question, reason: &str) -> Pill {
    let file = json!({
        "kind": question.kind, "command": question.command, "root": question.root, "title": question.title,
        "reason": text::field(reason, 600), "server": std::process::id(),
    });
    let bytes = file.to_string();
    let written = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(dir.join("ask.json"))
        .and_then(|mut out| std::io::Write::write_all(&mut out, bytes.as_bytes()));
    if written.is_err() || !std::path::Path::new(GDBUS).exists() {
        return Pill::Unavailable;
    }
    let mut command = Command::new(GDBUS);
    command
        .args(["call", "--session", "--timeout", &ASK_PATIENCE.to_string(), "--dest", "org.gnome.Shell", "--object-path", "/org/amethystora/Agent"])
        // The file's digest goes with the id, in this child's own arguments: a file rewritten after it was written is not shown
        .args(["--method", "org.amethystora.Agent.Ask", &format!("'{id}'"), &format!("'{}'", digest(bytes.as_bytes()))])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        command.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            Ok(())
        });
    }
    let started = Instant::now();
    let output = command.output();
    // What was asked is not kept once it has been answered
    let _ = std::fs::remove_file(dir.join("ask.json"));
    let Ok(output) = output else { return Pill::Unavailable };
    match reply(&String::from_utf8_lossy(&output.stdout)) {
        Some(true) => Pill::Allowed,
        Some(false) => Pill::Declined,
        None if String::from_utf8_lossy(&output.stderr).contains("org.amethystora.Agent.Unanswered") => Pill::Unanswered,
        // An error: a refusal at once is "not here"; one after the person had time to answer is no answer, which is a no
        None if started.elapsed() < UNAVAILABLE_WITHIN => Pill::Unavailable,
        None => Pill::Declined,
    }
}

/// Opens `argv` in a terminal with the agent's reason, and starts a job that ends when the person has
/// answered and it has run. The id is the job's and the record's; `ended` is told how it ended, first. `phase` is told where it
/// stands: `asking` (the pill's question is up), `opened` (the terminal asks), `running` (allowed in the pill, the terminal runs it).
pub fn open(
    jobs: &Jobs,
    id: &str,
    argv: &[String],
    reason: &str,
    question: Question,
    phase: Box<dyn Fn(&str) + Send>,
    ended: Box<dyn FnOnce(&str) + Send>,
) -> Result<(), String> {
    if !graphical() {
        return Err("There is no desktop session to open a terminal in: the user runs it themselves.".into());
    }
    let dir = dir(id);
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .map_err(|error| format!("{}: {error}", dir.display()))?;
    jobs.start(id);
    let jobs = jobs.clone();
    let id = id.to_string();
    let argv = argv.to_vec();
    let reason = reason.to_string();
    std::thread::spawn(move || {
        phase("asking");
        let pill = ask_pill(&id, &dir, &question, &reason);
        let answered = pill != Pill::Unanswered;
        let approved = match pill {
            Pill::Declined | Pill::Unanswered => {
                let _ = std::fs::write(dir.join("status"), if answered { "declined\n" } else { "unanswered\n" });
                let said = result(&id).unwrap_or_default();
                ended(&said);
                jobs.finish(&id, said);
                return;
            }
            Pill::Allowed => true,
            Pill::Unavailable => false,
        };
        phase(if approved { "running" } else { "opened" });
        let mut terminal = Command::new(IN_TERMINAL);
        terminal.arg("--agent").arg(text::field(&reason, 600)).arg("--report").arg(&dir);
        if approved {
            terminal.arg("--approved");
        }
        let child = terminal.arg("--").args(&argv).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn();
        let mut child = match child {
            Ok(child) => child,
            Err(error) => {
                let said = format!("Could not open a terminal: {IN_TERMINAL}: {error}");
                ended(&said);
                jobs.finish(&id, said);
                return;
            }
        };
        // The terminal launcher returns at once; the command's result comes through the folder
        let _ = child.wait();
        let started = Instant::now();
        let said = loop {
            if let Some(said) = result(&id) {
                break said;
            }
            if started.elapsed() > PATIENCE {
                break "Nobody answered in the terminal within an hour.".to_string();
            }
            std::thread::sleep(Duration::from_millis(500));
        };
        ended(&said);
        jobs.finish(&id, said);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_yes_or_a_no_from_the_pill_counts() {
        assert_eq!(reply("(true,)\n"), Some(true));
        assert_eq!(reply("(false,)"), Some(false));
        // An error, nothing, or anything else is neither: the terminal asks, or it is a no
        assert_eq!(reply(""), None);
        assert_eq!(reply("Error: GDBus.Error:org.freedesktop.DBus.Error.ServiceUnknown"), None);
        assert_eq!(reply("(true, true)"), None);
        assert_eq!(reply("true"), None);
    }
}
