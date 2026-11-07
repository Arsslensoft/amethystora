//! The one loop: `poll` over the listening socket, the clients, the bus and the work thread's pipe, with one deadline at most
//! and no polling. Only a person (polkit) or the disk's slowest write can make a client wait, and neither stalls the others:
//! an approval is a message on the bus, and scrypt runs on a thread of its own, one at a time, since each takes 128 MiB.
//!
//! Every account gets its turn: lines are taken a share per account per round, and queued work round the accounts, so that one
//! account's programs cannot hold up another's. A connection that asks nothing is let go after a few seconds, and an account
//! with no session at the machine (a container's own uids among them) is not taken at all.
//!
//! Exits when nothing is open and nothing has been asked for a while. systemd keeps the socket, and starts it again for the
//! next connection.

use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use zeroize::Zeroize;

use amethystora_vault_core::protocol::{self, Refusal, MAX_LINE};

use crate::outside::Event;
use crate::peer::Peer;
use crate::service::{Next, Service, Then};
use crate::ssh;

/// How long the service stays with nothing open and nothing asked: the budget's 60 s, less a margin
pub const IDLE_EXIT: Duration = Duration::from_secs(45);
const MOST_CLIENTS: usize = 128;
const CLIENTS_PER_ACCOUNT: usize = 16;
/// Work waiting for the thread, an account's and everyone's: each is a request somebody is waiting on
const JOBS_PER_ACCOUNT: usize = 4;
const MOST_JOBS: usize = 32;
/// An answer past which the heap is trimmed once it has been written (a list of every account, a chunk of a backup)
const LARGE_ANSWER: usize = 64 * 1024;
/// A connection with nothing asked of the service, a line started included, is let go after this
const IDLE_CONNECTION: Duration = Duration::from_secs(5);
/// An answer not taken
const STALLED: Duration = Duration::from_secs(30);
/// A person who has not answered has gone: polkit's dialog goes with the cancellation
const APPROVAL_PATIENCE: Duration = Duration::from_secs(5 * 60);
const READ_AT_ONCE: usize = 64 * 1024;
const LINES_AT_ONCE: usize = 32;
/// What one account's lines may come to in one round, before the others have had theirs
const BYTES_A_ROUND: usize = 256 * 1024;
/// A program the person said no to this often in a minute is not put in front of them again for a minute
const REFUSALS: usize = 3;
const REFUSAL_MEMORY: Duration = Duration::from_secs(60);

type Done = Box<dyn Any + Send>;

struct Client {
    stream: UnixStream,
    peer: Rc<Peer>,
    input: Vec<u8>,
    output: Vec<u8>,
    written: usize,
    /// Came in on the SSH agent's socket, so speaks its framing and nothing else
    ssh: bool,
    /// What it has told the agent about itself
    bound: ssh::Conn,
    /// Waiting for an approval or for work: nothing more is read until it is answered
    busy: bool,
    /// Say what is in output, and hang up
    closing: bool,
    /// When it is let go if nothing changes
    deadline: Option<Instant>,
}

impl Client {
    fn has_line(&self) -> bool {
        if self.ssh { ssh::ready(&self.input) } else { self.input.contains(&b'\n') }
    }

    fn can_take_line(&self) -> bool {
        !self.busy && !self.closing && self.written >= self.output.len() && self.has_line()
    }
}

enum Waiting {
    /// A person asked, or only logind about the lock (`asker` None)
    Approval { client: u64, uid: u32, then: Then<bool>, until: Instant, closes: u64, asker: Option<(u32, String)> },
    Work { client: u64, uid: u32, then: Then<Done>, closes: u64 },
}

/// Work on a thread of its own (`Next::Slow`, and the runs sync starts by itself): who is waiting for it, if anyone, and what
/// follows
enum Slow {
    Client { client: u64, then: Then<Done> },
    Own { then: crate::sync::Follow },
}

/// A job on the thread's queue
struct Queued {
    token: u64,
    uid: u32,
    job: Box<dyn FnOnce() -> Done + Send>,
}

pub struct Server {
    listener: UnixListener,
    /// The SSH agent's socket, on a system that has its socket unit
    ssh: Option<UnixListener>,
    /// Readable means stop: a signalfd for SIGTERM, or a test's pipe
    stop: Option<RawFd>,
    service: Service,
    clients: HashMap<u64, Client>,
    waiting: HashMap<u64, Waiting>,
    slow: HashMap<u64, Slow>,
    queue: VecDeque<Queued>,
    working: bool,
    /// Whose work ran last, so that the next is somebody else's if anybody else has some
    last_worker: Option<u32>,
    finished: Arc<Mutex<Vec<(u64, Done)>>>,
    /// The work thread writes a byte here when it has finished
    wake_read: OwnedFd,
    wake_write: Arc<OwnedFd>,
    next_id: u64,
    last_use: Instant,
    idle_exit: Duration,
    /// Kept between reads, and wiped after each
    buffer: Vec<u8>,
    /// When the person last said no to each program of each account
    refusals: HashMap<(u32, String), VecDeque<Instant>>,
    /// Where the last round of lines started, so that nobody is always first
    turn: u64,
}

impl Server {
    pub fn new(listener: UnixListener, stop: Option<RawFd>, service: Service, idle_exit: Duration) -> std::io::Result<Server> {
        let mut pipe = [0 as libc::c_int; 2];
        // SAFETY: pipe2 writes two descriptors, which are then owned here
        let (wake_read, wake_write) = unsafe {
            if libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            (OwnedFd::from_raw_fd(pipe[0]), OwnedFd::from_raw_fd(pipe[1]))
        };
        Ok(Server {
            listener,
            ssh: None,
            stop,
            service,
            clients: HashMap::new(),
            waiting: HashMap::new(),
            slow: HashMap::new(),
            queue: VecDeque::new(),
            working: false,
            last_worker: None,
            finished: Arc::new(Mutex::new(Vec::new())),
            wake_read,
            wake_write: Arc::new(wake_write),
            next_id: 1,
            last_use: Instant::now(),
            idle_exit,
            buffer: vec![0; READ_AT_ONCE],
            refusals: HashMap::new(),
            turn: 0,
        })
    }

    /// Serves the SSH agent's socket as well
    pub fn with_ssh(mut self, listener: UnixListener) -> Server {
        self.ssh = Some(listener);
        self
    }

    fn fresh_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn may_exit(&self) -> bool {
        self.service.is_quiet() && self.waiting.is_empty() && self.slow.is_empty() && self.queue.is_empty() && !self.working
    }

    /// When the next thing happens by itself, if anything will: an open vault with nothing to do has this one deadline, and
    /// a closed service the one that ends it
    fn deadline(&self) -> Option<Instant> {
        let mut soonest = self.service.deadline();
        let mut earlier = |when: Instant| soonest = Some(soonest.map_or(when, |now: Instant| now.min(when)));
        for waiting in self.waiting.values() {
            if let Waiting::Approval { until, .. } = waiting {
                earlier(*until);
            }
        }
        for client in self.clients.values() {
            if let Some(deadline) = client.deadline {
                earlier(deadline);
            }
        }
        if self.may_exit() {
            earlier(self.last_use + self.idle_exit);
        }
        soonest
    }

    /// Runs until it should exit. Dropping the service wipes every key still open.
    pub fn run(mut self) {
        loop {
            let mut polls = Vec::with_capacity(self.clients.len() + 5);
            if self.clients.len() < MOST_CLIENTS {
                polls.push(libc::pollfd { fd: self.listener.as_raw_fd(), events: libc::POLLIN, revents: 0 });
            } else {
                polls.push(libc::pollfd { fd: -1, events: 0, revents: 0 });
            }
            let agent = self.ssh.as_ref().filter(|_| self.clients.len() < MOST_CLIENTS).map_or(-1, |listener| listener.as_raw_fd());
            polls.push(libc::pollfd { fd: self.stop.unwrap_or(-1), events: libc::POLLIN, revents: 0 });
            polls.push(libc::pollfd { fd: self.wake_read.as_raw_fd(), events: libc::POLLIN, revents: 0 });
            let outside = self.service.outside.wait_on();
            polls.push(libc::pollfd { fd: outside.map_or(-1, |(fd, _)| fd), events: outside.map_or(0, |(_, events)| events), revents: 0 });
            polls.push(libc::pollfd { fd: agent, events: libc::POLLIN, revents: 0 });
            let mut ids: Vec<u64> = self.clients.keys().copied().collect();
            ids.sort_unstable();
            for id in &ids {
                let client = &self.clients[id];
                let events = if client.written < client.output.len() {
                    libc::POLLOUT
                } else if client.busy || client.closing || client.has_line() {
                    0
                } else {
                    libc::POLLIN
                };
                polls.push(libc::pollfd { fd: client.stream.as_raw_fd(), events, revents: 0 });
            }
            // Lines left over from a burst go on after everyone else has had a turn
            let more = self.service.outside.has_events() || self.clients.values().any(Client::can_take_line);
            let wait = if more {
                0
            } else {
                self.deadline().map_or(-1, |when| {
                    let left = when.saturating_duration_since(Instant::now());
                    // Rounded up, so that a deadline is never woken for a moment early and again
                    left.as_millis().saturating_add(1).min(i32::MAX as u128) as i32
                })
            };
            // SAFETY: polls is a live array of that length
            let ready = unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as libc::nfds_t, wait) };
            if ready < 0 {
                if std::io::Error::last_os_error().kind() == ErrorKind::Interrupted {
                    continue;
                }
                return;
            }
            if polls[1].revents != 0 {
                return;
            }
            if polls[2].revents != 0 {
                self.collect_work();
            }
            if polls[3].revents != 0 || self.service.outside.has_events() {
                let events = self.service.outside.ready();
                self.outside_said(events);
            }
            for (id, poll) in ids.iter().zip(&polls[5..]) {
                if poll.revents != 0 {
                    self.client_ready(*id, poll.revents);
                }
            }
            self.take_turns(&ids);
            if polls[0].revents != 0 {
                self.accept(false);
            }
            if polls[4].revents != 0 {
                self.accept(true);
            }
            let now = Instant::now();
            self.service.tick(now);
            self.start_own();
            self.after_service();
            self.expire(now);
            if self.may_exit() && now.duration_since(self.last_use) >= self.idle_exit {
                return;
            }
        }
    }

    fn accept(&mut self, agent: bool) {
        let listener = if agent { self.ssh.as_ref() } else { Some(&self.listener) };
        let Some(Ok((stream, _))) = listener.map(UnixListener::accept) else { return };
        self.last_use = Instant::now();
        let Some(peer) = Peer::of(stream.as_raw_fd()) else { return };
        // An account nobody is logged in as has nobody to approve anything, and its connections would be anyone's to hold:
        // unless it lingers, for the one request such an account may make (`Service::request`)
        // (the SSH agent is for a session: a lingering account has nothing open to sign with)
        if !(if agent { self.service.has_session(peer.uid) } else { self.service.serves(peer.uid) }) {
            return;
        }
        if self.clients.values().filter(|client| client.peer.uid == peer.uid).count() >= CLIENTS_PER_ACCOUNT {
            return;
        }
        if stream.set_nonblocking(true).is_err() {
            return;
        }
        let id = self.fresh_id();
        let deadline = Some(Instant::now() + IDLE_CONNECTION);
        self.clients.insert(id, Client { stream, peer: Rc::new(peer), input: Vec::new(), output: Vec::new(), written: 0, ssh: agent, bound: ssh::Conn::default(), busy: false, closing: false, deadline });
    }

    fn client_ready(&mut self, id: u64, events: i16) {
        if events & libc::POLLOUT != 0 {
            self.write(id);
        }
        if events & libc::POLLIN != 0 {
            self.read(id);
        } else if events & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
            self.gone(id);
        }
    }

    /// Reads what is there into a buffer that never holds more than one line and its newline: nothing past the limit is ever
    /// allocated, whatever a client sends
    fn read(&mut self, id: u64) {
        let Some(client) = self.clients.get_mut(&id) else { return };
        let limit = if client.ssh { ssh::MOST + 4 } else { MAX_LINE + 1 };
        let room = (limit - client.input.len().min(limit)).min(self.buffer.len());
        let count = match client.stream.read(&mut self.buffer[..room]) {
            Ok(count) => count,
            Err(error) if error.kind() == ErrorKind::WouldBlock || error.kind() == ErrorKind::Interrupted => return,
            Err(_) => return self.gone(id),
        };
        if count == 0 {
            return self.gone(id);
        }
        let wanted = client.input.len() + count;
        if wanted > client.input.capacity() {
            let grown = wanted.max(client.input.capacity() * 2).min(limit);
            client.input.reserve_exact(grown - client.input.len());
        }
        client.input.extend_from_slice(&self.buffer[..count]);
        self.buffer[..count].zeroize();
        self.last_use = Instant::now();
        if client.input.len() > MAX_LINE && !client.has_line() {
            client.input.zeroize();
            client.input = Vec::new();
            client.closing = true;
            self.answer(id, &protocol::refusal(Refusal::TooLarge));
        }
    }

    /// One round of lines: each account a share, starting from a different client each time
    fn take_turns(&mut self, ids: &[u64]) {
        if ids.is_empty() {
            return;
        }
        self.turn = self.turn.wrapping_add(1);
        let start = (self.turn as usize) % ids.len();
        let mut spent: HashMap<u32, usize> = HashMap::new();
        for id in ids[start..].iter().chain(&ids[..start]) {
            let Some(uid) = self.clients.get(id).map(|client| client.peer.uid) else { continue };
            for _ in 0..LINES_AT_ONCE {
                if spent.get(&uid).copied().unwrap_or(0) >= BYTES_A_ROUND {
                    break;
                }
                match self.next_line(*id) {
                    Some(length) => *spent.entry(uid).or_default() += length.max(1),
                    None => break,
                }
            }
        }
    }

    /// Handles the next line, if a whole one has arrived and the last one has been answered: its length, or None
    fn next_line(&mut self, id: u64) -> Option<usize> {
        let client = self.clients.get_mut(&id)?;
        if client.busy || client.closing || client.written < client.output.len() {
            return None;
        }
        if client.ssh {
            return self.next_frame(id);
        }
        let end = client.input.iter().position(|byte| *byte == b'\n')?;
        let peer = client.peer.clone();
        let line: Vec<u8> = client.input.drain(..=end).collect();
        // What the line was must not stay behind in the buffer's spare room, a password among it
        let left = client.input.len();
        let room = client.input.capacity();
        client.input.resize(room, 0);
        client.input.truncate(left);
        // A large line's room goes back at once, rather than staying with a client that may sit on it
        if client.input.is_empty() && client.input.capacity() > READ_AT_ONCE {
            client.input = Vec::new();
        }
        let next = self.service.request(&peer, &line[..end]);
        drop(line);
        self.drive(id, next);
        self.after_service();
        Some(end + 1)
    }

    /// `next_line` for the SSH agent's framing: how much it took, if a whole frame had come
    fn next_frame(&mut self, id: u64) -> Option<usize> {
        let client = self.clients.get_mut(&id)?;
        match ssh::take(&mut client.input) {
            ssh::Frame::Partial => None,
            ssh::Frame::TooLarge => {
                client.input = Vec::new();
                client.closing = true;
                self.answer(id, &protocol::refusal(Refusal::TooLarge));
                None
            }
            ssh::Frame::Whole(frame) => {
                let peer = client.peer.clone();
                let length = frame.len() + 4;
                let next = ssh::request(&mut self.service, &peer, &mut client.bound, &frame);
                drop(frame);
                self.drive(id, next);
                self.after_service();
                Some(length)
            }
        }
    }

    /// Carries a request on as far as it goes without waiting
    fn drive(&mut self, id: u64, next: Next) {
        let Some(client) = self.clients.get_mut(&id) else { return };
        let uid = client.peer.uid;
        match next {
            Next::Answer(answer) => {
                client.busy = false;
                self.answer(id, &answer);
            }
            Next::Approve { ask, then } => {
                let peer = client.peer.clone();
                let program = peer.program();
                let key = (uid, program);
                if self.refused_lately(&key) {
                    return self.answer(id, &protocol::refusal(Refusal::Refused));
                }
                self.wait_on_person(id, uid, then, Some(key));
                let token = self.next_id;
                self.service.outside.approve(token, &ask, &peer);
            }
            Next::WhenUnlocked { then } => {
                let peer = client.peer.clone();
                self.wait_on_person(id, uid, then, None);
                let token = self.next_id;
                self.service.outside.when_unlocked(token, &peer);
            }
            Next::Work { job, then } => {
                let queued = self.queue.iter().filter(|queued| queued.uid == uid).count();
                if queued >= JOBS_PER_ACCOUNT || self.queue.len() >= MOST_JOBS {
                    return self.answer(id, &protocol::refusal(Refusal::Failed));
                }
                client.busy = true;
                client.deadline = None;
                let token = self.fresh_id();
                let closes = self.service.closes_so_far(uid);
                self.waiting.insert(token, Waiting::Work { client: id, uid, then, closes });
                self.queue.push_back(Queued { token, uid, job });
                self.start_work();
            }
            Next::Slow { job, then } => {
                client.busy = true;
                client.deadline = None;
                let token = self.fresh_id();
                self.slow.insert(token, Slow::Client { client: id, then });
                self.start_slow(token, job);
            }
        }
    }

    /// A job that may take minutes, on a thread of its own: it never waits behind the work thread's queue, and the work thread
    /// never waits behind it. `sync.rs` lets an account have one at a time.
    fn start_slow(&mut self, token: u64, job: Box<dyn FnOnce() -> Done + Send>) {
        let (finished, wake) = (self.finished.clone(), self.wake_write.clone());
        let spawned = std::thread::Builder::new().stack_size(1024 * 1024).spawn(move || {
            let done = job();
            if let Ok(mut finished) = finished.lock() {
                finished.push((token, done));
            }
            // SAFETY: one byte from a live buffer, to a pipe this thread holds a reference to
            unsafe { libc::write(wake.as_raw_fd(), [1u8].as_ptr().cast(), 1) };
        });
        if spawned.is_err() {
            self.slow_done(token, None);
        }
    }

    /// A slow job has finished, or could not start (`None`): whoever waited hears, and a run sync started itself is told
    fn slow_done(&mut self, token: u64, done: Option<Done>) {
        match (self.slow.remove(&token), done) {
            (Some(Slow::Client { client, then }), Some(done)) => {
                if let Some(peer) = self.clients.get(&client).map(|client| client.peer.clone()) {
                    let next = then(&mut self.service, &peer, done);
                    self.drive(client, next);
                }
            }
            (Some(Slow::Client { client, .. }), None) => self.drive(client, Next::refuse(Refusal::Failed)),
            (Some(Slow::Own { then }), Some(done)) => then(&mut self.service, done),
            (Some(Slow::Own { .. }), None) | (None, _) => {}
        }
    }

    /// The runs sync started by itself (a timer, an opening, a change): each on a thread of its own
    fn start_own(&mut self) {
        for job in self.service.sync.take_jobs() {
            let token = self.fresh_id();
            self.slow.insert(token, Slow::Own { then: job.then });
            self.start_slow(token, job.job);
        }
    }

    /// Marks the client as waiting on a person, under a token that `self.next_id` then holds
    fn wait_on_person(&mut self, id: u64, uid: u32, then: Then<bool>, asker: Option<(u32, String)>) {
        if let Some(client) = self.clients.get_mut(&id) {
            client.busy = true;
            client.deadline = None;
        }
        let token = self.fresh_id();
        let closes = self.service.closes_so_far(uid);
        self.waiting.insert(token, Waiting::Approval { client: id, uid, then, until: Instant::now() + APPROVAL_PATIENCE, closes, asker });
    }

    fn refused_lately(&mut self, key: &(u32, String)) -> bool {
        let now = Instant::now();
        let Some(times) = self.refusals.get_mut(key) else { return false };
        while times.front().is_some_and(|when| now.duration_since(*when) >= REFUSAL_MEMORY) {
            times.pop_front();
        }
        if times.is_empty() {
            self.refusals.remove(key);
            return false;
        }
        times.len() >= REFUSALS
    }

    fn note_refusal(&mut self, key: (u32, String)) {
        // Bounded: a program can call itself anything, but each name costs the person a no
        if self.refusals.len() > 1024 {
            self.refusals.clear();
        }
        let times = self.refusals.entry(key).or_default();
        times.push_back(Instant::now());
        while times.len() > REFUSALS {
            times.pop_front();
        }
    }

    /// The next job, on a thread of its own, so that everything it allocated goes when it ends. Somebody else's first, if
    /// anybody else has some waiting.
    fn start_work(&mut self) {
        if self.working {
            return;
        }
        let at = self.queue.iter().position(|queued| Some(queued.uid) != self.last_worker).unwrap_or(0);
        let Some(queued) = self.queue.remove(at) else { return };
        self.last_worker = Some(queued.uid);
        let token = queued.token;
        let (finished, wake) = (self.finished.clone(), self.wake_write.clone());
        let spawned = std::thread::Builder::new().stack_size(512 * 1024).spawn(move || {
            let done = (queued.job)();
            if let Ok(mut finished) = finished.lock() {
                finished.push((queued.token, done));
            }
            // SAFETY: one byte from a live buffer, to a pipe this thread holds a reference to
            unsafe { libc::write(wake.as_raw_fd(), [1u8].as_ptr().cast(), 1) };
        });
        match spawned {
            Ok(_) => self.working = true,
            // No thread to be had: whoever asked hears so
            Err(_) => {
                if let Some(Waiting::Work { client, .. }) = self.waiting.remove(&token) {
                    self.drive(client, Next::refuse(Refusal::Failed));
                }
            }
        }
    }

    fn collect_work(&mut self) {
        let mut drained = [0u8; 16];
        // SAFETY: into a live buffer, from a non-blocking pipe
        while unsafe { libc::read(self.wake_read.as_raw_fd(), drained.as_mut_ptr().cast(), drained.len()) } > 0 {}
        let finished = self.finished.lock().map(|mut finished| std::mem::take(&mut *finished)).unwrap_or_default();
        for (token, done) in finished {
            if self.slow.contains_key(&token) {
                self.slow_done(token, Some(done));
                continue;
            }
            self.working = false;
            let Some(Waiting::Work { client, uid, then, closes }) = self.waiting.remove(&token) else { continue };
            let Some(peer) = self.clients.get(&client).map(|client| client.peer.clone()) else { continue };
            // Closed while it worked: what it made, a key among it, goes with it
            let next = if self.service.closes_so_far(uid) != closes { Next::refuse(Refusal::Closed) } else { then(&mut self.service, &peer, done) };
            self.drive(client, next);
            self.after_service();
        }
        self.start_work();
    }

    fn outside_said(&mut self, events: Vec<Event>) {
        for event in events {
            match event {
                Event::Approved(token, yes) => {
                    let Some(Waiting::Approval { client, uid, then, closes, asker, .. }) = self.waiting.remove(&token) else { continue };
                    if let (false, Some(key)) = (yes, asker) {
                        self.note_refusal(key);
                    }
                    let Some(peer) = self.clients.get(&client).map(|client| client.peer.clone()) else { continue };
                    // Locked, slept or closed while the person was asked: the approval was for a vault that is not there now
                    let next = if self.service.closes_so_far(uid) != closes { Next::refuse(Refusal::Closed) } else { then(&mut self.service, &peer, yes) };
                    self.drive(client, next);
                }
                Event::NotAsked(token) => {
                    if let Some(Waiting::Approval { client, .. }) = self.waiting.remove(&token) {
                        self.drive(client, Next::refuse(Refusal::Closed));
                    }
                }
                event => self.service.event(&event),
            }
            self.after_service();
        }
    }

    /// Whatever the service closed takes back what was being asked of that account: a prompt left up after a lock would sit over
    /// the lock screen, its item's name in it, for anyone at the machine, and an approval given then would be for a vault that
    /// is closed
    fn after_service(&mut self) {
        let closed = self.service.take_closed();
        if closed.is_empty() {
            return;
        }
        let goes = |uid: u32| closed.iter().any(|which| which.is_none_or(|closed| closed == uid));
        let tokens: Vec<(u64, u64)> = self
            .waiting
            .iter()
            .filter_map(|(token, waiting)| match waiting {
                Waiting::Approval { client, uid, .. } if goes(*uid) => Some((*token, *client)),
                _ => None,
            })
            .collect();
        for (token, client) in tokens {
            self.waiting.remove(&token);
            self.service.outside.cancel(token);
            self.drive(client, Next::refuse(Refusal::Closed));
        }
    }

    fn answer(&mut self, id: u64, answer: &Value) {
        let Some(client) = self.clients.get_mut(&id) else { return };
        client.busy = false;
        let line = if client.ssh { ssh::reply(answer) } else { protocol::line(answer) };
        if !client.ssh && line.len() > MAX_LINE {
            client.output = protocol::line(&protocol::refusal(Refusal::TooLarge));
        } else {
            client.output = line;
        }
        client.written = 0;
        self.write(id);
    }

    fn write(&mut self, id: u64) {
        let Some(client) = self.clients.get_mut(&id) else { return };
        while client.written < client.output.len() {
            match client.stream.write(&client.output[client.written..]) {
                Ok(0) => return self.gone(id),
                Ok(count) => client.written += count,
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    client.deadline = Some(Instant::now() + STALLED);
                    return;
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(_) => return self.gone(id),
            }
        }
        let large = client.output.len() > LARGE_ANSWER;
        client.output.zeroize();
        client.output = Vec::new();
        client.written = 0;
        client.deadline = Some(Instant::now() + IDLE_CONNECTION);
        if large {
            // What a large answer was made from is free now, and handed back, or the service stays that size
            // SAFETY: malloc_trim only releases free memory, and is safe to call at any time
            unsafe { libc::malloc_trim(0) };
        }
        if client.closing {
            self.gone(id);
        }
    }

    /// Whoever asked has gone, and takes what they were waiting for with them
    fn gone(&mut self, id: u64) {
        if let Some(mut client) = self.clients.remove(&id) {
            client.input.zeroize();
            client.output.zeroize();
        }
        let mine: Vec<u64> = self
            .waiting
            .iter()
            .filter(|(_, waiting)| match waiting {
                Waiting::Approval { client, .. } | Waiting::Work { client, .. } => *client == id,
            })
            .map(|(token, _)| *token)
            .collect();
        // A slow job already running finishes, and what it made goes with nobody to take it
        let abandoned: Vec<u64> = self.slow.iter().filter(|(_, slow)| matches!(slow, Slow::Client { client, .. } if *client == id)).map(|(token, _)| *token).collect();
        for token in abandoned {
            self.slow.insert(token, Slow::Own { then: Box::new(|_, _| {}) });
        }
        for token in mine {
            if let Some(Waiting::Approval { .. }) = self.waiting.get(&token) {
                self.waiting.remove(&token);
                self.service.outside.cancel(token);
            } else if self.queue.iter().any(|queued| queued.token == token) {
                self.queue.retain(|queued| queued.token != token);
                self.waiting.remove(&token);
            }
            // A job already running finishes, and its result goes with nobody to take it
        }
    }

    fn expire(&mut self, now: Instant) {
        let late: Vec<u64> = self.clients.iter().filter(|(_, client)| client.deadline.is_some_and(|deadline| deadline <= now)).map(|(id, _)| *id).collect();
        for id in late {
            self.gone(id);
        }
        let unanswered: Vec<u64> = self
            .waiting
            .iter()
            .filter(|(_, waiting)| matches!(waiting, Waiting::Approval { until, .. } if *until <= now))
            .map(|(token, _)| *token)
            .collect();
        for token in unanswered {
            self.service.outside.cancel(token);
            self.outside_said(vec![Event::Approved(token, false)]);
        }
    }
}
