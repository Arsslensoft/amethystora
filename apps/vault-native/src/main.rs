//! What the browser starts for the Vault's extension, and how a person lets it:
//!
//!   amethystora-vault-native MANIFEST ID          Firefox starts it so
//!   amethystora-vault-native chrome-extension://ID/   the Chromium family so
//!   amethystora-vault-native browser on|off|status    behind `ame vault browser`
//!   amethystora-vault-native browser manifest firefox|chromium   what `on` writes, for the watcher
//!
//! It starts only when the browser starts it, refuses any extension but ours, sleeps on its pipe, and exits when the browser closes.

use std::fs::File;
use std::os::fd::FromRawFd;
use std::path::PathBuf;
use std::process::ExitCode;

use amethystora_vault_native::host::{self, Places};
use amethystora_vault_native::install;

#[path = "alloc.rs"]
mod wiping;

#[global_allocator]
static ALLOCATOR: wiping::Wiping = wiping::Wiping;

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).filter(|home| home.is_absolute())
}

fn browser(arguments: &[String]) -> ExitCode {
    let Some(home) = home() else {
        eprintln!("amethystora-vault-native: HOME is not set");
        return ExitCode::FAILURE;
    };
    let done = match arguments.first().map(String::as_str) {
        Some("on") if arguments.len() == 1 => install::on(&home),
        Some("off") if arguments.len() == 1 => install::off(&home),
        Some("status") if arguments.len() == 1 => {
            println!("{}", if install::is_on(&home) { "on" } else { "off" });
            Ok(())
        }
        Some("manifest") if arguments.len() == 2 && matches!(arguments[1].as_str(), "firefox" | "chromium") => {
            print!("{}", install::manifest(arguments[1] == "firefox"));
            Ok(())
        }
        _ => {
            eprintln!("usage: amethystora-vault-native browser on|off|status|manifest firefox|chromium");
            return ExitCode::from(64);
        }
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("amethystora-vault-native: {error}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.get(1).map(String::as_str) == Some("browser") {
        return browser(&arguments[2..]);
    }
    if !install::started_for_us(&arguments) {
        eprintln!("amethystora-vault-native: this is only started by a browser, for Amethystora Vault's extension");
        return ExitCode::from(64);
    }
    // A password passes through here: nothing of the account's reads it from this process's memory or a core file
    // SAFETY: a prctl with plain integers
    unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) };
    // Plain files, not std's stdin and stdout, whose buffers would keep the last password until the process ends
    // SAFETY: the descriptors the browser gave this process, which nothing else here uses
    let (mut input, mut output) = unsafe { (File::from_raw_fd(0), File::from_raw_fd(1)) };
    match host::serve(&mut input, &mut output, &Places::machine()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
