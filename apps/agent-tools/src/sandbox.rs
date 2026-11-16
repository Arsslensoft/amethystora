//! An agent that can change its project and nothing else (plans/PLAN-agent-sandbox.md), and a profile's folders bound over
//! the home's for the agents with no variable of their own (plans/PLAN-agent-accounts.md, way 2): one recipe, built here and
//! nowhere else, printed by `ame agent sandbox show` and run by `amethystora-agent` exactly as printed.
//!
//! A sandbox starts from the whole machine and takes away: bubblewrap, with everything read-only, the project, the agent's
//! own folder and the build caches writable, the keys and logins hidden (an empty folder or file in their place), a /run, /tmp
//! and /dev of its own, its own processes, and a seccomp filter. A permission is a mount, nothing is a promise. It fails
//! closed: what cannot be set up stops the launch, which never starts plain instead.

use std::collections::BTreeSet;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicI32, Ordering};

use serde_json::{Value, json};

use crate::agents::{self, AgentPaths, Map, Table, Way, DEFAULT};
use crate::policy;

pub const BWRAP: &str = "/usr/bin/bwrap";
/// The filter build-apps.sh writes with libseccomp (seccomp/gen.c's agent mode): typing into the terminal, the kernel's
/// keyring, BPF and perf refused; namespaces of the agent's own allowed
pub const FILTER: &str = "/usr/share/amethystora/agent-sandbox.bpf";
pub const SANDBOX_VAR: &str = "AMETHYSTORA_AGENT_SANDBOX";
pub const PROJECT_VAR: &str = "AMETHYSTORA_AGENT_SANDBOX_PROJECT";
/// The descriptor the filter is handed to bubblewrap on, as the plugins' is
const FILTER_FD: i32 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// No sandbox: a profile's folders bound over the home's, and nothing else changed
    Profile,
    /// The home read-only, its secrets hidden
    Open,
    /// None of the home but the project, the agent's folder, the caches and the toolchains
    Closed,
}

impl Mode {
    pub fn word(self) -> &'static str {
        match self {
            Mode::Profile => "profile",
            Mode::Open => "open",
            Mode::Closed => "closed",
        }
    }

    pub fn parse(word: &str) -> Option<Mode> {
        match word {
            "open" => Some(Mode::Open),
            "closed" => Some(Mode::Closed),
            _ => None,
        }
    }
}

/// What a folder may be given beyond the default, each by the person in a terminal (ame agent sandbox allow)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Permission {
    /// A second place it may write: another repository, a path dependency built in place
    Write(PathBuf),
    /// A hidden place shown read-only: a kubeconfig for one project
    Show(PathBuf),
    /// The ssh agent's socket: it can then log in with every key the agent holds, without reading one
    SshAgent,
    /// The graphics card's devices
    Gpu,
}

impl Permission {
    /// A permission as the folder's rule keeps it, read strictly: a path is a real one, inside the home or the machine's
    /// own folders, never the home itself or a folder above it
    pub fn parse(text: &str, home: &Path) -> Result<Permission, String> {
        match text {
            "ssh-agent" => return Ok(Permission::SshAgent),
            "gpu" => return Ok(Permission::Gpu),
            _ => {}
        }
        let (kind, path) = text.split_once('=').ok_or_else(|| format!("{text} is not a permission: write=PATH, show=PATH, ssh-agent or gpu"))?;
        let path = match path.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None => PathBuf::from(path),
        };
        if !path.is_absolute() {
            return Err(format!("{} is not a whole path", path.display()));
        }
        let real = std::fs::canonicalize(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        if real == Path::new("/") || home.starts_with(&real) {
            return Err(format!("{} is the home or a folder above it, which a sandbox is never given", real.display()));
        }
        match kind {
            "write" => Ok(Permission::Write(real)),
            "show" => Ok(Permission::Show(real)),
            _ => Err(format!("{text} is not a permission: write=PATH, show=PATH, ssh-agent or gpu")),
        }
    }

    pub fn word(&self) -> String {
        match self {
            Permission::Write(path) => format!("write={}", path.display()),
            Permission::Show(path) => format!("show={}", path.display()),
            Permission::SshAgent => "ssh-agent".into(),
            Permission::Gpu => "gpu".into(),
        }
    }

    /// What it gives away, said before it is asked
    pub fn cost(&self) -> String {
        match self {
            Permission::Write(path) => format!("The agent can change everything in {}, as it can its project.", path.display()),
            Permission::Show(path) => format!("The agent can read {}, which is hidden from it otherwise, and send what it reads.", path.display()),
            Permission::SshAgent => "The agent can log in, as you, with every key your ssh agent holds (git push, ssh to servers), without reading the keys.".into(),
            Permission::Gpu => "The agent's programs can use the graphics card, as any program of yours can.".into(),
        }
    }
}

/// The project a launch works on: the top of the git repository it was made in, else the folder itself
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub top: PathBuf,
    /// The repository's own folder (.git, or a worktree's), and the folder it shares with its worktrees
    pub git: Option<PathBuf>,
    pub common: Option<PathBuf>,
}

/// A folder named in a .git file or commondir: relative to the folder it is written in
fn git_path(base: &Path, text: &str) -> Option<PathBuf> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let path = if Path::new(text).is_absolute() { PathBuf::from(text) } else { base.join(text) };
    std::fs::canonicalize(path).ok()
}

/// The project of a launch made in DIR, read without running git (a repository's own settings could run something): the
/// nearest folder up to the home (never the home's own repository, a dotfiles one) with a .git in it
pub fn project_of(dir: &Path, home: &Path) -> Result<Project, String> {
    let dir = std::fs::canonicalize(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let mut at = dir.as_path();
    loop {
        if at == home || at == Path::new("/") {
            break;
        }
        let dot = at.join(".git");
        if let Ok(meta) = std::fs::symlink_metadata(&dot) {
            let git = if meta.is_dir() {
                std::fs::canonicalize(&dot).ok()
            } else {
                std::fs::read_to_string(&dot).ok().and_then(|text| text.strip_prefix("gitdir:").and_then(|rest| git_path(at, rest)))
            };
            let common = git.as_ref().map(|git| {
                std::fs::read_to_string(git.join("commondir")).ok().and_then(|text| git_path(git, &text)).unwrap_or_else(|| git.clone())
            });
            return check(Project { top: at.to_path_buf(), git, common }, home);
        }
        let Some(up) = at.parent() else { break };
        at = up;
    }
    check(Project { top: dir.clone(), git: None, common: None }, home)
}

/// A project may not be the home, a folder above it, a dot-folder of the home, or a place a sandbox replaces
fn check(project: Project, home: &Path) -> Result<Project, String> {
    let top = &project.top;
    if home.starts_with(top) {
        return Err(format!("{} is your home or a folder above it: start the agent in a project's folder", top.display()));
    }
    if let Ok(inside) = top.strip_prefix(home) {
        if inside.components().next().is_some_and(|first| first.as_os_str().to_string_lossy().starts_with('.')) {
            return Err(format!("{} is in a hidden folder of your home, which is not a project", top.display()));
        }
    }
    for place in ["/tmp", "/var/tmp", "/run", "/proc", "/sys", "/dev", "/usr", "/etc", "/boot"] {
        if top.starts_with(place) {
            return Err(format!("{} is in {place}, which a sandbox has its own of", top.display()));
        }
    }
    Ok(project)
}

/// What a launch asks for
#[derive(Debug, Clone)]
pub struct Request {
    pub agent: String,
    pub profile: String,
    pub mode: Mode,
    pub project: Option<Project>,
    pub allow: Vec<Permission>,
    pub home: PathBuf,
    pub uid: u32,
    /// The launch's own folder: its /tmp, its copies
    pub run: PathBuf,
    /// Where the agent starts
    pub dir: PathBuf,
}

/// Something made before the launch, where it is not there already
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Make {
    /// A folder kept afterwards (an agent's own, and its empty settings folders)
    Dir(PathBuf),
    /// A file with its default contents, kept afterwards (an agent's settings file, empty)
    File(PathBuf, &'static str),
    /// An empty folder in the project, taken away afterwards if it is still empty
    Placeholder(PathBuf),
}

/// One recipe: bubblewrap's options and the environment, and what to make first
#[derive(Debug, Clone, Default)]
pub struct Recipe {
    pub args: Vec<String>,
    pub env: Vec<(String, Option<String>)>,
    pub make: Vec<Make>,
    /// Files copied into the launch's folder, each bound over the original: what the agent writes there is never written back
    pub copies: Vec<(PathBuf, PathBuf)>,
    /// Files in the project that are not there, which a write would make: looked at when the launch ends
    pub watched: Vec<PathBuf>,
    pub hidden: Vec<PathBuf>,
    pub writable: Vec<PathBuf>,
}

/// Variables of the session's sockets, which a sandbox never passes on
const SOCKETS: &[&str] = &[
    "DBUS_SESSION_BUS_ADDRESS", "DBUS_SYSTEM_BUS_ADDRESS", "SSH_AUTH_SOCK", "SSH_AGENT_PID", "GPG_AGENT_INFO", "GNUPGHOME",
    "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DISPLAY", "XAUTHORITY", "XAUTHLOCALHOSTNAME", "PULSE_SERVER", "PIPEWIRE_REMOTE",
    "DOCKER_HOST", "CONTAINER_HOST", "KRB5CCNAME", "SESSION_MANAGER", "GNOME_KEYRING_CONTROL", "KITTY_LISTEN_ON", "TMUX",
    "NOTIFY_SOCKET", "LISTEN_FDS", "LISTEN_PID", "LISTEN_FDNAMES", "INVOCATION_ID", "JOURNAL_STREAM", "SYSTEMD_EXEC_PID",
    "MANAGERPID", "ATSPI_BUS_ADDRESS", "AT_SPI_BUS_ADDRESS", "VAULT_ADDR",
];

/// Whether a variable's name says it holds a secret, which a sandbox never passes on but for the agent's own
pub fn secret_name(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    ["TOKEN", "SECRET", "PASSWORD", "PASSWD", "CREDENTIAL", "COOKIE", "PRIVATE"].iter().any(|word| name.contains(word))
        || name.ends_with("_KEY")
        || name.ends_with("APIKEY")
        || name.contains("_KEY_")
        || name.starts_with("AWS_")
}

fn s(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// A path's contents for a file made where an agent's settings file is not there: what each reads as nothing set
fn default_contents(path: &Path) -> Option<&'static str> {
    let name = path.file_name()?.to_string_lossy().to_string();
    if name.ends_with(".json") || name.ends_with(".jsonc") {
        Some("{}\n")
    } else if name.contains('.') && !name.starts_with('.') {
        Some("")
    } else {
        None
    }
}

/// The paths of a list that are not inside another one of it, so that no mount is made under a folder already hidden
fn outermost(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let sorted: BTreeSet<PathBuf> = paths.into_iter().collect();
    let mut kept: Vec<PathBuf> = Vec::new();
    for path in sorted {
        if !kept.iter().any(|outer| path.starts_with(outer)) {
            kept.push(path);
        }
    }
    kept
}

/// The agent's folders for this launch: the profile's copy of each, at the place the agent will look for it
fn agent_folders(table: &Table, request: &Request) -> Result<Vec<(PathBuf, PathBuf)>, String> {
    let paths = table.agent(&request.agent)?;
    let root = agents::profile_root(&request.home, &request.profile);
    Ok(paths
        .dirs
        .iter()
        .map(|dir| {
            let source = root.join(dir);
            // A profile of way 1 is looked for where its variable points (inside the profile); of way 2, over the home's
            let dest = match (&paths.way, request.profile.as_str()) {
                (_, DEFAULT) | (Some(Way::Variable { .. }), _) => source.clone(),
                _ => request.home.join(dir),
            };
            (source, dest)
        })
        .collect())
}

/// The recipe of a launch. Pure but for looking at what is there: nothing is made, so that `show` prints what `run` would run.
pub fn recipe(table: &Table, request: &Request) -> Result<Recipe, String> {
    let home = &request.home;
    let paths: &AgentPaths = table.agent(&request.agent)?;
    let mut recipe = Recipe::default();
    let folders = agent_folders(table, request)?;
    if request.profile != DEFAULT && !table.has_profiles(&request.agent) {
        return Err(format!("{} has no profiles: it keeps its login in the keyring, which no folder isolates", agents::title(&request.agent)));
    }
    let args = &mut recipe.args;
    let mut push = |words: &[&str]| args.extend(words.iter().map(|word| word.to_string()));

    if request.mode == Mode::Profile {
        // Way 2: the machine as it is, the profile's folders over the home's. Nothing setuid runs inside (no sudo)
        push(&["--unshare-user", "--dev-bind", "/", "/", "--die-with-parent"]);
        for (source, dest) in &folders {
            recipe.make.push(Make::Dir(source.clone()));
            recipe.make.push(Make::Dir(dest.clone()));
            recipe.args.extend(["--bind".into(), s(source), s(dest)]);
            recipe.writable.push(dest.clone());
        }
        recipe.args.extend(["--chdir".into(), s(&request.dir)]);
        recipe.env = agents::environment(table, home, &request.agent, &request.profile);
        return Ok(recipe);
    }

    let project = request.project.as_ref().ok_or("a sandbox needs a project")?;
    let runtime = format!("/run/user/{}", request.uid);
    let tmp = request.run.join("tmp");
    let vartmp = request.run.join("var-tmp");
    // Its own processes, IPC and user namespace (in which nothing is setuid); the machine's network
    push(&["--unshare-user", "--unshare-pid", "--unshare-ipc", "--unshare-cgroup-try", "--die-with-parent"]);
    push(&["--ro-bind", "/", "/", "--dev", "/dev", "--tmpfs", "/dev/shm", "--proc", "/proc"]);
    // No bus, no display, no keyring, no agent: an empty /run but for the resolver, and a runtime folder of its own
    push(&["--tmpfs", "/run", "--ro-bind-try", "/run/systemd/resolve", "/run/systemd/resolve", "--perms", "0700", "--dir", &runtime]);
    recipe.args.extend(["--bind".into(), s(&tmp), "/tmp".into(), "--bind".into(), s(&vartmp), "/var/tmp".into()]);

    // Hidden: the machine's own secret places always; in an open sandbox the home's secrets, the other agents' folders and
    // the other profiles (all of them: this launch's own is bound in again below)
    let mut hide: Vec<PathBuf> = table.hidden.iter().filter(|path| path.starts_with('/')).map(PathBuf::from).collect();
    let mut remount: Vec<PathBuf> = Vec::new();
    if request.mode == Mode::Closed {
        recipe.args.extend(["--tmpfs".into(), s(home)]);
        remount.push(home.clone());
    } else {
        hide.extend(table.hidden.iter().filter(|path| !path.starts_with('/')).map(|path| home.join(path)));
        for glob in agents::credential_globs() {
            hide.extend(agents::expand(home, &glob));
        }
        for (other, other_paths) in &table.agents {
            if other != &request.agent {
                hide.extend(other_paths.dirs.iter().map(|dir| home.join(dir)));
            }
        }
        // On a profile of way 1, the home's own folders and files of this agent are the home's profile, another account
        if request.profile != DEFAULT && matches!(paths.way, Some(Way::Variable { .. })) {
            hide.extend(paths.dirs.iter().chain(paths.copies.iter()).map(|path| home.join(path)));
        }
    }
    // A shown place stays out of what is hidden: it is bound read-only below
    let shown: Vec<PathBuf> = request.allow.iter().filter_map(|permission| match permission {
        Permission::Show(path) => Some(path.clone()),
        _ => None,
    }).collect();
    let hide: Vec<PathBuf> = hide.into_iter().filter(|path| std::fs::symlink_metadata(path).is_ok()).collect();
    for path in outermost(hide) {
        let Ok(meta) = std::fs::metadata(&path) else { continue };
        if project.top.starts_with(&path) {
            return Err(format!("{} is in a hidden place, which is never a project", project.top.display()));
        }
        if meta.is_dir() {
            recipe.args.extend(["--tmpfs".into(), s(&path)]);
            remount.push(path.clone());
        } else {
            recipe.args.extend(["--ro-bind".into(), "/dev/null".into(), s(&path)]);
        }
        recipe.hidden.push(path);
    }

    // Writable: the project (and a worktree's repository), the agent's folder, the caches that are there, and what was allowed
    let mut writable = vec![project.top.clone()];
    if let Some(common) = &project.common {
        if !common.starts_with(&project.top) {
            writable.push(common.clone());
        }
    }
    for (source, dest) in &folders {
        recipe.make.push(Make::Dir(source.clone()));
        if source != dest {
            recipe.make.push(Make::Dir(dest.clone()));
        }
    }
    for cache in &table.caches {
        let path = home.join(cache);
        if path.is_dir() {
            writable.push(path);
        }
    }
    for permission in &request.allow {
        if let Permission::Write(path) = permission {
            writable.push(path.clone());
        }
    }
    for path in &writable {
        if recipe.hidden.iter().any(|hidden| path.starts_with(hidden)) && !path.starts_with(home.join(agents::PROFILES)) {
            return Err(format!("{} is a hidden place, which a sandbox never makes writable", path.display()));
        }
        recipe.args.extend(["--bind".into(), s(path), s(path)]);
    }
    for (source, dest) in &folders {
        recipe.args.extend(["--bind".into(), s(source), s(dest)]);
        writable.push(dest.clone());
    }
    recipe.writable = writable;

    // Closed: the toolchains, read-only
    if request.mode == Mode::Closed {
        for tool in &table.tools {
            let path = home.join(tool);
            if std::fs::symlink_metadata(&path).is_ok() {
                recipe.args.extend(["--ro-bind".into(), s(&path), s(&path)]);
            }
        }
    }

    // Read-only inside what is writable: what a command outside would run later. The repository's hooks and settings
    // (a hook, core.fsmonitor, core.sshCommand or an alias written there runs at the developer's next git command), its
    // submodules' and worktrees' too, and every agent's settings in the project
    // Each repository folder is a mount of its own, so that it cannot be moved away and made again with settings of the agent's
    let mut gits: Vec<PathBuf> = project.git.iter().chain(project.common.iter()).cloned().collect();
    gits.sort();
    gits.dedup();
    let mut read_only: Vec<PathBuf> = Vec::new();
    for git in &gits {
        recipe.args.extend(["--bind".into(), s(git), s(git)]);
        for name in ["hooks", "config", "config.worktree", "info/attributes"] {
            read_only.push(git.join(name));
        }
        if let Ok(modules) = walk_modules(&git.join("modules")) {
            read_only.extend(modules);
        }
    }
    // A worktree's .git is a file naming its repository: another name would be another repository's hooks
    let dot_git = project.top.join(".git");
    if std::fs::symlink_metadata(&dot_git).is_ok_and(|meta| meta.is_file()) {
        read_only.push(dot_git);
    }
    for path in dedup(read_only) {
        match std::fs::symlink_metadata(&path) {
            Ok(_) => recipe.args.extend(["--ro-bind".into(), s(&path), s(&path)]),
            Err(_) if path.file_name().is_some_and(|name| name == "hooks") => {
                recipe.make.push(Make::Placeholder(path.clone()));
                recipe.args.extend(["--ro-bind".into(), s(&path), s(&path)]);
            }
            // A file git reads only when the read-only settings say so
            Err(_) => {}
        }
    }
    let mut lines: Vec<PathBuf> = table.project.iter().chain(table.agents.values().flat_map(|paths| paths.project.iter())).map(|line| project.top.join(line)).collect();
    lines = dedup(lines);
    for path in lines {
        if std::fs::symlink_metadata(&path).is_ok() {
            recipe.args.extend(["--ro-bind".into(), s(&path), s(&path)]);
        } else if is_folder_line(&path) {
            // A settings folder that is not there is made empty, read-only, and taken away afterwards if it still is
            recipe.make.push(Make::Placeholder(path.clone()));
            recipe.args.extend(["--ro-bind".into(), s(&path), s(&path)]);
        } else {
            // A file would be made where every agent can see it (git status): it is looked at when the launch ends instead
            recipe.watched.push(path);
        }
    }
    // The agent's own settings, commands and hooks, read-only, made empty first where they are not there
    let root = agents::profile_root(home, &request.profile);
    let place_of = |line: &str| -> PathBuf {
        let path = root.join(line);
        // A way-2 profile's copy is seen at the home's place
        for (source, dest) in &folders {
            if let Ok(rest) = path.strip_prefix(source) {
                return dest.join(rest);
            }
        }
        path
    };
    for line in &paths.ro {
        let (source, place) = (root.join(line), place_of(line));
        if std::fs::symlink_metadata(&source).is_err() {
            recipe.make.push(match default_contents(&source) {
                Some(contents) => Make::File(source.clone(), contents),
                None => Make::Dir(source.clone()),
            });
        }
        // bubblewrap reads a bind's source from the machine's own tree: the profile's file, put where the agent looks
        recipe.args.extend(["--ro-bind".into(), s(&source), s(&place)]);
    }
    // The files it rewrites that name commands: a private copy each, which goes with the launch
    for (number, line) in paths.copies.iter().enumerate() {
        let (source, place) = (root.join(line), place_of(line));
        // Only where the agent would look: for an agent whose variable moves its folder, the home's file on the home's
        // profile and the folder's own on another (Claude Code's ~/.claude.json, or .claude.json inside CLAUDE_CONFIG_DIR)
        let looked_at = match &paths.way {
            Some(Way::Variable { dir, .. }) => (request.profile == DEFAULT) != Path::new(line).starts_with(dir),
            _ => true,
        };
        if !looked_at {
            continue;
        }
        if std::fs::symlink_metadata(&source).is_err() {
            recipe.make.push(Make::File(source.clone(), "{}\n"));
        }
        let copy = request.run.join("copies").join(number.to_string());
        recipe.copies.push((source, copy.clone()));
        recipe.args.extend(["--bind".into(), s(&copy), s(&place)]);
    }

    // The agent's own program, where its installer keeps it in its folder: read-only, since the next plain launch runs it, and
    // there even when the folder is hidden (on another profile)
    for line in &paths.programs {
        let path = home.join(line);
        if std::fs::symlink_metadata(&path).is_ok() {
            recipe.args.extend(["--ro-bind".into(), s(&path), s(&path)]);
        }
    }

    // What the folder was allowed beyond the default
    let mut env = agents::environment(table, home, &request.agent, &request.profile);
    for path in &shown {
        recipe.args.extend(["--ro-bind".into(), s(path), s(path)]);
    }
    for permission in &request.allow {
        match permission {
            Permission::SshAgent => {
                let socket = std::env::var("SSH_AUTH_SOCK").map_err(|_| "ssh-agent was allowed, but no ssh agent runs (SSH_AUTH_SOCK)".to_string())?;
                let inside = format!("{runtime}/ssh-agent");
                recipe.args.extend(["--bind".into(), socket, inside.clone()]);
                env.push(("SSH_AUTH_SOCK".into(), Some(inside)));
            }
            Permission::Gpu => recipe.args.extend(["--dev-bind-try".into(), "/dev/dri".into(), "/dev/dri".into()]),
            _ => {}
        }
    }
    for path in remount {
        recipe.args.extend(["--remount-ro".into(), s(&path)]);
    }
    recipe.args.extend(["--seccomp".into(), FILTER_FD.to_string(), "--chdir".into(), s(&request.dir)]);

    // The environment: the session's sockets and every secret-looking variable out, but for the agent's own
    for (name, _) in std::env::vars_os().filter_map(|(name, value)| Some((name.into_string().ok()?, value))) {
        let kept = paths.env.contains(&name) || (name == "SSH_AUTH_SOCK" && request.allow.contains(&Permission::SshAgent));
        if !kept && (SOCKETS.contains(&name.as_str()) || secret_name(&name)) {
            env.push((name, None));
        }
    }
    if std::env::var("TMPDIR").is_ok_and(|dir| !dir.starts_with("/tmp") && !dir.starts_with("/var/tmp")) {
        env.push(("TMPDIR".into(), None));
    }
    env.push(("XDG_RUNTIME_DIR".into(), Some(runtime)));
    env.push((SANDBOX_VAR.into(), Some(request.mode.word().into())));
    env.push((PROJECT_VAR.into(), Some(s(&project.top))));
    // Amethystora's own tools, started inside with the agent, only read: a sandboxed agent works on a project, it does not
    // administer the machine
    env.push(("AMETHYSTORA_READ_ONLY".into(), Some("1".into())));
    recipe.env = env;
    Ok(recipe)
}

/// Whether a project line names a folder (an agent's settings folder: a dot and no other dot) rather than a file
fn is_folder_line(path: &Path) -> bool {
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    name.starts_with('.') && !name[1..].contains('.') && name != ".envrc"
}

fn dedup(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    paths.into_iter().filter(|path| seen.insert(path.clone())).collect()
}

/// A repository's submodules keep a repository of their own, with its hooks and settings, under modules/
fn walk_modules(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), 0)];
    while let Some((at, depth)) = stack.pop() {
        if depth > 6 || found.len() > 400 {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&at) else { continue };
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let path = entry.path();
            if path.join("config").is_file() && path.join("HEAD").exists() {
                found.extend([path.join("hooks"), path.join("config")]);
                stack.push((path.join("modules"), depth + 1));
            } else {
                stack.push((path, depth + 1));
            }
        }
    }
    Ok(found)
}

// --- Running it --------------------------------------------------------------------------------------------------------

/// The launch's folder, in the account's cache, removed when it ends; one a launch that was killed left is taken away by the next
pub fn runs_dir(home: &Path) -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME").filter(|dir| !dir.is_empty()).map(PathBuf::from).unwrap_or_else(|| home.join(".cache"));
    base.join("amethystora/agent-sandbox")
}

fn sweep(home: &Path) {
    let Ok(entries) = std::fs::read_dir(runs_dir(home)) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let pid = name.split('-').next().unwrap_or("");
        if pid.bytes().all(|b| b.is_ascii_digit()) && !Path::new("/proc").join(pid).exists() {
            take_back(&entry.path());
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// The empty folders a launch made in the project, as it noted them, taken away while still empty
fn take_back(run: &Path) {
    for line in std::fs::read_to_string(run.join("made")).unwrap_or_default().lines() {
        let _ = std::fs::remove_dir(line);
    }
}

/// Makes what the recipe needs: its folders and files where they are not there, the launch's folder and its copies
pub fn prepare(recipe: &Recipe, run: &Path) -> Result<(), String> {
    let folder = |path: &Path| std::fs::DirBuilder::new().recursive(true).mode(0o700).create(path).map_err(|error| format!("{}: {error}", path.display()));
    folder(&run.join("tmp"))?;
    folder(&run.join("var-tmp"))?;
    folder(&run.join("copies"))?;
    std::fs::set_permissions(run.join("tmp"), std::os::unix::fs::PermissionsExt::from_mode(0o1777)).map_err(|error| error.to_string())?;
    let mut made = String::new();
    for make in &recipe.make {
        match make {
            Make::Dir(path) => folder(path)?,
            Make::Placeholder(path) => {
                if std::fs::symlink_metadata(path).is_err() {
                    folder(path)?;
                    made.push_str(&format!("{}\n", path.display()));
                }
            }
            Make::File(path, contents) => {
                if std::fs::symlink_metadata(path).is_err() {
                    if let Some(parent) = path.parent() {
                        folder(parent)?;
                    }
                    let mut file = std::fs::OpenOptions::new().create_new(true).write(true).mode(0o600).open(path)
                        .map_err(|error| format!("{}: {error}", path.display()))?;
                    std::io::Write::write_all(&mut file, contents.as_bytes()).map_err(|error| error.to_string())?;
                }
            }
        }
    }
    std::fs::write(run.join("made"), made).map_err(|error| error.to_string())?;
    for (source, copy) in &recipe.copies {
        std::fs::copy(source, copy).map_err(|error| format!("{}: {error}", source.display()))?;
        std::fs::set_permissions(copy, std::os::unix::fs::PermissionsExt::from_mode(0o600)).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// The command line, as a shell would need it, for `show`
pub fn command_line(recipe: &Recipe, command: &[String]) -> String {
    let mut words = vec![BWRAP.to_string()];
    words.extend(recipe.args.iter().cloned());
    words.push("--".into());
    words.extend(command.iter().cloned());
    words.iter().map(|word| crate::history::quote(word)).collect::<Vec<_>>().join(" ")
}

static CHILD: AtomicI32 = AtomicI32::new(0);

// Only what a signal handler may do: the person's Stop (SIGTERM to this runner) ends the sandbox's first process, which takes
// everything in it (bubblewrap's --die-with-parent)
extern "C" fn pass_on(signal: libc::c_int) {
    let child = CHILD.load(Ordering::SeqCst);
    if child > 0 {
        unsafe { libc::kill(child, signal) };
    }
}

/// The session's file in the account's runtime folder, for the pill, `ame agent profile running` and Stop: the tool server
/// inside a sandbox cannot see that folder, so the runner writes it, under its own process number
fn session_file(row: &Value) -> Option<PathBuf> {
    let dir = crate::history::sessions_dir();
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir).ok()?;
    let path = dir.join(crate::history::host_pid().to_string());
    let mut file = std::fs::OpenOptions::new().create(true).truncate(true).write(true).mode(0o600).open(&path).ok()?;
    std::io::Write::write_all(&mut file, format!("{row}\n").as_bytes()).ok()?;
    Some(path)
}

/// Runs COMMAND under the recipe: bubblewrap as a child, waited for, so that the launch's folder, the empty folders it made
/// and its session's file go when it ends. Its exit is the command's.
pub fn run(recipe: &Recipe, request: &Request, command: &[String]) -> Result<i32, String> {
    sweep(&request.home);
    if !Path::new(BWRAP).exists() {
        return Err("bubblewrap is not there, so the agent cannot be held in a sandbox".into());
    }
    let filter = if request.mode == Mode::Profile {
        None
    } else {
        Some(std::fs::File::open(FILTER).map_err(|error| format!("the sandbox's filter {FILTER} cannot be read ({error}), so nothing starts"))?)
    };
    prepare(recipe, &request.run).inspect_err(|_| {
        take_back(&request.run);
        let _ = std::fs::remove_dir_all(&request.run);
    })?;
    let session = (request.mode != Mode::Profile).then(|| {
        session_file(&json!({"pid": crate::history::host_pid(), "started": crate::history::now(), "agent": request.agent, "profile": request.profile,
            "sandboxed": request.mode.word(), "project": request.project.as_ref().map(|project| s(&project.top)), "now": null, "recent": []}))
    }).flatten();
    let watched: Vec<PathBuf> = recipe.watched.iter().filter(|path| std::fs::symlink_metadata(path).is_err()).cloned().collect();
    let mut child = Command::new(BWRAP);
    child.args(&recipe.args).arg("--").args(command);
    for (name, value) in &recipe.env {
        match value {
            Some(value) => child.env(name, value),
            None => child.env_remove(name),
        };
    }
    if let Some(filter) = &filter {
        let fd = std::os::fd::AsRawFd::as_raw_fd(filter);
        unsafe {
            child.pre_exec(move || {
                if libc::dup2(fd, FILTER_FD) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    let spawned = child.spawn();
    // Ctrl+C and a closed terminal are the agent's to answer: this runner waits for it, and cleans up after. Ignored only once
    // the child is started, which would otherwise inherit the ignoring
    for signal in [libc::SIGINT, libc::SIGQUIT, libc::SIGHUP] {
        unsafe { libc::signal(signal, libc::SIG_IGN) };
    }
    let code = match spawned {
        Ok(mut process) => {
            CHILD.store(process.id() as i32, Ordering::SeqCst);
            unsafe { libc::signal(libc::SIGTERM, pass_on as extern "C" fn(libc::c_int) as libc::sighandler_t) };
            let status = process.wait();
            CHILD.store(0, Ordering::SeqCst);
            status.map(|status| status.code().unwrap_or_else(|| 128 + std::os::unix::process::ExitStatusExt::signal(&status).unwrap_or(1))).unwrap_or(1)
        }
        Err(error) => {
            take_back(&request.run);
            let _ = std::fs::remove_dir_all(&request.run);
            if let Some(path) = &session {
                let _ = std::fs::remove_file(path);
            }
            return Err(format!("bubblewrap did not start: {error}"));
        }
    };
    take_back(&request.run);
    let _ = std::fs::remove_dir_all(&request.run);
    if let Some(path) = &session {
        let _ = std::fs::remove_file(path);
    }
    // A settings file the sandbox made in the project is the next plain launch's to read: said, never run or removed
    for path in watched.iter().filter(|path| std::fs::symlink_metadata(path).is_ok()) {
        eprintln!("The sandboxed agent made {}, which an agent reads when it next starts in this project, outside a sandbox: read it first.", path.display());
    }
    Ok(code)
}

// --- What a launch gets ----------------------------------------------------------------------------------------------------

/// What `agent-run` was asked for
#[derive(Debug, Clone, Default)]
pub struct Asked {
    pub agent: String,
    pub profile: Option<String>,
    /// open, closed, no, or None for the folder's rule
    pub sandbox: Option<String>,
    pub dir: Option<PathBuf>,
}

/// The decision for one launch, worked out when it starts and belonging to it alone
#[derive(Debug, Clone)]
pub struct Plan {
    pub request: Request,
    pub why: &'static str,
    /// Whether anything is wrapped: a sandbox, or a way-2 profile
    pub wrapped: bool,
}

impl Plan {
    pub fn json(&self, table: &Table) -> Value {
        let request = &self.request;
        let login = table.agents.get(&request.agent).and_then(|paths| paths.logins.first()).map(|login| {
            let root = agents::profile_root(&request.home, &request.profile);
            s(root.join(login).strip_prefix(&request.home).unwrap_or(Path::new(login)))
        });
        json!({
            "agent": request.agent,
            "profile": request.profile,
            "why": self.why,
            "sandbox": (request.mode != Mode::Profile).then(|| request.mode.word()),
            "project": request.project.as_ref().map(|project| s(&project.top)),
            "allow": request.allow.iter().map(Permission::word).collect::<Vec<_>>(),
            "wrapped": self.wrapped,
            "login": login,
            "env": agents::environment(table, &request.home, &request.agent, &request.profile).into_iter()
                .filter_map(|(name, value)| value.map(|value| (name, Value::String(value)))).collect::<serde_json::Map<_, _>>(),
        })
    }
}

/// The folder's sandbox rule, if it has one: its mode and what it was allowed
pub fn rule_of(home: &Path, dir: &Path) -> Result<Option<(String, Mode, Vec<Permission>)>, String> {
    let map = Map::read(&agents::sandbox_map_path())?;
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let Some((folder, rule)) = map.find(&dir) else { return Ok(None) };
    if rule.get("mode").and_then(Value::as_str) == Some("off") {
        return Ok(None);
    }
    let mode = rule.get("mode").and_then(Value::as_str).and_then(Mode::parse).unwrap_or(Mode::Open);
    let allow = rule
        .get("allow")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_str).map(|text| Permission::parse(text, home)).collect::<Result<Vec<_>, _>>())
        .transpose()
        .map_err(|why| format!("{}: {why}", agents::sandbox_map_path().display()))?
        .unwrap_or_default();
    Ok(Some((folder.to_string(), mode, allow)))
}

/// Works out a launch: its profile, whether it is sandboxed and how, from what was asked, the terminal's pin, the folder's
/// rules and the administrator's SANDBOX
pub fn plan(table: &Table, asked: &Asked) -> Result<Plan, String> {
    let home = agents::real_home();
    if !agents::agent_name(&asked.agent) && asked.agent != agents::NO_AGENT {
        return Err(format!("{} is not an agent amethystora-agent offers", asked.agent));
    }
    let dir = match &asked.dir {
        Some(dir) => dir.clone(),
        None => std::env::current_dir().map_err(|error| error.to_string())?,
    };
    let dir = std::fs::canonicalize(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let pin = std::env::var(agents::PIN).ok();
    let (profile, why) = if asked.agent == agents::NO_AGENT {
        (DEFAULT.to_string(), "no agent's")
    } else {
        agents::which(&home, asked.profile.as_deref(), pin.as_deref(), &dir)?
    };
    if profile != DEFAULT && !table.has_profiles(&asked.agent) {
        return Err(format!("{} has no profiles: it keeps its login in the keyring, which no folder isolates", agents::title(&asked.agent)));
    }
    let always = policy::Conf::read(Path::new(policy::CONF)).sandbox_always;
    let rule = rule_of(&home, &dir)?;
    let has_project = project_of(&dir, &home).is_ok();
    let (mode, allow) = match asked.sandbox.as_deref() {
        Some("no") if always && has_project => {
            return Err("an administrator has every agent started in a project sandboxed on this machine (SANDBOX=always)".into());
        }
        Some("no") => (Mode::Profile, Vec::new()),
        Some(word) => {
            let mode = Mode::parse(word).ok_or_else(|| format!("{word} is not a sandbox: open or closed"))?;
            (mode, rule.map(|(_, _, allow)| allow).unwrap_or_default())
        }
        None => match rule {
            Some((_, mode, allow)) => (mode, allow),
            None if always && has_project => (Mode::Open, Vec::new()),
            None => (Mode::Profile, Vec::new()),
        },
    };
    let project = if mode == Mode::Profile { None } else { Some(project_of(&dir, &home)?) };
    let wrapped = mode != Mode::Profile || (profile != DEFAULT && matches!(table.agent(&asked.agent)?.way, Some(Way::Mount)));
    let run = runs_dir(&home).join(format!("{}-{}", std::process::id(), crate::history::new_id()));
    Ok(Plan {
        request: Request { agent: asked.agent.clone(), profile, mode, project, allow, home, uid: unsafe { libc::getuid() }, run, dir },
        why,
        wrapped,
    })
}

/// What the person is told at the start of a sandboxed launch: the project, what is hidden, that the network is open
pub fn say(plan: &Plan) -> Vec<String> {
    let request = &plan.request;
    let Some(project) = &request.project else { return Vec::new() };
    let tilde = |path: &Path| match path.strip_prefix(&request.home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    };
    let mut lines = vec![format!(
        "{} is sandboxed{} to {}: it can change this project and nothing else.",
        agents::title(&request.agent),
        if request.mode == Mode::Closed { ", closed," } else { "" },
        tilde(&project.top)
    )];
    lines.push(if request.mode == Mode::Closed {
        "It sees nothing of your home but the project, its own folder, the build caches and your toolchains.".into()
    } else {
        "Your keys and logins, the other agents' folders and the bus are hidden from it; the rest of your home it can only read.".into()
    });
    lines.push("The network is open: it can still send what it reads. Nothing in it can become root.".into());
    for permission in &request.allow {
        lines.push(format!("This folder allows it more: {}", permission.word()));
    }
    lines
}

/// `amethystora-agent-tools agent-run --agent A [--profile P] [--sandbox open|closed|no] [--dir D] [--quiet] -- COMMAND...`
pub fn agent_run(args: &[String]) -> i32 {
    let Some(split) = args.iter().position(|arg| arg == "--") else {
        eprintln!("usage: amethystora-agent-tools agent-run --agent AGENT [--profile NAME] [--sandbox open|closed|no] [--dir DIR] -- COMMAND...");
        return 64;
    };
    let (options, command) = (&args[..split], &args[split + 1..]);
    if command.is_empty() {
        eprintln!("agent-run: there is no command");
        return 64;
    }
    let asked = match asked_of(options) {
        Ok(asked) => asked,
        Err(why) => {
            eprintln!("{why}");
            return 64;
        }
    };
    let quiet = options.iter().any(|arg| arg == "--quiet");
    let table = match Table::load() {
        Ok(table) => table,
        Err(why) => {
            eprintln!("{why}");
            return 70;
        }
    };
    let plan = match plan(&table, &asked) {
        Ok(plan) => plan,
        Err(why) => {
            eprintln!("{why}");
            return 2;
        }
    };
    let words = agents::session_words(&json!({"agent": plan.request.agent, "profile": plan.request.profile,
        "sandboxed": (plan.request.mode != Mode::Profile).then(|| plan.request.mode.word())}));
    // The terminal's title says the account and the sandbox of the agent running in it, fixed at its start
    if unsafe { libc::isatty(2) } == 1 {
        eprint!("\x1b]2;{}\x07", words.replace(['\x1b', '\x07'], ""));
        if !quiet {
            for line in say(&plan) {
                eprintln!("{line}");
            }
        }
    }
    if !plan.wrapped {
        // A profile of way 1, or the home's: nothing to wrap, only the environment
        let mut process = Command::new(&command[0]);
        process.args(&command[1..]);
        for (name, value) in agents::environment(&table, &plan.request.home, &plan.request.agent, &plan.request.profile) {
            match value {
                Some(value) => process.env(name, value),
                None => process.env_remove(name),
            };
        }
        let error = process.exec();
        eprintln!("{}: {error}", command[0]);
        return 127;
    }
    let recipe = match recipe(&table, &plan.request) {
        Ok(recipe) => recipe,
        Err(why) => {
            eprintln!("{why}");
            return 2;
        }
    };
    match run(&recipe, &plan.request, command) {
        Ok(code) => code,
        Err(why) => {
            eprintln!("{why}");
            2
        }
    }
}

fn asked_of(options: &[String]) -> Result<Asked, String> {
    let mut asked = Asked::default();
    let mut at = 0;
    while at < options.len() {
        let value = || options.get(at + 1).cloned().ok_or_else(|| format!("{} needs a value", options[at]));
        match options[at].as_str() {
            "--agent" => asked.agent = value()?,
            "--profile" => asked.profile = Some(value()?),
            "--sandbox" => asked.sandbox = Some(value()?),
            "--dir" => asked.dir = Some(PathBuf::from(value()?)),
            "--quiet" => {
                at += 1;
                continue;
            }
            other => return Err(format!("agent-run: unknown option {other}")),
        }
        at += 2;
    }
    if asked.agent.is_empty() {
        return Err("agent-run needs --agent".into());
    }
    Ok(asked)
}

/// `amethystora-agent-tools agent-plan` (the same options): what a launch would get, as JSON, for the launcher; and `sandbox show`
pub fn agent_plan(args: &[String], show: bool) -> i32 {
    let options: Vec<String> = args.iter().take_while(|arg| *arg != "--").cloned().collect();
    let command: Vec<String> = args.iter().skip_while(|arg| *arg != "--").skip(1).cloned().collect();
    let json_out = options.iter().any(|arg| arg == "--json");
    let options: Vec<String> = options.into_iter().filter(|arg| arg != "--json").collect();
    let result = asked_of(&options).and_then(|asked| {
        let table = Table::load()?;
        let plan = plan(&table, &asked)?;
        Ok((table, plan))
    });
    let (table, plan) = match result {
        Ok(found) => found,
        Err(why) => {
            if json_out {
                println!("{}", json!({"error": why}));
            } else {
                eprintln!("{why}");
            }
            return 2;
        }
    };
    if !show {
        println!("{}", plan.json(&table));
        return 0;
    }
    let mut value = plan.json(&table);
    let recipe = if plan.wrapped { Some(recipe(&table, &plan.request)) } else { None };
    if let Some(Err(why)) = &recipe {
        value["refused"] = json!(why);
    }
    if let Some(Ok(recipe)) = &recipe {
        let command = if command.is_empty() { vec![plan.request.agent.clone()] } else { command };
        value["hidden"] = json!(recipe.hidden.iter().map(|path| s(path)).collect::<Vec<_>>());
        value["writable"] = json!(recipe.writable.iter().map(|path| s(path)).collect::<Vec<_>>());
        value["command"] = json!(command_line(recipe, &command));
    }
    if json_out {
        println!("{value:#}");
        return 0;
    }
    for line in say(&plan) {
        println!("{line}");
    }
    println!("Profile: {} ({})", plan.request.profile, plan.why);
    if let Some(why) = value["refused"].as_str() {
        println!("Refused: {why}");
        return 2;
    }
    if !plan.wrapped {
        println!("Not sandboxed: the agent starts as it always does here.");
        return 0;
    }
    for (title, key) in [("Writable", "writable"), ("Hidden", "hidden")] {
        println!("{title}:");
        for path in value[key].as_array().into_iter().flatten().filter_map(Value::as_str) {
            println!("  {path}");
        }
    }
    println!("The command:\n  {}", value["command"].as_str().unwrap_or(""));
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A home of the test's own, never under /tmp, which a sandbox replaces and so never takes a project from
    fn temp(name: &str) -> PathBuf {
        let base = std::env::var_os("HOME").map(PathBuf::from).filter(|home| !home.starts_with("/tmp")).unwrap_or_else(|| PathBuf::from("/var/lib/amethystora-test"));
        let dir = base.join(format!("amethystora-sandbox-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::canonicalize(&dir).unwrap()
    }

    fn table() -> Table {
        let mut text = String::new();
        for agent in agents::AGENTS {
            text.push_str(&match agent {
                "claude" => "way claude var CLAUDE_CONFIG_DIR .claude\ndir claude .claude\nlogin claude .claude/.credentials.json\n\
                    copy claude .claude.json\ncopy claude .claude/.claude.json\nro claude .claude/settings.json\nro claude .claude/hooks\n\
                    project claude .claude\nproject claude .mcp.json\nenv claude ANTHROPIC_API_KEY\n".to_string(),
                "cursor-agent" => "way cursor-agent mount\ndir cursor-agent .cursor\n".to_string(),
                other => format!("way {other} none\ndir {other} .{other}\n"),
            });
        }
        text.push_str("hidden - .ssh\nhidden - .local/share/amethystora/agent-profiles\nhidden - /var/home/.snapshots\ncache - .cargo/registry\ntool - .cargo/bin\n");
        Table::parse(&text).unwrap()
    }

    fn request(home: &Path, project: &Path, mode: Mode, profile: &str, agent: &str) -> Request {
        Request {
            agent: agent.into(),
            profile: profile.into(),
            mode,
            project: Some(project_of(project, home).unwrap()),
            allow: Vec::new(),
            home: home.to_path_buf(),
            uid: 1000,
            run: home.join(".cache/amethystora/agent-sandbox/1-x"),
            dir: project.to_path_buf(),
        }
    }

    /// The position of a mount of PATH with OPTION in the recipe
    fn at(recipe: &Recipe, option: &str, path: &Path) -> Option<usize> {
        let path = s(path);
        recipe.args.windows(3).position(|w| w[0] == option && w[2] == path)
            .or_else(|| recipe.args.windows(2).position(|w| w[0] == option && w[1] == path))
    }

    #[test]
    fn a_project_is_the_repository_and_never_the_home() {
        let home = temp("project");
        std::fs::create_dir_all(home.join("code/site/.git/hooks")).unwrap();
        std::fs::create_dir_all(home.join("code/site/src")).unwrap();
        std::fs::create_dir_all(home.join(".config/x")).unwrap();
        std::fs::create_dir_all(home.join("loose")).unwrap();
        assert_eq!(project_of(&home.join("code/site/src"), &home).unwrap().top, home.join("code/site"));
        assert_eq!(project_of(&home.join("loose"), &home).unwrap().top, home.join("loose"));
        assert!(project_of(&home, &home).is_err());
        assert!(project_of(Path::new("/"), &home).is_err());
        assert!(project_of(&home.join(".config/x"), &home).is_err());
        // A dotfiles repository in the home is not every folder's project
        std::fs::create_dir_all(home.join(".git")).unwrap();
        assert_eq!(project_of(&home.join("loose"), &home).unwrap().top, home.join("loose"));
        // A worktree: its repository's folder is elsewhere
        std::fs::create_dir_all(home.join("code/site/.git/worktrees/wt")).unwrap();
        std::fs::write(home.join("code/site/.git/worktrees/wt/commondir"), "../..\n").unwrap();
        std::fs::create_dir_all(home.join("code/wt")).unwrap();
        std::fs::write(home.join("code/wt/.git"), format!("gitdir: {}\n", home.join("code/site/.git/worktrees/wt").display())).unwrap();
        let worktree = project_of(&home.join("code/wt"), &home).unwrap();
        assert_eq!(worktree.common, Some(home.join("code/site/.git")));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn nothing_writable_is_under_something_hidden_and_the_secrets_are_hidden() {
        let home = temp("recipe");
        for dir in [".ssh", ".cursor", "code/site/.git/hooks", ".cargo/registry", ".claude", ".local/share/amethystora/agent-profiles/work/.claude"] {
            std::fs::create_dir_all(home.join(dir)).unwrap();
        }
        std::fs::write(home.join(".ssh/id_ed25519"), "k").unwrap();
        std::fs::write(home.join("code/site/.git/config"), "[core]\n").unwrap();
        let project = home.join("code/site");
        let recipe = recipe(&table(), &request(&home, &project, Mode::Open, DEFAULT, "claude")).unwrap();
        // Hidden: the keys, the other agents' folders, the profiles
        let ssh = at(&recipe, "--tmpfs", &home.join(".ssh")).expect(".ssh hidden");
        assert!(at(&recipe, "--tmpfs", &home.join(".cursor")).is_some(), "another agent's folder");
        assert!(recipe.args.iter().any(|arg| arg == "--remount-ro"));
        // Writable after: the project, the cache, its own folder
        let project_at = at(&recipe, "--bind", &project).expect("the project");
        assert!(project_at > ssh);
        assert!(at(&recipe, "--bind", &home.join(".cargo/registry")).is_some());
        assert!(at(&recipe, "--bind", &home.join(".claude")).is_some());
        // Read-only after the project: the repository's hooks and settings
        assert!(at(&recipe, "--ro-bind", &project.join(".git/hooks")).unwrap() > project_at);
        assert!(at(&recipe, "--ro-bind", &project.join(".git/config")).unwrap() > project_at);
        // Its own settings read-only, made first; its .claude.json a private copy
        assert!(at(&recipe, "--ro-bind", &home.join(".claude/settings.json")).is_some());
        assert!(recipe.make.contains(&Make::File(home.join(".claude/settings.json"), "{}\n")));
        assert!(recipe.copies.iter().any(|(source, _)| source == &home.join(".claude.json")));
        // A settings folder of the project that is not there is made empty, and a file is watched
        assert!(recipe.make.contains(&Make::Placeholder(project.join(".claude"))));
        assert!(recipe.watched.contains(&project.join(".mcp.json")));
        // The filter, last before the folder it starts in
        let filter = recipe.args.iter().position(|arg| arg == "--seccomp").unwrap();
        assert_eq!(recipe.args[filter + 1], "9");
        assert!(recipe.env.contains(&("AMETHYSTORA_READ_ONLY".into(), Some("1".into()))));
        assert!(recipe.env.contains(&(SANDBOX_VAR.into(), Some("open".into()))));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_profile_is_the_agents_folder_and_the_others_stay_hidden() {
        let home = temp("profile");
        for dir in ["code/a", ".claude", ".local/share/amethystora/agent-profiles/work/.claude", ".local/share/amethystora/agent-profiles/home/.claude"] {
            std::fs::create_dir_all(home.join(dir)).unwrap();
        }
        let project = home.join("code/a");
        let recipe = recipe(&table(), &request(&home, &project, Mode::Open, "work", "claude")).unwrap();
        let profiles = home.join(agents::PROFILES);
        let hidden = at(&recipe, "--tmpfs", &profiles).expect("the profiles hidden");
        let own = at(&recipe, "--bind", &profiles.join("work/.claude")).expect("its own profile's folder");
        assert!(own > hidden);
        assert!(at(&recipe, "--bind", &profiles.join("home/.claude")).is_none());
        assert!(recipe.env.contains(&("CLAUDE_CONFIG_DIR".into(), Some(s(&profiles.join("work/.claude"))))));
        // The remount that makes the profiles' folder read-only comes after its own folder is bound in it
        let remount = recipe.args.windows(2).position(|w| w[0] == "--remount-ro" && w[1] == s(&profiles)).unwrap();
        assert!(remount > own);
        // Way 2, no sandbox: the profile's copy over the home's
        let mut way2 = request(&home, &project, Mode::Profile, "work", "cursor-agent");
        way2.project = None;
        let recipe = super::recipe(&table(), &way2).unwrap();
        assert_eq!(recipe.args[..5], ["--unshare-user", "--dev-bind", "/", "/", "--die-with-parent"]);
        assert!(at(&recipe, "--bind", &profiles.join("work/.cursor")).is_some());
        assert!(recipe.args.contains(&s(&home.join(".cursor"))));
        // An agent with no profiles has none
        assert!(super::recipe(&table(), &request(&home, &project, Mode::Open, "work", "copilot")).is_err());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_closed_sandbox_has_only_what_it_names() {
        let home = temp("closed");
        for dir in ["code/a", ".cargo/bin", ".ssh"] {
            std::fs::create_dir_all(home.join(dir)).unwrap();
        }
        let project = home.join("code/a");
        let recipe = recipe(&table(), &request(&home, &project, Mode::Closed, DEFAULT, "claude")).unwrap();
        let tmpfs = at(&recipe, "--tmpfs", &home).expect("the home replaced");
        assert!(at(&recipe, "--ro-bind", &home.join(".cargo/bin")).unwrap() > tmpfs);
        assert!(at(&recipe, "--bind", &project).unwrap() > tmpfs);
        assert!(recipe.args.windows(2).any(|w| w[0] == "--remount-ro" && w[1] == s(&home)));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn permissions_are_read_strictly_and_say_their_cost() {
        let home = temp("allow");
        std::fs::create_dir_all(home.join("shared")).unwrap();
        assert_eq!(Permission::parse("write=~/shared", &home).unwrap(), Permission::Write(home.join("shared")));
        assert_eq!(Permission::parse("gpu", &home).unwrap(), Permission::Gpu);
        assert!(Permission::parse("write=/", &home).is_err());
        assert!(Permission::parse(&format!("write={}", home.display()), &home).is_err());
        assert!(Permission::parse("write=relative", &home).is_err());
        assert!(Permission::parse("network=any", &home).is_err());
        assert!(Permission::parse("show=~/missing", &home).is_err());
        assert!(Permission::SshAgent.cost().contains("every key"));
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn secret_looking_variables_are_not_passed_on() {
        for name in ["GITHUB_TOKEN", "AWS_SECRET_ACCESS_KEY", "AWS_ACCESS_KEY_ID", "NPM_TOKEN", "DB_PASSWORD", "STRIPE_API_KEY", "OPENAI_APIKEY", "MY_PRIVATE_THING"] {
            assert!(secret_name(name), "{name}");
        }
        for name in ["PATH", "HOME", "LANG", "TERM", "EDITOR", "KEYTIMEOUT", "CARGO_HOME"] {
            assert!(!secret_name(name), "{name}");
        }
    }
}
