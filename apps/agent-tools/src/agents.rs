//! Where each AI agent keeps its things, and the account's profiles of them (plans/PLAN-agent-accounts.md): several
//! accounts of one agent side by side, each in a folder of its own with the home's layout, picked when a launch starts and
//! belonging to that launch alone. Nothing here says which profile is "the current one": a running agent's profile is in its
//! environment or its own mount namespace, and the next launch works its own out again.
//!
//! The table is /usr/share/amethystora/agent-paths (its own header says each kind of line); the maps are the account's
//! ~/.config/amethystora/agent-profiles.json (folders to profiles, and the one the pill's question uses) and
//! agent-sandbox.json (folders to sandbox rules), each read by the longest prefix of the real path.

use std::collections::BTreeMap;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};

use crate::policy;

pub const TABLE: &str = "/usr/share/amethystora/agent-paths";
pub const CREDENTIALS: &str = "/usr/share/amethystora/credential-files";
/// The agents amethystora-agent offers, by the command each starts with (its AGENTS, which 20-tests.sh holds this to)
pub const AGENTS: [&str; 6] = ["claude", "opencode", "codex", "agy", "copilot", "cursor-agent"];
/// The profiles' folder, relative to the home: one folder each, with the home's layout
pub const PROFILES: &str = ".local/share/amethystora/agent-profiles";
pub const DEFAULT: &str = "default";
/// The agent of `ame agent sandbox run`: none, so no agent's folder is writable or even there
pub const NO_AGENT: &str = "none";
/// A pinned terminal's profile (ame agent profile terminal), which a folder never changes
pub const PIN: &str = "AMETHYSTORA_AGENT_PROFILE_PIN";
/// The profile a launch or a hooked shell is on, for the prompt, the tool server and the records
pub const PROFILE_VAR: &str = "AMETHYSTORA_AGENT_PROFILE";
/// The agent a tool server was started for, by the launcher
pub const AGENT_VAR: &str = "AMETHYSTORA_AGENT_NAME";

/// The people's name for an agent
pub fn title(agent: &str) -> &str {
    match agent {
        "claude" => "Claude Code",
        "opencode" => "Opencode",
        "codex" => "Codex",
        "agy" => "Antigravity CLI",
        "copilot" => "Copilot CLI",
        "cursor-agent" => "Cursor CLI",
        other => other,
    }
}

/// A profile's name: what a person types and a folder can be named, nothing that could be a path or an option
pub fn profile_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 32 && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') && !name.starts_with('-')
}

pub fn agent_name(name: &str) -> bool {
    AGENTS.contains(&name)
}

/// How a profile reaches an agent
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Way {
    /// Its own variable points it at the profile's copy of a folder
    Variable { name: String, dir: String },
    /// The profile's folders are bound over the home's, in a mount namespace of the launch's own
    Mount,
    /// No profile: its login is where no folder isolates it
    None,
}

#[derive(Debug, Clone, Default)]
pub struct AgentPaths {
    pub way: Option<Way>,
    pub dirs: Vec<String>,
    pub logins: Vec<String>,
    pub copies: Vec<String>,
    pub programs: Vec<String>,
    pub ro: Vec<String>,
    pub project: Vec<String>,
    pub env: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Table {
    pub agents: BTreeMap<String, AgentPaths>,
    /// The project lines of every agent
    pub project: Vec<String>,
    pub caches: Vec<String>,
    pub hidden: Vec<String>,
    pub tools: Vec<String>,
}

/// A path of the table: relative to the home, plain, never leaving it
fn relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && Path::new(path).components().all(|part| matches!(part, Component::Normal(_)))
}

fn variable(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_') && !name.starts_with(|c: char| c.is_ascii_digit())
}

impl Table {
    pub fn load() -> Result<Table, String> {
        let text = std::fs::read_to_string(TABLE).map_err(|error| format!("{TABLE}: {error}"))?;
        Table::parse(&text)
    }

    /// The table, read strictly: a line it does not understand is an error, never passed over, since a path left out of
    /// a sandbox's recipe is a path the agent can reach
    pub fn parse(text: &str) -> Result<Table, String> {
        let mut table = Table::default();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let words: Vec<&str> = line.split_whitespace().collect();
            let bad = || format!("{TABLE}:{}: cannot read: {line}", number + 1);
            let (kind, agent) = (words[0], *words.get(1).ok_or_else(bad)?);
            let rest = &words[2..];
            if agent != "-" && !agent_name(agent) {
                return Err(bad());
            }
            let path = |at: usize| -> Result<String, String> {
                let path = rest.get(at).ok_or_else(bad)?;
                if relative(path) { Ok(path.to_string()) } else { Err(bad()) }
            };
            if agent == "-" {
                match kind {
                    "project" => table.project.push(path(0)?),
                    "cache" => table.caches.push(path(0)?),
                    "tool" => table.tools.push(path(0)?),
                    "hidden" => {
                        let path = rest.first().ok_or_else(bad)?;
                        // The machine's own places are absolute; a home's are relative
                        if !(relative(path) || (path.starts_with('/') && Path::new(path).components().skip(1).all(|part| matches!(part, Component::Normal(_))))) {
                            return Err(bad());
                        }
                        table.hidden.push(path.to_string());
                    }
                    _ => return Err(bad()),
                }
                if rest.len() != 1 {
                    return Err(bad());
                }
                continue;
            }
            let entry = table.agents.entry(agent.to_string()).or_default();
            match (kind, rest) {
                ("way", ["var", name, dir]) if variable(name) && relative(dir) => {
                    entry.way = Some(Way::Variable { name: name.to_string(), dir: dir.to_string() })
                }
                ("way", ["mount"]) => entry.way = Some(Way::Mount),
                ("way", ["none"]) => entry.way = Some(Way::None),
                ("dir", [_]) => entry.dirs.push(path(0)?),
                ("login", [_]) => entry.logins.push(path(0)?),
                ("copy", [_]) => entry.copies.push(path(0)?),
                ("program", [_]) => entry.programs.push(path(0)?),
                ("ro", [_]) => entry.ro.push(path(0)?),
                ("project", [_]) => entry.project.push(path(0)?),
                ("env", [name]) if variable(name) => entry.env.push(name.to_string()),
                _ => return Err(bad()),
            }
        }
        for agent in AGENTS {
            let paths = table.agents.get(agent).ok_or_else(|| format!("{TABLE}: no lines for {agent}"))?;
            if paths.way.is_none() || paths.dirs.is_empty() {
                return Err(format!("{TABLE}: {agent} needs a way line and a dir line"));
            }
        }
        Ok(table)
    }

    pub fn agent(&self, agent: &str) -> Result<&AgentPaths, String> {
        // `ame agent sandbox run`: a command held as an agent would be, with no agent's folder at all
        static NONE: AgentPaths =
            AgentPaths { way: None, dirs: Vec::new(), logins: Vec::new(), copies: Vec::new(), programs: Vec::new(), ro: Vec::new(), project: Vec::new(), env: Vec::new() };
        if agent == NO_AGENT {
            return Ok(&NONE);
        }
        self.agents.get(agent).ok_or_else(|| format!("{agent} is not an agent amethystora-agent offers"))
    }

    /// Whether the agent can have profiles other than the home's
    pub fn has_profiles(&self, agent: &str) -> bool {
        self.agents.get(agent).is_some_and(|paths| !matches!(paths.way, Some(Way::None) | None))
    }
}

pub const NETWORK_HOSTS: &str = "/usr/share/amethystora/agent-network-hosts";

/// The shipped hosts a sandbox held to a list may reach, for AGENT and for every agent: names and *.names only
pub fn network_hosts(agent: &str) -> Vec<String> {
    hosts_in(&std::fs::read_to_string(NETWORK_HOSTS).unwrap_or_default(), agent)
}

pub fn hosts_in(text: &str, agent: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let (who, host) = (words.next()?, words.next()?);
            (words.next().is_none() && (who == "-" || who == agent) && amethystora_net_proxy::valid_host(host)).then(|| host.to_string())
        })
        .collect()
}

/// The credential files' globs (credential-files), one list with the security watcher's
pub fn credential_globs() -> Vec<String> {
    std::fs::read_to_string(CREDENTIALS)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && relative(line))
        .map(String::from)
        .collect()
}

/// The paths under BASE that GLOB (relative, * within one name only) names, that are there
pub fn expand(base: &Path, glob: &str) -> Vec<PathBuf> {
    let mut found = vec![base.to_path_buf()];
    for part in glob.split('/') {
        let mut next = Vec::new();
        for dir in &found {
            if !part.contains('*') {
                let path = dir.join(part);
                if std::fs::symlink_metadata(&path).is_ok() {
                    next.push(path);
                }
                continue;
            }
            let Ok(entries) = std::fs::read_dir(dir) else { continue };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if crate::checks::matches(part, &name) {
                    next.push(entry.path());
                }
            }
        }
        found = next;
        if found.len() > 4096 {
            break;
        }
    }
    found
}

// --- The account's own files ---------------------------------------------------------------------------------------

pub fn real_home() -> PathBuf {
    let home = policy::home();
    std::fs::canonicalize(&home).unwrap_or(home)
}

pub fn profiles_dir(home: &Path) -> PathBuf {
    home.join(PROFILES)
}

/// The folder a profile keeps the agents' folders in: the home itself for `default`
pub fn profile_root(home: &Path, name: &str) -> PathBuf {
    if name == DEFAULT { home.to_path_buf() } else { profiles_dir(home).join(name) }
}

/// Every profile there is, `default` first
pub fn profiles(home: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(profiles_dir(home))
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| profile_name(name) && name != DEFAULT)
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names.insert(0, DEFAULT.into());
    names
}

pub fn exists(home: &Path, name: &str) -> bool {
    name == DEFAULT || (profile_name(name) && profile_root(home, name).is_dir())
}

/// One of the account's two maps: its folders, each with what it says, and its other keys
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Map {
    pub folders: BTreeMap<String, Value>,
    pub other: serde_json::Map<String, Value>,
}

pub fn profile_map_path() -> PathBuf {
    policy::config_dir().join("agent-profiles.json")
}

pub fn sandbox_map_path() -> PathBuf {
    policy::config_dir().join("agent-sandbox.json")
}

impl Map {
    /// The map, or an empty one where there is none. A file that cannot be read is an error: a rule left unread would let a
    /// launch go plain that was meant to be sandboxed
    pub fn read(path: &Path) -> Result<Map, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Map::default()),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        let Ok(Value::Object(mut object)) = serde_json::from_str::<Value>(&text) else {
            return Err(format!("{} is not the JSON object it should be: fix or remove it", path.display()));
        };
        let folders = match object.remove("folders") {
            Some(Value::Object(folders)) => folders.into_iter().filter(|(folder, _)| folder.starts_with('/')).collect(),
            None => BTreeMap::new(),
            Some(_) => return Err(format!("{}: folders is not an object", path.display())),
        };
        Ok(Map { folders, other: object })
    }

    /// Written whole and moved into place, readable by the account alone
    pub fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        }
        let mut object = self.other.clone();
        object.insert("folders".into(), Value::Object(self.folders.clone().into_iter().collect()));
        let next = path.with_extension("json.new");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&next)
            .map_err(|error| format!("{}: {error}", next.display()))?;
        std::io::Write::write_all(&mut file, format!("{:#}\n", Value::Object(object)).as_bytes()).map_err(|error| error.to_string())?;
        std::fs::rename(&next, path).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// The folder of the map that holds DIR (a real path), the longest one: its own path and what it says
    pub fn find(&self, dir: &Path) -> Option<(&str, &Value)> {
        self.folders
            .iter()
            .filter(|(folder, _)| dir.starts_with(folder.as_str()))
            .max_by_key(|(folder, _)| Path::new(folder.as_str()).components().count())
            .map(|(folder, value)| (folder.as_str(), value))
    }
}

/// A folder as the maps keep it: its real path, which must be there
pub fn real_folder(dir: &Path) -> Result<PathBuf, String> {
    let real = std::fs::canonicalize(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    if !real.is_dir() {
        return Err(format!("{} is not a folder", dir.display()));
    }
    Ok(real)
}

/// Which profile a launch made in DIR gets, and why: the one asked for, the terminal's pin, the folder's, else the home's
pub fn which(home: &Path, asked: Option<&str>, pin: Option<&str>, dir: &Path) -> Result<(String, &'static str), String> {
    let check = |name: &str| -> Result<String, String> {
        if !profile_name(name) {
            return Err(format!("{name} is not a profile's name: lower-case letters, digits and -, 32 at most"));
        }
        if !exists(home, name) {
            return Err(format!("there is no profile {name}: ame agent profile add {name}"));
        }
        Ok(name.to_string())
    };
    if let Some(name) = asked {
        return Ok((check(name)?, "asked for"));
    }
    if let Some(name) = pin.filter(|name| !name.is_empty()) {
        return Ok((check(name)?, "the terminal's"));
    }
    let map = Map::read(&profile_map_path())?;
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if let Some((_, Value::String(name))) = map.find(&dir) {
        // A profile removed while a folder still names it falls back to the home's, said by `which`
        if exists(home, name) {
            return Ok((name.clone(), "the folder's"));
        }
    }
    Ok((DEFAULT.into(), "the home's"))
}

/// What a launch of AGENT on PROFILE is given in its environment: the variable of its way, and our own two. For the home's
/// profile the variable is taken away only when one of our own set it (a hooked shell): one the person set is theirs.
pub fn environment(table: &Table, home: &Path, agent: &str, profile: &str) -> Vec<(String, Option<String>)> {
    let mut env = Vec::new();
    let ours = std::env::var_os(PROFILE_VAR).is_some();
    for (other, paths) in &table.agents {
        if let Some(Way::Variable { name, dir }) = &paths.way {
            if other == agent && profile != DEFAULT {
                env.push((name.clone(), Some(profile_root(home, profile).join(dir).to_string_lossy().into_owned())));
            } else if ours {
                env.push((name.clone(), None));
            }
        }
    }
    env.push((PROFILE_VAR.into(), Some(profile.into())));
    env.push((AGENT_VAR.into(), Some(agent.into())));
    env
}

// --- ame agent profile ----------------------------------------------------------------------------------------------

/// The agents with a login in a profile's folder (never what the login says)
pub fn logged_in(table: &Table, root: &Path) -> Vec<String> {
    AGENTS
        .iter()
        .filter(|agent| table.agents.get(**agent).is_some_and(|paths| paths.logins.iter().any(|login| root.join(login).is_file())))
        .map(|agent| agent.to_string())
        .collect()
}

pub fn list(table: &Table, home: &Path) -> Result<Value, String> {
    let map = Map::read(&profile_map_path())?;
    let ask = map.other.get("ask").and_then(Value::as_str).unwrap_or(DEFAULT);
    let rows: Vec<Value> = profiles(home)
        .into_iter()
        .map(|name| {
            let folders: Vec<&String> = map.folders.iter().filter(|(_, value)| value.as_str() == Some(&name)).map(|(folder, _)| folder).collect();
            json!({"name": name, "agents": logged_in(table, &profile_root(home, &name)), "folders": folders, "ask": name == ask})
        })
        .collect();
    Ok(json!({"profiles": rows, "running": running()}))
}

pub fn add(home: &Path, name: &str) -> Result<String, String> {
    if !profile_name(name) || name == DEFAULT {
        return Err(format!("{name} cannot be a profile's name: lower-case letters, digits and -, 32 at most, not default"));
    }
    let root = profile_root(home, name);
    if root.exists() {
        return Err(format!("the profile {name} is there already"));
    }
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&root).map_err(|error| format!("{}: {error}", root.display()))?;
    Ok(format!("Profile {name} made. Sign in to an agent on it with: ame agent profile login {name} AGENT"))
}

/// Binds FOLDER (and every folder in it) to a profile, or takes it off with `default`
pub fn use_folder(home: &Path, name: &str, folder: &Path) -> Result<String, String> {
    if !exists(home, name) {
        return Err(format!("there is no profile {name}"));
    }
    let folder = real_folder(folder)?;
    let path = profile_map_path();
    let mut map = Map::read(&path)?;
    let key = folder.to_string_lossy().into_owned();
    if name == DEFAULT {
        map.folders.remove(&key);
    } else {
        map.folders.insert(key.clone(), json!(name));
    }
    map.write(&path)?;
    Ok(if name == DEFAULT {
        format!("{key} follows the home's profile again")
    } else {
        format!("Agents started in {key}, or in a folder in it, are on {name}")
    })
}

/// The profile the pill's question (Ask right there) goes to: the home's unless one is named. Never the focused window's.
pub fn set_ask(home: &Path, name: &str) -> Result<String, String> {
    if !exists(home, name) {
        return Err(format!("there is no profile {name}"));
    }
    let path = profile_map_path();
    let mut map = Map::read(&path)?;
    map.other.insert("ask".into(), json!(name));
    map.write(&path)?;
    Ok(format!("The live pill's questions go to {name}"))
}

/// The map without a profile that was taken away: its folders follow the home's again
pub fn forget(name: &str) -> Result<String, String> {
    let path = profile_map_path();
    let mut map = Map::read(&path)?;
    map.folders.retain(|_, value| value.as_str() != Some(name));
    if map.other.get("ask").and_then(Value::as_str) == Some(name) {
        map.other.remove("ask");
    }
    map.write(&path)?;
    Ok(String::new())
}

pub fn ask_profile(home: &Path) -> String {
    Map::read(&profile_map_path())
        .ok()
        .and_then(|map| map.other.get("ask").and_then(Value::as_str).map(String::from))
        .filter(|name| exists(home, name))
        .unwrap_or_else(|| DEFAULT.into())
}

/// The sessions running now, from the account's runtime folder: each tool server's file, and each sandbox's (written by its
/// runner), still the live process it names
pub fn running() -> Vec<Value> {
    let dir = crate::history::sessions_dir();
    let mut rows = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else { return rows };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.bytes().all(|b| b.is_ascii_digit()) || !Path::new("/proc").join(&name).exists() {
            continue;
        }
        let Ok(Value::Object(file)) = std::fs::read_to_string(entry.path()).map(|text| serde_json::from_str(&text).unwrap_or(Value::Null)) else {
            continue;
        };
        let field = |key: &str| file.get(key).cloned().unwrap_or(Value::Null);
        rows.push(json!({"pid": field("pid"), "agent": field("agent"), "profile": field("profile"), "sandboxed": field("sandboxed"),
            "project": field("project"), "started": field("started")}));
    }
    rows.sort_by_key(|row| row["started"].as_u64().unwrap_or(0));
    rows
}

/// Whether a session runs on the profile
pub fn in_use(name: &str) -> bool {
    running().iter().any(|row| row["profile"] == name)
}

/// The words to say each running session: "Claude Code · work · sandboxed"
pub fn session_words(row: &Value) -> String {
    let mut words = vec![title(row["agent"].as_str().unwrap_or("an agent")).to_string()];
    if let Some(profile) = row["profile"].as_str().filter(|profile| *profile != DEFAULT) {
        words.push(profile.to_string());
    }
    if row["sandboxed"].is_string() {
        words.push("sandboxed".into());
    }
    words.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("amethystora-agents-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::canonicalize(&dir).unwrap()
    }

    #[test]
    fn the_shipped_table_reads_and_names_every_agent() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../system_files/shared/usr/share/amethystora/agent-paths"));
        let Ok(text) = text else { return };
        let table = Table::parse(&text).unwrap();
        for agent in AGENTS {
            assert!(table.agents.contains_key(agent), "{agent}");
        }
        assert_eq!(table.agents["claude"].way, Some(Way::Variable { name: "CLAUDE_CONFIG_DIR".into(), dir: ".claude".into() }));
        assert_eq!(table.agents["copilot"].way, Some(Way::None));
        assert!(table.has_profiles("cursor-agent") && !table.has_profiles("copilot"));
        // Every login is in the agent's own folder, so that a profile's copy of the folder holds it
        for (agent, paths) in &table.agents {
            for login in &paths.logins {
                assert!(paths.dirs.iter().any(|dir| Path::new(login).starts_with(dir)), "{agent} {login}");
            }
        }
    }

    #[test]
    fn the_table_is_read_strictly() {
        let good = AGENTS.iter().map(|agent| format!("way {agent} mount\ndir {agent} .{agent}\n")).collect::<String>();
        assert!(Table::parse(&good).is_ok());
        for bad in ["dir claude /etc", "dir claude ../x", "dir nobody .x", "frob - .x", "hidden - ../x", "env claude lower", "way claude var X"] {
            assert!(Table::parse(&format!("{good}{bad}\n")).is_err(), "{bad}");
        }
        assert!(Table::parse("way claude mount\ndir claude .claude\n").is_err(), "an agent left out");
    }

    #[test]
    fn names_and_the_longest_folder() {
        assert!(profile_name("work") && profile_name("acme-2") && !profile_name("Work") && !profile_name("-x") && !profile_name("a/b"));
        assert!(!profile_name(&"a".repeat(33)) && !profile_name(""));
        let mut map = Map::default();
        map.folders.insert("/home/a/work".into(), json!("work"));
        map.folders.insert("/home/a/work/oss".into(), json!("personal"));
        assert_eq!(map.find(Path::new("/home/a/work/api")).unwrap().1, "work");
        assert_eq!(map.find(Path::new("/home/a/work/oss/lib")).unwrap().1, "personal");
        assert_eq!(map.find(Path::new("/home/a/work")).unwrap().1, "work");
        // A name that only begins the same is not inside the folder
        assert!(map.find(Path::new("/home/a/workshop")).is_none());
    }

    #[test]
    fn a_profile_belongs_to_the_launch() {
        let home = temp("which");
        std::fs::create_dir_all(home.join(PROFILES).join("work")).unwrap();
        let (dir, project) = (home.join("work"), home.join("code"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(&project).unwrap();
        assert!(exists(&home, "work") && exists(&home, DEFAULT) && !exists(&home, "nope"));
        assert_eq!(profiles(&home), vec!["default".to_string(), "work".to_string()]);
        // Asked for, then the pin, then the folder (no map here: the home's)
        assert_eq!(which(&home, Some("work"), Some("x"), &project).unwrap().0, "work");
        assert!(which(&home, Some("nope"), None, &project).is_err());
        assert_eq!(which(&home, None, Some("work"), &project).unwrap().0, "work");
        assert!(which(&home, None, Some("Bad/"), &project).is_err());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_profiles_variable_points_into_it() {
        let table = Table::parse(&AGENTS.iter().map(|agent| match *agent {
            "claude" => "way claude var CLAUDE_CONFIG_DIR .claude\ndir claude .claude\n".to_string(),
            other => format!("way {other} mount\ndir {other} .{other}\n"),
        }).collect::<String>()).unwrap();
        let env = environment(&table, Path::new("/h"), "claude", "work");
        assert!(env.contains(&("CLAUDE_CONFIG_DIR".into(), Some("/h/.local/share/amethystora/agent-profiles/work/.claude".into()))));
        assert!(env.contains(&(PROFILE_VAR.into(), Some("work".into()))));
        let env = environment(&table, Path::new("/h"), "codex", "work");
        assert!(!env.iter().any(|(name, value)| name == "CLAUDE_CONFIG_DIR" && value.is_some()));
    }

    #[test]
    fn globs_find_what_is_there() {
        let home = temp("globs");
        std::fs::create_dir_all(home.join(".mozilla/firefox/abc.default")).unwrap();
        std::fs::write(home.join(".mozilla/firefox/abc.default/logins.json"), "{}").unwrap();
        std::fs::create_dir_all(home.join(".ssh")).unwrap();
        std::fs::write(home.join(".ssh/id_ed25519"), "k").unwrap();
        assert_eq!(expand(&home, ".mozilla/firefox/*/logins.json"), vec![home.join(".mozilla/firefox/abc.default/logins.json")]);
        assert_eq!(expand(&home, ".ssh/id_*"), vec![home.join(".ssh/id_ed25519")]);
        assert!(expand(&home, ".aws/credentials").is_empty());
        std::fs::remove_dir_all(&home).unwrap();
    }
}
