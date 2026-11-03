//! git-credential-amethystora: git's credential helper. `get` answers from the vault. `store` and `erase` do nothing: git calls `store`
//! with whatever worked and `erase` with whatever a server turned down, which may be a password that has only expired, and a
//! change to what the vault holds is the person's, in the window or with `ame vault`.

use std::io::Write;
use std::process::ExitCode;

use amethystora_vault_client::Refusal;
use amethystora_vault_service::credentials::{find_login, git_answer, git_field, git_fields, other_helpers, read_input, read_login, say_closed};

fn get() -> Result<(), Refusal> {
    let input = read_input().ok_or(Refusal::BadRequest)?;
    let fields = git_fields(&input);
    let field = |name: &str| git_field(&fields, name);
    let (Some(protocol), Some(host)) = (field("protocol"), field("host")) else { return Ok(()) };
    if !matches!(protocol, "https" | "http") {
        return Ok(());
    }
    // git hands what worked to every helper it has, and a password from the vault is not for another that stores it. Asked here, where git
    // called us, since which helpers it has depends on the repository, the address and the account's files.
    match other_helpers(None) {
        Some(others) if others.is_empty() => {}
        _ => {
            eprintln!("git-credential-amethystora: git has another credential helper, so the vault gives nothing (ame vault helpers)");
            return Ok(());
        }
    }
    let Some(id) = find_login(protocol, host, field("username"))? else { return Ok(()) };
    let login = read_login(&id)?;
    if let Some(answer) = git_answer(&login) {
        let _ = std::io::stdout().write_all(answer.as_bytes());
    }
    Ok(())
}

fn main() -> ExitCode {
    let action = std::env::args_os().nth(1);
    if action.as_deref() != Some(std::ffi::OsStr::new("get")) {
        // Read to the end, so that git's write never meets a closed pipe
        let _ = read_input();
        return ExitCode::SUCCESS;
    }
    // git goes on to its next helper, or asks, whatever is wrong here: an account with no vault is not an error of git's
    if get() == Err(Refusal::Closed) {
        say_closed();
    }
    ExitCode::SUCCESS
}
