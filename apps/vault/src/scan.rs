//! Reading a QR code: from a picture file, from the screen through GNOME's screenshot portal, or from the camera through its portal.
//! Decoding happens here and never in the service, since a picture from anywhere is untrusted; a picture is read through GdkPixbuf's
//! glycin sandbox, and what comes out goes to the service as text, which parses it strictly.
//!
//! A portal will not serve a program it cannot read, and the window is not dumpable. So the window has a small helper, itself started
//! with `--portal-screenshot` or `--portal-camera`, which asks the portal and is gone: it holds no secret but the picture it passes on.
//! A screenshot's file is deleted as soon as it has been read, so that the picture stays in memory only.

use std::cell::RefCell;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::rc::Rc;

use amethystora_ui::{gdk, gio, glib};
use gio::prelude::*;
use zeroize::Zeroizing;

/// A picture from anywhere is refused rather than read past this
const MAX_FILE: u64 = 32 * 1024 * 1024;
const MAX_PIXELS: usize = 64_000_000;

pub const CAMERA_WIDTH: usize = 640;
pub const CAMERA_HEIGHT: usize = 480;

/// Every QR code in a greyscale picture, as text
pub fn decode_grey(width: usize, height: usize, grey: &[u8]) -> Vec<Zeroizing<String>> {
    if width == 0 || height == 0 || grey.len() < width * height {
        return Vec::new();
    }
    let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(width, height, |x, y| grey[y * width + x]);
    prepared.detect_grids().into_iter().filter_map(|grid| grid.decode().ok()).map(|(_, text)| Zeroizing::new(text)).collect()
}

fn decode_pixbuf(pixbuf: &gdk::gdk_pixbuf::Pixbuf) -> Vec<Zeroizing<String>> {
    let (width, height, stride, channels) = (pixbuf.width() as usize, pixbuf.height() as usize, pixbuf.rowstride() as usize, pixbuf.n_channels() as usize);
    if width.saturating_mul(height) > MAX_PIXELS || channels < 3 {
        return Vec::new();
    }
    let bytes = pixbuf.read_pixel_bytes();
    let mut grey = Zeroizing::new(vec![0u8; width * height]);
    for y in 0..height {
        for x in 0..width {
            let at = y * stride + x * channels;
            let Some(pixel) = bytes.get(at..at + 3) else { continue };
            // Over white, for a picture with a transparent ground
            let alpha = if channels == 4 { u32::from(bytes.get(at + 3).copied().unwrap_or(255)) } else { 255 };
            let luma = (u32::from(pixel[0]) * 299 + u32::from(pixel[1]) * 587 + u32::from(pixel[2]) * 114) / 1000;
            grey[y * width + x] = ((luma * alpha + 255 * (255 - alpha)) / 255) as u8;
        }
    }
    decode_grey(width, height, &grey)
}

/// Blocks: the codes in a file
fn decode_file(path: &std::path::Path) -> Result<Vec<Zeroizing<String>>, ()> {
    let size = std::fs::metadata(path).map_err(|_| ())?.len();
    if size > MAX_FILE {
        return Err(());
    }
    // The size is read from the header first: a small file may claim a picture far too large to hold
    let (_, width, height) = gdk::gdk_pixbuf::Pixbuf::file_info(path).ok_or(())?;
    if (width.max(0) as usize).saturating_mul(height.max(0) as usize) > MAX_PIXELS {
        return Err(());
    }
    let pixbuf = gdk::gdk_pixbuf::Pixbuf::from_file(path).map_err(|_| ())?;
    Ok(decode_pixbuf(&pixbuf))
}

/// The codes in a picture file, off the window's thread. `Err` is a file that is not a picture, or too large.
pub async fn picture(path: std::path::PathBuf) -> Result<Vec<Zeroizing<String>>, ()> {
    gio::spawn_blocking(move || decode_file(&path)).await.unwrap_or(Err(()))
}

/// This program again, as the helper. By the name the launcher knows it by.
fn helper_command(what: &str) -> Option<Command> {
    let program = std::env::current_exe().ok()?;
    let mut command = Command::new(program);
    command.arg0("amethystora-vault").arg(what).stdin(Stdio::null()).stderr(Stdio::null());
    Some(command)
}

/// Blocks: a screenshot the person chose, through the portal, its codes, and its file deleted
fn screen_now() -> Result<Vec<Zeroizing<String>>, ()> {
    let mut command = helper_command("--portal-screenshot").ok_or(())?;
    let output = command.stdout(Stdio::piped()).output().map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    let uri = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let file = gio::File::for_uri(&uri);
    let path = file.path().ok_or(())?;
    let codes = decode_file(&path);
    // What the portal wrote holds whatever was on the screen, and the person asked only for the code
    let _ = std::fs::remove_file(&path);
    codes
}

/// The codes on the screen, off the window's thread
pub async fn screen() -> Result<Vec<Zeroizing<String>>, ()> {
    gio::spawn_blocking(screen_now).await.unwrap_or(Err(()))
}

/// A frame of the camera, for the window to show
pub struct Frame {
    pub grey: Vec<u8>,
}

/// What the camera's thread hands the window
pub enum Seen {
    Frame(Frame),
    Code(Zeroizing<String>),
    /// The camera stopped, or could not be opened
    Stopped,
}

thread_local! {
    static WATCHING: RefCell<Option<Rc<dyn Fn(Seen)>>> = const { RefCell::new(None) };
}

fn hand_over(seen: Seen) {
    glib::MainContext::default().invoke(move || {
        let watcher = WATCHING.with(|watching| watching.borrow().clone());
        if let Some(watcher) = watcher {
            watcher(seen);
        }
    });
}

/// The camera running: a helper with the portal's descriptor, whose frames a thread reads and looks for codes in. It stops when this
/// is dropped.
pub struct Camera {
    child: Child,
}

impl Drop for Camera {
    fn drop(&mut self) {
        WATCHING.with(|watching| watching.borrow_mut().take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Starts the camera. Five frames a second, each read for a code on the thread, so that the window only ever shows them.
pub fn camera(on_seen: impl Fn(Seen) + 'static) -> Option<Camera> {
    let mut command = helper_command("--portal-camera")?;
    let mut child = command.stdout(Stdio::piped()).spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    WATCHING.with(|watching| watching.replace(Some(Rc::new(on_seen))));
    std::thread::spawn(move || {
        let mut grey = vec![0u8; CAMERA_WIDTH * CAMERA_HEIGHT];
        loop {
            if stdout.read_exact(&mut grey).is_err() {
                hand_over(Seen::Stopped);
                return;
            }
            let found = decode_grey(CAMERA_WIDTH, CAMERA_HEIGHT, &grey);
            hand_over(Seen::Frame(Frame { grey: grey.clone() }));
            for text in found {
                hand_over(Seen::Code(text));
            }
        }
    });
    Some(Camera { child })
}

// --- The helper --------------------------------------------------------------------------------------------------------------

/// `--portal-screenshot` prints the file's address, and `--portal-camera` becomes the camera's pipeline, its frames on standard output.
/// Anything else: not the helper.
pub fn helper() -> Option<glib::ExitCode> {
    match std::env::args().nth(1).as_deref() {
        Some("--portal-screenshot") => Some(run_helper(screenshot_helper)),
        Some("--portal-camera") => Some(run_helper(camera_helper)),
        _ => None,
    }
}

fn run_helper(helper: fn() -> Option<()>) -> glib::ExitCode {
    match helper() {
        Some(()) => glib::ExitCode::SUCCESS,
        None => glib::ExitCode::FAILURE,
    }
}

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

/// Asks a portal for something it answers on a request object of its own, and waits for the answer: what it gave, if the person agreed
fn portal_request(bus: &gio::DBusConnection, interface: &str, method: &str, parameters: impl Fn(&str) -> glib::Variant) -> Option<glib::Variant> {
    let token = format!("amethystora_vault_{}", std::process::id());
    let sender = bus.unique_name()?.trim_start_matches(':').replace('.', "_");
    let path = format!("{PORTAL_PATH}/request/{sender}/{token}");
    let main_loop = glib::MainLoop::new(None, false);
    let answer: Rc<RefCell<Option<glib::Variant>>> = Rc::new(RefCell::new(None));
    let (quit, keep) = (main_loop.clone(), answer.clone());
    let _subscription = bus.subscribe_to_signal(Some(PORTAL), Some("org.freedesktop.portal.Request"), Some("Response"), Some(&path), None, gio::DBusSignalFlags::NONE, move |signal| {
        if signal.parameters.child_value(0).get::<u32>() == Some(0) {
            keep.replace(Some(signal.parameters.child_value(1)));
        }
        quit.quit();
    });
    bus.call_sync(Some(PORTAL), PORTAL_PATH, interface, method, Some(&parameters(&token)), None, gio::DBusCallFlags::NONE, -1, gio::Cancellable::NONE).ok()?;
    main_loop.run();
    answer.take()
}

fn options(entries: &[(&str, glib::Variant)]) -> glib::Variant {
    let dict = glib::VariantDict::new(None);
    for (key, value) in entries {
        dict.insert_value(key, value);
    }
    dict.end()
}

fn screenshot_helper() -> Option<()> {
    let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()?;
    let results = portal_request(&bus, "org.freedesktop.portal.Screenshot", "Screenshot", |token| {
        ("", options(&[("handle_token", token.to_variant()), ("interactive", true.to_variant())])).to_variant()
    })?;
    let uri = glib::VariantDict::new(Some(&results)).lookup::<String>("uri").ok()??;
    println!("{uri}");
    Some(())
}

fn camera_helper() -> Option<()> {
    let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()?;
    portal_request(&bus, "org.freedesktop.portal.Camera", "AccessCamera", |token| (options(&[("handle_token", token.to_variant())]),).to_variant())?;
    let (_, fds) = bus
        .call_with_unix_fd_list_sync(
            Some(PORTAL),
            PORTAL_PATH,
            "org.freedesktop.portal.Camera",
            "OpenPipeWireRemote",
            Some(&(options(&[]),).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            -1,
            gio::UnixFDList::NONE,
            gio::Cancellable::NONE,
        )
        .ok()?;
    let fd = fds?.get(0).ok()?;
    use std::os::fd::IntoRawFd;
    let fd = fd.into_raw_fd();
    // SAFETY: a descriptor this process owns, kept open across the exec that follows
    unsafe { libc::fcntl(fd, libc::F_SETFD, 0) };
    let caps = format!("video/x-raw,format=GRAY8,width={CAMERA_WIDTH},height={CAMERA_HEIGHT},framerate=5/1");
    // Only returns if it could not start
    let _ = Command::new("gst-launch-1.0")
        .args(["-q", "pipewiresrc", &format!("fd={fd}"), "!", "videoconvert", "!", "videoscale", "!", "videorate", "!", &caps, "!", "fdsink", "fd=1"])
        .exec();
    None
}
