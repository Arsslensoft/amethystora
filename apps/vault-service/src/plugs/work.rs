//! One job in `amethystora-vault-work` (`src/work.rs`): everything that takes libcrypto, in a process of its own that lives only
//! for that job. libcrypto and its providers cost more than this service's whole budget, and this keeps their parsers out of
//! the process that holds every vault's key. It runs on the work thread, one at a time, so a job waits for the last to end.
//! Its environment is cleared, it inherits no descriptor but the two pipes, and a secret goes to it only on its standard input.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{json, Value};
use zeroize::Zeroizing;

/// More than any answer the socket could carry
const MOST: u64 = 16 * 1024 * 1024;

/// The job's answer, `{"ok":true,...}`, or a refusal: `{"ok":false,"why":...}`, with `problem` and `field` where a request was
/// the trouble
pub fn run(program: &Path, request: &Value) -> Value {
    let answer = attempt(program, request).unwrap_or_else(|| json!({"ok": false, "why": "failed"}));
    // What a key or a certificate took, and its answer, goes back to the system: the service is held to 8 MB for hours
    // SAFETY: malloc_trim only releases free memory, and is safe to call at any time
    unsafe { libc::malloc_trim(0) };
    answer
}

fn attempt(program: &Path, request: &Value) -> Option<Value> {
    let mut child = Command::new(program).env_clear().stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let answer = {
        let mut stdin = child.stdin.take()?;
        let mut stdout = child.stdout.take()?;
        let line = Zeroizing::new(request.to_string());
        if stdin.write_all(line.as_bytes()).is_err() {
            None
        } else {
            drop(stdin);
            let mut bytes = Zeroizing::new(Vec::new());
            stdout.by_ref().take(MOST).read_to_end(&mut bytes).ok().map(|_| bytes)
        }
    };
    let finished = child.wait().ok()?;
    if !finished.success() {
        return None;
    }
    serde_json::from_slice(&answer?).ok()
}

/// A job's answer as what to say next, if it did not go well
pub fn refused(answer: &Value) -> bool {
    answer["ok"] != json!(true)
}
