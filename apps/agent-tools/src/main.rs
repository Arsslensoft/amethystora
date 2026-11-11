//! amethystora-agent-tools: the AI agent's tools, as an MCP server and as a command.
//!
//!   amethystora-agent-tools mcp                      the MCP server, on stdin and stdout (the agent starts it)
//!   amethystora-agent-tools search QUERY... [-k N]   the same answers as the tools, for a person or an agent
//!   amethystora-agent-tools read ID                  without MCP (amethystora-agent search|read|state)
//!   amethystora-agent-tools state TOPIC [FILTER]
//!   amethystora-agent-tools history [ID]
//!   amethystora-agent-tools undo ID                  the tools' own undo of one change, for the live pill's Undo
//!   amethystora-agent-tools plugin-change PLUGIN ENTRY VALUE
//!                                                    a plugin's change of a Control entry it was granted (the plugin host's)
//!   amethystora-agent-tools manual-search [-k N] (-- WORDS... | --stdin --)
//!                                                    the manual's pages that answer WORDS, as JSON, for the
//!                                                    desktop's search: no policy asked, nothing recorded
//!   amethystora-agent-tools files-index [--dir DIR] [--model DIR]
//!                                                    files by meaning: JSON lines {path, modified, text} on stdin
//!                                                    into the account's own index (paths and vectors only)
//!   amethystora-agent-tools files-search [-k N] [--dir DIR] [--model DIR] (-- WORDS... | --stdin --)
//!                                                    the files nearest to WORDS, as JSON, for the desktop's search
//!   amethystora-agent-tools routines check FILE | list | show ID | add FILE | pause|resume|remove ID | run ID | undo ID | triggers
//!                                                    little routines, sealed in the Vault (ame agent routines)
//!   amethystora-agent-tools policy [--json|--level|--ceiling]
//!                                                    what this account may use, and the administrator's settings
//!   amethystora-agent-tools conf allow act|know|off|default [ACCOUNT] | conf root MODE
//!                                                    as root: the administrator's lock and root mode (ame agent lock, root)
//!   amethystora-agent-tools index --write DIR [--root ROOT] [--sources-only] [--model DIR]
//!                                                    the index, when the image is built
//!   amethystora-agent-tools eval GOLDEN [--dir DIR] [--min RECALL]
//!                                                    recall@3 of the golden questions, for each ranking

use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use amethystora_agent_tools::files;
use amethystora_agent_tools::history;
use amethystora_agent_tools::index::build::{self, Sources};
use amethystora_agent_tools::index::{self, Index, Ranking};
use amethystora_agent_tools::mcp;
use amethystora_agent_tools::policy::{self, Policy};
use amethystora_agent_tools::server::Server;
use serde_json::{Value, json};

fn usage() -> ExitCode {
    eprintln!("usage: amethystora-agent-tools mcp | search QUERY... | read ID | state TOPIC [FILTER] | history [ID] | undo ID | policy [--json]");
    eprintln!("       amethystora-agent-tools manual-search [-k N] (-- WORDS... | --stdin --)");
    eprintln!("       amethystora-agent-tools files-index [--dir DIR] [--model DIR] | files-search [-k N] [--dir DIR] [--model DIR] (-- WORDS... | --stdin --)");
    eprintln!("       amethystora-agent-tools index --write DIR [--root ROOT] [--sources-only] [--model DIR] | eval GOLDEN [--dir DIR] [--min R]");
    ExitCode::from(64)
}

/// What was typed in the desktop's search, read from standard input (--stdin): a command line is readable by every account
/// while the program runs and kept by an exec audit. At most 1 KiB, a longer text cut where it is
fn typed_words() -> String {
    let mut bytes = Vec::new();
    let _ = std::io::Read::read_to_end(&mut std::io::Read::take(std::io::stdin().lock(), 1024), &mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The value after an option, as in --root /
fn option(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|arg| arg == name).and_then(|at| args.get(at + 1)).cloned()
}

fn say(result: Result<String, String>) -> ExitCode {
    match result {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(text) => {
            eprintln!("{text}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first() else { return usage() };
    let rest = &args[1..];
    match command.as_str() {
        "mcp" => {
            // Named in the account's runtime folder while it runs, for the live pill and Stop
            let _session = history::Session::start();
            let server = Server::default();
            let stdin = std::io::stdin();
            let served = mcp::serve(&server, BufReader::new(stdin.lock()), std::io::stdout().lock());
            // The agent ended: so does the root helper of its session
            server.root.close();
            if served.is_ok() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        "search" => {
            let mut words: Vec<String> = Vec::new();
            let mut k = 4;
            let mut at = 0;
            while at < rest.len() {
                if rest[at] == "-k" {
                    k = rest.get(at + 1).and_then(|n| n.parse().ok()).unwrap_or(4);
                    at += 2;
                    continue;
                }
                words.push(rest[at].clone());
                at += 1;
            }
            say(Server::default().call("search", &json!({"query": words.join(" "), "k": k})))
        }
        // The desktop's search: the manual alone, for everyone, whatever the agent's switches say; nothing but the
        // manual's passages of the index read, by words alone (no vectors, no model), nothing written down
        "manual-search" => {
            let mut k = 3;
            let mut at = 0;
            let mut stdin = false;
            while at < rest.len() && rest[at] != "--" {
                if rest[at] == "--stdin" {
                    stdin = true;
                    at += 1;
                } else if rest[at] == "-k" {
                    k = rest.get(at + 1).and_then(|n| n.parse::<usize>().ok()).unwrap_or(3);
                    at += 2;
                } else {
                    return usage();
                }
            }
            let words = if stdin { typed_words() } else { rest.get(at + 1..).unwrap_or_default().join(" ") };
            let found = if words.trim().is_empty() {
                Vec::new()
            } else {
                match Index::load_words(std::path::Path::new(index::DIR), |passage| passage.kind == "manual") {
                    Ok(index) => index::manual_search(&index, &words, k.clamp(1, 5)),
                    Err(_) => Vec::new(),
                }
            };
            println!("{}", Value::Array(found));
            ExitCode::SUCCESS
        }
        // Files by meaning (plans/ui-ideas/spec-later.md, R5-37): the account's own index of Documents, from what
        // amethystora-file-meaning hands over; nothing of the agent's switches is asked, nothing recorded
        "files-index" => {
            let dir = option(rest, "--dir").map(PathBuf::from).unwrap_or_else(files::dir);
            let model = option(rest, "--model").unwrap_or_else(|| files::MODEL.to_string());
            let model = match index::embed::Model::load(std::path::Path::new(&model)) {
                Ok(model) => model,
                Err(error) => {
                    eprintln!("files-index: {error}");
                    return ExitCode::FAILURE;
                }
            };
            match files::write(&dir, &model, std::io::stdin().lock()) {
                Ok(count) => {
                    println!("{}", json!({"files": count}));
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("files-index: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        "files-search" => {
            let words_at = rest.iter().position(|arg| arg == "--").map(|at| at + 1).unwrap_or(rest.len());
            let options = &rest[..words_at.saturating_sub(1).min(rest.len())];
            let k = option(options, "-k").and_then(|n| n.parse::<usize>().ok()).unwrap_or(3).clamp(1, 10);
            let dir = option(options, "--dir").map(PathBuf::from).unwrap_or_else(files::dir);
            let model = option(options, "--model").unwrap_or_else(|| files::MODEL.to_string());
            let words = if options.iter().any(|arg| arg == "--stdin") { typed_words() } else { rest[words_at..].join(" ") };
            let found = match index::embed::Model::load(std::path::Path::new(&model)) {
                Ok(model) if !words.trim().is_empty() => files::search(&dir, &model, &words, k),
                _ => Vec::new(),
            };
            println!("{}", Value::Array(found));
            ExitCode::SUCCESS
        }
        // Little routines (R7-15): sealed in the Vault, run by the desktop's triggers
        "routines" => amethystora_agent_tools::routines::main(rest),
        "read" if rest.len() == 1 => say(Server::default().call("read", &json!({"id": rest[0]}))),
        "state" if !rest.is_empty() => {
            say(Server::default().call("state", &json!({"topic": rest[0], "filter": rest.get(1).cloned().unwrap_or_default()})))
        }
        "history" => {
            let args = rest.first().map(|id| json!({"id": id, "wait": 0})).unwrap_or(json!({}));
            say(Server::default().call("history", &args))
        }
        // The person's Undo, from the live pill (amethystora-agent undo ID): the tools' own undo, under the same
        // checks as the agent's, and this command waits for it to end, an install or a terminal's answer included
        "undo" if rest.len() == 1 => {
            let server = Server::default();
            let said = server.call("undo", &json!({"id": rest[0]}));
            server.jobs.wait_all(std::time::Duration::from_secs(3600));
            say(said)
        }
        // A plugin's change (the plugin host asks, for an entry it was granted and after a click): no terminal, the never list, recorded
        "plugin-change" if rest.len() == 3 => {
            let valid = rest[0].len() <= 128 && rest[0].bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-');
            if !valid {
                return ExitCode::from(64);
            }
            match amethystora_agent_tools::act::plugin_change(&rest[0], &rest[1], &rest[2]) {
                Ok(said) => {
                    println!("{said}");
                    ExitCode::SUCCESS
                }
                Err(why) => {
                    eprintln!("{why}");
                    ExitCode::FAILURE
                }
            }
        }
        "conf" => match policy::change_conf(std::path::Path::new(policy::CONF), rest) {
            Ok(conf) => {
                // In the words a note takes: lower case, no spaces
                let said = format!("allow={}:root={}:accounts={}", conf.allow.word(), conf.root.word(), conf.allow_for.len());
                // Noted for the security watcher, which then says the change without alarm, as for a switch's own command
                let _ = std::process::Command::new("/usr/libexec/amethystora-security-config")
                    .args(["note", "agent-conf", &said])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                println!("{}", conf.render().lines().filter(|line| !line.starts_with('#')).collect::<Vec<_>>().join("\n"));
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        "policy" => {
            let policy = Policy::current();
            if rest.iter().any(|arg| arg == "--json") {
                println!("{}", policy.json());
            } else if rest.iter().any(|arg| arg == "--level") {
                println!("{}", policy.level().word());
            } else if rest.iter().any(|arg| arg == "--ceiling") {
                println!("{}", policy.ceiling().word());
            } else {
                println!(
                    "Level: {} (agentic features {}, changes {}), most allowed: {}, root commands: {}",
                    policy.level().word(),
                    if policy.level1 { "on" } else { "off" },
                    if policy.level2 { "on" } else { "off" },
                    policy.ceiling().word(),
                    policy.conf.root.word()
                );
            }
            ExitCode::SUCCESS
        }
        "index" => {
            let Some(out) = option(rest, "--write").map(PathBuf::from) else { return usage() };
            let sources = Sources {
                root: option(rest, "--root").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/")),
                generated: !rest.iter().any(|arg| arg == "--sources-only"),
            };
            let model = option(rest, "--model").map(PathBuf::from).unwrap_or_else(|| out.join("model"));
            match build::write(&sources, &out, &model) {
                Ok(meta) => {
                    println!("{meta}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("index: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        "eval" => {
            let Some(golden) = rest.first() else { return usage() };
            let dir = option(rest, "--dir").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(index::DIR));
            let min: f32 = option(rest, "--min").and_then(|n| n.parse().ok()).unwrap_or(0.9);
            let set = match std::fs::read_to_string(golden) {
                Ok(text) => build::golden(&text),
                Err(error) => {
                    eprintln!("{golden}: {error}");
                    return ExitCode::FAILURE;
                }
            };
            let index = match Index::load(&dir) {
                Ok(index) => index,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::FAILURE;
                }
            };
            let (hits, asked, missed) = build::recall(&index, &set);
            let mut report = serde_json::Map::new();
            for (ranking, count) in &hits {
                let name = match ranking {
                    Ranking::Words => "words",
                    Ranking::Meaning => "meaning",
                    Ranking::Fused => "fused",
                };
                report.insert(name.into(), json!(*count as f32 / asked.max(1) as f32));
            }
            report.insert("questions".into(), json!(asked));
            report.insert("meaning_loaded".into(), json!(index.has_meaning()));
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{}", Value::Object(report.clone()));
            for miss in &missed {
                let _ = writeln!(out, "missed: {miss}");
            }
            let fused = report["fused"].as_f64().unwrap_or(0.0) as f32;
            if asked == 0 || fused < min { ExitCode::FAILURE } else { ExitCode::SUCCESS }
        }
        _ => usage(),
    }
}
