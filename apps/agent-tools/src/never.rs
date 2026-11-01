//! What the agent may never run, whoever allows it: the tools refuse it before a terminal opens, and the root
//! helper refuses it again as root. The agent names such a command, and the user runs it.
//!
//! An argument vector is judged as it is, so a program that would run another one out of sight of this list
//! (a shell, an interpreter given code, a wrapper such as env or systemd-run) is refused too: the agent runs
//! each step as a command of its own instead.

/// Commands of `ame security` the agent may hand over: those that read, and those that turn protection on.
/// Anything that loosens it or sends something away is the user's to run.
const SECURITY_READS: &[&str] = &["status", "events", "apps", "connections", "addresses", "audit", "home-networks", "sends"];
/// Switches whose `on` tightens what the image enforces or adds a watch
const SECURITY_ON: &[&str] = &[
    "app-sandbox", "audit-locked", "block-automount", "block-bluetooth", "block-user-extensions", "block-webcam",
    "block-xwayland", "blocked-modules", "bluetooth-privacy", "brew-attestations", "browser", "failed-logins", "firewall",
    "firewall-away", "firewire", "flathub-verified", "flatpak-password", "ipv6-privacy", "locked-poweroff", "lockout", "mac",
    "network-daemons", "no-coredumps", "noexec-temp", "package-cooldown", "ransomware", "signed-updates", "ssh-settings",
    "time", "usb", "virtual-input", "vscodium-extensions", "walk-away",
];

/// Programs that run other code the list cannot see
const RUNNERS: &[&str] = &[
    "sh", "bash", "dash", "zsh", "fish", "ksh", "csh", "tcsh", "busybox", "env", "xargs", "nohup", "setsid", "timeout",
    "nice", "ionice", "chroot", "systemd-run", "run0", "runuser", "setpriv", "unshare", "nsenter", "flock", "stdbuf",
    "script", "sudo", "su", "pkexec", "doas", "watch", "parallel", "expect", "tmux", "screen", "eval", "exec", "strace",
    "gdb", "ltrace",
];
/// Interpreters, refused when given code on the command line rather than a file
const INTERPRETERS: &[&str] = &["python", "python3", "perl", "ruby", "node", "lua", "php", "awk", "gawk", "osascript", "gjs"];

/// Paths no command of the agent's may name: who may become root, the agent's own lock, the duress
/// feature's files, and the notes
const PATHS: &[&str] = &[
    "/etc/sudoers", "/etc/polkit-1", "/usr/share/polkit-1", "/etc/amethystora/agent.conf", "/etc/pam.d", "/etc/authselect",
    "/etc/security", "amethystora-notes", "/etc/shadow", "/etc/gshadow", "text-shortcuts.json",
];

fn base(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Why `argv` is refused, or None when it may be handed over
pub fn refusal(argv: &[String]) -> Option<String> {
    let Some(first) = argv.first() else {
        return Some("there is nothing to run".into());
    };
    let program = base(first);
    let lower: Vec<String> = argv.iter().map(|arg| arg.to_lowercase()).collect();
    if lower.iter().any(|arg| arg.contains("duress")) {
        return Some("it is about the duress feature, which is the user's alone".into());
    }
    // Text shortcuts look at what the person types: their switch (gsettings, dconf), and their helper by its own name,
    // are the user's, however the command spells them
    if lower.iter().any(|arg| TYPING.iter().any(|name| arg.contains(name))) {
        return Some("text shortcuts look at what is typed, and their texts are the user's own writing: the user changes them".into());
    }
    if let Some(path) = PATHS.iter().find(|path| argv.iter().any(|arg| arg.contains(*path))) {
        return Some(format!("it names {path}, which the agent may never change"));
    }
    if program.starts_with("amethystora-agent") {
        return Some("the agent may not run its own tools or change its own lock or root mode".into());
    }
    if RUNNERS.contains(&program) {
        return Some(format!("{program} would run other commands out of sight; run each command on its own"));
    }
    let interpreter = program.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    if INTERPRETERS.contains(&program) || INTERPRETERS.contains(&interpreter) {
        if argv.iter().skip(1).any(|arg| matches!(arg.as_str(), "-c" | "-e" | "-E" | "--eval" | "-r" | "-p" | "--print") || arg.starts_with("-c")) {
            return Some(format!("{program} given code on its command line runs out of sight; save it as a file the user can read"));
        }
    }
    if matches!(program, "visudo" | "passwd" | "chpasswd" | "usermod" | "gpasswd" | "useradd" | "groupmod" | "authselect" | "cryptsetup") {
        return Some(format!("{program} changes who may log in or become root, or a disk's keys: the user runs it"));
    }
    if program == "restic" && argv.iter().any(|arg| matches!(arg.as_str(), "forget" | "prune" | "key" | "rewrite")) {
        return Some("it deletes from the backup, which nothing on this machine does".into());
    }
    if program == "btrfs" && argv.windows(2).any(|pair| pair[0] == "subvolume" && pair[1] == "delete") {
        return Some("it deletes a subvolume, and with it maybe a snapshot".into());
    }
    if program == "amethystora-raid" && argv.get(1).map(String::as_str) != Some("status") {
        return Some("pools of disks are made, changed and erased only by the user, in a terminal".into());
    }
    if matches!(program, "ame" | "ujust" | "just") {
        return match recipe_words(&argv[1..]) {
            // A group's own justfile is read both ways: as that group's recipes and as the file it may only be named like
            Ok((words, group)) => ame_refusal(&words).or_else(|| group.and_then(|group| ame_refusal(&[vec![group], words].concat()))),
            Err(why) => Some(why),
        };
    }
    if program == "dconf" && dconf_reaches(&argv[1..], TYPING_KEYS) {
        return Some("text shortcuts look at what is typed: the user turns them on, in Control".into());
    }
    // The live pill's list of what the agent changes, and the desktop's other ways in to the agent: the person's view
    // of the agent, which the agent may not turn off or move, however the command spells it
    if lower.iter().any(|arg| AGENT_UI.iter().any(|name| arg.contains(name)))
        || (program == "dconf" && dconf_reaches(&argv[1..], AGENT_KEYS))
        || (program == "gnome-extensions" && lower.iter().any(|arg| arg.starts_with("amethystora-ui@")))
    {
        return Some("the desktop's view of what the agent changes is the user's: the agent may not change it".into());
    }
    if program.starts_with("amethystora-security") || program == "amethystora-hardening" {
        return Some("security switches are changed only through ame security, and only to turn protection on".into());
    }
    None
}

/// What names the text shortcuts' switch or helper in any spelling: the GSettings schema, its dconf path, the helper
const TYPING: &[&str] = &["amethystora-ui.typing", "amethystora-ui/typing", "amethystora-text-shortcuts"];

/// Whether a dconf command writes, loads or resets the keys under the folder `keys` (the text shortcuts', the agent's):
/// by their own path, or by a folder above it, whose load or reset reaches them too
fn dconf_reaches(args: &[String], keys: &str) -> bool {
    matches!(args.first().map(String::as_str), Some("write" | "load" | "reset" | "compile" | "update"))
        && args[1..].iter().any(|arg| arg.starts_with('/') && arg.ends_with('/') && keys.starts_with(arg.as_str()))
}

const TYPING_KEYS: &str = "/org/gnome/shell/extensions/amethystora-ui/typing/";
/// The agent's own desktop keys (modules/agent.js): its changes in the live pill and the selection's key
const AGENT_KEYS: &str = "/org/gnome/shell/extensions/amethystora-ui/agent/";
const AGENT_UI: &[&str] = &["amethystora-ui.agent", "amethystora-ui/agent"];

/// Options of just (and of `ame` and `ujust`, which run it) that take a value, and how many
const JUST_VALUES: &[(&str, usize)] = &[
    ("--justfile", 1), ("-f", 1), ("--working-directory", 1), ("-d", 1), ("--color", 1), ("--chooser", 1),
    ("--list-heading", 1), ("--list-prefix", 1), ("--dotenv-filename", 1), ("--dotenv-path", 1), ("-E", 1),
    ("--timestamp-format", 1), ("--tempdir", 1),
];
/// ...and those that change what a recipe runs, or run something else entirely: refused, whatever follows
const JUST_RUNNERS: &[&str] = &["--command", "-c", "--shell", "--shell-arg", "--shell-command", "--set", "--evaluate",
    "--edit", "-e", "--choose", "--script"];

/// The words of the recipe a just command line names, as ame's recipes are matched: its options and their values left
/// out (`ame -q security`, `just --justfile FILE security`), every `--` passed over (a refusal never depends on where
/// one stands), and a module path `a::b` as the two words `a b`, each in lower case. A group's own justfile named with
/// -f or --justfile (`just -f /usr/share/amethystora/just/agent.just pill off`) is given back too, for the words with the
/// group in front, as `ame` names them. An option that makes just run something other than a recipe is refused
fn recipe_words(args: &[String]) -> Result<(Vec<String>, Option<String>), String> {
    let mut words = Vec::new();
    let mut justfile: Option<&str> = None;
    let mut options = true;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        i += 1;
        if arg == "--" {
            options = false;
            continue;
        }
        if options && arg.starts_with('-') && arg.len() > 1 {
            let name = arg.split('=').next().unwrap_or(arg);
            if JUST_RUNNERS.contains(&name) || (name.starts_with("-c") && !name.starts_with("--") && name.len() > 2) {
                return Err(format!("{name} makes just run something other than a recipe; run the recipe as it is"));
            }
            if matches!(name, "-f" | "--justfile") {
                justfile = arg.split_once('=').map(|(_, file)| file).or(args.get(i).map(String::as_str));
            } else if name.starts_with("-f") && !name.starts_with("--") {
                justfile = Some(&arg[2..]);
            }
            if !arg.contains('=') {
                if let Some((_, count)) = JUST_VALUES.iter().find(|(option, _)| *option == name) {
                    i += count;
                }
            }
            continue;
        }
        options = false;
        words.extend(arg.split("::").filter(|part| !part.is_empty()).map(str::to_lowercase));
    }
    let group = justfile.map(base).and_then(|file| file.strip_suffix(".just")).filter(|group| *group != "00-entry");
    Ok((words, group.map(str::to_lowercase)))
}

fn ame_refusal(args: &[String]) -> Option<String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["agent", "lock" | "root", ..] => Some("the agent may not change its own lock or root mode".into()),
        ["agent", "tools" | "toggle", "on", ..] => Some("only the user turns the agent's levels on".into()),
        // The live pill's list of the agent's changes is how the person sees them: only the person turns it off
        ["agent", "pill"] | ["agent", "pill", "off", ..] | ["desktop", "pill"] | ["desktop", "pill", "off", ..] => {
            Some("the live pill's list of what the agent changes is the user's to turn off".into())
        }
        ["apps", "aur", ..] => Some("turning the AUR on is the user's choice alone".into()),
        // External screens' brightness gives the person at the machine the displays' i2c buses, which reach other chips
        // of the graphics card too: an access grant only the machine's owner makes (taking it back is fine)
        ["desktop", "screen-brightness"] | ["desktop", "screen-brightness", "on", ..] => {
            Some("giving the displays' i2c buses to the person at the machine is the owner's choice: the user turns it on".into())
        }
        // Text shortcuts look at what the person types: turning them on (or flipping them, which may turn them on) is
        // theirs alone, and the shortcuts are their own writing, which the agent neither reads nor changes
        ["desktop", "text-shortcuts"] | ["desktop", "text-shortcuts", "on", ..] => {
            Some("turning text shortcuts on lets the desktop look at what is typed: the user turns it on".into())
        }
        ["desktop", "text-shortcut", rest @ ..] if rest.first() != Some(&"list") => {
            Some("the text shortcuts are the user's own writing: the user adds, changes and reads them".into())
        }
        ["system", "raid", rest @ ..] if rest.first() != Some(&"status") => {
            Some("pools of disks are made, changed and erased only by the user, in a terminal".into())
        }
        ["security", rest @ ..] => security_refusal(rest),
        _ => None,
    }
}

/// `ame security` only reads, or turns protection on
fn security_refusal(args: &[&str]) -> Option<String> {
    let allowed = match args {
        [] => true,
        [command] => SECURITY_READS.contains(command) || *command == "scan" || *command == "app-integrity",
        ["scan", "now"] | ["app-integrity", "status" | "check"] => true,
        ["kernel", "standard" | "strict"] | ["lockdown", "on" | "strict"] => true,
        [switch, "on"] => SECURITY_ON.contains(switch),
        ["profile", "light" | "balanced" | "thorough"] => true,
        ["kernel-args", argument, "on"] => argument.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '=' | '-')),
        // A resolver of the list, or resuming after a pause; custom asks its servers in a terminal
        ["dns", "quad9" | "mullvad" | "cloudflare" | "google" | "resume"] => true,
        _ => false,
    };
    if allowed {
        None
    } else {
        Some("this security command does not turn protection on: the agent names it, and the user runs it".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(words: &str) -> Vec<String> {
        words.split(' ').map(str::to_string).collect()
    }

    #[test]
    fn refuses_the_never_list() {
        for words in [
            "ame security duress setup",
            "cat /etc/amethystora/duress.d/fingers",
            "tee /etc/sudoers.d/agent",
            "cp rule /etc/polkit-1/rules.d/00.rules",
            "sed -i s/act/off/ /etc/amethystora/agent.conf",
            "ame agent lock act",
            "ame agent root each",
            "ame agent tools on",
            "ame apps aur on",
            "ame desktop screen-brightness on",
            "ame desktop screen-brightness",
            "ame -q desktop::screen-brightness on",
            "ame desktop text-shortcuts on",
            "ame desktop text-shortcuts",
            "ame desktop text-shortcut add ;x",
            "ame desktop text-shortcut show ;sig",
            "ame desktop text-shortcut remove ;sig",
            "cat /var/home/a/.config/amethystora/text-shortcuts.json",
            // The same, however it is spelled
            "gsettings set org.gnome.shell.extensions.amethystora-ui.typing expand true",
            "gsettings reset org.gnome.shell.extensions.amethystora-ui.typing expand",
            "dconf write /org/gnome/shell/extensions/amethystora-ui/typing/expand true",
            "dconf load /org/gnome/shell/extensions/amethystora-ui/",
            "dconf load /org/gnome/shell/extensions/",
            "dconf load /",
            "dconf reset -f /org/gnome/shell/extensions/amethystora-ui/",
            "/usr/libexec/amethystora-text-shortcuts show ;sig",
            "/usr/libexec/amethystora-text-shortcuts add ;x y",
            "amethystora-text-shortcuts remove ;sig",
            "ame desktop::text-shortcuts on",
            "ame -q desktop text-shortcuts on",
            "ame -- desktop text-shortcuts on",
            "ame --quiet -- desktop::text-shortcut show ;sig",
            "ame Desktop Text-Shortcuts on",
            "just --justfile /usr/share/amethystora/just/00-entry.just desktop text-shortcuts on",
            "just -f /usr/share/amethystora/just/00-entry.just desktop::text-shortcuts on",
            "just --justfile=/usr/share/amethystora/just/00-entry.just desktop text-shortcuts on",
            "just -d / -f /usr/share/amethystora/just/00-entry.just desktop text-shortcuts on",
            "/usr/bin/ujust desktop text-shortcuts on",
            // ...and the rules that were there before, spelled the same ways
            "ame -q security firewall off",
            "ame -- agent root each",
            "ame agent::root each",
            "ame security::firewall off",
            "ame -q apps::aur on",
            "just --justfile /usr/share/amethystora/just/00-entry.just security firewall off",
            "just --justfile /usr/share/amethystora/just/00-entry.just agent lock act",
            "ujust --working-directory /tmp system raid create",
            "ame --justfile x.just -- agent tools on",
            // just made to run something other than a recipe
            "ame --command id",
            "just -c id",
            "ame --shell /tmp/x desktop greeting on",
            "ame --set x y desktop greeting on",
            "just --evaluate",
            "ame security firewall off",
            "ame security countme on",
            "ame security connectivity-check on",
            "ame security firmware-metadata on",
            "ame security location on",
            "ame security browser privacy on",
            "ame security settings osprey on",
            "ame security settings triage on",
            "ame security allow report",
            "ame security profile off",
            "ame security lockdown off",
            "ame security dns pause",
            "ame security dns off",
            "ame security walk-away off",
            "ame security settings walk-away on",
            "ame system raid create",
            "amethystora-raid create",
            "restic -r x forget --prune",
            "btrfs subvolume delete /var/home/.snapshots/x",
            "bash -c id",
            "env FOO=1 rm -rf /",
            "python3 -c print(1)",
            "systemd-run --pty bash",
            "sudo rm x",
            "rm -rf /var/home/a/.local/share/amethystora-notes",
            "/usr/libexec/amethystora-hardening off firewall",
            "amethystora-agent-root session",
            "passwd root",
            // The live pill's list of the agent's changes, and the agent's other desktop keys, however spelled
            "ame agent pill off",
            "ame agent pill",
            "ame -q agent::pill off",
            "gsettings set org.gnome.shell.extensions.amethystora-ui.agent pill false",
            "gsettings set org.gnome.shell.extensions.amethystora-ui.agent ask-selection []",
            "dconf write /org/gnome/shell/extensions/amethystora-ui/agent/pill false",
            "dconf reset -f /org/gnome/shell/extensions/amethystora-ui/agent/",
            "amethystora-agent stop",
            "amethystora-agent undo ab12cd34",
            "ame desktop pill off",
            "gnome-extensions disable amethystora-ui@iarsslen.github.io",
            // A group's own justfile, and -- anywhere, for every rule
            "just -f /usr/share/amethystora/just/agent.just pill off",
            "just --justfile=/usr/share/amethystora/just/agent.just tools on",
            "just --justfile /usr/share/amethystora/just/agent.just toggle on",
            "ujust -f/usr/share/amethystora/just/agent.just lock act",
            "just -d /usr/share/amethystora/just -f agent.just root each",
            "just --working-directory /usr/share/amethystora/just --justfile agent.just lock off",
            "just -f /usr/share/amethystora/just/security.just firewall off",
            "ame -f /usr/share/amethystora/just/security.just lockdown off",
            "just -f /usr/share/amethystora/just/apps.just aur on",
            "ame agent pill -- off",
            "ame agent -- tools on",
            "ame security -- firewall off",
            "ame -- agent -- root each",
        ] {
            assert!(refusal(&argv(words)).is_some(), "{words}");
        }
    }

    #[test]
    fn hands_over_the_rest() {
        for words in [
            "rpm-ostree install htop",
            "systemctl restart NetworkManager.service",
            "ame security firewall on",
            "ame security walk-away on",
            "ame security status",
            "ame security sends",
            "ame security scan now",
            "ame security kernel strict",
            "ame security lockdown on",
            "ame security profile balanced",
            "ame security kernel-args init_on_free=1 on",
            "ame security dns quad9",
            "ame system raid status",
            "ame desktop text-shortcuts off",
            "ame desktop screen-brightness off",
            "ame desktop clear-voice on",
            "ame desktop sound-preset bass",
            "ame desktop text-shortcut list",
            "ame -q desktop::text-shortcuts off",
            "ame -- security status",
            "ame security::status",
            "just --justfile /usr/share/amethystora/just/00-entry.just security scan now",
            "ame -q system raid status",
            "ame desktop greeting on",
            "ame update",
            "ame",
            "just build amethystora latest main",
            "just --list",
            "gsettings set org.gnome.shell.extensions.amethystora-ui.emoji skin-tone medium",
            "dconf write /org/gnome/shell/extensions/amethystora-ui/emoji/skin-tone \"'none'\"",
            "dconf load /org/gnome/shell/extensions/amethystora-ui/emoji/",
            "ame system rebase amethystora stable",
            "python3 /home/a/fix.py",
            "bootc rollback",
            "flatpak override --user --socket=x11 com.example.App",
            "ame agent pill on",
            "ame agent history",
            "just -f /usr/share/amethystora/just/security.just status",
            "just -f /usr/share/amethystora/just/agent.just history",
            "ame desktop pill on",
            "gnome-extensions list",
        ] {
            assert_eq!(refusal(&argv(words)), None, "{words}");
        }
        assert!(refusal(&[]).is_some());
    }
}
