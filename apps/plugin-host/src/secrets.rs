//! A plugin's settings (plans/PLAN-ui-ideas-r7-build.md, "Changes to other plans"): the values a person gave it, declared in
//! its manifest. Every one is kept sealed under a key of the plugin's own in the Vault (they are what the person named or did: a
//! tracking number, an address, a token), so that none is a file in the clear, an argument or an environment variable. Only an
//! account with no Vault at all (`not-set-up`) keeps a plain setting in a plain file, and never a secret one; a closed Vault gives
//! neither. They reach the sandbox the same way: one JSON object on a pipe, descriptor 10, which the plugin reads once (`read -r -u
//! 10 settings`). A closed Vault means the plugin waits and says so.

use std::collections::BTreeMap;
use std::path::PathBuf;

use amethystora_vault_client::{Refusal, open_at, seal_at};
use sha2::{Digest, Sha256};

use crate::host::Env;

type Settings = BTreeMap<String, String>;

/// Why a plugin's settings could not be read: the Vault is closed, or there is none
pub fn why(r: Refusal) -> String {
    match r {
        Refusal::Closed => "vault-closed: its secret settings are in the Vault, which is closed".into(),
        Refusal::NotSetUp => "vault-closed: its secret settings need a Vault, and this account has none".into(),
        _ => "vault-closed: the Vault would not open its secret settings".into(),
    }
}

fn purpose(id: &str) -> String {
    let digest = Sha256::digest(id.as_bytes());
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("plugin-{hex}")
}

fn sealed_file(env: &Env, id: &str) -> PathBuf {
    env.state_root.join(format!("{id}.secrets"))
}

fn plain_file(env: &Env, id: &str) -> PathBuf {
    env.state_root.join(format!("{id}.settings.json"))
}

fn write(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let temp = path.with_extension("new");
    let _ = std::fs::remove_file(&temp);
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temp).map_err(|e| e.to_string())?;
    file.write_all(bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, path).map_err(|e| e.to_string())
}

fn read_sealed(env: &Env, id: &str) -> Result<Settings, String> {
    let Ok(sealed) = std::fs::read(sealed_file(env, id)) else { return Ok(Settings::new()) };
    let plain = open_at(&env.vault_socket, &purpose(id), id, &sealed).map_err(why)?;
    serde_json::from_slice(&plain).map_err(|_| "its secret settings could not be read".into())
}

fn read_plain(env: &Env, id: &str) -> Settings {
    std::fs::read(plain_file(env, id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Every setting the plugin holds, for the pipe: the plain ones, and the secret ones if the Vault opens
pub fn all(env: &Env, id: &str) -> Result<Settings, String> {
    let mut all = read_plain(env, id);
    all.extend(read_sealed(env, id)?);
    Ok(all)
}

/// Whether the plugin holds any secret: only then does a run need the Vault
pub fn has_secrets(env: &Env, id: &str) -> bool {
    sealed_file(env, id).exists()
}

/// Gives one setting a value. Everything goes into the Vault, and a plain file the account had before is taken into it. Only an account with no
/// Vault (not-set-up) keeps a non-secret setting in a plain file: a secret one is refused there, and a closed Vault refuses both
pub fn set(env: &Env, id: &str, name: &str, value: &str, secret: bool) -> Result<(), String> {
    std::fs::create_dir_all(&env.state_root).map_err(|e| e.to_string())?;
    if value.len() > 4096 {
        return Err("a setting is at most 4096 characters".into());
    }
    let mut current = read_plain(env, id);
    current.extend(read_sealed(env, id)?);
    current.insert(name.into(), value.into());
    let plain = serde_json::to_vec(&current).map_err(|e| e.to_string())?;
    match seal_at(&env.vault_socket, &purpose(id), id, &plain) {
        Ok(sealed) => {
            write(&sealed_file(env, id), &sealed)?;
            let _ = std::fs::remove_file(plain_file(env, id));
            Ok(())
        }
        Err(Refusal::NotSetUp) if !secret => write(&plain_file(env, id), &plain),
        Err(r) => Err(why(r)),
    }
}

/// Takes the settings away with the plugin: the files, and the plugin's key in the Vault (best effort: a closed Vault keeps it for the next time, and
/// without the file nothing is left to read with it)
pub fn forget(env: &Env, id: &str) {
    let had_secrets = sealed_file(env, id).exists();
    let _ = std::fs::remove_file(sealed_file(env, id));
    let _ = std::fs::remove_file(plain_file(env, id));
    if had_secrets {
        let _ = amethystora_vault_client::forget_at(&env.vault_socket, &purpose(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_purpose_is_a_vault_purpose_and_differs_by_plugin() {
        let (a, b) = (purpose("dev.example.a"), purpose("dev.example.b"));
        assert!(a.len() <= 40 && a.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'));
        assert_ne!(a, b);
    }

    /// A Vault that seals by wrapping and opens by unwrapping, over the same socket protocol (apps/vault-client): enough to see what is sealed and where
    fn stand_in_vault(socket: &std::path::Path) -> std::thread::JoinHandle<()> {
        use base64::Engine;
        use std::io::{BufRead, BufReader, Write};
        let listener = std::os::unix::net::UnixListener::bind(socket).unwrap();
        std::thread::spawn(move || {
            let engine = base64::engine::general_purpose::STANDARD;
            // Seal and open, a few times; the test ends the thread by connecting once more with nothing
            for stream in listener.incoming().flatten().take(6) {
                let mut line = String::new();
                if BufReader::new(stream.try_clone().unwrap()).read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                let request: serde_json::Value = serde_json::from_str(&line).unwrap();
                let data = engine.decode(request["data"].as_str().unwrap_or("")).unwrap_or_default();
                let out: Vec<u8> = match request["op"].as_str() {
                    Some("seal") => {
                        let mut sealed = b"AVs1:".to_vec();
                        sealed.extend_from_slice(request["purpose"].as_str().unwrap().as_bytes());
                        sealed.push(b':');
                        sealed.extend(data.iter().map(|b| b ^ 0x5a));
                        sealed
                    }
                    _ => data.splitn(3, |b| *b == b':').nth(2).unwrap_or_default().iter().map(|b| b ^ 0x5a).collect(),
                };
                (&stream).write_all(format!("{}\n", serde_json::json!({"ok": true, "data": engine.encode(out)})).as_bytes()).unwrap();
            }
        })
    }

    #[test]
    fn a_secret_goes_into_the_vault_and_nowhere_in_the_clear_and_comes_back_for_the_pipe() {
        let dir = std::env::temp_dir().join(format!("amethystora-secrets-vault-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let env = crate::host::Env {
            store: crate::store::Store { root: dir.clone(), shipped: dir.join("s") },
            uid: 1000,
            home: dir.clone(),
            runtime: dir.join("run"),
            state_root: dir.join("state"),
            conf_path: dir.join("c"),
            seccomp: dir.join("b"),
            drafts_root: dir.join("d"),
            vault_socket: dir.join("vault.sock"),
        };
        let vault = stand_in_vault(&env.vault_socket);
        set(&env, "dev.example.t", "token", "CANARY-TOKEN-1234", true).unwrap();
        set(&env, "dev.example.t", "city", "Lyon", false).unwrap();
        assert!(has_secrets(&env, "dev.example.t"));
        // Nothing of either is a file in the clear: no plain settings file exists, and the sealed one holds neither value
        for entry in std::fs::read_dir(&env.state_root).unwrap().flatten() {
            let bytes = std::fs::read(entry.path()).unwrap();
            assert!(!String::from_utf8_lossy(&bytes).contains("CANARY") && !String::from_utf8_lossy(&bytes).contains("Lyon"), "{}", entry.path().display());
        }
        assert!(!dir.join("state/dev.example.t.settings.json").exists());
        // What the pipe is given: both, as one object
        let all = all(&env, "dev.example.t").unwrap();
        assert_eq!((all["token"].as_str(), all["city"].as_str()), ("CANARY-TOKEN-1234", "Lyon"));
        // The sealed file is 0600
        use std::os::unix::fs::MetadataExt;
        assert_eq!(std::fs::metadata(dir.join("state/dev.example.t.secrets")).unwrap().mode() & 0o777, 0o600);
        drop(vault);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_account_with_no_vault_keeps_a_plain_setting_plainly_and_refuses_a_secret_one() {
        use std::io::{BufRead, BufReader, Write};
        let dir = std::env::temp_dir().join(format!("amethystora-secrets-novault-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let env = crate::host::Env {
            store: crate::store::Store { root: dir.clone(), shipped: dir.join("s") },
            uid: 1000,
            home: dir.clone(),
            runtime: dir.join("run"),
            state_root: dir.join("state"),
            conf_path: dir.join("c"),
            seccomp: dir.join("b"),
            drafts_root: dir.join("d"),
            vault_socket: dir.join("vault.sock"),
        };
        let listener = std::os::unix::net::UnixListener::bind(&env.vault_socket).unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten().take(4) {
                let mut line = String::new();
                let _ = BufReader::new(stream.try_clone().unwrap()).read_line(&mut line);
                let _ = (&stream).write_all(b"{\"ok\":false,\"why\":\"not-set-up\"}\n");
            }
        });
        set(&env, "dev.example.t", "city", "Lyon", false).unwrap();
        assert_eq!(all(&env, "dev.example.t").unwrap()["city"], "Lyon");
        assert!(set(&env, "dev.example.t", "token", "CANARY-1", true).unwrap_err().starts_with("vault-closed"));
        assert!(!std::fs::read_to_string(dir.join("state/dev.example.t.settings.json")).unwrap().contains("CANARY"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_closed_vault_is_said_and_keeps_nothing_in_the_clear() {
        let dir = std::env::temp_dir().join(format!("amethystora-secrets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let env = crate::host::Env {
            store: crate::store::Store { root: dir.clone(), shipped: dir.join("s") },
            uid: 1000,
            home: dir.clone(),
            runtime: dir.join("run"),
            state_root: dir.join("state"),
            conf_path: dir.join("c"),
            seccomp: dir.join("b"),
            drafts_root: dir.join("d"),
            vault_socket: dir.join("no-vault.sock"),
        };
        let error = set(&env, "dev.example.t", "token", "CANARY-TOKEN-1234", true).unwrap_err();
        assert!(error.starts_with("vault-closed"), "{error}");
        // A closed Vault gives neither a plain setting nor a secret one, and nothing is in the clear
        assert!(set(&env, "dev.example.t", "city", "Lyon", false).unwrap_err().starts_with("vault-closed"));
        assert!(all(&env, "dev.example.t").unwrap().is_empty());
        for entry in std::fs::read_dir(&env.state_root).into_iter().flatten().flatten() {
            assert!(!std::fs::read_to_string(entry.path()).unwrap().contains("CANARY"));
        }
        forget(&env, "dev.example.t");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
