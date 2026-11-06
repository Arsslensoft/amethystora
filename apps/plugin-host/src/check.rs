//! `ame plugin check`: what installing refuses (a manifest or a file shape), and what a plugin author is told before the
//! sandbox tells them the hard way. Runs nothing from the plugin.

use std::path::Path;

use crate::manifest::Manifest;
use crate::tree;

#[derive(Debug, Default)]
pub struct Report {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub manifest: Option<Manifest>,
}

/// Programs that find no bus in a sandbox, and what a plugin asks the host instead
const BUS_TOOLS: &[(&str, &str)] = &[
    ("gsettings", "plugin state"),
    ("busctl", "plugin state or plugin control"),
    ("gdbus", "plugin state or plugin control"),
    ("notify-send", "plugin notify"),
];

fn words(line: &str) -> impl Iterator<Item = &str> {
    line.split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
}

pub fn check(folder: &Path) -> Report {
    check_as(folder, false)
}

/// `system` lets the id start with amethystora., which the image's own plugins in /usr/share/amethystora/plugins have and an author's never does
pub fn check_as(folder: &Path, system: bool) -> Report {
    let mut report = Report::default();
    let entries = match tree::read_dir(folder) {
        Ok(entries) => entries,
        Err(e) => {
            report.errors.push(e);
            return report;
        }
    };
    let Some(entry) = entries.iter().find(|e| e.path == "manifest.json" && !e.dir) else {
        report.errors.push("there is no manifest.json".into());
        return report;
    };
    let manifest = match std::str::from_utf8(&entry.data).map_err(|_| "manifest.json is not text".to_string()).and_then(|text| Manifest::parse_with(text, system)) {
        Ok(m) => m,
        Err(e) => {
            report.errors.push(e);
            return report;
        }
    };
    let program = manifest.command[0].trim_start_matches("./");
    match entries.iter().find(|e| e.path == program) {
        Some(e) if !e.dir && e.exec => {}
        Some(e) if !e.dir => report.errors.push(format!("{program} is not executable (chmod +x)")),
        _ => report.errors.push(format!("the command's program, {program}, is not a file of the plugin")),
    }
    for e in entries.iter().filter(|e| !e.dir && e.path != "manifest.json" && e.data.len() <= 256 * 1024) {
        let Ok(text) = std::str::from_utf8(&e.data) else { continue };
        for (number, line) in text.lines().enumerate() {
            let line_words: Vec<&str> = words(line).collect();
            let at = format!("{}:{}", e.path, number + 1);
            for (tool, instead) in BUS_TOOLS {
                if !line.trim_start().starts_with('#') && line_words.contains(tool) {
                    report.warnings.push(format!("{at} uses {tool}, which finds no bus in the sandbox; ask the host with: {instead}"));
                }
            }
            // The state folder is the account's: code in it is what a program of the account could have changed
            let runs = ["source", "exec", "eval", "sh", "bash", "chmod"].iter().any(|w| line_words.contains(w)) || line.trim_start().starts_with(". ");
            if runs && (line.contains("PLUGIN_STATE") || line.contains("WIDGET_STATE") || line.contains("/state/")) {
                report.errors.push(format!("{at} runs or sources something from the state folder, which the account can change: a plugin never does"));
            }
        }
    }
    let perms = manifest.permissions();
    if perms.iter().any(|p| p.starts_with("files=")) && perms.iter().any(|p| p.starts_with("network=")) {
        report.warnings.push("it asks for files and for the network together: a person is told it can send those files to that host".into());
    }
    report.manifest = Some(manifest);
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Entry;

    fn folder(name: &str, script: &str, exec: bool) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("plugin-check-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = r#"{"schema":1,"id":"dev.example.c","name":"C","version":"1.0.0","license":"MIT","command":["./run.sh"],
            "needs":{"network":[{"host":"a.example.com","why":"x"}],"files":[{"path":"~/Docs","why":"y"}]}}"#;
        let entries = [
            Entry { path: "manifest.json".into(), dir: false, exec: false, data: manifest.as_bytes().to_vec() },
            Entry { path: "run.sh".into(), dir: false, exec, data: script.as_bytes().to_vec() },
        ];
        tree::write_tree(&dir, &entries).unwrap();
        dir
    }

    #[test]
    fn a_clean_plugin_and_its_warnings() {
        let dir = folder("clean", "#!/bin/sh\ndate\n# gsettings in a comment\ngsettings get a b\nnotify-send hi\n", true);
        let report = check(&dir);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.warnings.iter().filter(|w| w.contains("finds no bus")).count(), 2);
        assert!(report.warnings.iter().any(|w| w.contains("files and for the network")));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn errors_for_what_cannot_install() {
        let dir = folder("bad", "source \"$PLUGIN_STATE/helper.sh\"\n", false);
        let report = check(&dir);
        assert!(report.errors.iter().any(|e| e.contains("not executable")) && report.errors.iter().any(|e| e.contains("state folder")));
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(!check(Path::new("/nonexistent/plugin")).errors.is_empty());
    }
}
