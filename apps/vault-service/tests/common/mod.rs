//! A stand-in for polkit and logind, a temporary state folder, and the service on a real socket.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use amethystora_vault_core::seal;
use amethystora_vault_core::slots::fake::FakeChip;
use amethystora_vault_service::outside::{person_of, Ask, CannotWatch, Event, Outside, Person};
use amethystora_vault_service::peer::Peer;
use amethystora_vault_service::server::Server;
use amethystora_vault_service::service::{Config, Service};

pub struct Folder(pub PathBuf);

impl Folder {
    pub fn new(name: &str) -> Folder {
        let path = std::env::temp_dir().join(format!("avs-{name}-{}", seal::random_hex(4)));
        std::fs::create_dir_all(&path).unwrap();
        Folder(path)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What the outside was asked and what it answers
#[derive(Default)]
pub struct Said {
    /// What the person answers: yes unless told otherwise
    pub refuse: bool,
    /// Never answers: the person is thinking
    pub silent: bool,
    pub asked: Vec<Ask>,
    pub cancelled: Vec<u64>,
    pub watching: bool,
    pub watch_fails: bool,
    /// The screen is locked: nothing is put to anyone, and nothing opens
    pub locked: bool,
    /// Accounts with no login session, which the service does not serve
    pub no_session: Vec<u32>,
    /// Accounts with no session that linger: served the background key alone
    pub lingering: Vec<u32>,
    /// How often the lock was looked at
    pub unlocked_asked: usize,
    pub events: VecDeque<Event>,
    /// Who the account is, in place of what the machine says: a guest, a person with a name
    pub person: Option<Person>,
}

/// Writes a byte to the pipe that wakes the loop
#[derive(Clone)]
pub struct Handle {
    pub said: Arc<Mutex<Said>>,
    wake: Arc<OwnedFd>,
}

impl Handle {
    pub fn push(&self, event: Event) {
        self.said.lock().unwrap().events.push_back(event);
        // SAFETY: one byte, from a live buffer
        unsafe { libc::write(self.wake.as_raw_fd(), [1u8].as_ptr().cast(), 1) };
    }

    pub fn asked(&self) -> usize {
        self.said.lock().unwrap().asked.len()
    }
}

pub struct FakeOutside {
    handle: Handle,
    read: OwnedFd,
}

pub fn fake() -> (FakeOutside, Handle) {
    let mut pipe = [0; 2];
    // SAFETY: pipe2 writes two descriptors, which are then owned
    let (read, write) = unsafe {
        assert_eq!(libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK), 0);
        (OwnedFd::from_raw_fd(pipe[0]), OwnedFd::from_raw_fd(pipe[1]))
    };
    let handle = Handle { said: Arc::new(Mutex::new(Said::default())), wake: Arc::new(write) };
    (FakeOutside { handle: handle.clone(), read }, handle)
}

impl Outside for FakeOutside {
    fn wait_on(&self) -> Option<(RawFd, i16)> {
        Some((self.read.as_raw_fd(), libc::POLLIN))
    }

    fn ready(&mut self) -> Vec<Event> {
        let mut bytes = [0u8; 64];
        // SAFETY: into a live buffer, from a non-blocking pipe
        while unsafe { libc::read(self.read.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len()) } > 0 {}
        self.handle.said.lock().unwrap().events.drain(..).collect()
    }

    fn watch(&mut self) -> Result<(), CannotWatch> {
        let mut said = self.handle.said.lock().unwrap();
        if said.watch_fails {
            return Err(CannotWatch);
        }
        said.watching = true;
        Ok(())
    }

    fn unwatch(&mut self) {
        self.handle.said.lock().unwrap().watching = false;
    }

    fn approve(&mut self, token: u64, ask: &Ask, _peer: &Peer) {
        let (silent, refuse, locked) = {
            let mut said = self.handle.said.lock().unwrap();
            if said.locked {
                (false, false, true)
            } else {
                said.asked.push(ask.clone());
                (said.silent, said.refuse, false)
            }
        };
        if locked {
            self.handle.push(Event::NotAsked(token));
        } else if !silent {
            self.handle.push(Event::Approved(token, !refuse));
        }
    }

    fn when_unlocked(&mut self, token: u64, _peer: &Peer) {
        let locked = {
            let mut said = self.handle.said.lock().unwrap();
            said.unlocked_asked += 1;
            said.locked
        };
        self.handle.push(if locked { Event::NotAsked(token) } else { Event::Approved(token, true) });
    }

    fn has_session(&self, uid: u32) -> bool {
        !self.handle.said.lock().unwrap().no_session.contains(&uid)
    }

    fn lingering(&self, uid: u32) -> bool {
        self.handle.said.lock().unwrap().lingering.contains(&uid)
    }

    fn cancel(&mut self, token: u64) {
        self.handle.said.lock().unwrap().cancelled.push(token);
    }

    fn person(&self, uid: u32) -> Option<Person> {
        self.handle.said.lock().unwrap().person.clone().or_else(|| person_of(uid))
    }
}

/// The service on a real socket, on a thread, with the stand-ins, at a low cost of scrypt unless told otherwise
pub struct Running {
    pub folder: Folder,
    pub socket: PathBuf,
    pub outside: Handle,
    stop: OwnedFd,
    thread: Option<std::thread::JoinHandle<()>>,
}

pub fn start(name: &str) -> Running {
    start_with(name, 10, Duration::from_secs(600))
}

pub fn start_with(name: &str, log_n: u8, idle_exit: Duration) -> Running {
    start_worker(name, log_n, idle_exit, env!("CARGO_BIN_EXE_amethystora-vault-work").into())
}

/// With a job process of the test's own choosing
pub fn start_worker(name: &str, log_n: u8, idle_exit: Duration, worker: PathBuf) -> Running {
    let folder = Folder::new(name);
    let socket = folder.0.join("vault.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let agent = UnixListener::bind(folder.0.join("vault-ssh.sock")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o666)).unwrap();
    std::fs::set_permissions(folder.0.join("vault-ssh.sock"), std::fs::Permissions::from_mode(0o666)).unwrap();
    std::fs::set_permissions(&folder.0, std::fs::Permissions::from_mode(0o755)).unwrap();
    let state = folder.0.join("state");
    std::fs::create_dir_all(&state).unwrap();
    let mut pipe = [0; 2];
    // SAFETY: pipe2 writes two descriptors, which are then owned
    let (stop_read, stop_write) = unsafe {
        assert_eq!(libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC), 0);
        (OwnedFd::from_raw_fd(pipe[0]), OwnedFd::from_raw_fd(pipe[1]))
    };
    let (outside, handle) = fake();
    let thread = std::thread::spawn(move || {
        let service = Service::new(Config { state, log_n, worker }, Arc::new(FakeChip::new()), Box::new(outside));
        let server = Server::new(listener, Some(stop_read.as_raw_fd()), service, idle_exit).unwrap().with_ssh(agent);
        server.run();
        drop(stop_read);
    });
    Running { folder, socket, outside: handle, stop: stop_write, thread: Some(thread) }
}

impl Running {
    pub fn ask(&self, mut request: Value) -> Value {
        request["v"] = json!(1);
        amethystora_vault_client::exchange(&self.socket, &request, Duration::from_secs(30)).unwrap()
    }

    pub fn state(&self) -> PathBuf {
        self.folder.0.join("state")
    }

    pub fn stop(mut self) -> Self {
        // SAFETY: one byte, from a live buffer
        unsafe { libc::write(self.stop.as_raw_fd(), [1u8].as_ptr().cast(), 1) };
        self.thread.take().unwrap().join().unwrap();
        self
    }

    /// Sets the account up with this password and types the recovery key back: the key is returned
    pub fn set_up(&self, password: &str) -> String {
        let answer = self.ask(json!({"op": "setup", "method": "password", "name": "Personal", "secret": b64(password)}));
        assert_eq!(answer["ok"], json!(true), "{answer}");
        let recovery = answer["recovery"].as_str().unwrap().to_string();
        let answer = self.ask(json!({"op": "confirm", "recovery": b64(&recovery)}));
        assert_eq!(answer["ok"], json!(true), "{answer}");
        recovery
    }

    pub fn new_vault(&self, name: &str, hidden: bool, password: &str) -> String {
        let answer = self.ask(json!({"op": "new", "name": name, "hidden": hidden, "secret": b64(password)}));
        assert_eq!(answer["ok"], json!(true), "{answer}");
        let recovery = answer["recovery"].as_str().unwrap().to_string();
        let answer = self.ask(json!({"op": "confirm", "recovery": b64(&recovery)}));
        assert_eq!(answer["ok"], json!(true), "{answer}");
        recovery
    }

    pub fn wait_until(&self, what: impl Fn(&Said) -> bool) {
        for _ in 0..500 {
            if what(&self.outside.said.lock().unwrap()) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("it never happened");
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        // SAFETY: as in stop
        unsafe { libc::write(self.stop.as_raw_fd(), [1u8].as_ptr().cast(), 1) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn b64(text: &str) -> String {
    amethystora_vault_core::protocol::encode(text.as_bytes())
}

pub fn why(answer: &Value) -> &str {
    answer["why"].as_str().unwrap_or("")
}

pub fn exists(path: &Path) -> bool {
    path.exists()
}

/// Every file under a folder, for looking through what is on the disk
pub fn files_under(folder: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut folders = vec![folder.to_path_buf()];
    while let Some(next) = folders.pop() {
        for entry in std::fs::read_dir(next).into_iter().flatten().flatten() {
            if entry.path().is_dir() {
                folders.push(entry.path());
            } else {
                found.push(entry.path());
            }
        }
    }
    found
}
