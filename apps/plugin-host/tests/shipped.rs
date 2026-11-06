//! What the image ships as plugins (system_files/shared/usr/share/amethystora/plugins) and the examples to start from
//! (plugin-examples): every one passes the check install runs, a shipped one asks for nothing that reaches outside the machine (a click is
//! all it takes to add one), and every example's sample.json is something the shell could be given.

use std::path::PathBuf;

use amethystora_plugin_host::check::check;
use amethystora_plugin_host::manifest::{Manifest, shipped_refuses};
use amethystora_plugin_host::output;

fn folders(name: &str) -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../system_files/shared/usr/share/amethystora").join(name);
    let mut found: Vec<PathBuf> = std::fs::read_dir(&root).unwrap_or_else(|e| panic!("{}: {e}", root.display())).flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    found.sort();
    found
}

#[test]
fn every_shipped_plugin_passes_the_check_and_reaches_nothing_outside() {
    let all = folders("plugins");
    assert!(all.len() >= 7, "the six widgets and hello are shipped");
    for folder in all {
        let report = check(&folder);
        assert!(report.errors.is_empty(), "{}: {:?}", folder.display(), report.errors);
        let manifest = Manifest::parse_with(&std::fs::read_to_string(folder.join("manifest.json")).unwrap(), true).unwrap();
        assert_eq!(folder.file_name().unwrap().to_str().unwrap(), manifest.id);
        assert!(manifest.id.starts_with("amethystora."));
        for perm in manifest.permissions() {
            assert!(!shipped_refuses(&perm), "{} asks for {perm}", manifest.id);
        }
        // A bus tool in a script finds no bus in the sandbox: the check says so, and a shipped plugin has none
        assert!(report.warnings.iter().all(|w| !w.contains("finds no bus")), "{}: {:?}", folder.display(), report.warnings);
    }
}

#[test]
fn every_example_passes_the_check_and_has_a_sample_the_shell_could_draw() {
    let all = folders("plugin-examples");
    assert!(all.len() >= 5, "the build, the parcel, the score, the door and the stock");
    for folder in all {
        let report = check(&folder);
        assert!(report.errors.is_empty(), "{}: {:?}", folder.display(), report.errors);
        let manifest = report.manifest.unwrap();
        // An example is not the image's: it never claims a system id, and the ones that reach a service say which host
        assert!(!manifest.id.starts_with("amethystora."));
        let sample = std::fs::read_to_string(folder.join("sample.json")).unwrap();
        let printed = output::parse(&sample, &manifest.actions).unwrap_or_else(|e| panic!("{}: sample.json: {e}", folder.display()));
        assert!(printed["card"].as_array().unwrap().len() <= output::MAX_CARD);
        // A token or a password is a secret setting, never a plain one
        for setting in &manifest.settings {
            if setting.name.contains("token") || setting.name.contains("password") {
                assert!(setting.secret, "{}: {} is a secret", manifest.id, setting.name);
            }
        }
    }
}
