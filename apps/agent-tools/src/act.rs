//! Level 2: the tools that change things, each asked for in the agent's own prompt every time.
//!
//!   change    one of Control's settings, by the entry's id and one of its values, with the list's own command
//!             (amethystora-control-core, which Control uses too); one marked terminal opens in a terminal
//!   software  Flatpak, Homebrew, or an amethystora-pkg container, in that order of preference
//!   run       a command for the user: as root through the root helper, in the administrator's mode, or in a
//!             terminal, where the user reads it and says yes
//!   undo      the recorded inverse of a change, through the same path
//!
//! Every change is recorded with what it was before (history.rs). A change that takes longer than a tool call
//! may is a job: the call returns its id after a while, and `history` with the id waits for the rest.

use std::time::Duration;

use serde_json::{Value, json};

use crate::handoff;
use crate::history::{self, Jobs};
use crate::never;
use crate::policy::{Policy, RootMode};
use crate::root::Root;
use crate::run::{self, Cache};
use crate::text;

/// How long a call waits for a job before it gives back the job's id
const WAIT: Duration = Duration::from_secs(40);
const BREW: &str = "/home/linuxbrew/.linuxbrew/bin/brew";
const PKG: &str = "/usr/bin/amethystora-pkg";

fn string<'a>(args: &'a Value, name: &str) -> Option<&'a str> {
    args.get(name).and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty())
}

/// What happened, for the agent: its output's end, cleaned and marked as data
fn outcome(output: &run::Output) -> String {
    let all = run::strip_ansi(&format!("{}\n{}", output.stdout, output.stderr));
    let lines: Vec<String> = run::tail(all.trim(), 2000).lines().map(str::to_string).collect();
    let how = match (output.timed_out, output.code) {
        (true, _) => "It took too long and was stopped.".to_string(),
        (_, Some(0)) => "It worked.".to_string(),
        (_, Some(code)) => format!("It failed with exit code {code}."),
        (_, None) => "It could not be started.".to_string(),
    };
    if lines.is_empty() { how } else { format!("{how}\n{}", text::data_block("What it printed", &lines)) }
}

/// Starts `work` as a job and waits a while for it: its result, or where to ask for it
fn job(jobs: &Jobs, id: &str, work: impl FnOnce() -> String + Send + 'static) -> String {
    jobs.start(id);
    let done = jobs.clone();
    let key = id.to_string();
    std::thread::spawn(move || {
        let said = work();
        done.finish(&key, said);
    });
    match jobs.wait(id, WAIT) {
        Some(said) => format!("{said}\n(record {id})"),
        None => format!("Still running, as {id}: call history with id {id} and wait to see how it ends."),
    }
}

// --- change -----------------------------------------------------------------------------------------------

pub fn change(cache: &Cache, jobs: &Jobs, args: &Value) -> Result<String, String> {
    let entry = string(args, "id").ok_or("change needs the id of a setting, from state settings")?;
    let value = string(args, "value").ok_or("change needs one of the setting's values, from state settings")?;
    change_to(cache, jobs, entry, value, None)
}

fn read_list(cache: &Cache) -> Result<amethystora_control_core::List, String> {
    cache.forget();
    let output = run::run(&[amethystora_control_core::LIST, "--json"], Duration::from_secs(90));
    amethystora_control_core::parse_list(&output.stdout).ok_or_else(|| "Control's list could not be read".to_string())
}

fn change_to(cache: &Cache, jobs: &Jobs, entry: &str, value: &str, undoing: Option<&str>) -> Result<String, String> {
    let list = read_list(cache)?;
    let item = list.find(entry).ok_or_else(|| format!("There is no setting {entry}: state settings lists them."))?.clone();
    let before = item.state.clone();
    let planned = amethystora_control_core::change(Some(&list), entry, value).map_err(|refusal| match refusal {
        amethystora_control_core::Refusal::Refused if !item.available => {
            format!("{} is not available here: {}", item.title, item.reason.clone().unwrap_or_default())
        }
        _ => format!("{value} is not one of {entry}'s values now: state settings lists them."),
    })?;
    // The list holds what the agent may never do too: its own lock and root mode, its own levels, the AUR
    if let Some(why) = never::refusal(&planned.argv) {
        return Err(format!("Refused: {why}. The user changes {} themselves, in Control.", item.title));
    }
    // The way back: the value it had, when that is one of its values
    let undo = before
        .as_deref()
        .filter(|before| *before != value)
        .filter(|before| item.kind == "range" || item.choices.iter().any(|choice| choice.value == *before))
        .map(|before| json!({"tool": "change", "entry": entry, "value": before}));
    let id = history::new_id();
    let mut record = history::started(&id, "change", json!({"entry": entry, "value": value, "before": before, "argv": planned.argv}));
    if let Some(undo) = &undo {
        record["undo"] = undo.clone();
    }
    if let Some(of) = undoing {
        record["of"] = json!(of);
    }
    if planned.terminal {
        record["result"] = json!("asking");
        history::record(&record)?;
        let mut done = record.clone();
        let stage = done.clone();
        handoff::open(
            jobs,
            &id,
            &planned.argv,
            &format!("Change the setting {} to {value}", item.title),
            handoff::Question { kind: "change", command: planned.argv.clone(), root: false, title: Some(item.title.clone()) },
            Box::new(move |phase| {
                let mut at = stage.clone();
                at["result"] = json!(phase);
                let _ = history::record(&at);
            }),
            Box::new(move |said| {
                done["result"] = json!(if said.contains("exited with 0") { "ok" } else if said.contains("declined") { "declined" } else if said.contains("Nobody answered") { "not run" } else { "failed" });
                done["time"] = json!(history::now());
                let _ = history::record(&done);
            }),
        )?;
        return Ok(format!(
            "This change asks the user something, so it was put to them (in the desktop's pill, or in a terminal) and runs after their yes ({id}). Call history with id {id} and wait to see how it ended."
        ));
    }
    let argv: Vec<&str> = planned.argv.iter().map(String::as_str).collect();
    let output = run::run(&argv, Duration::from_secs(300));
    record["result"] = json!(if output.ok() { "ok" } else { "failed" });
    history::record(&record)?;
    let now = read_list(cache).ok().and_then(|list| list.find(entry).and_then(|item| item.state.clone()));
    let mut said = format!("{} {}: {} (was {}).", item.title, if output.ok() { "is set" } else { "was not set" }, now.as_deref().unwrap_or(value),
        before.as_deref().unwrap_or("unknown"));
    if output.ok() {
        if undo.is_some() {
            said.push_str(&format!(" Record {id}; undo({id}) puts it back."));
        } else {
            said.push_str(&format!(" Record {id}."));
        }
    } else {
        said.push('\n');
        said.push_str(&outcome(&output));
    }
    Ok(said)
}

/// A plugin's `plugin change ENTRY VALUE` (plans/PLAN-plugins.md, the permissions: `change`), which the plugin host asks only for an
/// entry the plugin was granted and after a click on the plugin's own element: Control's entry set as the agent's change sets it, held
/// to the same never list and recorded with its way back, under the plugin's name. An entry that asks something in a terminal is
/// refused: a plugin cannot answer, and the person did not ask for the question
pub fn plugin_change(plugin: &str, entry: &str, value: &str) -> Result<String, String> {
    let cache = Cache::default();
    let list = read_list(&cache)?;
    let item = list.find(entry).ok_or_else(|| format!("there is no setting {entry}"))?.clone();
    let before = item.state.clone();
    let planned = amethystora_control_core::change(Some(&list), entry, value).map_err(|_| format!("{value} is not one of {entry}'s values now"))?;
    if planned.terminal {
        return Err(format!("{entry} asks something in a terminal, which a plugin cannot answer"));
    }
    if let Some(why) = never::refusal(&planned.argv) {
        return Err(format!("refused: {why}"));
    }
    let undo = before
        .as_deref()
        .filter(|before| *before != value)
        .filter(|before| item.kind == "range" || item.choices.iter().any(|choice| choice.value == *before))
        .map(|before| json!({"tool": "change", "entry": entry, "value": before}));
    let id = history::new_id();
    let mut record = history::started(&id, "change", json!({"entry": entry, "value": value, "before": before, "argv": planned.argv, "by": format!("plugin {plugin}")}));
    if let Some(undo) = &undo {
        record["undo"] = undo.clone();
    }
    let argv: Vec<&str> = planned.argv.iter().map(String::as_str).collect();
    let output = run::run(&argv, Duration::from_secs(120));
    record["result"] = json!(if output.ok() { "ok" } else { "failed" });
    history::record(&record)?;
    if output.ok() { Ok(format!("{} is {value}", item.title)) } else { Err(format!("{} was not set", item.title)) }
}

// --- software ---------------------------------------------------------------------------------------------

fn flatpak_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    id.len() <= 255
        && parts.len() >= 3
        && parts.iter().all(|part| {
            !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') && !part.starts_with(|c: char| c.is_ascii_digit())
        })
}

fn simple_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 128 && name.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '+' | '.' | '_' | '-' | ':'))
}

/// Which installation a Flatpak goes in, or comes out of: the account's own where it has Flathub as a remote of
/// its own (or the app is there), and otherwise the machine's, where the image's own recipes install, which asks
/// for no password unless the stricter flatpak-password is on
fn flatpak_installation(action: &str, name: &str) -> &'static str {
    let user_has = |argv: &[&str]| run::run(argv, Duration::from_secs(20)).ok();
    let mine = if action == "remove" {
        user_has(&["flatpak", "info", "--user", name])
    } else {
        run::run(&["flatpak", "remotes", "--user", "--columns=name"], Duration::from_secs(20)).stdout.lines().any(|line| line.trim() == "flathub")
    };
    if mine { "--user" } else { "--system" }
}

/// The argument vector for a software action, checked; and the action that undoes it
pub fn software_argv(action: &str, source: &str, name: &str, container: Option<&str>) -> Result<(Vec<String>, Option<&'static str>), String> {
    let words = |list: &[&str]| list.iter().map(|word| word.to_string()).collect::<Vec<String>>();
    let back = match action {
        "install" => Some("remove"),
        "remove" => Some("install"),
        "search" => None,
        _ => return Err("action is install, remove or search".into()),
    };
    if action == "search" {
        if name.is_empty() || name.len() > 100 || name.starts_with('-') {
            return Err("a search is 1 to 100 characters, not starting with -".into());
        }
    }
    let argv = match source {
        "flathub" => {
            if action != "search" && !flatpak_id(name) {
                return Err("a Flatpak is named by its id, such as org.gnome.Calculator".into());
            }
            let installation = flatpak_installation(action, name);
            match action {
                "install" => words(&["flatpak", "install", installation, "--noninteractive", "flathub", name]),
                "remove" => words(&["flatpak", "uninstall", installation, "--noninteractive", name]),
                _ => words(&["flatpak", "search", "--columns=application,name,description", name]),
            }
        }
        "brew" => {
            if !std::path::Path::new(BREW).exists() {
                return Err("Homebrew is not set up on this machine".into());
            }
            if action != "search" && (!simple_name(name) || name.contains(':') || name.contains('/')) {
                return Err("a Homebrew formula is named alone, without a tap".into());
            }
            match action {
                "install" => words(&[BREW, "install", name]),
                "remove" => words(&[BREW, "uninstall", name]),
                _ => words(&[BREW, "search", name]),
            }
        }
        "container" => {
            let container = container.filter(|c| c.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') && !c.is_empty() && !c.starts_with('-'))
                .ok_or("container names one of the containers state software lists")?;
            if action != "search" && !simple_name(name) {
                return Err("a package is named by its name in the container's own package manager".into());
            }
            match action {
                "install" => words(&[PKG, container, "install", "--yes", name]),
                "remove" => words(&[PKG, container, "remove", "--yes", name]),
                _ => words(&[PKG, container, "search", name]),
            }
        }
        _ => return Err("source is flathub (a Flatpak for this account), brew (Homebrew) or container (amethystora-pkg)".into()),
    };
    Ok((argv, back))
}

pub fn software(jobs: &Jobs, cache: &Cache, args: &Value, undoing: Option<&str>) -> Result<String, String> {
    let action = string(args, "action").ok_or("software needs an action: install, remove or search")?;
    let source = string(args, "source").ok_or("software needs a source: flathub, brew or container")?;
    let name = string(args, "id").or_else(|| string(args, "query")).ok_or("software needs an id (or a query to search)")?;
    let container = string(args, "container");
    let (argv, back) = software_argv(action, source, name, container)?;
    if action == "search" {
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        let output = run::run(&words, Duration::from_secs(120));
        let lines: Vec<String> = run::strip_ansi(&output.stdout).lines().take(25).map(str::to_string).collect();
        if lines.is_empty() {
            return Ok(format!("Nothing found.\n{}", outcome(&output)));
        }
        return Ok(text::data_block("Found (names and descriptions are their makers')", &lines));
    }
    let id = history::new_id();
    let mut record = history::started(&id, "software", json!({"action": action, "source": source, "name": name, "container": container, "argv": argv}));
    if let Some(back) = back {
        record["undo"] = json!({"tool": "software", "action": back, "source": source, "id": name, "container": container});
    }
    if let Some(of) = undoing {
        record["of"] = json!(of);
    }
    history::record(&record)?;
    cache.forget();
    Ok(job(jobs, &id, move || {
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        let output = run::run(&words, Duration::from_secs(1800));
        record["result"] = json!(if output.ok() { "ok" } else { "failed" });
        record["time"] = json!(history::now());
        let _ = history::record(&record);
        outcome(&output)
    }))
}

// --- run --------------------------------------------------------------------------------------------------

fn argv_of(args: &Value) -> Result<Vec<String>, String> {
    let list = args.get("argv").and_then(Value::as_array).ok_or("run needs argv, the command as a list of words")?;
    if list.is_empty() || list.len() > 64 {
        return Err("argv is 1 to 64 words".into());
    }
    list.iter()
        .map(|word| match word.as_str() {
            Some(word) if word.len() <= 4096 && !word.contains('\0') => Ok(word.to_string()),
            _ => Err("each word of argv is text of at most 4096 characters".to_string()),
        })
        .collect()
}

pub fn run_command(jobs: &Jobs, root: &std::sync::Arc<Root>, policy: &Policy, args: &Value) -> Result<String, String> {
    let argv = argv_of(args)?;
    let reason = string(args, "reason").ok_or("run needs a reason: one sentence the user reads before saying yes")?.to_string();
    let as_root = args.get("root").and_then(Value::as_bool).unwrap_or(false);
    if let Some(why) = never::refusal(&argv) {
        return Err(format!("Refused: {why}. Name the command for the user to run instead."));
    }
    let command = argv.clone();
    let id = history::new_id();
    let mode = policy.conf.root;
    // The root mode too, for the live pill: only `each` asks for a password every time
    let record = history::started(&id, "run", json!({"argv": argv, "root": as_root, "reason": text::field(&reason, 600), "mode": mode.word()}));
    if !as_root || mode == RootMode::Terminal {
        let mut shown = argv.clone();
        if as_root {
            shown = [vec!["sudo".to_string(), "--".to_string()], argv].concat();
        }
        let mut opened = record.clone();
        opened["result"] = json!("asking");
        history::record(&opened)?;
        let stage = opened.clone();
        handoff::open(
            jobs,
            &id,
            &shown,
            &reason,
            handoff::Question { kind: "run", command: command.clone(), root: as_root, title: None },
            Box::new(move |phase| {
                let mut at = stage.clone();
                at["result"] = json!(phase);
                let _ = history::record(&at);
            }),
            Box::new(move |said| {
                opened["result"] = json!(if said.contains("exited with 0") { "ok" } else if said.contains("declined") { "declined" } else if said.contains("Nobody answered") { "not run" } else { "failed" });
                opened["time"] = json!(history::now());
                let _ = history::record(&opened);
            }),
        )?;
        return Ok(format!(
            "Put to the user, who reads it and says yes (in the desktop's pill or in a terminal){} ({id}). Call history with id {id} and wait to see how it ended.",
            if as_root { " and types their password" } else { "" }
        ));
    }
    let mut running = record.clone();
    history::record(&running)?;
    let root = root.clone();
    Ok(job(jobs, &id, move || {
        let (result, said) = match root.run(mode, &running["id"].as_str().unwrap_or("").to_string(), &argv, &reason) {
            Ok(answer) if answer["refused"].is_string() => ("refused", format!("The root helper refused: {}", answer["refused"].as_str().unwrap_or(""))),
            Ok(answer) => {
                let output = run::Output {
                    code: answer["status"].as_i64().map(|code| code as i32),
                    stdout: answer["output"].as_str().unwrap_or("").to_string(),
                    stderr: String::new(),
                    timed_out: answer["timed_out"] == true,
                };
                (if output.ok() { "ok" } else { "failed" }, format!("As root: {}", outcome(&output)))
            }
            Err(error) => ("not run", error),
        };
        running["result"] = json!(result);
        running["time"] = json!(history::now());
        let _ = history::record(&running);
        said
    }))
}

// --- undo -------------------------------------------------------------------------------------------------

pub fn undo(jobs: &Jobs, cache: &Cache, args: &Value) -> Result<String, String> {
    let id = string(args, "id").ok_or("undo needs the id of a change, from history")?;
    let record = history::find(id).ok_or_else(|| format!("No change {id} was recorded."))?;
    if !matches!(record["result"].as_str(), Some("ok")) {
        return Err(format!("{id} did not change anything ({}), so there is nothing to undo.", record["result"].as_str().unwrap_or("?")));
    }
    let undo = record.get("undo").filter(|undo| undo.is_object()).ok_or_else(|| format!("{id} cannot be undone by the tools: it was a command."))?;
    // The record is the account's own file: its way back has to be the inverse of what it says it did, which is what
    // the person reads in the live pill, and only a record that did that way back counts as its undoing
    if !inverse(&record, undo) {
        return Err(format!("{id}'s way back is not the inverse of what it records, so it is not undone."));
    }
    if history::records().iter().any(|other| undoes(other, id, undo)) {
        return Err(format!("{id} was undone already."));
    }
    match undo["tool"].as_str() {
        Some("change") => change_to(cache, jobs, undo["entry"].as_str().unwrap_or(""), undo["value"].as_str().unwrap_or(""), Some(id)),
        Some("software") => software(jobs, cache, undo, Some(id)),
        _ => Err(format!("{id} has no way back the tools know.")),
    }
}

/// Whether a record's way back undoes exactly what it says it did: a setting set back to the value it had, the same app
/// taken out (or put back) from the same source
pub fn inverse(record: &Value, undo: &Value) -> bool {
    match (record["tool"].as_str(), undo["tool"].as_str()) {
        (Some("change"), Some("change")) => undo["entry"] == record["entry"] && undo["value"] == record["before"] && record["before"].is_string(),
        (Some("software"), Some("software")) => {
            let back = match record["action"].as_str() {
                Some("install") => "remove",
                Some("remove") => "install",
                _ => return false,
            };
            undo["action"] == back && undo["source"] == record["source"] && undo["id"] == record["name"] && undo["container"] == record["container"]
        }
        _ => false,
    }
}

/// Whether `other` is a finished undoing of the change `id`, whose way back is `undo`
pub fn undoes(other: &Value, id: &str, undo: &Value) -> bool {
    other["of"] == id && other["result"] == "ok" && other["tool"] == undo["tool"] && match undo["tool"].as_str() {
        Some("change") => other["entry"] == undo["entry"] && other["value"] == undo["value"],
        _ => other["action"] == undo["action"] && other["name"] == undo["id"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_takes_names_only() {
        let (argv, back) = software_argv("install", "flathub", "org.gnome.Calculator", None).unwrap();
        assert_eq!(argv[..2], ["flatpak", "install"]);
        assert!(argv[2] == "--user" || argv[2] == "--system");
        assert_eq!(argv[3..], ["--noninteractive", "flathub", "org.gnome.Calculator"]);
        assert_eq!(back, Some("remove"));
        assert!(software_argv("install", "flathub", "--system", None).is_err());
        assert!(software_argv("install", "flathub", "org.gnome", None).is_err());
        assert!(software_argv("install", "flathub", "org.gnome.Calc;rm", None).is_err());
        assert!(software_argv("install", "container", "htop", Some("../x")).is_err());
        assert!(software_argv("install", "container", "--force", Some("arch")).is_err());
        assert_eq!(software_argv("remove", "container", "htop", Some("arch")).unwrap().0, [PKG, "arch", "remove", "--yes", "htop"]);
        assert!(software_argv("search", "flathub", "-x", None).is_err());
        assert!(software_argv("layer", "flathub", "a.b.c", None).is_err());
        assert!(software_argv("install", "rpm", "htop", None).is_err());
    }

    #[test]
    fn a_way_back_is_the_inverse_of_the_record() {
        let change = json!({"tool": "change", "entry": "layout", "value": "classic", "before": "tiling"});
        assert!(inverse(&change, &json!({"tool": "change", "entry": "layout", "value": "tiling"})));
        assert!(!inverse(&change, &json!({"tool": "change", "entry": "theme", "value": "nord"})));
        assert!(!inverse(&change, &json!({"tool": "software", "action": "remove", "source": "flathub", "id": "a.b.c"})));
        let app = json!({"tool": "software", "action": "install", "source": "flathub", "name": "a.b.C", "container": null});
        assert!(inverse(&app, &json!({"tool": "software", "action": "remove", "source": "flathub", "id": "a.b.C", "container": null})));
        assert!(!inverse(&app, &json!({"tool": "software", "action": "remove", "source": "flathub", "id": "x.y.Z", "container": null})));
        let undo = json!({"tool": "change", "entry": "layout", "value": "tiling"});
        assert!(undoes(&json!({"of": "ab", "result": "ok", "tool": "change", "entry": "layout", "value": "tiling"}), "ab", &undo));
        assert!(!undoes(&json!({"of": "ab", "result": "ok"}), "ab", &undo));
    }

    #[test]
    fn run_takes_a_word_list() {
        assert!(argv_of(&json!({"argv": []})).is_err());
        assert!(argv_of(&json!({"argv": "rm -rf /"})).is_err());
        assert!(argv_of(&json!({"argv": ["a", 1]})).is_err());
        assert_eq!(argv_of(&json!({"argv": ["rpm-ostree", "status"]})).unwrap(), ["rpm-ostree", "status"]);
    }
}
