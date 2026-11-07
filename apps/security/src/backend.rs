//! What Amethystora Security knows of the machine: what main.js did for the Electron window.
//!
//! The report is /usr/libexec/amethystora-security-status --json, the script `ame security status`
//! prints, so the window and the terminal cannot disagree. A scan started here runs as the user and
//! reads only what the user can: through clamd when its socket is open to the user, and with clamscan,
//! which loads the signatures itself, when it is not. The whole machine is scanned by starting the
//! weekly scan's own unit, which 50-amethystora-virus-scan.rules lets the person at the machine do
//! without a password. What any of them finds is only reported, unless ON_DETECTION in
//! /etc/amethystora/security.conf says to quarantine or delete it: then a scan started here hands what it
//! found to /usr/libexec/amethystora-quarantine, as root, after an administrator's password.
//!
//! Network protection is Suricata's (/usr/libexec/amethystora-ips). What it blocked and noticed is read
//! from the security watcher's network.json, which the members of wheel can read, and allowing a rule
//! again goes through the helper, as root, after an administrator's password.
//!
//! What is installed outside Flatpak, the containers amethystora-pkg manages with the apps exported from
//! them and what came from the AUR, is what `amethystora-pkg containers list --json` says, run as the
//! user: the containers are each account's own. What each Flatpak can reach beyond its sandbox is what
//! /usr/libexec/amethystora-app-permissions says, also run as the user, whose overrides are the ones
//! that apply; taking back what the account granted one app is `flatpak override --user --reset`.
//!
//! Every security feature is a switch, and the Settings page lists them: the detection switches in
//! /etc/amethystora/security.conf (`amethystora-security-config --list --json`) and what the image
//! enforces from the start (`amethystora-hardening status --json`), both read as the user. A change runs
//! the switch's own `ame security` command in a terminal, which says what it changes and asks; the page
//! hands over a key and one of the values this program read for it, and nothing else. Each switch names
//! its category, and the page shows a category at a time.
//!
//! A duress finger or password is set up, tested and removed by `ame security duress` in a terminal, one of
//! its own steps a button. Nothing is read for it: the window says no more than the report does, which is
//! nothing about whether an account has one.
//!
//! A command offered to run in a terminal is looked up in the report this program read, never taken from
//! what the page shows; an event is looked up again in events.json by its id, and its entry is the one
//! the watcher wrote into it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use amethystora_ui::process::{self, Options, Output};
use amethystora_ui::{glib, t};
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

pub const REPORT: &str = "/usr/libexec/amethystora-security-status";
pub const SIGNATURE_AGE: &str = "/usr/libexec/amethystora-clamav-signature-age";
pub const SECURITY_CONFIG: &str = "/usr/libexec/amethystora-security-config";
pub const HARDENING: &str = "/usr/libexec/amethystora-hardening";
pub const AME: &str = "/usr/bin/ame";
pub const QUARANTINE: &str = "/usr/libexec/amethystora-quarantine";
pub const CLAMD_CONFIG: &str = "/etc/clamd.d/scan.conf";
pub const CLAMD_SOCKET: &str = "/run/clamd.scan/clamd.sock";
/// The weekly scan of the whole machine, and the root-only file it writes its findings to
pub const MACHINE_SERVICE: &str = "amethystora-clamav-scan.service";
pub const MACHINE_TIMER: &str = "amethystora-clamav-scan.timer";
/// The service's object on systemd's bus
pub const MACHINE_UNIT_PATH: &str = "/org/freedesktop/systemd1/unit/amethystora_2dclamav_2dscan_2eservice";
pub const MACHINE_FINDINGS: &str = "/var/log/amethystora-clamav-findings.log";
/// Network protection: its helper, the rules allowed on this machine, and what the watcher keeps of its
/// alerts (network.json, for wheel) and of its state (status.json, for everyone)
pub const IPS: &str = "/usr/libexec/amethystora-ips";
pub const IPS_UNIT: &str = "amethystora-ips.service";
pub const IPS_ALLOWED: &str = "/etc/amethystora/ips-allowed";
pub const NETWORK_EVENTS: &str = "/var/lib/amethystora/security/network.json";
/// What the security watcher found, newest first, which the members of wheel can read: the Events page
pub const WATCH_EVENTS: &str = "/var/lib/amethystora/security/events.json";
pub const WATCH_STATUS: &str = "/var/lib/amethystora/security/status.json";
pub const ALLOW: &str = "/usr/libexec/amethystora-security-allow";
/// The containers of other distributions
pub const PKG: &str = "/usr/bin/amethystora-pkg";
/// The AI agent, which a check that needs attention can be handed to, unless the account turned it off
pub const AGENT: &str = "/usr/bin/amethystora-agent";
/// What each Flatpak can reach beyond its sandbox, and Flatseal, where any of it is changed
pub const PERMISSIONS: &str = "/usr/libexec/amethystora-app-permissions";
pub const FLATSEAL: &str = "com.github.tchx84.Flatseal";
/// A YARA rule's match names its author, as the rule's licence asks: from the credits amethystora-yara keeps
/// for the rules it put in ClamAV's folder, "rule<TAB>source<TAB>author<TAB>licence" a line
pub const YARA_CREDITS: &str = "/var/lib/amethystora/yara/credits.tsv";
/// The scans started here that are kept
pub const HISTORY_LENGTH: usize = 20;
/// A scan that finds more than this keeps the first of them: enough to act on
pub const FINDINGS_KEPT: usize = 500;

/// The pages `amethystora-security PAGE` opens, besides the overview
pub const PAGES: [&str; 6] = ["scan", "network", "apps", "containers", "events", "settings"];
/// The Settings page's categories, which `amethystora-security settings/CATEGORY` opens (the shield in the top bar opens
/// the one a line is about). Never the Under duress page, whose page must look the same whatever an account has.
pub const CATEGORIES: [&str; 11] =
    ["watching", "audit", "detection", "scans", "network", "logins", "devices", "apps", "system", "notices", "away"];

/// A page the command line may open: one of PAGES, settings/ and one of CATEGORIES, or apps/ and a Flatpak app's ID
/// (the app grid's What can it reach? opens the Apps page on that app)
pub fn openable(page: &str) -> bool {
    PAGES.contains(&page)
        || page.strip_prefix("settings/").is_some_and(|category| CATEGORIES.contains(&category))
        || page.strip_prefix("apps/").is_some_and(flatpak_id)
}

/// Flatpak's own rule for an app's ID: two dots at least, each part a letter or underscore first, 255 at most
pub fn flatpak_id(id: &str) -> bool {
    static ID: OnceLock<Regex> = OnceLock::new();
    id.len() <= 255
        && ID.get_or_init(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_-]*(\.[A-Za-z_][A-Za-z0-9_-]*)+$").expect("a valid expression")).is_match(id)
}

// --- Where things are ---------------------------------------------------------------------------------------

fn env_dir(name: &str, fallback: &str) -> PathBuf {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| amethystora_ui::palette::home().join(fallback))
}

/// The scans started here: what was scanned, when, and what was found. Readable by this account only.
pub fn history_file() -> PathBuf {
    env_dir("XDG_STATE_HOME", ".local/state").join("amethystora").join("security-scans.json")
}

/// Whether the AUR is on for this account (ame apps aur)
pub fn aur_on() -> bool {
    env_dir("XDG_CONFIG_HOME", ".config").join("amethystora").join("aur").exists()
}

/// /var/home/<name> here, which is also how clamd's ExcludePath rules spell a home directory
pub fn home() -> String {
    let home = amethystora_ui::palette::home();
    std::fs::canonicalize(&home).unwrap_or(home).to_string_lossy().into_owned()
}

/// The account this window runs as, as os.userInfo() named it
pub fn username() -> String {
    // SAFETY: getpwuid's answer is read straight away, on the one thread that asks
    unsafe {
        let entry = libc::getpwuid(libc::getuid());
        if entry.is_null() || (*entry).pw_name.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr((*entry).pw_name).to_string_lossy().into_owned()
    }
}

pub fn now() -> f64 {
    amethystora_ui::i18n::now()
}

// --- JSON as the page read it ---------------------------------------------------------------------------------

/// How JavaScript's String() writes a number
fn js_number(n: f64) -> String {
    if n.is_nan() {
        "NaN".into()
    } else if n.fract() == 0.0 && n.abs() < 1e21 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// What JavaScript's String() makes of a value: the ids and rule numbers the page compared as text
pub fn js_string(value: Option<&Value>) -> String {
    match value {
        None => "undefined".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => js_number(n.as_f64().unwrap_or(f64::NAN)),
        Some(other) => other.to_string(),
    }
}

/// A value as the page showed it: nothing for one that is null or not there
pub fn text(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(value) => js_string(Some(value)),
    }
}

/// JavaScript's truthiness, which the page decided most things by
pub fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|n| n != 0.0 && !n.is_nan()),
        Some(Value::String(text)) => !text.is_empty(),
        Some(_) => true,
    }
}

fn number(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64)
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value.and_then(Value::as_array).map(|list| list.iter().map(|item| text(Some(item))).collect()).unwrap_or_default()
}

/// A string that is there and not empty
fn some_text(value: Option<&Value>) -> Option<String> {
    Some(text(value)).filter(|text| !text.is_empty())
}

/// A number written as an integer when it is one, as JSON.stringify writes it
fn whole<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 9e15 {
        serializer.serialize_i64(*value as i64)
    } else {
        serializer.serialize_f64(*value)
    }
}

/// Any number, and nothing for anything else, as Number() would leave it unusable
fn any_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    Ok(Value::deserialize(deserializer)?.as_f64().unwrap_or(0.0))
}

fn any_text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(text(Some(&Value::deserialize(deserializer)?)))
}

fn any_bool<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(truthy(Some(&Value::deserialize(deserializer)?)))
}

/// A file read as JSON: not there or not JSON, or not readable by this account, which the page says
pub enum Json {
    Missing,
    Denied,
    Value(Value),
}

pub fn read_json(file: &str) -> Json {
    match std::fs::read_to_string(file) {
        Ok(text) => serde_json::from_str(&text).map(Json::Value).unwrap_or(Json::Missing),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => Json::Denied,
        Err(_) => Json::Missing,
    }
}

impl Json {
    fn array(self) -> Option<Vec<Value>> {
        match self {
            Json::Value(Value::Array(list)) => Some(list),
            _ => None,
        }
    }
}

/// execFile with a timeout of its own
async fn run_for(program: &str, args: &[&str], seconds: u64) -> Output {
    process::run_with(program, args, Options { timeout: Duration::from_secs(seconds), ..Default::default() }).await
}

/// "{program} stopped with status {code}." for a helper that said nothing of why
fn stopped(program: &str, output: &Output) -> String {
    let said = output.stderr.trim();
    if said.is_empty() { t!("{program} stopped with status {code}.", program = program, code = output.code.to_string()) } else { said.to_string() }
}

// --- The report -----------------------------------------------------------------------------------------------

/// One check of the report, in the words the report wrote it in
#[derive(Debug, Clone, Default)]
pub struct Check {
    pub group: String,
    pub id: String,
    /// ok, check, off or info; for the optional protections on, off or unavailable
    pub state: String,
    pub title: String,
    pub text: String,
    pub details: Vec<String>,
    /// One command, which fixes it or turns it on, in a terminal
    pub command: Option<String>,
    /// Accepted on this machine in the state it is in (ame security allow report)
    pub accepted: bool,
    /// Its owner switched it off through its switch
    pub switched_off: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ReportSettings {
    pub on_detection: String,
    pub network: String,
    pub virus_scan: String,
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub image: String,
    pub settings: ReportSettings,
    pub checks: Vec<Check>,
    pub error: Option<String>,
    /// When it was read, in seconds
    pub checked: f64,
    /// Whether a check can be handed to the AI agent: the agentic features are on for this account
    pub agent: bool,
}

/// The report's JSON, or None when it is not one with a list of checks
pub fn parse_report(stdout: &str) -> Option<Report> {
    let parsed: Value = serde_json::from_str(stdout).ok()?;
    let checks = parsed.get("checks")?.as_array()?;
    let settings = parsed.get("settings");
    let setting = |key: &str| text(settings.and_then(|s| s.get(key)));
    Some(Report {
        image: text(parsed.get("image")),
        settings: ReportSettings {
            on_detection: setting("on_detection"),
            network: setting("network"),
            virus_scan: setting("virus_scan"),
        },
        checks: checks
            .iter()
            .map(|check| Check {
                group: text(check.get("group")),
                id: text(check.get("id")),
                state: text(check.get("state")),
                title: text(check.get("title")),
                text: text(check.get("text")),
                details: strings(check.get("details")),
                command: some_text(check.get("command")),
                accepted: truthy(check.get("accepted")),
                switched_off: truthy(check.get("switched_off")),
            })
            .collect(),
        ..Default::default()
    })
}

pub async fn report() -> Report {
    let status = glib::spawn_future_local(process::run(REPORT, &["--json"]));
    let agent = glib::spawn_future_local(process::run(AGENT, &["status"]));
    let status = status.await.unwrap_or_default();
    let agent = agent.await.map(|o| o.code == 0).unwrap_or(false);
    let mut report = parse_report(&status.stdout).unwrap_or_else(|| {
        let said = status.stderr.trim();
        Report {
            image: t!("unknown"),
            error: Some(if said.is_empty() { t!("The report could not be read.") } else { said.to_string() }),
            ..Default::default()
        }
    });
    report.checked = now();
    report.agent = agent;
    report
}

impl Report {
    /// A check that needs attention, by its id: never one of the optional protections, nor what the machine sends
    pub fn attention_check(&self, id: &str) -> Option<&Check> {
        self.checks.iter().find(|check| check.id == id && check.group != "optional" && check.group != "sends")
    }
}

/// A check's command, in a terminal, looked up in the report this window read. A command is one command
/// and its arguments, split where the report put spaces.
pub fn run_check(report: Option<&Report>, id: &str) {
    let Some(command) = report.and_then(|r| r.checks.iter().find(|check| check.id == id)).and_then(|check| check.command.clone()) else { return };
    static SPACES: OnceLock<Regex> = OnceLock::new();
    let spaces = SPACES.get_or_init(|| Regex::new(r"\s+").expect("a valid expression"));
    let args: Vec<&str> = spaces.split(&command).collect();
    process::in_terminal(&args);
}

/// A check that needs attention, handed to the agent in a terminal by the title and sentence the report
/// gave it. The agent reads the report itself, and the diagnose skill keeps it from changing anything
/// until asked. The words around them are the agent's prompt, not the page's, and stay in English like the
/// rest of it (amethystora-agent).
pub fn diagnose(report: Option<&Report>, id: &str) {
    let Some(report) = report.filter(|r| r.agent) else { return };
    if let Some(check) = report.attention_check(id) {
        let prompt = format!("The Amethystora security report (/usr/libexec/amethystora-security-status) flags \"{}\": {}", check.title, check.text);
        process::launch(AGENT, &["diagnose", &prompt]);
    }
}

/// A check that needs attention, accepted on this machine, or shown again: `ame security allow report` in a
/// terminal, which asks for the password. Only a check the report this window read gives in that state.
pub fn accept(report: Option<&Report>, id: &str, again: bool) {
    let Some(check) = report.and_then(|r| r.attention_check(id)) else { return };
    if if again { !check.accepted } else { check.state != "check" && check.state != "off" } {
        return;
    }
    if again {
        process::in_terminal(&[AME, "security", "allow", "report", &check.id, "remove"]);
    } else {
        process::in_terminal(&[AME, "security", "allow", "report", &check.id]);
    }
}

// --- The switches ---------------------------------------------------------------------------------------------

/// What a switch costs while it is on, from systemd's accounting
#[derive(Debug, Clone, Default)]
pub struct Cost {
    pub memory: Option<f64>,
    pub seconds: Option<f64>,
    pub milliseconds: Option<f64>,
}

/// A detection switch in /etc/amethystora/security.conf
#[derive(Debug, Clone, Default)]
pub struct Setting {
    pub key: String,
    /// Its name for `ame security settings`
    pub setting: String,
    pub value: String,
    pub title: String,
    pub text: String,
    pub group: String,
    pub allowed: Vec<String>,
    /// What it sends to somebody else, said before it is turned on
    pub explain: Option<String>,
    pub cost: Option<Cost>,
}

/// One kernel argument of the kernel-args switch
#[derive(Debug, Clone, Default)]
pub struct Argument {
    pub argument: String,
    pub state: String,
}

/// One of what the image enforces from the start, or of the stricter settings
#[derive(Debug, Clone, Default)]
pub struct Hardening {
    pub name: String,
    pub state: String,
    pub title: String,
    pub text: String,
    /// The Settings page's category, among the detection switches' own
    pub group: String,
    /// It cannot change on this machine (a sealed one's kernel arguments)
    pub fixed: bool,
    /// on for what the image enforces, off for the stricter ones
    pub default: String,
    pub level: String,
    pub levels: Option<Vec<String>>,
    pub arguments: Option<Vec<Argument>>,
}

#[derive(Debug, Clone, Default)]
pub struct Switches {
    /// Which profile the keys match: off, default, light, balanced, thorough or custom
    pub profile: String,
    pub settings: Vec<Setting>,
    pub hardening: Vec<Hardening>,
    pub error: Option<String>,
}

fn parse_cost(value: Option<&Value>) -> Option<Cost> {
    let value = value.filter(|v| v.is_object())?;
    Some(Cost { memory: number(value.get("memory")), seconds: number(value.get("seconds")), milliseconds: number(value.get("milliseconds")) })
}

/// `amethystora-security-config --list --json`: {profile, settings: [...]}
pub fn parse_settings(stdout: &str) -> Option<(Option<String>, Vec<Setting>)> {
    let parsed: Value = serde_json::from_str(stdout).ok()?;
    let profile = parsed.get("profile").map(|p| text(Some(p)));
    let settings = parsed
        .get("settings")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|entry| Setting {
                    key: text(entry.get("key")),
                    setting: text(entry.get("setting")),
                    value: text(entry.get("value")),
                    title: text(entry.get("title")),
                    text: text(entry.get("text")),
                    group: text(entry.get("group")),
                    allowed: strings(entry.get("allowed")),
                    explain: some_text(entry.get("explain")),
                    cost: parse_cost(entry.get("cost")),
                })
                .collect()
        })
        .unwrap_or_default();
    Some((profile, settings))
}

/// `amethystora-hardening status --json`: one entry a switch
pub fn parse_hardening(stdout: &str) -> Option<Vec<Hardening>> {
    let parsed: Value = serde_json::from_str(stdout).ok()?;
    let list = parsed.as_array()?;
    Some(
        list.iter()
            .map(|entry| Hardening {
                name: text(entry.get("name")),
                state: text(entry.get("state")),
                title: text(entry.get("title")),
                text: text(entry.get("text")),
                group: text(entry.get("group")),
                fixed: truthy(entry.get("fixed")),
                default: text(entry.get("default")),
                level: text(entry.get("level")),
                levels: entry.get("levels").and_then(Value::as_array).map(|_| strings(entry.get("levels"))),
                arguments: entry.get("arguments").and_then(Value::as_array).map(|list| {
                    list.iter().map(|item| Argument { argument: text(item.get("argument")), state: text(item.get("state")) }).collect()
                }),
            })
            .collect(),
    )
}

pub async fn switches() -> Switches {
    let detection = glib::spawn_future_local(process::run(SECURITY_CONFIG, &["--list", "--json"]));
    let enforced = glib::spawn_future_local(process::run(HARDENING, &["status", "--json"]));
    let detection = detection.await.unwrap_or_default();
    let enforced = enforced.await.unwrap_or_default();
    let mut switches = Switches { profile: "custom".into(), ..Default::default() };
    match parse_settings(&detection.stdout) {
        Some((profile, settings)) => {
            if let Some(profile) = profile {
                switches.profile = profile;
            }
            switches.settings = settings;
        }
        None => switches.error = Some(stopped("amethystora-security-config", &detection)),
    }
    match parse_hardening(&enforced.stdout) {
        Some(hardening) => switches.hardening = hardening,
        None => {
            if switches.error.is_none() {
                switches.error = Some(stopped("amethystora-hardening", &enforced));
            }
        }
    }
    switches
}

/// What changing a switch is
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Profile,
    Setting,
    Hardening,
}

/// The switch's own command, in a terminal: `ame security settings KEY VALUE` for a detection switch,
/// `ame security NAME [ARGUMENT] on|off` for one of what the image enforces, and a profile. The value has to
/// be one the list gave that switch; an empty one lets the command ask.
pub fn change_command(switches: Option<&Switches>, kind: Kind, key: &str, value: &str, argument: &str) -> Option<Vec<String>> {
    let own = |args: &[&str]| args.iter().map(|arg| arg.to_string()).collect::<Vec<String>>();
    match kind {
        Kind::Profile => ["off", "default", "light", "balanced", "thorough"].contains(&key).then(|| own(&[AME, "security", "profile", key])),
        Kind::Setting => {
            let entry = switches?.settings.iter().find(|entry| entry.key == key)?;
            if !value.is_empty() && !entry.allowed.iter().any(|allowed| allowed == value) {
                return None;
            }
            let mut command = own(&[AME, "security", "settings", &entry.setting]);
            if !value.is_empty() {
                command.push(value.to_string());
            }
            Some(command)
        }
        Kind::Hardening => {
            let entry = switches?.hardening.iter().find(|entry| entry.name == key)?;
            let levels = entry.levels.clone().unwrap_or_default();
            if entry.fixed || !(["on", "off", ""].contains(&value) || levels.iter().any(|level| level == value)) {
                return None;
            }
            let chosen = entry.arguments.as_ref().and_then(|list| list.iter().find(|item| item.argument == argument));
            if !argument.is_empty() && chosen.is_none() {
                return None;
            }
            let mut command = own(&[AME, "security", &entry.name]);
            if let Some(chosen) = chosen {
                command.push(chosen.argument.clone());
            }
            if !value.is_empty() {
                command.push(value.to_string());
            }
            Some(command)
        }
    }
}

pub fn change(switches: Option<&Switches>, kind: Kind, key: &str, value: &str, argument: &str) {
    if let Some(command) = change_command(switches, kind, key, value, argument) {
        let args: Vec<&str> = command.iter().map(String::as_str).collect();
        process::in_terminal(&args);
    }
}

/// The steps of `ame security duress` a button starts, by the recipe's own names for them
pub const DURESS_STEPS: [&str; 8] = ["setup", "test", "action-fingerprint", "remove", "setup-password", "action-password", "remove-password", "off"];

pub fn duress_command(step: &str) -> Option<Vec<String>> {
    DURESS_STEPS.contains(&step).then(|| [AME, "security", "duress", step].iter().map(|arg| arg.to_string()).collect())
}

pub fn duress(step: &str) {
    if let Some(command) = duress_command(step) {
        let args: Vec<&str> = command.iter().map(String::as_str).collect();
        process::in_terminal(&args);
    }
}

// --- The scanner ----------------------------------------------------------------------------------------------

/// Days since the signatures were updated, and whether that is too long
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Signatures {
    #[serde(serialize_with = "whole", deserialize_with = "any_number", default)]
    pub days: f64,
    #[serde(deserialize_with = "any_bool", default)]
    pub stale: bool,
}

/// The weekly scan of the whole machine, as systemd has it
#[derive(Debug, Clone, Default)]
pub struct Machine {
    pub running: bool,
    pub started: Option<f64>,
    /// systemd forgets when the service last ran at every boot, but not when its timer fired
    pub finished: Option<f64>,
    /// Only known for a run since boot: 0 clean, 1 found, 2 no signatures, 3 signatures too old
    pub status: Option<i32>,
    pub found: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Scanner {
    pub signatures: Option<Signatures>,
    pub machine: Machine,
}

/// Whether the owner turned the virus scanner off (VIRUS_SCAN=off): then nothing scans, here either
pub async fn scanner_off() -> bool {
    run_for(SECURITY_CONFIG, &["VIRUS_SCAN"], 10).await.stdout.trim() == "off"
}

/// Days since the signatures were updated, or None when there are none to scan with
pub async fn signature_age() -> Option<Signatures> {
    let output = run_for(SIGNATURE_AGE, &[], 10).await;
    if output.code != 0 && output.code != 2 {
        return None;
    }
    Some(Signatures { days: output.stdout.trim().parse().unwrap_or(f64::NAN), stale: output.code == 2 })
}

/// clamd's socket is only reachable by users its directory lets through
pub fn daemon_reachable() -> bool {
    let Ok(socket) = std::ffi::CString::new(CLAMD_SOCKET) else { return false };
    // SAFETY: a NUL-terminated path
    unsafe { libc::access(socket.as_ptr(), libc::R_OK | libc::W_OK) == 0 }
}

fn properties(text: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for line in text.split('\n') {
        if let Some(at) = line.find('=').filter(|at| *at > 0) {
            values.insert(line[..at].to_string(), line[at + 1..].to_string());
        }
    }
    values
}

/// systemctl --timestamp=unix: "@1758330764", or nothing for what has not happened since boot
pub fn unix_time(value: Option<&String>) -> Option<f64> {
    let digits = value?.strip_prefix('@')?;
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())).then(|| digits.parse().ok()).flatten()
}

/// The machine scan as systemctl shows its service and its timer
pub fn parse_machine(service: &str, timer: &str, found: bool) -> Machine {
    let unit = properties(service);
    let finished = unix_time(unit.get("InactiveEnterTimestamp"));
    let triggered = unix_time(properties(timer).get("LastTriggerUSec"));
    let active = unit.get("ActiveState").map(String::as_str).unwrap_or("");
    let latest = finished.unwrap_or(0.0).max(triggered.unwrap_or(0.0));
    Machine {
        running: active == "activating" || active == "active",
        started: unix_time(unit.get("InactiveExitTimestamp")),
        finished: (latest != 0.0).then_some(latest),
        status: finished.map(|_| unit.get("ExecMainStatus").and_then(|s| s.trim().parse().ok()).unwrap_or(0)),
        found,
    }
}

pub async fn scanner() -> Scanner {
    let service = glib::spawn_future_local(process::run(
        "systemctl",
        &["show", MACHINE_SERVICE, "--timestamp=unix", "-p", "ActiveState", "-p", "ExecMainStatus", "-p", "InactiveExitTimestamp", "-p", "InactiveEnterTimestamp"],
    ));
    let timer = glib::spawn_future_local(process::run("systemctl", &["show", MACHINE_TIMER, "--timestamp=unix", "-p", "LastTriggerUSec"]));
    let service = service.await.unwrap_or_default();
    let timer = timer.await.unwrap_or_default();
    // /var/log is traversable, so the size of the root-only findings file shows even though what is in it
    // does not. No findings file: the last scan was clean, or there has not been one.
    let found = std::fs::metadata(MACHINE_FINDINGS).map(|m| m.len() > 0).unwrap_or(false);
    Scanner { signatures: signature_age().await, machine: parse_machine(&service.stdout, &timer.stdout, found) }
}

/// Starting the unit is all the polkit rule allows without a password; stopping it is not
pub async fn scan_machine() -> Result<(), String> {
    if scanner_off().await {
        return Err(t!("The virus scanner is turned off. Turn it on in Settings first."));
    }
    let output = process::run("systemctl", &["start", "--no-block", MACHINE_SERVICE]).await;
    if output.code == 0 {
        Ok(())
    } else {
        let said = output.stderr.trim();
        Err(if said.is_empty() { t!("The scan could not be started.") } else { said.to_string() })
    }
}

// --- Scans started here -----------------------------------------------------------------------------------------

/// What a scan is of, as the values its sentences choose their words by: the home folder, a number of
/// files and folders, or one of them by its name
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Label {
    /// home, items or named
    pub place: String,
    pub items: Option<f64>,
    pub name: Option<String>,
    /// A label from before the page put what was scanned into words of its own: it named it in English
    pub legacy: bool,
}

impl Serialize for Label {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut object = serde_json::Map::new();
        object.insert("place".into(), Value::String(self.place.clone()));
        if let Some(items) = self.items {
            object.insert("items".into(), Value::from(items as i64));
        }
        if let Some(name) = &self.name {
            object.insert("name".into(), Value::String(name.clone()));
        }
        Value::Object(object).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Label {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Label, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Ok(match &value {
            Value::Object(_) => Label {
                place: text(value.get("place")),
                items: number(value.get("items")),
                name: value.get("name").filter(|v| !v.is_null()).map(|v| text(Some(v))),
                legacy: false,
            },
            _ => Label { legacy: true, ..Default::default() },
        })
    }
}

/// labelFor in main.js
pub fn label_for(targets: &[String], home: &str) -> Label {
    if targets.len() > 1 {
        return Label { place: "items".into(), items: Some(targets.len() as f64), ..Default::default() };
    }
    let target = targets.first().cloned().unwrap_or_default();
    if target == home {
        return Label { place: "home".into(), ..Default::default() };
    }
    let name = node_basename(&target);
    Label { place: "named".into(), name: Some(if name.is_empty() { target } else { name }), ..Default::default() }
}

/// path.basename: the last part of a path, trailing slashes aside
pub fn node_basename(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string()
}

/// Who wrote a YARA rule a match came from, as its licence asks to say
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Credit {
    #[serde(deserialize_with = "any_text")]
    pub author: String,
    #[serde(deserialize_with = "any_text")]
    pub licence: String,
    #[serde(deserialize_with = "any_text")]
    pub source: String,
}

/// One file a scan found
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Found {
    #[serde(deserialize_with = "any_text")]
    pub path: String,
    #[serde(deserialize_with = "any_text")]
    pub signature: String,
    pub credit: Option<Credit>,
    /// What was done with it: quarantined, deleted, declined, failed, clean
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
}

/// A scan started here, as the history keeps it, in the same file the Electron window wrote
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    #[serde(deserialize_with = "any_text")]
    pub id: String,
    /// home or pick
    #[serde(deserialize_with = "any_text")]
    pub kind: String,
    pub label: Label,
    #[serde(deserialize_with = "any_bool")]
    pub single: bool,
    pub targets: Vec<String>,
    /// Milliseconds since the epoch, as the file has always had it
    #[serde(serialize_with = "whole", deserialize_with = "any_number")]
    pub started: f64,
    #[serde(serialize_with = "whole", deserialize_with = "any_number")]
    pub seconds: f64,
    #[serde(serialize_with = "whole", deserialize_with = "any_number")]
    pub files: f64,
    #[serde(serialize_with = "whole", deserialize_with = "any_number")]
    pub errors: f64,
    pub signatures: Option<Signatures>,
    pub found: Vec<Found>,
    #[serde(deserialize_with = "any_bool")]
    pub stopped: bool,
    /// found, clean, stopped or failed
    #[serde(deserialize_with = "any_text")]
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not", deserialize_with = "any_bool")]
    pub dismissed: bool,
}

impl Entry {
    pub fn started_seconds(&self) -> f64 {
        self.started / 1000.0
    }
}

/// The history, with a scan from before the page put what was scanned into words of its own named again
pub fn parse_history(text: &str, home: &str) -> Vec<Entry> {
    let Ok(Value::Array(entries)) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    entries
        .into_iter()
        .filter_map(|entry| serde_json::from_value::<Entry>(entry).ok())
        .map(|mut entry| {
            if entry.label.legacy && !entry.targets.is_empty() {
                entry.label = label_for(&entry.targets, home);
            }
            entry
        })
        .collect()
}

pub fn load_history(home: &str) -> Vec<Entry> {
    std::fs::read_to_string(history_file()).map(|text| parse_history(&text, home)).unwrap_or_default()
}

/// Written whole and then moved into place, so that a crash halfway leaves the old list rather than half of
/// a new one. It names files and where they are, so it is this account's alone.
pub fn save_history(entries: &[Entry]) {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let file = history_file();
    let Some(folder) = file.parent() else { return };
    let _ = std::fs::create_dir_all(folder);
    let mut text = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut text, formatter);
    if entries.serialize(&mut serializer).is_err() {
        return;
    }
    let temporary = PathBuf::from(format!("{}.{}", file.display(), std::process::id()));
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)
        .and_then(|mut out| out.write_all(&text));
    // Nowhere to keep it: the scan still shows, and is forgotten when the window closes
    if written.is_ok() {
        let _ = std::fs::rename(&temporary, &file);
    } else {
        let _ = std::fs::remove_file(&temporary);
    }
}

/// An id for a scan: the time and a little chance, as main.js made them
pub fn scan_id() -> String {
    fn base36(mut n: u128) -> String {
        const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
        let mut out = Vec::new();
        loop {
            out.push(DIGITS[(n % 36) as usize]);
            n /= 36;
            if n == 0 {
                break;
            }
        }
        out.reverse();
        String::from_utf8(out).expect("ASCII")
    }
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let mut chance = (since.as_nanos() as u64) ^ ((std::process::id() as u64) << 32);
    // xorshift, for four letters nobody relies on
    for _ in 0..3 {
        chance ^= chance << 13;
        chance ^= chance >> 7;
        chance ^= chance << 17;
    }
    let tail: String = base36(chance as u128 % 36u128.pow(4) + 36u128.pow(4)).chars().skip(1).collect();
    format!("{}{}", base36(since.as_millis()), tail)
}

/// clamd reads its ExcludePath rules itself, and clamscan has to be handed the same list. A rule such as
/// ^/var/home/[^/]+/\.cache/ names what is inside the cache, and clamscan matches the directory.
pub fn exclusions_of(config: &str) -> Vec<String> {
    static RULE: OnceLock<Regex> = OnceLock::new();
    let rule = RULE.get_or_init(|| Regex::new(r"(?m)^ExcludePath\s+(\S+)\s*$").expect("a valid expression"));
    rule.captures_iter(config)
        .map(|found| {
            let path = &found[1];
            let path = path.strip_suffix('/').map(|p| format!("{p}(/|$)")).unwrap_or_else(|| path.to_string());
            format!("--exclude-dir={path}")
        })
        .collect()
}

pub fn exclusions() -> Vec<String> {
    std::fs::read_to_string(CLAMD_CONFIG).map(|text| exclusions_of(&text)).unwrap_or_default()
}

/// One line per file: "<path>: OK", or "<path>: <signature> FOUND". None for what is not about a file.
pub fn parse_result(line: &str) -> Option<(String, Option<String>)> {
    static RESULT: OnceLock<Regex> = OnceLock::new();
    let result = RESULT.get_or_init(|| Regex::new(r"^(.*): (?:(\S+) FOUND|OK)$").expect("a valid expression"));
    let found = result.captures(line)?;
    Some((found[1].to_string(), found.get(2).map(|m| m.as_str().to_string())))
}

/// The credit of a YARA rule's match, from the credits file's text
pub fn credit_in(signature: &str, credits: &str) -> Option<Credit> {
    let rule = signature.strip_prefix("YARA.")?.strip_suffix(".UNOFFICIAL")?;
    if rule.is_empty() || !rule.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let line = credits.split('\n').find(|row| row.starts_with(&format!("{rule}\t")))?;
    let fields: Vec<&str> = line.split('\t').collect();
    let field = |at: usize| fields.get(at).map(|s| s.to_string()).unwrap_or_default();
    Some(Credit { source: field(1), author: field(2), licence: field(3) })
}

pub fn yara_credit(signature: &str) -> Option<Credit> {
    if !signature.starts_with("YARA.") {
        return None;
    }
    credit_in(signature, &std::fs::read_to_string(YARA_CREDITS).ok()?)
}

/// A file's resolved path, /home being /var/home
pub fn real(file: &str) -> Option<String> {
    std::fs::canonicalize(file).ok().map(|p| p.to_string_lossy().into_owned())
}

/// A file's name can hold a newline, and then its own line "<anything>: <signature> FOUND": only a file that
/// is there, in what was scanned, is a finding (compared resolved)
pub fn within(file: &str, targets: &[String]) -> bool {
    let Some(file) = real(file) else { return false };
    targets.iter().any(|target| {
        real(target).is_some_and(|place| {
            let folder = if place.ends_with('/') { place.clone() } else { format!("{place}/") };
            file == place || file.starts_with(&folder)
        })
    })
}

/// Files and folders the page names, dropped onto the window: only absolute paths that are there
pub fn existing(paths: &[String]) -> Vec<String> {
    paths.iter().filter(|path| path.starts_with('/') && Path::new(path.as_str()).exists()).cloned().collect()
}

/// What this machine does with what a scan finds: report, quarantine or delete
pub async fn detection() -> String {
    let mode = run_for(SECURITY_CONFIG, &["ON_DETECTION"], 10).await.stdout.trim().to_string();
    if mode == "quarantine" || mode == "delete" { mode } else { "report".into() }
}

/// What amethystora-quarantine says it did with each file: "<outcome>\t<signature>\t<path>" a line
pub fn parse_outcomes(stdout: &str) -> HashMap<String, String> {
    let mut outcomes = HashMap::new();
    for line in stdout.split('\n') {
        let fields: Vec<&str> = line.split('\t').collect();
        if let Some(file) = fields.get(2).filter(|f| !f.is_empty()) {
            outcomes.insert(file.to_string(), fields[0].to_string());
        }
    }
    outcomes
}

/// The files are the user's and the quarantine is root's, so this is the one step that asks for a password,
/// once for the whole scan. The helper scans each file again before it does anything, and says what
/// happened to each: quarantined, deleted, clean, failed. Declining the password leaves them.
pub async fn act(entry: &mut Entry) {
    let mode = detection().await;
    entry.detection = Some(mode.clone());
    if mode == "report" || entry.found.is_empty() {
        return;
    }
    let mut args = vec!["act".to_string()];
    args.extend(entry.found.iter().map(|item| item.path.clone()));
    let mut argv: Vec<&str> = vec![QUARANTINE];
    argv.extend(args.iter().map(String::as_str));
    let output = run_for("pkexec", &argv, 600).await;
    let outcomes = parse_outcomes(&output.stdout);
    // pkexec: 126 when the password was not given, 127 when it was refused
    let nobody = if output.code == 126 || output.code == 127 { "declined" } else { "failed" };
    for item in entry.found.iter_mut() {
        item.outcome = Some(outcomes.get(&item.path).cloned().unwrap_or_else(|| nobody.to_string()));
    }
}

// --- Network protection -----------------------------------------------------------------------------------------

/// One rule that went off, for one pair of addresses, as network.json keeps it
#[derive(Debug, Clone, Default)]
pub struct NetworkEvent {
    pub time: f64,
    pub action: String,
    /// The rule's number, as text
    pub sid: String,
    pub sid_number: Option<f64>,
    pub signature: String,
    pub src: String,
    pub dest: String,
    /// The port, when there is one
    pub port: String,
    pub host: String,
    pub count: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Network {
    /// A machine where nothing has gone off yet has no file; one this account may not read says so
    pub readable: bool,
    pub events: Vec<NetworkEvent>,
    pub allowed: Vec<f64>,
    pub running: bool,
    /// When the rules were last built, in seconds
    pub rules: Option<f64>,
}

pub fn parse_network_events(list: &[Value]) -> Vec<NetworkEvent> {
    list.iter()
        .map(|event| NetworkEvent {
            time: number(event.get("time")).unwrap_or(f64::NAN),
            action: text(event.get("action")),
            sid: js_string(event.get("sid")),
            sid_number: number(event.get("sid")),
            signature: text(event.get("signature")),
            src: text(event.get("src")),
            dest: text(event.get("dest")),
            port: if truthy(event.get("port")) { js_string(event.get("port")) } else { String::new() },
            host: if truthy(event.get("host")) { text(event.get("host")) } else { String::new() },
            count: number(event.get("count")).unwrap_or(0.0),
        })
        .collect()
}

/// The rules left out on this machine, one number a line
pub fn parse_allowed(text: &str) -> Vec<f64> {
    text.split('\n').filter(|line| !line.is_empty() && line.chars().all(|c| c.is_ascii_digit())).filter_map(|line| line.parse().ok()).collect()
}

/// What it blocked and noticed, newest first, and what the page shows beside that: whether Suricata is
/// running, and when its rules were last built
pub async fn network() -> Network {
    let events = read_json(NETWORK_EVENTS);
    let readable = !matches!(events, Json::Denied);
    let allowed = std::fs::read_to_string(IPS_ALLOWED).map(|text| parse_allowed(&text)).unwrap_or_default();
    let output = process::run("systemctl", &["is-active", IPS_UNIT]).await;
    let rules = match read_json(WATCH_STATUS) {
        Json::Value(status) => number(status.get("network").and_then(|n| n.get("rules"))),
        _ => None,
    };
    Network {
        readable,
        events: events.array().map(|list| parse_network_events(&list)).unwrap_or_default(),
        allowed,
        running: output.stdout.trim() == "active",
        rules,
    }
}

/// What came of allowing a rule
pub enum RuleOutcome {
    Allowed,
    Declined,
    Error(String),
}

/// Leaves one rule out on this machine, which asks for an administrator's password. Only a rule that
/// actually went off here, as network.json says, never one the page names out of nowhere.
pub async fn allow_rule(rule: &str) -> RuleOutcome {
    let digits = !rule.is_empty() && rule.len() <= 10 && rule.chars().all(|c| c.is_ascii_digit());
    let known = read_json(NETWORK_EVENTS).array().is_some_and(|list| list.iter().any(|event| js_string(event.get("sid")) == rule));
    if !digits || !known {
        return RuleOutcome::Error(t!("That rule is not one network protection used on this machine."));
    }
    let output = run_for("pkexec", &[IPS, "allow", rule], 600).await;
    match output.code {
        0 => RuleOutcome::Allowed,
        // pkexec: 126 when the password was not given, 127 when it was refused
        126 | 127 => RuleOutcome::Declined,
        code => {
            let said = if output.stderr.is_empty() { &output.stdout } else { &output.stderr };
            let last = said.trim().split('\n').next_back().unwrap_or("").to_string();
            RuleOutcome::Error(if last.is_empty() { t!("The rule could not be allowed (status {code}).", code = code.to_string()) } else { last })
        }
    }
}

// --- What the security watcher found ------------------------------------------------------------------------------

/// One finding of the watcher's, which it has made plain text already
#[derive(Debug, Clone, Default)]
pub struct Event {
    pub id: String,
    pub time: f64,
    /// critical and normal were said out loud, quiet only recorded
    pub level: String,
    pub key: String,
    pub account: String,
    pub tactic: String,
    pub text: String,
    /// It can be allowed (an entry for `ame security allow`)
    pub allow: bool,
    /// The snapshot from before it, which Backups opens
    pub at: bool,
    /// It is about this account
    pub mine: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Events {
    pub readable: bool,
    pub events: Vec<Event>,
    /// Whether an event may be handed to the AI agent
    pub triage: bool,
}

pub fn parse_events(list: &[Value], me: &str) -> Vec<Event> {
    list.iter()
        .map(|event| Event {
            id: js_string(event.get("id")),
            time: number(event.get("time")).unwrap_or(f64::NAN),
            level: text(event.get("level")),
            key: text(event.get("key")),
            account: text(event.get("account")),
            tactic: text(event.get("tactic")),
            text: text(event.get("text")),
            allow: truthy(event.get("allow")),
            at: truthy(event.get("at")),
            mine: event.get("account").and_then(Value::as_str) == Some(me),
        })
        .collect()
}

/// What the watcher found: each finding's time, level, the part that found it, its account and its sentence,
/// and whether it is about this account. One this account may not read says so.
pub async fn events() -> Events {
    let events = read_json(WATCH_EVENTS);
    let readable = !matches!(events, Json::Denied);
    let me = username();
    Events { readable, events: events.array().map(|list| parse_events(&list, &me)).unwrap_or_default(), triage: triage_on().await }
}

/// Whether an event may be handed to the AI agent: TRIAGE on, which sends the event to the agent's maker and
/// was chosen after its explanation, and the agentic features on for this account
pub async fn triage_on() -> bool {
    let setting = glib::spawn_future_local(run_for(SECURITY_CONFIG, &["--is", "TRIAGE", "on"], 10));
    let agent = glib::spawn_future_local(run_for(AGENT, &["status"], 10));
    let setting = setting.await.map(|o| o.code == 0).unwrap_or(false);
    let agent = agent.await.map(|o| o.code == 0).unwrap_or(false);
    setting && agent
}

/// One event of the watcher's, by the id the page sent: never anything else the page says
pub fn find_event(id: &str) -> Option<Value> {
    read_json(WATCH_EVENTS).array()?.into_iter().find(|event| event.get("id").and_then(Value::as_str) == Some(id))
}

/// The facts of an event the agent is handed, as JSON.stringify wrote them, in this order
pub fn triage_facts(event: &Value) -> String {
    let facts: Vec<String> = ["time", "level", "key", "account", "tactic", "text"]
        .iter()
        .filter_map(|key| {
            let value = event.get(*key)?;
            Some(format!("{}:{}", serde_json::to_string(key).ok()?, serde_json::to_string(value).ok()?))
        })
        .collect();
    format!("{{{}}}", facts.join(","))
}

/// An event handed to the agent, which explains it in a terminal and changes nothing: the facts the watcher
/// wrote, looked up by the id the page sent, which amethystora-agent quotes as data
pub async fn triage(id: &str) {
    let Some(event) = find_event(id) else { return };
    if triage_on().await {
        process::launch(AGENT, &["triage", &triage_facts(&event)]);
    }
}

/// A snapshot's name, as Backups takes it
pub fn snapshot_name(at: &str) -> bool {
    let bytes = at.as_bytes();
    bytes.len() == 15
        && bytes.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            10 => *b == b'_',
            _ => b.is_ascii_digit(),
        })
}

/// The snapshot from before an event, opened in Backups: only for an event about this account, whose home is
/// the one Backups shows, and only a snapshot's name
pub fn open_backups(id: &str) {
    let Some(event) = find_event(id) else { return };
    let at = text(event.get("at"));
    if event.get("account").and_then(Value::as_str) == Some(username().as_str()) && snapshot_name(&at) {
        process::launch("amethystora-backups", &[&format!("--at={at}")]);
    }
}

/// A key the allowlist helper could keep a list for: /^[a-z][a-z-]*$/
fn feature_name(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_lowercase() || c == '-')
}

/// What the event names stops being said: `ame security allow KEY ENTRY` in a terminal, which shows the entry
/// and asks first, then for the password. The entry is the one the watcher wrote into the event, and its key
/// one the allowlist helper keeps a list for.
pub async fn allow_event(id: &str) {
    let Some(event) = find_event(id) else { return };
    let Some(entry) = event.get("allow").and_then(Value::as_str).filter(|entry| !entry.is_empty()).map(str::to_string) else { return };
    let key = js_string(event.get("key"));
    if !feature_name(&key) {
        return;
    }
    let features = run_for(ALLOW, &["--features"], 10).await.stdout;
    if features.split('\n').any(|feature| feature == key) {
        process::in_terminal(&[AME, "security", "allow", &key, &entry]);
    }
}

// --- Installed outside Flatpak -------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct Container {
    pub name: String,
    pub status: String,
    pub template: String,
    /// It has a home folder of its own
    pub home: bool,
    pub exported_apps: Vec<String>,
    pub exported_bins: Vec<String>,
    pub packages: Vec<String>,
    pub files: Vec<String>,
    pub aur_packages: Vec<String>,
    pub aur_pending: f64,
    pub created: Option<f64>,
    pub last_upgrade: Option<f64>,
    pub last_upgrade_result: String,
}

#[derive(Debug, Clone, Default)]
pub struct Inventory {
    pub containers: Vec<Container>,
    pub aur: bool,
    pub error: Option<String>,
}

pub fn parse_containers(list: &[Value]) -> Vec<Container> {
    let names = |value: Option<&Value>| value.and_then(Value::as_array).map(|list| list.iter().map(|item| text(item.get("name"))).collect()).unwrap_or_default();
    list.iter()
        .map(|container| Container {
            name: text(container.get("name")),
            status: text(container.get("status")),
            template: text(container.get("template")),
            home: truthy(container.get("home")),
            exported_apps: names(container.get("exported_apps")),
            exported_bins: strings(container.get("exported_bins")),
            packages: strings(container.get("packages")),
            files: names(container.get("files")),
            aur_packages: strings(container.get("aur_packages")),
            aur_pending: number(container.get("aur_pending")).unwrap_or(0.0),
            created: number(container.get("created")).filter(|t| *t != 0.0),
            last_upgrade: number(container.get("last_upgrade")).filter(|t| *t != 0.0),
            last_upgrade_result: text(container.get("last_upgrade_result")),
        })
        .collect()
}

pub async fn inventory() -> Inventory {
    let output = run_for(PKG, &["containers", "list", "--json"], 60).await;
    match serde_json::from_str::<Value>(&output.stdout) {
        Ok(list) => Inventory { containers: list.as_array().map(|l| parse_containers(l)).unwrap_or_default(), aur: aur_on(), error: None },
        Err(_) => Inventory { containers: Vec::new(), aur: aur_on(), error: Some(stopped("amethystora-pkg", &output)) },
    }
}

// --- App permissions ---------------------------------------------------------------------------------------------

/// One way out of its sandbox an app has, in the words amethystora-app-permissions gave it
#[derive(Debug, Clone, Default)]
pub struct Reach {
    pub id: String,
    pub text: String,
    /// Granted on this machine rather than asked for by the app
    pub granted: bool,
}

#[derive(Debug, Clone, Default)]
pub struct App {
    pub app: String,
    pub name: String,
    /// This account granted it something of its own
    pub user_overrides: bool,
    pub reaches: Vec<Reach>,
    pub network: bool,
    pub network_blocked: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Permissions {
    pub apps: Vec<App>,
    /// The account's plugins, as `ame plugin list` gives them (plans/done/PLAN-plugins.md: Security keeps the reach)
    pub plugins: Vec<Plugin>,
    pub error: Option<String>,
}

/// One plugin the account added: what it was granted, as the store holds it, never what it says of itself
#[derive(Debug, Clone, Default)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    /// Normal-form permissions (`network=api.github.com`, `files=~/Documents:ro`, `notify`...)
    pub granted: Vec<String>,
    /// All of its network is blocked
    pub blocked: bool,
    /// Its files are still the ones approved
    pub verified: bool,
    /// The account turned it on
    pub enabled: bool,
}

pub const PLUGIN_HOST: &str = "/usr/libexec/amethystora-plugin-host";
pub const PLUGIN_ROOT: &str = "/usr/libexec/amethystora-plugin-root";

pub fn parse_plugins(list: &[Value]) -> Vec<Plugin> {
    list.iter()
        .filter(|p| !truthy(p.get("draft")))
        .map(|p| Plugin {
            id: text(p.get("id")),
            name: text(p.get("name")),
            granted: strings(p.get("granted")),
            blocked: truthy(p.get("blocked")),
            verified: truthy(p.get("verified")),
            enabled: truthy(p.get("enabled")),
        })
        .filter(|p| valid_plugin_id(&p.id))
        .collect()
}

/// A plugin id as the host prints it: what is passed back to the helper is held to this first
fn valid_plugin_id(id: &str) -> bool {
    (1..=128).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-') && !id.starts_with(['.', '-'])
}

async fn plugins() -> Vec<Plugin> {
    if !std::path::Path::new(PLUGIN_HOST).exists() {
        return Vec::new();
    }
    let output = run_for(PLUGIN_HOST, &["list", "--json"], 20).await;
    serde_json::from_str::<Value>(&output.stdout).ok().and_then(|v| v.as_array().map(|l| parse_plugins(l))).unwrap_or_default()
}

/// Takes from one plugin everything it was given (`revoke` with no permission), or blocks all it sends (`block`): both only take
/// away, so the person at the machine needs no password for them (50-amethystora-plugins.rules). Only for a plugin the list just
/// read says has something to take
pub async fn plugin_take(list: &[Plugin], id: &str, block: bool) -> Result<(), String> {
    let Some(entry) = list.iter().find(|p| p.id == id && if block { has_network(p) && !p.blocked } else { !p.granted.is_empty() }) else {
        return Err(t!("That plugin is not one the list can change."));
    };
    let output = process::run("pkexec", &[PLUGIN_ROOT, if block { "block" } else { "revoke" }, &entry.id]).await;
    if output.code == 0 { Ok(()) } else { Err(stopped("amethystora-plugin-root", &output)) }
}

pub fn has_network(plugin: &Plugin) -> bool {
    plugin.granted.iter().any(|p| p.starts_with("network="))
}

pub fn parse_apps(list: &[Value]) -> Vec<App> {
    list.iter()
        .map(|app| App {
            app: text(app.get("app")),
            name: text(app.get("name")),
            user_overrides: truthy(app.get("user_overrides")),
            reaches: app
                .get("reaches")
                .and_then(Value::as_array)
                .map(|list| list.iter().map(|reach| Reach { id: text(reach.get("id")), text: text(reach.get("text")), granted: truthy(reach.get("granted")) }).collect())
                .unwrap_or_default(),
            network: truthy(app.get("network")),
            network_blocked: truthy(app.get("network_blocked")),
        })
        .collect()
}

/// Run as the user, whose overrides are the ones that apply
pub async fn permissions() -> Permissions {
    let plugins = plugins().await;
    let output = run_for(PERMISSIONS, &["--json"], 120).await;
    match serde_json::from_str::<Value>(&output.stdout) {
        Ok(list) => Permissions { apps: list.as_array().map(|l| parse_apps(l)).unwrap_or_default(), plugins, error: None },
        Err(_) => Permissions { apps: Vec::new(), plugins, error: Some(stopped("amethystora-app-permissions", &output)) },
    }
}

/// Takes back everything this account granted one app: its own overrides, never the machine's or the image's.
/// Only for an app the list just read says has some.
pub async fn reset_permissions(list: &[App], id: &str) -> Result<(), String> {
    let Some(entry) = list.iter().find(|item| item.app == id && item.user_overrides) else {
        return Err(t!("That app has nothing granted by this account to take back."));
    };
    let output = process::run("flatpak", &["override", "--user", "--reset", &entry.app]).await;
    if output.code == 0 { Ok(()) } else { Err(stopped("flatpak override", &output)) }
}

/// Takes the network away from one app, or gives back what this account took, as an override of the
/// account's own (Flatpak's own --unshare, from the app's next start). Only for an app the list just read.
pub async fn set_network(list: &[App], id: &str, allow: bool) -> Result<(), String> {
    let Some(entry) = list.iter().find(|item| item.app == id && if allow { item.network_blocked } else { item.network }) else {
        return Err(t!("That app is not one the list can change."));
    };
    let output = process::run("flatpak", &["override", "--user", if allow { "--share=network" } else { "--unshare=network" }, &entry.app]).await;
    if output.code == 0 { Ok(()) } else { Err(stopped("flatpak override", &output)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_plugins_and_leaves_drafts_and_odd_ids_out() {
        let list: Value = serde_json::from_str(
            r#"[{"id":"dev.example.ci","name":"CI","granted":["network=api.github.com","notify"],"blocked":false,"verified":true,"enabled":true},
            {"id":"dev.example.draft","name":"Draft","granted":[],"draft":true},
            {"id":"-x; rm","name":"Odd","granted":["notify"]}]"#,
        )
        .unwrap();
        let plugins = parse_plugins(list.as_array().unwrap());
        assert_eq!(plugins.len(), 1);
        assert!(has_network(&plugins[0]) && plugins[0].verified && plugins[0].enabled);
    }

    #[test]
    fn reads_the_report() {
        let report = parse_report(
            r#"{"image":"ghcr.io/arsslensoft/amethystora:stable","settings":{"on_detection":"quarantine","realtime":"off","network":"watch","virus_scan":"weekly","profile":"default"},
            "checks":[{"group":"updates","id":"signed-updates","state":"ok","title":"Signed updates","text":"Updates are checked","details":["a","Run:"],"command":"ame security signed-updates","accepted":null,"switched_off":false},
            {"group":"optional","id":"usb-protection","state":"off","title":"USB","text":"x","details":[],"command":null,"accepted":"check","switched_off":true},
            {"group":"sends","id":"sends-countme","state":"off","title":"Count","text":"x","details":[],"command":"ame security countme","accepted":null,"switched_off":false}]}"#,
        )
        .unwrap();
        assert_eq!(report.image, "ghcr.io/arsslensoft/amethystora:stable");
        assert_eq!(report.settings.on_detection, "quarantine");
        assert_eq!(report.settings.network, "watch");
        assert_eq!(report.checks.len(), 3);
        assert_eq!(report.checks[0].command.as_deref(), Some("ame security signed-updates"));
        assert_eq!(report.checks[0].details, vec!["a", "Run:"]);
        assert!(!report.checks[0].accepted);
        assert!(report.checks[1].accepted && report.checks[1].switched_off);
        assert_eq!(report.checks[1].command, None);
        assert!(report.attention_check("usb-protection").is_none());
        // What the machine sends is information whatever its state: switched off, it is no problem
        assert!(report.attention_check("sends-countme").is_none());
        assert!(parse_report(r#"{"image":"x"}"#).is_none());
        assert!(parse_report("not json").is_none());
    }

    #[test]
    fn reads_the_switches() {
        let (profile, settings) = parse_settings(
            r#"{"profile":"light","settings":[{"key":"VIRUS_SCAN","setting":"virus-scan","value":"weekly","title":"Virus scanner","text":"ClamAV.","group":"scans","allowed":["off","manual","weekly"],"applies":["now"],"sends":"downloads","explain":null,"cost":{"memory":104857600}},
            {"key":"TRIAGE","setting":"triage","value":"off","title":"T","text":"x","group":"away","allowed":["off","on"],"explain":"Each event goes","cost":null}]}"#,
        )
        .unwrap();
        assert_eq!(profile.as_deref(), Some("light"));
        assert_eq!(settings[0].allowed, vec!["off", "manual", "weekly"]);
        assert_eq!(settings[0].cost.as_ref().unwrap().memory, Some(104857600.0));
        assert_eq!(settings[0].explain, None);
        assert_eq!(settings[1].explain.as_deref(), Some("Each event goes"));
        assert!(settings[1].cost.is_none());
        let hardening = parse_hardening(
            r#"[{"name":"kernel","state":"on","title":"Kernel settings","text":"t","group":"system","command":"ame security kernel","fixed":false,"default":"on","level":"standard","levels":["off","standard","strict"],"arguments":null},
            {"name":"kernel-args","state":"on","title":"K","text":"t","fixed":true,"default":"on","level":null,"levels":null,"arguments":[{"argument":"init_on_alloc=1","state":"on","default":"on"}]}]"#,
        )
        .unwrap();
        assert_eq!(hardening[0].levels.as_deref(), Some(&["off".to_string(), "standard".into(), "strict".into()][..]));
        assert_eq!(hardening[0].level, "standard");
        assert_eq!(hardening[0].group, "system");
        assert!(hardening[1].fixed);
        assert_eq!(hardening[1].arguments.as_ref().unwrap()[0].argument, "init_on_alloc=1");
        assert!(parse_hardening("{}").is_none());
    }

    #[test]
    fn changes_only_what_the_lists_give() {
        let switches = Switches {
            profile: "custom".into(),
            settings: vec![Setting { key: "VIRUS_SCAN".into(), setting: "virus-scan".into(), allowed: vec!["off".into(), "weekly".into()], ..Default::default() }],
            hardening: vec![
                Hardening { name: "kernel".into(), levels: Some(vec!["off".into(), "standard".into(), "strict".into()]), ..Default::default() },
                Hardening { name: "kernel-args".into(), arguments: Some(vec![Argument { argument: "init_on_free=1".into(), state: "off".into() }]), ..Default::default() },
                Hardening { name: "boot-password".into(), fixed: true, ..Default::default() },
            ],
            error: None,
        };
        let s = Some(&switches);
        assert_eq!(change_command(s, Kind::Setting, "VIRUS_SCAN", "off", ""), Some(vec![AME.into(), "security".into(), "settings".into(), "virus-scan".into(), "off".into()]));
        assert_eq!(change_command(s, Kind::Setting, "VIRUS_SCAN", "", "").unwrap().len(), 4);
        assert_eq!(change_command(s, Kind::Setting, "VIRUS_SCAN", "rm -rf", ""), None);
        assert_eq!(change_command(s, Kind::Setting, "OTHER", "off", ""), None);
        assert_eq!(change_command(s, Kind::Hardening, "kernel", "strict", "").unwrap().last().unwrap(), "strict");
        assert_eq!(change_command(s, Kind::Hardening, "kernel", "loose", ""), None);
        assert_eq!(
            change_command(s, Kind::Hardening, "kernel-args", "on", "init_on_free=1"),
            Some(vec![AME.into(), "security".into(), "kernel-args".into(), "init_on_free=1".into(), "on".into()])
        );
        assert_eq!(change_command(s, Kind::Hardening, "kernel-args", "on", "quiet"), None);
        assert_eq!(change_command(s, Kind::Hardening, "boot-password", "on", ""), None);
        assert_eq!(change_command(s, Kind::Profile, "balanced", "", "").unwrap().last().unwrap(), "balanced");
        assert_eq!(change_command(s, Kind::Profile, "custom", "", ""), None);
        assert_eq!(change_command(None, Kind::Setting, "VIRUS_SCAN", "off", ""), None);
        assert_eq!(duress_command("setup-password"), Some(vec![AME.into(), "security".into(), "duress".into(), "setup-password".into()]));
        assert_eq!(duress_command("forget"), None);
        assert_eq!(duress_command(""), None);
    }

    #[test]
    fn reads_scan_lines() {
        assert_eq!(parse_result("/home/a/b.txt: OK"), Some(("/home/a/b.txt".into(), None)));
        assert_eq!(parse_result("/home/a: b.exe: Win.Test.EICAR_HDB-1 FOUND"), Some(("/home/a: b.exe".into(), Some("Win.Test.EICAR_HDB-1".into()))));
        assert_eq!(parse_result("/home/a/c: Empty file"), None);
        assert_eq!(parse_result("LibClamAV Warning: x"), None);
        assert_eq!(
            exclusions_of("LogTime yes\nExcludePath ^/var/home/[^/]+/\\.cache/\nExcludePath ^/proc\n#ExcludePath x\n"),
            vec!["--exclude-dir=^/var/home/[^/]+/\\.cache(/|$)".to_string(), "--exclude-dir=^/proc".into()]
        );
        let credits = "Linux_Trojan_Mirai\telastic\tElastic Security\tElastic License 2.0\nOther\ts\ta\tl\n";
        assert_eq!(
            credit_in("YARA.Linux_Trojan_Mirai.UNOFFICIAL", credits),
            Some(Credit { author: "Elastic Security".into(), licence: "Elastic License 2.0".into(), source: "elastic".into() })
        );
        assert_eq!(credit_in("YARA.Missing.UNOFFICIAL", credits), None);
        assert_eq!(credit_in("Win.Test.EICAR_HDB-1", credits), None);
        assert_eq!(credit_in("YARA.a-b.UNOFFICIAL", credits), None);
        let outcomes = parse_outcomes("quarantined\tEicar\t/var/home/a/x\nfailed\t-\t/var/home/a/y\n\n");
        assert_eq!(outcomes.get("/var/home/a/x").map(String::as_str), Some("quarantined"));
        assert_eq!(outcomes.get("/var/home/a/y").map(String::as_str), Some("failed"));
        assert_eq!(outcomes.len(), 2);
    }

    #[test]
    fn labels_and_names() {
        let home = "/var/home/ana";
        assert_eq!(label_for(&[home.to_string()], home).place, "home");
        let items = label_for(&["/a".into(), "/b".into()], home);
        assert_eq!((items.place.as_str(), items.items), ("items", Some(2.0)));
        assert_eq!(label_for(&["/var/home/ana/Downloads/".into()], home).name.as_deref(), Some("Downloads"));
        assert_eq!(label_for(&["/".into()], home).name.as_deref(), Some("/"));
        assert_eq!(node_basename("/a/b//"), "b");
        assert!(scan_id().len() >= 12);
    }

    #[test]
    fn history_round_trips() {
        let text = r#"[{"id":"abc","kind":"home","label":"Your home folder","single":false,"targets":["/var/home/ana"],"started":1760000000000,"seconds":42,"files":12,"errors":0,
            "signatures":{"days":1,"stale":false},"found":[{"path":"/var/home/ana/x","signature":"Eicar","credit":null}],"stopped":false,"status":"found","detection":"report"},
            {"broken":true,"label":{"place":"named","name":"x"},"targets":["/x"]}, 3]"#;
        let entries = parse_history(text, "/var/home/ana");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].label.place, "home");
        assert_eq!(entries[0].started_seconds(), 1760000000.0);
        assert_eq!(entries[0].found[0].credit, None);
        assert_eq!(entries[1].label.name.as_deref(), Some("x"));
        let written = serde_json::to_string(&entries[0]).unwrap();
        assert!(written.contains(r#""started":1760000000000"#), "{written}");
        assert!(written.contains(r#""label":{"place":"home"}"#), "{written}");
        assert!(!written.contains("dismissed"));
        assert!(!written.contains("outcome"));
        let again = parse_history(&format!("[{written}]"), "/var/home/ana");
        assert_eq!(again[0].found, entries[0].found);
    }

    #[test]
    fn reads_systemd() {
        let machine = parse_machine(
            "ActiveState=inactive\nExecMainStatus=1\nInactiveExitTimestamp=@1760000000\nInactiveEnterTimestamp=@1760000600\n",
            "LastTriggerUSec=@1760000000\n",
            true,
        );
        assert!(!machine.running && machine.found);
        assert_eq!((machine.started, machine.finished, machine.status), (Some(1760000000.0), Some(1760000600.0), Some(1)));
        let fresh = parse_machine("ActiveState=activating\nExecMainStatus=0\nInactiveExitTimestamp=\nInactiveEnterTimestamp=\n", "LastTriggerUSec=@1759000000\n", false);
        assert!(fresh.running);
        assert_eq!((fresh.finished, fresh.status), (Some(1759000000.0), None));
        let never = parse_machine("ActiveState=inactive\n", "LastTriggerUSec=n/a\n", false);
        assert_eq!(never.finished, None);
    }

    #[test]
    fn reads_network_and_events() {
        let list: Vec<Value> = serde_json::from_str(
            r#"[{"time":1760000000,"action":"blocked","sid":2024897,"signature":"ET MALWARE x","src":"10.0.0.2","dest":"1.2.3.4","port":443,"host":"bad.example","count":3},
            {"time":1760000100,"action":"allowed","sid":"7","signature":"y","src":"a","dest":"b","port":0,"host":null}]"#,
        )
        .unwrap();
        let events = parse_network_events(&list);
        assert_eq!((events[0].sid.as_str(), events[0].port.as_str(), events[0].host.as_str()), ("2024897", "443", "bad.example"));
        assert_eq!((events[1].sid.as_str(), events[1].port.as_str(), events[1].host.as_str()), ("7", "", ""));
        assert_eq!(parse_allowed("2024897\nx\n12a\n\n7\n"), vec![2024897.0, 7.0]);
        let list: Vec<Value> = serde_json::from_str(
            r#"[{"id":"1760-0","time":1760000000,"level":"critical","key":"persistence","account":"ana","tactic":"Persistence","text":"t","allow":"~/.config/autostart/x.desktop","at":"2026-10-16_0900"},
            {"id":"1760-1","time":1760000000,"level":"quiet","key":"journal","account":"-","tactic":"-","text":"u"}]"#,
        )
        .unwrap();
        let events = parse_events(&list, "ana");
        assert!(events[0].mine && events[0].allow && events[0].at);
        assert!(!events[1].mine && !events[1].allow);
        assert_eq!(triage_facts(&list[0]), r#"{"time":1760000000,"level":"critical","key":"persistence","account":"ana","tactic":"Persistence","text":"t"}"#);
        assert!(snapshot_name("2026-10-16_0900"));
        assert!(!snapshot_name("2026-10-16_09000"));
        assert!(!snapshot_name("../2026-10-16"));
        assert!(feature_name("persistence") && feature_name("user-paths"));
        assert!(!feature_name("Persistence") && !feature_name("-x") && !feature_name(""));
    }

    #[test]
    fn reads_containers_and_apps() {
        let list: Vec<Value> = serde_json::from_str(
            r#"[{"name":"debian","status":"running","template":"debian","home":null,"exported_apps":[{"name":"GIMP (on debian)","file":"/x"}],"exported_bins":["/var/home/a/.local/bin/htop"],
            "packages":["vim","yay"],"files":[{"name":"x.deb"}],"aur_packages":["yay"],"aur_pending":2,"created":1760000000,"last_upgrade":null,"last_upgrade_result":"failed"}]"#,
        )
        .unwrap();
        let containers = parse_containers(&list);
        assert_eq!(containers[0].exported_apps, vec!["GIMP (on debian)"]);
        assert_eq!(containers[0].files, vec!["x.deb"]);
        assert_eq!((containers[0].aur_pending, containers[0].last_upgrade, containers[0].home), (2.0, None, false));
        let list: Vec<Value> = serde_json::from_str(
            r#"[{"app":"com.valvesoftware.Steam","name":"Steam","installation":"system","user_overrides":true,"reaches":[{"id":"x11","text":"Use X11","granted":true}],"network":true,"network_blocked":false}]"#,
        )
        .unwrap();
        let apps = parse_apps(&list);
        assert_eq!(apps[0].reaches[0].id, "x11");
        assert!(apps[0].user_overrides && apps[0].network && !apps[0].network_blocked);
    }

    #[test]
    fn javascript_values() {
        assert_eq!(js_string(Some(&serde_json::json!(2024897))), "2024897");
        assert_eq!(js_string(Some(&serde_json::json!(1.5))), "1.5");
        assert_eq!(js_string(None), "undefined");
        assert_eq!(text(Some(&Value::Null)), "");
        assert!(!truthy(Some(&serde_json::json!(0))) && truthy(Some(&serde_json::json!("x"))) && !truthy(Some(&serde_json::json!(""))));
    }
}
