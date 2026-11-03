//! What the tests against real verifiers share. A verifier that is not installed skips its test with a line saying so,
//! so that a build stage without it still builds, unless VAULT_VERIFIERS_REQUIRED is set: then it fails, so that a run that
//! claims to have checked something has.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

pub fn need(program: &str) -> bool {
    let found = Command::new("sh").arg("-c").arg(format!("command -v {program}")).output().is_ok_and(|out| out.status.success());
    if !found {
        assert!(std::env::var_os("VAULT_VERIFIERS_REQUIRED").is_none(), "{program} is not installed");
        eprintln!("SKIPPED: {program} is not installed");
    }
    found
}

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A folder of its own for one test, gone at its end
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("amethystora-vault-{}-{name}-{number}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    pub fn dir(&self) -> &Path {
        &self.0
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    pub fn write(&self, name: &str, data: &[u8]) -> PathBuf {
        let path = self.path(name);
        std::fs::write(&path, data).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn describe(command: &Command) -> String {
    format!("{:?}", command)
}

/// Runs it, and fails the test with what it said unless it succeeded
pub fn run(command: &mut Command) -> String {
    let out = command.output().unwrap_or_else(|error| panic!("{}: {error}", describe(command)));
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{}\n{text}", describe(command));
    text
}

pub fn run_in(command: &mut Command, dir: &Path) -> String {
    run(command.current_dir(dir))
}

/// Standard output only
pub fn output(command: &mut Command) -> String {
    let out = command.output().unwrap_or_else(|error| panic!("{}: {error}", describe(command)));
    assert!(out.status.success(), "{}\n{}", describe(command), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Whether it succeeded, and everything it said
pub fn attempt(command: &mut Command) -> (bool, String) {
    let out = command.output().unwrap_or_else(|error| panic!("{}: {error}", describe(command)));
    (out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}
