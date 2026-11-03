//! Service logos, resolved online and never bundled: only from the service's own site, by `curl`, with only a host name going out, off
//! until the person chooses (`~/.config/amethystora/vault-logos`, which `ame vault logos on|off` writes too). A picture from anywhere
//! is untrusted: its header is checked, it is decoded through GdkPixbuf off the window's thread, at 256 KiB at most, and what is kept is
//! the window's own small PNG of it, sealed in the first vault as an item of kind `logo` named for the host: a cache outside the vault
//! would be a list of accounts. A hidden vault's items never ask for one, and nothing is kept for them.
//!
//! Costs nothing day to day: a logo is asked for only when an item that has a host is drawn and none is kept, at most once a month for
//! a host (a failure is kept too), one at a time, never on a metered connection, and with no timer: the next one starts when the last
//! has ended. Closing the vault forgets everything held here.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;

use amethystora_ui::gtk::prelude::GtkWindowExt;
use amethystora_ui::{gdk, gio, glib, palette, process};
use base64::Engine;
use gio::prelude::*;
use serde_json::json;

use crate::page::Vault;
use crate::widgets::Avatar;

/// What the site may send: more is refused unread
const MAX_PICTURE: usize = 256 * 1024;
/// What is kept: a picture no larger than this on a side
const SIDE: i32 = 64;
/// The widest a picture may say it is, which is checked before anything decodes it
const MAX_SIDE: u32 = 1024;
const FILE_KIND: &str = "logo";
/// A host is asked again after a month, kept or failed
const MONTH: i64 = 30 * 86_400;
/// Textures held in memory at once
const MOST_HELD: usize = 128;
const PATHS: [&str; 2] = ["/favicon.ico", "/apple-touch-icon.png"];
const METERED: &str = "/usr/libexec/amethystora-metered";
/// Names of networks that are not the site's: nothing is asked of them
const LOCAL_ENDINGS: [&str; 9] = [".local", ".localhost", ".localdomain", ".internal", ".lan", ".home", ".home.arpa", ".corp", ".intranet"];

/// `ame vault logos on|off` writes the same marker, which `amethystora-sends` reads
fn marker() -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()).map_or_else(|| palette::home().join(".config"), PathBuf::from);
    config.join("amethystora").join("vault-logos")
}

/// A picture ready to draw, and the PNG that is kept of it
struct Picture {
    width: i32,
    height: i32,
    stride: usize,
    rgba: Vec<u8>,
    png: Vec<u8>,
}

/// What the stored item of a host said when the list was read
#[derive(Clone)]
struct Known {
    id: String,
    /// When it was made, in seconds
    changed: i64,
    /// A picture is kept, rather than only the word that the site had none
    has_picture: bool,
    /// The picture, in base64, while a month has passed and a fresh one is being fetched: if the site gives none, this is kept again
    kept: Option<String>,
}

#[derive(Default)]
struct State {
    on: bool,
    /// The first vault, when it is open: where logos are kept
    first: Option<String>,
    textures: HashMap<String, gdk::Texture>,
    known: Option<HashMap<String, Known>>,
    waiting: HashMap<String, Vec<glib::WeakRef<Avatar>>>,
    queue: VecDeque<String>,
    /// Hosts being looked up, so that none is asked for twice
    working: HashSet<String>,
    busy: bool,
    reading_list: bool,
    /// The vault would not keep one: no more are asked for until it closes
    given_up: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// The window, which a stone in a list asks for its logo through
    static WINDOW: RefCell<std::rc::Weak<Vault>> = const { RefCell::new(std::rc::Weak::new()) };
}

pub fn init(vault: &Rc<Vault>) {
    WINDOW.with(|window| window.replace(Rc::downgrade(vault)));
}

pub fn is_on() -> bool {
    STATE.with(|state| state.borrow().on)
}

/// The marker read again, off the window's thread
pub async fn refresh() {
    let on = gio::spawn_blocking(|| marker().exists()).await.unwrap_or(false);
    STATE.with(|state| state.borrow_mut().on = on);
}

pub async fn set_on(on: bool) {
    let _ = gio::spawn_blocking(move || {
        let path = marker();
        if on {
            if let Some(folder) = path.parent() {
                let _ = std::fs::create_dir_all(folder);
            }
            let _ = std::fs::write(&path, "");
        } else {
            let _ = std::fs::remove_file(&path);
        }
    })
    .await;
    refresh().await;
    if !on {
        forget();
    }
}

/// The first vault when it is open, which is the only place logos are kept
pub fn set_first(first: Option<String>) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.first != first {
            state.known = None;
            state.given_up = false;
        }
        state.first = first;
    });
}

/// Everything held: the vault has closed
pub fn forget() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.textures.clear();
        state.known = None;
        state.waiting.clear();
        state.queue.clear();
        state.working.clear();
        state.first = None;
        state.given_up = false;
    });
}

/// Whether a logo can be asked for or drawn now
pub fn available() -> bool {
    STATE.with(|state| {
        let state = state.borrow();
        state.on && state.first.is_some() && !state.given_up
    })
}

/// A host name, and nothing that could hold anything else: letters, digits, dots and dashes, a name of the open internet. An address
/// of a machine, a name of a network at home and anything with a port, a path or a login in it are left alone.
pub fn host_of(address: &str) -> Option<String> {
    let address = address.trim();
    let rest = match address.split_once("://") {
        Some(("https" | "http", rest)) => rest,
        Some(_) => return None,
        None => address,
    };
    let host = rest.split(['/', '?', '#']).next()?;
    // A login in front, or a port after, is not a host name
    if host.contains('@') || host.contains(':') {
        return None;
    }
    let host = host.to_ascii_lowercase();
    let sound = host.len() <= 253
        && host.contains('.')
        && host.split('.').all(|part| !part.is_empty() && part.len() <= 63 && !part.starts_with('-') && !part.ends_with('-') && part.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'))
        // A name ends in letters: a number at the end is an address
        && host.rsplit('.').next().is_some_and(|last| last.bytes().any(|byte| byte.is_ascii_alphabetic()));
    let dotted = format!(".{host}");
    (sound && !LOCAL_ENDINGS.iter().any(|ending| dotted.ends_with(ending))).then_some(host)
}

// --- The picture ---------------------------------------------------------------------------------------------------------------

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn le16(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?)))
}

fn le32(bytes: &[u8], at: usize) -> Option<i64> {
    Some(i64::from(i32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?)))
}

/// A PNG that says it is no wider than `MAX_SIDE` each way, from its first chunk
fn png_fits(bytes: &[u8]) -> bool {
    bytes.starts_with(&PNG_SIGNATURE)
        && bytes.get(12..16) == Some(b"IHDR")
        && matches!((be32(bytes, 16), be32(bytes, 20)), (Some(width), Some(height)) if (1..=MAX_SIDE).contains(&width) && (1..=MAX_SIDE).contains(&height))
}

/// Whether a picture is one of the two kinds a site's icon is (PNG, or ICO of PNGs and bitmaps), and says it is small enough to
/// decode. Read from its own bytes, before GdkPixbuf sees it: a small file may claim a picture far too large to hold. Anything else
/// (SVG, JPEG, GIF, WebP) is refused, since an icon is not.
fn bounded(bytes: &[u8]) -> bool {
    if bytes.len() > MAX_PICTURE {
        return false;
    }
    if bytes.starts_with(&PNG_SIGNATURE) {
        return png_fits(bytes);
    }
    // ICO: reserved 0, type 1, a count, then 16 bytes for each picture
    let (Some(0), Some(1), Some(count)) = (le16(bytes, 0), le16(bytes, 2), le16(bytes, 4)) else { return false };
    if !(1..=32).contains(&count) {
        return false;
    }
    (0..count as usize).all(|at| {
        let entry = 6 + at * 16;
        let (Some(size), Some(offset)) = (le32(bytes, entry + 8), le32(bytes, entry + 12)) else { return false };
        let (Ok(size), Ok(offset)) = (usize::try_from(size), usize::try_from(offset)) else { return false };
        let Some(data) = offset.checked_add(size).and_then(|end| bytes.get(offset..end)) else { return false };
        if data.starts_with(&PNG_SIGNATURE) {
            return png_fits(data);
        }
        // A bitmap: its header size, then width, and height twice over (the mask follows the colours)
        matches!((le32(data, 0), le32(data, 4), le32(data, 8)), (Some(header), Some(width), Some(height)) if header >= 40 && (1..=256).contains(&width) && (1..=512).contains(&height.abs()))
    })
}

/// Blocks: a picture checked, decoded, made no larger than `SIDE` and given its alpha channel, and the PNG it is kept as
fn prepare(bytes: &[u8]) -> Option<Picture> {
    if !bounded(bytes) {
        return None;
    }
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from(bytes));
    let mut pixbuf = gdk::gdk_pixbuf::Pixbuf::from_stream(&stream, gio::Cancellable::NONE).ok()?;
    let (width, height) = (pixbuf.width(), pixbuf.height());
    if width < 1 || height < 1 || width.max(height) > MAX_SIDE as i32 {
        return None;
    }
    if width.max(height) > SIDE {
        let scale = f64::from(SIDE) / f64::from(width.max(height));
        pixbuf = pixbuf.scale_simple(((f64::from(width) * scale) as i32).max(1), ((f64::from(height) * scale) as i32).max(1), gdk::gdk_pixbuf::InterpType::Bilinear)?;
    }
    if !pixbuf.has_alpha() {
        pixbuf = pixbuf.add_alpha(false, 0, 0, 0).ok()?;
    }
    let png = pixbuf.save_to_bufferv("png", &[]).ok()?;
    Some(Picture { width: pixbuf.width(), height: pixbuf.height(), stride: pixbuf.rowstride() as usize, rgba: pixbuf.read_pixel_bytes().to_vec(), png })
}

/// Blocks: what the site sends at `url`, at 256 KiB at most. `curl` as an argument vector, to the site's own address and nothing
/// else: only https, even through a redirect, which is followed twice at most, no settings file, no cookies kept, a time limit.
fn curl_get(url: &str) -> Option<Vec<u8>> {
    let mut child = Command::new("curl")
        .args(["-q", "-s", "-f", "-L", "--proto", "=https", "--proto-redir", "=https", "--max-redirs", "2", "--max-time", "8", "--max-filesize", "262144", "--", url])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    let mut bytes = Vec::new();
    // One byte past the limit says it was too much; the rest is not read
    let read = child.stdout.take()?.take(MAX_PICTURE as u64 + 1).read_to_end(&mut bytes);
    if bytes.len() > MAX_PICTURE {
        let _ = child.kill();
    }
    let status = child.wait().ok();
    (read.is_ok() && bytes.len() <= MAX_PICTURE && !bytes.is_empty() && status.is_some_and(|status| status.success())).then_some(bytes)
}

/// Blocks: the first picture the site's own address gives
fn fetch_now(host: &str) -> Option<Picture> {
    PATHS.iter().find_map(|path| curl_get(&format!("https://{host}{path}")).and_then(|bytes| prepare(&bytes)))
}

/// Blocks: a kept PNG, decoded
fn decode_kept(base64_png: &str) -> Option<Picture> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(base64_png).ok()?;
    if !png_fits(&bytes) {
        return None;
    }
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from(&bytes[..]));
    let pixbuf = gdk::gdk_pixbuf::Pixbuf::from_stream(&stream, gio::Cancellable::NONE).ok()?;
    if pixbuf.width().max(pixbuf.height()) > SIDE {
        return None;
    }
    let pixbuf = if pixbuf.has_alpha() { pixbuf } else { pixbuf.add_alpha(false, 0, 0, 0).ok()? };
    Some(Picture { width: pixbuf.width(), height: pixbuf.height(), stride: pixbuf.rowstride() as usize, rgba: pixbuf.read_pixel_bytes().to_vec(), png: Vec::new() })
}

fn texture_of(picture: &Picture) -> gdk::Texture {
    gdk::MemoryTexture::new(picture.width, picture.height, gdk::MemoryFormat::R8g8b8a8, &glib::Bytes::from(&picture.rgba[..]), picture.stride).upcast()
}

// --- What is drawn ---------------------------------------------------------------------------------------------------------------

/// Gives a stone its logo if one is held, and otherwise has it looked for: the letter stays until one arrives, and whenever none does
pub fn attach(avatar: &Avatar, host: Option<&str>) {
    avatar.set_host(host.map(str::to_string));
    let Some(host) = host else { return };
    let Some(vault) = WINDOW.with(|window| window.borrow().upgrade()) else { return };
    if !available() {
        return;
    }
    if let Some(texture) = STATE.with(|state| state.borrow().textures.get(host).cloned()) {
        avatar.set_logo(Some(texture));
        return;
    }
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let waiting = state.waiting.entry(host.to_string()).or_default();
        waiting.retain(|other| other.upgrade().is_some());
        waiting.push(avatar.downgrade());
    });
    look(&vault, host.to_string());
}

/// The bundled copy of 2fa.directory, trimmed to the services' pages
pub const DIRECTORY: &str = "/usr/share/amethystora/vault/2fa-directory.json";
const MOST_DIRECTORY: u64 = 4 * 1024 * 1024;

/// Blocks: the site of each service the directory knows, by the name it was asked by. Read, looked through and let go; nothing is
/// kept of it, and the name is not sent anywhere. A host is taken only if it has the shape of one.
fn hosts_in(directory: &[u8], names: &[String]) -> HashMap<String, String> {
    let Some(directory) = amethystora_vault_core::codes::Directory::parse(directory) else { return HashMap::new() };
    names.iter().filter_map(|name| directory.find(name).and_then(|row| host_of(&row.domain)).map(|host| (name.clone(), host))).collect()
}

fn directory_hosts(names: &[String]) -> HashMap<String, String> {
    let mut bytes = Vec::new();
    let read = std::fs::File::open(DIRECTORY).and_then(|file| file.take(MOST_DIRECTORY + 1).read_to_end(&mut bytes));
    if read.is_err() || bytes.len() as u64 > MOST_DIRECTORY {
        return HashMap::new();
    }
    hosts_in(&bytes, names)
}

/// Gives the rows of a list of services (authenticator accounts, backup codes) the site each stands for, from the directory, once
/// the list is drawn. Never for what a hidden vault holds.
pub async fn fill(vault: &Rc<Vault>, list: &Rc<crate::list::ListPage>) {
    if !available() {
        return;
    }
    let hidden = vault.hidden_vaults();
    let names: Vec<String> = list.titles().into_iter().filter(|(_, held)| !hidden.contains(held)).map(|(title, _)| title).collect();
    if names.is_empty() {
        return;
    }
    let found = gio::spawn_blocking(move || directory_hosts(&names)).await.unwrap_or_default();
    if !found.is_empty() {
        list.set_hosts(|row| if hidden.contains(&row.vault) { None } else { found.get(&row.title).cloned() });
    }
}

/// A texture came: every stone waiting for it that still shows that host takes it
fn arrived(host: &str, texture: gdk::Texture) {
    let waiting = STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.textures.len() >= MOST_HELD {
            state.textures.clear();
        }
        state.textures.insert(host.to_string(), texture.clone());
        state.waiting.remove(host).unwrap_or_default()
    });
    for avatar in waiting.into_iter().filter_map(|avatar| avatar.upgrade()) {
        if avatar.host().as_deref() == Some(host) {
            avatar.set_logo(Some(texture.clone()));
        }
    }
}

/// What the first vault keeps, read once for the time it is open: the host each item is named for, and when it was made
async fn read_known(vault: &Rc<Vault>) {
    if STATE.with(|state| state.borrow().known.is_some()) {
        return;
    }
    if STATE.with(|state| std::mem::replace(&mut state.borrow_mut().reading_list, true)) {
        // Another lookup is reading it: wait for that
        while STATE.with(|state| state.borrow().reading_list) {
            glib::timeout_future(std::time::Duration::from_millis(40)).await;
        }
        return;
    }
    let mut known = HashMap::new();
    let first = STATE.with(|state| state.borrow().first.clone());
    if let Ok(answer) = vault.call(json!({"op": "list", "kind": FILE_KIND})).await {
        for item in answer["items"].as_array().into_iter().flatten() {
            if first.is_some() && item["vault"].as_str() != first.as_deref() {
                continue;
            }
            let (Some(name), Some(id)) = (item["name"].as_str(), item["id"].as_str()) else { continue };
            known.insert(name.to_string(), Known { id: id.to_string(), changed: item["changed"].as_i64().unwrap_or(0), has_picture: item["meta"]["picture"] == json!(true), kept: None });
        }
    }
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.known = Some(known);
        state.reading_list = false;
    });
}

fn now_secs() -> i64 {
    amethystora_ui::i18n::now() as i64
}

/// Finds the logo of a host: the one the vault keeps, or one to be fetched
fn look(vault: &Rc<Vault>, host: String) {
    if !STATE.with(|state| state.borrow_mut().working.insert(host.clone())) {
        return;
    }
    let vault = vault.clone();
    glib::spawn_future_local(async move {
        read_known(&vault).await;
        let known = STATE.with(|state| state.borrow().known.as_ref().and_then(|known| known.get(&host).cloned()));
        let stale = known.as_ref().is_none_or(|known| now_secs() - known.changed >= MONTH);
        if let Some(known) = known.as_ref().filter(|known| known.has_picture) {
            if let Ok(answer) = vault.call(json!({"op": "get", "kind": FILE_KIND, "id": known.id})).await {
                let kept = answer["value"]["picture"].as_str().map(str::to_string);
                drop(answer);
                if let Some(kept) = kept {
                    if stale {
                        STATE.with(|state| {
                            if let Some(known) = state.borrow_mut().known.as_mut().and_then(|known| known.get_mut(&host)) {
                                known.kept = Some(kept.clone());
                            }
                        });
                    }
                    let picture = gio::spawn_blocking(move || decode_kept(&kept)).await.ok().flatten();
                    if let Some(picture) = picture {
                        arrived(&host, texture_of(&picture));
                    }
                }
            }
        }
        if stale {
            STATE.with(|state| state.borrow_mut().queue.push_back(host.clone()));
            pump(&vault);
        } else {
            STATE.with(|state| state.borrow_mut().working.remove(&host));
        }
    });
}

/// The next fetch, when none is going: one at a time, only while the window has the focus
pub fn pump(vault: &Rc<Vault>) {
    let next = STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.busy || state.given_up || !state.on || state.first.is_none() || !vault.shell.window.is_active() {
            return None;
        }
        let next = state.queue.pop_front()?;
        state.busy = true;
        Some(next)
    });
    let Some(host) = next else { return };
    let vault = vault.clone();
    glib::spawn_future_local(async move {
        fetch_and_keep(&vault, &host).await;
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.busy = false;
            state.working.remove(&host);
        });
        pump(&vault);
    });
}

/// Whether the connection counts what it carries, as `amethystora-metered` has it: 0 for yes, 1 for no. A program that cannot be
/// asked counts as yes, so that nothing is fetched on a guess.
async fn metered() -> bool {
    process::run(METERED, &["metered"]).await.code != 1
}

async fn fetch_and_keep(vault: &Rc<Vault>, host: &str) {
    if metered().await || !available() {
        // Asked again when the screen is drawn again
        return;
    }
    let name = host.to_string();
    let found = gio::spawn_blocking(move || fetch_now(&name)).await.ok().flatten();
    let (id, before) = STATE.with(|state| {
        let state = state.borrow();
        let known = state.known.as_ref().and_then(|known| known.get(host));
        (known.map(|known| known.id.clone()), known.and_then(|known| known.kept.clone()))
    });
    let first = STATE.with(|state| state.borrow().first.clone());
    let Some(first) = first else { return };
    // A site with none leaves a note of it, so that it is not asked again for a month; a picture it gave before is kept as it was
    let (picture, texture) = match &found {
        Some(picture) => (Some(base64::engine::general_purpose::STANDARD.encode(&picture.png)), Some(texture_of(picture))),
        None => (before, None),
    };
    let has_picture = picture.is_some();
    let mut request = json!({"op": "put", "kind": FILE_KIND, "name": host, "meta": {"picture": has_picture}, "value": {}, "vault": first, "approval": "never"});
    if let Some(id) = id {
        request["id"] = json!(id);
    }
    if let Some(picture) = picture {
        request["value"] = json!({"picture": picture});
    }
    match vault.call(request).await {
        Ok(answer) => {
            let id = answer["id"].as_str().unwrap_or("").to_string();
            STATE.with(|state| {
                if let Some(known) = state.borrow_mut().known.as_mut() {
                    known.insert(host.to_string(), Known { id, changed: now_secs(), has_picture, kept: None });
                }
            });
            if let Some(texture) = texture {
                arrived(host, texture);
            }
        }
        // The vault would not keep it, or could not: nothing more is asked of sites until it has closed
        Err(_) => STATE.with(|state| state.borrow_mut().given_up = true),
    }
}

/// The window has the focus again: what waited for it goes on
pub fn resume(vault: &Rc<Vault>) {
    pump(vault);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_host_goes_out() {
        assert_eq!(host_of("https://github.com/login?x=1").as_deref(), Some("github.com"));
        assert_eq!(host_of("Example.org/path").as_deref(), Some("example.org"));
        assert_eq!(host_of("http://sub.example.co.uk").as_deref(), Some("sub.example.co.uk"));
        assert_eq!(host_of("user:secret@Example.org:8443/path"), None);
        assert_eq!(host_of("https://user@example.org/"), None);
        assert_eq!(host_of("https://example.org:8443/"), None);
        assert_eq!(host_of("localhost"), None);
        assert_eq!(host_of("https://exa mple.org"), None);
        assert_eq!(host_of("https://-bad.org"), None);
        assert_eq!(host_of("ftp://example.org"), None);
        assert_eq!(host_of("javascript:alert(1)//example.org"), None);
        assert_eq!(host_of(""), None);
    }

    #[test]
    fn a_machine_or_a_home_network_is_not_asked() {
        assert_eq!(host_of("https://192.168.1.5/"), None);
        assert_eq!(host_of("https://8.8.8.8"), None);
        assert_eq!(host_of("https://nas.local"), None);
        assert_eq!(host_of("https://router.home.arpa/admin"), None);
        assert_eq!(host_of("https://wiki.internal"), None);
        assert_eq!(host_of("https://[::1]/"), None);
    }

    #[test]
    fn a_service_is_found_in_the_directory_and_its_host_checked() {
        let file = br#"{"entries":[["GitHub","github.com",0,0],["Odd","exa mple.org",0,0],["Local","nas.local",0,0]]}"#;
        let found = hosts_in(file, &["GitHub".to_string(), "Odd".to_string(), "Local".to_string(), "Unknown".to_string()]);
        assert_eq!(found.get("GitHub").map(String::as_str), Some("github.com"));
        // A row whose domain is not a host name, or is one of a network at home, or is not there at all, gets nothing
        assert_eq!(found.len(), 1);
    }

    /// A PNG of one grey pixel claiming a size
    fn png_claiming(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = PNG_SIGNATURE.to_vec();
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 0, 0, 0, 0]);
        bytes
    }

    #[test]
    fn a_picture_that_claims_too_much_is_refused_before_it_is_read() {
        assert!(bounded(&png_claiming(64, 64)));
        assert!(bounded(&png_claiming(1024, 1024)));
        assert!(!bounded(&png_claiming(1025, 16)));
        assert!(!bounded(&png_claiming(30_000, 30_000)));
        assert!(!bounded(&png_claiming(0, 16)));
        assert!(!bounded(b"GIF89a\x01\x00\x01\x00"));
        assert!(!bounded(b"<svg xmlns='http://www.w3.org/2000/svg'/>"));
        assert!(!bounded(&[]));
        // Not a picture at all, and larger than anyone may send
        assert!(prepare(b"just some text").is_none());
        assert!(!bounded(&vec![0x89; MAX_PICTURE + 1]));
    }

    /// An ICO of one bitmap picture claiming a size
    fn ico_claiming(width: i32, height: i32) -> Vec<u8> {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40i32.to_le_bytes());
        dib.extend_from_slice(&width.to_le_bytes());
        dib.extend_from_slice(&height.to_le_bytes());
        dib.extend_from_slice(&[0; 28]);
        let mut bytes = vec![0, 0, 1, 0, 1, 0];
        bytes.extend_from_slice(&[0, 0, 0, 0, 1, 0, 32, 0]);
        bytes.extend_from_slice(&(dib.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&22u32.to_le_bytes());
        bytes.extend_from_slice(&dib);
        bytes
    }

    #[test]
    fn an_icon_file_is_checked_by_what_it_holds() {
        assert!(bounded(&ico_claiming(32, 64)));
        assert!(!bounded(&ico_claiming(30_000, 60_000)));
        assert!(!bounded(&ico_claiming(32, 100_000)));
        let mut short = ico_claiming(32, 64);
        short.truncate(30);
        assert!(!bounded(&short));
    }

    #[test]
    fn a_real_picture_is_made_small_and_kept_as_png() {
        let pixbuf = gdk::gdk_pixbuf::Pixbuf::new(gdk::gdk_pixbuf::Colorspace::Rgb, false, 8, 200, 100).unwrap();
        pixbuf.fill(0x3366ccff);
        let png = pixbuf.save_to_bufferv("png", &[]).unwrap();
        assert!(bounded(&png));
        let made = prepare(&png).expect("a PNG decodes");
        assert_eq!((made.width, made.height), (64, 32));
        assert!(made.png.len() < 4096);
        assert!(png_fits(&made.png));
        // What is kept comes back the same size
        let kept = decode_kept(&base64::engine::general_purpose::STANDARD.encode(&made.png)).expect("what is kept reads");
        assert_eq!((kept.width, kept.height), (64, 32));
        // Damaged after its header: refused when decoded, not a panic
        let mut damaged = png.clone();
        let half = damaged.len() / 2;
        damaged.truncate(half);
        assert!(prepare(&damaged).is_none());
    }
}
