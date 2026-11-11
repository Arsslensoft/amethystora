//! What Amethystora Help runs. Two helpers say everything the window shows: /usr/libexec/amethystora-wrong looks at what
//! a person picked and names one thing to press, and /usr/libexec/amethystora-changes lists what changed with an Undo for
//! each. The window keeps the answers in memory while it is open and writes none of them down.
//!
//! The window runs only what a helper gave, as an argument vector, never through a shell. A fix has to be an `ame`
//! command (the words `ame` first, and no word a shell or the recipe's option parser could take for anything else), and
//! a line about security only ever opens one of Security's pages. An Undo passes back only the id the list gave, and the
//! helper reads its list again before it does anything.

use std::time::Duration;

use amethystora_ui::process::{self, Options, Output};
use serde::de::DeserializeOwned;
use serde::Deserialize;

pub const WRONG: &str = "/usr/libexec/amethystora-wrong";
pub const CHANGES: &str = "/usr/libexec/amethystora-changes";
pub const SECURITY: &str = "amethystora-security";
pub const PAGES: [&str; 2] = ["wrong", "changed"];

/// What Security's command line opens: its pages and Settings' categories (`openable` in apps/security/src/backend.rs)
const SECURITY_PAGES: [&str; 17] = [
    "scan", "network", "apps", "containers", "events", "settings", "settings/watching", "settings/audit", "settings/detection", "settings/scans", "settings/network", "settings/logins",
    "settings/devices", "settings/apps", "settings/system", "settings/notices", "settings/away",
];

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Topic {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Fix {
    pub label: String,
    pub command: Vec<String>,
    pub terminal: bool,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct OpensSecurity {
    pub label: String,
    pub page: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Found {
    pub topic: String,
    pub state: String,
    pub words: String,
    pub fix: Option<Fix>,
    pub security: Option<OpensSecurity>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Undo {
    pub label: String,
    pub terminal: bool,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Change {
    pub id: String,
    pub kind: String,
    pub what: String,
    pub detail: String,
    pub time: f64,
    pub undo: Option<Undo>,
    pub security: Option<OpensSecurity>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Changes {
    pub items: Vec<Change>,
    pub snapshots: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Undone {
    pub ok: bool,
    pub words: String,
}

/// An `ame` command the window will run: `ame` first, then plain words a recipe takes. Refuses a helper's answer that is
/// anything else, whatever made it so, and any `ame security` command (`security::x` is the same group): security stays in
/// Security.
pub fn fix_allowed(command: &[String]) -> bool {
    command.first().is_some_and(|word| word == "ame")
        && command.len() >= 2
        && !in_security(&command[1])
        && command.iter().all(|word| !word.is_empty() && !word.starts_with('-') && word.chars().all(|c| c.is_ascii_alphanumeric() || "_.:-".contains(c)))
}

/// The old flat names (`00-entry.just`) of recipes in the security group, which a test keeps equal to that file
const SECURITY_ALIASES: [&str; 15] = [
    "blocked-addresses",
    "blocked-connections",
    "enroll-secure-boot-key",
    "security-audit",
    "security-events",
    "security-settings",
    "security-status",
    "setup-browser-protection",
    "setup-disk-unlock",
    "setup-input-remapper",
    "setup-ransomware-protection",
    "setup-security-key",
    "setup-usb-protection",
    "toggle-tpm2",
    "virus-scan",
];

/// Whether a command's first word is the security group (`security::x` too), anything starting with `security`, or an old name of
/// one of its recipes
fn in_security(word: &str) -> bool {
    word.starts_with("security") || SECURITY_ALIASES.contains(&word)
}

pub fn security_allowed(page: &str) -> bool {
    SECURITY_PAGES.contains(&page)
}

/// The page to open: `amethystora-help changed`
pub fn page_argument(args: &[String]) -> Option<&'static str> {
    let page = args.iter().find(|arg| !arg.starts_with('-'))?;
    PAGES.iter().find(|known| *known == page).copied()
}

fn parse<T: DeserializeOwned>(output: &Output) -> Option<T> {
    serde_json::from_str(&output.stdout).ok()
}

async fn ask<T: DeserializeOwned>(program: &str, args: &[&str], seconds: u64) -> Option<T> {
    parse(&process::run_with(program, args, Options { timeout: Duration::from_secs(seconds), ..Default::default() }).await)
}

pub async fn topics() -> Vec<Topic> {
    ask(WRONG, &["topics", "--json"], 15).await.unwrap_or_default()
}

pub async fn look(topic: &str) -> Option<Found> {
    ask(WRONG, &["look", topic, "--json"], 60).await
}

pub async fn changes() -> Option<Changes> {
    ask(CHANGES, &["list", "--json"], 120).await
}

pub async fn undo(id: &str) -> Option<Undone> {
    ask(CHANGES, &["undo", id, "--json"], 86400).await
}

pub fn undo_in_terminal(id: &str) {
    process::in_terminal(&[CHANGES, "undo", id]);
}

pub fn open_security(page: &str) {
    if security_allowed(page) {
        process::launch(SECURITY, &[page]);
    }
}

pub fn run_in_terminal(command: &[String]) {
    if fix_allowed(command) {
        let words: Vec<&str> = command.iter().map(String::as_str).collect();
        process::in_terminal(&words);
    }
}

/// A fix that asks nothing, run here, with whether it worked and the end of what it said
pub async fn run_fix(command: &[String]) -> (bool, String) {
    if !fix_allowed(command) {
        return (false, String::new());
    }
    let args: Vec<&str> = command[1..].iter().map(String::as_str).collect();
    let output = process::run_with(&command[0], &args, Options { timeout: Duration::from_secs(60), ..Default::default() }).await;
    (output.code == 0, output.last_error_line())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|word| word.to_string()).collect()
    }

    #[test]
    fn runs_only_ame_commands() {
        assert!(fix_allowed(&words(&["ame", "system", "sound", "use", "52"])));
        assert!(fix_allowed(&words(&["ame", "update"])));
        assert!(fix_allowed(&words(&["ame", "system", "printer", "resume", "HP_LaserJet-1.2"])));
        for bad in [
            &["sh", "-c", "x"][..],
            &["ame"],
            &[],
            &["ame", "--command", "x"],
            &["ame", "system", "printer", "resume", "$(x)"],
            &["ame", "system", "printer", "resume", "a b"],
            &["ame", ""],
            &["/usr/bin/ame", "update"],
            &["ame", "security", "scan", "now"],
            &["ame", "security"],
            &["ame", "security::scan"],
            &["ame", "security-audit"],
            &["ame", "security-events"],
            &["ame", "enroll-secure-boot-key"],
            &["ame", "blocked-addresses"],
            &["ame", "virus-scan", "now"],
        ] {
            assert!(!fix_allowed(&words(bad)), "{bad:?}");
        }
    }

    /// Every alias of `00-entry.just` that stands for a security recipe is refused (skips where the source tree is not there)
    #[test]
    fn refuses_every_old_name_of_a_security_recipe() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../system_files/shared/usr/share/amethystora/just/00-entry.just");
        let Ok(entry) = std::fs::read_to_string(path) else { return };
        let mut found = 0;
        for line in entry.lines() {
            let Some(rest) = line.trim().strip_prefix("alias ") else { continue };
            let Some((name, target)) = rest.split_once(":=") else { continue };
            if target.trim().starts_with("security::") {
                found += 1;
                assert!(!fix_allowed(&words(&["ame", name.trim()])), "{name}");
            }
        }
        assert_eq!(found, SECURITY_ALIASES.len());
    }

    #[test]
    fn opens_only_securitys_own_pages() {
        for page in ["settings", "settings/network", "settings/devices", "events"] {
            assert!(security_allowed(page), "{page}");
        }
        for page in ["settings/duress", "--help", "", "settings/../x", "apps/x;y"] {
            assert!(!security_allowed(page), "{page}");
        }
    }

    #[test]
    fn reads_what_the_helpers_say() {
        let found: Found = serde_json::from_str(
            r#"{"version":1,"topic":"sound","state":"found","words":"Sound is muted.",
               "fix":{"label":"Turn sound on","command":["ame","system","sound","unmute"],"terminal":false},"security":null}"#,
        )
        .unwrap();
        assert_eq!(found.fix.unwrap().command, words(&["ame", "system", "sound", "unmute"]));
        let changes: Changes = serde_json::from_str(
            r#"{"version":1,"snapshots":true,"items":[{"id":"trash:a.trashinfo","kind":"file","what":"You deleted a","detail":"","time":12,
               "undo":{"label":"Undo","terminal":false},"security":null},
               {"id":"security:1","kind":"security","what":"A security setting changed","detail":"x","time":3,"undo":null,
               "security":{"label":"Open Security","page":"settings"}}]}"#,
        )
        .unwrap();
        assert!(changes.snapshots);
        assert_eq!(changes.items[0].undo.as_ref().unwrap().label, "Undo");
        assert_eq!(changes.items[1].security.as_ref().unwrap().page, "settings");
    }

    #[test]
    fn opens_on_a_page() {
        assert_eq!(page_argument(&words(&["changed"])), Some("changed"));
        assert_eq!(page_argument(&words(&["wrong"])), Some("wrong"));
        assert_eq!(page_argument(&words(&["elsewhere"])), None);
        assert_eq!(page_argument(&[]), None);
    }
}
