//! The commands of profiles and sandboxes, which `ame agent profile ...` and `ame agent sandbox ...` run through
//! amethystora-agent. What asks the person (a removal, a permission) asks in the recipe's terminal before it comes here.
//!
//!   profile list [--json] | add NAME | use NAME [FOLDER] | which [FOLDER] [--agent AGENT] | running [--json]
//!   profile in-use NAME | ask NAME | ask-profile | root NAME | env --shell bash|zsh|fish [--dir DIR]
//!   sandbox list [--json] | use FOLDER [open|closed] | off FOLDER | allow FOLDER PERMISSION | disallow FOLDER PERMISSION
//!   sandbox cost PERMISSION [FOLDER] | show ... (agent-plan's options)

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::agents::{self, Map, Table, Way, DEFAULT};
use crate::sandbox::{self, Permission};

fn table() -> Result<Table, String> {
    Table::load()
}

fn folder_arg(arg: Option<&String>) -> Result<PathBuf, String> {
    match arg {
        Some(folder) => agents::real_folder(Path::new(folder)),
        None => std::env::current_dir().map_err(|error| error.to_string()).and_then(|dir| agents::real_folder(&dir)),
    }
}

fn out(result: Result<String, String>) -> i32 {
    match result {
        Ok(text) => {
            if !text.is_empty() {
                println!("{text}");
            }
            0
        }
        Err(why) => {
            eprintln!("{why}");
            1
        }
    }
}

/// A word for a shell, quoted as that shell reads it
fn quoted(word: &str) -> String {
    format!("'{}'", word.replace('\'', "'\\''"))
}

pub fn profile(args: &[String]) -> i32 {
    let home = agents::real_home();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["list"] | [] | ["list", "--json"] => out(table().and_then(|table| agents::list(&table, &home)).map(|listed| {
            if words.contains(&"--json") {
                return listed.to_string();
            }
            let mut lines = Vec::new();
            for row in listed["profiles"].as_array().into_iter().flatten() {
                let agents: Vec<&str> = row["agents"].as_array().into_iter().flatten().filter_map(Value::as_str).map(agents::title).collect();
                let folders: Vec<&str> = row["folders"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                lines.push(format!(
                    "{}{}: {}{}",
                    row["name"].as_str().unwrap_or("?"),
                    if row["ask"] == true { " (the pill's questions)" } else { "" },
                    if agents.is_empty() { "no agent signed in".to_string() } else { format!("signed in to {}", agents.join(", ")) },
                    if folders.is_empty() { String::new() } else { format!("; folders {}", folders.join(", ")) }
                ));
            }
            lines.join("\n")
        })),
        ["add", name] => out(agents::add(&home, name)),
        ["use", name] => out(folder_arg(None).and_then(|folder| agents::use_folder(&home, name, &folder))),
        ["use", name, folder] => out(agents::use_folder(&home, name, &PathBuf::from(folder))),
        ["which", rest @ ..] => {
            let mut folder = None;
            let mut agent = None;
            let mut at = 0;
            while at < rest.len() {
                if rest[at] == "--agent" {
                    agent = rest.get(at + 1).map(|agent| agent.to_string());
                    at += 2;
                } else {
                    folder = Some(rest[at].to_string());
                    at += 1;
                }
            }
            out(folder_arg(folder.as_ref()).and_then(|dir| {
                let pin = std::env::var(agents::PIN).ok();
                let (name, why) = agents::which(&home, None, pin.as_deref(), &dir)?;
                let mut said = format!("{name} ({why})");
                if let Some(agent) = agent {
                    let table = table()?;
                    if name != DEFAULT && !table.has_profiles(&agent) {
                        said.push_str(&format!(": {} has no profiles and starts on the home's", agents::title(&agent)));
                    }
                }
                if let Ok(Some((folder, mode, _))) = sandbox::rule_of(&home, &dir) {
                    said.push_str(&format!("; sandboxed ({}) by the rule of {folder}", mode.word()));
                }
                Ok(said)
            }))
        }
        ["running"] | ["running", "--json"] => {
            let rows = agents::running();
            if words.contains(&"--json") {
                println!("{}", Value::Array(rows));
                return 0;
            }
            if rows.is_empty() {
                println!("No agent runs now.");
            }
            for row in rows {
                let project = row["project"].as_str().map(|project| format!(", in {project}")).unwrap_or_default();
                println!("{}: {}{project}, since {}", row["pid"], agents::session_words(&row), crate::history::ago(row["started"].as_u64().unwrap_or(0)));
            }
            0
        }
        // For `remove`, which the recipe asks in a terminal and then moves to the trash: refused while a session runs on it
        ["in-use", name] => {
            if agents::in_use(name) {
                println!("yes");
                0
            } else {
                1
            }
        }
        ["root", name] if *name != DEFAULT && agents::exists(&home, name) => {
            println!("{}", agents::profile_root(&home, name).display());
            0
        }
        ["ask", name] => out(agents::set_ask(&home, name)),
        // A removed profile's folders, and the pill's questions if they went to it, follow the home's again
        ["forget", name] => out(agents::forget(name)),
        ["ask-profile"] => {
            println!("{}", agents::ask_profile(&home));
            0
        }
        ["env", rest @ ..] => env(rest, &home),
        _ => {
            eprintln!("usage: amethystora-agent-tools profile list|add|use|which|running|in-use|ask|ask-profile|root|env ...");
            64
        }
    }
}

/// What a hooked shell exports when its folder changes (or a pinned one once): each agent of way 1's variable for the folder's
/// profile, our own profile variable, and the words of the prompt's segment. A pinned shell keeps its pin, whatever the folder.
fn env(args: &[&str], home: &Path) -> i32 {
    let shell = args.windows(2).find(|pair| pair[0] == "--shell").map(|pair| pair[1]).unwrap_or("bash");
    let dir = args.windows(2).find(|pair| pair[0] == "--dir").map(|pair| PathBuf::from(pair[1]));
    let Ok(table) = table() else { return 0 };
    let dir = dir.or_else(|| std::env::current_dir().ok()).unwrap_or_else(|| home.to_path_buf());
    let pin = std::env::var(agents::PIN).ok().filter(|pin| !pin.is_empty());
    let (profile, _) = agents::which(home, None, pin.as_deref(), &dir).unwrap_or((DEFAULT.into(), ""));
    let rule = sandbox::rule_of(home, &dir).ok().flatten().map(|(_, mode, _)| mode);
    let mut set: Vec<(String, Option<String>)> = Vec::new();
    // On the home's profile a variable is taken away only when this hook set it (our own profile variable is there): one the
    // person set in their own shell files is theirs
    let ours = std::env::var_os(agents::PROFILE_VAR).is_some();
    for paths in table.agents.values() {
        if let Some(Way::Variable { name, dir }) = &paths.way {
            let value = (profile != DEFAULT).then(|| agents::profile_root(home, &profile).join(dir).to_string_lossy().into_owned());
            if value.is_some() || ours {
                set.push((name.clone(), value));
            }
        }
    }
    set.push((agents::PROFILE_VAR.into(), Some(profile.clone())));
    set.push(("AMETHYSTORA_AGENT_SANDBOX_RULE".into(), rule.map(|mode| mode.word().to_string())));
    let mut prompt: Vec<String> = Vec::new();
    if profile != DEFAULT {
        prompt.push(profile.clone());
    }
    if rule.is_some() {
        prompt.push("sandboxed".into());
    }
    set.push(("AMETHYSTORA_AGENT_PROMPT".into(), (!prompt.is_empty()).then(|| prompt.join(" · "))));
    for (name, value) in set {
        let line = match (shell, value) {
            ("fish", Some(value)) => format!("set -gx {name} {}", quoted(&value)),
            ("fish", None) => format!("set -e {name}"),
            (_, Some(value)) => format!("export {name}={}", quoted(&value)),
            (_, None) => format!("unset {name}"),
        };
        println!("{line}");
    }
    0
}

pub fn sandbox_cmd(args: &[String]) -> i32 {
    let home = agents::real_home();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let map_path = agents::sandbox_map_path();
    let change = |folder: &str, edit: &dyn Fn(&mut serde_json::Map<String, Value>) -> Result<String, String>| -> Result<String, String> {
        let folder = agents::real_folder(Path::new(folder))?;
        let key = folder.to_string_lossy().into_owned();
        if home.starts_with(&folder) {
            return Err(format!("{key} is your home or a folder above it: a rule is for the folders your projects are in"));
        }
        let mut map = Map::read(&map_path)?;
        let mut rule = map.folders.get(&key).and_then(Value::as_object).cloned().unwrap_or_default();
        let said = edit(&mut rule)?;
        if rule.is_empty() {
            map.folders.remove(&key);
        } else {
            map.folders.insert(key, Value::Object(rule));
        }
        map.write(&map_path)?;
        Ok(said)
    };
    match words.as_slice() {
        [] | ["list"] | ["list", "--json"] => {
            let map = match Map::read(&map_path) {
                Ok(map) => map,
                Err(why) => {
                    eprintln!("{why}");
                    return 1;
                }
            };
            let running: Vec<Value> = agents::running().into_iter().filter(|row| row["sandboxed"].is_string()).collect();
            if words.contains(&"--json") {
                println!("{}", json!({"folders": map.folders, "running": running}));
                return 0;
            }
            if map.folders.is_empty() {
                println!("No folder has a sandbox rule: agents start as they always do, unless started with --sandbox.");
            }
            for (folder, rule) in &map.folders {
                let allow: Vec<&str> = rule["allow"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                println!(
                    "{folder}: {}{}",
                    rule["mode"].as_str().unwrap_or("open"),
                    if allow.is_empty() { String::new() } else { format!(", allowed {}", allow.join(", ")) }
                );
            }
            for row in running {
                println!("Running: {} in {}", agents::session_words(&row), row["project"].as_str().unwrap_or("?"));
            }
            0
        }
        ["use", folder] | ["use", folder, "open"] => out(change(folder, &|rule| {
            rule.insert("mode".into(), json!("open"));
            Ok(format!("Agents started in {folder}, or a folder in it, are sandboxed to their project"))
        })),
        ["use", folder, "closed"] => out(change(folder, &|rule| {
            rule.insert("mode".into(), json!("closed"));
            Ok(format!("Agents started in {folder}, or a folder in it, are sandboxed to their project and see nothing else of your home"))
        })),
        // `off` takes the rule away; a folder inside one with a rule can say off for itself
        ["off", folder] => out(change(folder, &|rule| {
            let inherited = rule.is_empty();
            rule.clear();
            if inherited {
                rule.insert("mode".into(), json!("off"));
            }
            Ok(format!("Agents started in {folder} start as they always do"))
        })),
        ["allow", folder, permission] => out(Permission::parse(permission, &home).and_then(|permission| {
            change(folder, &|rule| {
                let mut allow: Vec<Value> = rule.get("allow").and_then(Value::as_array).cloned().unwrap_or_default();
                let word = json!(permission.word());
                if !allow.contains(&word) {
                    allow.push(word);
                }
                rule.insert("allow".into(), Value::Array(allow));
                rule.entry("mode").or_insert(json!("open"));
                Ok(format!("{folder}'s sandbox allows {}", permission.word()))
            })
        })),
        ["disallow", folder, permission] => out(change(folder, &|rule| {
            if let Some(Value::Array(allow)) = rule.get_mut("allow") {
                allow.retain(|word| word.as_str() != Some(permission));
                if allow.is_empty() {
                    rule.remove("allow");
                }
            }
            Ok(format!("{folder}'s sandbox no longer allows {permission}"))
        })),
        // What a permission gives away, for the recipe to say before it asks
        ["cost", permission] => out(Permission::parse(permission, &home).map(|permission| permission.cost())),
        ["show", rest @ ..] => sandbox::agent_plan(&rest.iter().map(|word| word.to_string()).collect::<Vec<_>>(), true),
        _ => {
            eprintln!("usage: amethystora-agent-tools sandbox list|use|off|allow|disallow|cost|show ...");
            64
        }
    }
}
