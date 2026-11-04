//! Running programs, the way main.js did with execFile and spawn: always an argument vector, never a shell.
//!
//! Everything here runs on the GLib main loop (gio::Subprocess), so a window never waits on a program.

use std::cell::RefCell;
use std::ffi::OsStr;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

use gtk::gio;
use gtk::prelude::*;
use gtk::glib;

/// How a program ended: never an error, since each caller has something to say about a program that is
/// missing or failed. `code` is the exit status, or -1 when the program could not start, was killed by a
/// signal or ran out of time, as execFile's callback had it.
#[derive(Debug, Clone, Default)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub fn ok(&self) -> bool {
        self.code == 0
    }

    /// The last line of what it said on stderr, for a message that falls back to one of the app's own
    pub fn last_error_line(&self) -> String {
        self.stderr.trim().lines().last().unwrap_or("").to_string()
    }
}

/// What to run a program with
#[derive(Debug, Clone)]
pub struct Options {
    pub timeout: Duration,
    /// Written to its stdin, which is closed afterwards; without it stdin is empty
    pub stdin: Option<String>,
    /// Variables set for it on top of the window's own
    pub env: Vec<(String, String)>,
}

impl Default for Options {
    fn default() -> Self {
        Options { timeout: Duration::from_secs(120), stdin: None, env: Vec::new() }
    }
}

/// Runs a program to its end and gives what it printed: execFile with a 120-second timeout
pub async fn run(program: &str, args: &[&str]) -> Output {
    run_with(program, args, Options::default()).await
}

pub async fn run_owned(program: &str, args: &[String]) -> Output {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run(program, &args).await
}

pub async fn run_with(program: &str, args: &[&str], options: Options) -> Output {
    let mut argv: Vec<&OsStr> = vec![OsStr::new(program)];
    argv.extend(args.iter().map(OsStr::new));
    let launcher = gio::SubprocessLauncher::new(
        gio::SubprocessFlags::STDIN_PIPE | gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE,
    );
    for (name, value) in &options.env {
        launcher.setenv(name, value, true);
    }
    let process = match launcher.spawn(&argv) {
        Ok(process) => process,
        Err(_) => return Output { code: -1, ..Default::default() },
    };
    // An empty stdin unless there is something to write: GLib wants the bytes whenever stdin is a pipe
    let input = glib::Bytes::from_owned(options.stdin.unwrap_or_default().into_bytes());
    let communicate = process.communicate_future(Some(&input));
    let (stdout, stderr, timed_out) = match glib::future_with_timeout(options.timeout, communicate).await {
        Ok(Ok((stdout, stderr))) => (stdout, stderr, false),
        Ok(Err(_)) => (None, None, false),
        Err(_) => {
            process.force_exit();
            (None, None, true)
        }
    };
    let text = |bytes: Option<glib::Bytes>| bytes.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let code = if timed_out {
        -1
    } else {
        // communicate has waited for it to end
        if process.has_exited() { process.exit_status() } else { -1 }
    };
    Output { code, stdout: text(stdout), stderr: text(stderr) }
}

/// A program that outlives the window, such as the Manual or a terminal: started in a process group of its
/// own, with nothing to read from or write to, and never waited on beyond being reaped
pub fn launch(program: &str, args: &[&str]) {
    use std::os::unix::process::CommandExt;
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
    if let Ok(child) = child {
        let pid = glib::Pid(child.id() as i32);
        // GLib reaps it when it ends, whether or not the window is still open then
        glib::child_watch_add_local(pid, |_, _| {});
    }
}

pub fn launch_owned(program: &str, args: &[String]) {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    launch(program, &args);
}

/// A command in a terminal of its own, for one that asks something, explains a risk or needs a password
pub const IN_TERMINAL: &str = "/usr/libexec/amethystora-in-terminal";

pub fn in_terminal(args: &[&str]) {
    launch(IN_TERMINAL, args);
}

/// The same, with `ended` called on the window's thread the moment the command has finished, while the terminal still shows what
/// it printed, so that a window shows what it changed at once. The wrapper's `--done` writes only the exit status, into a folder of
/// the account's own in the runtime folder, which a monitor watches: no timer. A terminal closed before the command ended, or one
/// that never opened, calls it too.
pub fn in_terminal_then(args: &[&str], ended: impl Fn() + 'static) {
    use std::os::unix::fs::DirBuilderExt;
    use std::os::unix::process::CommandExt;
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let parent = glib::user_runtime_dir().join("amethystora");
    let dir = parent.join(format!("ended-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let made = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&parent).and_then(|_| std::fs::DirBuilder::new().mode(0o700).create(&dir));
    let monitor = made.ok().and_then(|_| gio::File::for_path(&dir).monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE).ok());
    let Some(monitor) = monitor else {
        in_terminal(args);
        return;
    };
    let ended: Rc<dyn Fn()> = Rc::new(ended);
    let status = dir.join("status");
    let kept = Rc::new(RefCell::new(Some(monitor.clone())));
    // Once the status is there: said once, and the folder and its monitor gone
    let finish: Rc<dyn Fn() -> bool> = {
        let (kept, ended, dir) = (kept.clone(), ended.clone(), dir.clone());
        Rc::new(move || {
            if !status.exists() {
                return false;
            }
            if let Some(monitor) = kept.take() {
                monitor.cancel();
                let _ = std::fs::remove_dir_all(&dir);
                ended();
            }
            true
        })
    };
    let on_change = finish.clone();
    monitor.connect_changed(move |_, _, _, _| {
        on_change();
    });
    let mut argv = vec!["--done", dir.to_str().unwrap_or_default(), "--"];
    argv.extend_from_slice(args);
    let child = Command::new(IN_TERMINAL).args(&argv).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn();
    let Ok(child) = child else {
        kept.take();
        let _ = std::fs::remove_dir_all(&dir);
        return;
    };
    glib::child_watch_add_local(glib::Pid(child.id() as i32), move |_, _| {
        // The terminal has gone: the command ended with it or before it, and the screen is read again either way
        if !finish() && kept.take().is_some() {
            let _ = std::fs::remove_dir_all(&dir);
            ended();
        }
    });
}

/// A page of the Manual, `page` or `page#heading`, and nothing else
pub fn manual(page: &str) {
    if valid_manual_page(page) {
        launch("amethystora-manual", &[page]);
    }
}

/// main.js's /^[a-z0-9-]+(#[a-z0-9-]+)?$/, and never starting with a dash, which the Manual would take
/// for an option
pub fn valid_manual_page(page: &str) -> bool {
    let word = |part: &str| {
        !part.is_empty() && !part.starts_with('-') && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };
    match page.split_once('#') {
        Some((page, heading)) => word(page) && word(heading),
        None => word(page),
    }
}

/// A web address in the browser, through the portal, which knows the account's own choice of browser
pub fn open_uri(window: Option<&gtk::Window>, uri: &str) {
    gtk::UriLauncher::new(uri).launch(window, gio::Cancellable::NONE, |_| {});
}

/// The lines a program prints as it prints them, such as journalctl --follow; stopped when this is dropped
/// or stop() is called
pub struct Follow {
    process: gio::Subprocess,
    stopped: Rc<RefCell<bool>>,
}

impl Follow {
    pub fn stop(&self) {
        *self.stopped.borrow_mut() = true;
        self.process.send_signal(libc::SIGTERM);
    }
}

impl Drop for Follow {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn follow(program: &str, args: &[&str], on_line: impl Fn(&str) + 'static) -> Option<Follow> {
    let mut argv: Vec<&OsStr> = vec![OsStr::new(program)];
    argv.extend(args.iter().map(OsStr::new));
    let process = gio::Subprocess::newv(&argv, gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_SILENCE).ok()?;
    let stdout = process.stdout_pipe()?;
    let stopped = Rc::new(RefCell::new(false));
    let reading = stopped.clone();
    glib::spawn_future_local(async move {
        // Read as bytes and split here, so that a line that is not UTF-8 is shown with replacement
        // characters rather than ending the reading
        let mut pending: Vec<u8> = Vec::new();
        loop {
            let chunk = match stdout.read_bytes_future(16384, glib::Priority::DEFAULT).await {
                Ok(chunk) if !chunk.is_empty() => chunk,
                _ => break,
            };
            if *reading.borrow() {
                break;
            }
            pending.extend_from_slice(&chunk);
            while let Some(end) = pending.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = pending.drain(..=end).collect();
                on_line(String::from_utf8_lossy(&line[..line.len() - 1]).trim_end_matches('\r'));
                if *reading.borrow() {
                    return;
                }
            }
        }
        if !pending.is_empty() && !*reading.borrow() {
            on_line(&String::from_utf8_lossy(&pending));
        }
    });
    process.wait_async(gio::Cancellable::NONE, |_| {});
    Some(Follow { process, stopped })
}

/// The text on the clipboard
pub fn copy(text: &str) {
    if let Some(display) = gtk::gdk::Display::default() {
        display.clipboard().set_text(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_pages_are_words() {
        assert!(valid_manual_page("updates"));
        assert!(valid_manual_page("updates#rolling-back"));
        assert!(!valid_manual_page("updates#"));
        assert!(!valid_manual_page("../etc"));
        assert!(!valid_manual_page("Updates"));
        assert!(!valid_manual_page("-x"));
    }
}
