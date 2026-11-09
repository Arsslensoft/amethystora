//! docker-credential-amethystora: Docker's credential helper, named as `credsStore` looks for it. `get` answers from the vault, `store`
//! keeps what `docker login` was given, and `erase`, which `docker logout` calls, removes it after the person says so, as removing
//! always asks. `list` names only what `docker login` stored: Docker asks for everything it lists before a build, and a person
//! should not be asked for every password they have.

use std::io::Write;
use std::process::ExitCode;

use serde_json::{json, Value};

use amethystora_vault_client::Refusal;
use amethystora_vault_service::credentials::{ask, docker_server, find_login, logins_for, read_input, read_login, say_closed};

/// These exact words are how Docker knows there is nothing, rather than that something went wrong
const NOT_FOUND: &str = "credentials not found in native keychain";
/// What `docker login` and `docker logout` show when the vault did not do it: those words would say the wrong thing there
const NOT_KEPT: &str = "login not kept in the vault: it was refused or the vault is closed, or a login of your own for this registry is there already";
const NOT_ERASED: &str = "login not removed from the vault: it was refused or the vault is closed";

fn found(server: &str) -> Result<Option<String>, Refusal> {
    let Some((scheme, host)) = docker_server(server) else { return Ok(None) };
    find_login(&scheme, &host, None)
}

fn get() -> Result<Option<String>, Refusal> {
    let input = read_input().ok_or(Refusal::BadRequest)?;
    let server = input.trim();
    let Some(id) = found(server)? else { return Ok(None) };
    let login = read_login(&id)?;
    Ok(Some(json!({"ServerURL": server, "Username": login.username, "Secret": login.password.as_str()}).to_string()))
}

fn store() -> Result<(), Refusal> {
    let input = read_input().ok_or(Refusal::BadRequest)?;
    let given: Value = serde_json::from_str(&input).map_err(|_| Refusal::BadRequest)?;
    let (Some(server), Some(username), Some(secret)) = (given["ServerURL"].as_str(), given["Username"].as_str(), given["Secret"].as_str()) else { return Err(Refusal::BadRequest) };
    // A login the person made is theirs: this one is not put over it, and it is not asked about
    let (scheme, host) = docker_server(server).ok_or(Refusal::BadRequest)?;
    let fitting = logins_for(&scheme, &host, None)?;
    if fitting.iter().any(|(_, docker)| !docker) || fitting.len() > 1 {
        return Err(Refusal::Refused);
    }
    let id = fitting.into_iter().next().map(|(id, _)| id);
    let meta = json!({"username": username, "address": server, "docker": true});
    let value = json!({"title": server, "username": username, "address": server, "password": secret});
    let mut request = json!({"op": "put", "kind": "password", "name": server, "meta": meta, "value": value});
    if let Some(id) = id {
        request["id"] = json!(id);
    }
    ask(request).map(|_| ())
}

fn erase() -> Result<(), Refusal> {
    let input = read_input().ok_or(Refusal::BadRequest)?;
    // Only what Docker stored: a login the person made is not Docker's to remove
    let Some((scheme, host)) = docker_server(input.trim()) else { return Ok(()) };
    match logins_for(&scheme, &host, None)?.as_slice() {
        [(id, true)] => ask(json!({"op": "remove", "kind": "password", "id": id})).map(|_| ()),
        _ => Ok(()),
    }
}

fn list() -> Result<String, Refusal> {
    let listed = ask(json!({"op": "list", "kind": "password"}))?;
    let mut servers = serde_json::Map::new();
    for item in listed["items"].as_array().into_iter().flatten().filter(|item| item["meta"]["docker"] == json!(true)) {
        if let Some(server) = item["meta"]["address"].as_str() {
            servers.insert(server.to_string(), item["meta"]["username"].clone());
        }
    }
    Ok(Value::Object(servers).to_string())
}

fn main() -> ExitCode {
    let action = std::env::args().nth(1).unwrap_or_default();
    let result = match action.as_str() {
        "get" => get().map(|answer| Some(answer.unwrap_or_else(|| NOT_FOUND.to_string()))),
        "store" => store().map(|()| None),
        "erase" => erase().map(|()| None),
        "list" => list().map(Some),
        _ => {
            eprintln!("usage: docker-credential-amethystora get|store|erase|list");
            return ExitCode::from(64);
        }
    };
    match result {
        Ok(Some(text)) => {
            let _ = writeln!(std::io::stdout(), "{text}");
            if text == NOT_FOUND { ExitCode::FAILURE } else { ExitCode::SUCCESS }
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(why) => {
            // As far as Docker goes, a vault that cannot be asked holds nothing, and it says so on its terminal
            if why == Refusal::Closed {
                say_closed();
            }
            let said = match action.as_str() {
                "store" => NOT_KEPT,
                "erase" => NOT_ERASED,
                _ => NOT_FOUND,
            };
            let _ = writeln!(std::io::stdout(), "{said}");
            ExitCode::FAILURE
        }
    }
}
