//! What `git-credential-amethystora` and `docker-credential-amethystora` share: finding a login in the vault by the address its list
//! says, and asking for it. Neither ever writes one to disk: the secret goes to the tool on standard output and is gone. The person
//! is asked as the item says, like for any other program.

use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};
use zeroize::Zeroizing;

use amethystora_vault_client::{exchange, Refusal, SOCKET};

/// A person has five minutes to answer, and the service answers for them after that
const PATIENCE: Duration = Duration::from_secs(6 * 60);
/// More than any request a tool has a reason to make
const MOST_INPUT: u64 = 64 * 1024;

pub fn socket() -> PathBuf {
    std::env::var_os("AMETHYSTORA_VAULT_SOCKET").map_or_else(|| PathBuf::from(SOCKET), PathBuf::from)
}

/// One request: its answer if it was granted
pub fn ask(mut request: Value) -> Result<Value, Refusal> {
    request["v"] = json!(1);
    let answer = exchange(&socket(), &request, PATIENCE)?;
    if answer["ok"] == json!(true) {
        return Ok(answer);
    }
    Err(Refusal::from_name(answer["why"].as_str().unwrap_or("failed")))
}

/// What a tool says on standard input, bounded
pub fn read_input() -> Option<Zeroizing<String>> {
    let mut input = Zeroizing::new(String::new());
    std::io::stdin().lock().take(MOST_INPUT).read_to_string(&mut input).ok()?;
    Some(input)
}

/// The sentence for a tool's terminal, in the person's language where `po/` has it. `vault.just` has this message too.
pub fn say_closed() {
    let message = "The vault is closed. Open it with: ame vault open";
    let translated = Command::new("gettext").args(["-d", "amethystora", "-s", message]).output().ok().filter(|done| done.status.success());
    eprintln!("{}", translated.map_or_else(|| message.to_string(), |done| String::from_utf8_lossy(&done.stdout).trim_end().to_string()));
}

/// The scheme if it says one, and the host (and port) in lowercase, of an address as a person typed it or a tool says it
pub fn parse_address(address: &str) -> Option<(Option<String>, String)> {
    let address = address.trim();
    let (scheme, rest) = match address.split_once("://") {
        Some((scheme, rest)) => (Some(scheme.to_ascii_lowercase()), rest),
        None => (None, address),
    };
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit('@').next()?.to_ascii_lowercase();
    let shaped = !host.is_empty() && host.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':' | b'[' | b']' | b'_'));
    shaped.then_some((scheme, host))
}

/// Whether a login kept for this address is for this tool's request. An address that names no scheme is https's: a redirect to
/// plain http gets nothing.
pub fn address_fits(address: &str, protocol: &str, host: &str) -> bool {
    let Some((scheme, kept)) = parse_address(address) else { return false };
    let Some(wanted) = request_host(host) else { return false };
    kept == wanted && scheme.map_or(protocol == "https", |scheme| scheme == protocol)
}

/// The host a tool asks for, which is a host and nothing else: no login in front of it, no path after it, nothing a line could end in
fn request_host(host: &str) -> Option<String> {
    let shaped = !host.is_empty() && host.len() <= 255 && host.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':' | b'[' | b']' | b'_'));
    shaped.then(|| host.to_ascii_lowercase())
}

pub struct Login {
    pub id: String,
    pub username: String,
    pub password: Zeroizing<String>,
}

/// The id of the one login for this address, narrowed by a username if the tool knows it. Several and none are the same to a tool:
/// nothing to say, so it goes on to whatever else it has.
pub fn find_login(protocol: &str, host: &str, username: Option<&str>) -> Result<Option<String>, Refusal> {
    let mut fitting = logins_for(protocol, host, username)?;
    Ok(if fitting.len() == 1 { fitting.pop().map(|(id, _)| id) } else { None })
}

/// Every login kept for this address, with whether Docker stored it (and so may replace or erase it)
pub fn logins_for(protocol: &str, host: &str, username: Option<&str>) -> Result<Vec<(String, bool)>, Refusal> {
    let listed = ask(json!({"op": "list", "kind": "password"}))?;
    let mut fitting: Vec<(String, bool)> = listed["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| item["meta"]["address"].as_str().is_some_and(|address| address_fits(address, protocol, host)))
        .filter(|item| username.is_none_or(|wanted| item["meta"]["username"].as_str().is_none_or(|kept| kept.is_empty() || kept == wanted)))
        .filter_map(|item| Some((item["id"].as_str()?.to_string(), item["meta"]["docker"] == json!(true))))
        .collect();
    fitting.dedup();
    Ok(fitting)
}

/// Every credential helper git would call that is not this one, from every file git reads and every spelling of the setting: `credential.helper`,
/// a URL's (`credential.https://host.helper`) and a repository's. Run from where git called us, so that the repository's is read. `None` if git
/// cannot say, which counts as there being one.
pub fn other_helpers(folder: Option<&std::path::Path>) -> Option<Vec<String>> {
    let mut command = Command::new("git");
    command.args(["config", "--get-regexp", "^credential(\\..*)?\\.helper$"]);
    if let Some(folder) = folder {
        command.current_dir(folder);
    }
    let done = command.stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
    match done.status.code() {
        // Nothing set
        Some(1) => return Some(Vec::new()),
        Some(0) => {}
        _ => return None,
    }
    let ours = |helper: &str| helper == "amethystora" || helper.rsplit('/').next() == Some("git-credential-amethystora");
    Some(String::from_utf8_lossy(&done.stdout)
        .lines()
        .filter_map(|line| line.split_once(' ').map(|(_, value)| value.trim().to_string()).or_else(|| Some(String::new())))
        .filter(|helper| !helper.is_empty() && !ours(helper))
        .collect())
}

/// The login itself, which is what asks the person
pub fn read_login(id: &str) -> Result<Login, Refusal> {
    let answer = ask(json!({"op": "get", "kind": "password", "id": id}))?;
    let password = Zeroizing::new(answer["value"]["password"].as_str().ok_or(Refusal::Failed)?.to_string());
    Ok(Login { id: id.to_string(), username: answer["value"]["username"].as_str().unwrap_or_default().to_string(), password })
}

/// What git says on standard input: `key=value` lines to a blank one. Lines end at a newline and nowhere else, as git's do: a carriage return is part of the value.
pub fn git_fields(input: &str) -> Vec<(&str, &str)> {
    input.split('\n').take_while(|line| !line.is_empty()).filter_map(|line| line.split_once('=')).collect()
}

/// The value of a field, the way git reads one said twice: the last wins
pub fn git_field<'a>(fields: &[(&'a str, &'a str)], name: &str) -> Option<&'a str> {
    fields.iter().rev().find(|(key, _)| *key == name).map(|(_, value)| *value)
}

/// git's answer to `get`, or nothing: a newline in a value would let the login write fields of its own
pub fn git_answer(login: &Login) -> Option<Zeroizing<String>> {
    let fine = |text: &str| !text.contains(['\n', '\r', '\0']);
    if !fine(&login.username) || !fine(&login.password) {
        return None;
    }
    let mut out = Zeroizing::new(String::new());
    if !login.username.is_empty() {
        out.push_str(&format!("username={}\n", login.username));
    }
    out.push_str(&format!("password={}\n", login.password.as_str()));
    Some(out)
}

/// The host and scheme Docker's name for a registry comes to: a bare name is https, and Docker Hub's old name is its own
pub fn docker_server(server: &str) -> Option<(String, String)> {
    // A registry's name has no login in it: `a@b` is b's, and a request for it names a
    let rest = server.trim().split_once("://").map_or(server.trim(), |(_, rest)| rest);
    if rest.split(['/', '?', '#']).next()?.contains('@') {
        return None;
    }
    let (scheme, host) = parse_address(server)?;
    Some((scheme.unwrap_or_else(|| "https".to_string()), host))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_fits_by_its_host_and_scheme() {
        assert!(address_fits("https://github.com/", "https", "github.com"));
        assert!(address_fits("github.com", "https", "github.com"));
        assert!(address_fits("GitHub.com/org", "https", "github.com"));
        assert!(address_fits("https://me@github.com/x", "https", "GITHUB.com"));
        assert!(address_fits("http://intranet:8080", "http", "intranet:8080"));
        // A login never goes out in the clear unless it says so
        assert!(!address_fits("github.com", "http", "github.com"));
        assert!(!address_fits("https://github.com", "http", "github.com"));
        // Nor anywhere that only looks like it
        assert!(!address_fits("github.com", "https", "github.com.evil.example"));
        assert!(!address_fits("github.com", "https", "evil.example"));
        assert!(!address_fits("github.com.evil.example", "https", "github.com"));
        assert!(!address_fits("https://evil.example/github.com", "https", "github.com"));
        assert!(!address_fits("", "https", ""));
        assert!(!address_fits("a b", "https", "a b"));
    }

    #[test]
    fn git_gets_nothing_a_login_could_write_fields_with() {
        let login = |username: &str, password: &str| Login { id: String::new(), username: username.to_string(), password: Zeroizing::new(password.to_string()) };
        assert_eq!(git_answer(&login("me", "pw")).unwrap().as_str(), "username=me\npassword=pw\n");
        assert_eq!(git_answer(&login("", "pw")).unwrap().as_str(), "password=pw\n");
        assert!(git_answer(&login("me", "pw\nurl=https://evil.example")).is_none());
        assert!(git_answer(&login("me\nx=y", "pw")).is_none());
        assert!(git_answer(&login("me", "pw\0")).is_none());
    }

    #[test]
    fn what_git_says_is_read_to_the_blank_line() {
        let fields = git_fields("protocol=https\nhost=github.com\nnonsense\n\nhost=other\n");
        assert_eq!(fields, [("protocol", "https"), ("host", "github.com")]);
    }

    #[test]
    fn a_field_said_twice_is_the_last_one_and_a_carriage_return_is_not_dropped() {
        let fields = git_fields("host=evil.example\nhost=github.com\n");
        assert_eq!(git_field(&fields, "host"), Some("github.com"));
        let fields = git_fields("host=github.com\r\n");
        assert_eq!(git_field(&fields, "host"), Some("github.com\r"));
        assert!(!address_fits("github.com", "https", "github.com\r"));
        assert!(!address_fits("github.com", "https", "evil.example@github.com"));
    }

    #[test]
    fn a_registry_with_a_login_in_front_of_it_is_nobody_s() {
        assert_eq!(docker_server("evil.example@registry.example"), None);
        assert_eq!(docker_server("https://evil.example@registry.example/v2/"), None);
        assert_eq!(docker_server("registry.example/v2/"), Some(("https".to_string(), "registry.example".to_string())));
    }
}
