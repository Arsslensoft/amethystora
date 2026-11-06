//! Files, in both directions (spec-plugins.md 5). A file from a phone is fetched on a connection of its own and
//! put in the phone's folder under a name that cannot leave it; a file to a phone is offered on a port of ours
//! and written to the one connection that passes the pin. Each transfer is a task that can be cancelled, and the
//! partial file of one that ends any other way than whole is removed.

use std::cell::Cell;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use futures_util::future::abortable;
use gio::prelude::*;
use serde_json::json;

use crate::dbus;
use crate::device::{BatchState, Device};
use crate::limits;
use crate::link::Link;
use crate::notify::Notice;
use crate::packet::{self, Packet};
use crate::payload;
use crate::service::{ApiError, ApiResult, Service, Transfer};
use crate::share::{self, Batch};
use crate::text::{self, tr, trf};

/// Removes a partial file unless the transfer kept it
struct Part {
    path: PathBuf,
    keep: bool,
}

impl Drop for Part {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// How much of a disk is left alone when a file would fill it
const RESERVE: u64 = 64 * 1024 * 1024;

/// Files from one phone that may be announced and not done with: the rest are refused at once
const MOST_QUEUED: usize = 64;

/// The size of a file being fetched, set aside from the free space until it is done or given up
struct Reserved<'a> {
    total: &'a Cell<u64>,
    bytes: u64,
}

impl<'a> Reserved<'a> {
    fn new(total: &'a Cell<u64>, bytes: u64) -> Reserved<'a> {
        total.set(total.get().saturating_add(bytes));
        Reserved { total, bytes }
    }
}

impl Drop for Reserved<'_> {
    fn drop(&mut self) {
        self.total.set(self.total.get().saturating_sub(self.bytes));
    }
}

/// Progress, at most ten times a second
struct Throttle {
    last: Cell<Instant>,
}

impl Throttle {
    fn new() -> Throttle {
        Throttle { last: Cell::new(Instant::now() - Duration::from_secs(1)) }
    }

    fn due(&self) -> bool {
        if self.last.get().elapsed() >= Duration::from_millis(100) {
            self.last.set(Instant::now());
            true
        } else {
            false
        }
    }
}

impl Service {
    fn register_transfer(self: &Rc<Self>, tid: &str, dev: &Rc<Device>, handle: futures_util::future::AbortHandle, name: &str, incoming: bool, total: u64) {
        self.transfers.borrow_mut().insert(tid.to_string(), Transfer { device: dev.id.clone(), abort: handle, name: name.to_string(), incoming, total });
    }

    fn finish_transfer(&self, tid: &str) {
        self.transfers.borrow_mut().remove(tid);
    }

    pub fn cancel_transfer(self: &Rc<Self>, device: &str, tid: &str) -> ApiResult<()> {
        let belongs = self.transfers.borrow().get(tid).is_some_and(|t| t.device == device);
        if !belongs {
            return Err(ApiError::unavailable("no such transfer"));
        }
        self.cancel_transfer_by_id(tid);
        Ok(())
    }

    /// Ends a transfer: its task is dropped, which closes its connection and removes its partial file
    pub fn cancel_transfer_by_id(self: &Rc<Self>, tid: &str) {
        let Some(t) = self.transfers.borrow_mut().remove(tid) else { return };
        t.abort.abort();
        self.notifier.close(&format!("xfer:{}:{tid}", t.device));
        if let Some(dev) = self.device(&t.device) {
            dbus::transfer(self, &dev, tid, &t.name, t.incoming, 0, t.total, "cancelled");
        }
    }

    // --- From a phone ---------------------------------------------------------------------------------------

    pub fn receive_file(self: &Rc<Self>, dev: &Rc<Device>, link: &Rc<Link>, p: &Packet, name: String, modified: Option<i64>, batch: Option<Batch>) {
        // Whose files wait for Accept (ask.rs): the others are fetched at once, as they always were
        if !self.hold_for_accept(dev, link, p, &name, modified, batch) {
            self.receive_file_now(dev, link, p, name, modified, batch);
        }
    }

    pub(crate) fn receive_file_now(self: &Rc<Self>, dev: &Rc<Device>, link: &Rc<Link>, p: &Packet, name: String, modified: Option<i64>, batch: Option<Batch>) {
        let tid = format!("t{}", self.next_id());
        let shown = text::file_name(&name);
        let payload_ = match p.payload() {
            Ok(Some(pl)) => Some(pl),
            Ok(None) if p.declared_size() == Some(0) => None,
            _ => {
                self.transfer_failed(dev, &tid, &shown, true, 0, &tr("The phone did not say how to fetch the file."), batch);
                return;
            }
        };
        let size = payload_.map_or(0, |pl| pl.size);
        // A phone that announces more files than can be waited for gets the rest refused
        if dev.st.borrow().queued >= MOST_QUEUED {
            if let (Some(pl), Some(pin)) = (payload_, self.pin_of(&dev.id)) {
                let (ip, scope, cert) = (link.ip(), link.peer.2, self.me.tls.clone());
                self.spawn_task(async move { payload::refuse(ip, scope, pl.port, &cert, pin).await });
            }
            return;
        }
        dev.st.borrow_mut().queued += 1;
        let service = self.clone();
        let (dev, link) = (dev.clone(), link.clone());
        let task = async move {
            // At most four at once for a phone, eight in all: the others wait their turn
            while dev.st.borrow().downloads >= limits::DOWNLOADS_PER_DEVICE || service.downloads_active.get() >= limits::DOWNLOADS_IN_ALL {
                glib::timeout_future(Duration::from_millis(200)).await;
            }
            dev.st.borrow_mut().downloads += 1;
            service.downloads_active.set(service.downloads_active.get() + 1);
            service.receive_one(&dev, &link, &tid, &shown, payload_, size, modified, batch).await;
            {
                let mut st = dev.st.borrow_mut();
                st.downloads = st.downloads.saturating_sub(1);
            }
            service.downloads_active.set(service.downloads_active.get().saturating_sub(1));
            {
                let mut st = dev.st.borrow_mut();
                st.queued = st.queued.saturating_sub(1);
            }
        };
        self.spawn_task(task);
    }

    #[allow(clippy::too_many_arguments)]
    async fn receive_one(self: &Rc<Self>, dev: &Rc<Device>, link: &Rc<Link>, tid: &str, shown: &str, payload_: Option<packet::Payload>, size: u64, modified: Option<i64>, batch: Option<Batch>) {
        let (work, handle) = abortable(self.fetch_into_folder(dev, link, tid, shown, payload_, size, modified, batch));
        self.register_transfer(tid, dev, handle, shown, true, size);
        let _ = work.await;
        self.finish_transfer(tid);
    }

    #[allow(clippy::too_many_arguments)]
    async fn fetch_into_folder(self: &Rc<Self>, dev: &Rc<Device>, link: &Rc<Link>, tid: &str, shown: &str, payload_: Option<packet::Payload>, size: u64, modified: Option<i64>, batch: Option<Batch>) {
        let refuse = |service: &Rc<Service>| {
            if let (Some(pl), Some(pin)) = (payload_, service.pin_of(&dev.id)) {
                let (ip, scope, cert) = (link.ip(), link.peer.2, service.me.tls.clone());
                service.spawn_task(async move { payload::refuse(ip, scope, pl.port, &cert, pin).await });
            }
        };
        let folder = match self.folder_for(dev, shown) {
            Ok(f) => f,
            Err(e) => {
                refuse(self);
                self.transfer_failed(dev, tid, shown, true, size, &trf("The folder for files from phones cannot be made: {error}", &[("error", &e.to_string())]), batch);
                return;
            }
        };
        // What the files already on the way will take is not there to be had again
        if share::free_space(&folder).is_some_and(|free| size > free.saturating_sub(RESERVE).saturating_sub(self.reserved.get())) {
            refuse(self);
            self.transfer_failed(dev, tid, shown, true, size, &tr("There is not enough space for this file."), batch);
            return;
        }
        let _reserved = Reserved::new(&self.reserved, size);
        let (mut file, part_path) = match share::create_part(&folder) {
            Ok(x) => x,
            Err(e) => {
                refuse(self);
                self.transfer_failed(dev, tid, shown, true, size, &e.to_string(), batch);
                return;
            }
        };
        let mut part = Part { path: part_path, keep: false };
        dbus::transfer(self, dev, tid, shown, true, 0, size, "started");
        if batch.is_none() {
            self.raise(Notice {
                key: format!("xfer:{}:{tid}", dev.id),
                summary: trf("Receiving from “{name}”", &[("name", &dev.name())]),
                body: shown.to_string(),
                icon: Some("document-save-symbolic".into()),
                actions: vec![("cancel".into(), tr("Cancel"))],
                ..Notice::default()
            });
        }

        let outcome = match payload_ {
            // An empty file has nothing to fetch
            None => Ok(0),
            Some(pl) => {
                let Some(pin) = self.pin_of(&dev.id) else {
                    self.transfer_failed(dev, tid, shown, true, size, &tr("The phone is not paired."), batch);
                    return;
                };
                let throttle = Throttle::new();
                let service = self.clone();
                payload::fetch(
                    link.ip(),
                    link.peer.2,
                    pl.port,
                    &self.me.tls,
                    pin,
                    size,
                    &mut |chunk| file.write_all(chunk),
                    &|done| {
                        if throttle.due() {
                            dbus::transfer(&service, dev, tid, shown, true, done, size, "progress");
                        }
                    },
                )
                .await
            }
        };
        self.notifier.close(&format!("xfer:{}:{tid}", dev.id));
        let total = match outcome {
            Ok(total) if total >= size => total,
            Ok(_) => {
                drop(file);
                self.transfer_failed(dev, tid, shown, true, size, &tr("The phone stopped sending before the file was whole."), batch);
                return;
            }
            Err(e) => {
                drop(file);
                self.transfer_failed(dev, tid, shown, true, size, &trf("The transfer failed: {error}", &[("error", &e.to_string())]), batch);
                return;
            }
        };
        let _ = file.sync_all();
        drop(file);
        match share::publish(&part.path, &folder, shown) {
            Ok(path) => {
                part.keep = true;
                if let Some(when) = modified.and_then(|m| share::plausible_time(m, SystemTime::now())) {
                    share::set_modified(&path, when);
                }
                self.transfer_done(dev, tid, shown, total, &path, batch);
            }
            Err(e) => self.transfer_failed(dev, tid, shown, true, size, &e.to_string(), batch),
        }
    }

    fn transfer_failed(self: &Rc<Self>, dev: &Rc<Device>, tid: &str, name: &str, incoming: bool, total: u64, why: &str, batch: Option<Batch>) {
        dbus::transfer(self, dev, tid, name, incoming, 0, total, "failed");
        self.notifier.close(&format!("xfer:{}:{tid}", dev.id));
        if let (true, Some(b)) = (incoming, batch) {
            if self.batch_step(dev, b, None, false) {
                return;
            }
        }
        self.raise(Notice {
            key: format!("fail:{}:{tid}", dev.id),
            summary: if incoming { trf("A file from “{name}” was not received", &[("name", &dev.name())]) } else { trf("A file for “{name}” was not sent", &[("name", &dev.name())]) },
            body: format!("{name}\n{why}"),
            icon: Some("dialog-warning-symbolic".into()),
            ..Notice::default()
        });
    }

    fn transfer_done(self: &Rc<Self>, dev: &Rc<Device>, tid: &str, name: &str, total: u64, path: &Path, batch: Option<Batch>) {
        dbus::transfer(self, dev, tid, name, true, total, total, "finished");
        {
            let mut received = self.received.borrow_mut();
            received.push_back((tid.to_string(), path.to_path_buf()));
            while received.len() > 50 {
                received.pop_front();
            }
        }
        if let Some(b) = batch {
            if self.batch_step(dev, b, Some(path.to_path_buf()), true) {
                return;
            }
        }
        // Open is offered for the kinds of file that are safe to open; for any other the folder is the way
        let mut actions = Vec::new();
        if share::safe_to_open(path) {
            actions.push(("open".to_string(), tr("Open")));
        }
        actions.push(("show".to_string(), tr("Show in folder")));
        self.raise(Notice {
            key: format!("got:{}:{tid}", dev.id),
            summary: trf("Received from “{name}”", &[("name", &dev.name())]),
            body: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            icon: Some("document-save-symbolic".into()),
            actions,
            ..Notice::default()
        });
    }

    /// A file of a batch is in, or has failed. True when the batch told of itself here (so no notice of the file's own).
    fn batch_step(self: &Rc<Self>, dev: &Rc<Device>, b: Batch, path: Option<PathBuf>, ok: bool) -> bool {
        if b.files < 2 {
            return false;
        }
        let finished = {
            let mut st = dev.st.borrow_mut();
            let current = match &mut st.batch {
                Some(c) if c.files == b.files && c.bytes == b.bytes && c.started.elapsed() < limits::BATCH_QUIET => c,
                slot => {
                    *slot = Some(BatchState { files: b.files, bytes: b.bytes, done: 0, failed: 0, last: None, started: Instant::now() });
                    slot.as_mut().expect("just set")
                }
            };
            if ok {
                current.done += 1;
                current.last = path;
            } else {
                current.failed += 1;
            }
            current.started = Instant::now();
            current.done + current.failed >= current.files
        };
        if finished {
            self.finish_batch(dev);
        }
        true
    }

    fn finish_batch(self: &Rc<Self>, dev: &Rc<Device>) {
        let Some(batch) = dev.st.borrow_mut().batch.take() else { return };
        let tid = format!("b{}", self.next_id());
        if let Some(last) = &batch.last {
            let mut received = self.received.borrow_mut();
            received.push_back((tid.clone(), last.clone()));
        }
        let body = if batch.failed == 0 {
            trf("{count} files", &[("count", &batch.done.to_string())])
        } else {
            trf("{done} of {count} files; {failed} failed", &[("done", &batch.done.to_string()), ("count", &batch.files.to_string()), ("failed", &batch.failed.to_string())])
        };
        self.raise(Notice {
            key: format!("batch:{}:{tid}", dev.id),
            summary: trf("Received from “{name}”", &[("name", &dev.name())]),
            body,
            icon: Some("document-save-symbolic".into()),
            actions: if batch.last.is_some() { vec![("show".into(), tr("Show in folder"))] } else { Vec::new() },
            ..Notice::default()
        });
    }

    /// A batch that stopped coming is told of as it stands
    pub fn batch_quiet(self: &Rc<Self>, dev: &Rc<Device>) {
        let quiet = dev.st.borrow().batch.as_ref().is_some_and(|b| b.started.elapsed() >= limits::BATCH_QUIET);
        if quiet {
            self.finish_batch(dev);
        }
    }

    /// The person pressed Open or Show on a file that came
    pub fn on_received_event(self: &Rc<Self>, tid: &str, what: &str) {
        let Some(path) = self.received.borrow().iter().find(|(t, _)| t == tid).map(|(_, p)| p.clone()) else { return };
        let uri = gio::File::for_path(&path).uri().to_string();
        match what {
            "open" if share::safe_to_open(&path) => self.open_uri(&uri),
            "show" => {
                let service = self.clone();
                self.spawn_task(async move {
                    let shown = match service.bus.borrow().clone() {
                        Some(bus) => bus
                            .call_future(Some("org.freedesktop.FileManager1"), "/org/freedesktop/FileManager1", "org.freedesktop.FileManager1", "ShowItems", Some(&(vec![uri.clone()], "").to_variant()), None, gio::DBusCallFlags::NONE, 3000)
                            .await
                            .is_ok(),
                        None => false,
                    };
                    if !shown {
                        // No file manager on the bus: the folder opens in whatever the person's default is
                        let folder = gio::File::for_uri(&uri).parent().map(|f| f.uri().to_string());
                        if let Some(folder) = folder {
                            service.open_uri(&folder);
                        }
                    }
                });
            }
            _ => {}
        }
    }

    // --- To a phone -----------------------------------------------------------------------------------------

    pub fn send_files(self: &Rc<Self>, device: &str, uris: &[String]) -> ApiResult<Vec<String>> {
        let dev = self.device(device).ok_or_else(|| ApiError::unavailable("no such device"))?;
        if !dev.paired() || !dev.connected() {
            return Err(ApiError::unavailable("the phone is not paired and connected"));
        }
        if !dev.takes("kdeconnect.share.request") {
            return Err(ApiError::unavailable("the phone does not take files"));
        }
        if uris.is_empty() || uris.len() > 1000 {
            return Err(ApiError::InvalidArgs("no files, or too many".into()));
        }
        let mut files = Vec::new();
        for uri in uris {
            let (path, size) = share::sendable(uri).map_err(ApiError::InvalidArgs)?;
            // Not this program's own certificate, key and list of phones
            if let (Ok(file), Ok(own)) = (path.canonicalize(), self.cfg.config_dir.canonicalize()) {
                if file.starts_with(&own) {
                    return Err(ApiError::InvalidArgs("the program's own files are not sent".into()));
                }
            }
            files.push((path, size));
        }
        let batch = (files.len() > 1).then(|| Batch { files: files.len() as u32, bytes: files.iter().map(|f| f.1).sum() });
        if let Some(b) = batch {
            if dev.takes("kdeconnect.share.request.update") {
                self.send_to(&dev, "kdeconnect.share.request.update", json!({ "numberOfFiles": b.files, "totalPayloadSize": b.bytes }));
            }
        }
        let mut ids = Vec::new();
        let mut previous: Option<futures_channel::oneshot::Receiver<()>> = None;
        for (path, size) in files {
            let tid = format!("t{}", self.next_id());
            ids.push(tid.clone());
            let (done, next) = futures_channel::oneshot::channel::<()>();
            let waits_for = previous.replace(next);
            let name = text::file_name(&path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
            let service = self.clone();
            let work_dev = dev.clone();
            let (work, handle) = abortable({
                let (tid, name) = (tid.clone(), name.clone());
                async move {
                    // One file at a time: the next is announced when the one before is done, or was cancelled
                    if let Some(before) = waits_for {
                        let _ = before.await;
                    }
                    service.send_one(&work_dev, &tid, &name, &path, size, batch).await;
                    let _ = done.send(());
                }
            });
            self.register_transfer(&tid, &dev, handle, &name, false, size);
            let service = self.clone();
            self.spawn_task(async move {
                let _ = work.await;
                service.finish_transfer(&tid);
            });
        }
        Ok(ids)
    }

    async fn send_one(self: &Rc<Self>, dev: &Rc<Device>, tid: &str, name: &str, path: &Path, size: u64, batch: Option<Batch>) {
        let fail = |service: &Rc<Service>, why: String| service.transfer_failed(dev, tid, name, false, size, &why, None);
        let (Some(link), Some(pin)) = (dev.link(), self.pin_of(&dev.id)) else {
            return fail(self, tr("The phone is not connected."));
        };
        let mut file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(_) => return fail(self, tr("The file cannot be read.")),
        };
        let offer = match payload::offer(&self.cfg.payload_ports) {
            Ok(o) => o,
            Err(e) => return fail(self, e.to_string()),
        };
        dbus::transfer(self, dev, tid, name, false, 0, size, "started");
        let modified = std::fs::metadata(path).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64);
        let packet = Packet::new("kdeconnect.share.request", share::file_body(name, modified, batch)).with_payload(size, offer.port);
        // The listener exists before the phone is told of it
        if !link.send(&packet) {
            return fail(self, tr("The phone is not connected."));
        }
        let throttle = Throttle::new();
        let service = self.clone();
        let result = offer
            .serve(&self.me.tls, pin, size, &mut |buffer| file.read(buffer), &|done| {
                if throttle.due() {
                    dbus::transfer(&service, dev, tid, name, false, done, size, "progress");
                }
            })
            .await;
        match result {
            Ok(()) => dbus::transfer(self, dev, tid, name, false, size, size, "finished"),
            Err(e) => fail(self, trf("The transfer failed: {error}", &[("error", &e.to_string())])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_on_the_way_is_set_aside_until_it_is_done() {
        let total = Cell::new(0u64);
        {
            let _a = Reserved::new(&total, 100);
            let _b = Reserved::new(&total, 50);
            assert_eq!(total.get(), 150);
            drop(_a);
            assert_eq!(total.get(), 50);
        }
        assert_eq!(total.get(), 0, "a transfer that is dropped (cancelled) gives its space back");
        // Sizes a phone chose do not overflow anything
        let _huge = Reserved::new(&total, u64::MAX);
        let _more = Reserved::new(&total, u64::MAX);
        assert_eq!(total.get(), u64::MAX);
    }
}
