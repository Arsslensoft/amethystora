//! Little routines (R7-15): "when I join the office Wi-Fi, open Slack and mute", as one readable JSON file the agent writes and the
//! person keeps. A routine names one trigger (a time, a network, an app, the screen, the battery, a phone) and up to eight actions,
//! each an app to open or an `ame` command as words. Never a shell line, never root, never an `ame security` command, and never
//! anything the agent's own never list refuses: `check` holds a file to all of that, and so does every run again.
//!
//! What a routine names (a network, an app, a place in a time) tells a stranger about the person, so the whole store is **sealed**
//! under the Vault's `background` key (`amethystora-vault-client`, purpose `background`, record `routines`): the one key the Vault serves
//! with the vault closed and nobody asked, which is what lets a trigger run unattended. Closed with no such key, the routines wait,
//! and nothing here prompts. With no vault set up, none can be kept. The store is `~/.local/share/amethystora/routines/routines.avs`.
//!
//! The trigger side is Amethystora UI's `agent/routines.js`, which connects only the sources a routine names and calls `fire`. The agent
//! may `list`, `show` and `check`, and `pause`; `add`, `resume`, `remove`, `run` and `undo` are the person's (never.rs).

use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;
use std::time::Duration;

use amethystora_vault_client::{self as vault, Refusal};
use regex::Regex;
use serde_json::{Value, json};

use crate::never;
use crate::policy;
use crate::run;

/// The Vault's purpose that is served with the vault closed, and this store's record in it
pub const PURPOSE: &str = "background";
pub const RECORD: &str = "routines";
const MAX_ROUTINES: usize = 40;
const MAX_ACTIONS: usize = 8;
const MAX_WORDS: usize = 8;
/// What a file of a routine may be
const MAX_FILE: u64 = 16 * 1024;
const AME: &str = "/usr/bin/ame";
const LAUNCH: &str = "gtk-launch";
/// How long one `ame` action may take
const ACTION_TIME: Duration = Duration::from_secs(60);

pub fn dir() -> PathBuf {
    policy::home().join(".local/share/amethystora/routines")
}

pub fn store_path() -> PathBuf {
    dir().join("routines.avs")
}

// --- What a routine is ------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    /// A launcher's id to open
    pub open: Option<String>,
    /// An `ame` command as words
    pub ame: Vec<String>,
    /// The `ame` command that puts it back, if the author gave one
    pub undo: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Routine {
    pub id: String,
    pub name: String,
    /// The trigger, as `check` normalizes it: one of time, network, app, screen, battery, phone, and its extras
    pub when: Value,
    pub actions: Vec<Action>,
    pub while_locked: bool,
    pub paused: bool,
    /// Why it stopped by itself: a routine that fails says so once and stops
    pub failed: Option<String>,
}

static DESKTOP_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,119}$").unwrap());
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._:@+-]{0,79}$").unwrap());
static TIME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([01][0-9]|2[0-3]):[0-5][0-9]$").unwrap());
/// What reads as a literal secret: a key block, a name that says what it holds followed by its value, the shapes of the commonest
/// tokens, and any long run of token characters. A routine is a readable file in plain view; the Vault is where secrets go.
static SECRET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(-----BEGIN|(password|passwd|secret|token|api[-_ ]?key|bearer|authorization)\s*[=:]|\bgh[pousr]_[A-Za-z0-9]{20,}|github_pat_|\bsk-[A-Za-z0-9_-]{16,}|\bxox[abprs]-|\bAKIA[0-9A-Z]{16}|[A-Za-z0-9+/_-]{32,})",
    )
    .unwrap()
});

pub fn looks_secret(text: &str) -> bool {
    SECRET.is_match(text)
}

/// A name or a place a person typed: printable, one line, short, and no secret
fn shown_text(text: &str, longest: usize, what: &str) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > longest || text.chars().any(|c| c.is_control() || matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')) {
        return Err(format!("{what} is one printable line of at most {longest} characters"));
    }
    if looks_secret(text) {
        return Err(format!("{what} looks like a secret: a routine is a file anyone can read, and secrets belong in the Vault"));
    }
    Ok(text.to_string())
}

pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').chars().take(40).collect()
}

fn days_of(value: Option<&Value>) -> Result<Value, String> {
    const WEEK: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
    match value {
        None => Ok(json!("daily")),
        Some(Value::String(word)) if matches!(word.as_str(), "daily" | "weekdays" | "weekends") => Ok(json!(word)),
        Some(Value::Array(days)) if !days.is_empty() && days.len() <= 7 => {
            let mut kept: Vec<&str> = Vec::new();
            for day in days {
                let day = day.as_str().map(str::to_lowercase).ok_or("days are mon to sun")?;
                let day = WEEK.iter().find(|known| **known == day).ok_or("days are mon, tue, wed, thu, fri, sat, sun")?;
                if !kept.contains(day) {
                    kept.push(day);
                }
            }
            kept.sort_by_key(|day| WEEK.iter().position(|known| known == day));
            Ok(json!(kept))
        }
        _ => Err("days is daily, weekdays, weekends or a list of mon to sun".into()),
    }
}

fn text_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// The trigger, normalized: exactly one of the six, and what goes with it
fn parse_when(when: &Value, while_locked: bool) -> Result<Value, String> {
    let object = when.as_object().ok_or("when is an object with one trigger")?;
    const KINDS: [&str; 6] = ["time", "network", "app", "screen", "battery", "phone"];
    let named: Vec<&str> = KINDS.iter().copied().filter(|kind| object.contains_key(*kind)).collect();
    if named.len() != 1 {
        return Err("when names exactly one of time, network, app, screen, battery, phone".into());
    }
    let allowed: Vec<&str> = match named[0] {
        "time" => vec!["time", "days"],
        "app" => vec!["app", "state"],
        "phone" => vec!["phone", "name"],
        other => vec![other],
    };
    if let Some(extra) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("when has no {extra} for a {} trigger", named[0]));
    }
    match named[0] {
        "time" => {
            let at = text_of(when, "time").filter(|at| TIME.is_match(at)).ok_or("a time is written 18:00, from 00:00 to 23:59")?;
            Ok(json!({"time": at, "days": days_of(object.get("days"))?}))
        }
        "network" => Ok(json!({"network": shown_text(text_of(when, "network").unwrap_or(""), 32, "a network's name")?})),
        "app" => {
            let app = text_of(when, "app").filter(|app| DESKTOP_ID.is_match(app)).ok_or("an app is its launcher's id, such as org.gnome.Totem.desktop")?;
            let state = text_of(when, "state").unwrap_or("opens");
            if !matches!(state, "opens" | "closes" | "fullscreen") {
                return Err("an app's state is opens, closes or fullscreen".into());
            }
            Ok(json!({"app": app, "state": state}))
        }
        "screen" => match text_of(when, "screen") {
            Some("unlocks") => Ok(json!({"screen": "unlocks"})),
            // It runs while the screen is locked, which a routine may do only if it says so
            Some("locks") if while_locked => Ok(json!({"screen": "locks"})),
            Some("locks") => Err("a routine for when the screen locks runs while it is locked, which it says with \"while_locked\": true".into()),
            _ => Err("a screen trigger is unlocks or locks".into()),
        },
        "battery" => {
            let battery = text_of(when, "battery").unwrap_or("");
            let below = battery.strip_prefix("below ").and_then(|n| n.parse::<u32>().ok()).filter(|n| (5..=95).contains(n));
            match (battery, below) {
                ("on-battery" | "plugged-in", _) => Ok(json!({"battery": battery})),
                (_, Some(n)) => Ok(json!({"battery": format!("below {n}")})),
                _ => Err("a battery trigger is on-battery, plugged-in or below 5 to 95".into()),
            }
        }
        _ => {
            let phone = text_of(when, "phone").filter(|p| matches!(*p, "near" | "away")).ok_or("a phone trigger is near or away")?;
            let mut out = json!({"phone": phone});
            if let Some(name) = text_of(when, "name") {
                out["name"] = json!(shown_text(name, 40, "a phone's name")?);
            }
            Ok(out)
        }
    }
}

/// An `ame` command as words: each plain, no option, and nothing the agent's never list refuses
fn parse_words(value: &Value, what: &str) -> Result<Vec<String>, String> {
    let list = value.as_array().filter(|list| !list.is_empty() && list.len() <= MAX_WORDS).ok_or_else(|| format!("{what} is 1 to {MAX_WORDS} words"))?;
    let words: Vec<String> = list
        .iter()
        .map(|word| match word.as_str() {
            Some(word) if WORD.is_match(word) => Ok(word.to_string()),
            _ => Err(format!("each word of {what} is plain text of letters, digits and . _ : @ + -, and never starts with a dash")),
        })
        .collect::<Result<_, _>>()?;
    if words.iter().any(|word| looks_secret(word)) {
        return Err(format!("{what} holds a word that looks like a secret: secrets belong in the Vault"));
    }
    // Security stays in Security, the Vault and the agent's own levers are the person's, and a routine never starts a routine
    if matches!(words[0].as_str(), "security" | "vault" | "agent" | "plugin") {
        return Err(format!("{what} may not be an `ame {}` command: those are the person's own, never a routine's", words[0]));
    }
    let mut argv = vec!["ame".to_string()];
    argv.extend(words.iter().cloned());
    if let Some(why) = never::refusal(&argv) {
        return Err(format!("{what} is refused: {why}"));
    }
    Ok(words)
}

fn parse_action(action: &Value) -> Result<Action, String> {
    let object = action.as_object().ok_or("an action is an object: {\"open\": ...} or {\"ame\": [...]}")?;
    match (object.get("open"), object.get("ame")) {
        (Some(open), None) => {
            if object.len() != 1 {
                return Err("an open action has only open".into());
            }
            let id = open.as_str().filter(|id| DESKTOP_ID.is_match(id)).ok_or("open is an app's launcher id, such as org.gnome.Totem.desktop: never a path or a command")?;
            Ok(Action { open: Some(id.to_string()), ame: Vec::new(), undo: Vec::new() })
        }
        (None, Some(ame)) => {
            if let Some(extra) = object.keys().find(|key| !matches!(key.as_str(), "ame" | "undo")) {
                return Err(format!("an ame action has no {extra}"));
            }
            let undo = match object.get("undo") {
                Some(undo) => parse_words(undo, "undo")?,
                None => Vec::new(),
            };
            Ok(Action { open: None, ame: parse_words(ame, "an ame action")?, undo })
        }
        _ => Err("an action is either open or ame".into()),
    }
}

/// A routine as a file says it, checked. `exists` says whether an `ame` command is one the image has (a test stands in for it).
pub fn parse(file: &Value, exists: &dyn Fn(&[String]) -> bool) -> Result<Routine, String> {
    let object = file.as_object().ok_or("a routine is one JSON object")?;
    if let Some(extra) = object.keys().find(|key| !matches!(key.as_str(), "name" | "when" | "do" | "while_locked" | "paused" | "failed")) {
        return Err(format!("a routine has no {extra}: name, when, do, while_locked"));
    }
    let name = shown_text(text_of(file, "name").unwrap_or(""), 60, "name")?;
    let id = slug(&name);
    if id.is_empty() {
        return Err("name has no letters or digits".into());
    }
    let while_locked = match object.get("while_locked") {
        None => false,
        Some(Value::Bool(flag)) => *flag,
        Some(_) => return Err("while_locked is true or false".into()),
    };
    let when = parse_when(object.get("when").ok_or("a routine needs when")?, while_locked)?;
    let list = object.get("do").and_then(Value::as_array).filter(|list| !list.is_empty() && list.len() <= MAX_ACTIONS).ok_or("do is a list of 1 to 8 actions")?;
    let actions = list.iter().map(parse_action).collect::<Result<Vec<_>, _>>()?;
    for action in &actions {
        for words in [&action.ame, &action.undo] {
            if !words.is_empty() && !exists(words) {
                return Err(format!("`ame {}` is not a command this image has", words.join(" ")));
            }
        }
    }
    Ok(Routine {
        id,
        name,
        when,
        actions,
        while_locked,
        paused: object.get("paused").and_then(Value::as_bool).unwrap_or(false),
        failed: object.get("failed").and_then(Value::as_str).map(|why| why.chars().take(160).collect()),
    })
}

impl Routine {
    /// The routine as it is stored and shown: the file's shape, normalized
    pub fn to_value(&self) -> Value {
        let actions: Vec<Value> = self
            .actions
            .iter()
            .map(|action| match &action.open {
                Some(open) => json!({"open": open}),
                None if action.undo.is_empty() => json!({"ame": action.ame}),
                None => json!({"ame": action.ame, "undo": action.undo}),
            })
            .collect();
        let mut out = json!({"name": self.name, "when": self.when, "do": actions});
        if self.while_locked {
            out["while_locked"] = json!(true);
        }
        if self.paused {
            out["paused"] = json!(true);
        }
        if let Some(why) = &self.failed {
            out["failed"] = json!(why);
        }
        out
    }

    /// When it runs, in a sentence
    pub fn when_words(&self) -> String {
        let text = |key| self.when[key].as_str().unwrap_or("");
        if self.when.get("time").is_some() {
            let days = match &self.when["days"] {
                Value::String(word) if word == "weekdays" => " on weekdays".to_string(),
                Value::String(word) if word == "weekends" => " on weekends".to_string(),
                Value::Array(days) => format!(" on {}", days.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")),
                _ => String::new(),
            };
            return format!("At {}{days}", text("time"));
        }
        if self.when.get("network").is_some() {
            return format!("When I join {}", text("network"));
        }
        if self.when.get("app").is_some() {
            let what = match text("state") {
                "closes" => "closes",
                "fullscreen" => "goes fullscreen",
                _ => "opens",
            };
            return format!("When {} {what}", text("app"));
        }
        if self.when.get("screen").is_some() {
            return format!("When the screen {}", text("screen"));
        }
        if self.when.get("battery").is_some() {
            return match text("battery") {
                "on-battery" => "When I unplug".into(),
                "plugged-in" => "When I plug in".into(),
                below => format!("When the battery goes {below}%"),
            };
        }
        let name = self.when["name"].as_str().map(|name| format!(" ({name})")).unwrap_or_default();
        format!("When my phone is {}{name}", if text("phone") == "near" { "near" } else { "gone" })
    }

    /// What it does, one line each
    pub fn action_words(&self) -> Vec<String> {
        self.actions
            .iter()
            .map(|action| match &action.open {
                Some(open) => format!("Open {open}"),
                None if action.undo.is_empty() => format!("Run ame {}", action.ame.join(" ")),
                None => format!("Run ame {} (back: ame {})", action.ame.join(" "), action.undo.join(" ")),
            })
            .collect()
    }

    pub fn summary(&self) -> Value {
        json!({"id": self.id, "name": self.name, "when": self.when, "when_words": self.when_words(), "do": self.action_words(),
            "paused": self.paused, "failed": self.failed, "while_locked": self.while_locked,
            "undoable": self.actions.iter().all(|a| a.open.is_some() || !a.undo.is_empty()) && self.actions.iter().any(|a| !a.undo.is_empty())})
    }
}

/// Whether the image has this `ame` command, as `ame --show` says
pub fn ame_has(words: &[String]) -> bool {
    let path = match words {
        [one] => one.clone(),
        [group, recipe, ..] => format!("{group}::{recipe}"),
        [] => return false,
    };
    run::run(&[AME, "--show", &path], Duration::from_secs(20)).ok()
}

// --- The sealed store ---------------------------------------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub enum Trouble {
    /// No vault: nothing can be kept
    NotSetUp,
    /// The vault is closed and no background key is there
    Closed,
    Other(String),
}

impl Trouble {
    pub fn word(&self) -> &'static str {
        match self {
            Trouble::NotSetUp => "not-set-up",
            Trouble::Closed => "closed",
            Trouble::Other(_) => "error",
        }
    }

    pub fn say(&self) -> String {
        match self {
            Trouble::NotSetUp => "Routines are kept sealed in the Vault, which is not set up on this account: ame vault setup".into(),
            Trouble::Closed => "The Vault is closed, and its background key is off, so the routines wait: open the Vault, or turn the background key on (ame vault methods background on)".into(),
            Trouble::Other(why) => why.clone(),
        }
    }
}

fn trouble(refusal: Refusal) -> Trouble {
    match refusal {
        Refusal::NotSetUp => Trouble::NotSetUp,
        Refusal::Closed => Trouble::Closed,
        other => Trouble::Other(format!("the Vault said {}", other.name())),
    }
}

/// Every routine kept, or an empty list when none was ever made (the Vault is not asked then)
pub fn load_at(socket: &Path, file: &Path) -> Result<Vec<Routine>, Trouble> {
    let sealed = match std::fs::read(file) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(Trouble::Other(format!("{}: {error}", file.display()))),
    };
    let plain = vault::open_at(socket, PURPOSE, RECORD, &sealed).map_err(trouble)?;
    let body: Value = serde_json::from_slice(&plain).map_err(|_| Trouble::Other("the routines could not be read".into()))?;
    let list = body["routines"].as_array().ok_or_else(|| Trouble::Other("the routines could not be read".into()))?;
    // What was kept was checked when it was kept; it is checked again for what runs (never list, words), `ame` commands aside
    list.iter().map(|one| parse(one, &|_| true).map_err(Trouble::Other)).collect()
}

pub fn save_at(socket: &Path, file: &Path, routines: &[Routine]) -> Result<(), Trouble> {
    if routines.len() > MAX_ROUTINES {
        return Err(Trouble::Other(format!("at most {MAX_ROUTINES} routines")));
    }
    let body = json!({"v": 1, "routines": routines.iter().map(Routine::to_value).collect::<Vec<_>>()});
    let sealed = vault::seal_at(socket, PURPOSE, RECORD, body.to_string().as_bytes()).map_err(trouble)?;
    let folder = file.parent().ok_or_else(|| Trouble::Other("no folder".into()))?;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(folder).map_err(|error| Trouble::Other(error.to_string()))?;
    let new = file.with_extension("new");
    let written = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&new)
        .and_then(|mut out| out.write_all(&sealed))
        .and_then(|()| std::fs::rename(&new, file));
    written.map_err(|error| Trouble::Other(format!("{}: {error}", file.display())))
}

fn load() -> Result<Vec<Routine>, Trouble> {
    load_at(Path::new(vault::SOCKET), &store_path())
}

fn save(routines: &[Routine]) -> Result<(), Trouble> {
    save_at(Path::new(vault::SOCKET), &store_path(), routines)
}

// --- Running ---------------------------------------------------------------------------------------------------------------------

/// What one run did, as the pill says it: each action by its words, whether it worked, and why the routine stopped if it did
pub struct Ran {
    pub results: Vec<Value>,
    pub failed: Option<String>,
}

/// Runs a routine's actions in order, stopping at the first that fails. `ame` is the program to run `ame` actions with and
/// `launch` the one that opens an app, so that a test can stand in for both.
pub fn run_actions(routine: &Routine, ame: &str, launch: &str) -> Ran {
    let mut results = Vec::new();
    let mut failed = None;
    for action in &routine.actions {
        let (kind, what, argv): (&str, String, Vec<String>) = match &action.open {
            Some(open) => ("open", open.clone(), vec![launch.to_string(), open.clone()]),
            None => ("ame", format!("ame {}", action.ame.join(" ")), std::iter::once(ame.to_string()).chain(action.ame.iter().cloned()).collect()),
        };
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        let output = run::run_env(&words, &[], ACTION_TIME);
        let ok = output.ok();
        results.push(json!({"kind": kind, "what": what, "ok": ok, "undo": !action.undo.is_empty()}));
        if !ok {
            failed = Some(if output.timed_out { format!("{what} took too long") } else { format!("{what} did not work") });
            break;
        }
    }
    Ran { results, failed }
}

/// Puts back what a routine's actions say how to, newest first: its `ame` commands' own way back. The apps it opened are the
/// shell's to close, which it knew were not running before.
pub fn undo_actions(routine: &Routine, ame: &str) -> Vec<Value> {
    let mut results = Vec::new();
    for action in routine.actions.iter().rev().filter(|action| !action.undo.is_empty()) {
        let argv: Vec<&str> = std::iter::once(ame).chain(action.undo.iter().map(String::as_str)).collect();
        let output = run::run_env(&argv, &[], ACTION_TIME);
        results.push(json!({"what": format!("ame {}", action.undo.join(" ")), "ok": output.ok()}));
    }
    results
}

fn find<'a>(routines: &'a mut [Routine], id: &str) -> Result<&'a mut Routine, String> {
    routines.iter_mut().find(|routine| routine.id == id).ok_or_else(|| format!("There is no routine {id}: ame agent routines list names them."))
}

// --- The command ---------------------------------------------------------------------------------------------------------------

fn read_file(path: &str) -> Result<Value, String> {
    let meta = std::fs::metadata(path).map_err(|error| format!("{path}: {error}"))?;
    if !meta.is_file() || meta.len() > MAX_FILE {
        return Err(format!("{path} is not a file of at most {MAX_FILE} bytes"));
    }
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("{path} is not JSON: {error}"))
}

fn print_routine(routine: &Routine) {
    println!("{}", routine.name);
    println!("  {}", routine.when_words());
    for line in routine.action_words() {
        println!("  - {line}");
    }
    println!("  It runs as you, never as root, and {}.", if routine.while_locked { "also while the screen is locked" } else { "never while the screen is locked" });
}

fn ask_yes() -> bool {
    if unsafe { libc::isatty(0) } != 1 {
        return false;
    }
    print!("Keep it? [y/N] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).is_ok() && answer.trim().to_lowercase().starts_with('y')
}

fn fail(text: impl AsRef<str>) -> ExitCode {
    eprintln!("{}", text.as_ref());
    ExitCode::FAILURE
}

fn trouble_exit(json_out: bool, trouble: &Trouble) -> ExitCode {
    if json_out {
        println!("{}", json!({"state": trouble.word(), "said": trouble.say()}));
        ExitCode::SUCCESS
    } else {
        fail(trouble.say())
    }
}

/// `amethystora-agent-tools routines ...`
pub fn main(args: &[String]) -> ExitCode {
    let json_out = args.iter().any(|arg| arg == "--json");
    let words: Vec<&str> = args.iter().map(String::as_str).filter(|arg| *arg != "--json").collect();
    match words.as_slice() {
        // A file's check, without keeping it
        ["check", file] => match read_file(file).and_then(|value| parse(&value, &ame_has)) {
            Ok(routine) if json_out => {
                println!("{}", json!({"ok": true, "routine": routine.summary()}));
                ExitCode::SUCCESS
            }
            Ok(routine) => {
                print_routine(&routine);
                println!("It passes the check.");
                ExitCode::SUCCESS
            }
            Err(why) if json_out => {
                println!("{}", json!({"ok": false, "why": why}));
                ExitCode::FAILURE
            }
            Err(why) => fail(why),
        },
        // What the shell connects: nothing for no store (the Vault is not asked), the paused ones marked
        ["triggers"] => {
            if !store_path().exists() {
                println!("{}", json!({"state": "none", "routines": []}));
                return ExitCode::SUCCESS;
            }
            match load() {
                Ok(routines) => {
                    let listed: Vec<Value> = routines.iter().map(Routine::summary).collect();
                    println!("{}", json!({"state": "ok", "routines": listed}));
                    ExitCode::SUCCESS
                }
                Err(trouble) => trouble_exit(true, &trouble),
            }
        }
        ["list"] => match load() {
            Ok(routines) if json_out => {
                println!("{}", json!({"state": "ok", "routines": routines.iter().map(Routine::summary).collect::<Vec<_>>()}));
                ExitCode::SUCCESS
            }
            Ok(routines) if routines.is_empty() => {
                println!("No routines yet. Ask the agent to write one, then: ame agent routines add FILE");
                ExitCode::SUCCESS
            }
            Ok(routines) => {
                for routine in &routines {
                    let state = match (&routine.failed, routine.paused) {
                        (Some(why), _) => format!(" (stopped: {why})"),
                        (None, true) => " (paused)".to_string(),
                        _ => String::new(),
                    };
                    println!("{}: {} - {}{state}", routine.id, routine.name, routine.when_words());
                }
                ExitCode::SUCCESS
            }
            Err(trouble) => trouble_exit(json_out, &trouble),
        },
        ["show", id] => match load() {
            Ok(routines) => match routines.iter().find(|routine| routine.id == *id) {
                Some(routine) => {
                    println!("{}", serde_json::to_string_pretty(&routine.to_value()).unwrap_or_default());
                    ExitCode::SUCCESS
                }
                None => fail(format!("There is no routine {id}")),
            },
            Err(trouble) => fail(trouble.say()),
        },
        // Keeping one is the person's, in a terminal, after reading what it does: there is no --yes
        ["add", file] => {
            let routine = match read_file(file).and_then(|value| parse(&value, &ame_has)) {
                Ok(routine) => routine,
                Err(why) => return fail(why),
            };
            print_routine(&routine);
            let mut routines = match load() {
                Ok(routines) => routines,
                Err(trouble) => return fail(trouble.say()),
            };
            if routines.iter().any(|kept| kept.id == routine.id) {
                return fail(format!("A routine named {} is kept already: remove it first, or give this one another name.", routine.name));
            }
            if !ask_yes() {
                return fail("Not kept: a routine is kept only after you say yes in a terminal.");
            }
            routines.push(routine.clone());
            match save(&routines) {
                Ok(()) => {
                    println!("Kept as {}. It is sealed in the Vault.", routine.id);
                    ExitCode::SUCCESS
                }
                Err(trouble) => fail(trouble.say()),
            }
        }
        [verb @ ("pause" | "resume" | "remove"), id] => {
            let mut routines = match load() {
                Ok(routines) => routines,
                Err(trouble) => return fail(trouble.say()),
            };
            let changed = match *verb {
                "remove" => {
                    let before = routines.len();
                    routines.retain(|routine| routine.id != *id);
                    routines.len() != before
                }
                _ => match find(&mut routines, id) {
                    Ok(routine) => {
                        routine.paused = *verb == "pause";
                        // Starting again is a fresh start: what stopped it is let go of
                        routine.failed = None;
                        true
                    }
                    Err(_) => false,
                },
            };
            if !changed {
                return fail(format!("There is no routine {id}"));
            }
            match save(&routines) {
                Ok(()) => {
                    println!("{id}: {}", match *verb { "pause" => "paused", "resume" => "running", _ => "removed" });
                    ExitCode::SUCCESS
                }
                Err(trouble) => fail(trouble.say()),
            }
        }
        // The shell's and the person's run of one routine (a trigger fired, or ame agent routines run), with what it did as JSON
        ["fire", id] | ["run", id] => {
            let mut routines = match load() {
                Ok(routines) => routines,
                Err(trouble) => return trouble_exit(json_out, &trouble),
            };
            let routine = match find(&mut routines, id) {
                Ok(routine) => routine.clone(),
                Err(why) => return fail(why),
            };
            // Only `run` by hand starts a paused one; a trigger finds it paused
            if routine.paused && words[0] == "fire" {
                println!("{}", json!({"state": "paused", "id": routine.id}));
                return ExitCode::SUCCESS;
            }
            let ran = run_actions(&routine, AME, LAUNCH);
            if let Some(why) = &ran.failed {
                // A routine that fails says so once and stops
                if let Ok(routine) = find(&mut routines, id) {
                    routine.paused = true;
                    routine.failed = Some(why.clone());
                }
                let _ = save(&routines);
            }
            let said = json!({"state": "ran", "id": routine.id, "name": routine.name, "results": ran.results, "failed": ran.failed,
                "undoable": routine.actions.iter().any(|a| !a.undo.is_empty())});
            if json_out { println!("{said}") } else { println!("{}: {}", routine.name, ran.failed.as_deref().unwrap_or("done")) }
            ExitCode::SUCCESS
        }
        ["undo", id] => {
            let routines = match load() {
                Ok(routines) => routines,
                Err(trouble) => return trouble_exit(json_out, &trouble),
            };
            let Some(routine) = routines.iter().find(|routine| routine.id == *id) else { return fail(format!("There is no routine {id}")) };
            let results = undo_actions(routine, AME);
            println!("{}", json!({"state": "undone", "id": routine.id, "results": results}));
            ExitCode::SUCCESS
        }
        _ => fail("usage: amethystora-agent-tools routines check FILE | list | show ID | add FILE | pause|resume|remove ID | run ID | undo ID"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    fn yes(_: &[String]) -> bool {
        true
    }

    fn office() -> Value {
        json!({"name": "Office morning", "when": {"network": "Office Wi-Fi"},
            "do": [{"open": "com.slack.Slack.desktop"}, {"ame": ["desktop", "volume", "mute"], "undo": ["desktop", "volume", "unmute"]}]})
    }

    fn refused(value: Value) -> String {
        parse(&value, &yes).expect_err("it should be refused")
    }

    #[test]
    fn the_boards_routine_passes_and_reads_in_words() {
        let routine = parse(&office(), &yes).unwrap();
        assert_eq!(routine.id, "office-morning");
        assert_eq!(routine.when_words(), "When I join Office Wi-Fi");
        assert_eq!(routine.action_words()[0], "Open com.slack.Slack.desktop");
        assert_eq!(routine.action_words()[1], "Run ame desktop volume mute (back: ame desktop volume unmute)");
        // What is stored reads back the same
        assert_eq!(parse(&routine.to_value(), &yes).unwrap(), routine);
    }

    #[test]
    fn the_six_triggers() {
        let at = |when: Value| parse(&json!({"name": "x", "when": when, "do": [{"open": "a.desktop"}]}), &yes);
        assert_eq!(at(json!({"time": "18:00", "days": "weekdays"})).unwrap().when_words(), "At 18:00 on weekdays");
        assert_eq!(at(json!({"time": "07:30", "days": ["Tue", "mon"]})).unwrap().when_words(), "At 07:30 on mon, tue");
        assert!(at(json!({"time": "24:00"})).is_err());
        assert_eq!(at(json!({"app": "org.gnome.Totem.desktop", "state": "fullscreen"})).unwrap().when_words(), "When org.gnome.Totem.desktop goes fullscreen");
        assert_eq!(at(json!({"battery": "below 20"})).unwrap().when_words(), "When the battery goes below 20%");
        assert!(at(json!({"battery": "below 2"})).is_err());
        assert_eq!(at(json!({"battery": "on-battery"})).unwrap().when_words(), "When I unplug");
        assert_eq!(at(json!({"phone": "away", "name": "Pixel"})).unwrap().when_words(), "When my phone is gone (Pixel)");
        assert_eq!(at(json!({"screen": "unlocks"})).unwrap().when_words(), "When the screen unlocks");
        // One trigger only, and nothing it does not take
        assert!(at(json!({"network": "a", "time": "10:00"})).is_err());
        assert!(at(json!({"network": "a", "days": "daily"})).is_err());
        assert!(at(json!({})).is_err());
        // A screen that locks runs while it is locked, which has to be said
        assert!(at(json!({"screen": "locks"})).is_err());
        let locked = parse(&json!({"name": "x", "when": {"screen": "locks"}, "while_locked": true, "do": [{"ame": ["desktop", "volume", "mute"]}]}), &yes).unwrap();
        assert!(locked.while_locked);
    }

    #[test]
    fn never_a_shell_line_root_or_security() {
        let with = |action: Value| parse(&json!({"name": "x", "when": {"network": "n"}, "do": [action]}), &yes);
        // Apps by launcher id only
        assert!(with(json!({"open": "/usr/bin/evil"})).is_err());
        assert!(with(json!({"open": "a.desktop; rm"})).is_err());
        assert!(with(json!({"open": "a b"})).is_err());
        // Words, not a line
        assert!(with(json!({"ame": "desktop volume mute"})).is_err());
        assert!(with(json!({"ame": ["desktop volume mute"]})).is_err());
        assert!(with(json!({"ame": ["desktop", "$(id)"]})).is_err());
        assert!(with(json!({"ame": ["desktop", "--command=x"]})).is_err());
        assert!(with(json!({"ame": ["desktop", ";", "id"]})).is_err());
        assert!(with(json!({"sh": ["id"]})).is_err());
        assert!(with(json!({"ame": ["desktop", "x"], "open": "a.desktop"})).is_err());
        // Never root, and never the person's own levers
        // `ame` is the only program a routine runs, so there is no `sudo` to name: a routine runs as the account, and a recipe that
        // needs root asks for a password a routine cannot give
        for group in ["security", "vault", "agent", "plugin"] {
            assert!(with(json!({"ame": [group, "status"]})).is_err(), "{group}");
        }
        assert!(with(json!({"ame": ["desktop", "volume", "mute"], "undo": ["security", "usb", "off"]})).is_err());
        assert!(with(json!({"ame": vec!["a"; 9]})).is_err());
        // The duress feature is refused by the agent's never list, whatever it is called
        assert!(with(json!({"ame": ["system", "duress-x"]})).is_err());
        // Too many actions, and none
        assert!(parse(&json!({"name": "x", "when": {"network": "n"}, "do": vec![json!({"open": "a.desktop"}); 9]}), &yes).is_err());
        assert!(parse(&json!({"name": "x", "when": {"network": "n"}, "do": []}), &yes).is_err());
        // An undo belongs to an ame action
        assert!(with(json!({"open": "a.desktop", "undo": ["desktop", "x"]})).is_err());
    }

    #[test]
    fn a_literal_secret_is_refused_wherever_it_is() {
        assert!(looks_secret("password=hunter2"));
        assert!(looks_secret("ghp_0123456789abcdef0123456789abcdef0123"));
        assert!(looks_secret("-----BEGIN OPENSSH PRIVATE KEY-----"));
        assert!(looks_secret("sk-ant-0123456789abcdef0123"));
        assert!(looks_secret("AKIAABCDEFGHIJKLMNOP"));
        assert!(looks_secret("abcdefghijklmnopqrstuvwxyz0123456789"));
        assert!(!looks_secret("Office Wi-Fi"));
        assert!(!looks_secret("desktop"));
        assert!(refused(json!({"name": "token: abc123", "when": {"network": "n"}, "do": [{"open": "a.desktop"}]})).contains("secret"));
        assert!(refused(json!({"name": "x", "when": {"network": "password=hunter2"}, "do": [{"open": "a.desktop"}]})).contains("secret"));
        assert!(refused(json!({"name": "x", "when": {"network": "n"}, "do": [{"ame": ["desktop", "ghp_0123456789abcdef0123456789abcdef0123"]}]})).contains("secret"));
        // A long network name is no secret and no network name either
        assert!(refused(json!({"name": "x", "when": {"network": "n".repeat(40)}, "do": [{"open": "a.desktop"}]})).contains("32"));
    }

    #[test]
    fn a_command_the_image_lacks_is_said() {
        let none = |_: &[String]| false;
        assert!(parse(&office(), &none).unwrap_err().contains("not a command this image has"));
        assert!(parse(&json!({"name": "x", "when": {"network": "n"}, "do": [{"open": "a.desktop"}]}), &none).is_ok());
    }

    #[test]
    fn the_store_is_sealed_through_the_vault_and_only_asked_when_there_is_one() {
        let dir = std::env::temp_dir().join(format!("amethystora-routines-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("vault.sock");
        let file = dir.join("routines").join("routines.avs");
        // No store: nothing is asked of any vault (there is none here)
        assert_eq!(load_at(&socket, &file), Ok(Vec::new()));
        // A vault of this test: it seals by reversing the bytes and says what it was asked, so that the purpose is seen
        let listener = UnixListener::bind(&socket).unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        let thread = std::thread::spawn(move || {
            for _ in 0..2 {
                let (stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                log.lock().unwrap().push((request["op"].as_str().unwrap().to_string(), request["purpose"].as_str().unwrap().to_string(), request["record"].as_str().unwrap().to_string()));
                let data = request["data"].as_str().unwrap();
                use base64::Engine;
                let engine = base64::engine::general_purpose::STANDARD;
                let mut bytes = engine.decode(data).unwrap();
                bytes.reverse();
                let mut stream = &stream;
                writeln!(stream, "{}", json!({"ok": true, "data": engine.encode(bytes)})).unwrap();
            }
        });
        let routine = parse(&office(), &yes).unwrap();
        save_at(&socket, &file, std::slice::from_ref(&routine)).unwrap();
        // What is on disk is not the plain text
        let on_disk = std::fs::read(&file).unwrap();
        assert!(!String::from_utf8_lossy(&on_disk).contains("Office"));
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(load_at(&socket, &file), Ok(vec![routine]));
        thread.join().unwrap();
        // The only purpose served with the vault closed, under this store's own record
        assert_eq!(*seen.lock().unwrap(), [("seal".to_string(), "background".to_string(), "routines".to_string()), ("open".to_string(), "background".to_string(), "routines".to_string())]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_closed_vault_waits_and_a_missing_one_says_so() {
        assert_eq!(trouble(Refusal::Closed), Trouble::Closed);
        assert_eq!(trouble(Refusal::NotSetUp), Trouble::NotSetUp);
        assert!(Trouble::Closed.say().contains("background key"));
        assert_eq!(Trouble::NotSetUp.word(), "not-set-up");
    }

    #[test]
    fn a_run_stops_at_the_first_action_that_fails_and_undo_goes_backwards() {
        let routine = parse(
            &json!({"name": "x", "when": {"network": "n"}, "do": [
                {"ame": ["a"], "undo": ["a-back"]}, {"ame": ["fail"]}, {"ame": ["never"], "undo": ["never-back"]}]}),
            &yes,
        )
        .unwrap();
        // `true` and `false` stand in for the programs that open an app and that run `ame`: `false` fails whatever it is given
        let ran = run_actions(&routine, "true", "true");
        assert!(ran.failed.is_none());
        assert_eq!(ran.results.len(), 3);
        let ran = run_actions(&routine, "false", "true");
        assert_eq!(ran.results.len(), 1);
        assert!(ran.failed.unwrap().contains("did not work"));
        let back = undo_actions(&routine, "true");
        assert_eq!(back.iter().map(|r| r["what"].as_str().unwrap()).collect::<Vec<_>>(), ["ame never-back", "ame a-back"]);
    }
}
