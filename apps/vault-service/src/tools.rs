//! The part of the command line that lets tools use a secret without holding it (plans/PLAN-vault.md, Using a secret): the SSH agent's
//! switch, `get` for a tool's own settings, `run`, protected files and the helpers' settings. Its words come from `vault.just`, as the rest.
//!
//! More exit numbers than `cli.rs` says: 10 not in the home, through a link or not a plain file, 11 nothing there, 12 holds no secret,
//! 13 a file or a setting of somebody else's is there, 14 another command has it open, 15 it changed meanwhile, 16 no namespace to
//! be had, 17 the tool is not installed, 18 it is protected already, 19 the file has another name, 20 something above it cannot be changed,
//! 21 what a command ran over was taken from under it, 22 a variable that steers programs.

use std::ffi::OsString;
use std::io::{IsTerminal, Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};
use zeroize::Zeroizing;

use amethystora_vault_service::protect::{self, Place, Tree, Why, KIND};

use super::{call, Args, USAGE};

const AGENT_SOCKET: &str = "/run/amethystora/vault-ssh.sock";
const NOT_INSTALLED: u8 = 17;
const OURS: u8 = 13;
const REFUSED: u8 = 1;
const CLOSED: u8 = 2;
/// A variable that decides how programs start
const VARIABLE: u8 = 22;

fn why(why: Why) -> u8 {
    why.code()
}

/// The account's home. Its own word, since all it could fool is itself.
fn home() -> Result<PathBuf, u8> {
    std::env::var_os("HOME").map(PathBuf::from).filter(|home| home.is_absolute()).ok_or(6)
}

fn items_of(kind: &str) -> Result<Vec<Value>, u8> {
    Ok(call(json!({"op": "list", "kind": kind}))?["items"].as_array().cloned().unwrap_or_default())
}

/// An item by what a person calls it. Two of the same name are for the person to tell apart, in the window: here, a refusal.
fn named(kind: &str, name: &str) -> Result<Value, u8> {
    let mut found: Vec<Value> = items_of(kind)?.into_iter().filter(|item| item["name"] == json!(name)).collect();
    if found.len() == 1 { Ok(found.remove(0)) } else { Err(REFUSED) }
}

fn id_of(item: &Value) -> Result<String, u8> {
    item["id"].as_str().map(str::to_string).ok_or(6)
}

// --- The SSH agent --------------------------------------------------------------------------------------------------------------

fn agent_socket() -> PathBuf {
    std::env::var_os("AMETHYSTORA_VAULT_SSH_SOCKET").map_or_else(|| PathBuf::from(AGENT_SOCKET), PathBuf::from)
}

/// How many keys the agent lists now: `None` if nothing answers
fn agent_keys() -> Option<u32> {
    let mut stream = UnixStream::connect(agent_socket()).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    stream.write_all(&[0, 0, 0, 1, 11]).ok()?;
    let mut head = [0u8; 9];
    stream.read_exact(&mut head).ok()?;
    (head[4] == 12).then(|| u32::from_be_bytes([head[5], head[6], head[7], head[8]]))
}

/// Refuses a file that is a link, or whose folder leads out of the home: what is written through one lands somewhere else, and what replaces
/// one is not what the link led to
fn no_links(path: &Path) -> Result<(), u8> {
    if std::fs::symlink_metadata(path).is_ok_and(|stat| stat.file_type().is_symlink()) {
        return Err(OURS);
    }
    let home = home()?;
    let mut above = path.parent().ok_or(6u8)?;
    // The folder may not be made yet: the nearest one that is
    while !above.exists() {
        above = above.parent().ok_or(6u8)?;
    }
    if path.starts_with(&home) && !above.canonicalize().and_then(|above| home.canonicalize().map(|home| above.starts_with(home))).unwrap_or(false) {
        return Err(OURS);
    }
    Ok(())
}

/// What a file holds, if it is a plain file of a size a setting has: not a link, and a pipe is never opened for reading. None if it is not there.
fn read_plain(path: &Path, most: usize) -> Result<Option<Vec<u8>>, u8> {
    let file = match std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) if error.raw_os_error() == Some(libc::ELOOP) => return Err(OURS),
        Err(_) => return Err(6),
    };
    if !file.metadata().is_ok_and(|stat| stat.is_file()) {
        return Err(OURS);
    }
    let mut bytes = Vec::new();
    file.take(most as u64 + 1).read_to_end(&mut bytes).map_err(|_| 6u8)?;
    if bytes.len() > most {
        return Err(OURS);
    }
    Ok(Some(bytes))
}

/// A file written whole, for the account alone, or not at all
fn write_private(path: &Path, text: &str) -> Result<(), u8> {
    no_links(path)?;
    let parent = path.parent().ok_or(6u8)?;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(parent).map_err(|_| 6u8)?;
    // A file that was there keeps the mode it had
    let mode = std::fs::metadata(path).map(|stat| std::os::unix::fs::PermissionsExt::mode(&stat.permissions()) & 0o777).unwrap_or(0o600);
    let temporary = parent.join(format!(".amethystora-vault-{}", std::process::id()));
    let written = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(&temporary)?;
        file.write_all(text.as_bytes())?;
        std::fs::set_permissions(&temporary, std::os::unix::fs::PermissionsExt::from_mode(mode))?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(6);
    }
    Ok(())
}

fn user_manager(args: &[&str]) {
    let _ = Command::new("systemctl").arg("--user").args(args).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
}

/// `SSH_AUTH_SOCK` for this account's next login, and nothing while it is off. GNOME's own agent sets that variable as its socket starts,
/// after `environment.d`, which its unit is told not to do for as long as this is on.
pub fn ssh(args: &Args) -> Result<(), u8> {
    let config = home()?.join(".config");
    let variable = config.join("environment.d/60-amethystora-vault-ssh.conf");
    let gcr = config.join("systemd/user/gcr-ssh-agent.socket.d/60-amethystora-vault-ssh.conf");
    match args.word(1).unwrap_or("status") {
        "on" => {
            write_private(&variable, &format!("SSH_AUTH_SOCK={AGENT_SOCKET}\n"))?;
            write_private(&gcr, "[Socket]\nExecStartPost=\n")?;
            user_manager(&["daemon-reload"]);
            user_manager(&["set-environment", &format!("SSH_AUTH_SOCK={AGENT_SOCKET}")]);
        }
        "off" => {
            let _ = std::fs::remove_file(&variable);
            let _ = std::fs::remove_file(&gcr);
            if let Some(folder) = gcr.parent() {
                let _ = std::fs::remove_dir(folder);
            }
            user_manager(&["daemon-reload"]);
            user_manager(&["unset-environment", "SSH_AUTH_SOCK"]);
        }
        "status" => {
            println!("ssh\t{}", if variable.exists() { "on" } else { "off" });
            println!("keys\t{}", agent_keys().map_or_else(|| "-".to_string(), |count| count.to_string()));
            let here = std::env::var_os("SSH_AUTH_SOCK").is_some_and(|socket| Path::new(&socket) == agent_socket());
            println!("this-session\t{}", if here { "yes" } else { "no" });
        }
        _ => return Err(USAGE),
    }
    Ok(())
}

// --- get --------------------------------------------------------------------------------------------------------------------------

/// A password as the tool that asks wants it: as it is for `apiKeyHelper` and a script, or as AWS's `credential_process` and kubectl's
/// `exec` read their answer. For AWS the item's username is the access key's id.
pub fn get(args: &Args) -> Result<(), u8> {
    let item = named("password", args.word(1).ok_or(USAGE)?)?;
    let answer = call(json!({"op": "get", "kind": "password", "id": id_of(&item)?}))?;
    let secret = answer["value"]["password"].as_str().ok_or(6u8)?;
    let out = match args.word(2) {
        None => secret.to_string(),
        Some("aws") => json!({"Version": 1, "AccessKeyId": answer["value"]["username"], "SecretAccessKey": secret}).to_string(),
        Some("kube") => json!({"apiVersion": "client.authentication.k8s.io/v1", "kind": "ExecCredential", "status": {"token": secret}}).to_string(),
        Some(_) => return Err(USAGE),
    };
    let mut stdout = std::io::stdout();
    stdout.write_all(out.as_bytes()).and_then(|()| stdout.write_all(b"\n")).and_then(|()| stdout.flush()).map_err(|_| 6u8)
}

// --- run --------------------------------------------------------------------------------------------------------------------------

/// `run` takes what comes after `--` as it is, so it reads the arguments itself rather than the options `Args` makes of them
fn run_words() -> Option<(OsString, Vec<OsString>)> {
    let raw: Vec<OsString> = std::env::args_os().skip(1).collect();
    let start = raw.iter().position(|arg| arg == "run")? + 1;
    let split = start + raw[start..].iter().position(|arg| arg == "--")?;
    if split != start + 1 || split + 1 >= raw.len() {
        return None;
    }
    Some((raw[start].clone(), raw[split + 1..].to_vec()))
}

fn plain_variable(text: &str) -> bool {
    !text.is_empty() && !text.as_bytes()[0].is_ascii_digit() && text.bytes().all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

/// What a variable is called when nobody says: `github token` is `GITHUB_TOKEN`
fn variable_for(name: &str) -> String {
    let mut text: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
    if !plain_variable(&text) {
        text.insert(0, '_');
    }
    text
}

/// The item a word names among the protected ones: by the name it has, or the path it came from. A copy a command that did not end well left
/// is only ever named, never found by the path, which belongs to the good one.
fn find_protected(word: &str) -> Result<Option<Value>, u8> {
    let items = items_of(KIND)?;
    let by_name = items.iter().find(|item| item["name"] == json!(word));
    let by_path = || {
        let home = home().ok()?;
        let path = protect::under_home(&home, Path::new(word)).ok()?;
        items.iter().find(|item| item["meta"]["path"] == json!(path) && item["meta"]["killed"] != json!(true))
    };
    Ok(by_name.or_else(by_path).cloned())
}

/// What `run` says if it never got as far as the command: after that, its exit status is the command's, and a number could mean anything
fn say_why_not(args: &Args, code: u8) {
    let said = match code {
        REFUSED => args.say("say-refused", "No: that was refused, or it did not match."),
        CLOSED => args.say("say-closed", "The vault is closed. Open it with: ame vault open"),
        3 => args.say("say-not-set-up", "There is no vault on this account yet. Set one up with: ame vault setup"),
        13 => args.say("say-file-there", "Something else is already there, and it was left as it is."),
        14 => args.say("say-in-use", "Another command has that file open."),
        16 => args.say("say-no-namespace", "This system gives a command no namespace of its own, so the file stays sealed."),
        VARIABLE => args.say("say-variable", "That variable changes how programs start, so a secret is never put in it. Name another: VARIABLE=NAME"),
        _ => return,
    };
    eprintln!("{said}");
}

pub fn run(args: &Args) -> Result<(), u8> {
    let mut started = false;
    let result = run_started(args, &mut started);
    if let (Err(code), false) = (&result, started) {
        say_why_not(args, *code);
    }
    result
}

/// Variables that decide what a program starts, loads or runs, or where it looks for its settings. A secret is never put in one, whichever way
/// the name came: a variable's name can be an item's, and an item's is anybody's to choose. The list is written out so that adding to it is
/// a decision (a test holds it).
const STEERING: &[&str] = &[
    "PATH", "IFS", "ENV", "BASH_ENV", "CDPATH", "SHELL", "SHELLOPTS", "BASHOPTS", "GLOBIGNORE", "PROMPT_COMMAND", "PS0", "PS1", "PS2", "PS4", "HOME", "USER", "LOGNAME", "TMPDIR", "TZDIR",
    "GCONV_PATH", "LOCPATH", "NLSPATH", "HOSTALIASES", "RESOLV_HOST_CONF", "TERMINFO", "TERMINFO_DIRS", "EDITOR", "VISUAL", "PAGER", "MANPAGER", "BROWSER", "LESSOPEN", "LESSCLOSE",
    "SSH_ASKPASS", "SSH_AUTH_SOCK", "GIT_ASKPASS", "GIT_SSH", "GIT_SSH_COMMAND", "GIT_EXEC_PATH", "GIT_PROXY_COMMAND", "GIT_EXTERNAL_DIFF", "CLASSPATH", "LUA_PATH", "LUA_CPATH", "GOFLAGS",
    "GOROOT", "CARGO_HOME", "RUSTUP_HOME", "NODE_EXTRA_CA_CERTS", "SSL_CERT_FILE", "SSL_CERT_DIR", "CURL_CA_BUNDLE", "REQUESTS_CA_BUNDLE", "DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS",
];
const STEERING_STARTS: &[&str] = &["LD_", "DYLD_", "MALLOC_", "BASH_FUNC_", "XDG_", "PYTHON", "PERL", "RUBY", "GIT_CONFIG", "GIT_DIR"];
const STEERING_ENDS: &[&str] = &["_OPTIONS", "_OPTS", "_PATH", "_ENV", "_COMMAND", "_ASKPASS"];

fn steers(name: &str) -> bool {
    STEERING.contains(&name) || STEERING_STARTS.iter().any(|start| name.starts_with(start)) || STEERING_ENDS.iter().any(|end| name.ends_with(end))
}

fn run_started(args: &Args, started: &mut bool) -> Result<(), u8> {
    let (word, command) = run_words().ok_or(USAGE)?;
    let word = word.into_string().map_err(|_| USAGE)?;
    let (variable, name) = match word.split_once('=') {
        Some((variable, name)) if plain_variable(variable) => (Some(variable.to_string()), name.to_string()),
        Some(_) => return Err(USAGE),
        None => (None, word),
    };
    if variable.is_none()
        && let Some(item) = find_protected(&name)?
    {
        return run_protected(args, &item, &command, started);
    }
    run_with_variable(args, variable, &name, &command)
}

/// A variable never goes through a namespace. That alone would open the command's memory to every program of the account, which an ordinary
/// process has Yama to keep closed (the spike's question 6). The command takes this process's place, so nothing stays here to hold it.
fn run_with_variable(args: &Args, variable: Option<String>, name: &str, command: &[OsString]) -> Result<(), u8> {
    let variable = variable.unwrap_or_else(|| variable_for(name));
    // Before the vault is asked about anything
    if steers(&variable) {
        return Err(VARIABLE);
    }
    let item = named("password", name)?;
    let answer = call(json!({"op": "get", "kind": "password", "id": id_of(&item)?}))?;
    let secret = Zeroizing::new(answer["value"]["password"].as_str().ok_or(6u8)?.to_string());
    if secret.contains('\0') {
        return Err(REFUSED);
    }
    eprintln!("{}", args.say("say-inherit", "The secret is in {variable} for this command. Every program it starts inherits it, and other programs of this account can read it while it runs.").replace("{variable}", &variable));
    let error = Command::new(&command[0]).args(&command[1..]).env(&variable, secret.as_str()).exec();
    eprintln!("{}: {error}", command[0].to_string_lossy());
    Err(127)
}

fn signals() -> libc::sigset_t {
    // SAFETY: a signal set this function owns
    unsafe {
        let mut set = std::mem::zeroed::<libc::sigset_t>();
        libc::sigemptyset(&raw mut set);
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT, libc::SIGCHLD] {
            libc::sigaddset(&raw mut set, signal);
        }
        set
    }
}

/// How a process ended, as a shell says it
fn exit_code(status: i32) -> i32 {
    if libc::WIFEXITED(status) { libc::WEXITSTATUS(status) } else { 128 + libc::WTERMSIG(status) }
}

/// The first process of the command's namespace. It starts the command, passes on what it is sent, and when the command ends it ends too, which
/// takes every process the command started with it: nothing is left holding the file. If this process's own parent goes, so does everything.
fn init_process(command: &[OsString], set: &libc::sigset_t) -> ! {
    // SAFETY: prctl, unshare and mount are given live strings and flags; this process is alone in its namespace and has one thread
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
        // A /proc of this namespace, so that the command sees its own processes and not the account's. Where the system refuses it, the old one stays.
        if libc::unshare(libc::CLONE_NEWNS) == 0 {
            libc::mount(std::ptr::null(), c"/".as_ptr(), std::ptr::null(), libc::MS_REC | libc::MS_PRIVATE, std::ptr::null());
            libc::mount(c"proc".as_ptr(), c"/proc".as_ptr(), c"proc".as_ptr(), libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC, std::ptr::null());
        }
    }
    let mut start = Command::new(&command[0]);
    start.args(&command[1..]);
    // SAFETY: sigprocmask only, which is safe between fork and exec. The command takes the signals it is sent, which this process has been blocking.
    unsafe {
        start.pre_exec(|| {
            let mut none = std::mem::zeroed::<libc::sigset_t>();
            libc::sigemptyset(&raw mut none);
            libc::sigprocmask(libc::SIG_SETMASK, &raw const none, std::ptr::null_mut());
            Ok(())
        });
    }
    let child = match start.spawn() {
        Ok(child) => child.id() as i32,
        Err(error) => {
            eprintln!("{}: {error}", command[0].to_string_lossy());
            // SAFETY: ends this process
            unsafe { libc::_exit(127) }
        }
    };
    loop {
        // SAFETY: a live set, and a value of the type the kernel fills in
        let (signal, code) = unsafe {
            let mut info = std::mem::zeroed::<libc::siginfo_t>();
            (libc::sigwaitinfo(set, &raw mut info), info.si_code)
        };
        if signal == libc::SIGCHLD {
            loop {
                let mut status = 0;
                // SAFETY: a value the kernel fills in
                let done = unsafe { libc::waitpid(-1, &raw mut status, libc::WNOHANG) };
                if done <= 0 {
                    break;
                }
                if done == child {
                    // SAFETY: ends this process, and with it the namespace
                    unsafe { libc::_exit(exit_code(status)) };
                }
            }
        } else if signal > 0 && code != libc::SI_KERNEL {
            // SAFETY: the command has not been waited for, so its number is still its own
            unsafe { libc::kill(child, signal) };
        }
    }
}

/// Runs the command to its end, in the process namespace this one entered, and says how it ended as a shell would. This process stays to
/// seal the file again whatever becomes of the command, so it takes the signals that would otherwise end it too: one sent to it goes on to the
/// command, and one from the terminal has reached the command already.
fn supervise(command: &[OsString]) -> Result<i32, u8> {
    let set = signals();
    // SAFETY: blocked in this one thread, and waited for below; the first process of the namespace is made while they are
    let init = unsafe {
        libc::sigprocmask(libc::SIG_BLOCK, &raw const set, std::ptr::null_mut());
        libc::fork()
    };
    match init {
        -1 => Err(why(Why::NoNamespace)),
        0 => init_process(command, &set),
        init => loop {
            // SAFETY: a live set, and a value of the type the kernel fills in
            let (signal, code) = unsafe {
                let mut info = std::mem::zeroed::<libc::siginfo_t>();
                (libc::sigwaitinfo(&raw const set, &raw mut info), info.si_code)
            };
            if signal == libc::SIGCHLD {
                let mut status = 0;
                // SAFETY: a value the kernel fills in
                if unsafe { libc::waitpid(init, &raw mut status, libc::WNOHANG) } == init {
                    return Ok(exit_code(status));
                }
            } else if signal > 0 && code != libc::SI_KERNEL {
                // SAFETY: the first process has not been waited for, so its number is still its own
                unsafe { libc::kill(init, signal) };
            }
        },
    }
}

/// One lock for each protected item, in the account's runtime folder: nowhere a program of the account could take it away by moving the file it
/// protects, and it goes with the process that holds it, however that ends.
fn lock_item(id: &str) -> Result<OwnedFd, u8> {
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(6);
    }
    // SAFETY: getuid cannot fail
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })));
    let folder = runtime.join("amethystora-vault-run");
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&folder).map_err(|_| 6u8)?;
    let file = std::fs::OpenOptions::new().write(true).create(true).truncate(false).mode(0o600).custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC).open(folder.join(id)).map_err(|_| 6u8)?;
    let held = OwnedFd::from(file);
    // SAFETY: a live descriptor
    if unsafe { libc::flock(held.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(why(Why::InUse));
    }
    Ok(held)
}

fn run_protected(args: &Args, item: &Value, command: &[OsString], started: &mut bool) -> Result<(), u8> {
    let (id, name) = (id_of(item)?, item["name"].as_str().ok_or(6u8)?.to_string());
    let path = item["meta"]["path"].as_str().ok_or(6u8)?.to_string();
    let home_path = home()?;
    let home_fd = protect::open_home(&home_path).map_err(why)?;
    // Another command running on it, or an unprotect, comes first: nothing is asked
    let _lock = lock_item(&id)?;
    // The person says yes first: nothing is set up for a no
    let answer = call(json!({"op": "get", "kind": KIND, "id": id}))?;
    // The vault it came from, which what the command leaves goes back to: with several open, the first is not necessarily this one
    let vault = answer["vault"].as_str().ok_or(6u8)?.to_string();
    let tree = Tree::from_value(&answer["value"]).map_err(why)?;
    drop(answer);
    let (before, before_size) = (tree.digest(), tree.size());
    // What the file held, to know a copy of it when the command makes one beside it
    let old_data = Zeroizing::new(tree.files.first().filter(|_| tree.what == protect::What::File).map(|file| file.data.to_vec()).unwrap_or_default());
    let place = Place::prepare(&home_path, &home_fd, &path, &tree).map_err(why)?;
    let entered = protect::enter_namespace().and_then(|()| place.mount(&tree));
    drop(tree);
    if let Err(trouble) = entered {
        place.leave();
        return Err(why(trouble));
    }
    eprintln!("{}", args.say("say-exposed", "While this command runs, other programs of this account can read this file and the command's memory."));
    *started = true;
    let status = supervise(command);
    // The command has ended, one way or another. The old copy stays sealed until the new one is.
    let code = status.unwrap_or_else(i32::from);
    let mut sealed = Ok(());
    if !place.intact() {
        // Something outside took the file's place away, and then the command was writing to the disk, in the open
        eprintln!("{}", args.say("say-detached", "Something took the file's place away while the command ran, so what it wrote went to your disk, not into the vault, and nothing was sealed. The old copy is still sealed."));
        sealed = Err(why(Why::Detached));
    } else {
        let mut new_data = Zeroizing::new(Vec::new());
        match place.read_back() {
            Ok(Some(after)) if after.digest() != before => {
                if after.what == protect::What::File {
                    new_data = Zeroizing::new(after.files.first().map(|file| file.data.to_vec()).unwrap_or_default());
                }
                if after.size() == 0 && before_size > 0 {
                    // A tool that fails halfway writes nothing, and a good copy is never replaced by that
                    eprintln!("{}", args.say("say-emptied", "The command left the file empty, so the sealed copy was kept as it was."));
                } else {
                    sealed = seal_back(args, &id, &name, &vault, item, &after, code != 0);
                }
            }
            Ok(_) => {}
            Err(trouble) => {
                eprintln!("{}", args.say("say-unsealable", "What the command left cannot be sealed again, so the file stays as it was before it ran."));
                sealed = Err(why(trouble));
            }
        }
        say_left(args, &place.keep_beside(&old_data, &new_data, code != 0));
    }
    place.leave();
    match (sealed, code) {
        (Err(trouble), 0) => Err(trouble),
        (_, 0) => Ok(()),
        (_, code) => Err(code.clamp(1, 255) as u8),
    }
}

/// Says what was not kept of what the command made beside the file, and why: nothing is dropped without a word
fn say_left(args: &Args, left: &protect::Left) {
    let list = |names: &[String]| {
        let shown: Vec<String> = names.iter().filter(|name| !name.is_empty()).take(20).map(|name| amethystora_vault_service::peer::clean(name, 100)).collect();
        shown.join(", ")
    };
    if !left.on_purpose.is_empty() {
        let count = left.on_purpose.len().to_string();
        eprintln!("{}", args.say("say-copy-left", "Not kept, since it may be a copy of the protected file, or half of one: its name has the file's in it, it holds its content, or the command did not end well ({count} in all): {names}").replace("{count}", &count).replace("{names}", &list(&left.on_purpose)));
    }
    let count = left.failed.len() + left.unshowable;
    if count > 0 {
        let names = list(&left.failed);
        eprintln!("{}", args.say("say-not-kept", "These could not be kept, and are gone with the command's private space ({count} in all, and any with names that cannot be shown): {names}").replace("{count}", &count.to_string()).replace("{names}", &names));
    }
}

/// Seals what the command left, in the person's time: the vault may have closed while it ran, and what it wrote exists nowhere else. A command
/// that did not end well may have been halfway through writing, so what it left goes beside the good copy and never over it.
fn seal_back(args: &Args, id: &str, name: &str, vault: &str, item: &Value, after: &Tree, failed: bool) -> Result<(), u8> {
    let value = after.to_value().map_err(|trouble| {
        eprintln!("{}", args.say("say-unsealable", "What the command left cannot be sealed again, so the file stays as it was before it ran."));
        why(trouble)
    })?;
    let mut meta = json!({"path": item["meta"]["path"], "what": after.what.name(), "size": after.size(), "files": after.files.len()});
    let mut request = json!({"op": "put", "kind": KIND, "meta": meta, "value": value, "vault": vault});
    if failed {
        let kept = format!("{name} (from a command that did not end well)");
        meta["killed"] = json!(true);
        request["meta"] = meta;
        request["approval"] = json!("ask");
        if let Some(other) = find_protected(&kept).unwrap_or(None) {
            request["id"] = other["id"].clone();
        }
        request["name"] = json!(kept);
    } else {
        request["id"] = json!(id);
        request["name"] = json!(name);
    }
    loop {
        match call(request.clone()) {
            Ok(_) => {
                if failed {
                    eprintln!("{}", args.say("say-killed", "The command did not end well after changing the file. What it left is sealed beside the old copy, under its own name. See: ame vault protected"));
                }
                return Ok(());
            }
            Err(code @ (CLOSED | REFUSED)) => {
                let question = if code == CLOSED { "say-reopen" } else { "say-again" };
                let default = if code == CLOSED {
                    "The vault closed while the command ran, and what it left is only here. Open the vault, then press Enter to seal it, or type no to lose it: "
                } else {
                    "That was refused, and what the command left is only here. Press Enter to be asked again, or type no to lose it: "
                };
                if !ask_again(&args.say(question, default)) {
                    eprintln!("{}", args.say("say-lost", "What the command left is gone. The file is as it was before it ran."));
                    return Err(code);
                }
            }
            Err(code) => {
                eprintln!("{}", args.say("say-lost", "What the command left is gone. The file is as it was before it ran."));
                return Err(code);
            }
        }
    }
}

fn ask_again(question: &str) -> bool {
    if !std::io::stdin().is_terminal() {
        return false;
    }
    eprint!("{question}");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).is_ok_and(|count| count > 0) && line.trim().is_empty()
}

// --- Protected files --------------------------------------------------------------------------------------------------------------

pub fn protect(args: &Args) -> Result<(), u8> {
    let home_path = home()?;
    let path = protect::under_home(&home_path, Path::new(args.word(1).ok_or(USAGE)?)).map_err(why)?;
    // `run` takes a word with an equals sign for a variable's name and an item
    if path.contains('=') {
        return Err(why(Why::NotPlain));
    }
    let home_fd = protect::open_home(&home_path).map_err(why)?;
    let tree = protect::read(&home_fd, &path).map_err(why)?;
    if tree.files.is_empty() {
        return Err(why(Why::Missing));
    }
    // Nothing to protect, and an empty file is what a killed command leaves in a file's place
    if tree.what == protect::What::File && tree.size() == 0 {
        return Err(why(Why::NoSecret));
    }
    let existing = find_protected(&path)?;
    // Not while a command is running on it
    let _lock = match &existing {
        Some(item) => Some(lock_item(&id_of(item)?)?),
        None => None,
    };
    let mut request = json!({"op": "put", "kind": KIND, "name": path});
    // What the sealed copy is to hold: this, or for a folder already protected, what it held and this
    let sealed = match &existing {
        None => {
            request["approval"] = json!("ask");
            None
        }
        Some(item) => {
            // A file or a folder that is there again is never put over what is sealed: that would lose it, and the person was not told
            if tree.what == protect::What::File || item["meta"]["what"] != json!("folder") {
                return Err(why(Why::AlreadyProtected));
            }
            let answer = call(json!({"op": "get", "kind": KIND, "id": id_of(item)?}))?;
            let old = Tree::from_value(&answer["value"]).map_err(why)?;
            request["id"] = item["id"].clone();
            request["vault"] = item["vault"].clone();
            Some(old.merged_with(&tree).map_err(why)?)
        }
    };
    let whole = sealed.as_ref().unwrap_or(&tree);
    request["meta"] = json!({"path": path, "what": whole.what.name(), "size": whole.size(), "files": whole.files.len()});
    request["value"] = whole.to_value().map_err(why)?;
    let id = id_of(&call(request)?)?;
    // The sealed copy has to open and hold the same, or the original goes nowhere
    let answer = call(json!({"op": "get", "kind": KIND, "id": id}))?;
    let copy = Tree::from_value(&answer["value"]).map_err(why)?;
    if !copy.same_as(whole) {
        return Err(why(Why::Changed));
    }
    protect::remove_original(&home_fd, &path, &tree).map_err(why)?;
    println!("{id}\t{path}");
    Ok(())
}

pub fn protected(args: &Args) -> Result<(), u8> {
    let as_json = args.option("json").is_some();
    if args.word(1) == Some("usual") {
        let found = protect::usual_places(&home()?);
        if as_json {
            let list: Vec<Value> = found.iter().map(|(path, size)| json!({"path": path, "size": size})).collect();
            println!("{}", json!({"ok": true, "usual": list}));
        } else {
            for (path, size) in found {
                println!("{path}\t{size}");
            }
        }
        return Ok(());
    }
    let items = items_of(KIND)?;
    if as_json {
        println!("{}", json!({"ok": true, "items": items}));
    } else {
        for item in items {
            println!("{}\t{}\t{}\t{}", item["name"].as_str().unwrap_or(""), item["meta"]["path"].as_str().unwrap_or(""), item["meta"]["what"].as_str().unwrap_or(""), item["meta"]["size"]);
        }
    }
    Ok(())
}

pub fn unprotect(args: &Args) -> Result<(), u8> {
    let item = find_protected(args.word(1).ok_or(USAGE)?)?.ok_or(REFUSED)?;
    let id = id_of(&item)?;
    let path = item["meta"]["path"].as_str().ok_or(6u8)?;
    // Not while a command has it: it would be taken from under it
    let _lock = lock_item(&id)?;
    let home_path = home()?;
    let home_fd = protect::open_home(&home_path).map_err(why)?;
    let answer = call(json!({"op": "get", "kind": KIND, "id": id}))?;
    let tree = Tree::from_value(&answer["value"]).map_err(why)?;
    protect::restore(&home_fd, path, &tree).map_err(why)?;
    // Only now, and removing always asks. A no leaves it in both places, which loses nothing.
    call(json!({"op": "remove", "kind": KIND, "id": id})).map(|_| ())
}

// --- The tools' own settings ------------------------------------------------------------------------------------------------------

fn on_path(tool: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|folder| folder.join(tool).is_file()))
}

/// A name that goes into a setting another program reads, some of them through a shell: nothing that needs quoting
fn plain_word(text: &str) -> bool {
    !text.is_empty() && !text.starts_with('-') && text.len() <= 100 && text.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'@' | b':' | b'+'))
}

fn read_json(path: &Path) -> Result<Value, u8> {
    match read_plain(path, 4 * 1024 * 1024)? {
        Some(bytes) => serde_json::from_slice::<Value>(&bytes).ok().filter(Value::is_object).ok_or(OURS),
        None => Ok(json!({})),
    }
}

/// A text file of the tools', which is left alone if it cannot be read as text
fn read_text(path: &Path) -> Result<String, u8> {
    match read_plain(path, 4 * 1024 * 1024)? {
        Some(bytes) => String::from_utf8(bytes).map_err(|_| OURS),
        None => Ok(String::new()),
    }
}

fn write_json(path: &Path, value: &Value) -> Result<(), u8> {
    let mut text = serde_json::to_string_pretty(value).map_err(|_| 6u8)?;
    text.push('\n');
    write_private(path, &text)
}

/// git, from the home: the folder this was started in may be a repository git will not read
fn git() -> Command {
    let mut command = Command::new("git");
    if let Ok(home) = home() {
        command.current_dir(home);
    }
    command
}

fn git_helpers() -> Vec<String> {
    let output = git().args(["config", "--global", "--get-all", "credential.helper"]).output();
    output.map(|done| String::from_utf8_lossy(&done.stdout).lines().map(str::to_string).collect()).unwrap_or_default()
}

/// Every other credential helper git would call, from every file it reads and in every spelling of the setting. If git cannot say: one.
fn git_helpers_everywhere() -> Vec<String> {
    amethystora_vault_service::credentials::other_helpers(home().ok().as_deref()).unwrap_or_else(|| vec!["(git could not say)".to_string()])
}

const AWS_MARK: &str = "# amethystora-vault";

fn aws_config() -> Result<PathBuf, u8> {
    Ok(std::env::var_os("AWS_CONFIG_FILE").map(PathBuf::from).unwrap_or(home()?.join(".aws/config")))
}

/// A block of `~/.aws/config` between marks of its own, so that taking it away leaves every other line as it was. Ini has no way to say this
/// with the file's own tools, and the `aws` command need not be there.
fn aws_on(item: &str, profile: &str) -> Result<(), u8> {
    if !plain_word(item) || !plain_word(profile) {
        return Err(USAGE);
    }
    let path = aws_config()?;
    // What cannot be read is not replaced
    let text = read_text(&path)?;
    let section = if profile == "default" { "[default]".to_string() } else { format!("[profile {profile}]") };
    if text.lines().any(|line| line.trim() == section) {
        return Err(OURS);
    }
    let mut new = text.clone();
    if !new.is_empty() && !new.ends_with('\n') {
        new.push('\n');
    }
    new.push_str(&format!("{AWS_MARK} {profile} begin\n{section}\ncredential_process = /usr/bin/ame vault get {item} aws\n{AWS_MARK} {profile} end\n"));
    write_private(&path, &new)
}

fn aws_off() -> Result<(), u8> {
    let path = aws_config()?;
    let text = read_text(&path)?;
    let mut inside = false;
    let mut kept = String::new();
    for line in text.lines() {
        if line.starts_with(AWS_MARK) {
            inside = line.ends_with(" begin");
        } else if !inside {
            kept.push_str(line);
            kept.push('\n');
        }
    }
    // A block whose end is gone is not a block this made: everything after it would go with it
    if inside {
        return Err(OURS);
    }
    write_private(&path, &kept)
}

fn kube_config() -> Result<PathBuf, u8> {
    Ok(home()?.join(".kube/config"))
}

/// Which users of the kube config ask this vault, by the lines kubectl writes for them
fn kube_is_on() -> bool {
    let text = kube_config().and_then(|path| read_text(&path)).unwrap_or_default();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    lines.windows(2).any(|pair| pair[0] == "- vault" && pair[1] == "- get")
}

const CLAUDE_START: &str = "/usr/bin/ame vault get ";

fn claude_settings() -> Result<PathBuf, u8> {
    Ok(home()?.join(".claude/settings.json"))
}

fn helper_on(tool: &str) -> bool {
    match tool {
        "git" => git_helpers().iter().any(|helper| helper == "amethystora"),
        "docker" => home().ok().and_then(|home| read_json(&home.join(".docker/config.json")).ok()).is_some_and(|config| config["credsStore"] == json!("amethystora")),
        "aws" => aws_config().and_then(|path| read_text(&path)).is_ok_and(|text| text.lines().any(|line| line.starts_with(AWS_MARK))),
        "kube" => kube_is_on(),
        "claude" => claude_settings().ok().and_then(|path| read_json(&path).ok()).is_some_and(|settings| settings["apiKeyHelper"].as_str().is_some_and(|helper| helper.starts_with(CLAUDE_START))),
        _ => false,
    }
}

const TOOLS: [(&str, &str); 5] = [("git", "git"), ("docker", "docker"), ("aws", "aws"), ("kube", "kubectl"), ("claude", "claude")];

fn status_of_helpers(args: &Args) {
    let rows: Vec<(&str, bool, bool)> = TOOLS.iter().map(|(name, program)| (*name, on_path(program), helper_on(name))).collect();
    if args.option("json").is_some() {
        let list: Vec<Value> = rows.iter().map(|(name, installed, on)| json!({"tool": name, "installed": installed, "on": on})).collect();
        println!("{}", json!({"ok": true, "helpers": list}));
    } else {
        for (name, installed, on) in rows {
            println!("{name}\t{}\t{}", if installed { "installed" } else { "not-installed" }, if on { "on" } else { "off" });
        }
    }
}

/// Says which are set up, and writes a tool's settings only when asked. Each option here was checked with the tool's own help where the
/// build's container has it: see plans/vault/work/secret.md for which.
pub fn helpers(args: &Args) -> Result<(), u8> {
    let Some(tool) = args.word(1) else {
        status_of_helpers(args);
        return Ok(());
    };
    let switch = args.word(2).ok_or(USAGE)?;
    let (item, more) = (args.word(3), args.word(4));
    let succeeded = |command: &mut Command| command.status().map_err(|_| NOT_INSTALLED).and_then(|status| if status.success() { Ok(()) } else { Err(6) });
    match (tool, switch) {
        ("git", "on") => {
            if git_helpers().iter().any(|helper| helper == "amethystora") {
                return Ok(());
            }
            // git hands what worked to every helper it has, and this one answers with a password from the vault: another that stores would keep it
            if let Some(other) = git_helpers_everywhere().into_iter().next() {
                eprintln!("{}", args.say("say-other-helper", "git already asks another credential helper, {helper}, and would hand it what the vault answers. Remove that one from git's settings first, or keep the vault's logins out of git.").replace("{helper}", &other));
                return Err(OURS);
            }
            succeeded(git().args(["config", "--global", "--add", "credential.helper", "amethystora"]))
        }
        ("git", "off") => {
            if !git_helpers().iter().any(|helper| helper == "amethystora") {
                return Ok(());
            }
            succeeded(git().args(["config", "--global", "--unset-all", "credential.helper", "^amethystora$"]))
        }
        ("docker", "on") => {
            let path = home()?.join(".docker/config.json");
            let mut config = read_json(&path)?;
            if config["credsStore"].as_str().is_some_and(|store| store != "amethystora") {
                return Err(OURS);
            }
            config["credsStore"] = json!("amethystora");
            write_json(&path, &config)
        }
        ("docker", "off") => {
            let path = home()?.join(".docker/config.json");
            let mut config = read_json(&path)?;
            if config["credsStore"] == json!("amethystora") {
                config.as_object_mut().ok_or(6u8)?.remove("credsStore");
                write_json(&path, &config)?;
            }
            Ok(())
        }
        ("aws", "on") => aws_on(item.ok_or(USAGE)?, more.unwrap_or("default")),
        ("aws", "off") => aws_off(),
        ("kube", "on") => {
            let item = item.ok_or(USAGE)?;
            let user = more.unwrap_or(item);
            if !plain_word(item) || !plain_word(user) {
                return Err(USAGE);
            }
            if !on_path("kubectl") {
                return Err(NOT_INSTALLED);
            }
            let (user_arg, item_arg) = (user.to_string(), format!("--exec-arg={item}"));
            succeeded(Command::new("kubectl").args(["config", "set-credentials", &user_arg, "--exec-api-version=client.authentication.k8s.io/v1", "--exec-command=/usr/bin/ame", "--exec-arg=vault", "--exec-arg=get", &item_arg, "--exec-arg=kube", "--exec-interactive-mode=Never"]))
        }
        ("kube", "off") => {
            let user = item.ok_or(USAGE)?;
            if !plain_word(user) {
                return Err(USAGE);
            }
            if !on_path("kubectl") {
                return Err(NOT_INSTALLED);
            }
            succeeded(Command::new("kubectl").args(["config", "unset", &format!("users.{user}.exec")]))
        }
        ("claude", "on") => {
            let item = item.ok_or(USAGE)?;
            if !plain_word(item) {
                return Err(USAGE);
            }
            let path = claude_settings()?;
            let mut settings = read_json(&path)?;
            if settings["apiKeyHelper"].as_str().is_some_and(|helper| !helper.starts_with(CLAUDE_START)) {
                return Err(OURS);
            }
            settings["apiKeyHelper"] = json!(format!("{CLAUDE_START}{item}"));
            write_json(&path, &settings)
        }
        ("claude", "off") => {
            let path = claude_settings()?;
            let mut settings = read_json(&path)?;
            if settings["apiKeyHelper"].as_str().is_some_and(|helper| helper.starts_with(CLAUDE_START)) {
                settings.as_object_mut().ok_or(6u8)?.remove("apiKeyHelper");
                write_json(&path, &settings)?;
            }
            Ok(())
        }
        _ => Err(USAGE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_variable_that_steers_programs_is_refused_whatever_its_name_came_from() {
        for name in [
            "PATH", "IFS", "ENV", "BASH_ENV", "LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT", "DYLD_INSERT_LIBRARIES", "GCONV_PATH", "LOCPATH", "NLSPATH", "NODE_OPTIONS", "JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS", "RES_OPTIONS",
            "PYTHONPATH", "PYTHONSTARTUP", "PERL5OPT", "PERL5LIB", "RUBYOPT", "RUBYLIB", "GIT_SSH_COMMAND", "GIT_ASKPASS", "GIT_CONFIG_GLOBAL", "SSH_ASKPASS", "PROMPT_COMMAND", "PS4", "HOME", "TMPDIR", "XDG_CONFIG_HOME", "MALLOC_CHECK_",
            "BASH_FUNC_x%%", "SHELL", "EDITOR", "CLASSPATH", "SSL_CERT_FILE", "SSH_AUTH_SOCK",
        ] {
            assert!(steers(name), "{name} is not refused");
        }
        for name in ["GITHUB_TOKEN", "NPM_TOKEN", "AWS_SECRET_ACCESS_KEY", "MY_VAR", "API_KEY", "ANTHROPIC_API_KEY", "VAULT_TOKEN", "_"] {
            assert!(!steers(name), "{name} is refused");
        }
        // The name an item gives
        assert!(steers(&variable_for("path")) && steers(&variable_for("ld preload")) && steers(&variable_for("Node Options")));
        assert!(!steers(&variable_for("github token")));
    }
}
