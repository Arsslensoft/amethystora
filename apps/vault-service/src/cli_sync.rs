//! `ame vault sync ...`: connecting the vault to a Vaultwarden or Bitwarden server, and running it. The words come from
//! `vault.just` through `--say-...` options, which is where gettext has them; what is printed to a script is tab-separated.
//! A secret is read with its echo off, or from standard input, and travels to the service only as base64 in a request.

use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::os::unix::net::UnixStream;

use amethystora_vault_client::{exchange, MAX_LINE};
use amethystora_vault_core::protocol::encode;
use serde_json::{json, Value};

use super::{read_secret, socket, Args, PATIENCE, USAGE};

/// A request, and whatever the service answered, a refusal's reasons included
fn ask(request: Value) -> Result<Value, u8> {
    let mut request = request;
    request["v"] = json!(1);
    let answer = exchange(&socket(), &request, PATIENCE).map_err(super::code_of)?;
    if answer.get("wait").is_some() {
        return Err(1);
    }
    Ok(answer)
}

fn refusal_code(answer: &Value) -> u8 {
    super::code_of(amethystora_vault_client::Refusal::from_name(answer["why"].as_str().unwrap_or("failed")))
}

/// One connection to the service for a whole conversation: what a person says yes to is said yes to for the connection it was asked on,
/// so a connection to a server (the yes, then a code, then a certificate) is all made on this one
struct Talk {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
}

impl Talk {
    fn open() -> Result<Talk, u8> {
        let failed = || super::code_of(amethystora_vault_client::Refusal::Failed);
        let stream = UnixStream::connect(socket()).map_err(|_| failed())?;
        stream.set_read_timeout(Some(PATIENCE)).and_then(|()| stream.set_write_timeout(Some(PATIENCE))).map_err(|_| failed())?;
        let reader = BufReader::new(stream.try_clone().map_err(|_| failed())?);
        Ok(Talk { stream, reader })
    }

    fn ask(&mut self, request: Value) -> Result<Value, u8> {
        let failed = || super::code_of(amethystora_vault_client::Refusal::Failed);
        let mut request = request;
        request["v"] = json!(1);
        let mut line = zeroize::Zeroizing::new(request.to_string());
        line.push('\n');
        self.stream.write_all(line.as_bytes()).map_err(|_| failed())?;
        let mut answer = String::new();
        (&mut self.reader).take(MAX_LINE as u64).read_line(&mut answer).map_err(|_| failed())?;
        let answer: Value = serde_json::from_str(&answer).map_err(|_| failed())?;
        if answer.get("wait").is_some() {
            return Err(1);
        }
        Ok(answer)
    }
}

/// A line that is not a secret
fn plain(prompt: &str) -> Option<String> {
    if std::io::stdin().is_terminal() {
        eprint!("{prompt}");
        std::io::stderr().flush().ok();
    }
    let mut line = String::new();
    std::io::stdin().lock().take_bytes(1024, &mut line)?;
    let line = line.trim().to_string();
    if line.is_empty() { None } else { Some(line) }
}

trait TakeBytes {
    fn take_bytes(&mut self, most: u64, into: &mut String) -> Option<()>;
}

impl<T: BufRead> TakeBytes for T {
    fn take_bytes(&mut self, most: u64, into: &mut String) -> Option<()> {
        use std::io::Read;
        self.by_ref().take(most).read_line(into).ok()?;
        Some(())
    }
}

/// A sentence the recipe passed, with `{name}` filled in
fn said(args: &Args, name: &str, otherwise: &str, fill: &[(&str, &str)]) -> String {
    let mut text = args.option(name).unwrap_or(otherwise).to_string();
    for (key, value) in fill {
        text = text.replace(&format!("{{{key}}}"), value);
    }
    text
}

/// What a server said is data: one line, nothing that moves the cursor
fn data(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(200).collect()
}

fn problem_text(args: &Args, answer: &Value) -> String {
    let name = answer["problem"].as_str().unwrap_or("failed");
    let fallback = format!("It did not work: {name}");
    let mut text = said(args, &format!("say-problem-{name}"), &fallback, &[("n", &answer["count"].to_string())]);
    if let Some(message) = answer["message"].as_str() {
        text.push_str(&format!(" {}", data(message)));
    }
    text
}

fn marker() -> Option<std::path::PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").map(|home| std::path::Path::new(&home).join(".config")))?;
    Some(config.join("amethystora").join("vault-sync"))
}

/// What `amethystora-sends` reads to say that the vault sends to a server: the address, and nothing secret
fn write_marker(address: &str) {
    if let Some(file) = marker() {
        if let Some(folder) = file.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        let _ = std::fs::write(file, format!("{address}\n"));
    }
}

fn remove_marker() {
    if let Some(file) = marker() {
        let _ = std::fs::remove_file(file);
    }
}

pub fn run(args: &Args) -> Result<(), u8> {
    match args.word(1).ok_or(USAGE)? {
        "connect" => connect(args),
        "now" => now(args),
        "status" => status(args),
        "off" => off(),
        "choose" => choose(args),
        _ => Err(USAGE),
    }
}

fn status(args: &Args) -> Result<(), u8> {
    let answer = ask(json!({"op": "sync-status"}))?;
    if answer["ok"] != json!(true) {
        return Err(refusal_code(&answer));
    }
    if args.option("json").is_some() {
        println!("{answer}");
        return Ok(());
    }
    if answer["connected"] != json!(true) {
        remove_marker();
        println!("connected\tno");
        return Ok(());
    }
    write_marker(answer["address"].as_str().unwrap_or(""));
    println!("connected\tyes");
    println!("address\t{}", data(answer["address"].as_str().unwrap_or("")));
    println!("email\t{}", data(answer["email"].as_str().unwrap_or("")));
    println!("linked\t{}", answer["linked"]);
    println!("running\t{}", if answer["running"] == json!(true) { "yes" } else { "no" });
    if let Some(at) = answer["last_success_ms"].as_i64() {
        println!("last\t{}", at / 1000);
    }
    if let Some(problem) = answer["problem"].as_object() {
        let detail = problem.get("detail");
        let said = json!({"problem": problem.get("name"), "count": detail.and_then(|detail| detail.get("count")), "message": detail.and_then(|detail| detail.get("message"))});
        println!("problem\t{}", data(&problem_text(args, &said)));
    }
    Ok(())
}

fn now(args: &Args) -> Result<(), u8> {
    let answer = ask(json!({"op": "sync-now", "accept_gone": args.word(2) == Some("accept")}))?;
    if answer["ok"] != json!(true) && answer.get("report").is_none() {
        eprintln!("{}", problem_text(args, &answer));
        return Err(refusal_code(&answer));
    }
    let report = &answer["report"];
    for (name, label) in [("sent", "sent"), ("received", "received"), ("created_there", "created-there"), ("created_here", "created-here"), ("trashed_there", "trashed-there"), ("trashed_here", "trashed-here"), ("conflicts", "conflicts"), ("organization", "organization"), ("left_alone", "left-alone")] {
        if let Some(count) = report[name].as_u64() {
            println!("{label}\t{count}");
        }
    }
    for refused in report["refused"].as_array().into_iter().flatten() {
        println!("refused\t{}", data(refused["why"].as_str().unwrap_or("")));
    }
    if answer["ok"] != json!(true) {
        let problem = &answer["problem"];
        eprintln!("{}", problem_text(args, &json!({"problem": problem["name"], "count": problem["detail"]["count"], "message": problem["detail"]["message"]})));
        return Err(1);
    }
    Ok(())
}

fn off() -> Result<(), u8> {
    let answer = ask(json!({"op": "sync-off"}))?;
    if answer["ok"] != json!(true) {
        return Err(refusal_code(&answer));
    }
    remove_marker();
    Ok(())
}

fn choose(args: &Args) -> Result<(), u8> {
    let answer = ask(json!({"op": "sync-choose", "kind": args.word(2).ok_or(USAGE)?, "id": args.word(3).ok_or(USAGE)?, "choice": args.word(4).ok_or(USAGE)?}))?;
    if answer["ok"] != json!(true) {
        eprintln!("{}", problem_text(args, &answer));
        return Err(refusal_code(&answer));
    }
    Ok(())
}

/// Whether what the owner gave is this fingerprint, the whole of it: hex, with colons or spaces between the bytes or none
fn same_fingerprint(given: &str, shown: &str) -> bool {
    let given: String = given.chars().filter(|c| !matches!(c, ':' | ' ' | '-')).collect::<String>().to_ascii_lowercase();
    shown.len() == 64 && given.len() == 64 && given.bytes().all(|b| b.is_ascii_hexdigit()) && amethystora_vault_core::seal::same(given.as_bytes(), shown.to_ascii_lowercase().as_bytes())
}

/// The certificate to trust for this connection: the last the server showed if it signed itself (a private CA's root), else
/// the server's own
fn chosen(chain: &[Value]) -> Option<&Value> {
    chain.iter().rev().find(|cert| cert["self_signed"] == json!(true)).or_else(|| chain.first())
}

fn connect(args: &Args) -> Result<(), u8> {
    let address = args.word(2).ok_or(USAGE)?;
    eprintln!("{}", said(args, "say-sync-intro", "This sends what the vault holds that syncs to the server, encrypted, and keeps it in step. Whoever opens this vault can reach that whole account, so a dedicated account is the better choice.", &[]));
    let mut talk = Talk::open()?;
    let address = address.to_string();
    let email = plain(&said(args, "say-sync-email", "Email address of the account: ", &[])).ok_or(1u8)?;
    let method = plain(&said(args, "say-sync-method", "Log in with the personal API key (it asks no code) or the master password? Type key or password: ", &[])).ok_or(1u8)?;
    let mut request = json!({"op": "sync-connect", "address": address, "email": email});
    if method == "key" {
        let id = plain(&said(args, "say-sync-api-id", "API key client_id: ", &[])).ok_or(1u8)?;
        let secret = read_secret(&said(args, "say-sync-api-secret", "API key client_secret: ", &[])).ok_or(1u8)?;
        request["api_key"] = json!({"id": id, "secret": secret.as_str()});
    }
    let password = read_secret(&said(args, "say-sync-password", "Master password (it opens the account's key here and is not kept): ", &[])).ok_or(1u8)?;
    request["secret"] = json!(encode(password.as_bytes()));
    let mut step = 0;
    let answer = loop {
        step += 1;
        if step > 8 {
            return Err(1);
        }
        let answer = talk.ask(request.clone())?;
        if answer["ok"] == json!(true) {
            break answer;
        }
        match answer["problem"].as_str().unwrap_or("") {
            "untrusted" if request.get("ca").is_none() => {
                let seen = talk.ask(json!({"op": "sync-probe", "address": address}))?;
                let chain: Vec<Value> = seen["chain"].as_array().cloned().unwrap_or_default();
                let Some(cert) = chosen(&chain) else {
                    eprintln!("{}", problem_text(args, &answer));
                    return Err(1);
                };
                let print = |label: &str, value: &str| eprintln!("  {label} {}", data(value));
                eprintln!("{}", said(args, "say-sync-trust", "The server's certificate is not one this machine knows. To trust it for this connection only, give the fingerprint of it that the server's owner gave you:", &[]));
                print(&said(args, "say-sync-subject", "Subject:", &[]), cert["subject"].as_str().unwrap_or(""));
                print(&said(args, "say-sync-issuer", "Issuer:", &[]), cert["issuer"].as_str().unwrap_or(""));
                // Not the one the certificate showed: a fingerprint read off this screen proves that it was read, and nothing about whose it
                // is. The owner's, given whole, has to be the same (the review's L8).
                let fingerprint = cert["sha256"].as_str().unwrap_or("").to_string();
                let typed = plain(&said(args, "say-sync-trust-type", "SHA-256 fingerprint from the server's owner: ", &[])).ok_or(1u8)?;
                if !same_fingerprint(&typed, &fingerprint) {
                    eprintln!("{}", said(args, "say-sync-trust-mismatch", "That is not this certificate's fingerprint. Nothing was trusted.", &[]));
                    return Err(1);
                }
                request["ca"] = cert["pem"].clone();
                request["pin"] = json!(fingerprint);
            }
            "kdf-memory-confirm" => {
                let memory = answer["memory"].as_u64().unwrap_or(0);
                let typed = plain(&said(args, "say-sync-kdf-memory", "This account's key takes {n} MiB of memory to make, which is a lot, and it is made in a program of its own that is gone afterwards. Type yes to go on: ", &[("n", &memory.to_string())])).ok_or(1u8)?;
                if typed != "yes" {
                    return Err(1);
                }
                request["confirm_kdf"] = json!(memory);
            }
            "kdf-confirm" => {
                let rounds = answer["iterations"].as_u64().unwrap_or(0);
                let typed = plain(&said(args, "say-sync-kdf", "This account's key is made with only {n} rounds, which is far fewer than Bitwarden's default and easier to guess a password against. Type yes to go on: ", &[("n", &rounds.to_string())])).ok_or(1u8)?;
                if typed != "yes" {
                    return Err(1);
                }
                request["confirm_kdf"] = json!(rounds);
            }
            "two-factor" => {
                let providers: Vec<u64> = answer["providers"].as_array().into_iter().flatten().filter_map(Value::as_u64).collect();
                let provider = if providers.len() == 1 {
                    providers[0]
                } else {
                    match plain(&said(args, "say-sync-provider", "Which second step? Type authenticator, email or yubikey: ", &[])).as_deref() {
                        Some("authenticator") if providers.contains(&0) => 0,
                        Some("email") if providers.contains(&1) => 1,
                        Some("yubikey") if providers.contains(&3) => 3,
                        _ => return Err(1),
                    }
                };
                if provider == 1 {
                    let mut ask_code = request.clone();
                    ask_code["send_email_code"] = json!(true);
                    talk.ask(ask_code)?;
                }
                let prompt = match provider {
                    0 => said(args, "say-sync-code-app", "Code from your authenticator app: ", &[]),
                    1 => said(args, "say-sync-code-email", "Code sent to your email: ", &[]),
                    _ => said(args, "say-sync-code-yubikey", "Touch your YubiKey: ", &[]),
                };
                let code = read_secret(&prompt).ok_or(1u8)?;
                request["two_factor"] = json!({"provider": provider, "code": code.as_str(), "remember": false});
            }
            "new-device" => loop {
                let typed = read_secret(&said(args, "say-sync-device", "This server checks new devices, and sent a code to the account's email. Code, or type resend: ", &[])).ok_or(1u8)?;
                if typed.as_str() == "resend" {
                    let mut resend = request.clone();
                    resend["resend_device_code"] = json!(true);
                    talk.ask(resend)?;
                    continue;
                }
                request["new_device"] = json!(typed.as_str());
                break;
            },
            _ => {
                eprintln!("{}", problem_text(args, &answer));
                return Err(refusal_code(&answer));
            }
        }
    };
    write_marker(&address);
    eprintln!("{}", said(args, "say-sync-done", "Connected. While the vault is open, it and the account keep each other up to date.", &[]));
    println!("connected\t{}", data(answer["address"].as_str().unwrap_or("")));
    Ok(())
}
