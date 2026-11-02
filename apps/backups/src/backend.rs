//! What Amethystora Backups runs: what main.js did for the Electron window.
//!
//! Earlier versions of your files, from the hourly snapshots of the home folders and the daily restic
//! backup, on one timeline, and putting them back. What there is, and everything done to it, is
//! /usr/libexec/amethystora-restore's, which `ame backup snapshots` prints too: this window shows what that
//! says and works nothing out for itself.
//!
//! The page only ever sends back what this module read: a point in time that `points` gave, a folder or
//! file that `ls` listed (or the home folder, or the folder the window was opened on and those above it),
//! and one of three ways to put back. Each reaches the helper as an argument of its own, never through a
//! shell, and the helper checks it again: nothing comes back outside the home folder, and nothing is
//! removed. A path that starts with "-" is refused here as the helper refuses it, before anything runs.
//! Setting the backup up runs `ame backup setup` in a terminal. The hourly snapshots are Amethystora
//! Security's to switch, and this window only opens it.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use amethystora_ui::glib;
use amethystora_ui::process::{self, Options, Output};
use regex::Regex;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

pub const RESTORE: &str = "/usr/libexec/amethystora-restore";
/// The AI agent, which a failed backup can be handed to, unless the account turned it off
pub const AGENT: &str = "/usr/bin/amethystora-agent";
/// The daily backup, a user unit (amethystora-backup.timer)
pub const UNIT: &str = "amethystora-backup.service";
/// The three ways to put back, as the helper takes them (--keep-both, --replace, --skip)
pub const MODES: [&str; 3] = ["keep-both", "replace", "skip"];
pub const PAGES: [&str; 2] = ["browse", "status"];
/// A point in time as `points` gives them: a snapshot's name or a backup's id
const POINT: &str = r"^([0-9]{4}-[0-9]{2}-[0-9]{2}_[0-9]{4}|[0-9a-f]{8,64})$";
/// "A timeout of 0 waits as long as it takes", for a whole folder coming back: a month
const NO_LIMIT: Duration = Duration::from_secs(30 * 86400);

fn point_expression() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    CELL.get_or_init(|| Regex::new(POINT).expect("a valid expression"))
}

// --- What the helper says ----------------------------------------------------------------------------------

/// One point in time: a snapshot on this machine (source "local") or a backup ("backup")
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Point {
    pub source: String,
    pub id: String,
    pub time: f64,
    /// The machine a backup was made by, which may be another one backing up to the same repository
    pub host: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Snapshots {
    pub possible: bool,
    pub on: bool,
    pub covers: bool,
    pub count: f64,
    pub latest: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct BackupState {
    pub set_up: bool,
    pub repository: Option<String>,
    pub daily: bool,
    pub running: bool,
    pub failed: bool,
    pub last_success: Option<f64>,
    pub since: Option<f64>,
    pub next: Option<f64>,
    pub stale: bool,
    pub stale_days: f64,
}

/// amethystora-restore status, and whether the agentic features are on for this account
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Status {
    pub snapshots: Snapshots,
    pub backup: BackupState,
    #[serde(skip)]
    pub agent: bool,
}

/// amethystora-restore points: every point, newest first, whether the backup could be read, and this
/// machine's name
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Timeline {
    pub points: Vec<Point>,
    pub backup: String,
    pub detail: Option<String>,
    #[serde(skip)]
    pub hostname: String,
}

/// One entry of a folder as it was, with its state now: missing, changed, same, or present (a folder)
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Entry {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub size: Option<f64>,
    pub mtime: f64,
    pub now: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Listing {
    pub path: String,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Version {
    pub source: String,
    pub id: String,
    pub time: f64,
    pub host: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub size: Option<f64>,
    pub mtime: f64,
}

impl Version {
    pub fn point(&self) -> Point {
        Point { source: self.source.clone(), id: self.id.clone(), time: self.time, host: self.host.clone() }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Now {
    #[serde(rename = "type")]
    pub kind: String,
    pub size: Option<f64>,
    pub mtime: f64,
}

/// The points at which a file or folder differs from the one before, newest first, and it as it is now
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Versions {
    pub path: String,
    pub now: Option<Now>,
    pub versions: Vec<Version>,
    pub backup: String,
    pub detail: Option<String>,
}

/// What putting back did: added, already the same, kept beside a newer version, replaced, left as it is
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PutBack {
    pub added: f64,
    pub same: f64,
    pub kept: f64,
    pub replaced: f64,
    pub skipped: f64,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Opened {
    pub opened: String,
}

/// What went wrong, as the helper names it ({error, detail}), which the page says in its own words
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Failure {
    pub error: String,
    pub detail: String,
}

impl Failure {
    fn new(error: &str, detail: impl Into<String>) -> Failure {
        Failure { error: error.to_string(), detail: detail.into() }
    }

    /// What this window will not ask: a point or a path it was never given
    pub fn refused() -> Failure {
        Failure::new("refused", "")
    }
}

pub type Answer<T> = Result<T, Failure>;

/// The last `count` lines of what a program said
fn tail(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text.trim().split('\n').collect();
    lines[lines.len().saturating_sub(count)..].join("\n")
}

/// What the helper said, as JSON: what was asked for, or {error, detail}. A helper that said nothing it
/// could read failed, with the end of what it said on stderr.
pub fn answer<T: DeserializeOwned>(output: &Output) -> Answer<T> {
    let value: Value = match serde_json::from_str(&output.stdout) {
        Ok(value) => value,
        Err(_) => return Err(Failure::new("failed", tail(&output.stderr, 3))),
    };
    match value.get("error") {
        Some(Value::Null | Value::Bool(false)) | None => {}
        Some(error) => {
            let error = error.as_str().map(str::to_string).unwrap_or_else(|| error.to_string());
            let detail = value.get("detail").and_then(Value::as_str).unwrap_or("").to_string();
            return Err(Failure { error, detail });
        }
    }
    serde_json::from_value(value).map_err(|_| Failure::new("failed", ""))
}

async fn run(program: &str, args: &[&str], timeout: Duration) -> Output {
    process::run_with(program, args, Options { timeout, ..Default::default() }).await
}

async fn helper<T: DeserializeOwned>(args: &[&str], timeout: Duration) -> Answer<T> {
    let mut all: Vec<&str> = args.to_vec();
    all.push("--json");
    answer(&run(RESTORE, &all, timeout).await)
}

// --- The arguments ---------------------------------------------------------------------------------------

/// A path with its . and .. taken away, as path.resolve leaves it
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The home folder, with its links followed
pub fn home() -> PathBuf {
    let home = amethystora_ui::palette::home();
    std::fs::canonicalize(&home).unwrap_or(home)
}

/// The page to open: `amethystora-backups status` opens on the status
pub fn page_argument(args: &[String]) -> Option<&'static str> {
    let page = args.iter().find(|arg| !arg.starts_with('-'))?;
    PAGES.iter().find(|known| *known == page).copied()
}

/// The folder to open on, from the command line or from Files' Open With: inside the home folder, or
/// nothing. A file opens its folder. "" is the home folder itself.
pub fn folder_argument(args: &[String], cwd: &Path, home: &Path) -> Option<String> {
    let given = args.iter().find(|arg| !arg.starts_with('-'))?;
    let given = given.strip_prefix("file://").unwrap_or(given);
    let mut resolved = std::fs::canonicalize(normalize(&cwd.join(given))).ok()?;
    if !resolved.is_dir() {
        resolved = resolved.parent()?.to_path_buf();
    }
    if resolved == home {
        return Some(String::new());
    }
    let inside = resolved.strip_prefix(home).ok()?;
    (!inside.as_os_str().is_empty() && home != Path::new("/")).then(|| inside.to_string_lossy().into_owned())
}

/// The point in time to open on, `--at=POINT`: a snapshot's name or a backup's id, in the shape `points`
/// gives them. The page opens on it only when the list it reads has it.
pub fn point_argument(args: &[String]) -> String {
    let point = args.iter().find_map(|arg| arg.strip_prefix("--at=")).unwrap_or("");
    if point_expression().is_match(point) { point.to_string() } else { String::new() }
}

/// The file whose earlier versions to open on, `--versions=FILE` (Files' Earlier versions): inside the home
/// folder, or nothing
pub fn versions_argument(args: &[String], cwd: &Path, home: &Path) -> Option<String> {
    let given = args.iter().find_map(|arg| arg.strip_prefix("--versions="))?;
    let resolved = std::fs::canonicalize(normalize(&cwd.join(given))).ok()?;
    let inside = resolved.strip_prefix(home).ok()?;
    (!inside.as_os_str().is_empty() && home != Path::new("/")).then(|| inside.to_string_lossy().into_owned())
}

// --- What has been read ----------------------------------------------------------------------------------

/// The points and paths the helper gave: the only ones sent back to it
pub struct Known {
    /// The points in time `points` last gave, by id, and those `versions` found besides
    points: HashMap<String, Point>,
    /// The folders and files the helper has listed, and the folders above them. "" is the home folder.
    paths: HashSet<String>,
}

impl Default for Known {
    fn default() -> Self {
        Known { points: HashMap::new(), paths: HashSet::from([String::new()]) }
    }
}

/// "folder/name", or "name" in the home folder
pub fn join(folder: &str, name: &str) -> String {
    if folder.is_empty() { name.to_string() } else { format!("{folder}/{name}") }
}

/// The folder a path is in, "" for the home folder
pub fn parent(path: &str) -> &str {
    path.rfind('/').map(|at| &path[..at]).unwrap_or("")
}

impl Known {
    /// A path and every folder above it, as paths the page may send back
    pub fn remember(&mut self, relative: &str) {
        let mut current = relative;
        while !current.is_empty() {
            self.paths.insert(current.to_string());
            current = parent(current);
        }
    }

    pub fn has_point(&self, id: &str) -> bool {
        self.points.contains_key(id)
    }

    pub fn point(&self, id: &str) -> Option<&Point> {
        self.points.get(id)
    }

    pub fn has_path(&self, path: &str) -> bool {
        self.paths.contains(path)
    }

    /// A path this window may hand to the helper: one it was given, and never one the helper would take
    /// for an option
    fn path(&self, path: &str) -> Answer<()> {
        if !self.has_path(path) {
            return Err(Failure::refused());
        }
        if path.starts_with('-') {
            return Err(Failure::new("outside", ""));
        }
        Ok(())
    }

    pub fn check_list(&self, id: &str, folder: &str) -> Answer<()> {
        if !self.has_point(id) {
            return Err(Failure::refused());
        }
        self.path(folder)
    }

    pub fn check_versions(&self, file: &str) -> Answer<()> {
        if file.is_empty() {
            return Err(Failure::refused());
        }
        self.path(file)
    }

    pub fn check_put_back(&self, id: &str, files: &[String], mode: &str) -> Answer<()> {
        if !self.has_point(id) || !MODES.contains(&mode) || files.is_empty() {
            return Err(Failure::refused());
        }
        files.iter().try_for_each(|file| self.path(file))
    }

    pub fn check_look(&self, id: &str, file: &str) -> Answer<()> {
        if !self.has_point(id) || file.is_empty() {
            return Err(Failure::refused());
        }
        self.path(file)
    }

    fn set_points(&mut self, points: &[Point]) {
        self.points = points.iter().map(|point| (point.id.clone(), point.clone())).collect();
    }

    fn listed(&mut self, folder: &str, listing: &Listing) {
        for entry in &listing.entries {
            self.remember(&join(folder, &entry.name));
        }
    }

    fn versions_found(&mut self, versions: &Versions) {
        for version in &versions.versions {
            self.points.entry(version.id.clone()).or_insert_with(|| version.point());
        }
    }
}

/// The helper, asked only about what it gave
#[derive(Default)]
pub struct Restore {
    pub known: RefCell<Known>,
}

impl Restore {
    pub async fn status(&self) -> Answer<Status> {
        let agent = glib::spawn_future_local(async { run(AGENT, &["status"], Duration::from_secs(15)).await });
        let result: Answer<Status> = helper(&["status"], Duration::from_secs(60)).await;
        // The agentic features are on for this account unless `ame agent toggle` turned them off
        let on = agent.await.map(|output| output.code == 0).unwrap_or(false);
        result.map(|status| Status { agent: on, ..status })
    }

    pub async fn points(&self) -> Answer<Timeline> {
        let result: Answer<Timeline> = helper(&["points"], Duration::from_secs(300)).await;
        let mut timeline = result?;
        self.known.borrow_mut().set_points(&timeline.points);
        timeline.hostname = glib::host_name().to_string();
        Ok(timeline)
    }

    /// One folder as it was at a point, with each entry's state now
    pub async fn ls(&self, id: &str, folder: &str) -> Answer<Listing> {
        self.known.borrow().check_list(id, folder)?;
        let listing: Listing = helper(&["ls", id, folder], Duration::from_secs(300)).await?;
        self.known.borrow_mut().listed(folder, &listing);
        Ok(listing)
    }

    /// The points at which a file or folder that was listed changed
    pub async fn versions(&self, file: &str) -> Answer<Versions> {
        self.known.borrow().check_versions(file)?;
        let versions: Versions = helper(&["versions", file], Duration::from_secs(600)).await?;
        self.known.borrow_mut().versions_found(&versions);
        Ok(versions)
    }

    /// The same, from this machine's snapshots alone, which is a moment's work; `backup` says "later" when
    /// there is a backup still to ask
    pub async fn versions_here(&self, file: &str) -> Answer<Versions> {
        self.known.borrow().check_versions(file)?;
        let versions: Versions = helper(&["versions", file, "--local"], Duration::from_secs(60)).await?;
        self.known.borrow_mut().versions_found(&versions);
        Ok(versions)
    }

    pub async fn put_back(&self, id: &str, files: &[String], mode: &str) -> Answer<PutBack> {
        self.known.borrow().check_put_back(id, files, mode)?;
        let flag = format!("--{mode}");
        let mut args: Vec<&str> = vec!["put-back", id];
        args.extend(files.iter().map(String::as_str));
        args.push(&flag);
        helper(&args, NO_LIMIT).await
    }

    /// A read-only copy, opened in whatever opens it: the snapshot itself, or a copy from the backup
    pub async fn look(&self, id: &str, file: &str) -> Answer<Opened> {
        self.known.borrow().check_look(id, file)?;
        helper(&["open", id, file], NO_LIMIT).await
    }
}

// --- Backing up now --------------------------------------------------------------------------------------

/// The service's state, as systemctl shows it: ActiveState, Result and the invocation it is on
pub fn unit_properties(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

pub async fn unit_state() -> HashMap<String, String> {
    let output = run("systemctl", &["--user", "show", "-p", "ActiveState", "-p", "Result", "-p", "InvocationID", UNIT], Duration::from_secs(15)).await;
    unit_properties(&output.stdout)
}

pub fn running(state: &HashMap<String, String>) -> bool {
    matches!(state.get("ActiveState").map(String::as_str), Some("activating" | "active"))
}

/// What the backup logged last, for when it failed
pub async fn last_lines() -> String {
    let unit = format!("--user-unit={UNIT}");
    let output = run("journalctl", &["--user", &unit, "--lines=12", "--output=cat", "--no-pager", "--quiet"], Duration::from_secs(15)).await;
    output.stdout.trim().to_string()
}

pub async fn start_backup() -> Result<(), String> {
    let output = run("systemctl", &["--user", "start", "--no-block", UNIT], Duration::from_secs(15)).await;
    if output.code == 0 { Ok(()) } else { Err(output.stderr.trim().to_string()) }
}

/// The two of the recipe's own actions this window offers: setting up, which asks, in a terminal, and the
/// daily run on or off, which does not. What the second said, its last four lines.
pub async fn daily(on: bool) -> (bool, String) {
    let output = run("ame", &["backup", if on { "on" } else { "off" }], Duration::from_secs(60)).await;
    (output.code == 0, tail(&format!("{}\n{}", output.stdout, output.stderr), 4))
}

pub fn set_up() {
    process::in_terminal(&["ame", "backup", "setup"]);
}

pub fn security() {
    process::launch("amethystora-security", &[]);
}

/// The failed backup handed to the agent, which looks into it in a terminal and changes nothing until
/// asked: only ever the one unit's name, never anything from the page
pub fn diagnose() {
    process::launch(AGENT, &["diagnose", UNIT]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(stdout: &str, stderr: &str) -> Output {
        Output { code: 1, stdout: stdout.into(), stderr: stderr.into() }
    }

    fn known_with(points: &[&str], paths: &[&str]) -> Known {
        let mut known = Known::default();
        known.set_points(&points.iter().map(|id| Point { id: id.to_string(), ..Default::default() }).collect::<Vec<_>>());
        for path in paths {
            known.remember(path);
        }
        known
    }

    #[test]
    fn reads_what_the_helper_says() {
        let status: Status = answer(&output(
            r#"{"version":1,"home":"/var/home/a","snapshots":{"possible":true,"on":true,"covers":true,"count":3,"latest":1760000000},
               "backup":{"set_up":true,"repository":"rest:https://h:8000/a/","daily":true,"running":false,"failed":false,
               "last_success":null,"since":1750000000,"next":1760100000,"stale":false,"stale_days":7}}"#,
            "",
        ))
        .unwrap();
        assert!(status.snapshots.on && status.backup.set_up && status.backup.last_success.is_none());
        assert_eq!(status.snapshots.latest, Some(1760000000.0));
        assert_eq!(status.backup.repository.as_deref(), Some("rest:https://h:8000/a/"));

        let timeline: Timeline = answer(&output(
            r#"{"version":1,"points":[{"source":"local","id":"2026-10-04_1400","time":1759586400},
               {"source":"backup","id":"4f1c9a2e","time":1759500000,"host":"other"}],"backup":"ok","detail":null}"#,
            "",
        ))
        .unwrap();
        assert_eq!(timeline.points.len(), 2);
        assert_eq!(timeline.points[1].host.as_deref(), Some("other"));
        assert_eq!(timeline.backup, "ok");

        let listing: Listing = answer(&output(
            r#"{"version":1,"point":{"source":"local","id":"x","time":1},"path":"Documents","entries":[
               {"name":"notes","type":"dir","size":null,"mtime":10,"now":"present"},
               {"name":"a.txt","type":"file","size":4,"mtime":11,"now":"changed"}]}"#,
            "",
        ))
        .unwrap();
        assert_eq!(listing.entries[0].kind, "dir");
        assert_eq!(listing.entries[0].size, None);
        assert_eq!(listing.entries[1].size, Some(4.0));

        let versions: Versions = answer(&output(
            r#"{"version":1,"path":"a.txt","now":null,"versions":[{"source":"backup","id":"4f1c9a2e","time":5,"host":"h",
               "type":"file","size":3,"mtime":4}],"backup":"unreachable","detail":"Fatal: no"}"#,
            "",
        ))
        .unwrap();
        assert!(versions.now.is_none());
        assert_eq!(versions.versions[0].point().id, "4f1c9a2e");
        assert_eq!(versions.detail.as_deref(), Some("Fatal: no"));

        let put: PutBack = answer(&output(r#"{"version":1,"added":1,"same":0,"kept":2,"replaced":0,"skipped":0,"missing":["x"]}"#, "")).unwrap();
        assert_eq!((put.added, put.kept, put.missing.len()), (1.0, 2.0, 1));
    }

    #[test]
    fn says_what_went_wrong() {
        let failed: Answer<Listing> = answer(&output(r#"{"error":"not-found","detail":"There is no folder x in the snapshot y."}"#, ""));
        assert_eq!(failed.unwrap_err(), Failure { error: "not-found".into(), detail: "There is no folder x in the snapshot y.".into() });
        // Nothing it could read: the end of what it said on stderr
        let failed: Answer<Status> = answer(&output("", "one\ntwo\nthree\nfour\n"));
        assert_eq!(failed.unwrap_err(), Failure { error: "failed".into(), detail: "two\nthree\nfour".into() });
        let failed: Answer<Status> = answer(&output("Usage: ...", ""));
        assert_eq!(failed.unwrap_err().error, "failed");
    }

    #[test]
    fn remembers_paths_and_the_folders_above_them() {
        let mut known = Known::default();
        assert!(known.has_path(""));
        known.remember("Documents/notes/a.txt");
        assert!(known.has_path("Documents") && known.has_path("Documents/notes") && known.has_path("Documents/notes/a.txt"));
        assert!(!known.has_path("Documents/notes/b.txt"));
        let listing = Listing { entries: vec![Entry { name: "b.txt".into(), ..Default::default() }], ..Default::default() };
        known.listed("Documents/notes", &listing);
        assert!(known.has_path("Documents/notes/b.txt"));
        known.listed("", &listing);
        assert!(known.has_path("b.txt"));
    }

    #[test]
    fn refuses_what_it_was_not_given() {
        let known = known_with(&["2026-10-04_1400", "4f1c9a2e"], &["Documents/report.txt", "-x", "Documents/-y"]);
        assert!(known.check_list("2026-10-04_1400", "").is_ok());
        assert!(known.check_list("2026-10-04_1400", "Documents").is_ok());
        assert_eq!(known.check_list("2026-10-04_1500", "Documents"), Err(Failure::refused()));
        assert_eq!(known.check_list("4f1c9a2e", "Pictures"), Err(Failure::refused()));
        // Outside the home, or past it, is never one it was given
        for path in ["../../etc/passwd", "/etc/passwd", "Documents/../..", "escape/passwd"] {
            assert_eq!(known.check_versions(path), Err(Failure::refused()), "{path}");
            assert_eq!(known.check_look("4f1c9a2e", path), Err(Failure::refused()), "{path}");
            assert_eq!(known.check_put_back("4f1c9a2e", &[path.to_string()], "replace"), Err(Failure::refused()), "{path}");
        }
        // A path that starts with "-", even one it listed, is one the helper would take for an option
        assert_eq!(known.check_versions("-x").unwrap_err().error, "outside");
        assert_eq!(known.check_look("4f1c9a2e", "-x").unwrap_err().error, "outside");
        assert_eq!(known.check_list("4f1c9a2e", "-x").unwrap_err().error, "outside");
        assert_eq!(known.check_put_back("4f1c9a2e", &["Documents/report.txt".into(), "-x".into()], "skip").unwrap_err().error, "outside");
        assert!(known.check_versions("Documents/-y").is_ok());
        // The versions and a look need a file; the home folder itself is put back whole
        assert_eq!(known.check_versions(""), Err(Failure::refused()));
        assert_eq!(known.check_look("4f1c9a2e", ""), Err(Failure::refused()));
        assert!(known.check_put_back("4f1c9a2e", &["".into()], "replace").is_ok());
        // Only the three ways, and something to put back
        assert!(known.check_put_back("4f1c9a2e", &["Documents/report.txt".into()], "keep-both").is_ok());
        assert_eq!(known.check_put_back("4f1c9a2e", &["Documents/report.txt".into()], "delete"), Err(Failure::refused()));
        assert_eq!(known.check_put_back("4f1c9a2e", &["Documents/report.txt".into()], "--replace"), Err(Failure::refused()));
        assert_eq!(known.check_put_back("4f1c9a2e", &[], "replace"), Err(Failure::refused()));
        assert_eq!(known.check_put_back("ffffffff", &["Documents".into()], "replace"), Err(Failure::refused()));
    }

    #[test]
    fn versions_add_their_points() {
        let mut known = known_with(&["a1b2c3d4"], &[]);
        let versions = Versions {
            versions: vec![Version { id: "a1b2c3d4".into(), ..Default::default() }, Version { id: "e5f6a7b8".into(), source: "backup".into(), time: 3.0, ..Default::default() }],
            ..Default::default()
        };
        known.versions_found(&versions);
        assert!(known.has_point("e5f6a7b8"));
        assert_eq!(known.points["e5f6a7b8"].time, 3.0);
    }

    #[test]
    fn reads_the_arguments() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(page_argument(&args(&["status"])), Some("status"));
        assert_eq!(page_argument(&args(&["--at=x", "browse"])), Some("browse"));
        assert_eq!(page_argument(&args(&["Documents"])), None);
        assert_eq!(page_argument(&args(&[])), None);
        assert_eq!(point_argument(&args(&["--at=2026-10-04_1400"])), "2026-10-04_1400");
        assert_eq!(point_argument(&args(&["x", "--at=4f1c9a2e"])), "4f1c9a2e");
        assert_eq!(point_argument(&args(&["--at=4f1c9a2"])), "");
        assert_eq!(point_argument(&args(&["--at=2026-10-04_14:00"])), "");
        assert_eq!(point_argument(&args(&["--at=../etc"])), "");
        assert_eq!(point_argument(&args(&["--at=4F1C9A2E"])), "");

        let base = std::env::temp_dir().join(format!("amethystora-backups-test-{}", std::process::id()));
        let home = base.join("home");
        std::fs::create_dir_all(home.join("Documents/notes")).unwrap();
        std::fs::create_dir_all(base.join("elsewhere")).unwrap();
        std::fs::write(home.join("Documents/report.txt"), "x").unwrap();
        let home = std::fs::canonicalize(&home).unwrap();
        let folder = |list: &[&str], cwd: &Path| folder_argument(&args(list), cwd, &home);
        assert_eq!(folder(&[home.join("Documents").to_str().unwrap()], &base), Some("Documents".into()));
        assert_eq!(folder(&["Documents/notes"], &home), Some("Documents/notes".into()));
        assert_eq!(folder(&["notes/../notes/"], &home.join("Documents")), Some("Documents/notes".into()));
        // A file opens its folder; the home folder is ""
        assert_eq!(folder(&["Documents/report.txt"], &home), Some("Documents".into()));
        assert_eq!(folder(&[&format!("file://{}", home.display())], &base), Some(String::new()));
        assert_eq!(folder(&["--at=4f1c9a2e", "."], &home), Some(String::new()));
        // Outside the home, or not there at all: nothing
        assert_eq!(folder(&["elsewhere"], &base), None);
        assert_eq!(folder(&[".."], &home), None);
        assert_eq!(folder(&["Documents/nothing"], &home), None);
        assert_eq!(folder(&[], &home), None);
        // Earlier versions: a file inside the home, never the home itself, a path out of it, or a missing file
        let file = |list: &[&str], cwd: &Path| versions_argument(&args(list), cwd, &home);
        assert_eq!(file(&["--versions=Documents/report.txt"], &home), Some("Documents/report.txt".into()));
        assert_eq!(file(&[&format!("--versions={}", home.join("Documents/report.txt").display())], &base), Some("Documents/report.txt".into()));
        assert_eq!(file(&["--versions=."], &home), None);
        assert_eq!(file(&["--versions=../elsewhere"], &home), None);
        assert_eq!(file(&["--versions=Documents/nothing"], &home), None);
        assert_eq!(file(&["Documents/report.txt"], &home), None);
        // It is an option, so it is never taken for a folder or a page
        assert_eq!(folder(&["--versions=Documents/report.txt"], &home), None);
        assert_eq!(page_argument(&args(&["--versions=status"])), None);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn reads_the_units_state() {
        let state = unit_properties("ActiveState=activating\nResult=success\nInvocationID=abc\n");
        assert!(running(&state));
        assert_eq!(state["InvocationID"], "abc");
        assert!(!running(&unit_properties("ActiveState=failed\nResult=exit-code\n")));
        assert!(!running(&unit_properties("")));
    }

    #[test]
    fn keeps_the_end_of_what_was_said() {
        assert_eq!(tail("a\nb\nc\nd\ne\n", 4), "b\nc\nd\ne");
        assert_eq!(tail("  one  ", 4), "one");
        assert_eq!(tail("", 3), "");
    }
}
