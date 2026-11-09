//! amethystora-vault-cli: what `ame vault` runs (plans/done/PLAN-vault.md). It talks to the service's socket, and reads a password from
//! the terminal with its echo off or from standard input, never from an argument, which any process can read. Its words come
//! from `vault.just`, through `--say-...` options, which is where gettext has them; it prints only values and tab-separated lines.
//!
//! A vault's name, and whether it is hidden, are read from standard input like a password, never taken from an argument: a
//! command line is in the journal when polkit logs a prompt that failed, and a hidden vault must not be there.
//!
//! Exits 0, or the refusal's number: 1 refused, 2 closed, 3 not set up, 4 too large, 5 bad request, 6 failed, 7 typed twice
//! differently or not typed back, 10 to 17 and 18 to 22 a protected file or a tool's helper (`tools.rs`), 64 called wrongly. `run` exits as its
//! command did.

#[path = "cli_items.rs"]
mod items;
#[path = "tools.rs"]
mod tools;
#[path = "cli_moving.rs"]
mod moving;

use std::io::{BufRead, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

use amethystora_vault_client::{exchange, Refusal, SOCKET};
use amethystora_vault_core::protocol::encode;
use amethystora_vault_core::seal;
use amethystora_vault_core::slots::RecoveryKey;
// The allocator that wipes what it is given back
use amethystora_vault_service as _;
use serde_json::{json, Value};
use zeroize::{Zeroize, Zeroizing};

mod cli_sync;

/// A person has five minutes to answer the service's prompt, and it answers for them after that
const PATIENCE: Duration = Duration::from_secs(6 * 60);
const RELYING_PARTY: &str = "amethystora-vault";
pub(crate) const USAGE: u8 = 64;
const DIFFERENT: u8 = 7;

fn socket() -> PathBuf {
    std::env::var_os("AMETHYSTORA_VAULT_SOCKET").map_or_else(|| PathBuf::from(SOCKET), PathBuf::from)
}

/// The machine's root will not open, and only `ame vault root rotate` makes one that does
const ROOT_UNOPENED: u8 = 23;
/// A set-up let go by a lock, a sleep or its time, before its recovery key was typed back
const NO_DRAFT: u8 = 24;

fn code_of(why: Refusal) -> u8 {
    match why {
        Refusal::Refused => 1,
        Refusal::Closed => 2,
        Refusal::NotSetUp => 3,
        Refusal::TooLarge => 4,
        Refusal::BadRequest => 5,
        Refusal::Failed => 6,
    }
}

/// One request: its answer if it was granted, otherwise the code to exit with
pub(crate) fn call(request: Value) -> Result<Value, u8> {
    let mut request = request;
    request["v"] = json!(1);
    let answer = exchange(&socket(), &request, PATIENCE).map_err(code_of)?;
    if answer["ok"] == json!(true) {
        return Ok(answer);
    }
    if answer["problem"] == json!("root-unopened") {
        return Err(ROOT_UNOPENED);
    }
    if answer["problem"] == json!("no-draft") {
        return Err(NO_DRAFT);
    }
    Err(code_of(Refusal::from_name(answer["why"].as_str().unwrap_or("failed"))))
}

/// The options given as `--name=value`, and the rest in order
pub(crate) struct Args {
    pub(crate) words: Vec<String>,
    options: Vec<(String, String)>,
}

impl Args {
    fn read() -> Args {
        // Lossy: a command's arguments are only read here in passing, and a name that is not UTF-8 must not stop this
        Args::parse(std::env::args_os().skip(1).map(|arg| arg.to_string_lossy().into_owned()))
    }

    fn parse(arguments: impl Iterator<Item = String>) -> Args {
        let (mut words, mut options) = (Vec::new(), Vec::new());
        let mut only_words = false;
        for arg in arguments {
            // A bare `--` ends the options: what follows is words, even a path or a title that starts with dashes
            if !only_words && arg == "--" {
                only_words = true;
                continue;
            }
            if only_words {
                words.push(arg);
                continue;
            }
            match arg.strip_prefix("--") {
                Some(option) => match option.split_once('=') {
                    Some((name, value)) => options.push((name.to_string(), value.to_string())),
                    None => options.push((option.to_string(), String::new())),
                },
                None => words.push(arg),
            }
        }
        Args { words, options }
    }

    pub(crate) fn option(&self, name: &str) -> Option<&str> {
        self.options.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
    }

    pub(crate) fn say(&self, name: &str, otherwise: &'static str) -> String {
        self.option(name).unwrap_or(otherwise).to_string()
    }

    pub(crate) fn word(&self, at: usize) -> Option<&str> {
        self.words.get(at).map(String::as_str)
    }
}

/// A line of secret: from the terminal with its echo off, or from standard input. The prompt goes to standard error.
pub(crate) fn read_secret(prompt: &str) -> Option<Zeroizing<String>> {
    read_line(prompt, false).filter(|line| !line.is_empty())
}

/// A line of text, from the terminal (with its echo on or off) or from standard input; empty if the line is, None at the end
fn read_line(prompt: &str, echo: bool) -> Option<Zeroizing<String>> {
    let stdin = std::io::stdin();
    let terminal = stdin.is_terminal();
    let mut saved: libc::termios = unsafe { std::mem::zeroed() };
    if terminal {
        eprint!("{prompt}");
        // SAFETY: a termios value this function owns, for standard input, which is a terminal
        unsafe {
            libc::tcgetattr(0, &raw mut saved);
            let mut quiet = saved;
            if !echo {
                quiet.c_lflag &= !libc::ECHO;
            }
            libc::tcsetattr(0, libc::TCSANOW, &raw const quiet);
        }
    }
    let mut line = Zeroizing::new(String::new());
    // Bounded: a password's limit is the service's, and this is well over it
    let read = stdin.lock().take(8192).read_line(&mut line);
    if terminal {
        // SAFETY: as above
        unsafe { libc::tcsetattr(0, libc::TCSANOW, &raw const saved) };
        if !echo {
            eprintln!();
        }
    }
    // Nothing read at all is not an empty line
    if read.ok()? == 0 {
        return None;
    }
    let trimmed = line.trim_end_matches(['\r', '\n']).len();
    line.truncate(trimmed);
    Some(line)
}

/// A new password, twice from a terminal, once from standard input
pub(crate) fn new_password(args: &Args) -> Result<Zeroizing<String>, u8> {
    let first = read_secret(&args.say("say-new", "New vault password: ")).ok_or(DIFFERENT)?;
    if std::io::stdin().is_terminal() {
        let again = read_secret(&args.say("say-again", "Once more: ")).ok_or(DIFFERENT)?;
        if !seal::same(first.as_bytes(), again.as_bytes()) {
            return Err(DIFFERENT);
        }
    }
    Ok(first)
}

/// Says the recovery key, and has it typed back: until then nothing is kept
fn type_back(args: &Args, recovery: &str) -> Result<(), u8> {
    println!("{}", args.say("say-recovery", "Your recovery key. Write it down: it is the only way back if every other way in is lost."));
    println!("{recovery}");
    std::io::stdout().flush().ok();
    for _ in 0..3 {
        let Some(typed) = read_secret(&args.say("say-type-back", "Type the recovery key back: ")) else { break };
        match call(json!({"op": "confirm", "recovery": encode(typed.as_bytes())})) {
            Ok(_) => return Ok(()),
            Err(1) => {}
            Err(code) => return Err(code),
        }
    }
    Err(DIFFERENT)
}

fn setup(args: &Args) -> Result<(), u8> {
    let method = args.word(1).unwrap_or("password");
    let mut request = json!({"op": "setup", "method": method});
    if let Some(name) = args.option("name") {
        request["name"] = json!(name);
    }
    if method == "password" {
        let password = new_password(args)?;
        request["secret"] = json!(encode(password.as_bytes()));
    }
    let answer = call(request)?;
    moving::mark();
    let recovery = Zeroizing::new(answer["recovery"].as_str().unwrap_or("").to_string());
    type_back(args, &recovery)
}

fn new_vault(args: &Args) -> Result<(), u8> {
    let name = read_line(&args.say("say-name", "Name for the vault: "), true).filter(|name| !name.is_empty()).ok_or(USAGE)?;
    let hidden = read_line(&args.say("say-hidden", "To make it hidden, type: hidden "), true).ok_or(USAGE)?;
    let password = new_password(args)?;
    let answer = call(json!({"op": "new", "name": name.as_str(), "hidden": hidden.as_str() == "hidden", "secret": encode(password.as_bytes())}))?;
    let recovery = Zeroizing::new(answer["recovery"].as_str().unwrap_or("").to_string());
    type_back(args, &recovery)
}

/// One of libfido2's tools, as `apps/notes/src/passkeys.rs` runs them: what it needs as lines on standard input, the PIN asked on
/// the terminal by the tool itself
fn fido(tool: &str, args: &[&str], lines: &[String]) -> Result<Vec<String>, u8> {
    let mut child = Command::new(tool).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn().map_err(|_| 6u8)?;
    if let Some(mut stdin) = child.stdin.take() {
        let input: String = lines.iter().map(|line| format!("{line}\n")).collect();
        stdin.write_all(input.as_bytes()).map_err(|_| 6u8)?;
    }
    let output = child.wait_with_output().map_err(|_| 6u8)?;
    if !output.status.success() {
        return Err(6);
    }
    Ok(String::from_utf8_lossy(&output.stdout).split('\n').map(str::to_string).collect())
}

fn random_base64(count: usize) -> String {
    let mut bytes = vec![0u8; count];
    seal::random(&mut bytes);
    encode(&bytes)
}

/// The one security key plugged in
fn security_key() -> Result<String, u8> {
    let listed = fido("fido2-token", &["-L"], &[])?;
    let devices: Vec<&String> = listed.iter().filter(|line| line.contains(": vendor=0x")).collect();
    match devices.as_slice() {
        [one] => Ok(one.split(": vendor=0x").next().unwrap_or_default().to_string()),
        _ => Err(1),
    }
}

/// The secret this key computes for this credential and salt, only after its PIN or a fingerprint: a touch alone must never open
/// a vault, and this asks with the PIN required
fn security_key_secret(device: &str, credential: &str, salt: &str) -> Result<Zeroizing<Vec<u8>>, u8> {
    let lines = [random_base64(32), RELYING_PARTY.to_string(), credential.to_string(), salt.to_string()];
    let printed = fido("fido2-assert", &["-G", "-h", "-t", "pin=true", device], &lines)?;
    let secret = printed.get(4).map(String::as_str).unwrap_or("");
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(secret).map(Zeroizing::new).map_err(|_| 6u8)
}

fn open_with_security_key() -> Result<Value, u8> {
    let methods = call(json!({"op": "methods"}))?;
    let device = security_key()?;
    for vault in methods["vaults"].as_array().into_iter().flatten() {
        for slot in vault["slots"].as_array().into_iter().flatten().filter(|slot| slot["kind"] == "security-key") {
            let Ok(secret) = security_key_secret(&device, slot["credential"].as_str().unwrap_or(""), slot["salt"].as_str().unwrap_or("")) else { continue };
            if let Ok(answer) = call(json!({"op": "unlock", "method": "security-key", "slot": slot["id"], "secret": encode(&secret)})) {
                return Ok(answer);
            }
        }
    }
    Err(1)
}

fn open(args: &Args) -> Result<(), u8> {
    let status = call(json!({"op": "status"}))?;
    if status["set_up"] != json!(true) {
        return Err(3);
    }
    let methods: Vec<String> = status["vaults"].as_array().into_iter().flatten().filter(|vault| vault["first"] == json!(true)).flat_map(|vault| vault["methods"].as_array().cloned().unwrap_or_default()).filter_map(|method| method.as_str().map(str::to_string)).collect();
    let has = |name: &str| methods.iter().any(|method| method == name);
    let chosen = args.word(1).map(str::to_string).unwrap_or_else(|| {
        if has("approval") {
            "approval"
        } else if has("no-prompt") {
            "no-prompt"
        } else {
            "password"
        }
        .to_string()
    });
    let answer = match chosen.as_str() {
        "approval" | "no-prompt" => call(json!({"op": "unlock", "method": chosen}))?,
        "security-key" => open_with_security_key()?,
        "password" | "recovery" => {
            let typed = read_secret(&args.say("say-password", "Vault password or recovery key: ")).ok_or(1u8)?;
            // A recovery key has a shape and a checksum no password is likely to, and a hidden vault's opens the same way
            let method = if RecoveryKey::parse(&typed).is_some() { "recovery" } else { "password" };
            call(json!({"op": "unlock", "method": method, "secret": encode(typed.as_bytes())}))?
        }
        _ => return Err(USAGE),
    };
    for vault in answer["opened"].as_array().into_iter().flatten() {
        println!("{}\t{}", vault["id"].as_str().unwrap_or(""), vault["name"].as_str().unwrap_or(""));
    }
    Ok(())
}

fn status(args: &Args) -> Result<(), u8> {
    let answer = call(json!({"op": "status"}))?;
    if answer["set_up"] == json!(true) {
        moving::mark();
    }
    if args.option("json").is_some() {
        println!("{answer}");
        return Ok(());
    }
    println!("set-up\t{}", if answer["set_up"] == json!(true) { "yes" } else { "no" });
    println!("tpm\t{}", if answer["tpm"] == json!(true) { "yes" } else { "no" });
    for vault in answer["vaults"].as_array().into_iter().flatten() {
        let methods: Vec<&str> = vault["methods"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        println!(
            "vault\t{}\t{}\t{}\t{}\t{}",
            vault["id"].as_str().unwrap_or(""),
            if vault["open"] == json!(true) { "open" } else { "closed" },
            if vault["first"] == json!(true) { "first" } else { "-" },
            methods.join(","),
            vault["name"].as_str().unwrap_or(""),
        );
    }
    Ok(())
}

fn methods(args: &Args) -> Result<(), u8> {
    match args.word(1) {
        None => {
            let answer = call(json!({"op": "methods"}))?;
            println!("{answer}");
            Ok(())
        }
        Some("add") => match args.word(2) {
            Some(kind @ ("approval" | "no-prompt")) => call(json!({"op": "add-slot", "kind": kind})).map(|_| ()),
            Some("security-key") => add_security_key(args),
            Some("password") => change_password(args),
            _ => Err(USAGE),
        },
        Some("remove") => call(json!({"op": "remove-slot", "slot": args.word(2).ok_or(USAGE)?})).map(|_| ()),
        Some("background") => match args.word(2) {
            Some("on") => call(json!({"op": "background", "on": true})).map(|_| ()),
            Some("off") => call(json!({"op": "background", "on": false})).map(|_| ()),
            _ => Err(USAGE),
        },
        Some("idle") => {
            let minutes: u32 = args.word(2).and_then(|minutes| minutes.parse().ok()).ok_or(USAGE)?;
            call(json!({"op": "set-idle", "minutes": minutes})).map(|_| ())
        }
        _ => Err(USAGE),
    }
}

/// A credential on the key that is plugged in, then its secret, which seals the vault key once more. As Notes: protection level
/// 3 has the key refuse it without the PIN or a fingerprint, and nothing is stored on the key.
fn add_security_key(args: &Args) -> Result<(), u8> {
    let device = security_key()?;
    let name = args.word(3).unwrap_or("Security key");
    // SAFETY: getuid cannot fail
    let user = unsafe { libc::getuid() }.to_string();
    let lines = [random_base64(32), RELYING_PARTY.to_string(), user, random_base64(16)];
    let made = fido("fido2-cred", &["-M", "-h", "-c", "3", &device], &lines)?;
    let credential = made.get(4).ok_or(6u8)?.clone();
    let salt = random_base64(32);
    let secret = security_key_secret(&device, &credential, &salt)?;
    call(json!({"op": "add-security-key", "name": name, "credential": credential, "salt": salt, "secret": encode(&secret)})).map(|_| ())
}

fn change_password(args: &Args) -> Result<(), u8> {
    let status = call(json!({"op": "status"}))?;
    let open: Vec<&Value> = status["vaults"].as_array().into_iter().flatten().filter(|vault| vault["open"] == json!(true)).collect();
    // The vault asked for, or the only one open
    let target = match (args.option("vault"), open.as_slice()) {
        (Some(id), _) => id.to_string(),
        (None, [one]) => one["id"].as_str().unwrap_or("").to_string(),
        (None, []) => return Err(2),
        (None, _) => return Err(USAGE),
    };
    let chosen = open.iter().find(|vault| vault["id"] == json!(target)).ok_or(2u8)?;
    let has_password = chosen["methods"].as_array().into_iter().flatten().any(|method| method == "password");
    let mut request = json!({"op": "change-password", "vault": target});
    if has_password {
        let current = read_secret(&args.say("say-current", "Current vault password: ")).ok_or(1u8)?;
        request["current"] = json!(encode(current.as_bytes()));
    }
    let new = new_password(args)?;
    request["secret"] = json!(encode(new.as_bytes()));
    call(request).map(|_| ())
}

/// `item put KIND NAME [ID]`, the value as JSON on standard input, or as plain text with `--text`
fn item(args: &Args) -> Result<(), u8> {
    let print = |answer: Value| println!("{answer}");
    match args.word(1).ok_or(USAGE)? {
        "put" => {
            let (kind, name) = (args.word(2).ok_or(USAGE)?, args.word(3).ok_or(USAGE)?);
            let mut input = Zeroizing::new(String::new());
            std::io::stdin().lock().take(4 * 1024 * 1024).read_to_string(&mut input).map_err(|_| 6u8)?;
            let value = if args.option("text").is_some() {
                json!(input.trim_end_matches(['\r', '\n']))
            } else {
                serde_json::from_str(&input).map_err(|_| USAGE)?
            };
            let mut request = json!({"op": "put", "kind": kind, "name": name, "value": value});
            if let Some(id) = args.word(4) {
                request["id"] = json!(id);
            }
            if let Some(approval) = args.option("approval") {
                request["approval"] = json!(approval);
            }
            if let Some(meta) = args.option("meta") {
                request["meta"] = serde_json::from_str(meta).map_err(|_| USAGE)?;
            }
            if let Some(vault) = args.option("vault") {
                request["vault"] = json!(vault);
            }
            print(call(request)?);
        }
        "get" => {
            let answer = call(json!({"op": "get", "kind": args.word(2).ok_or(USAGE)?, "id": args.word(3).ok_or(USAGE)?}))?;
            match &answer["value"] {
                Value::String(text) => println!("{text}"),
                value => println!("{value}"),
            }
        }
        "list" => {
            let mut request = json!({"op": "list"});
            if let Some(kind) = args.word(2) {
                request["kind"] = json!(kind);
            }
            let answer = call(request)?;
            if args.option("json").is_some() {
                print(answer);
            } else {
                for item in answer["items"].as_array().into_iter().flatten() {
                    println!("{}\t{}\t{}\t{}", item["kind"].as_str().unwrap_or(""), item["id"].as_str().unwrap_or(""), item["approval"].as_str().unwrap_or(""), item["name"].as_str().unwrap_or(""));
                }
            }
        }
        "remove" => {
            call(json!({"op": "remove", "kind": args.word(2).ok_or(USAGE)?, "id": args.word(3).ok_or(USAGE)?}))?;
        }
        "approval" => {
            call(json!({"op": "set-approval", "kind": args.word(2).ok_or(USAGE)?, "id": args.word(3).ok_or(USAGE)?, "approval": args.word(4).ok_or(USAGE)?}))?;
        }
        _ => return Err(USAGE),
    }
    Ok(())
}

fn run(args: &Args) -> Result<(), u8> {
    match args.word(0).ok_or(USAGE)? {
        "status" | "vaults" => status(args),
        "setup" => setup(args),
        "open" => open(args),
        "lock" => call(json!({"op": "lock"})).map(|_| ()),
        "methods" => methods(args),
        "change-password" => change_password(args),
        "backup" => moving::backup(args),
        "restore" => moving::restore(args),
        "new" => new_vault(args),
        "rename" => {
            let name = read_line(&args.say("say-name", "Name for the vault: "), true).filter(|name| !name.is_empty()).ok_or(USAGE)?;
            let mut request = json!({"op": "rename", "name": name.as_str()});
            if let Some(vault) = args.option("vault") {
                request["vault"] = json!(vault);
            }
            call(request).map(|_| ())
        }
        "remove-this" => {
            let mut request = json!({"op": "remove-this"});
            if let Some(vault) = args.option("vault") {
                request["vault"] = json!(vault);
            }
            call(request).map(|_| ())
        }
        "forget" => call(json!({"op": "forget-everything"})).map(|_| ()),
        "item" => item(args),
        "sync" => cli_sync::run(args),
        "password" => items::password(args),
        "codes" => items::codes_command(args),
        "auth" => items::auth(args),
        "key" => items::key(args),
        "cert" => items::cert(args),
        "pdf" => items::pdf(args),
        "tls" => items::tls(args),
        "root" => items::root(args),
        "ssh" => tools::ssh(args),
        "get" => tools::get(args),
        "run" => tools::run(args),
        "protect" => tools::protect(args),
        "protected" => tools::protected(args),
        "unprotect" => tools::unprotect(args),
        "helpers" => tools::helpers(args),
        _ => Err(USAGE),
    }
}

fn main() -> ExitCode {
    let mut args = Args::read();
    let result = run(&args);
    for (_, value) in &mut args.options {
        value.zeroize();
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(USAGE) => {
            eprintln!("usage: amethystora-vault-cli status | setup [METHOD] | open [METHOD] | lock | methods [add KIND | remove SLOT | background on|off | idle MINUTES] | change-password | new | vaults | rename | remove-this | forget | item put|get|list|remove|approval ... | ssh on|off|status | get NAME [aws|kube] | run [VARIABLE=]NAME -- COMMAND... | protect PATH | protected [usual] | unprotect NAME | helpers [git|docker|aws|kube|claude on|off ...] | password generate|add|new|list|get|remove | codes add|list|next|used|remove | auth add|list|code|qr|remove | key new|import|list|public|export|remove | cert new|list|show|chain|export|renew|ca|remove | pdf sign FILE [CERT] | root [rotate] | backup | restore FILE | tls trust browsers on|off | sync connect ADDRESS|now|status|off|choose");
            ExitCode::from(USAGE)
        }
        Err(code) => ExitCode::from(code),
    }
}

#[cfg(test)]
mod tests {
    use super::Args;

    fn read(words: &[&str]) -> Args {
        Args::parse(words.iter().map(|word| word.to_string()))
    }

    #[test]
    fn a_bare_double_dash_ends_the_options() {
        let args = read(&["pdf", "sign", "--module=/x.so", "--", "--odd name.pdf", "-dash", "--"]);
        assert_eq!(args.words, ["pdf", "sign", "--odd name.pdf", "-dash", "--"]);
        assert_eq!(args.option("module"), Some("/x.so"));
        assert_eq!(args.option(""), None);
        assert_eq!(args.option("odd name.pdf"), None);
        // Without it, an option is an option
        let args = read(&["key", "export", "--out=f", "--plain"]);
        assert_eq!((args.option("out"), args.option("plain")), (Some("f"), Some("")));
    }
}
