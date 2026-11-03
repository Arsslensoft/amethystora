//! The machine's root and its two CAs, in `<state>/machine/`, each one file written whole, so that a crash leaves the old one or
//! the new one. The key is sealed to the TPM behind the same `Chip` as a vault's slots, or where there is none is a file only the
//! service reaches, which leaves only the disk's encryption protecting it at rest (plans/PLAN-vault.md, The machine's root).
//! None of this needs libcrypto: the jobs that make and use them do. It all runs on the work thread, one at a time, which is
//! what makes "made once, whichever account asks first" hold.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use amethystora_vault_core::protocol::encode;
use amethystora_vault_core::slots::{chip_name, Chip};
use amethystora_vault_core::store::{make_folder, read_bounded, write_atomic};

pub const ROOT: &str = "root";
pub const DEVICE_CA: &str = "device-ca";
pub const LOCAL_CA: &str = "local-ca";

const MOST: u64 = 1024 * 1024;

fn file_of(state: &Path, which: &str) -> PathBuf {
    state.join("machine").join(format!("{which}.json"))
}

/// What a file says, its key still sealed. `Ok(None)` is not made yet. A file that does not read is refused and never made again
/// over: only `root-rotate` replaces a root.
pub fn read(state: &Path, which: &str) -> Result<Option<Value>, ()> {
    let bytes = match read_bounded(&file_of(state, which), MOST) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    let record: Value = serde_json::from_slice(&bytes).map_err(|_| ())?;
    if record["cert"].as_str().is_none() || record["key"]["data"].as_str().is_none() {
        return Err(());
    }
    Ok(Some(record))
}

/// The record without its key, for what only shows it
pub fn public_part(mut record: Value) -> Value {
    if let Some(object) = record.as_object_mut() {
        object.remove("key");
    }
    record
}

/// The key, in the form a job takes it
pub fn open_key(chip: &Arc<dyn Chip>, which: &str, record: &Value) -> Result<Zeroizing<String>, ()> {
    let data = record["key"]["data"].as_str().ok_or(())?;
    if record["key"]["sealed"] != json!(true) {
        return Ok(Zeroizing::new(data.to_string()));
    }
    let sealed = base64::engine::general_purpose::STANDARD.decode(data).map_err(|_| ())?;
    let plain = chip.open(&chip_name(&format!("machine-{which}")), &sealed).map_err(|_| ())?;
    std::str::from_utf8(&plain).map(|text| Zeroizing::new(text.to_string())).map_err(|_| ())
}

/// Keeps what a job made: `{cert, key, type, description, ...}`, the key sealed on the way if there is a chip to seal it to
pub fn save(state: &Path, chip: &Arc<dyn Chip>, which: &str, made: &Value) -> Result<Value, ()> {
    let key = made["key"].as_str().ok_or(())?;
    let key_part = if chip.present() {
        let sealed = chip.seal(&chip_name(&format!("machine-{which}")), key.as_bytes()).map_err(|_| ())?;
        json!({"sealed": true, "data": encode(&sealed)})
    } else {
        json!({"sealed": false, "data": key})
    };
    let mut record = made.clone();
    record["key"] = key_part;
    if let Some(object) = record.as_object_mut() {
        object.remove("ok");
    }
    make_folder(&state.join("machine")).map_err(|_| ())?;
    write_atomic(&file_of(state, which), record.to_string().as_bytes()).map_err(|_| ())?;
    Ok(record)
}

pub fn remove(state: &Path, which: &str) {
    let _ = std::fs::remove_file(file_of(state, which));
}

/// Whether a CA hangs from this root and is within its dates: both written when it was made, by what made it
pub fn current(record: &Value, root_sha256: &str, now: i64) -> bool {
    record["root_sha256"].as_str() == Some(root_sha256) && record["description"]["not_after"].as_i64().is_some_and(|not_after| now < not_after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use amethystora_vault_core::seal;
    use amethystora_vault_core::slots::fake::FakeChip;
    use std::sync::atomic::Ordering;

    fn folder(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("avm-{name}-{}", seal::random_hex(4)));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn a_key_is_sealed_to_the_chip_and_stays_in_the_file_when_there_is_none() {
        let state = folder("chip");
        let chip = Arc::new(FakeChip::new());
        let handle: Arc<dyn Chip> = chip.clone();
        assert!(read(&state, ROOT).unwrap().is_none());
        let made = json!({"ok": true, "key": "S0VZ", "cert": "Q0VSVA==", "type": "rsa-4096", "description": {"sha256": "aa", "not_after": 10}});
        let record = save(&state, &handle, ROOT, &made).unwrap();
        assert_eq!(record["key"]["sealed"], json!(true));
        assert!(record.get("ok").is_none());
        let on_disk = std::fs::read_to_string(state.join("machine/root.json")).unwrap();
        assert!(!on_disk.contains("S0VZ"), "{on_disk}");
        let back = read(&state, ROOT).unwrap().unwrap();
        assert_eq!(open_key(&handle, ROOT, &back).unwrap().as_str(), "S0VZ");
        // Another chip, or the file passed off as another CA's, does not open it
        let other: Arc<dyn Chip> = Arc::new(FakeChip::new());
        assert!(open_key(&other, ROOT, &back).is_err());
        assert!(open_key(&handle, DEVICE_CA, &back).is_err());
        assert!(public_part(back).get("key").is_none());
        // With no TPM, a file only the service reaches, which is all the plan promises for that
        chip.here.store(false, Ordering::Relaxed);
        let record = save(&state, &handle, LOCAL_CA, &made).unwrap();
        assert_eq!(record["key"], json!({"sealed": false, "data": "S0VZ"}));
        assert_eq!(open_key(&handle, LOCAL_CA, &read(&state, LOCAL_CA).unwrap().unwrap()).unwrap().as_str(), "S0VZ");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(state.join("machine/local-ca.json")).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(state.join("machine")).unwrap().permissions().mode() & 0o777, 0o700);
        let _ = std::fs::remove_dir_all(&state);
    }

    #[test]
    fn a_file_that_does_not_read_is_refused() {
        let state = folder("bad");
        make_folder(&state.join("machine")).unwrap();
        for bad in ["", "{", "{}", "[]", r#"{"cert":"a"}"#, r#"{"key":{"data":"a"}}"#] {
            std::fs::write(state.join("machine/root.json"), bad).unwrap();
            assert!(read(&state, ROOT).is_err(), "{bad}");
        }
        let _ = std::fs::remove_dir_all(&state);
    }

    #[test]
    fn a_ca_is_current_under_its_own_root_and_dates() {
        let ca = json!({"root_sha256": "aa", "description": {"not_after": 100}});
        assert!(current(&ca, "aa", 99));
        assert!(!current(&ca, "aa", 100));
        assert!(!current(&ca, "bb", 50));
        assert!(!current(&json!({}), "aa", 50));
    }
}
