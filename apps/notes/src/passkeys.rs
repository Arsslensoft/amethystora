//! Passkeys, as main.js had them. A passkey is a credential made on a FIDO2 security key with the hmac-secret
//! extension. Asked with the salt kept beside it in vault.json, and only once its PIN or a fingerprint has
//! been checked, the key answers with 32 bytes that nothing else can compute, and the data key is sealed
//! under those. A passkey opens the notes beside the passphrase, never in its place.
//!
//! The key is spoken to through libfido2's own tools (fido2-tools), which take what they need as lines on
//! stdin and, with no terminal to ask on, ask for the PIN on stderr and read it from stdin too. Each runs in
//! a session of its own, so that it never finds a terminal to ask on. Nothing is stored on the key: the
//! credential it returns is all there is of it, and it stays in vault.json, which no backup carries.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use serde_json::{json, Value};

use amethystora_ui::{gio, glib, t};
use gio::prelude::*;

use crate::backend::{self, Shared};
use crate::data::{self, Fields};
use crate::vault::{self, Key};

pub const RELYING_PARTY: &str = "amethystora-notes";

/// What went wrong, said in plain words: the first pattern the tool's message matches
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
        t!("This security key is not one that opens your notes.")
    } else if has("aborted") {
        t!("Cancelled.")
    } else {
        message.to_string()
    }
}

fn clean(text: &str) -> String {
    text.replace(['\r', '\n'], "")
}

/// Where a tool is: the tests' stand-in folder, else /usr/bin, never a search of the PATH
fn program_path(tools: &Option<PathBuf>, tool: &str) -> String {
    match tools {
        Some(dir) => dir.join(tool).to_string_lossy().into_owned(),
        None => format!("/usr/bin/{tool}"),
    }
}

/// The lines one of the tools prints, given these on stdin; the PIN, when it asks for one. Fails with what
/// the tool said.
pub async fn fido(tools: Option<PathBuf>, tool: &str, args: &[&str], lines: &[String], pin: &str, cancel: Option<gio::Cancellable>) -> Result<Vec<String>, String> {
    let program = program_path(&tools, tool);
    let mut argv: Vec<&std::ffi::OsStr> = vec![std::ffi::OsStr::new(&program)];
    argv.extend(args.iter().map(std::ffi::OsStr::new));
    let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::STDIN_PIPE | gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE);
    // In a session of its own: with a terminal, the tool would ask for the PIN there
    launcher.set_child_setup(|| {
        // SAFETY: setsid is async-signal-safe, and this runs in the child between fork and exec
        unsafe {
            libc::setsid();
        }
    });
    let process = launcher.spawn(&argv).map_err(|_| format!("spawn {tool} ENOENT"))?;
    // Cancelling kills it, as the AbortController did
    let cancelled = cancel.as_ref().and_then(|cancel| {
        let process = process.clone();
        cancel.connect_cancelled_local(move |_| process.send_signal(libc::SIGTERM)).map(|id| (cancel.clone(), id))
    });
    let stdin = process.stdin_pipe();
    let printed = Rc::new(RefCell::new(Vec::<u8>::new()));
    let said = Rc::new(RefCell::new(String::new()));
    let input = lines.iter().map(|line| format!("{}\n", clean(line))).collect::<String>();
    if let Some(stdin) = &stdin {
        let _ = stdin.write_all_future(input.into_bytes(), glib::Priority::DEFAULT).await;
    }
    let (out_done, out_finished) = crate::sync::oneshot::<()>();
    match process.stdout_pipe() {
        Some(stdout) => {
            let printed = printed.clone();
            glib::spawn_future_local(async move {
                while let Ok(chunk) = stdout.read_bytes_future(4096, glib::Priority::DEFAULT).await {
                    if chunk.is_empty() {
                        break;
                    }
                    printed.borrow_mut().extend_from_slice(&chunk);
                }
                out_done.send(());
            });
        }
        None => out_done.send(()),
    }
    let (err_done, err_finished) = crate::sync::oneshot::<()>();
    match process.stderr_pipe() {
        Some(stderr) => {
            let (said, process, stdin, pin) = (said.clone(), process.clone(), stdin.clone(), pin.to_string());
            glib::spawn_future_local(async move {
                let mut asked = false;
                while let Ok(chunk) = stderr.read_bytes_future(4096, glib::Priority::DEFAULT).await {
                    if chunk.is_empty() {
                        break;
                    }
                    said.borrow_mut().push_str(&String::from_utf8_lossy(&chunk));
                    if !asked && said.borrow().contains("Enter PIN") {
                        asked = true;
                        if pin.is_empty() {
                            *said.borrow_mut() = "PIN_REQUIRED".into();
                            process.send_signal(libc::SIGTERM);
                        } else if let Some(stdin) = &stdin {
                            let _ = stdin.write_all_future(format!("{}\n", clean(&pin)).into_bytes(), glib::Priority::DEFAULT).await;
                            let _ = stdin.close_future(glib::Priority::DEFAULT).await;
                        }
                    }
                }
                err_done.send(());
            });
        }
        None => err_done.send(()),
    }
    // Two minutes at most, as the window gave each one
    if glib::future_with_timeout(Duration::from_secs(120), process.wait_future()).await.is_err() {
        process.send_signal(libc::SIGTERM);
        let _ = process.wait_future().await;
    }
    // What it printed last, still on its way through the pipes
    let _ = glib::future_with_timeout(Duration::from_secs(2), out_finished).await;
    let _ = glib::future_with_timeout(Duration::from_secs(2), err_finished).await;
    if let Some((cancel, id)) = cancelled {
        cancel.disconnect_cancelled(id);
        if cancel.is_cancelled() {
            return Err("The operation was aborted".into());
        }
    }
    let code = if process.has_exited() { process.exit_status() } else { -1 };
    if code == 0 {
        let text = String::from_utf8_lossy(&printed.borrow()).into_owned();
        return Ok(text.split('\n').map(str::to_string).collect());
    }
    let said = said.borrow().trim().to_string();
    Err(if said.is_empty() { t!("{tool} failed", tool = tool) } else { said })
}

/// A run of one of the tools, which cancelling the passkey (touching) kills
async fn run(this: &Shared, tool: &str, args: &[&str], lines: &[String], pin: &str) -> Result<Vec<String>, String> {
    let (tools, cancel) = {
        let backend = this.borrow();
        (backend.tools.clone(), backend.touching.clone())
    };
    fido(tools, tool, args, lines, pin, cancel).await
}

/// One passkey operation at a time, with what went wrong said in plain words
pub async fn with_key<T>(this: &Shared, work: impl AsyncFnOnce() -> Result<T, String>) -> Result<T, String> {
    if this.borrow().touching.is_some() {
        return Err(t!("The security key is busy. Try again in a moment."));
    }
    this.borrow_mut().touching = Some(gio::Cancellable::new());
    let result = work().await;
    this.borrow_mut().touching = None;
    result.map_err(|error| plain_words(&error))
}

/// The tool waiting for the key to be touched, stopped (passkey-cancel)
pub fn cancel(this: &Shared) {
    if let Some(cancel) = this.borrow().touching.clone() {
        cancel.cancel();
    }
}

/// A security key plugged in: where it is, and what it calls itself
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub device: String,
    pub name: String,
}

/// /^(.+?): vendor=0x[0-9a-f]+, product=0x[0-9a-f]+ \((.*)\)$/
pub fn device_line(line: &str) -> Option<Device> {
    let at = line.find(": vendor=0x")?;
    let device = &line[..at];
    if device.is_empty() {
        return None;
    }
    let rest = &line[at + ": vendor=0x".len()..];
    let hex_run = |s: &str| s.chars().take_while(|c| c.is_ascii_digit() || ('a'..='f').contains(c)).count();
    let vendor = hex_run(rest);
    let rest = rest[vendor..].strip_prefix(", product=0x").filter(|_| vendor > 0)?;
    let product = hex_run(rest);
    let rest = rest[product..].strip_prefix(" (").filter(|_| product > 0)?;
    let name = rest.strip_suffix(')')?;
    let name = name.trim();
    Some(Device { device: device.to_string(), name: if name.is_empty() { t!("Security key") } else { name.to_string() } })
}

/// The security keys plugged in
async fn devices(this: &Shared) -> Result<Vec<Device>, String> {
    Ok(run(this, "fido2-token", &["-L"], &[], "").await?.iter().filter_map(|line| device_line(line)).collect())
}

/// Whether the key keeps secrets for apps and can refuse a passkey without its PIN or a fingerprint, has
/// a PIN, and has a fingerprint enrolled
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Info {
    pub secrets: bool,
    pub protect: bool,
    pub pin: bool,
    pub uv: bool,
}

pub fn info_lines(lines: &[String]) -> Info {
    let list = |label: &str| -> Vec<String> {
        let prefix = format!("{label}: ");
        lines.iter().find(|line| line.starts_with(&prefix)).map(|line| line[prefix.len()..].split(", ").map(str::to_string).collect()).unwrap_or_default()
    };
    let options = list("options");
    let extensions = list("extension strings");
    let has = |list: &[String], word: &str| list.iter().any(|item| item == word);
    Info { secrets: has(&extensions, "hmac-secret"), protect: has(&extensions, "credProtect"), pin: has(&options, "clientPin"), uv: has(&options, "uv") }
}

async fn info(this: &Shared, device: &str) -> Result<Info, String> {
    Ok(info_lines(&run(this, "fido2-token", &["-I", device], &[], "").await?))
}

fn random_base64(count: usize) -> String {
    let mut bytes = vec![0u8; count];
    vault::random(&mut bytes);
    vault::base64(&bytes)
}

/// The secret the key computes for this passkey once it is touched: after its PIN, or, given none, after
/// the fingerprint it reads itself. Either way the key has checked who is asking, and the secret is the
/// same; a touch alone would get a different one.
async fn secret(this: &Shared, device: &str, credential: &str, salt: &str, pin: &str) -> Result<Key, String> {
    let lines = [random_base64(32), RELYING_PARTY.to_string(), credential.to_string(), salt.to_string()];
    let printed = run(this, "fido2-assert", &["-G", "-h", "-t", if pin.is_empty() { "uv=true" } else { "pin=true" }, device], &lines, pin).await?;
    let bytes = vault::unbase64(printed.get(4).map(String::as_str).unwrap_or(""));
    Key::from_slice(&bytes).ok_or_else(|| t!("The security key gave no secret."))
}

/// How opening with a passkey went: the notes open, or the PIN of a key that reads no fingerprint wanted
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unlocked {
    Open,
    NeedsPin,
}

/// With a passkey. A passkey made to need the PIN or a fingerprint is not even admitted to without one, so
/// the key cannot be asked first which passkey is its own: each is tried in turn, and the key turns down
/// the ones it did not make.
pub async fn unlock(this: &Shared, pin: String) -> Result<Unlocked, String> {
    if this.borrow().store.is_some() {
        return Ok(Unlocked::Open);
    }
    let found = devices(this).await?;
    if found.len() > 1 {
        return Err(t!("Leave only the security key that opens your notes plugged in."));
    }
    let passkeys: Vec<Value> = this.borrow().vault.as_ref().and_then(|v| v.get("passkeys")).and_then(Value::as_array).cloned().unwrap_or_default();
    let Some(device) = found.first().map(|d| d.device.clone()).filter(|_| !passkeys.is_empty()) else {
        return Err(t!("Plug in a security key that opens your notes."));
    };
    if pin.is_empty() && !info(this, &device).await?.uv {
        return Ok(Unlocked::NeedsPin);
    }
    for passkey in &passkeys {
        let secret = match secret(this, &device, passkey.str("credential"), passkey.str("salt"), &pin).await {
            Ok(secret) => secret,
            Err(error) if error.contains("NO_CREDENTIALS") => continue,
            Err(error) => return Err(error),
        };
        let sealed = vault::unbase64(passkey.str("key"));
        let opening = vault::unseal(&secret, &sealed, &format!("passkey:{}", passkey.str("id"))).ok().and_then(|bytes| Key::from_slice(&bytes));
        drop(secret);
        let Some(opening) = opening else {
            return Err(t!("This passkey no longer opens your notes. Use your passphrase."));
        };
        backend::open_with_key(this, opening)?;
        return Ok(Unlocked::Open);
    }
    Err(t!("This security key is not one that opens your notes."))
}

/// The account's name, as os.userInfo().username
fn username() -> String {
    // SAFETY: getpwuid's answer is read at once, on the one thread that asks
    unsafe {
        let entry = libc::getpwuid(libc::getuid());
        if !entry.is_null() && !(*entry).pw_name.is_null() {
            return std::ffi::CStr::from_ptr((*entry).pw_name).to_string_lossy().into_owned();
        }
    }
    std::env::var("USER").unwrap_or_default()
}

/// Seals the data key once more, under the secret of the one security key that is plugged in
pub async fn add(this: &Shared, current: String, pin: String) -> Result<(), String> {
    if this.borrow().store.is_none() || !this.borrow().encrypted() {
        return Err(t!("Unlock your notes first."));
    }
    let sealed = this.borrow().vault.clone().unwrap_or(Value::Null);
    if backend::unwrap(sealed, current).await?.is_none() {
        return Err(t!("That is not the current passphrase."));
    }
    let found = devices(this).await?;
    if found.len() != 1 {
        return Err(if found.is_empty() { t!("Plug in your security key.") } else { t!("Leave only the security key you are adding plugged in.") });
    }
    let Device { device, name } = found.into_iter().next().expect("one");
    let info = info(this, &device).await?;
    if !info.secrets {
        return Err(t!("This security key cannot keep a secret for an app (FIDO2 hmac-secret), so it cannot open your notes."));
    }
    // Without it, a key of the first FIDO2 generation may give the same secret for a touch alone
    if !info.protect {
        return Err(t!("This security key cannot be told to refuse a passkey without its PIN or a fingerprint (FIDO2 credProtect), so it cannot open your notes."));
    }
    if !info.pin && !info.uv {
        return Err(t!("This security key has no PIN yet. Set one in Firefox at {page}, then add it here.", page = "about:webauthn"));
    }
    if pin.is_empty() && !info.uv {
        return Err(t!("Enter the PIN of the security key."));
    }
    // Nothing is stored on the key: the credential it returns is all there is of it, and it stays here.
    // Protection level 3 has the key refuse it without the PIN or a fingerprint.
    let mut args = vec!["-M", "-h", "-c", "3"];
    if pin.is_empty() {
        args.push("-v");
    }
    args.push(&device);
    let lines = [random_base64(32), RELYING_PARTY.to_string(), username(), random_base64(16)];
    let made = run(this, "fido2-cred", &args, &lines, &pin).await?;
    let id = data::new_id();
    let credential = made.get(4).cloned();
    let salt = random_base64(32);
    let secret = secret(this, &device, credential.as_deref().unwrap_or(""), &salt, &pin).await?;
    let sealed_key = {
        let backend = this.borrow();
        let Some(key) = backend.key.as_ref() else { return Err(t!("Your notes locked before the passkey was added.")) };
        vault::base64(&vault::seal(&secret, key.bytes(), &format!("passkey:{id}")))
    };
    drop(secret);
    let mut passkey = json!({ "id": id, "name": name, "created": data::ms(data::now_ms()), "protect": 3, "salt": salt, "key": sealed_key });
    if let Some(credential) = credential {
        data::set(&mut passkey, "credential", credential);
    }
    let mut changed = this.borrow().vault.clone().unwrap_or(Value::Null);
    let mut list = changed.get("passkeys").and_then(Value::as_array).cloned().unwrap_or_default();
    list.push(passkey);
    data::set(&mut changed, "passkeys", Value::Array(list));
    let paths = this.borrow().paths.clone();
    vault::write_atomic(&paths.vault, vault::pretty(&changed).as_bytes(), false).map_err(|e| e.to_string())?;
    this.borrow_mut().vault = Some(changed);
    Ok(())
}

pub fn remove(this: &Shared, id: &str) -> Result<(), String> {
    let mut changed = {
        let backend = this.borrow();
        let has = backend.vault.as_ref().and_then(|v| v.get("passkeys")).and_then(Value::as_array).is_some_and(|list| list.iter().any(|p| p.str("id") == id));
        if backend.store.is_none() || !has {
            return Err(t!("Nothing to change."));
        }
        backend.vault.clone().unwrap_or(Value::Null)
    };
    let list: Vec<Value> = changed.get("passkeys").and_then(Value::as_array).cloned().unwrap_or_default().into_iter().filter(|p| p.str("id") != id).collect();
    data::set(&mut changed, "passkeys", Value::Array(list));
    let paths = this.borrow().paths.clone();
    vault::write_atomic(&paths.vault, vault::pretty(&changed).as_bytes(), false).map_err(|e| e.to_string())?;
    this.borrow_mut().vault = Some(changed);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_the_tools_print() {
        assert_eq!(
            device_line("/dev/hidraw3: vendor=0x1050, product=0x0407 (Yubico YubiKey OTP+FIDO+CCID)"),
            Some(Device { device: "/dev/hidraw3".into(), name: "Yubico YubiKey OTP+FIDO+CCID".into() })
        );
        assert_eq!(device_line("/dev/hidraw3: vendor=0x1050, product=0x0407 ( )").map(|d| d.name), Some("Security key".into()));
        assert_eq!(device_line("garbage"), None);
        let info = info_lines(&["options: rk, up, noplat, clientPin, credMgmt".into(), "extension strings: credProtect, hmac-secret".into()]);
        assert_eq!(info, Info { secrets: true, protect: true, pin: true, uv: false });
        assert_eq!(plain_words("FIDO_ERR_PIN_AUTH_BLOCKED"), t!("Too many wrong PINs. Unplug the security key, plug it back in and try again."));
        assert_eq!(plain_words("fido2-assert: fido_dev_get_assert: FIDO_ERR_NO_CREDENTIALS"), t!("This security key is not one that opens your notes."));
        assert_eq!(plain_words("something else"), "something else");
    }

    #[test]
    fn the_tools_are_never_found_through_the_path() {
        for tool in ["fido2-token", "fido2-cred", "fido2-assert"] {
            assert_eq!(program_path(&None, tool), format!("/usr/bin/{tool}"));
        }
        assert_eq!(program_path(&Some(PathBuf::from("/x")), "fido2-token"), PathBuf::from("/x").join("fido2-token").to_string_lossy());
    }
}
