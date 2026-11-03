//! Who may start the host, and the manifests that let a browser start it at all. The image holds no such manifest (the
//! maintainer's choice): `ame vault browser on` writes them into the account's own folders, and `off` takes them away.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

use serde_json::json;

pub const HOST: &str = "org.amethystora.vault";
pub const PROGRAM: &str = "/usr/libexec/amethystora-vault-native";

/// The add-on's id on Firefox, `browser_specific_settings.gecko.id` in apps/vault-extension/manifest.json. An add-on chooses its
/// own, and AMO keeps it from the first upload.
pub const FIREFOX_IDS: &[&str] = &["vault@amethystora.org"];

/// The extension's ids on the Chromium family, each the hash of the public key in the manifest's `key`.
///
/// PLACEHOLDER, to be replaced before anything is published: this is the id of a key made for tests alone
/// (apps/vault-extension/tests/chromium-key.txt, which tests/chromium.py adds to a copy of the manifest; that folder is not in the store
/// package). The Chrome Web Store gives a listing its own id and refuses a manifest with a key in it: put the listing's id here, and
/// Edge's beside it if it is listed there. An id proves nothing about who is calling: it is a hash of a public key, and anyone can load
/// an unpacked extension with that key. What stands between a page and the Vault is the host's own checks, and the browser's list is
/// only the first of them.
pub const CHROMIUM_IDS: &[&str] = &["fcfklbbmhabecjliaimhlhpiklpdacno"];

/// Firefox reads this, and not `~/.config/mozilla`, although its profiles have moved there (Firefox 158, spike question 8)
const FIREFOX_FOLDER: &str = ".mozilla/native-messaging-hosts";
/// Each browser's per-user folder, as `amethystora-browser-policy` covers them. Written whether or not it is installed, so that one
/// installed later works.
const CHROMIUM_FOLDERS: &[&str] = &[
    ".config/google-chrome/NativeMessagingHosts",
    ".config/chromium/NativeMessagingHosts",
    ".config/microsoft-edge/NativeMessagingHosts",
    ".config/BraveSoftware/Brave-Browser/NativeMessagingHosts",
];
/// How far up `off` tidies a folder it may have made: never `.config` or the home itself
const NEVER_TIDIED: &[&str] = &["", ".config"];

/// What a browser's manifest says, byte for byte, so that the watcher can accept exactly this and nothing else in that folder
pub fn manifest(firefox: bool) -> String {
    let mut text = if firefox {
        json!({"name": HOST, "description": "Amethystora Vault", "path": PROGRAM, "type": "stdio", "allowed_extensions": FIREFOX_IDS})
    } else {
        let origins: Vec<String> = CHROMIUM_IDS.iter().map(|id| format!("chrome-extension://{id}/")).collect();
        json!({"name": HOST, "description": "Amethystora Vault", "path": PROGRAM, "type": "stdio", "allowed_origins": origins})
    }
    .to_string();
    text.push('\n');
    text
}

fn folders() -> impl Iterator<Item = (&'static str, bool)> {
    std::iter::once((FIREFOX_FOLDER, true)).chain(CHROMIUM_FOLDERS.iter().map(|folder| (*folder, false)))
}

fn file_name() -> String {
    format!("{HOST}.json")
}

/// Writes every browser's manifest, each put in place whole
pub fn on(home: &Path) -> io::Result<()> {
    for (folder, firefox) in folders() {
        let folder = home.join(folder);
        fs::create_dir_all(&folder)?;
        let temporary = folder.join(format!(".{}.{}", file_name(), std::process::id()));
        fs::write(&temporary, manifest(firefox))?;
        fs::rename(&temporary, folder.join(file_name())).inspect_err(|_| {
            let _ = fs::remove_file(&temporary);
        })?;
    }
    Ok(())
}

/// Takes them away, whatever they say by now, and the folders that leaves empty
pub fn off(home: &Path) -> io::Result<()> {
    for (folder, _) in folders() {
        match fs::remove_file(home.join(folder).join(file_name())) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut path = Some(Path::new(folder));
        while let Some(place) = path.filter(|place| !NEVER_TIDIED.contains(&place.to_str().unwrap_or(""))) {
            // Fails on anything not empty, which then stays
            if fs::remove_dir(home.join(place)).is_err() {
                break;
            }
            path = place.parent();
        }
    }
    Ok(())
}

/// Whether every manifest is there and says what it should
pub fn is_on(home: &Path) -> bool {
    folders().all(|(folder, firefox)| fs::read_to_string(home.join(folder).join(file_name())).is_ok_and(|text| text == manifest(firefox)))
}

/// Whether the browser says this program was started for one of our extensions. Firefox passes the manifest's path and the add-on's
/// id, and the Chromium family `chrome-extension://ID/`. Anything else is not a browser starting it for us.
pub fn started_for_us(arguments: &[String]) -> bool {
    match arguments {
        [_, origin] => CHROMIUM_IDS.iter().any(|id| origin.strip_prefix("chrome-extension://").and_then(|rest| rest.strip_suffix('/')) == Some(id)),
        [_, manifest, id] => manifest.ends_with(&format!("/{}", file_name())) && FIREFOX_IDS.contains(&id.as_str()),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Home(PathBuf);

    impl Home {
        fn new(name: &str) -> Home {
            let path = std::env::temp_dir().join(format!("vault-native-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Home(path)
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|text| text.to_string()).collect()
    }

    #[test]
    fn only_our_ids_are_let_through() {
        let manifest = format!("/home/a/.mozilla/native-messaging-hosts/{HOST}.json");
        assert!(started_for_us(&strings(&["host", &manifest, FIREFOX_IDS[0]])));
        assert!(started_for_us(&strings(&["host", &format!("chrome-extension://{}/", CHROMIUM_IDS[0])])));
        for bad in [
            vec!["host"],
            vec!["host", "browser"],
            vec!["host", &manifest],
            vec!["host", &manifest, "other@example.com"],
            vec!["host", "/somewhere/else.json", FIREFOX_IDS[0]],
            vec!["host", "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/"],
            vec!["host", "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
            vec!["host", &format!("chrome-extension://{}", CHROMIUM_IDS[0])],
            vec!["host", "moz-extension://vault@amethystora.org/"],
            vec!["host", &format!("chrome-extension://{}/", CHROMIUM_IDS[0]), "more"],
            vec!["host", &manifest, FIREFOX_IDS[0], "more"],
        ] {
            assert!(!started_for_us(&strings(&bad)), "{bad:?}");
        }
    }

    #[test]
    fn a_manifest_names_our_extensions_alone() {
        let firefox: serde_json::Value = serde_json::from_str(&manifest(true)).unwrap();
        assert_eq!(firefox["name"], HOST);
        assert_eq!(firefox["path"], PROGRAM);
        assert_eq!(firefox["type"], "stdio");
        assert_eq!(firefox["allowed_extensions"], json!(FIREFOX_IDS));
        assert!(firefox.get("allowed_origins").is_none());
        let chromium: serde_json::Value = serde_json::from_str(&manifest(false)).unwrap();
        assert_eq!(chromium["allowed_origins"].as_array().unwrap().len(), CHROMIUM_IDS.len());
        assert!(chromium.get("allowed_extensions").is_none());
    }

    #[test]
    fn on_writes_each_browser_and_makes_the_mozilla_folder() {
        let home = Home::new("on");
        assert!(!home.0.join(".mozilla").exists());
        on(&home.0).unwrap();
        assert!(is_on(&home.0));
        let firefox = home.0.join(".mozilla/native-messaging-hosts").join(format!("{HOST}.json"));
        assert_eq!(fs::read_to_string(&firefox).unwrap(), manifest(true));
        for folder in CHROMIUM_FOLDERS {
            assert_eq!(fs::read_to_string(home.0.join(folder).join(format!("{HOST}.json"))).unwrap(), manifest(false));
        }
        // Firefox does not read it there, so nothing goes there
        assert!(!home.0.join(".config/mozilla").exists());
        // Nothing half-written is left beside them
        assert_eq!(fs::read_dir(firefox.parent().unwrap()).unwrap().count(), 1);
        // And again changes nothing
        on(&home.0).unwrap();
        assert!(is_on(&home.0));
    }

    #[test]
    fn off_takes_away_what_on_wrote_and_nothing_else() {
        let home = Home::new("off");
        on(&home.0).unwrap();
        // A browser's own data, and a manifest somebody changed
        fs::create_dir_all(home.0.join(".mozilla/firefox")).unwrap();
        fs::write(home.0.join(".mozilla/firefox/profiles.ini"), "kept").unwrap();
        fs::write(home.0.join(".config/chromium/Local State"), "kept").unwrap();
        fs::write(home.0.join(".config/chromium/NativeMessagingHosts/other.json"), "kept").unwrap();
        fs::create_dir_all(home.0.join(".config/mozilla/firefox")).unwrap();
        fs::write(home.0.join(".mozilla/native-messaging-hosts").join(format!("{HOST}.json")), "pointed elsewhere").unwrap();
        assert!(!is_on(&home.0));
        off(&home.0).unwrap();
        assert!(!home.0.join(".mozilla/native-messaging-hosts").exists());
        assert_eq!(fs::read_to_string(home.0.join(".mozilla/firefox/profiles.ini")).unwrap(), "kept");
        assert_eq!(fs::read_to_string(home.0.join(".config/chromium/Local State")).unwrap(), "kept");
        assert_eq!(fs::read_to_string(home.0.join(".config/chromium/NativeMessagingHosts/other.json")).unwrap(), "kept");
        assert!(!home.0.join(".config/chromium/NativeMessagingHosts").join(format!("{HOST}.json")).exists());
        assert!(home.0.join(".config/mozilla/firefox").exists());
        // What it made and left empty is gone, and off again is no error
        assert!(!home.0.join(".config/google-chrome").exists());
        assert!(!home.0.join(".config/BraveSoftware").exists());
        off(&home.0).unwrap();
    }

    #[test]
    fn off_on_a_home_that_never_had_it_touches_nothing() {
        let home = Home::new("never");
        fs::create_dir_all(home.0.join(".config")).unwrap();
        off(&home.0).unwrap();
        assert!(home.0.join(".config").exists());
        assert!(!home.0.join(".mozilla").exists());
    }
}
