//! A plugin's settings (plans/PLAN-ui-ideas-r7-build.md, "Changes to other plans"): the values a person gave it, declared in
//! its manifest. A secret one (a token) is kept sealed under a key of its own in the Vault, so that it is never a file in the
//! clear, an argument or an environment variable; the others are a plain file. Both reach the sandbox the same way: one JSON object
//! on a pipe, descriptor 10, which the plugin reads once (`read -r -u 10 settings`). A closed Vault means the plugin waits and says
//! so, and nothing is ever written in the clear that was said to be secret.

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

/// Gives one setting a value. A secret goes into the Vault and nothing of it into a plain file
pub fn set(env: &Env, id: &str, name: &str, value: &str, secret: bool) -> Result<(), String> {
    std::fs::create_dir_all(&env.state_root).map_err(|e| e.to_string())?;
    if value.len() > 4096 {
        return Err("a setting is at most 4096 characters".into());
    }
    if secret {
        let mut current = read_sealed(env, id)?;
        current.insert(name.into(), value.into());
        let plain = serde_json::to_vec(&current).map_err(|e| e.to_string())?;
        let sealed = seal_at(&env.vault_socket, &purpose(id), id, &plain).map_err(why)?;
        write(&sealed_file(env, id), &sealed)
    } else {
        let mut current = read_plain(env, id);
        current.insert(name.into(), value.into());
        write(&plain_file(env, id), &serde_json::to_vec(&current).map_err(|e| e.to_string())?)
    }
}

/// Takes the settings away with the plugin (and the key's data, which nothing can read without the file)
pub fn forget(env: &Env, id: &str) {
    let _ = std::fs::remove_file(sealed_file(env, id));
    let _ = std::fs::remove_file(plain_file(env, id));
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
        // A plain setting is a plain file; a secret one is nowhere in the clear
        set(&env, "dev.example.t", "city", "Lyon", false).unwrap();
        assert_eq!(all(&env, "dev.example.t").unwrap()["city"], "Lyon");
        for entry in std::fs::read_dir(&env.state_root).unwrap().flatten() {
            assert!(!std::fs::read_to_string(entry.path()).unwrap().contains("CANARY"));
        }
        forget(&env, "dev.example.t");
        assert!(all(&env, "dev.example.t").unwrap().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
