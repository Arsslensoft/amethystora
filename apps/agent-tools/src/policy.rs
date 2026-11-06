//! Who may use what: the account's two switches and the administrator's /etc/amethystora/agent.conf.
//!
//! Level 1, "Agentic features", is on while ~/.config/amethystora/agentic exists: the agent, its skills and
//! the read-only tools. Level 2, "Let the agent make changes", is on while ~/.config/amethystora/agent-tools
//! exists as well: the tools that change things. Neither exists for any account as the image ships.
//!
//! agent.conf caps both, and says how the agent's root commands are allowed:
//!
//!   ALLOW=act|know|off          the most any account may turn on
//!   ALLOW_FOR=account:level     that account's own ceiling instead, one line each
//!   ROOT=session|minutes|each|terminal
//!
//! The file is read here, by the tool server, the launcher (`amethystora-agent policy`, which runs this) and
//! the root helper, which runs as root and trusts nothing its caller says. A value it cannot read counts as
//! the strictest there is, and a missing file as the image's defaults.

use std::collections::BTreeMap;
use std::ffi::CStr;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

pub const CONF: &str = "/etc/amethystora/agent.conf";

/// What an account may do with the agent, from nothing to everything
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Off,
    Know,
    Act,
}

impl Level {
    pub fn parse(word: &str) -> Option<Level> {
        match word {
            "off" => Some(Level::Off),
            "know" => Some(Level::Know),
            "act" => Some(Level::Act),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Level::Off => "off",
            Level::Know => "know",
            Level::Act => "act",
        }
    }
}

/// How the agent's root commands are allowed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootMode {
    /// One password, then a helper held open for that agent until it ends, the screen locks or an hour passes
    Session,
    /// A password, then polkit's own five minutes, for that tool process only
    Minutes,
    /// A password for every command
    Each,
    /// No polkit: every root command opens in a terminal, with sudo
    Terminal,
}

impl RootMode {
    pub fn parse(word: &str) -> Option<RootMode> {
        match word {
            "session" => Some(RootMode::Session),
            "minutes" => Some(RootMode::Minutes),
            "each" => Some(RootMode::Each),
            "terminal" => Some(RootMode::Terminal),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            RootMode::Session => "session",
            RootMode::Minutes => "minutes",
            RootMode::Each => "each",
            RootMode::Terminal => "terminal",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Conf {
    pub allow: Level,
    pub root: RootMode,
    pub allow_for: BTreeMap<String, Level>,
}

impl Default for Conf {
    /// The image's own defaults, which /usr/etc/amethystora/agent.conf ships too
    fn default() -> Self {
        Conf { allow: Level::Act, root: RootMode::Session, allow_for: BTreeMap::new() }
    }
}

/// An account's name as ALLOW_FOR takes it: what useradd and a domain's names allow, nothing that could
/// split the line
fn account_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '@' | '$' | '\\'))
        && !name.starts_with('-')
}

impl Conf {
    /// The file's text, read strictly: every line is a comment, empty, or KEY=VALUE with a known key and value.
    /// Anything else makes what it would have set the strictest there is.
    pub fn parse(text: &str) -> Conf {
        let mut conf = Conf::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                conf.allow = Level::Off;
                continue;
            };
            let value = value.trim().trim_matches('"');
            match key.trim() {
                "ALLOW" => conf.allow = Level::parse(value).unwrap_or(Level::Off),
                "ROOT" => conf.root = RootMode::parse(value).unwrap_or(RootMode::Terminal),
                "ALLOW_FOR" => match value.rsplit_once(':') {
                    Some((name, level)) if account_name(name) => {
                        conf.allow_for.insert(name.to_string(), Level::parse(level).unwrap_or(Level::Off));
                    }
                    _ => conf.allow = Level::Off,
                },
                _ => conf.allow = Level::Off,
            }
        }
        conf
    }

    /// The file, or the image's defaults where there is none. One that cannot be read counts as off.
    pub fn read(path: &Path) -> Conf {
        match std::fs::read_to_string(path) {
            Ok(text) => Conf::parse(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Conf::default(),
            Err(_) => Conf { allow: Level::Off, root: RootMode::Terminal, allow_for: BTreeMap::new() },
        }
    }

    /// The most this account may turn on: its own line, else everyone's
    pub fn ceiling(&self, account: &str) -> Level {
        self.allow_for.get(account).copied().unwrap_or(self.allow)
    }

    /// The file as `ame agent lock` and `ame agent root` write it: the two settings, then each account's line
    pub fn render(&self) -> String {
        let mut text = String::from(
            "# The AI agent, for every account on this machine (ame agent lock, ame agent root; Control's AI agent page).\n\
             # The most any account may turn on: act (both levels), know (level 1 only) or off\n",
        );
        text.push_str(&format!("ALLOW={}\n", self.allow.word()));
        text.push_str("# How the agent's root commands are allowed: session, minutes, each or terminal\n");
        text.push_str(&format!("ROOT={}\n", self.root.word()));
        if !self.allow_for.is_empty() {
            text.push_str("# One account's own ceiling, in place of ALLOW for it\n");
            for (name, level) in &self.allow_for {
                text.push_str(&format!("ALLOW_FOR={name}:{}\n", level.word()));
            }
        }
        text
    }
}

/// Whether an account by this name exists, as the system knows them (a domain's included)
pub fn account_exists(name: &str) -> bool {
    let Ok(name) = std::ffi::CString::new(name) else { return false };
    let mut buffer = vec![0 as libc::c_char; 16384];
    let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result: *mut libc::passwd = std::ptr::null_mut();
    let found = unsafe { libc::getpwnam_r(name.as_ptr(), &mut entry, buffer.as_mut_ptr(), buffer.len(), &mut result) };
    found == 0 && !result.is_null()
}

/// `amethystora-agent-tools conf ...`, as root: one change to agent.conf, written whole and moved into place.
///
///   conf allow act|know|off [ACCOUNT]   the most every account, or one account, may turn on
///   conf allow default ACCOUNT          that account follows everyone's again
///   conf root session|minutes|each|terminal
pub fn change_conf(path: &Path, args: &[String]) -> Result<Conf, String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("the AI agent's lock and root mode are an administrator's: run it with sudo".into());
    }
    let mut conf = Conf::read(path);
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["allow", "default", account] if account_name(account) => {
            conf.allow_for.remove(*account);
        }
        ["allow", level] => conf.allow = Level::parse(level).ok_or("a level is act, know or off")?,
        ["allow", level, account] => {
            let level = Level::parse(level).ok_or("a level is act, know, off or default")?;
            if !account_name(account) || !account_exists(account) {
                return Err(format!("there is no account {account}"));
            }
            conf.allow_for.insert(account.to_string(), level);
        }
        ["root", mode] => conf.root = RootMode::parse(mode).ok_or("a root mode is session, minutes, each or terminal")?,
        _ => return Err("usage: conf allow act|know|off [ACCOUNT] | conf allow default ACCOUNT | conf root session|minutes|each|terminal".into()),
    }
    let next = path.with_extension("conf.new");
    std::fs::write(&next, conf.render()).map_err(|error| format!("{}: {error}", next.display()))?;
    std::fs::set_permissions(&next, std::os::unix::fs::PermissionsExt::from_mode(0o644)).map_err(|error| error.to_string())?;
    std::fs::rename(&next, path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(conf)
}

/// The account's config folder, ~/.config/amethystora, as the launcher and the theme scripts have it
pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"));
    base.join("amethystora")
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

/// The name of the account with this uid, as the system knows it (a domain's accounts included)
pub fn account_of(uid: u32) -> Option<String> {
    let mut buffer = vec![0 as libc::c_char; 16384];
    let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result: *mut libc::passwd = std::ptr::null_mut();
    let found = unsafe { libc::getpwuid_r(uid, &mut entry, buffer.as_mut_ptr(), buffer.len(), &mut result) };
    if found != 0 || result.is_null() {
        return None;
    }
    Some(unsafe { CStr::from_ptr(entry.pw_name) }.to_string_lossy().into_owned())
}

/// This account's name
pub fn account() -> String {
    account_of(unsafe { libc::getuid() }).unwrap_or_else(|| unsafe { libc::getuid() }.to_string())
}

/// What this account has turned on, capped by the administrator's ceiling
#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    pub conf: Conf,
    pub account: String,
    /// The account's own switches, whatever the ceiling
    pub level1: bool,
    pub level2: bool,
}

impl Policy {
    /// The policy now. A server started with AMETHYSTORA_READ_ONLY=1 (an agent asked a question in the pill, which changes nothing:
    /// `amethystora-agent ask-here`) never has the tools that change things, whatever the account turned on: the variable can only
    /// lower the level, so a program that sets it gains nothing.
    pub fn current() -> Policy {
        let mut policy = Policy::load(Path::new(CONF), &config_dir(), account());
        if std::env::var_os("AMETHYSTORA_READ_ONLY").is_some_and(|value| value == "1") {
            policy.level2 = false;
        }
        policy
    }

    pub fn load(conf: &Path, config: &Path, account: String) -> Policy {
        Policy {
            conf: Conf::read(conf),
            account,
            level1: config.join("agentic").exists(),
            level2: config.join("agent-tools").exists(),
        }
    }

    pub fn ceiling(&self) -> Level {
        self.conf.ceiling(&self.account)
    }

    /// The level in effect: what the account turned on, as far as the ceiling lets it
    pub fn level(&self) -> Level {
        let own = match (self.level1, self.level2) {
            (false, _) => Level::Off,
            (true, false) => Level::Know,
            (true, true) => Level::Act,
        };
        own.min(self.ceiling())
    }

    pub fn json(&self) -> Value {
        let allow_for: serde_json::Map<String, Value> =
            self.conf.allow_for.iter().map(|(name, level)| (name.clone(), json!(level.word()))).collect();
        json!({
            "account": self.account,
            "allow": self.conf.allow.word(),
            "allow_for": allow_for,
            "ceiling": self.ceiling().word(),
            "root": self.conf.root.word(),
            "agentic": self.level1,
            "tools": self.level2,
            "level": self.level().word(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_file_strictly() {
        assert_eq!(Conf::parse(""), Conf::default());
        let conf = Conf::parse("# comment\nALLOW=know\nROOT=each\nALLOW_FOR=kid:off\nALLOW_FOR=me@example.com:act\n");
        assert_eq!(conf.allow, Level::Know);
        assert_eq!(conf.root, RootMode::Each);
        assert_eq!(conf.ceiling("kid"), Level::Off);
        assert_eq!(conf.ceiling("me@example.com"), Level::Act);
        assert_eq!(conf.ceiling("other"), Level::Know);
        // What cannot be read counts as the strictest
        assert_eq!(Conf::parse("ALLOW=yes").allow, Level::Off);
        assert_eq!(Conf::parse("ROOT=always").root, RootMode::Terminal);
        assert_eq!(Conf::parse("garbage").allow, Level::Off);
        assert_eq!(Conf::parse("ALLOW_FOR=a b:act").allow, Level::Off);
        assert_eq!(Conf::parse("ALLOW_FOR=kid:maybe").ceiling("kid"), Level::Off);
        assert_eq!(Conf::parse("UNKNOWN=1").allow, Level::Off);
        assert_eq!(Conf::parse("ALLOW=\"know\"").allow, Level::Know);
        // What it writes reads back the same
        assert_eq!(Conf::parse(&conf.render()), conf);
    }

    #[test]
    fn levels_are_capped_by_the_ceiling() {
        let dir = std::env::temp_dir().join(format!("amethystora-policy-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let conf = dir.join("agent.conf");
        let policy = |name: &str| Policy::load(&conf, &dir, name.to_string());
        assert_eq!(policy("a").level(), Level::Off);
        std::fs::write(dir.join("agent-tools"), "").unwrap();
        // Level 2 needs level 1
        assert_eq!(policy("a").level(), Level::Off);
        std::fs::write(dir.join("agentic"), "").unwrap();
        assert_eq!(policy("a").level(), Level::Act);
        std::fs::write(&conf, "ALLOW=know\n").unwrap();
        assert_eq!(policy("a").level(), Level::Know);
        std::fs::write(&conf, "ALLOW=act\nALLOW_FOR=kid:off\n").unwrap();
        assert_eq!(policy("kid").level(), Level::Off);
        assert_eq!(policy("a").level(), Level::Act);
        assert_eq!(policy("kid").json()["level"], "off");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
