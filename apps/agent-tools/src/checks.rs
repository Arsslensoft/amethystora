//! Whether the agent that started these tools runs with its own checks turned off: a flag or a variable from
//! /usr/share/amethystora/agent-cli-flags on it, or a setting of its own that has it never ask. The tools
//! that change things then refuse, and say why: everything they do relies on the agent asking first.
//!
//! The agent is found as the security watcher finds one (amethystora-security-watch's agent_of and
//! agent_flag): by its program and first two arguments, among the processes above this one.

use std::path::Path;

pub const FLAGS: &str = "/usr/share/amethystora/agent-cli-flags";

/// One process's program, command line and environment, as /proc has them
#[derive(Debug, Clone, Default)]
pub struct Process {
    pub pid: u32,
    pub exe: String,
    pub args: Vec<String>,
    pub environment: Vec<String>,
}

fn nul_separated(bytes: &[u8]) -> Vec<String> {
    bytes.split(|b| *b == 0).filter(|part| !part.is_empty()).map(|part| String::from_utf8_lossy(part).into_owned()).collect()
}

impl Process {
    pub fn read(pid: u32) -> Option<Process> {
        let dir = Path::new("/proc").join(pid.to_string());
        let args = nul_separated(&std::fs::read(dir.join("cmdline")).ok()?);
        let environment = nul_separated(&std::fs::read(dir.join("environ")).unwrap_or_default());
        let exe = std::fs::read_link(dir.join("exe")).map(|path| path.to_string_lossy().into_owned()).unwrap_or_default();
        Some(Process { pid, exe, args, environment })
    }

    pub fn parent_of(pid: u32) -> Option<u32> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // The name in brackets may hold anything, spaces and brackets too: the fields follow the last one
        let rest = &stat[stat.rfind(')')? + 2..];
        rest.split(' ').nth(1)?.parse().ok()
    }
}

fn base(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Which agent a process is, from its program and its first two arguments: the command, or the script
/// node or bash runs
pub fn agent_of(process: &Process) -> Option<&'static str> {
    let first = process.args.first().map(String::as_str).unwrap_or("");
    let second = process.args.get(1).map(String::as_str).unwrap_or("");
    for name in [first, second] {
        let name = base(name);
        match name {
            "claude" => return Some("claude"),
            "codex" => return Some("codex"),
            "copilot" => return Some("copilot"),
            "opencode" => return Some("opencode"),
            "cursor-agent" => return Some("cursor-agent"),
            "gemini" | "gemini.js" => return Some("gemini"),
            _ if name == "agy" || (name.starts_with("agy.") && name.ends_with(".old")) => return Some("agy"),
            _ => {}
        }
    }
    if process.exe.contains("/cursor-agent/versions/") || second.contains("/cursor-agent/") {
        return Some("cursor-agent");
    }
    if second.contains("/gemini-cli/") {
        return Some("gemini");
    }
    None
}

/// A glob as bash matches one: * and ? and [...] classes
fn glob(pattern: &[char], text: &[char]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some(('*', rest)) => (0..=text.len()).any(|at| glob(rest, &text[at..])),
        Some(('?', rest)) => !text.is_empty() && glob(rest, &text[1..]),
        Some(('[', rest)) => {
            let Some(close) = rest.iter().skip(1).position(|c| *c == ']').map(|at| at + 1) else {
                return text.first() == Some(&'[') && glob(rest, &text[1..]);
            };
            let Some(first) = text.first() else { return false };
            let (negate, class) = match rest[..close].split_first() {
                Some(('!' | '^', class)) => (true, class),
                _ => (false, &rest[..close]),
            };
            let mut hit = false;
            let mut at = 0;
            while at < class.len() {
                if at + 2 < class.len() && class[at + 1] == '-' {
                    hit |= (class[at]..=class[at + 2]).contains(first);
                    at += 3;
                } else {
                    hit |= class[at] == *first;
                    at += 1;
                }
            }
            hit != negate && glob(&rest[close + 1..], &text[1..])
        }
        Some((c, rest)) => text.first() == Some(c) && glob(rest, &text[1..]),
    }
}

/// The first of the agent's flags that the process carries, as the watcher matches them
pub fn flag_of(flags: &str, agent: &str, process: &Process) -> Option<String> {
    let mut joined = String::from(" ");
    for arg in process.args.iter().skip(1) {
        // --option=value counts as --option value
        let arg = match arg.split_once('=') {
            Some((option, value))
                if option.starts_with("--") && option.len() > 2 && option[2..].chars().all(|c| c.is_ascii_lowercase() || c == '-') =>
            {
                format!("{option} {value}")
            }
            _ => arg.clone(),
        };
        joined.push_str(&arg);
        joined.push(' ');
    }
    for line in flags.lines().filter(|line| !line.starts_with('#')) {
        let Some((name, flag)) = line.split_once('\t') else { continue };
        if name != agent {
            continue;
        }
        if let Some(variable) = flag.strip_prefix("env:") {
            let Some((variable, pattern)) = variable.split_once('=') else { continue };
            let pattern: Vec<char> = pattern.chars().collect();
            for entry in &process.environment {
                if let Some((key, value)) = entry.split_once('=') {
                    if key == variable && glob(&pattern, &value.chars().collect::<Vec<_>>()) {
                        return Some(variable.to_string());
                    }
                }
            }
        } else if joined.contains(&format!(" {flag} ")) {
            return Some(flag.to_string());
        }
    }
    None
}

/// The agent above this process, at most a few processes up, and the flag that turns its checks off, if it
/// has one. None when no agent is found: the tools were started by a person or a script.
pub fn agent_above(start: u32, flags: &str) -> Option<(String, Option<String>)> {
    let (agent, process) = find_agent(start)?;
    let flag = flag_of(flags, agent, &process);
    Some((agent.to_string(), flag))
}

fn find_agent(start: u32) -> Option<(&'static str, Process)> {
    let mut pid = Process::parent_of(start)?;
    for _ in 0..6 {
        if pid <= 1 {
            return None;
        }
        if let Some(process) = Process::read(pid) {
            if let Some(agent) = agent_of(&process) {
                return Some((agent, process));
            }
        }
        pid = Process::parent_of(pid)?;
    }
    None
}

/// What amethystora-agent puts in the environment of the tool server, through the MCP server entry it gives each
/// agent (the entry's `env`). It is read from the tool server's own environment, never from the agent's, which a
/// non-dumpable agent keeps unreadable. The tools that change things, and the root helper, serve only a tool server
/// that has it with an agent above it: a program that starts the tool server itself is refused. It is a check against
/// that, not a lock: a program of the same account can set a variable and name itself like an agent, so the person's
/// own answer (the agent's question, the password polkit asks) is what stops it.
pub const LAUNCHED: &str = "AMETHYSTORA_AGENT_LAUNCHED";

/// The tool server `pid`'s environment as /proc has it, None when it cannot be read
pub fn environment_of(pid: u32) -> Option<Vec<String>> {
    std::fs::read(format!("/proc/{pid}/environ")).ok().map(|bytes| nul_separated(&bytes))
}

/// Whether the tool server at `server` was started by an agent through amethystora-agent, else why not.
/// `environment` is the tool server's own environment, None when it could not be read.
pub fn launched_by_launcher(server: u32, environment: Option<&[String]>) -> Result<(), String> {
    let Some((agent, _)) = find_agent(server) else {
        return Err("no AI agent started these tools".into());
    };
    match environment {
        None => Err("the tool server's environment could not be read, so it cannot be told who started it".into()),
        Some(environment) if marked(environment) => Ok(()),
        Some(_) => Err(format!("{agent} did not start these tools through amethystora-agent")),
    }
}

fn marked(environment: &[String]) -> bool {
    environment.iter().any(|entry| entry == &format!("{LAUNCHED}=1"))
}

/// Why the tools that change things refuse, or None when the agent's checks are on. `config` is what the
/// security reader found in the agents' own settings that turns their checks off, one sentence each.
pub fn checks_off(start: u32, config: &[String]) -> Option<String> {
    let flags = std::fs::read_to_string(FLAGS).unwrap_or_default();
    if let Some((agent, Some(flag))) = agent_above(start, &flags) {
        return Some(format!("{agent} was started with {flag}, which turns its own checks off"));
    }
    config.first().map(|finding| format!("an agent's own settings turn its checks off: {finding}"))
}

/// What the security reader says about the agents' own settings that turns their checks off: its findings in
/// the tactic Defense Evasion, which is how it marks a mode that never asks
pub fn config_findings(reader_output: &str) -> Vec<String> {
    reader_output
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|finding| finding["tactic"] == "Defense Evasion")
        .filter_map(|finding| finding["text"].as_str().map(|text| crate::text::field(text, 300)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLAGS_FILE: &str = "# comment\nclaude\t--dangerously-skip-permissions\nclaude\t--permission-mode bypassPermissions\n\
        codex\t--sandbox danger-full-access\ncopilot\tenv:COPILOT_ALLOW_ALL=[1tTyYoO]*\ncursor-agent\t-f\nopencode\tenv:OPENCODE_PERMISSION=*allow*\n";

    fn process(args: &[&str], environment: &[&str]) -> Process {
        Process {
            pid: 2,
            exe: String::new(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            environment: environment.iter().map(|entry| entry.to_string()).collect(),
        }
    }

    #[test]
    fn finds_the_agent_as_the_watcher_does() {
        assert_eq!(agent_of(&process(&["/home/a/.local/bin/claude", "hi"], &[])), Some("claude"));
        assert_eq!(agent_of(&process(&["node", "/usr/local/bin/copilot"], &[])), Some("copilot"));
        assert_eq!(agent_of(&process(&["agy.3.old"], &[])), Some("agy"));
        assert_eq!(agent_of(&process(&["node", "/home/a/.local/share/cursor-agent/versions/1/index.js"], &[])), Some("cursor-agent"));
        assert_eq!(agent_of(&process(&["bash"], &[])), None);
    }

    #[test]
    fn matches_flags_and_variables() {
        let flag = |agent, args: &[&str], env: &[&str]| flag_of(FLAGS_FILE, agent, &process(args, env));
        assert_eq!(flag("claude", &["claude", "--dangerously-skip-permissions"], &[]).as_deref(), Some("--dangerously-skip-permissions"));
        assert_eq!(flag("claude", &["claude", "--permission-mode=bypassPermissions"], &[]).as_deref(), Some("--permission-mode bypassPermissions"));
        assert_eq!(flag("claude", &["claude", "--permission-mode", "default"], &[]), None);
        assert_eq!(flag("codex", &["codex", "--sandbox", "danger-full-access"], &[]).as_deref(), Some("--sandbox danger-full-access"));
        assert_eq!(flag("copilot", &["copilot"], &["COPILOT_ALLOW_ALL=true"]).as_deref(), Some("COPILOT_ALLOW_ALL"));
        assert_eq!(flag("copilot", &["copilot"], &["COPILOT_ALLOW_ALL=0"]), None);
        assert_eq!(flag("opencode", &["opencode"], &["OPENCODE_PERMISSION={\"bash\":\"allow\"}"]).as_deref(), Some("OPENCODE_PERMISSION"));
        assert_eq!(flag("opencode", &["opencode"], &["OPENCODE_PERMISSION={\"bash\":\"ask\"}"]), None);
        // Another agent's flag, or the flag inside an argument, is not this one's
        assert_eq!(flag("cursor-agent", &["cursor-agent", "--fix"], &[]), None);
        assert_eq!(flag("claude", &["claude", "-f"], &[]), None);
    }

    #[test]
    fn only_the_launchers_mark_counts() {
        let env = |entries: &[&str]| entries.iter().map(|entry| entry.to_string()).collect::<Vec<_>>();
        assert!(marked(&env(&["HOME=/home/a", "AMETHYSTORA_AGENT_LAUNCHED=1"])));
        assert!(!marked(&env(&["HOME=/home/a"])));
        assert!(!marked(&env(&["AMETHYSTORA_AGENT_LAUNCHED=0", "X_AMETHYSTORA_AGENT_LAUNCHED=1"])));
        // Nothing above a process that has no parent to find: refused, with or without a mark
        assert!(launched_by_launcher(1, Some(&env(&["AMETHYSTORA_AGENT_LAUNCHED=1"]))).is_err());
        assert!(launched_by_launcher(1, None).is_err());
        // This process's own environment can be read
        assert!(environment_of(std::process::id()).is_some());
    }

    #[test]
    fn reads_the_readers_findings() {
        let out = "{\"id\":\"a\",\"tactic\":\"Defense Evasion\",\"text\":\"~/.claude/settings.json turns Claude Code's checks off: x\"}\n\
                   {\"id\":\"b\",\"tactic\":\"Persistence\",\"text\":\"hook\"}\nnot json\n";
        assert_eq!(config_findings(out), vec!["~/.claude/settings.json turns Claude Code's checks off: x".to_string()]);
    }
}
