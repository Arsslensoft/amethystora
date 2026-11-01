//! What Amethystora Updates knows of the machine, and the update it follows: what main.js did for the
//! Electron window.
//!
//! "Update now" starts uupd-manual.service: uupd, the updater the automatic updates run from uupd.timer,
//! without the hardware checks that hold those back on a busy machine. It updates what `ame update` does,
//! the system image, the Flatpaks of the machine and of everyone signed in, and Homebrew, and the polkit
//! rule the uupd package ships lets it be started without a password. An update that is already running,
//! the automatic one included, is followed instead of being started a second time.
//!
//! What the window shows while it runs is uupd's own JSON log, read from the journal, which Fedora opens to
//! the members of wheel. For anyone else the window still knows from systemd whether an update is running
//! and how it ended.
//!
//! The containers amethystora-pkg manages are each account's own, and uupd's own module for containers
//! stays off (/etc/uupd/config.json). Once an update started here has finished, the window starts the
//! account's amethystora-pkg-upgrade.service, which upgrades them, and follows the line of JSON it logs for
//! each one. How that goes shows on the Containers step and nowhere else: a container that did not upgrade
//! never makes the system's update a failed one.
//!
//! An update that leaves the kernel, its initramfs and its arguments as they are is finished with a soft
//! reboot: only the system's programs start again, not the computer. amethystora-soft-reboot says when, has the
//! update mounted for it and asks logind for the restart, which logind then makes one. GNOME's own restart
//! dialog, which every other restart here goes through, cannot do that, so the window asks first itself, and
//! leaves the restart to GNOME's dialog when an app asks not to end the session, which only that dialog lets
//! it say.
//!
//! A machine started from an older version than the one it starts by default, which is what picking the
//! second entry in the boot menu does, is said so, with the choice of keeping that version, which
//! `bootc rollback` does after an administrator's password, or restarting into the newest.
//!
//! Firmware is not uupd's: the window asks fwupd which devices have an update on LVFS, which needs no
//! password, and installing one is `fwupdmgr update`, in a terminal, where fwupd says what it is about to
//! do and asks before it restarts the machine. fwupd-refresh.timer fetches what LVFS has every day.

use std::collections::HashMap;

use amethystora_ui::glib;
use amethystora_ui::process::{self, Output};
use amethystora_ui::t;
use serde_json::Value;

/// The update started from here, then the automatic one
pub const MANUAL_UNIT: &str = "uupd-manual.service";
pub const UNITS: [&str; 2] = [MANUAL_UNIT, "uupd.service"];
pub const TIMER: &str = "uupd.timer";
pub const RELEASES: &str = "https://github.com/iarsslen/amethystora/releases";
/// Lines of what the updater said, kept for the page's details
pub const LOG_LENGTH: usize = 400;
/// The containers' upgrade, a unit of the account's own, and what it logs each container under
pub const PKG: &str = "/usr/bin/amethystora-pkg";
pub const CONTAINERS_UNIT: &str = "amethystora-pkg-upgrade.service";
pub const CONTAINERS_LOG: &str = "amethystora-pkg-upgrade";
/// The AI agent, which a failed update can be handed to, unless the account turned it off
pub const AGENT: &str = "/usr/bin/amethystora-agent";
pub const SOFT_REBOOT: &str = "/usr/libexec/amethystora-soft-reboot";
/// Restart while you are away: the night time chosen, tonight's timer, and what the last night did
pub const NIGHT: &str = "/usr/libexec/amethystora-night-restart";
/// Saving data: whether the connection counts data, and since when the automatic update has waited
pub const METERED: &str = "/usr/libexec/amethystora-metered";

/// One version of the system: what it is called, when it was built (seconds) and which image it follows
#[derive(Debug, Clone, Default)]
pub struct Deployment {
    pub version: String,
    pub built: Option<f64>,
    pub image: String,
}

#[derive(Debug, Clone, Default)]
pub struct Machine {
    pub running: Deployment,
    /// The update waiting for a restart
    pub next: Option<Deployment>,
    /// ...and that it can be finished with a soft reboot
    pub quick: bool,
    /// The newest, when the machine was started from an older one at the boot menu
    pub latest: Option<Deployment>,
    pub previous: Option<Deployment>,
}

#[derive(Debug, Clone, Default)]
pub struct Automatic {
    pub enabled: bool,
    pub next: Option<f64>,
}

/// The last update that ended since the machine started, from here or by itself
#[derive(Debug, Clone, Default)]
pub struct Recent {
    pub unit: String,
    pub finished: f64,
    pub result: String,
}

#[derive(Debug, Clone, Default)]
pub struct Containers {
    pub count: usize,
    /// When the daily upgrade last ran over all of them, the longest ago
    pub upgraded: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct Device {
    pub name: String,
    pub vendor: String,
    pub current: String,
    pub next: String,
    pub summary: String,
    pub urgency: String,
    /// A UEFI capsule, which the firmware writes itself at the next start
    pub restart: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Firmware {
    pub error: Option<String>,
    pub devices: Vec<Device>,
}

/// A night that did not restart: when it was set for (seconds), why (late, unlocked, inhibited, battery, refused), and
/// the app that held the logout, by its ID
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Waited {
    pub night: f64,
    pub why: String,
    pub app: Option<String>,
}

/// Restart while you are away, as amethystora-night-restart says
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Night {
    /// HH:MM between 02:00 and 05:00, while it is on
    pub time: Option<String>,
    /// When tonight's timer is set for (seconds)
    pub armed: Option<f64>,
    pub last: Option<Waited>,
}

/// Saving data on a connection that counts it
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metered {
    pub on: bool,
    /// When the automatic update first waited for another connection (seconds)
    pub since: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct Status {
    pub machine: Option<Machine>,
    pub automatic: Automatic,
    pub recent: Option<Recent>,
    pub containers: Containers,
    pub agent: bool,
    pub firmware: Firmware,
    pub night: Night,
    pub metered: Metered,
}

/// One of uupd's modules, as the page shows it
#[derive(Debug, Clone, Default)]
pub struct Step {
    /// waiting, running, done, current (already the newest), failed
    pub state: String,
    pub detail: String,
    pub percent: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct Failure {
    pub context: String,
    pub output: String,
}

/// The containers' upgrade that follows an update started here
#[derive(Debug, Clone, Default)]
pub struct ContainersRun {
    pub state: String,
    pub started: f64,
    pub done: usize,
    pub count: usize,
    pub index: usize,
    pub current: String,
    pub failed: Vec<String>,
}

/// The update being followed, or the last one that ended while the window was open
#[derive(Debug, Clone, Default)]
pub struct Update {
    /// Tells two runs apart, as main.js did by the object itself
    pub serial: u64,
    pub unit: String,
    pub invocation: String,
    pub automatic: bool,
    /// A unit that failed a minute ago and is being tried again by systemd
    pub retry: bool,
    /// running, done, failed
    pub state: String,
    /// uupd's modules in the order it runs them, and any it names that the page did not know of
    pub steps: Vec<(String, Step)>,
    pub overall: Option<f64>,
    pub failures: Vec<Failure>,
    pub errors: Vec<String>,
    pub log: Vec<String>,
    /// success or errors, as uupd said at the end
    pub ended: String,
    /// Something came through: the journal is readable here
    pub journal: bool,
    pub containers: Option<ContainersRun>,
    pub finished: Option<f64>,
    /// systemd runs a failed update again a minute later
    pub retrying: bool,
}

impl Update {
    pub fn step(&self, name: &str) -> Option<&Step> {
        self.steps.iter().find(|(key, _)| key == name).map(|(_, step)| step)
    }

    pub fn step_mut(&mut self, name: &str) -> Option<&mut Step> {
        self.steps.iter_mut().find(|(key, _)| key == name).map(|(_, step)| step)
    }

    pub fn log(&mut self, text: &str) {
        if text.is_empty() || self.log.last().map(String::as_str) == Some(text) {
            return;
        }
        let text = if text.chars().count() > 2000 { format!("{}…", text.chars().take(2000).collect::<String>()) } else { text.to_string() };
        self.log.push(text);
        if self.log.len() > LOG_LENGTH {
            let extra = self.log.len() - LOG_LENGTH;
            self.log.drain(..extra);
        }
    }
}

/// A unit as `systemctl show` gave it
pub type Unit = HashMap<String, String>;

fn properties(text: &str) -> Unit {
    let mut values = HashMap::new();
    for line in text.lines() {
        if let Some(at) = line.find('=') {
            if at > 0 {
                values.insert(line[..at].to_string(), line[at + 1..].to_string());
            }
        }
    }
    values
}

/// systemctl --timestamp=unix: "@1758330764", or nothing for what has not happened since boot
pub fn unix_time(value: Option<&String>) -> Option<f64> {
    let value = value?.strip_prefix('@')?;
    (!value.is_empty() && value.chars().all(|c| c.is_ascii_digit())).then(|| value.parse().ok()).flatten()
}

pub fn now() -> f64 {
    amethystora_ui::i18n::now()
}

// --- This machine ---------------------------------------------------------------------------------------

/// rpm-ostree's list rather than bootc's, whose status needs root: amethystora-deployments is rpm-ostree on
/// an ostree image, and gives the same list from bootc on a sealed one, which has no rpm-ostree. The
/// deployment listed first is the one the next boot starts. When that is not the running one, it is either
/// the update waiting for a restart, which is staged, or the newest version, when the machine was started
/// from an older one at the boot menu: then `latest` is set, and `next` is not.
pub async fn machine() -> Option<Machine> {
    let Output { stdout, .. } = process::run("/usr/libexec/amethystora-deployments", &[]).await;
    let parsed: Value = serde_json::from_str(&stdout).ok()?;
    let deployments = parsed.get("deployments").and_then(Value::as_array).cloned().unwrap_or_default();
    let summary = |deployment: &Value| {
        let timestamp = deployment.get("timestamp").and_then(Value::as_f64).filter(|t| *t != 0.0);
        let reference = deployment.get("container-image-reference").and_then(Value::as_str).unwrap_or("");
        Deployment {
            version: deployment.get("version").and_then(Value::as_str).unwrap_or("").to_string(),
            built: timestamp,
            image: image_of(reference),
        }
    };
    let booted = deployments.iter().position(|d| d.get("booted").and_then(Value::as_bool).unwrap_or(false))?;
    let first = &deployments[0];
    let stamp = |d: &Value| d.get("timestamp").and_then(Value::as_f64).unwrap_or(0.0);
    let staged = first.get("staged").and_then(Value::as_bool).unwrap_or(false);
    let older = booted > 0 && !staged && stamp(first) > stamp(&deployments[booted]);
    let next = (booted > 0 && !older).then(|| summary(first));
    Some(Machine {
        running: summary(&deployments[booted]),
        quick: next.is_some() && quick().await,
        next,
        latest: older.then(|| summary(first)),
        previous: if older { None } else { deployments.get(booted + 1).map(summary) },
    })
}

/// Whether the update waiting for the restart can be finished with a soft reboot
async fn quick() -> bool {
    let Output { stdout, .. } = process::run(SOFT_REBOOT, &["status"]).await;
    serde_json::from_str::<Value>(&stdout).ok().and_then(|v| v.get("quick").and_then(Value::as_bool)).unwrap_or(false)
}

/// What came of the soft reboot
pub enum Restart {
    /// On its way: the session ends
    Started,
    /// Not to be had now, or an app asks not to end the session: GNOME's restart dialog takes over
    Full,
    Failed(String),
}

pub async fn restart_quickly() -> Restart {
    let output = process::run(SOFT_REBOOT, &["restart"]).await;
    match output.code {
        0 => Restart::Started,
        1 | 3 => Restart::Full,
        _ => {
            let line = output.last_error_line();
            Restart::Failed(if line.is_empty() { t!("The machine could not be restarted.") } else { line })
        }
    }
}

/// The apps open now, kept by Amethystora UI for the first login after the restart (its workspaces module's
/// org.amethystora.UI.Resume, `ame desktop resume`), before GNOME's restart dialog. Nothing is kept without the
/// extension or with the key off, and the restart goes on either way.
pub async fn keep_open_apps() {
    process::run(
        "gdbus",
        &["call", "--session", "--timeout", "5", "--dest", "org.gnome.Shell", "--object-path", "/org/amethystora/UI/Resume", "--method", "org.amethystora.UI.Resume.Save"],
    )
    .await;
}

// --- Restart while you are away --------------------------------------------------------------------------------

/// What amethystora-night-restart status --json says
pub fn read_night(text: &str) -> Night {
    let Ok(value) = serde_json::from_str::<Value>(text) else { return Night::default() };
    let time = value.get("time").and_then(Value::as_str).filter(|t| valid_night_time(t)).map(str::to_string);
    let armed = value.get("armed").and_then(Value::as_f64).filter(|t| *t > 0.0);
    let last = value.get("last").filter(|l| l.is_object()).and_then(|last| {
        let why = last.get("why").and_then(Value::as_str)?;
        ["late", "unlocked", "inhibited", "battery", "refused"].contains(&why).then(|| Waited {
            night: last.get("night").and_then(Value::as_f64).unwrap_or(0.0),
            why: why.to_string(),
            app: last
                .get("app")
                .and_then(Value::as_str)
                .filter(|a| !a.is_empty() && a.len() <= 200 && a.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)))
                .map(str::to_string),
        })
    });
    Night { time, armed, last }
}

/// HH:MM from 02:00 to 05:00, as the helper takes it
pub fn valid_night_time(time: &str) -> bool {
    let bytes = time.as_bytes();
    if bytes.len() != 5 || bytes[2] != b':' || !time.chars().enumerate().all(|(i, c)| i == 2 || c.is_ascii_digit()) {
        return false;
    }
    let (hour, minute): (u32, u32) = (time[..2].parse().unwrap_or(99), time[3..].parse().unwrap_or(99));
    minute < 60 && ((2..5).contains(&hour) || (hour == 5 && minute == 0))
}

pub async fn night() -> Night {
    read_night(&process::run(NIGHT, &["status", "--json"]).await.stdout)
}

/// Tonight's restart set, or taken away; the helper's own words when it could not be
pub async fn set_tonight(on: bool) -> Result<(), String> {
    let output = process::run(NIGHT, &[if on { "arm" } else { "cancel" }]).await;
    if output.ok() {
        Ok(())
    } else {
        let line = output.last_error_line();
        Err(if line.is_empty() { t!("Tonight's restart could not be set.") } else { line })
    }
}

/// An app's name from its launcher, for the night that did not restart because of it
pub fn app_name(id: &str) -> Option<String> {
    use amethystora_ui::gio::prelude::AppInfoExt;
    let wanted = format!("{id}.desktop");
    amethystora_ui::gio::AppInfo::all().into_iter().find(|app| app.id().as_deref() == Some(wanted.as_str())).map(|app| app.name().to_string())
}

// --- Saving data ----------------------------------------------------------------------------------------------

/// What amethystora-metered status --json says
pub fn read_metered(text: &str) -> Metered {
    let Ok(value) = serde_json::from_str::<Value>(text) else { return Metered::default() };
    Metered {
        on: value.get("metered").and_then(Value::as_bool).unwrap_or(false),
        since: value.get("waiting").and_then(|w| w.get("system")).and_then(Value::as_f64).filter(|t| *t > 0.0),
    }
}

pub async fn metered() -> Metered {
    read_metered(&process::run(METERED, &["status", "--json"]).await.stdout)
}

/// "ostree-image-signed:docker://ghcr.io/iarsslen/amethystora:stable" -> "ghcr.io/iarsslen/amethystora:stable"
fn image_of(reference: &str) -> String {
    match reference.find(':') {
        Some(at) if at > 0 => {
            let rest = &reference[at + 1..];
            rest.strip_prefix("docker://").unwrap_or(rest).to_string()
        }
        _ => reference.to_string(),
    }
}

/// What came of keeping the running version
pub struct Kept {
    pub machine: Option<Machine>,
    pub declined: bool,
    pub kept: bool,
    pub error: Option<String>,
}

/// Makes the running version the one the machine starts by default. bootc reverses the queued rollback a
/// start from the boot menu amounts to, and needs root, so polkit asks for an administrator's password.
pub async fn keep_version() -> Kept {
    let output = process::run("pkexec", &["/usr/bin/bootc", "rollback"]).await;
    let machine = machine().await;
    // pkexec: 126 when the password was not given, 127 when it was refused
    match output.code {
        126 | 127 => Kept { machine, declined: true, kept: false, error: None },
        0 => Kept { machine, declined: false, kept: true, error: None },
        _ => {
            let line = output.last_error_line();
            let error = if line.is_empty() { t!("This version could not be made the one the machine starts.") } else { line };
            Kept { machine, declined: false, kept: false, error: Some(error) }
        }
    }
}

/// The containers amethystora-pkg manages: how many, and when the daily upgrade last ran over them
pub async fn containers() -> Containers {
    let Output { stdout, .. } = process::run(PKG, &["containers", "list", "--json"]).await;
    let Ok(Value::Array(list)) = serde_json::from_str::<Value>(&stdout) else {
        return Containers::default();
    };
    let upgraded: Vec<f64> = list.iter().filter_map(|item| item.get("last_upgrade").and_then(Value::as_f64)).filter(|t| *t != 0.0).collect();
    Containers { count: list.len(), upgraded: upgraded.iter().cloned().reduce(f64::min) }
}

pub async fn automatic() -> Automatic {
    let enabled = glib::spawn_future_local(process::run("systemctl", &["is-enabled", TIMER]));
    let shown = glib::spawn_future_local(process::run("systemctl", &["show", TIMER, "--timestamp=unix", "-p", "NextElapseUSecRealtime"]));
    let enabled = enabled.await.map(|o| o.stdout.trim() == "enabled").unwrap_or(false);
    let shown = shown.await.unwrap_or_default();
    Automatic { enabled, next: if enabled { unix_time(properties(&shown.stdout).get("NextElapseUSecRealtime")) } else { None } }
}

/// Both updater units as systemd has them, in the order of UNITS
pub async fn units() -> Vec<Unit> {
    let mut args = vec!["show"];
    args.extend(UNITS);
    args.extend(["--timestamp=unix", "-p", "Id", "-p", "ActiveState", "-p", "SubState", "-p", "Result", "-p", "InvocationID", "-p", "ExecMainStartTimestamp", "-p", "ExecMainExitTimestamp"]);
    let output = process::run("systemctl", &args).await;
    // One block of properties per unit, with a blank line between them
    let mut blocks = vec![String::new()];
    for line in output.stdout.trim().lines() {
        if line.trim().is_empty() {
            if !blocks.last().expect("one at least").is_empty() {
                blocks.push(String::new());
            }
        } else {
            let block = blocks.last_mut().expect("one at least");
            block.push_str(line);
            block.push('\n');
        }
    }
    blocks.iter().map(|block| properties(block)).filter(|unit| unit.get("Id").is_some_and(|id| !id.is_empty())).collect()
}

/// A oneshot is "activating" for as long as it runs. One that failed and waits to be tried again, which
/// uupd's units do a minute later, is not running.
pub fn running(unit: &Unit) -> bool {
    let active = unit.get("ActiveState").map(String::as_str).unwrap_or("");
    ["activating", "active", "deactivating", "reloading"].contains(&active) && unit.get("SubState").map(String::as_str) != Some("auto-restart")
}

/// The last update that ended since the machine started, from here or by itself
pub fn recent(list: &[Unit]) -> Option<Recent> {
    let mut ended: Vec<Recent> = list
        .iter()
        .filter_map(|unit| {
            Some(Recent {
                unit: unit.get("Id")?.clone(),
                finished: unix_time(unit.get("ExecMainExitTimestamp"))?,
                result: unit.get("Result").cloned().unwrap_or_default(),
            })
        })
        .collect();
    ended.sort_by(|a, b| b.finished.total_cmp(&a.finished));
    ended.into_iter().next()
}

/// Whether the agentic features are on for this account (`ame agent toggle`)
pub async fn agent_on() -> bool {
    process::run(AGENT, &["status"]).await.ok()
}

// --- Firmware -------------------------------------------------------------------------------------------

/// The devices with a firmware update on LVFS, each with the release fwupdmgr update would install: the
/// first one listed. With --json fwupdmgr never asks anything, and says {"Devices":[]} when there is
/// nothing, so anything else means fwupd could not be asked.
pub async fn firmware() -> Firmware {
    let output = process::run("fwupdmgr", &["get-updates", "--json"]).await;
    let devices = serde_json::from_str::<Value>(&output.stdout).ok().and_then(|v| v.get("Devices").and_then(Value::as_array).cloned());
    let Some(devices) = devices else {
        let line = output.last_error_line();
        return Firmware { error: Some(if line.is_empty() { t!("fwupd could not be asked about firmware.") } else { line }), devices: Vec::new() };
    };
    let text = |v: &Value, key: &str| match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    };
    Firmware {
        error: None,
        devices: devices
            .iter()
            .map(|device| {
                let release = device.get("Releases").and_then(Value::as_array).and_then(|r| r.first()).cloned().unwrap_or(Value::Null);
                let name = text(device, "Name");
                Device {
                    name: if name.is_empty() { text(device, "DeviceId") } else { name },
                    vendor: text(device, "Vendor"),
                    current: text(device, "Version"),
                    next: text(&release, "Version"),
                    summary: text(&release, "Summary"),
                    urgency: text(&release, "Urgency"),
                    restart: device
                        .get("Flags")
                        .and_then(Value::as_array)
                        .is_some_and(|flags| flags.iter().any(|f| f.as_str() == Some("needs-reboot"))),
                }
            })
            .collect(),
    }
}

/// Asks LVFS again now rather than at the timer's next run. fwupdmgr says 2 when what it has is new enough
/// already, which is as good as having fetched it.
pub async fn refresh_firmware() -> Firmware {
    let output = process::run("fwupdmgr", &["refresh"]).await;
    let mut result = firmware().await;
    if output.code != 0 && output.code != 2 && result.error.is_none() {
        let line = output.last_error_line();
        result.error = Some(if line.is_empty() { t!("LVFS could not be reached.") } else { line });
    }
    result
}

/// What came of switching automatic updates
pub struct Switched {
    pub automatic: Automatic,
    pub error: Option<String>,
}

/// Enabling or disabling the timer is not something the uupd rule allows, so systemd asks polkit, and GNOME
/// asks for the password
pub async fn set_automatic(on: bool) -> Switched {
    let output = process::run("systemctl", &[if on { "enable" } else { "disable" }, "--now", TIMER]).await;
    let automatic = automatic().await;
    let error = (output.code != 0).then(|| {
        let lower = output.stderr.to_lowercase();
        if lower.contains("access denied") || lower.contains("authenticat") || lower.contains("not authorized") {
            t!("Left as it was: the password was not given.")
        } else if output.stderr.trim().is_empty() {
            t!("Automatic updates could not be changed.")
        } else {
            output.stderr.trim().to_string()
        }
    });
    Switched { automatic, error }
}

// --- The update -----------------------------------------------------------------------------------------

/// uupd's modules by the title its progress gives them, in the order it runs them. uupd's own Distrobox
/// module is off in /etc/uupd/config.json: the Distrobox step, Containers on the page, is the account's
/// amethystora-pkg-upgrade.service, which follows an update started here.
pub const MODULES: [&str; 3] = ["System", "Brew", "Flatpak"];

/// What a failed command was part of, from the context uupd gives it: "System Update", "Brew Upgrade",
/// "System Apps", "Apps for User: <name>"
fn module_of(context: &str) -> &'static str {
    if context.starts_with("System Update") {
        "System"
    } else if context.starts_with("Brew") {
        "Brew"
    } else if context.contains("Apps") {
        "Flatpak"
    } else if context.to_lowercase().contains("distrobox") {
        "Distrobox"
    } else {
        ""
    }
}

fn text_of(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// One line of uupd's log: {"level":"INFO","msg":"Updating","title":"System","description":"Downloading",
/// "overall":14,...}. Anything that is not JSON is systemd's own and says nothing new. Says whether the
/// page has anything new to show.
pub fn read_line(update: &mut Update, line: &str) -> bool {
    let Ok(Value::Object(entry)) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    let Some(msg) = entry.get("msg").and_then(Value::as_str) else {
        return false;
    };
    let number = |key: &str| entry.get(key).and_then(Value::as_f64).filter(|n| n.is_finite());
    if msg == "Updating" {
        let title = text_of(entry.get("title"));
        if update.step(&title).is_none() {
            update.steps.push((title.clone(), Step { state: "waiting".into(), ..Default::default() }));
        }
        for (name, step) in update.steps.iter_mut() {
            if *name != title && step.state == "running" {
                step.state = "done".into();
            }
        }
        let detail = text_of(entry.get("description"));
        let step = update.step_mut(&title).expect("added above");
        step.state = "running".into();
        step.detail = detail.clone();
        step.percent = if title == "System" { number("step_progress") } else { None };
        if let Some(overall) = number("overall") {
            update.overall = Some(overall);
        }
        update.log(&format!("{title}: {detail}"));
    } else if msg == "System Updater module status"
        && entry.get("enabled") == Some(&Value::Bool(false))
        && update.step("System").is_some_and(|s| s.state == "waiting")
    {
        // No new image, or the system module turned off: nothing to download
        update.step_mut("System").expect("checked").state = "current".into();
    } else if msg == "module_fail" {
        let output = entry.get("output");
        let mut context = text_of(entry.get("module"));
        if context.is_empty() {
            context = text_of(output.and_then(|o| o.get("Context")));
        }
        let name = module_of(&context);
        if !name.is_empty() {
            if let Some(step) = update.step_mut(name) {
                step.state = "failed".into();
            }
        }
        let stdout = text_of(output.and_then(|o| o.get("Stdout")));
        let stdout = stdout.trim();
        let tail: String = {
            let chars: Vec<char> = stdout.chars().collect();
            chars[chars.len().saturating_sub(4000)..].iter().collect()
        };
        update.failures.push(Failure { context: context.clone(), output: tail });
        let cli = text_of(entry.get("cli"));
        let context = if cli.is_empty() { context } else { format!("{context} ({cli})") };
        update.log(&t!("Failed: {context}", context = context));
    } else if msg == "Updates Completed Successfully" || msg == "Updates finished with errors!" {
        update.ended = if msg == "Updates Completed Successfully" { "success" } else { "errors" }.into();
        update.log(msg);
    } else if msg.starts_with('{') || msg == "scanned progress" {
        // bootc's own progress, many times a second: the "Updating" lines already carry it
        return false;
    } else {
        if entry.get("level").and_then(Value::as_str) == Some("ERROR") {
            update.errors.push(msg.to_string());
        }
        let detail = match entry.get("error") {
            Some(Value::String(error)) => format!(": {error}"),
            _ => String::new(),
        };
        update.log(&format!("{msg}{detail}"));
    }
    true
}

/// One line amethystora-pkg logged: {"container":"debian","state":"running","index":1,"count":2}, then the
/// same with "ok" or "failed", and {"done":true,...} at the end, or {"done":true,"skipped":"..."}. Says
/// whether the page has anything new to show, and whether the containers are done.
pub fn read_containers(update: &mut Update, line: &str, followed: bool) -> (bool, bool) {
    let Ok(Value::Object(entry)) = serde_json::from_str::<Value>(line) else {
        return (false, false);
    };
    if update.containers.is_none() || update.step("Distrobox").is_none() {
        return (false, false);
    }
    let truthy = |v: Option<&Value>| match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::Null) | None => false,
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    };
    let mut finished = false;
    if truthy(entry.get("done")) {
        let state = if truthy(entry.get("failed")) { "failed" } else { "done" };
        update.containers.as_mut().expect("checked").state = state.into();
        let step = update.step_mut("Distrobox").expect("checked");
        step.state = state.into();
        step.detail.clear();
        finished = true;
    } else if let Some(Value::String(container)) = entry.get("container") {
        let state = text_of(entry.get("state"));
        let boxes = update.containers.as_mut().expect("checked");
        if let Some(count) = entry.get("count").and_then(Value::as_f64).filter(|n| *n != 0.0) {
            boxes.count = count as usize;
        }
        if state == "running" {
            boxes.current = container.clone();
            boxes.index = entry.get("index").and_then(Value::as_f64).filter(|n| *n != 0.0).map(|n| n as usize).unwrap_or(boxes.done + 1);
            let detail = t!("{container} · {index} of {count}", container = container, index = boxes.index, count = boxes.count);
            update.step_mut("Distrobox").expect("checked").detail = detail;
        } else if state == "failed" {
            boxes.failed.push(container.clone());
        }
        if followed {
            update.log(&t!(
                "Containers: {container}, {state, select, running {upgrading} ok {up to date} failed {did not upgrade} other {{state}}}",
                container = container,
                state = state
            ));
        }
    }
    (true, finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update() -> Update {
        Update {
            state: "running".into(),
            steps: MODULES.iter().map(|m| (m.to_string(), Step { state: "waiting".into(), ..Default::default() })).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn reads_uupds_log() {
        let mut u = update();
        assert!(read_line(&mut u, r#"{"level":"INFO","msg":"Updating","title":"System","description":"Downloading","overall":14,"step_progress":40}"#));
        assert_eq!(u.step("System").unwrap().state, "running");
        assert_eq!(u.step("System").unwrap().percent, Some(40.0));
        assert_eq!(u.overall, Some(14.0));
        assert!(read_line(&mut u, r#"{"msg":"Updating","title":"Brew","description":"x","overall":50}"#));
        assert_eq!(u.step("System").unwrap().state, "done");
        assert_eq!(u.step("Brew").unwrap().percent, None);
        assert!(read_line(&mut u, r#"{"msg":"module_fail","module":"Apps for User: ana","output":{"Stdout":"  boom "},"cli":"flatpak"}"#));
        assert_eq!(u.step("Flatpak").unwrap().state, "failed");
        assert_eq!(u.failures[0].output, "boom");
        assert_eq!(u.log.last().unwrap(), "Failed: Apps for User: ana (flatpak)");
        assert!(!read_line(&mut u, r#"{"msg":"scanned progress"}"#));
        assert!(!read_line(&mut u, "not json"));
        assert!(read_line(&mut u, r#"{"msg":"Updates finished with errors!"}"#));
        assert_eq!(u.ended, "errors");
        assert!(read_line(&mut u, r#"{"msg":"Updating","title":"Distrobox","description":"y"}"#));
        assert_eq!(u.steps.last().unwrap().0, "Distrobox");
    }

    #[test]
    fn system_current_only_while_waiting() {
        let mut u = update();
        read_line(&mut u, r#"{"msg":"System Updater module status","enabled":false}"#);
        assert_eq!(u.step("System").unwrap().state, "current");
    }

    #[test]
    fn reads_containers() {
        let mut u = update();
        u.steps.push(("Distrobox".into(), Step { state: "running".into(), ..Default::default() }));
        u.containers = Some(ContainersRun { state: "running".into(), count: 2, ..Default::default() });
        assert_eq!(read_containers(&mut u, r#"{"container":"debian","state":"running","index":1,"count":2}"#, true), (true, false));
        assert_eq!(u.step("Distrobox").unwrap().detail, "debian · 1 of 2");
        assert_eq!(u.log.last().unwrap(), "Containers: debian, upgrading");
        read_containers(&mut u, r#"{"container":"arch","state":"failed"}"#, true);
        assert_eq!(u.containers.as_ref().unwrap().failed, vec!["arch"]);
        assert_eq!(read_containers(&mut u, r#"{"done":true,"failed":1}"#, true), (true, true));
        assert_eq!(u.step("Distrobox").unwrap().state, "failed");
    }

    #[test]
    fn night_restart() {
        for good in ["02:00", "03:30", "04:59", "05:00"] {
            assert!(valid_night_time(good), "{good}");
        }
        for bad in ["01:59", "05:01", "06:00", "3:00", "03:60", "0a:00", "03-00", "", "03:000"] {
            assert!(!valid_night_time(bad), "{bad}");
        }
        assert_eq!(read_night("garbage"), Night::default());
        assert_eq!(
            read_night(r#"{"time":"03:00","armed":1761966000,"last":null}"#),
            Night { time: Some("03:00".into()), armed: Some(1761966000.0), last: None }
        );
        let waited = read_night(r#"{"time":"07:00","armed":null,"last":{"night":1,"result":"waited","why":"inhibited","app":"org.gnome.TextEditor"}}"#);
        assert_eq!(waited.time, None);
        assert_eq!(waited.last, Some(Waited { night: 1.0, why: "inhibited".into(), app: Some("org.gnome.TextEditor".into()) }));
        // A reason the page does not know, or an app ID that is no ID, is left out
        assert_eq!(read_night(r#"{"last":{"night":1,"why":"other"}}"#).last, None);
        assert_eq!(read_night(r#"{"last":{"night":1,"why":"late","app":"../x y"}}"#).last.unwrap().app, None);
    }

    #[test]
    fn saving_data() {
        assert_eq!(read_metered(""), Metered::default());
        assert_eq!(read_metered(r#"{"metered":true,"waiting":{"system":1761900000,"containers":null,"backup":null}}"#), Metered { on: true, since: Some(1761900000.0) });
        assert_eq!(read_metered(r#"{"metered":false,"waiting":{"system":null}}"#), Metered { on: false, since: None });
    }

    #[test]
    fn images_and_times() {
        assert_eq!(image_of("ostree-image-signed:docker://ghcr.io/iarsslen/amethystora:stable"), "ghcr.io/iarsslen/amethystora:stable");
        assert_eq!(image_of("ostree-unverified-registry:ghcr.io/x/y:latest"), "ghcr.io/x/y:latest");
        assert_eq!(unix_time(Some(&"@1758330764".to_string())), Some(1758330764.0));
        assert_eq!(unix_time(Some(&"".to_string())), None);
        assert_eq!(unix_time(None), None);
    }
}
