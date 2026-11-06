//! amethystora-connect: the phone link's program. See plans/PLAN-amethystora-connect.md.
//!
//! It runs as the account, on the session bus, as the user unit `amethystora-connect.service`, and only while
//! the person has the phone link on (`ame desktop phone on`). With no arguments it is that service. The
//! arguments are for the tests and for a lab, and none of them is something a person needs:
//!
//!   --config-dir DIR --state-dir DIR --download-dir DIR --pictures-dir DIR
//!                             where it keeps things
//!   --home-interfaces A,B     announce on these interfaces without asking the firewall which are home
//!   --allow-loopback          talk to 127.0.0.1 too (a test of two copies on one machine)
//!   --app-command PROGRAM     what the notifications start (the app, by default)
//!   --no-launcher             start it directly, not as a unit of the user's manager (systemd-run)
//!   --app-exe PATH, --module-exe PATH
//!                             the programs the bus lets call the service (see dbus.rs)
//!   --show-silent, --show-ongoing
//!                             show the phone's notifications that it marks silent, or ongoing (not shown as shipped)
//!   --verbose                 say what happens

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;

use amethystora_connect::dbus::{self, BUS_NAME};
use amethystora_connect::notify::Desktop;
use amethystora_connect::ask::AskFirst;
use amethystora_connect::service::{Config, PhotoFolder, Service, Settings};
use gio::prelude::*;

const SCHEMA: &str = "org.amethystora.Connect";

fn usage(why: &str) -> ExitCode {
    eprintln!("amethystora-connect: {why}");
    eprintln!("usage: amethystora-connect [--verbose]  (the other options are for tests: see the top of main.rs)");
    ExitCode::from(64)
}

/// SMBIOS's chassis type, which says laptop, tablet or desktop
fn chassis() -> Option<u32> {
    std::fs::read_to_string("/sys/class/dmi/id/chassis_type").ok()?.trim().parse().ok()
}

/// The machine's pretty host name, if the administrator set one
fn pretty_host() -> Option<String> {
    let info = std::fs::read_to_string("/etc/machine-info").ok()?;
    info.lines().find_map(|line| line.strip_prefix("PRETTY_HOSTNAME=")).map(|v| v.trim().trim_matches(|c| c == '"' || c == '\'').to_string()).filter(|v| !v.is_empty())
}

fn main() -> ExitCode {
    // SAFETY: setlocale with an empty string reads the environment; nothing else is running yet
    unsafe {
        libc::setlocale(libc::LC_ALL, c"".as_ptr());
    }
    // Before any thread exists, so that none of them takes a signal the main loop is to hear
    let stop_signals = stop_signals();
    let mut cfg = Config::production();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        let result: Result<(), String> = match arg.as_str() {
            "--config-dir" => value("--config-dir").map(|v| cfg.config_dir = PathBuf::from(v)),
            "--state-dir" => value("--state-dir").map(|v| cfg.state_dir = PathBuf::from(v)),
            "--download-dir" => value("--download-dir").map(|v| cfg.download_dir = Some(PathBuf::from(v))),
            "--pictures-dir" => value("--pictures-dir").map(|v| cfg.pictures_dir = Some(PathBuf::from(v))),
            "--home-interfaces" => value("--home-interfaces").map(|v| cfg.home_interfaces = Some(v.split(',').filter(|s| !s.is_empty()).map(str::to_string).collect())),
            "--app-command" => value("--app-command").map(|v| cfg.app_command = vec![v]),
            "--app-exe" => value("--app-exe").map(|v| cfg.app_exe = v),
            "--module-exe" => value("--module-exe").map(|v| cfg.module_exe = v),
            "--no-launcher" => {
                cfg.launcher = Vec::new();
                Ok(())
            }
            "--allow-loopback" => {
                cfg.allow_loopback = true;
                Ok(())
            }
            "--verbose" => {
                cfg.verbose = true;
                Ok(())
            }
            "--show-silent" => {
                cfg.show_silent = true;
                Ok(())
            }
            "--show-ongoing" => {
                cfg.show_ongoing = true;
                Ok(())
            }
            other => Err(format!("unknown argument {other}")),
        };
        if let Err(why) = result {
            return usage(&why);
        }
    }
    cfg.kind = identity_kind();

    let bus = match gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) {
        Ok(bus) => bus,
        Err(e) => {
            eprintln!("amethystora-connect: no session bus: {}", e.message());
            return ExitCode::FAILURE;
        }
    };
    let settings_object = gio::SettingsSchemaSource::default().and_then(|source| source.lookup(SCHEMA, true)).map(|schema| gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None));
    let settings = match &settings_object {
        Some(s) => Settings {
            name: s.string("device-name").to_string(),
            clipboard_send: s.boolean("clipboard-send"),
            clipboard_receive: s.boolean("clipboard-receive"),
            photo_folder: PhotoFolder::from_key(&s.string("photo-folder")),
            computers: s.boolean("computers"),
            music: s.boolean("music"),
            clicker: s.boolean("clicker"),
            codes: s.boolean("codes"),
            photo: s.boolean("photo"),
            ask_first: AskFirst::from_key(&s.string("ask-first")),
        },
        None => Settings::default(),
    };
    let notifier = Desktop::new(bus.clone());
    let service = match Service::new(cfg, Box::new(notifier), settings, pretty_host()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("amethystora-connect: {e}");
            return ExitCode::FAILURE;
        }
    };
    let main_loop = glib::MainLoop::new(None, false);
    service.start();
    if let Err(e) = dbus::export(&service, &bus) {
        eprintln!("amethystora-connect: cannot put the program on the bus: {}", e.message());
        return ExitCode::FAILURE;
    }

    // The settings are watched: ame desktop phone writes them
    if let Some(s) = settings_object {
        let weak = Rc::downgrade(&service);
        let read = {
            let s = s.clone();
            move |service: &Rc<Service>| {
                service.set_clipboard_switches(s.boolean("clipboard-send"), s.boolean("clipboard-receive"));
                service.set_photo_folder(PhotoFolder::from_key(&s.string("photo-folder")));
                service.set_computers(s.boolean("computers"));
                service.set_media_switches(s.boolean("music"), s.boolean("clicker"));
                service.codes.set(s.boolean("codes"));
                service.set_photo(s.boolean("photo"));
                service.ask_first.set(AskFirst::from_key(&s.string("ask-first")));
                service.set_name(&s.string("device-name"), pretty_host().as_deref());
            }
        };
        s.connect_changed(None, move |_, _| {
            if let Some(service) = weak.upgrade() {
                read(&service);
            }
        });
        // Kept alive for as long as the program runs
        std::mem::forget(s);
    }

    // The name is asked for last, once everything is in place: systemd (Type=dbus) takes the service as started when it
    // has it. The variant that makes its own connection to the bus is the one that is told when the bus is gone
    // (the connection is then none), which the other form does not take.
    let lost = Rc::new(RefCell::new(false));
    let quit = main_loop.clone();
    let lost_flag = lost.clone();
    let _owner = gio::bus_own_name(
        gio::BusType::Session,
        BUS_NAME,
        gio::BusNameOwnerFlags::DO_NOT_QUEUE,
        |_, _| {},
        |_, _| {},
        move |_, name| {
            eprintln!("amethystora-connect: the name {name} is taken or was lost; another copy is running, or the bus is gone");
            *lost_flag.borrow_mut() = true;
            quit.quit();
        },
    );

    // Stopping is orderly: listeners first, then every connection closed
    if let Some(fd) = stop_signals {
        let (service, main_loop) = (service.clone(), main_loop.clone());
        // A thread that does nothing but wait for the signal, and says so to the main loop
        let (tell, heard) = futures_channel::oneshot::channel::<()>();
        std::thread::spawn(move || {
            use std::io::Read;
            // One signalfd_siginfo is 128 bytes; any of the three ends the program
            let mut record = [0u8; 128];
            let _ = std::fs::File::from(fd).read(&mut record);
            let _ = tell.send(());
        });
        glib::spawn_future_local(async move {
            let _ = heard.await;
            service.stop().await;
            main_loop.quit();
        });
    }
    main_loop.run();
    if *lost.borrow() { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

fn identity_kind() -> String {
    amethystora_connect::identity::kind_of_chassis(chassis()).to_string()
}

/// SIGTERM, SIGINT and SIGHUP, blocked and read from a file descriptor, so that the main loop hears them
fn stop_signals() -> Option<std::os::fd::OwnedFd> {
    use std::os::fd::FromRawFd;
    // SAFETY: the set is initialised by sigemptyset before it is used; signalfd returns a new descriptor
    // that this function owns
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            libc::sigaddset(&mut set, signal);
        }
        if libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) != 0 {
            return None;
        }
        let fd = libc::signalfd(-1, &set, libc::SFD_CLOEXEC);
        (fd >= 0).then(|| std::os::fd::OwnedFd::from_raw_fd(fd))
    }
}
