//! A security key, through libfido2's own tools, as Notes speaks to one: what a tool needs as lines on its standard input and, with
//! no terminal to ask on, the PIN too, once it asks. Off the window's thread, since a tool waits for a touch. A credential is made
//! with protection level 3, and the secret asked for with the PIN or a fingerprint: a touch alone must never open a vault.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

use amethystora_ui::t;
use base64::Engine;
use zeroize::Zeroizing;

use crate::rpc;

pub const RELYING_PARTY: &str = "amethystora-vault";

/// What went wrong, in plain words: the first the tool's message matches
fn plain_words(message: &str) -> String {
    let has = |word: &str| message.contains(word);
    if has("PIN_INVALID") {
        t!("That is not the PIN of this security key.")
    } else if has("PIN_AUTH_BLOCKED") {
        t!("Too many wrong PINs. Unplug the security key, plug it back in and try again.")
    } else if has("PIN_BLOCKED") {
        t!("The PIN of this security key is blocked, which only resetting the key undoes.")
    } else if has("PIN_REQUIRED") || has("PIN_NOT_SET") || has("invalid PIN length") {
        t!("Enter the PIN of the security key: 4 to 63 characters.")
    } else if has("UV_INVALID") || has("UV_BLOCKED") {
        t!("The security key did not recognise the fingerprint.")
    } else if has("ACTION_TIMEOUT") || has("KEEPALIVE_CANCEL") {
        t!("The security key was not touched in time.")
    } else if has("NO_CREDENTIALS") {
        t!("This security key is not one that opens a vault here.")
    } else {
        t!("The security key could not do that.")
    }
}

/// Blocks: one tool, to its end
fn tool(program: &str, args: &[&str], lines: &[String], pin: Option<&str>) -> Result<Vec<String>, String> {
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    // SAFETY: setsid is async-signal-safe. In a session of its own, a tool finds no terminal to ask for the PIN on
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut child = command.spawn().map_err(|_| t!("The security key's tools are not installed."))?;
    let mut stdin = child.stdin.take();
    if let Some(stdin) = &mut stdin {
        let input = Zeroizing::new(lines.iter().map(|line| format!("{}\n", line.replace(['\r', '\n'], ""))).collect::<String>());
        let _ = stdin.write_all(input.as_bytes());
    }
    let mut said = String::new();
    if let Some(stderr) = child.stderr.take() {
        let mut reader = BufReader::new(stderr);
        let mut asked = false;
        let mut buffer = [0u8; 256];
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 {
                break;
            }
            said.push_str(&String::from_utf8_lossy(&buffer[..count]));
            if !asked && said.contains("Enter PIN") {
                asked = true;
                match (pin, &mut stdin) {
                    (Some(pin), Some(stdin)) => {
                        let line = Zeroizing::new(format!("{}\n", pin.replace(['\r', '\n'], "")));
                        let _ = stdin.write_all(line.as_bytes());
                    }
                    _ => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(String::from("PIN_REQUIRED"));
                    }
                }
                drop(stdin.take());
            }
        }
    }
    drop(stdin);
    let mut printed = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            printed.push(line);
        }
    }
    match child.wait() {
        Ok(status) if status.success() => Ok(printed),
        _ => Err(said),
    }
}

/// The one key plugged in
fn device() -> Result<String, String> {
    let lines = tool("fido2-token", &["-L"], &[], None)?;
    let devices: Vec<&String> = lines.iter().filter(|line| line.contains(": vendor=0x")).collect();
    match devices.as_slice() {
        [one] => Ok(one.split(": vendor=0x").next().unwrap_or_default().to_string()),
        [] => Err(t!("No security key is plugged in.")),
        _ => Err(t!("More than one security key is plugged in. Leave only the one you mean.")),
    }
}

fn random_base64(count: usize) -> String {
    let mut bytes = vec![0u8; count];
    amethystora_vault_core::seal::random(&mut bytes);
    rpc::base64_of(&bytes)
}

/// Blocks: the secret this key computes for this credential and salt
fn secret(device: &str, credential: &str, salt: &str, pin: Option<&str>) -> Result<Zeroizing<Vec<u8>>, String> {
    let lines = [random_base64(32), RELYING_PARTY.to_string(), credential.to_string(), salt.to_string()];
    let printed = tool("fido2-assert", &["-G", "-h", "-t", if pin.is_some() { "pin=true" } else { "uv=true" }, device], &lines, pin)?;
    base64::engine::general_purpose::STANDARD
        .decode(printed.get(4).map(String::as_str).unwrap_or(""))
        .map(Zeroizing::new)
        .map_err(|_| t!("The security key gave no secret."))
}

/// A slot's credential, its salt and what opens with it
pub struct Slot {
    pub id: String,
    pub credential: String,
    pub salt: String,
}

/// Blocks. Each slot's secret, tried in turn since a key that needs its PIN will not say which credential is its own. The first that
/// the key answers: its slot, and the secret. Whether it is the right vault is the service's to say.
pub fn answer(slots: &[Slot], pin: Option<&str>) -> Result<Vec<(String, Zeroizing<Vec<u8>>)>, String> {
    let device = device().map_err(|words| if words.contains("PIN") { plain_words(&words) } else { words })?;
    let mut found = Vec::new();
    let mut last = String::new();
    for slot in slots {
        match secret(&device, &slot.credential, &slot.salt, pin) {
            Ok(secret) => found.push((slot.id.clone(), secret)),
            Err(said) => {
                // The PIN wrong or wanted is the same for every slot
                if said.contains("PIN") {
                    return Err(plain_words(&said));
                }
                last = said;
            }
        }
    }
    if found.is_empty() {
        return Err(plain_words(&last));
    }
    Ok(found)
}

/// Whether what was said means the PIN should be asked for
pub fn wants_pin(words: &str) -> bool {
    words == t!("Enter the PIN of the security key: 4 to 63 characters.")
}

/// Blocks: a new credential on the key plugged in, and its secret. Refused on a key that cannot hold back a secret until it has
/// checked who is asking, as Notes refuses one.
pub fn enrol(pin: &str) -> Result<(String, String, Zeroizing<Vec<u8>>), String> {
    let device = device()?;
    let info = tool("fido2-token", &["-I", &device], &[], None).map_err(|said| plain_words(&said))?;
    let extensions = info.iter().find(|line| line.starts_with("extension strings: ")).cloned().unwrap_or_default();
    if !extensions.contains("hmac-secret") || !extensions.contains("credProtect") {
        return Err(t!("This security key cannot keep a secret until it has checked who is asking, so it cannot open a vault."));
    }
    // SAFETY: getuid cannot fail
    let user = unsafe { libc::getuid() }.to_string();
    let lines = [random_base64(32), RELYING_PARTY.to_string(), user, random_base64(16)];
    let made = tool("fido2-cred", &["-M", "-h", "-c", "3", &device], &lines, Some(pin)).map_err(|said| plain_words(&said))?;
    let credential = made.get(4).cloned().ok_or_else(|| t!("The security key gave no secret."))?;
    let salt = random_base64(32);
    let answer = secret(&device, &credential, &salt, Some(pin)).map_err(|said| plain_words(&said))?;
    Ok((credential, salt, answer))
}
