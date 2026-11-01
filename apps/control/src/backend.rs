//! What Amethystora Control runs: what main.js did for the Electron window.
//!
//! Amethystora's own settings outside security, in a window of their own, for what is otherwise only in
//! `ame`. What each setting is now, and the command that changes it, come from one script,
//! /usr/libexec/amethystora-control-list, which `ame control` prints too. This window shows that list and
//! runs its commands, and nothing of its own.
//!
//! The page only ever sends back an entry's id and one of the values the list gave for it, and what runs
//! is the argument vector the list has for that, never through a shell. The few things the page does type
//! are checked here first: a slider's number against the list's range, a new widget's name against the
//! rule amethystora-widgets keeps for names, and a repository to trust against the rule
//! amethystora-trust-image keeps. The key to trust is picked in the window's own file dialog, and the page
//! never names it. A command that asks something, explains a risk or needs a password opens in a terminal
//! (amethystora-in-terminal), and the page reads the list again when the window has the focus back.
//!
//! Security's settings are not here: they are Amethystora Security's, which this window only opens.

use std::path::Path;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Duration;

use amethystora_ui::process::{self, Options, Output};
use amethystora_ui::t;
use regex::Regex;
use serde_json::Value;

pub use amethystora_control_core::{Item, List, Refusal, Run, LIST};
use amethystora_control_core as core;

pub const SETUP: &str = "/usr/libexec/amethystora-setup";
pub const IMAGE_INFO: &str = "/usr/share/amethystora/image-info.json";
/// The pages of the sidebar, in its order
pub const PAGES: [&str; 7] = ["appearance", "desktop", "keyboard", "apps", "agent", "system", "setup"];
/// Pick a look, the page a new account sees once (`amethystora-control welcome`), which is not in the sidebar
pub const WELCOME: &str = "welcome";
/// The pictures a wallpaper can be, by their extension
pub const PICTURES: [&str; 5] = ["jpg", "jpeg", "png", "webp", "svg"];
/// Terminal colour codes some commands write into what they say
const ANSI: &str = r"\x1b\[[0-9;?]*[A-Za-z]";

fn ansi() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| Regex::new(ANSI).expect("a valid expression"))
}

/// The page to open: `amethystora-control system` opens on the system's settings
pub fn page_argument(args: &[String]) -> Option<&'static str> {
    let page = args.iter().find(|arg| !arg.starts_with('-'))?;
    PAGES.iter().chain([WELCOME].iter()).find(|known| *known == page).copied()
}

/// The entry after the page (`amethystora-control desktop focus`), whose card the page opens scrolled to: an id as the
/// list writes them, and nothing else
pub fn entry_argument(args: &[String]) -> Option<String> {
    let entry = args.iter().filter(|arg| !arg.starts_with('-')).nth(1)?;
    let fits = !entry.is_empty() && entry.len() <= 64 && entry.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    fits.then(|| entry.clone())
}

// --- The list ---------------------------------------------------------------------------------------------
//
// What the list holds and how an entry's id and value become a command are amethystora-control-core's,
// which the agent's tools share, so that both change a setting the same way. What it says when it refuses
// is said here, in the window's own words.

/// What to say when the list does not offer what was asked, or what was typed is not taken
fn words(refusal: Refusal) -> String {
    match refusal {
        Refusal::Refused => refused(),
        Refusal::WidgetName => t!("A widget's name is lower case letters, digits and dashes, and not the name of one you have."),
        Refusal::Repository => t!("That is not a repository such as ghcr.io/you/your-image: lower case, and no tag."),
        Refusal::OwnRepository => t!("Amethystora's own images are checked against Amethystora's key, and that stays as it is."),
        Refusal::NoKey => t!("Choose the key first: the cosign.pub of your repository."),
        // amethystora-text-shortcuts says the same when it refuses
        Refusal::ShortcutAbbr => t!("A shortcut is 2 to 32 characters, with no spaces."),
        Refusal::ShortcutLength => t!("The text is 1 to 2000 characters."),
        Refusal::ShortcutCharacter => t!("The text has a character that cannot be typed."),
        // amethystora-theme-file says the same when it refuses
        Refusal::ThemeName => t!("A theme's name is 1 to 40 letters, digits, spaces and dashes."),
        Refusal::ThemeColours => t!("Every colour of the theme has to be chosen."),
        Refusal::Accelerator => t!("A shortcut needs Super, Ctrl or Alt with a key, or a function key."),
    }
}

/// The list from what the script printed, or None when that was not JSON
pub fn parse_list(stdout: &str) -> Option<List> {
    core::parse_list(stdout)
}

/// The list, read again, or what to say when it could not be read
pub async fn read_list() -> Result<List, String> {
    let result = process::run_with(LIST, &["--json"], Options { timeout: Duration::from_secs(60), ..Default::default() }).await;
    parse_list(&result.stdout).ok_or_else(|| {
        let error = ansi().replace_all(&result.stderr, "").trim().to_string();
        if error.is_empty() { t!("The settings could not be read.") } else { error }
    })
}

// --- Running what the list says ---------------------------------------------------------------------------

/// What came of a command: it opened in a terminal, it could not be run, or it ran here and said this
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Terminal,
    Error(String),
    Ran { ok: bool, output: String },
}

fn refused() -> String {
    t!("That cannot be done from here now. Read the settings again and try once more.")
}

/// What a command run here printed, as the page shows it under its card: the last lines, without colours
pub fn said(result: &Output) -> String {
    let all = format!("{}\n{}", result.stdout, result.stderr);
    let plain = ansi().replace_all(&all, "");
    let lines: Vec<&str> = plain.trim().split('\n').collect();
    lines[lines.len().saturating_sub(12)..].join("\n")
}

/// A command from the list, in a terminal when it asks for one, and otherwise here, with what it said
pub async fn perform(plan: Result<Run, String>) -> Outcome {
    let run = match plan {
        Ok(run) => run,
        Err(error) => return Outcome::Error(error),
    };
    if run.terminal {
        process::launch_owned(process::IN_TERMINAL, &run.argv);
        return Outcome::Terminal;
    }
    let args: Vec<&str> = run.argv[1..].iter().map(String::as_str).collect();
    // What it reads on stdin (a text shortcut's text) is written there, never put among its arguments
    let options = Options { timeout: Duration::from_secs(300), stdin: run.stdin.clone(), ..Default::default() };
    let result = process::run_with(&run.argv[0], &args, options).await;
    Outcome::Ran { ok: result.code == 0, output: said(&result) }
}

/// A setting set to one of its choices, or a slider to a number in its range
pub fn change(list: Option<&List>, id: &str, value: &str) -> Result<Run, String> {
    core::change(list, id, value).map_err(words)
}

pub fn act(list: Option<&List>, id: &str) -> Result<Run, String> {
    core::act(list, id).map_err(words)
}

/// The rule amethystora-widgets has for a widget's name, as the page checks it before it sends one
pub fn valid_widget_name(item: &Item, name: &str) -> bool {
    core::valid_widget_name(item, name)
}

/// A widget the list named: on, off, fix or remove; or restart and make, which name none
pub fn widget(list: Option<&List>, action: &str, id: &str) -> Result<Run, String> {
    core::widget(list, action, id).map_err(words)
}

/// A new widget from one of the examples, under a name of its own
pub fn new_widget(list: Option<&List>, id: &str, example: &str) -> Result<Run, String> {
    core::new_widget(list, id, example).map_err(words)
}

/// A widget of the gallery, added to the bar or the desktop in one click
pub fn gallery_widget(list: Option<&List>, example: &str, place: &str) -> Result<Run, String> {
    core::gallery_widget(list, example, place).map_err(words)
}

/// A web app the list named, removed; or a new one, added in a terminal
pub fn webapp(list: Option<&List>, action: &str, name: &str) -> Result<Run, String> {
    core::webapp(list, action, name).map_err(words)
}

/// One of the actions the list gave a pool of disks
pub fn pool(list: Option<&List>, action: &str, id: &str) -> Result<Run, String> {
    core::pool(list, action, id).map_err(words)
}

// --- A theme of one's own --------------------------------------------------------------------------------

/// A palette made in the theme maker, saved as the account's theme: the name one argument, the palette on stdin
pub fn save_theme(list: Option<&List>, name: &str, colors: &std::collections::BTreeMap<String, String>) -> Result<Run, String> {
    core::save_theme(list, name, colors).map_err(words)
}

/// A colour file picked in the file dialog, imported in a terminal, which shows it and asks first
pub fn import_theme(list: Option<&List>, file: Option<&Path>) -> Result<Run, String> {
    core::import_theme(list, file).map_err(words)
}

// --- Keyboard shortcuts ----------------------------------------------------------------------------------

/// A shortcut the list named set to the keys recorded, taken off or put back as the image ships it
pub fn keybinding(list: Option<&List>, action: &str, schema: &str, key: &str, accel: &str) -> Result<Run, String> {
    core::keybinding(list, action, schema, key, accel).map_err(words)
}

/// One of the actions the list gave a row of a rows entry: what starts at login, an app changed in the grid
pub fn row(list: Option<&List>, card: &str, action: &str, id: &str) -> Result<Run, String> {
    core::row(list, card, action, id).map_err(words)
}

// --- Text shortcuts ---------------------------------------------------------------------------------------

/// A text shortcut added (or its text changed) or removed: an abbreviation the list named, or one with its text
/// checked here as amethystora-text-shortcuts checks them, each one argument
pub fn shortcut(list: Option<&List>, action: &str, abbr: &str, text: &str) -> Result<Run, String> {
    core::shortcut(list, action, abbr, text).map_err(words)
}

/// Why a new shortcut would not be taken, in the words the helper uses, or None
pub fn shortcut_problem(abbr: &str, text: &str) -> Option<String> {
    if !core::valid_shortcut_abbr(abbr) {
        return Some(words(Refusal::ShortcutAbbr));
    }
    core::shortcut_text_problem(text).map(words)
}

/// A shortcut's text, read only when Edit is pressed, so that the list itself never holds one
pub async fn shortcut_text(list: Option<Rc<List>>, abbr: &str) -> Result<String, String> {
    let run = shortcut(list.as_deref(), "show", abbr, "")?;
    let args: Vec<&str> = run.argv[1..].iter().map(String::as_str).collect();
    let result = process::run_with(&run.argv[0], &args, Options { timeout: Duration::from_secs(30), ..Default::default() }).await;
    if result.code == 0 { Ok(result.stdout) } else { Err(said(&result)) }
}

/// What the field to try shortcuts in shows happened, change by change: the text before ended with a listed
/// abbreviation and a space and the change took exactly those away (the shortcut's deletion), then the next change
/// added a text and a space after what was left (its expansion)
#[derive(Debug, Clone, PartialEq)]
pub enum Tried {
    Nothing,
    Waiting { abbr: String, left: String },
    Expanded { abbr: String, text: String },
}

pub fn tried(before: &str, now: &str, waiting: Option<(&str, &str)>, abbrs: &[String]) -> Tried {
    if let Some((abbr, left)) = waiting {
        if let Some(added) = now.strip_prefix(left).and_then(|rest| rest.strip_suffix(' ')) {
            if !added.is_empty() {
                return Tried::Expanded { abbr: abbr.to_string(), text: added.to_string() };
            }
        }
    }
    for abbr in abbrs {
        if before.strip_suffix(&format!("{abbr} ")).is_some_and(|left| left == now) {
            return Tried::Waiting { abbr: abbr.clone(), left: now.to_string() };
        }
    }
    Tried::Nothing
}

/// The first line of an expansion, 60 characters at most, for the line under the field
pub fn first_line(text: &str) -> String {
    let line = text.split('\n').next().unwrap_or_default();
    if line.chars().count() > 60 { format!("{}…", line.chars().take(60).collect::<String>()) } else { line.to_string() }
}

// --- An image of one's own --------------------------------------------------------------------------------

/// The rule amethystora-trust-image has for a repository
pub fn valid_repository(repository: &str) -> bool {
    core::valid_repository(repository)
}

/// Amethystora's own repositories, which keep Amethystora's key, as amethystora-trust-image has them
pub fn own_repository(image_info: &str) -> Option<String> {
    core::own_repository(image_info)
}

/// Trusting a repository with the key picked in the file dialog. `own` is Amethystora's own repository.
pub fn trust_image(list: Option<&List>, repository: &str, own: Option<&str>, key: Option<&Path>) -> Result<Run, String> {
    core::trust_image(list, repository, own, key).map_err(words)
}

// --- The setup file ---------------------------------------------------------------------------------------

/// The setup file saved here, or applied in a terminal, where `ame setup apply` shows what it adds first
pub fn setup(list: Option<&List>, action: &str) -> Result<Run, String> {
    core::setup(list, action).map_err(words)
}

/// What the machine and the setup file disagree on, which `ame setup diff` works out and changes nothing
pub async fn setup_diff() -> Result<Vec<Value>, String> {
    let result = process::run_with(SETUP, &["diff", "--json"], Options { timeout: Duration::from_secs(120), ..Default::default() }).await;
    parse_diff(&result).ok_or_else(|| {
        let said = said(&result);
        if said.is_empty() { t!("Your setup file could not be compared with this machine.") } else { said }
    })
}

/// The changes, or None when what it printed was not JSON. Anything but a list of them is as if it had
/// found none, as the page took it.
pub fn parse_diff(result: &Output) -> Option<Vec<Value>> {
    let value: Value = serde_json::from_str(&result.stdout).ok()?;
    Some(match value {
        Value::Array(changes) => changes,
        _ => Vec::new(),
    })
}

// --- Elsewhere --------------------------------------------------------------------------------------------

/// The windows a link opens, each by the one command that opens it
pub fn link_program(link: &str, list: Option<&List>) -> Option<&'static str> {
    match link {
        "security" => Some("amethystora-security"),
        "updates" => Some("amethystora-update"),
        "backups" if list.is_some_and(|list| list.backups) => Some("amethystora-backups"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST_JSON: &str = r#"{"version": 1, "sealed": false, "elsewhere": {"automatic_updates": "on", "backups": true}, "items": [
        {"page": "appearance", "id": "transition-length", "kind": "range", "group": null, "title": "Transition length", "text": "How long",
         "state": "1200", "available": true, "min": 200, "max": 5000, "step": 100, "unit": "ms", "terminal": false,
         "command": ["ame", "desktop", "transition", "grow", "{value}"]},
        {"page": "desktop", "id": "greeting", "kind": "switch", "title": "Terminal greeting", "state": "on", "available": true,
         "choices": [{"value": "on", "title": "On", "text": null, "terminal": false, "command": ["ame", "desktop", "greeting", "on"]},
                     {"value": "off", "title": "Off", "text": null, "terminal": true, "command": ["ame", "desktop", "greeting", "off"]}]},
        {"page": "apps", "id": "gaming", "kind": "switch", "title": "Games", "state": "off", "available": false, "reason": "No",
         "choices": [{"value": "on", "title": "On", "terminal": true, "command": ["ame", "apps", "gaming", "on"]}]},
        {"page": "apps", "id": "cli", "kind": "action", "title": "Tools", "available": true, "terminal": true, "command": ["ame", "apps", "cli"], "button": "Install…"},
        {"page": "apps", "id": "bad", "kind": "action", "title": "Bad", "available": true, "terminal": false, "command": ["ame", 3]},
        {"page": "desktop", "id": "widgets", "kind": "widgets", "title": "Top bar widgets", "available": true, "bar": false, "agentic": false,
         "widgets": [{"id": "pomodoro", "name": "Pomodoro", "on": true, "broken": false}, {"id": "half", "name": null, "on": false, "broken": true}],
         "examples": ["hello", "pomodoro"],
         "commands": {"on": {"terminal": false, "command": ["ame", "desktop", "widgets", "on", "{id}"]},
                      "new": {"terminal": false, "command": ["ame", "desktop", "widgets", "new", "{id}", "{example}"]},
                      "restart": {"terminal": false, "command": ["ame", "desktop", "widgets", "restart"]},
                      "fix": {"terminal": true, "command": ["ame", "desktop", "widgets", "fix", "{id}"]}}},
        {"page": "apps", "id": "webapps", "kind": "webapps", "title": "Web apps", "available": true,
         "webapps": [{"name": "Element", "address": "https://app.element.io"}],
         "commands": {"install": {"terminal": true, "command": ["ame", "apps", "webapps", "install"]},
                      "remove": {"terminal": false, "command": ["ame", "apps", "webapps", "remove", "{name}"]}}},
        {"page": "system", "id": "trust-image", "kind": "trust-image", "title": "Own", "available": true, "terminal": true,
         "command": ["ame", "system", "trust-image", "{repository}", "{key}"]},
        {"page": "system", "id": "disks", "kind": "disks", "title": "Pools", "available": true,
         "disks": [{"id": "media", "title": "media, at /var/mnt/media", "text": "Two copies.", "state": "ok",
                    "actions": [{"id": "forget", "title": "Forget…", "terminal": true, "command": ["ame", "system", "raid", "forget", "media"]}]}]},
        {"page": "setup", "id": "setup-file", "kind": "setup", "title": "Your setup file", "available": true, "file": "/home/a/.config/amethystora/setup.json",
         "saved": 1760000000, "state": "saved",
         "commands": {"save": {"terminal": false, "command": ["ame", "setup", "save"]}, "apply": {"terminal": true, "command": ["ame", "setup", "apply"]}}}
    ]}"#;

    fn list() -> List {
        parse_list(LIST_JSON).expect("the list parses")
    }

    fn argv(run: Result<Run, String>) -> Vec<String> {
        run.expect("allowed").argv
    }

    #[test]
    fn says_what_was_refused_in_the_windows_words() {
        let list = list();
        assert_eq!(argv(change(Some(&list), "greeting", "off")), ["ame", "desktop", "greeting", "off"]);
        assert_eq!(change(Some(&list), "greeting", "maybe"), Err(refused()));
        assert_eq!(new_widget(Some(&list), "My-clock", "hello"), Err(words(Refusal::WidgetName)));
        assert_ne!(words(Refusal::WidgetName), refused());
        assert_eq!(trust_image(Some(&list), "ghcr.io/you/image", Some("ghcr.io/iarsslen"), None), Err(words(Refusal::NoKey)));
    }

    #[test]
    fn says_the_last_lines_without_colours() {
        let output = Output { code: 1, stdout: "one\ntwo\n".into(), stderr: "\u{1b}[31merror:\u{1b}[0m three\n".into() };
        assert_eq!(said(&output), "one\ntwo\n\nerror: three");
        let long: String = (1..=20).map(|n| format!("{n}\n")).collect();
        assert_eq!(said(&Output { code: 0, stdout: long, stderr: String::new() }), (9..=20).map(|n| n.to_string()).collect::<Vec<_>>().join("\n"));
        assert_eq!(said(&Output::default()), "");
    }

    #[test]
    fn diffs_and_pages() {
        let output = |stdout: &str| Output { code: 0, stdout: stdout.into(), stderr: String::new() };
        assert_eq!(parse_diff(&output(r#"[{"area": "theme"}]"#)).unwrap().len(), 1);
        assert!(parse_diff(&output(r#"{"area": "theme"}"#)).unwrap().is_empty());
        assert!(parse_diff(&output("oops")).is_none());
        assert_eq!(page_argument(&["--gapplication-service".into(), "system".into()]), Some("system"));
        assert_eq!(page_argument(&["security".into()]), None);
        assert_eq!(page_argument(&["welcome".into()]), Some("welcome"));
        assert_eq!(page_argument(&[]), None);
        let list = list();
        assert_eq!(link_program("backups", Some(&list)), Some("amethystora-backups"));
        assert_eq!(link_program("backups", None), None);
        assert_eq!(link_program("manual", Some(&list)), None);
    }

    #[test]
    fn sees_an_expansion_in_its_own_field() {
        let abbrs = vec![";sig".to_string(), ";sig2".to_string()];
        // The shortcut and its space taken away, then the text and a space put in their place
        let waiting = tried("Hi ;sig2 ", "Hi ", None, &abbrs);
        assert_eq!(waiting, Tried::Waiting { abbr: ";sig2".into(), left: "Hi ".into() });
        assert_eq!(tried("Hi ", "Hi Second ", Some((";sig2", "Hi ")), &abbrs), Tried::Expanded { abbr: ";sig2".into(), text: "Second".into() });
        // Typing on, or deleting by hand, is no expansion
        assert_eq!(tried("Hi ;sig", "Hi ;sig ", None, &abbrs), Tried::Nothing);
        assert_eq!(tried("Hi ;sig ", "Hi ;sig", None, &abbrs), Tried::Nothing);
        assert_eq!(tried("Hi ", "Hi x", Some((";sig", "Hi ")), &abbrs), Tried::Nothing);
        assert_eq!(tried("Hi ", "Hi  ", Some((";sig", "Hi ")), &abbrs), Tried::Nothing);
        assert_eq!(first_line("Best wishes,\nSam"), "Best wishes,");
        assert_eq!(first_line(&"é".repeat(61)), format!("{}…", "é".repeat(60)));
        assert_eq!(first_line(&"é".repeat(60)), "é".repeat(60));
        assert_eq!(shortcut_problem("a b", "x"), Some(words(Refusal::ShortcutAbbr)));
        assert_eq!(shortcut_problem(";x", ""), Some(words(Refusal::ShortcutLength)));
        assert_eq!(shortcut_problem(";x", "\u{1b}[2J"), Some(words(Refusal::ShortcutCharacter)));
        assert_eq!(shortcut_problem(";x", "fine"), None);
    }
}
