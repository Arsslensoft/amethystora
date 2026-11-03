//! amethystora-plugin-host: the plugin system's user service, and the commands `ame plugin` is made of
//! (/usr/libexec/amethystora-plugins runs them). With no arguments it is the service, `org.amethystora.Plugins`, which runs
//! while a plugin is on. The commands read the store and never write it: that is amethystora-plugin-root's, behind pkexec.
//!
//!   check DIR [--json]        what installing would refuse, and what the author should know (runs nothing)
//!   pack DIR                  a tar of the folder on standard output, for `amethystora-plugin-root prepare`
//!   list [--json]             the account's plugins
//!   show ID [--json]          one of them: what it may do, what it asked for, its files, its sandbox
//!   verify [ID]               every file of what is installed against the digest that was approved
//!   history [ID] [--json]     what was installed, granted, blocked and removed
//!   sandbox ID [ACTION]       the command line it runs under, printed and never run
//!   try ID [ACTION]           one run in the sandbox, with what it printed as the shell would be given it
//!   enable ID | disable ID    whether it is on, which the account decides (it can only stay inside what was approved)
//!   shim SOCKET -- COMMAND    inside a sandbox: a forwarder to the host's proxy (sandbox.rs starts it)

mod service;

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use amethystora_plugin_host::check;
use amethystora_plugin_host::host::{Env, Runner, load};
use amethystora_plugin_host::manifest::{Manifest, valid_id};
use amethystora_plugin_host::shim;
use amethystora_plugin_host::store::Grant;
use amethystora_plugin_host::tree;
use serde_json::{Value, json};

fn fail(why: &str) -> ExitCode {
    eprintln!("amethystora-plugin-host: {why}");
    ExitCode::from(1)
}

fn usage() -> ExitCode {
    eprintln!("usage: amethystora-plugin-host [check|pack|list|show|verify|history|sandbox|try|enable|disable] ...");
    ExitCode::from(64)
}

/// A time as the history prints it, UTC (days since 1970 to a calendar date, as Howard Hinnant's algorithm has it)
fn iso(secs: u64) -> String {
    let (days, rest) = ((secs / 86400) as i64, secs % 86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}", rest / 3600, rest % 3600 / 60)
}

fn id_of(args: &[String], at: usize) -> Result<String, ExitCode> {
    match args.get(at) {
        Some(id) if valid_id(id, true).is_ok() => Ok(id.clone()),
        _ => Err(usage()),
    }
}

fn read_manifest(env: &Env, id: &str, grant: &Grant) -> Option<Manifest> {
    let text = std::fs::read_to_string(env.store.folder_of(id, grant).join("manifest.json")).ok()?;
    Manifest::parse_with(&text, grant.shipped).ok()
}

fn summary(env: &Env, id: &str, grant: &Grant) -> Value {
    let manifest = read_manifest(env, id, grant);
    let verified = env.store.verify(&env.store.folder_of(id, grant), &grant.digest);
    json!({
        "id": id,
        "name": manifest.as_ref().map(|m| m.name.as_str()).unwrap_or(id),
        "version": manifest.as_ref().map(|m| m.version.as_str()).unwrap_or(""),
        "license": manifest.as_ref().map(|m| m.license.as_str()).unwrap_or(""),
        "digest": grant.digest,
        "previous": grant.previous,
        "shipped": grant.shipped,
        "enabled": env.enabled().iter().any(|e| e == id),
        "verified": verified.is_ok(),
        "problem": verified.err(),
        "granted": grant.perms,
        "asked": manifest.as_ref().map(|m| m.permissions().into_iter().collect::<Vec<_>>()).unwrap_or_default(),
        "why": manifest.as_ref().map(|m| m.needs.iter().map(|n| (n.perm.clone(), Value::from(n.why.clone()))).collect::<serde_json::Map<_, _>>()).unwrap_or_default(),
        "blocked": grant.blocked,
        "blocked_hosts": grant.blocked_hosts,
        "limits": grant.limits,
        "contributes": manifest.as_ref().map(|m| m.contributes.clone()).unwrap_or_default(),
        "actions": manifest.as_ref().map(|m| m.actions.clone()).unwrap_or_default(),
    })
}

fn cmd_check(args: &[String]) -> ExitCode {
    let Some(dir) = args.first() else { return usage() };
    let report = check::check(Path::new(dir));
    if args.iter().any(|a| a == "--json") {
        let perms: Vec<String> = report.manifest.iter().flat_map(|m| m.permissions()).collect();
        println!("{}", json!({"ok": report.errors.is_empty(), "errors": report.errors, "warnings": report.warnings, "permissions": perms}));
    } else {
        report.errors.iter().for_each(|e| println!("error\t{e}"));
        report.warnings.iter().for_each(|w| println!("warning\t{w}"));
        if let Some(m) = &report.manifest {
            println!("id\t{}\nname\t{}\nversion\t{}", m.id, m.name, m.version);
            m.needs.iter().for_each(|n| println!("perm\t{}\t{}", n.perm, n.why));
        }
    }
    if report.errors.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(1) }
}

fn cmd_pack(args: &[String]) -> ExitCode {
    let Some(dir) = args.first() else { return usage() };
    match tree::read_dir(Path::new(dir)).and_then(|entries| tree::write_tar(&entries)) {
        Ok(tar) => match std::io::Write::write_all(&mut std::io::stdout().lock(), &tar) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(&e.to_string()),
        },
        Err(e) => fail(&e),
    }
}

fn cmd_list(env: &Env, json_out: bool) -> ExitCode {
    let grants = match env.store.grants(env.uid) {
        Ok(g) => g,
        Err(e) => return fail(&e),
    };
    let all: Vec<Value> = grants.plugins.iter().map(|(id, g)| summary(env, id, g)).collect();
    if json_out {
        println!("{}", Value::Array(all));
    } else {
        for p in &all {
            let state = if p["verified"] == false { "changed" } else if p["enabled"] == true { "on" } else { "off" };
            println!("{}\t{}\t{}\t{}", p["id"].as_str().unwrap_or(""), p["version"].as_str().unwrap_or(""), state, p["name"].as_str().unwrap_or(""));
        }
    }
    ExitCode::SUCCESS
}

fn cmd_show(env: &Env, id: &str, json_out: bool) -> ExitCode {
    let grants = match env.store.grants(env.uid) {
        Ok(g) => g,
        Err(e) => return fail(&e),
    };
    let Some(grant) = grants.plugins.get(id) else { return fail(&format!("{id} is not installed")) };
    let p = summary(env, id, grant);
    // The command line it would run under, without starting anything
    let sandbox = load(env, id).ok().and_then(|l| Runner::new(Arc::new(env.clone()), Arc::new(|_, _| {})).command_line(&l, None, false).ok());
    if json_out {
        let mut p = p;
        p["sandbox"] = sandbox.map(|s| s.join(" ")).into();
        println!("{p}");
        return ExitCode::SUCCESS;
    }
    for key in ["id", "name", "version", "license", "digest"] {
        println!("{key}\t{}", p[key].as_str().unwrap_or(""));
    }
    println!("previous\t{}\nshipped\t{}\nenabled\t{}", p["previous"].as_str().unwrap_or(""), p["shipped"], p["enabled"]);
    println!("verified\t{}", p["problem"].as_str().unwrap_or("yes"));
    let (granted, asked) = (p["granted"].as_array().cloned().unwrap_or_default(), p["asked"].as_array().cloned().unwrap_or_default());
    for perm in &asked {
        let name = perm.as_str().unwrap_or("");
        let word = if granted.contains(perm) { "granted" } else { "asked" };
        println!("{word}\t{name}\t{}", p["why"][name].as_str().unwrap_or(""));
    }
    if p["blocked"] == true {
        println!("blocked\tall");
    }
    for h in p["blocked_hosts"].as_array().into_iter().flatten() {
        println!("blocked\t{}", h.as_str().unwrap_or(""));
    }
    for (host, bytes) in &grant.limits {
        println!("limit\t{host}\t{bytes}");
    }
    if let Some(line) = sandbox {
        println!("sandbox\t{}", line.join(" "));
    }
    ExitCode::SUCCESS
}

fn cmd_verify(env: &Env, only: Option<&String>) -> ExitCode {
    let grants = match env.store.grants(env.uid) {
        Ok(g) => g,
        Err(e) => return fail(&e),
    };
    let mut bad = false;
    for (id, grant) in grants.plugins.iter().filter(|(id, _)| only.is_none_or(|o| o == *id)) {
        match env.store.verify(&env.store.folder_of(id, grant), &grant.digest) {
            Ok(()) => println!("ok\t{id}"),
            Err(e) => {
                bad = true;
                println!("changed\t{id}\t{e}");
            }
        }
    }
    if only.is_some_and(|o| !grants.plugins.contains_key(o)) {
        return fail("that plugin is not installed");
    }
    if bad { ExitCode::from(1) } else { ExitCode::SUCCESS }
}

fn cmd_history(env: &Env, only: Option<&String>, json_out: bool) -> ExitCode {
    for line in env.store.history(env.uid).into_iter().filter(|l| only.is_none_or(|o| l["id"].as_str() == Some(o.as_str()))) {
        if json_out {
            println!("{line}");
        } else {
            let perms: Vec<&str> = line["perms"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            println!("{}\t{}\t{}\t{}", iso(line["t"].as_u64().unwrap_or(0)), line["action"].as_str().unwrap_or(""), line["id"].as_str().unwrap_or(""), perms.join(" "));
        }
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    let json_out = rest.iter().any(|a| a == "--json");
    let positional: Vec<String> = rest.iter().filter(|a| !a.starts_with("--")).cloned().collect();
    match args.first().map(String::as_str) {
        None | Some("serve") => match Env::production() {
            Ok(env) => service::serve(env),
            Err(e) => fail(&e),
        },
        Some("check") => cmd_check(rest),
        Some("pack") => cmd_pack(rest),
        Some("shim") => match rest.iter().position(|a| a == "--") {
            Some(at) if at == 1 && rest.len() > 2 => ExitCode::from(shim::run(&rest[0], &rest[at + 1..]).clamp(0, 255) as u8),
            _ => usage(),
        },
        Some(cmd @ ("list" | "show" | "verify" | "history" | "sandbox" | "try" | "enable" | "disable")) => {
            let env = match Env::production() {
                Ok(env) => env,
                Err(e) => return fail(&e),
            };
            match cmd {
                "list" => cmd_list(&env, json_out),
                "verify" => cmd_verify(&env, positional.first()),
                "history" => cmd_history(&env, positional.first(), json_out),
                _ => {
                    let id = match id_of(&positional, 0) {
                        Ok(id) => id,
                        Err(code) => return code,
                    };
                    match cmd {
                        "show" => cmd_show(&env, &id, json_out),
                        "sandbox" => match load(&env, &id).and_then(|l| Runner::new(Arc::new(env.clone()), Arc::new(|_, _| {})).command_line(&l, positional.get(1).map(String::as_str), false)) {
                            Ok(line) => {
                                println!("{}", line.join(" "));
                                ExitCode::SUCCESS
                            }
                            Err(e) => fail(&e),
                        },
                        "try" => match Runner::new(Arc::new(env.clone()), Arc::new(|_, _| {})).run(&id, positional.get(1).map(String::as_str)) {
                            Ok(out) => {
                                println!("{out}");
                                ExitCode::SUCCESS
                            }
                            Err(e) => fail(&e),
                        },
                        // On is the account's, and only for what was approved: the host runs nothing that is not granted
                        _ => match env.store.grants(env.uid).map(|g| g.plugins.contains_key(&id)) {
                            Ok(true) => env.set_enabled(&id, cmd == "enable").map_or_else(|e| fail(&e), |_| ExitCode::SUCCESS),
                            Ok(false) if cmd == "disable" => env.set_enabled(&id, false).map_or_else(|e| fail(&e), |_| ExitCode::SUCCESS),
                            Ok(false) => fail(&format!("{id} is not installed")),
                            Err(e) => fail(&e),
                        },
                    }
                }
            }
        }
        _ => usage(),
    }
}

#[cfg(test)]
mod tests {
    use super::iso;

    #[test]
    fn dates() {
        assert_eq!(iso(0), "1970-01-01 00:00");
        assert_eq!(iso(1_700_000_000), "2023-11-14 22:13");
    }
}
