//! The item commands of `ame vault`: passwords, backup codes, authenticator accounts, keys and certificates. A secret never comes
//! as an argument: it is read from the terminal or standard input. Foreign files are parsed here, as the person, never in the service.
//! Each prints values and tab-separated lines; the words are `vault.just`'s.

use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use base64::Engine;
use serde_json::{json, Value};
use zeroize::Zeroizing;

use amethystora_vault_core::codes;
use amethystora_vault_core::import::{self, ImportError, Source};
use amethystora_vault_core::items::{new_id, Password, Secret};
use amethystora_vault_core::keys::{self, KeyError};
use amethystora_vault_core::protocol::encode;

use super::{call, new_password, read_secret, Args, USAGE};

/// `import-merge` takes this many at once
const AT_ONCE: usize = 256;
/// Where 13-apps.sh puts the module, and what NSS loads
const MODULE: &str = "/usr/lib64/amethystora-vault/libamethystora_vault_pkcs11.so";

fn lines(items: &Value, columns: impl Fn(&Value) -> Vec<String>) {
    for item in items.as_array().into_iter().flatten() {
        println!("{}", columns(item).join("\t"));
    }
}

fn word(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// A name a person typed as an item: its id, its exact name, or the only one whose name holds it
fn find(kind: &str, typed: &str) -> Result<String, u8> {
    let listed = call(json!({"op": "list", "kind": kind}))?;
    let items = listed["items"].as_array().cloned().unwrap_or_default();
    if let Some(item) = items.iter().find(|item| item["id"] == json!(typed) || item["name"] == json!(typed)) {
        return Ok(word(&item["id"]));
    }
    let lower = typed.to_lowercase();
    let mut close = items.iter().filter(|item| item["name"].as_str().is_some_and(|name| name.to_lowercase().contains(&lower)));
    match (close.next(), close.next()) {
        (Some(one), None) => Ok(word(&one["id"])),
        _ => Err(1),
    }
}

pub(crate) fn input_of(path: Option<&str>) -> Result<Zeroizing<Vec<u8>>, u8> {
    let mut bytes = Zeroizing::new(Vec::new());
    match path {
        Some(path) => {
            let file = std::fs::File::open(path).map_err(|_| 1u8)?;
            file.take(32 * 1024 * 1024).read_to_end(&mut bytes).map_err(|_| 1u8)?;
        }
        None => {
            std::io::stdin().lock().take(32 * 1024 * 1024).read_to_end(&mut bytes).map_err(|_| 1u8)?;
        }
    }
    Ok(bytes)
}

// --- Passwords ------------------------------------------------------------------------------------------------------------

fn generated(args: &Args, at: usize) -> Result<Value, u8> {
    let request = match args.word(at) {
        None => json!({"op": "password-generate"}),
        Some("words") => json!({"op": "password-generate", "style": "words"}),
        Some(length) => json!({"op": "password-generate", "length": length.parse::<u64>().map_err(|_| USAGE)?}),
    };
    call(request)
}

fn secret_text(value: &Value) -> Result<Zeroizing<String>, u8> {
    let bytes = Zeroizing::new(base64::engine::general_purpose::STANDARD.decode(value.as_str().ok_or(6u8)?).map_err(|_| 6u8)?);
    Ok(Zeroizing::new(String::from_utf8_lossy(&bytes).into_owned()))
}

fn store_password(args: &Args, password: &str) -> Result<(), u8> {
    let name = args.word(2).ok_or(USAGE)?;
    let (username, address) = (args.word(3).unwrap_or(""), args.word(4).unwrap_or(""));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_millis() as i64);
    let item = Password {
        id: new_id().map_err(|_| 6u8)?,
        title: name.to_string(),
        username: username.to_string(),
        address: address.to_string(),
        password: Secret::new(password),
        added_at: now,
        ..Password::default()
    };
    let value = serde_json::to_value(&item).map_err(|_| 6u8)?;
    call(json!({"op": "put", "kind": "password", "name": name, "meta": {"username": username, "address": address}, "value": value})).map(|_| ())
}

pub fn password(args: &Args) -> Result<(), u8> {
    match args.word(1).ok_or(USAGE)? {
        "generate" => println!("{}", secret_text(&generated(args, 2)?["secret"])?.as_str()),
        "add" => {
            let typed = read_secret(&args.say("say-this-password", "Password: ")).ok_or(1u8)?;
            store_password(args, &typed)?;
        }
        "new" => store_password(args, &secret_text(&call(json!({"op": "password-generate"}))?["secret"])?)?,
        "list" => {
            let answer = call(json!({"op": "list", "kind": "password"}))?;
            lines(&answer["items"], |item| vec![word(&item["id"]), word(&item["name"]), word(&item["meta"]["username"]), word(&item["meta"]["address"])]);
        }
        "get" => {
            let id = find("password", args.word(2).ok_or(USAGE)?)?;
            println!("{}", secret_text(&call(json!({"op": "password-copy", "id": id}))?["secret"])?.as_str());
        }
        "remove" => {
            let id = find("password", args.word(2).ok_or(USAGE)?)?;
            call(json!({"op": "remove", "kind": "password", "id": id}))?;
        }
        _ => return Err(USAGE),
    }
    Ok(())
}

// --- Backup codes ---------------------------------------------------------------------------------------------------------

/// A PDF's text through `pdftotext`, its path an argument of its own and its text on a pipe, kept to what looks like a code
fn codes_in_pdf(path: &str) -> Result<codes::ParsedCodes, u8> {
    let out = Command::new("pdftotext").arg(absolute(path)?).arg("-").stderr(Stdio::null()).output().map_err(|_| 6u8)?;
    if !out.status.success() {
        return Err(1);
    }
    let text = Zeroizing::new(String::from_utf8_lossy(&out.stdout).into_owned());
    Ok(codes::parse_document_codes(&text))
}

pub fn codes_command(args: &Args) -> Result<(), u8> {
    match args.word(1).ok_or(USAGE)? {
        "add" => {
            let service = args.word(2).ok_or(USAGE)?;
            let file = args.word(3);
            let parsed = match file {
                Some(path) if path.to_lowercase().ends_with(".pdf") => codes_in_pdf(path)?,
                _ => {
                    let bytes = input_of(file)?;
                    codes::parse_codes(&String::from_utf8_lossy(&bytes))
                }
            };
            // A text past the limit is not a list of codes: none was read (the codes past the limits are in `parsed.dropped`)
            if parsed.text_too_large {
                return Err(1);
            }
            let dropped = parsed.dropped;
            let parsed = parsed.codes;
            if parsed.is_empty() {
                return Err(1);
            }
            // What was found, for a person to confirm before it is kept: standard input is free only when a file was named
            if file.is_some() && std::io::stdin().is_terminal() {
                eprint!("{}", args.say("say-codes-found", "Codes found, keep them? y/n ").replace("{count}", &parsed.len().to_string()));
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer).map_err(|_| 1u8)?;
                if !answer.trim().eq_ignore_ascii_case("y") {
                    return Err(1);
                }
            }
            let answer = call(json!({"op": "codes-add", "service": service, "codes": parsed}))?;
            println!("{}", word(&answer["added"]));
            // What was not kept is said, never lost quietly: the codes the reading dropped (too long, or past what a text may hold), the
            // ones the service held already, and the ones past what an item holds
            let left_out = [dropped as u64, answer["repeats"].as_u64().unwrap_or(0), answer["over_limit"].as_u64().unwrap_or(0)];
            if left_out.iter().any(|count| *count > 0) {
                eprintln!("{}", args.say("say-codes-left-out", "Not kept: {dropped} too long or past what one text may hold, {repeats} already kept, {over_limit} past what one item holds.").replace("{dropped}", &left_out[0].to_string()).replace("{repeats}", &left_out[1].to_string()).replace("{over_limit}", &left_out[2].to_string()));
            }
        }
        "list" => {
            let answer = call(json!({"op": "codes-list"}))?;
            lines(&answer["items"], |item| vec![word(&item["id"]), word(&item["meta"]["service"]), word(&item["meta"]["left"]), word(&item["meta"]["total"])]);
        }
        "next" => {
            let id = find("backup-codes", args.word(2).ok_or(USAGE)?)?;
            let answer = call(json!({"op": "codes-next", "id": id}))?;
            if answer["code"].is_null() {
                return Err(1);
            }
            println!("{}\t{}\t{}", answer["index"].as_u64().unwrap_or(0) + 1, word(&answer["code"]), word(&answer["left"]));
        }
        "used" => {
            let id = find("backup-codes", args.word(2).ok_or(USAGE)?)?;
            let number: u64 = args.word(3).and_then(|number| number.parse().ok()).filter(|number| *number > 0).ok_or(USAGE)?;
            let used = args.option("undo").is_none();
            let answer = call(json!({"op": "codes-used", "id": id, "index": number - 1, "used": used}))?;
            println!("{}", word(&answer["left"]));
        }
        "remove" => {
            let id = find("backup-codes", args.word(2).ok_or(USAGE)?)?;
            call(json!({"op": "remove", "kind": "backup-codes", "id": id}))?;
        }
        _ => return Err(USAGE),
    }
    Ok(())
}

// --- Authenticator --------------------------------------------------------------------------------------------------------

/// A file of accounts, read here as the person: what the service gets is only finished items, in as many requests as it takes
fn import_file(args: &Args, path: &str) -> Result<(), u8> {
    let bytes = input_of(Some(path))?;
    let read = match import::read_file(&bytes, "") {
        Err(ImportError::PasswordRequired) => {
            let typed = read_secret(&args.say("say-file-password", "Password of the file: ")).ok_or(1u8)?;
            import::read_file(&bytes, &typed)
        }
        other => other,
    };
    let imported = read.map_err(|why| match why {
        ImportError::TooLarge => 4,
        _ => 1,
    })?;
    let source = if imported.source == Source::BackMyCodes { "backmycodes" } else { "links" };
    let accounts: Vec<Value> = imported.accounts.iter().map(serde_json::to_value).collect::<Result<_, _>>().map_err(|_| 6u8)?;
    let services: Vec<Value> = imported.services.iter().map(serde_json::to_value).collect::<Result<_, _>>().map_err(|_| 6u8)?;
    let (mut added, mut same) = (0, 0);
    let mut sent = |accounts: &[Value], services: &[Value]| -> Result<(), u8> {
        let answer = call(json!({"op": "import-merge", "source": source, "accounts": accounts, "services": services}))?;
        added += answer["added"].as_u64().unwrap_or(0) + answer["added_services"].as_u64().unwrap_or(0);
        same += answer["same"].as_u64().unwrap_or(0) + answer["same_services"].as_u64().unwrap_or(0);
        Ok(())
    };
    for batch in accounts.chunks(AT_ONCE) {
        sent(batch, &[])?;
    }
    for batch in services.chunks(AT_ONCE) {
        sent(&[], batch)?;
    }
    println!("{added}\t{same}\t{}", imported.skipped);
    Ok(())
}

/// The QR on the terminal, the link on qrencode's standard input and never on its command line
fn show_qr(link: &str) -> Result<(), u8> {
    let mut child = Command::new("qrencode").args(["-t", "ANSIUTF8"]).stdin(Stdio::piped()).spawn().map_err(|_| 6u8)?;
    child.stdin.take().ok_or(6u8)?.write_all(link.as_bytes()).map_err(|_| 6u8)?;
    child.wait().map_err(|_| 6u8)?;
    Ok(())
}

pub fn auth(args: &Args) -> Result<(), u8> {
    match args.word(1).ok_or(USAGE)? {
        "add" => match args.word(2) {
            Some(path) => import_file(args, path)?,
            None => {
                let link = read_secret(&args.say("say-link", "The otpauth:// link: ")).ok_or(1u8)?;
                println!("{}", word(&call(json!({"op": "auth-add", "link": link.as_str()}))?["id"]));
            }
        },
        "list" => {
            let answer = call(json!({"op": "auth-list"}))?;
            lines(&answer["items"], |item| vec![word(&item["id"]), word(&item["name"]), word(&item["meta"]["type"])]);
        }
        "code" => {
            let id = find("authenticator", args.word(2).ok_or(USAGE)?)?;
            let answer = call(json!({"op": "auth-code", "id": id, "step": args.option("step").is_some()}))?;
            println!("{}\t{}", word(&answer["code"]), word(&answer["seconds_left"]));
        }
        "qr" => {
            let id = find("authenticator", args.word(2).ok_or(USAGE)?)?;
            let answer = call(json!({"op": "auth-link", "id": id}))?;
            show_qr(answer["link"].as_str().ok_or(6u8)?)?;
            println!("{}", word(&answer["key"]));
        }
        "remove" => {
            let id = find("authenticator", args.word(2).ok_or(USAGE)?)?;
            call(json!({"op": "remove", "kind": "authenticator", "id": id}))?;
        }
        _ => return Err(USAGE),
    }
    Ok(())
}

// --- Keys -----------------------------------------------------------------------------------------------------------------

fn print_public(public: &Value) {
    if let Some(line) = public["openssh"].as_str() {
        println!("{line}");
    } else if let Some(pem) = public["pem"].as_str() {
        print!("{pem}");
    }
}

fn phrase_for_export(args: &Args) -> Result<Value, u8> {
    if args.option("plain").is_some() {
        return Ok(json!({"plain": true}));
    }
    Ok(json!({"passphrase": encode(new_password(args)?.as_bytes())}))
}

/// What goes out goes to `--out=FILE`, created for this account alone and never over another, or to a pipe. Never to a terminal.
fn write_out(args: &Args, bytes: &[u8]) -> Result<(), u8> {
    use std::os::unix::fs::OpenOptionsExt;
    match args.option("out") {
        Some(path) => {
            let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(|_| 1u8)?;
            file.write_all(bytes).map_err(|_| 6u8)
        }
        None if std::io::stdout().is_terminal() => Err(USAGE),
        None => std::io::stdout().write_all(bytes).map_err(|_| 6u8),
    }
}

fn decoded(value: &Value) -> Result<Zeroizing<Vec<u8>>, u8> {
    base64::engine::general_purpose::STANDARD.decode(value.as_str().ok_or(6u8)?).map(Zeroizing::new).map_err(|_| 6u8)
}

pub fn key(args: &Args) -> Result<(), u8> {
    match args.word(1).ok_or(USAGE)? {
        "new" => {
            let name = args.word(2).ok_or(USAGE)?;
            let mut request = json!({"op": "key-new", "name": name, "key_kind": args.option("kind").unwrap_or("ssh")});
            if let Some(key_type) = args.word(3) {
                request["type"] = json!(key_type);
            }
            if args.option("not-exportable").is_some() {
                request["exportable"] = json!(false);
            }
            print_public(&call(request)?["public"]);
        }
        "import" => {
            // A name, or the file's own
            let (name, path) = match (args.word(2), args.word(3)) {
                (Some(name), Some(path)) => (name.to_string(), path),
                (Some(path), None) => (std::path::Path::new(path).file_name().and_then(|name| name.to_str()).ok_or(USAGE)?.to_string(), path),
                _ => return Err(USAGE),
            };
            // A file that is not there is vault.just's 11, "Nothing is there"
            if std::fs::metadata(path).is_err_and(|why| why.kind() == std::io::ErrorKind::NotFound) {
                return Err(11);
            }
            let file = input_of(Some(path))?;
            let imported = match keys::import_openssh(&file, b"") {
                Err(KeyError::WrongPassphrase) => {
                    let typed = read_secret(&args.say("say-key-passphrase", "Passphrase of the key: ")).ok_or(1u8)?;
                    keys::import_openssh(&file, typed.as_bytes())
                }
                other => other,
            }
            .map_err(|_| 1u8)?;
            let answer = call(json!({"op": "key-import", "name": name, "type": imported.key.key_type().name(), "private": encode(imported.key.as_bytes()), "comment": imported.comment}))?;
            print_public(&answer["public"]);
        }
        "list" => {
            let answer = call(json!({"op": "list", "kind": "key"}))?;
            lines(&answer["items"], |item| vec![word(&item["id"]), word(&item["name"]), word(&item["meta"]["key_kind"]), word(&item["meta"]["type"]), word(&item["meta"]["sha256"])]);
        }
        "public" => {
            let id = find("key", args.word(2).ok_or(USAGE)?)?;
            print_public(&call(json!({"op": "key-public", "id": id}))?["public"]);
        }
        "export" => {
            let id = find("key", args.word(2).ok_or(USAGE)?)?;
            let mut request = phrase_for_export(args)?;
            request["op"] = json!("key-export");
            request["id"] = json!(id);
            write_out(args, &decoded(&call(request)?["data"])?)?;
        }
        "remove" => {
            let id = find("key", args.word(2).ok_or(USAGE)?)?;
            call(json!({"op": "remove", "kind": "key", "id": id}))?;
        }
        _ => return Err(USAGE),
    }
    Ok(())
}

// --- Certificates ---------------------------------------------------------------------------------------------------------

fn profile_of(kind: &str) -> Result<&'static str, u8> {
    match kind {
        "pdf" => Ok("pdf"),
        "code" => Ok("code"),
        "mail" => Ok("smime"),
        "tls" => Ok("local-tls"),
        "device" => Ok("device-identity"),
        "login" => Ok("service-login"),
        _ => Err(USAGE),
    }
}

pub fn cert(args: &Args) -> Result<(), u8> {
    match args.word(1).ok_or(USAGE)? {
        "new" => {
            let (title, kind) = (args.word(2).ok_or(USAGE)?, args.word(3).ok_or(USAGE)?);
            let mut request = json!({"op": "cert-new", "title": title, "profile": profile_of(kind)?});
            for (option, field) in [("name", "name"), ("email", "email"), ("web", "web"), ("org", "organization"), ("country", "country"), ("key", "key_type"), ("digest", "digest")] {
                if let Some(value) = args.option(option) {
                    request[field] = json!(value);
                }
            }
            let hosts: Vec<&String> = args.words.iter().skip(4).collect();
            if !hosts.is_empty() {
                request["hosts"] = json!(hosts);
            }
            let answer = call(request)?;
            println!("{}\t{}", word(&answer["id"]), word(&answer["description"]["subject"]));
        }
        "list" => {
            let answer = call(json!({"op": "cert-list"}))?;
            lines(&answer["items"], |item| vec![word(&item["id"]), word(&item["name"]), word(&item["meta"]["profile"]), word(&item["meta"]["subject"]), word(&item["meta"]["not_after"])]);
        }
        "show" => {
            let id = find("certificate", args.word(2).ok_or(USAGE)?)?;
            println!("{}", call(json!({"op": "cert-show", "id": id}))?);
        }
        "chain" => {
            let id = find("certificate", args.word(2).ok_or(USAGE)?)?;
            print!("{}", word(&call(json!({"op": "cert-chain", "id": id}))?["pem"]));
        }
        "export" => {
            let id = find("certificate", args.word(2).ok_or(USAGE)?)?;
            if args.word(3) == Some("pem") {
                let mut request = phrase_for_export(args)?;
                request["op"] = json!("cert-key");
                request["id"] = json!(id);
                let answer = call(request)?;
                let mut out = decoded(&answer["key"])?;
                out.extend_from_slice(word(&answer["pem"]).as_bytes());
                write_out(args, &out)?;
            } else {
                let password = new_password(args)?;
                let answer = call(json!({"op": "cert-pfx", "id": id, "password": encode(password.as_bytes())}))?;
                write_out(args, &decoded(&answer["data"])?)?;
            }
        }
        "renew" => {
            let id = find("certificate", args.word(2).ok_or(USAGE)?)?;
            let answer = call(json!({"op": "cert-renew", "id": id}))?;
            println!("{}\t{}", word(&answer["id"]), word(&answer["description"]["subject"]));
        }
        "ca" => print!("{}", word(&call(json!({"op": "ca-cert", "which": args.word(2).unwrap_or("account")}))?["pem"])),
        "remove" => {
            let id = find("certificate", args.word(2).ok_or(USAGE)?)?;
            call(json!({"op": "remove", "kind": "certificate", "id": id}))?;
        }
        _ => return Err(USAGE),
    }
    Ok(())
}

pub fn root(args: &Args) -> Result<(), u8> {
    match args.word(1) {
        None => {
            let machine = call(json!({"op": "machine"}))?;
            if machine["root"] != json!(true) {
                return Err(1);
            }
            eprintln!("{}\t{}", word(&machine["number"]), word(&machine["sha256"]));
            print!("{}", word(&call(json!({"op": "ca-cert", "which": "root"}))?["pem"]));
        }
        Some("rotate") => println!("{}", word(&call(json!({"op": "root-rotate"}))?["number"])),
        Some(_) => return Err(USAGE),
    }
    Ok(())
}

// --- PDF ------------------------------------------------------------------------------------------------------------------

/// A folder of this account's in the runtime directory, gone when this ends: the database NSS runs on holds only the module
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Scratch, u8> {
        use std::os::unix::fs::DirBuilderExt;
        let base = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, PathBuf::from);
        let folder = base.join(format!("amethystora-vault-pdf-{}", amethystora_vault_core::seal::random_hex(8)));
        std::fs::DirBuilder::new().mode(0o700).create(&folder).map_err(|_| 6u8)?;
        Ok(Scratch(folder))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn quiet(command: &mut Command) -> Result<(), u8> {
    let done = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map_err(|_| 6u8)?;
    if done.success() { Ok(()) } else { Err(6) }
}

/// Beside the original, named for what it is, and never over anything
fn signed_path(file: &Path) -> PathBuf {
    let stem = file.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    let mut path = file.with_file_name(format!("{stem} (signed).pdf"));
    let mut number = 2;
    while path.exists() {
        path = file.with_file_name(format!("{stem} (signed {number}).pdf"));
        number += 1;
    }
    path
}

/// A file's whole path, which starts with `/`: poppler's tools take no `--`, and a name starting with `-` would read as an option
fn absolute(path: &str) -> Result<PathBuf, u8> {
    std::fs::canonicalize(path).map_err(|_| 1u8)
}

/// The PDF is signed by `pdfsig` as the person, through the module, with the key staying in the service: a digest goes there and a
/// signature comes back, the certificate's approval between. `-nssdir` always, since without it pdfsig reads Firefox's profile.
pub fn pdf(args: &Args) -> Result<(), u8> {
    if args.word(1) != Some("sign") {
        return Err(USAGE);
    }
    let input = args.word(2).ok_or(USAGE)?;
    let listed = call(json!({"op": "cert-list", "purpose": "pdf"}))?;
    let offered: Vec<&Value> = listed["items"].as_array().into_iter().flatten().filter(|item| item["meta"]["renewed_by"].is_null()).collect();
    let id = match (args.word(3), offered.as_slice()) {
        (Some(cert), _) => find("certificate", cert)?,
        (None, [one]) => word(&one["id"]),
        (None, _) => return Err(1),
    };
    let scratch = Scratch::new()?;
    let database = format!("sql:{}", scratch.0.display());
    let module = args.option("module").map_or_else(|| MODULE.to_string(), str::to_string);
    quiet(Command::new("certutil").args(["-N", "--empty-password", "-d", &database]))?;
    quiet(Command::new("modutil").args(["-force", "-dbdir", &database, "-add", "amethystora-vault", "-libfile", &module]))?;
    let input = absolute(input)?;
    let output = signed_path(&input);
    let nick = format!("pkcs11:token=Amethystora%20Vault;object={id};type=cert");
    let done = Command::new("pdfsig")
        .args(["-nssdir", &database, "-add-signature", "-nick", &nick, "-new-signature-field-name", "AmethystoraVault"])
        .arg(&input)
        .arg(&output)
        .stdin(Stdio::null())
        .status()
        .map_err(|_| 6u8)?;
    if !done.success() {
        let _ = std::fs::remove_file(&output);
        return Err(1);
    }
    println!("{}", output.display());
    // The root this machine signs under, trusted for signatures in this account's readers, if it is not already: the copy is made either way
    let _ = trust_root_for_signatures();
    Ok(())
}

// --- What this machine's readers and browsers trust, in the account's own NSS database ---------------------------------------

/// `certutil`, or the same inside the Papers Flatpak, whose NSS is older than the image's and reads trust the image's NSS wrote as
/// nothing ("untrusted issuer"): what the sandbox's certutil writes, both read as trusted. Papers is given the two folders it needs for
/// this run only, not for good.
fn certutil_in(papers: Option<&[&Path]>) -> Command {
    match papers {
        None => Command::new("certutil"),
        Some(folders) => {
            let mut command = Command::new("flatpak");
            command.args(["run", "--command=certutil"]);
            for folder in folders {
                command.arg(format!("--filesystem={}", folder.display()));
            }
            command.arg("org.gnome.Papers");
            command
        }
    }
}

fn nss_folder() -> Result<PathBuf, u8> {
    Ok(PathBuf::from(std::env::var_os("HOME").filter(|home| !home.is_empty()).ok_or(6u8)?).join(".pki").join("nssdb"))
}

/// The account's database, made empty if there is none: `sql:` is the format Chromium and `pdfsig` read
fn nss_database() -> Result<(PathBuf, String), u8> {
    use std::os::unix::fs::DirBuilderExt;
    let folder = nss_folder()?;
    let database = format!("sql:{}", folder.display());
    if !folder.join("cert9.db").exists() {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&folder).map_err(|_| 6u8)?;
        quiet(Command::new("certutil").args(["-N", "--empty-password", "-d", &database]))?;
    }
    Ok((folder, database))
}

/// A certificate in the database under this nickname with these trust flags, in place of one of that nickname that was there
fn trust_certificate(mut certutil: impl FnMut() -> Command, database: &str, nickname: &str, pem: &Path, flags: &str) -> Result<(), u8> {
    let _ = quiet(certutil().args(["-D", "-d", database, "-n", nickname]));
    quiet(certutil().args(["-A", "-d", database, "-n", nickname, "-t", flags, "-a", "-i"]).arg(pem))
}

/// The nicknames in the database that start with this
fn nicknames(database: &str, start: &str) -> Vec<String> {
    let listed = Command::new("certutil").args(["-L", "-d", database]).stdin(Stdio::null()).stderr(Stdio::null()).output().map(|out| String::from_utf8_lossy(&out.stdout).into_owned()).unwrap_or_default();
    listed.lines().filter(|line| line.starts_with(start)).filter_map(|line| line.trim_end().rsplit_once("  ").map(|(name, _)| name.trim_end().to_string())).collect()
}

fn certificate_file(scratch: &Scratch, pem: &str) -> Result<PathBuf, u8> {
    let file = scratch.0.join("trust.pem");
    std::fs::write(&file, pem).map_err(|_| 6u8)?;
    Ok(file)
}

/// This machine's root, trusted for signatures only (`,C,`: email and documents, never a website), in the account's database and in
/// Papers' view of it, so that what the Vault signs shows as trusted. Only the account's own files change, and nothing is touched
/// that already holds it. Papers is optional and best effort: the window says when it still shows untrusted.
fn trust_root_for_signatures() -> Result<(), u8> {
    let answer = call(json!({"op": "ca-cert", "which": "root"}))?;
    let nickname = format!("Amethystora OS Root CA ({})", word(&answer["sha256"]).chars().take(8).collect::<String>());
    let (folder, database) = nss_database()?;
    if quiet(Command::new("certutil").args(["-L", "-d", &database, "-n", &nickname])).is_ok() {
        return Ok(());
    }
    let scratch = Scratch::new()?;
    let pem = certificate_file(&scratch, &word(&answer["pem"]))?;
    trust_certificate(|| Command::new("certutil"), &database, &nickname, &pem, ",C,")?;
    if Command::new("flatpak").args(["info", "org.gnome.Papers"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|status| status.success()) {
        let folders = [folder.as_path(), scratch.0.as_path()];
        let _ = trust_certificate(|| certutil_in(Some(&folders)), &database, &nickname, &pem, ",C,");
    }
    Ok(())
}

/// `ame vault tls trust browsers on|off`: the `Local Services CA`, which a local TLS certificate chains to, trusted for websites
/// (`C,,`) by the browsers that read the account's database. Never the root, and never `/etc/pki/ca-trust`. Firefox keeps its own
/// store and is not touched.
pub fn tls(args: &Args) -> Result<(), u8> {
    if args.word(1) != Some("trust") || args.word(2) != Some("browsers") {
        return Err(USAGE);
    }
    let (_, database) = nss_database()?;
    let start = "Amethystora Local Services CA";
    match args.word(3) {
        Some("on") => {
            let answer = call(json!({"op": "ca-cert", "which": "local"}))?;
            let scratch = Scratch::new()?;
            let pem = certificate_file(&scratch, &word(&answer["pem"]))?;
            for old in nicknames(&database, start) {
                let _ = quiet(Command::new("certutil").args(["-D", "-d", &database, "-n", &old]));
            }
            let nickname = format!("{start} ({})", word(&answer["sha256"]).chars().take(8).collect::<String>());
            trust_certificate(|| Command::new("certutil"), &database, &nickname, &pem, "C,,")
        }
        Some("off") => {
            for old in nicknames(&database, start) {
                let _ = quiet(Command::new("certutil").args(["-D", "-d", &database, "-n", &old]));
            }
            Ok(())
        }
        _ => Err(USAGE),
    }
}
