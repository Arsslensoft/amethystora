//! The daemon: load the BPF object, register every watched folder by its kernel inode, then follow the ring
//! buffer and write each event out as audit records. Registration learns a folder's inode by opening it,
//! which the file_open program sees (it is the sensor's own process) and leaves in .bss for the daemon to
//! read; the daemon keeps the descriptor so the inode cannot be reused while it is watched.

use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use libbpf_rs::{Map, MapCore, MapFlags, RingBufferBuilder};

use crate::event::{self, Event};
use crate::plan::{self, Kind};
use crate::record::Record;
use crate::refused::{refused_path, RefusedMatcher};
use crate::{bpf, keys, record};

const LOG_DIR: &str = "/var/log/amethystora-sensor";
/// The log is rotated past this size; the watcher reads only what is new since its checkpoint.
const LOG_MAX: u64 = 32 * 1024 * 1024;
/// A watched folder's inode (its kernel pointer) is held open, so the map can only hold so many.
const MAX_FOLDERS: u32 = 60000;
/// What one watched tree may register when the sensor starts, and what one account's own mkdirs inside the
/// watched trees may register while it runs: a folder is held open and never freed, so without a bound one
/// account's thousands of empty folders would use the whole table and blind every folder made after them.
const BOOT_TREE_FOLDERS: u32 = 4096;
const ACCOUNT_FOLDERS: u32 = 4096;
/// Where the sensor says a table that could not take a folder, one line each, for the watcher to report.
const FULL_FILE: &str = "full";
/// How deep a watched tree is walked when registering it.
const MAX_DEPTH: u32 = 64;
/// SDIR_HAS_FILES in sensor.h: the folder holds single watched files, matched by name.
const SDIR_HAS_FILES: u32 = 1;
/// The longest the logins wait for the sensor to be watching: a machine slow enough to take longer lets them
/// in anyway, while the sensor goes on loading.
const LOGIN_WAIT: Duration = Duration::from_secs(30);

/// Runs the sensor, or just probes it (load and attach every program, then stop) for the generator.
pub fn run(probe: bool) -> Result<(), Box<dyn Error>> {
    if probe {
        // Every key, so every program is loaded and attached; dropping the result detaches them.
        let all = (1u32 << keys::KEYS.len()) - 1;
        bpf::load(all, true)?;
        return Ok(());
    }

    let sensor_keys = plan::sensor_keys();
    if sensor_keys == 0 {
        // Nothing for the sensor to do; the plan gave every key to auditd. The ExecCondition stops this
        // from running at all, so this is only a safeguard.
        return Ok(());
    }

    // The readiness marker: absent while the BPF object loads (the verifier, about a second) and the watched
    // folders register, written once every folder and file is in the maps. Until then the programs may be
    // attached but the maps empty, so a read is not yet matched; a reader that must know the sensor is fully
    // watching (the VM test, and anything after it) waits for this. Cleared first in case a previous instance
    // left it after a restart within the same boot.
    let ready = plan::run_dir().join("ready");
    let _ = fs::remove_file(&ready);
    let _ = fs::remove_file(plan::run_dir().join(FULL_FILE));
    // systemd holds the logins until the sensor says it is watching (Type=notify, before
    // systemd-user-sessions.service), so that nothing an account does goes unrecorded; never past LOGIN_WAIT.
    std::thread::spawn(|| {
        std::thread::sleep(LOGIN_WAIT);
        notify_ready();
    });

    let loaded = bpf::load(sensor_keys, false)?;
    let dirs = map_named(&loaded.object, "dirs")?;
    let files = map_named(&loaded.object, "files")?;
    let bss = map_named(&loaded.object, "bss")?;
    let events = map_named(&loaded.object, "events")?;
    let dropped = map_named(&loaded.object, "dropped")?;

    let mut reg = Registry::new();
    let mut refused = RefusedMatcher::default();
    for watch in plan::watches(sensor_keys) {
        reg.add_watch(&watch, &dirs, &files, &bss);
        refused.add(&watch);
    }
    // Every watch is now in the maps: the sensor is fully watching.
    let _ = fs::write(&ready, b"ready\n");
    notify_ready();

    let mut log = Log::open()?;
    let offset = boot_offset_ns();

    // The callback copies each record out; the loop drains and writes them, so it is free to touch the
    // other maps (dynamic registration) without the ring buffer holding them.
    let queue: Rc<RefCell<Vec<Vec<u8>>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = queue.clone();
    let mut builder = RingBufferBuilder::new();
    builder.add(&events, move |data: &[u8]| {
        sink.borrow_mut().push(data.to_vec());
        0
    })?;
    let ring = builder.build()?;

    // A copy of each event goes to the journal as well (authpriv), so the Logs app's Security view keeps
    // showing the file events it used to read from auditd (PLAN-audit-cost.md Q5). Best-effort: a failed
    // connection or send never stops the sensor.
    let journal = std::os::unix::net::UnixDatagram::unbound().ok();
    if let Some(j) = &journal {
        let _ = j.connect("/dev/log");
    }

    let mut serial = (std::process::id() as u64) << 16;
    let mut drops_said = 0u64;
    loop {
        ring.poll(Duration::from_millis(500))?;
        let batch: Vec<Vec<u8>> = queue.borrow_mut().drain(..).collect();
        // Events the BPF could not write (the ring buffer full in a burst), counted per CPU: the watcher reports
        // them as it does audit's lost events. Looked at only after a batch, since a burst is what fills it.
        if !batch.is_empty() {
            let drops = count_drops(&dropped);
            if drops != drops_said {
                let _ = fs::write(plan::run_dir().join("dropped"), format!("{drops}\n"));
                drops_said = drops;
            }
        }
        for raw in batch {
            if let Some(ev) = Event::parse(&raw) {
                serial = serial.wrapping_add(1);
                let mut rec = reg.record_of(&ev, offset);
                // A refused open (open_refused in the BPF) carries every enabled read or write key and the
                // raw path, because no inode is matched in the kernel; keep only the keys whose watches
                // actually cover the path, and drop it when none do. Without this, every EACCES/EPERM open
                // an account makes — reading /proc, probing a path it may not read — would be recorded under
                // credentials and the decoy, as if a secret had been read.
                // A refused exec (execve_exit) likewise carries both exec keys and the name it was given.
                if rec.flags & event::SOPEN_REFUSED != 0 && (rec.op == event::SOP_OPEN || rec.op == event::SOP_EXEC) {
                    let path = refused_path(&rec.path2, &rec.cwd);
                    rec.keys &= if rec.op == event::SOP_EXEC {
                        refused.exec_keys(&path)
                    } else {
                        refused.keys(&path, rec.flags & event::SOPEN_READ == 0)
                    };
                    if rec.keys == 0 {
                        continue;
                    }
                }
                if let Err(e) = log.write(&rec, serial) {
                    eprintln!("amethystora-sensor: writing a record: {e}");
                }
                if let Some(j) = &journal {
                    let _ = j.send(&journal_line(&rec));
                }
                reg.register_new_dir(&ev, &rec, &dirs, &bss);
            }
        }
    }
}

/// A short, readable copy of an event for the journal, at authpriv.notice, which the Logs app's Security
/// view reads (SYSLOG_FACILITY=10). The watcher reads the full audit record from the sensor's own log; this
/// is only so the raw journal view is not left empty where the sensor stands in for the audit rules.
fn journal_line(rec: &Record) -> Vec<u8> {
    let key = crate::keys::names_of(rec.keys).first().copied().unwrap_or("sensor");
    let object = if !rec.dir.is_empty() {
        let mut v = rec.dir.clone();
        v.push(b'/');
        v.extend_from_slice(&rec.name);
        v
    } else if !rec.path2.is_empty() {
        rec.path2.clone()
    } else if !rec.module.is_empty() {
        rec.module.clone()
    } else {
        rec.name.clone()
    };
    // authpriv (10) * 8 + notice (5) = 85
    let mut line = format!("<85>amethystora-sensor[{}]: {key}: ", std::process::id()).into_bytes();
    // A path or name is untrusted text; keep only printable bytes here (the full record keeps the kernel's
    // hex form). The watcher's own text filter handles what reaches a person.
    for &b in &object {
        line.push(if (0x20..0x7f).contains(&b) { b } else { b'?' });
    }
    line.extend_from_slice(
        format!(" (comm={} uid={} auid={})", String::from_utf8_lossy(&rec.comm), rec.uid, rec.auid).as_bytes(),
    );
    line
}

/// The events the BPF dropped since it loaded, summed over the CPUs (the `dropped` per-CPU array, slot 0).
fn count_drops(dropped: &Map) -> u64 {
    match dropped.lookup_percpu(&0u32.to_ne_bytes(), MapFlags::ANY) {
        Ok(Some(cpus)) => cpus
            .iter()
            .filter_map(|v| v.get(..8).and_then(|b| b.try_into().ok()).map(u64::from_ne_bytes))
            .sum(),
        _ => 0,
    }
}

/// Tells systemd the sensor is watching (sd_notify's READY=1), which the logins wait for. A second READY is
/// ignored, and without $NOTIFY_SOCKET (run by hand) there is nobody to tell.
fn notify_ready() {
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixDatagram};

    let Some(socket) = std::env::var_os("NOTIFY_SOCKET") else { return };
    let addr = match socket.as_bytes() {
        [b'@', name @ ..] => SocketAddr::from_abstract_name(name),
        [b'/', ..] => SocketAddr::from_pathname(&socket),
        _ => return,
    };
    if let (Ok(addr), Ok(sock)) = (addr, UnixDatagram::unbound()) {
        let _ = sock.send_to_addr(b"READY=1", &addr);
    }
}

// --- Registration ------------------------------------------------------------------------------------------

struct FolderRec {
    path: Vec<u8>,
    write_keys: u32,
    read_keys: u32,
    exec_keys: u32,
    flags: u32,
}

struct Registry {
    by_inode: HashMap<u64, u32>,
    folders: HashMap<u32, FolderRec>,
    fds: Vec<OwnedFd>,
    next_id: u32,
    /// How many more new folders the registration under way may take, and for whom (the account whose mkdir it
    /// answers; none at start-up).
    budget: u32,
    owner: Option<u32>,
    /// New folders registered for each account while running.
    per_account: HashMap<u32, u32>,
    /// What could not be registered, said in `full` (a line each: `table`, `tree`, `uid N`).
    full: std::collections::BTreeSet<String>,
}

impl Registry {
    fn new() -> Self {
        Registry {
            by_inode: HashMap::new(),
            folders: HashMap::new(),
            fds: Vec::new(),
            next_id: 1,
            budget: BOOT_TREE_FOLDERS,
            owner: None,
            per_account: HashMap::new(),
            full: Default::default(),
        }
    }

    /// A folder that could not be registered is not watched, and nothing in it would be recorded: say so once,
    /// in the run directory, where the watcher reads it.
    fn note_full(&mut self, why: String) {
        if self.full.insert(why) {
            let text: String = self.full.iter().map(|l| format!("{l}\n")).collect();
            let _ = fs::write(plan::run_dir().join(FULL_FILE), text);
        }
    }

    fn add_watch(&mut self, w: &plan::Watch, dirs: &Map, files: &Map, bss: &Map) {
        self.budget = BOOT_TREE_FOLDERS;
        self.owner = None;
        let path = w.path.as_os_str().as_bytes().to_vec();
        let bit = 1u32 << w.key_bit;
        match w.kind {
            Kind::Tree => self.register_tree(&path, bit, 0, 0, 0, MAX_DEPTH, dirs, bss),
            Kind::ReadTree => self.register_tree(&path, 0, bit, 0, 0, MAX_DEPTH, dirs, bss),
            Kind::ExecTree => {
                self.register_folder(&path, 0, 0, bit, 0, dirs, bss);
            }
            Kind::File | Kind::ReadFile => self.register_file(&w.path, w.kind, bit, dirs, files, bss),
        }
    }

    /// A single watched file: its folder registered so it holds files, and an entry under the folder's inode
    /// and the file's name.
    fn register_file(&mut self, path: &std::path::Path, kind: Kind, bit: u32, dirs: &Map, files: &Map, bss: &Map) {
        let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
            return;
        };
        let name = name.as_bytes();
        if name.is_empty() || name.len() >= 64 {
            return;
        }
        let parent = parent.as_os_str().as_bytes().to_vec();
        let Some((_, inode)) = self.register_folder(&parent, 0, 0, 0, SDIR_HAS_FILES, dirs, bss) else {
            return;
        };
        // file_key: parent inode then the name, zero-padded to 64.
        let mut key = Vec::with_capacity(72);
        key.extend_from_slice(&inode.to_ne_bytes());
        let mut padded = [0u8; 64];
        padded[..name.len()].copy_from_slice(name);
        key.extend_from_slice(&padded);
        // file_info: id (the folder's is used for the event, so 0 here), write_keys, read_keys.
        let (write_keys, read_keys) = if kind == Kind::ReadFile { (0, bit) } else { (bit, 0) };
        // Merge with any entry already there.
        let (mut w, mut r) = (write_keys, read_keys);
        if let Ok(Some(old)) = files.lookup(&key, MapFlags::ANY) {
            if old.len() >= 12 {
                w |= u32::from_ne_bytes(old[4..8].try_into().unwrap());
                r |= u32::from_ne_bytes(old[8..12].try_into().unwrap());
            }
        }
        let mut val = Vec::with_capacity(12);
        val.extend_from_slice(&0u32.to_ne_bytes());
        val.extend_from_slice(&w.to_ne_bytes());
        val.extend_from_slice(&r.to_ne_bytes());
        let _ = files.update(&key, &val, MapFlags::ANY);
    }

    /// A watched folder and every folder below it (writes or reads), to MAX_DEPTH.
    #[allow(clippy::too_many_arguments)]
    fn register_tree(&mut self, path: &[u8], w: u32, r: u32, e: u32, flags: u32, depth: u32, dirs: &Map, bss: &Map) {
        if self.register_folder(path, w, r, e, flags, dirs, bss).is_none() || depth == 0 {
            return;
        }
        // Its subfolders, not through a link. read_dir opens the folder (a self open), which is why the
        // folder's own inode was captured in register_folder first.
        let os = std::ffi::OsStr::from_bytes(path);
        let Ok(entries) = fs::read_dir(os) else {
            return;
        };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(ft) if ft.is_dir() && !ft.is_symlink() => {
                    let child = entry.path().as_os_str().as_bytes().to_vec();
                    self.register_tree(&child, w, r, e, flags, depth - 1, dirs, bss);
                }
                _ => {}
            }
        }
    }

    /// Registers one folder: open it (which the BPF sees and leaves its inode in .bss), then hold the
    /// descriptor and record the folder under that inode in the map. Returns the folder's id and inode.
    fn register_folder(&mut self, path: &[u8], w: u32, r: u32, e: u32, flags: u32, dirs: &Map, bss: &Map) -> Option<(u32, u64)> {
        let (fd, inode) = open_folder(path, bss)?;

        let id = if let Some(&id) = self.by_inode.get(&inode) {
            let rec = self.folders.get_mut(&id).unwrap();
            rec.write_keys |= w;
            rec.read_keys |= r;
            rec.exec_keys |= e;
            rec.flags |= flags;
            id
        } else {
            if self.next_id >= MAX_FOLDERS {
                self.note_full("table".to_string());
                return None;
            }
            if self.budget == 0 {
                let why = self.owner.map_or("tree".to_string(), |uid| format!("uid {uid}"));
                self.note_full(why);
                return None;
            }
            self.budget -= 1;
            let id = self.next_id;
            self.next_id += 1;
            self.by_inode.insert(inode, id);
            self.folders.insert(id, FolderRec { path: path.to_vec(), write_keys: w, read_keys: r, exec_keys: e, flags });
            self.fds.push(fd);
            id
        };
        let rec = &self.folders[&id];
        let mut val = Vec::with_capacity(20);
        val.extend_from_slice(&rec.id_bytes(id));
        let _ = dirs.update(&inode.to_ne_bytes(), &val, MapFlags::ANY);
        Some((id, inode))
    }

    /// A mkdir, or a rename, that made a new folder inside a watched tree: register it (and rescan it) so
    /// what appears in it afterwards is watched too.
    fn register_new_dir(&mut self, ev: &Event, rec: &Record, dirs: &Map, bss: &Map) {
        if ev.head.result != 0 {
            return;
        }
        let (parent_id, is_dir) = match ev.head.op {
            event::SOP_MKDIR => (ev.head.dir, true),
            event::SOP_RENAME => (ev.head.dir2, rec.obj_mode & 0o170000 == 0o040000),
            _ => return,
        };
        if !is_dir || parent_id == 0 {
            return;
        }
        let Some(parent) = self.folders.get(&parent_id) else {
            return;
        };
        let (w, r, e) = (parent.write_keys, parent.read_keys, parent.exec_keys);
        if w == 0 && r == 0 {
            return;
        }
        let name = if ev.head.op == event::SOP_MKDIR { &rec.name } else { &rec.name2 };
        let mut child = parent.path.clone();
        child.push(b'/');
        child.extend_from_slice(name);
        // What one account's folders may take of the table: root's own are not an account's to fill, so only
        // the table itself bounds them
        let uid = ev.head.uid;
        let used = self.per_account.get(&uid).copied().unwrap_or(0);
        self.budget = if uid == 0 { MAX_FOLDERS } else { ACCOUNT_FOLDERS.saturating_sub(used) };
        self.owner = Some(uid);
        let before = self.budget;
        self.register_tree(&child, w, r, e, 0, MAX_DEPTH, dirs, bss);
        if uid != 0 {
            *self.per_account.entry(uid).or_insert(0) += before - self.budget;
        }
        self.owner = None;
    }

    /// The folder a numbered id names, empty when none.
    fn path_of(&self, id: u32) -> Vec<u8> {
        if id == 0 {
            Vec::new()
        } else {
            self.folders.get(&id).map(|f| f.path.clone()).unwrap_or_default()
        }
    }

    /// Turns a ring-buffer event into the record the writer takes, resolving folder ids to their paths.
    fn record_of(&self, ev: &Event, offset: i128) -> Record {
        let h = &ev.head;
        let is_module = h.op == event::SOP_MODULE_LOAD || h.op == event::SOP_MODULE_DELETE;
        let is_memory = h.op == event::SOP_MEMORY;
        let argv: Vec<Vec<u8>> =
            ev.strings[event::SSTR_ARGV].split(|&b| b == 0).filter(|s| !s.is_empty()).map(|s| s.to_vec()).collect();
        let wall = (h.time as i128 + offset).max(0) as u128;
        Record {
            op: h.op,
            syscall: h.syscall,
            result: h.result as i64,
            keys: h.keys,
            args: h.args,
            pid: h.pid,
            ppid: h.ppid,
            auid: h.auid,
            ses: h.ses,
            uid: h.uid,
            gid: h.gid,
            euid: h.euid,
            suid: h.suid,
            fsuid: h.fsuid,
            egid: h.egid,
            sgid: h.sgid,
            fsgid: h.fsgid,
            comm: cstr(&h.comm),
            tty: cstr(&h.tty),
            exe: ev.strings[event::SSTR_EXE].to_vec(),
            cwd: ev.strings[event::SSTR_CWD].to_vec(),
            flags: h.flags,
            obj_mode: h.mode,
            obj_ino: h.ino,
            obj_ouid: h.ouid,
            obj_ogid: h.ogid,
            obj_dev: dev_str(h.dev),
            obj_rdev: dev_str(h.rdev),
            parent_mode: h.parent_mode,
            dir: self.path_of(h.dir),
            name: if is_module || is_memory { Vec::new() } else { ev.strings[event::SSTR_NAME].to_vec() },
            dir2: self.path_of(h.dir2),
            name2: ev.strings[event::SSTR_NAME2].to_vec(),
            path2: ev.strings[event::SSTR_PATH2].to_vec(),
            argc: h.argc,
            argv,
            module: if is_module { ev.strings[event::SSTR_NAME].to_vec() } else { Vec::new() },
            target: h.target,
            target_auid: h.target_auid,
            target_uid: h.target_uid,
            target_comm: if is_memory { ev.strings[event::SSTR_NAME].to_vec() } else { Vec::new() },
            target_mnt: h.target_mnt,
            own_mnt: h.own_mnt,
            time_ns: wall,
        }
    }
}

impl FolderRec {
    /// dir_info: id, write_keys, read_keys, exec_keys, flags.
    fn id_bytes(&self, id: u32) -> [u8; 20] {
        let mut b = [0u8; 20];
        b[0..4].copy_from_slice(&id.to_ne_bytes());
        b[4..8].copy_from_slice(&self.write_keys.to_ne_bytes());
        b[8..12].copy_from_slice(&self.read_keys.to_ne_bytes());
        b[12..16].copy_from_slice(&self.exec_keys.to_ne_bytes());
        b[16..20].copy_from_slice(&self.flags.to_ne_bytes());
        b
    }
}

// --- The log ------------------------------------------------------------------------------------------------

struct Log {
    file: File,
    written: u64,
}

impl Log {
    fn open() -> std::io::Result<Log> {
        fs::create_dir_all(LOG_DIR)?;
        let path = PathBuf::from(LOG_DIR).join("audit.log");
        let written = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let file = OpenOptions::new().create(true).append(true).mode(0o600).open(&path)?;
        Ok(Log { file, written })
    }

    fn write(&mut self, rec: &Record, serial: u64) -> std::io::Result<()> {
        let mut buf = Vec::new();
        record::write_event(&mut buf, rec, serial)?;
        self.file.write_all(&buf)?;
        self.file.flush()?;
        self.written += buf.len() as u64;
        if self.written > LOG_MAX {
            self.rotate()?;
        }
        Ok(())
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        let path = PathBuf::from(LOG_DIR).join("audit.log");
        let _ = fs::rename(&path, path.with_extension("log.1"));
        self.file = OpenOptions::new().create(true).append(true).mode(0o600).open(&path)?;
        self.written = 0;
        Ok(())
    }
}

// --- Small helpers ------------------------------------------------------------------------------------------

fn map_named<'a>(object: &'a libbpf_rs::Object, needle: &str) -> Result<Map<'a>, Box<dyn Error>> {
    object
        .maps()
        .find(|m| m.name().to_str().map(|s| s.contains(needle)).unwrap_or(false))
        .ok_or_else(|| format!("the BPF object has no {needle} map").into())
}

/// Opens a folder read-only without following a final-component link, and reads back the inode the BPF left
/// in .bss for the sensor's own opens.
fn open_folder(path: &[u8], bss: &Map) -> Option<(OwnedFd, u64)> {
    let cpath = CString::new(path).ok()?;
    let fd = unsafe {
        libc::open(cpath.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
    };
    if fd < 0 {
        return None;
    }
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    // The file_open program wrote this folder's inode to globals.registered_inode (offset 8 of .bss).
    let value = bss.lookup(&0u32.to_ne_bytes(), MapFlags::ANY).ok()??;
    if value.len() < 16 {
        return None;
    }
    let inode = u64::from_ne_bytes(value[8..16].try_into().ok()?);
    if inode == 0 {
        return None;
    }
    Some((owned, inode))
}

fn cstr(b: &[u8]) -> Vec<u8> {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    b[..end].to_vec()
}

/// A dev_t as audit prints it: major:minor in hex, from the kernel's 32-bit encoding.
fn dev_str(dev: u32) -> String {
    format!("{:02x}:{:02x}", dev >> 20, dev & 0xf_ffff)
}

/// Realtime minus boottime, so an event's boottime stamp becomes a wall-clock one.
fn boot_offset_ns() -> i128 {
    let read = |clock| {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        unsafe { libc::clock_gettime(clock, &mut ts) };
        ts.tv_sec as i128 * 1_000_000_000 + ts.tv_nsec as i128
    };
    read(libc::CLOCK_REALTIME) - read(libc::CLOCK_BOOTTIME)
}
