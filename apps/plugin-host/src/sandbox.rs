//! The sandbox recipe of plans/PLAN-plugins.md ("The sandbox"), in one function, which `amethystora-plugin-host
//! sandbox ID` prints and the host runs. A plugin starts with its own folder (read-only), a private state folder, a
//! private /tmp and the image's /usr: no home, no network, no bus, no display, no other process. A permission is a
//! bind mount added here, or a socket to the host's proxy; nothing is a promise.
//!
//! TODO(phase 0): bubblewrap under SELinux in a user service, `--disable-userns`, and the filter's list were not tried in
//! the lab yet. The recipe is the plan's, and the VM suite (72-plugins.sh) is what shows whether it holds.

use std::path::{Path, PathBuf};

/// The descriptor the seccomp filter is passed on: bubblewrap reads it from there
pub const SECCOMP_FD: i32 = 9;
pub const SECCOMP_FILE: &str = "/usr/share/amethystora/plugins/seccomp.bpf";
pub const HOST_EXE: &str = "/usr/libexec/amethystora-plugin-host";
/// Where the sandbox sees what it was given: its own files, its state, and the proxy's socket
pub const PLUGIN_DIR: &str = "/plugin";
pub const STATE_DIR: &str = "/state";
pub const PROXY_DIR: &str = "/proxy";
pub const SHIM_PORT: u16 = 3128;
/// The most /tmp may hold
const TMP_BYTES: &str = "16777216";

pub struct Spec<'a> {
    pub id: &'a str,
    /// The plugin's own files, in the store or the image
    pub folder: &'a Path,
    /// The account's state folder for the plugin
    pub state: &'a Path,
    pub command: &'a [String],
    /// A file or folder of the home, already resolved, and whether it may be written
    pub files: &'a [(PathBuf, bool)],
    /// The folder the host's proxy socket is in; given only to a plugin that holds a network permission
    pub proxy_dir: Option<&'a Path>,
    /// A one-line /etc/passwd of the account's, so that programs asking who they are get an answer
    pub passwd: &'a Path,
    /// The only environment it keeps (see `kept_env`)
    pub env: &'a [(String, String)],
    pub action: Option<&'a str>,
}

/// LANG, LC_* and TZ of the account: `date` still speaks the person's language, and nothing else leaks in
pub fn kept_env(vars: impl Iterator<Item = (String, String)>) -> Vec<(String, String)> {
    vars.filter(|(k, v)| (k == "LANG" || k == "TZ" || k.starts_with("LC_")) && v.len() <= 64 && v.bytes().all(|b| b.is_ascii_graphic())).collect()
}

fn s(text: &str) -> String {
    text.to_string()
}

fn p(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// bubblewrap's arguments, from `--unshare-all` to the plugin's own command. The seccomp filter comes on descriptor 9
pub fn bwrap_args(spec: &Spec) -> Vec<String> {
    let mut a: Vec<String> = [
        "--unshare-all", "--unshare-user", "--disable-userns", "--new-session", "--die-with-parent", "--cap-drop", "ALL", "--clearenv",
        // The image's /usr, and the links a merged-/usr system has at the top
        "--ro-bind", "/usr", "/usr", "--symlink", "usr/bin", "/bin", "--symlink", "usr/lib", "/lib", "--symlink", "usr/lib64", "/lib64",
        "--symlink", "usr/sbin", "/sbin",
        // From /etc only certificates, the time zone, fonts and the loader's cache
        "--ro-bind-try", "/etc/ssl", "/etc/ssl", "--ro-bind-try", "/etc/pki", "/etc/pki", "--ro-bind-try", "/etc/localtime", "/etc/localtime",
        "--ro-bind-try", "/etc/fonts", "/etc/fonts", "--ro-bind-try", "/etc/ld.so.cache", "/etc/ld.so.cache",
    ]
    .iter()
    .map(|x| s(x))
    .collect();
    a.extend(["--ro-bind".into(), p(spec.passwd), "/etc/passwd".into()]);
    a.extend(["--size".into(), s(TMP_BYTES), "--tmpfs".into(), "/tmp".into(), "--proc".into(), "/proc".into(), "--dev".into(), "/dev".into()]);
    a.extend(["--ro-bind".into(), p(spec.folder), s(PLUGIN_DIR), "--bind".into(), p(spec.state), s(STATE_DIR), "--chdir".into(), s(PLUGIN_DIR)]);
    for (path, writable) in spec.files {
        a.extend([if *writable { "--bind" } else { "--ro-bind" }.into(), p(path), p(path)]);
    }
    let mut env: Vec<(String, String)> = vec![
        (s("PATH"), s("/usr/bin")),
        (s("HOME"), s(STATE_DIR)),
        (s("PLUGIN_ID"), s(spec.id)),
        (s("PLUGIN_DIR"), s(PLUGIN_DIR)),
        (s("WIDGET_STATE"), s(STATE_DIR)),
        (s("PLUGIN_STATE"), s(STATE_DIR)),
    ];
    if let Some(action) = spec.action {
        env.push((s("PLUGIN_ACTION"), s(action)));
    }
    env.extend(spec.env.iter().cloned());
    let mut command: Vec<String> = Vec::new();
    if let Some(dir) = spec.proxy_dir {
        // The network namespace has only loopback: a forwarder inside it takes HTTPS_PROXY's connections to the host's socket
        a.extend(["--ro-bind".into(), p(dir), s(PROXY_DIR)]);
        let proxy = format!("http://127.0.0.1:{SHIM_PORT}");
        for name in ["HTTPS_PROXY", "https_proxy", "ALL_PROXY"] {
            env.push((s(name), proxy.clone()));
        }
        command.extend([s(HOST_EXE), s("shim"), format!("{PROXY_DIR}/proxy.sock"), s("--")]);
    }
    for (key, value) in &env {
        a.extend(["--setenv".into(), key.clone(), value.clone()]);
    }
    a.extend(["--seccomp".into(), SECCOMP_FD.to_string(), "--".into()]);
    a.extend(command);
    a.extend(spec.command.iter().cloned());
    a
}

/// The budgets of the transient scope the sandbox runs in (the user's manager enforces them)
pub const SCOPE_ARGS: &[&str] = &["--user", "--scope", "--quiet", "--collect", "-p", "MemoryMax=256M", "-p", "CPUQuota=25%", "-p", "TasksMax=64"];

/// The whole command line: the scope, then bubblewrap
pub fn command_line(spec: &Spec) -> Vec<String> {
    let mut line = vec![s("systemd-run")];
    line.extend(SCOPE_ARGS.iter().map(|x| s(x)));
    line.push(s("--description"));
    line.push(format!("Plugin {}", spec.id));
    line.push(s("bwrap"));
    line.extend(bwrap_args(spec));
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec<'a>(command: &'a [String], files: &'a [(PathBuf, bool)], proxy: Option<&'a Path>, env: &'a [(String, String)], action: Option<&'a str>) -> Spec<'a> {
        Spec {
            id: "dev.example.t",
            folder: Path::new("/var/lib/amethystora/plugins/store/abc"),
            state: Path::new("/home/a/.local/state/amethystora/plugins/dev.example.t"),
            command,
            files,
            proxy_dir: proxy,
            passwd: Path::new("/run/user/1000/amethystora/plugins/passwd"),
            env,
            action,
        }
    }

    fn has(args: &[String], run: &[&str]) -> bool {
        args.windows(run.len()).any(|w| w.iter().zip(run).all(|(a, b)| a == b))
    }

    #[test]
    fn the_recipe_as_the_plan_lists_it() {
        let command = vec!["./run.sh".to_string()];
        let a = bwrap_args(&spec(&command, &[], None, &[], None));
        for flag in ["--unshare-all", "--unshare-user", "--disable-userns", "--new-session", "--die-with-parent", "--clearenv"] {
            assert!(a.iter().any(|x| x == flag), "{flag}");
        }
        assert!(has(&a, &["--cap-drop", "ALL"]) && has(&a, &["--ro-bind", "/usr", "/usr"]) && has(&a, &["--tmpfs", "/tmp"]));
        assert!(has(&a, &["--ro-bind", "/var/lib/amethystora/plugins/store/abc", "/plugin"]));
        assert!(has(&a, &["--seccomp", "9", "--", "./run.sh"]));
        // --clearenv comes before every --setenv, and what the plugin does not hold is nowhere in it
        let clear = a.iter().position(|x| x == "--clearenv").unwrap();
        assert!(a.iter().enumerate().filter(|(_, x)| *x == "--setenv").all(|(i, _)| i > clear));
        assert!(!a.iter().any(|x| x.starts_with("/home") && !x.contains("plugins")), "no home");
        assert!(!a.iter().any(|x| x == "--share-net" || x == "--dev-bind"));
        assert!(!a.iter().any(|x| x.contains("HTTPS_PROXY")), "no network, no proxy");
        assert!(!a.iter().any(|x| x == "/dev/dri"));
    }

    #[test]
    fn permissions_are_mounts_and_a_proxy() {
        let command = vec!["./run.sh".to_string(), "a".to_string()];
        let files = vec![(PathBuf::from("/home/a/Documents/ci"), false), (PathBuf::from("/home/a/Pictures"), true)];
        let env = vec![("LANG".to_string(), "fr_FR.UTF-8".to_string())];
        let a = bwrap_args(&spec(&command, &files, Some(Path::new("/run/user/1000/amethystora/plugins/dev.example.t")), &env, Some("open-page")));
        assert!(has(&a, &["--ro-bind", "/home/a/Documents/ci", "/home/a/Documents/ci"]));
        assert!(has(&a, &["--bind", "/home/a/Pictures", "/home/a/Pictures"]));
        assert!(has(&a, &["--setenv", "PLUGIN_ACTION", "open-page"]) && has(&a, &["--setenv", "LANG", "fr_FR.UTF-8"]));
        assert!(has(&a, &["--setenv", "HTTPS_PROXY", "http://127.0.0.1:3128"]));
        assert!(has(&a, &["--", "/usr/libexec/amethystora-plugin-host", "shim", "/proxy/proxy.sock", "--", "./run.sh", "a"]));
        let line = command_line(&spec(&command, &[], None, &[], None));
        assert_eq!(&line[..3], ["systemd-run", "--user", "--scope"]);
        assert!(has(&line, &["-p", "MemoryMax=256M", "-p", "CPUQuota=25%", "-p", "TasksMax=64"]) && has(&line, &["bwrap", "--unshare-all"]));
    }

    #[test]
    fn only_the_language_and_time_zone_are_kept() {
        let vars = [("LANG", "en_US.UTF-8"), ("LC_ALL", "C"), ("TZ", "UTC"), ("HOME", "/home/a"), ("DBUS_SESSION_BUS_ADDRESS", "x"), ("LC_X", "a b")];
        let kept = kept_env(vars.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        let names: Vec<&str> = kept.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["LANG", "LC_ALL", "TZ"]);
    }
}
