//! amethystora-move: the two ends of Move to this computer (R7-25), run in a terminal by `ame system move`.
//!
//!   amethystora-move receive            on the new computer: waits for the old one, shows six digits, asks what to bring
//!   amethystora-move send [--to ADDR]   on the old computer: finds the new one (or calls ADDR), shows six digits, sends
//!
//! Both need a person at a terminal: the digits are compared by eye and the answers are typed, and there is no option to
//! skip either. Both work only on a network named home. The key lives for this run alone and is never written.

use std::io::{BufRead, IsTerminal, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use amethystora_move::discover::{self, Found};
use amethystora_move::plan::Item;
use amethystora_move::session::{self, Offered, Summary, Ui};
use amethystora_move::text::{clean, size, tr, trf};
use amethystora_move::zone::{Firewalld, Home};
use amethystora_move::PORT;

const SETUP: &str = "/usr/libexec/amethystora-setup";
/// How long the new computer waits for the old one, and how long either waits on the other
const WAIT: Duration = Duration::from_secs(600);
const SILENCE: Duration = Duration::from_secs(900);

unsafe extern "C" {
    fn setlocale(category: i32, locale: *const std::ffi::c_char) -> *const std::ffi::c_char;
}

/// A move runs behind everything else: idle disk priority and a low processor priority, so that the desktop is never what waits
fn be_gentle() {
    // SAFETY: plain system calls on this process; a failure only means the priority stays as it was
    unsafe {
        libc::syscall(libc::SYS_ioprio_set, 1, 0, 3 << 13);
        libc::setpriority(libc::PRIO_PROCESS, 0, 10);
    }
}

struct Terminal {
    last: Instant,
    shown: bool,
}

impl Terminal {
    fn new() -> Terminal {
        Terminal { last: Instant::now() - Duration::from_secs(1), shown: false }
    }

    fn ask(&self, question: &str, default_yes: bool) -> bool {
        let hint = if default_yes { tr("[Y/n]") } else { tr("[y/N]") };
        loop {
            eprint!("{question} {hint} ");
            let _ = std::io::stderr().flush();
            let mut line = String::new();
            if std::io::stdin().lock().read_line(&mut line).unwrap_or(0) == 0 {
                return false;
            }
            match line.trim().to_lowercase().as_str() {
                "" => return default_yes,
                "y" | "yes" | "j" | "o" | "s" | "ja" | "oui" | "si" | "sí" | "да" | "نعم" => return true,
                "n" | "no" | "non" | "nein" | "нет" | "لا" => return false,
                _ => continue,
            }
        }
    }
}

impl Ui for Terminal {
    fn confirm_digits(&mut self, digits: &str) -> bool {
        let spaced: Vec<String> = digits.chars().map(String::from).collect();
        eprintln!();
        eprintln!("    {}", spaced.join("  "));
        eprintln!();
        self.ask(&tr("Does the other computer show the same six digits?"), false)
    }

    fn choose(&mut self, from: &str, offered: &[Offered]) -> Vec<Item> {
        eprintln!("{}", trf("Moving from “{name}”", &[("name", from)]));
        let mut chosen = Vec::new();
        for o in offered {
            let question = match o.item {
                Item::Home => trf("Bring the home folder ({size}, {files} files)?", &[("size", &size(o.bytes)), ("files", &o.files.to_string())]),
                Item::Setup => tr("Bring the setup file, which names its apps, theme and shortcuts?"),
            };
            if self.ask(&question, true) {
                chosen.push(o.item);
            }
        }
        chosen
    }

    fn say(&mut self, line: &str) {
        eprintln!("{line}");
    }

    fn progress(&mut self, done: u64, total: u64) {
        // At most five times a second, and a last line when it is done
        if self.last.elapsed() < Duration::from_millis(200) && done < total {
            return;
        }
        self.last = Instant::now();
        let percent = if total == 0 { 100 } else { done.saturating_mul(100) / total };
        eprint!("\r  {percent:>3} %   {} / {}      ", size(done), size(total));
        let _ = std::io::stderr().flush();
        self.shown = true;
        if done >= total {
            eprintln!();
            self.shown = false;
        }
    }
}

fn host_name() -> String {
    std::fs::read_to_string("/etc/hostname").map(|s| clean(s.trim())).unwrap_or_else(|_| "this computer".into())
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).filter(|h| h.is_absolute() && h.is_dir())
}

fn report(summary: &Summary) {
    println!("{}", trf("Brought over: {written} files ({size}).", &[("written", &summary.written.to_string()), ("size", &size(summary.bytes))]));
    if summary.kept > 0 {
        println!("{}", trf("{kept} files were already here, and were kept as they are.", &[("kept", &summary.kept.to_string())]));
    }
    if summary.refused > 0 {
        println!("{}", trf("{refused} things were left out: passwords, keys and places that start programs never cross.", &[("refused", &summary.refused.to_string())]));
    }
}

fn receive() -> Result<(), String> {
    let home = home_dir().ok_or_else(|| tr("There is no home folder to move into."))?;
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, PORT)).map_err(|e| trf("Cannot listen on port {port}: {error}", &[("port", &PORT.to_string()), ("error", &e.to_string())]))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let udp = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, PORT)).map_err(|e| e.to_string())?;
    let checker: Arc<dyn Home + Send + Sync> = Arc::new(Firewalld);
    let mut answering = Some(discover::Answering::start(udp, PORT, host_name(), checker.clone()).map_err(|e| e.to_string())?);
    eprintln!("{}", tr("Waiting for the old computer. On it, run: ame system move send"));
    eprintln!("{}", tr("Only a computer on a network you named home can reach this one."));
    let until = Instant::now() + WAIT;
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, from)) => {
                if checker.is_home(from.ip()) {
                    break stream;
                }
                // Not a network named home: dropped before a word is read
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= until {
                    return Err(tr("Nobody called, so the wait is over."));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    // Nobody is asked or answered after the first one: the move listens for the wait, and for no longer
    drop(answering.take());
    drop(listener);
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(SILENCE)).map_err(|e| e.to_string())?;
    let mut keys = session::handshake_receiver(&mut stream).map_err(|e| e.0)?;
    let mut ui = Terminal::new();
    let summary = session::receive(&mut stream, &mut keys, &mut ui, &home).map_err(|e| e.0)?;
    report(&summary);
    if home.join("From old computer/setup.json").is_file() {
        println!("{}", tr("The setup file is in “From old computer”. To bring its apps, theme and shortcuts, which asks first: ame setup apply \"$HOME/From old computer/setup.json\""));
    }
    Ok(())
}

/// The setup file as `ame setup save` writes it, or nothing when it cannot be made
fn setup_file() -> Option<Vec<u8>> {
    let out = Command::new(SETUP).args(["save", "-"]).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    (out.status.success() && out.stdout.len() <= 1 << 20 && serde_json::from_slice::<serde_json::Value>(&out.stdout).is_ok()).then_some(out.stdout)
}

fn parse_address(text: &str) -> Result<SocketAddr, String> {
    if let Ok(addr) = text.parse::<SocketAddr>() {
        return Ok(addr);
    }
    text.parse::<IpAddr>().map(|ip| SocketAddr::new(ip, PORT)).map_err(|_| trf("“{address}” is not an address of the form 192.168.1.20.", &[("address", &clean(text))]))
}

fn send(to: Option<&str>) -> Result<(), String> {
    let home = home_dir().ok_or_else(|| tr("There is no home folder to move from."))?;
    let checker = Firewalld;
    let target = match to {
        Some(text) => {
            let addr = parse_address(text)?;
            Found { name: addr.ip().to_string(), addr }
        }
        None => {
            eprintln!("{}", tr("Looking for the new computer on this network…"));
            let mut found = discover::find(SocketAddr::new(Ipv4Addr::BROADCAST.into(), PORT), Duration::from_secs(3), &checker, PORT).map_err(|e| e.to_string())?;
            match found.len() {
                0 => return Err(tr("No computer answered. On the new one, run: ame system move receive. Both must be on a network named home.")),
                1 => found.remove(0),
                _ => {
                    for (i, f) in found.iter().enumerate() {
                        eprintln!("  {}  {}  ({})", i + 1, f.name, f.addr.ip());
                    }
                    eprint!("{} ", tr("Which one? Its number:"));
                    let _ = std::io::stderr().flush();
                    let mut line = String::new();
                    std::io::stdin().lock().read_line(&mut line).map_err(|e| e.to_string())?;
                    let at: usize = line.trim().parse().ok().filter(|n| (1..=found.len()).contains(n)).ok_or_else(|| tr("That is not one of the numbers."))?;
                    found.remove(at - 1)
                }
            }
        }
    };
    if !checker.is_home(target.addr.ip()) {
        return Err(tr("That computer is not on a network you named home, so nothing is sent to it."));
    }
    eprintln!("{}", trf("Calling “{name}”…", &[("name", &target.name)]));
    let mut stream = TcpStream::connect_timeout(&target.addr, Duration::from_secs(10)).map_err(|e| trf("Cannot reach it: {error}", &[("error", &e.to_string())]))?;
    stream.set_read_timeout(Some(SILENCE)).map_err(|e| e.to_string())?;
    let mut keys = session::handshake_sender(&mut stream).map_err(|e| e.0)?;
    let setup = setup_file();
    let mut ui = Terminal::new();
    let summary = session::send(&mut stream, &mut keys, &mut ui, &home, &host_name(), setup.as_deref()).map_err(|e| e.0)?;
    report(&summary);
    println!("{}", tr("Nothing on this computer was changed."));
    Ok(())
}

fn main() -> ExitCode {
    // SAFETY: the empty string asks for the environment's locale, before anything else reads it
    unsafe { setlocale(libc::LC_ALL, c"".as_ptr()) };
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Nothing here runs without a person looking: the digits are compared by eye and each question is typed
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        eprintln!("amethystora-move: needs a terminal, with a person at it");
        return ExitCode::from(64);
    }
    be_gentle();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["receive"] => receive(),
        ["send"] => send(None),
        ["send", "--to", address] => send(Some(address)),
        _ => {
            eprintln!("usage: amethystora-move receive | send [--to ADDRESS]");
            return ExitCode::from(64);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("amethystora-move: {why}");
            ExitCode::FAILURE
        }
    }
}
