//! Kept in step with the machine: an app is told, within a moment, that a file it reads changed or that a unit
//! was enabled, disabled or masked, whatever changed it (a terminal, another app, the agent), so that what a
//! window shows never waits for a refresh or for the window to be focused again

use crate::{gio, glib};
use gio::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

/// How long a burst of changes is let settle before the app reads again: a switch writes several files at once
const SETTLE: Duration = Duration::from_millis(300);

/// Which systemd managers' unit files the app follows
#[derive(Clone, Copy, Default)]
pub struct Units {
    pub system: bool,
    pub user: bool,
}

/// What `watch` keeps listening for as long as the app holds it
pub struct Live {
    monitors: RefCell<Vec<gio::FileMonitor>>,
    subscriptions: RefCell<Vec<(gio::DBusConnection, gio::SignalSubscriptionId)>>,
    pending: RefCell<Option<glib::SourceId>>,
    changed: Box<dyn Fn()>,
}

impl Live {
    /// A change heard: `changed` runs once the burst it belongs to has settled
    fn heard(self: &Rc<Self>) {
        if let Some(pending) = self.pending.take() {
            pending.remove();
        }
        let weak = Rc::downgrade(self);
        let source = glib::timeout_add_local_once(SETTLE, move || {
            let Some(this) = weak.upgrade() else { return };
            this.pending.take();
            (this.changed)();
        });
        self.pending.replace(Some(source));
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        if let Some(pending) = self.pending.take() {
            pending.remove();
        }
        for monitor in self.monitors.take() {
            monitor.cancel();
        }
        #[allow(deprecated)]
        for (bus, id) in self.subscriptions.take() {
            bus.signal_unsubscribe(id);
        }
    }
}

/// Follows `paths` (a folder for any file in it, a file whether or not it exists yet) and the unit files of the
/// managers `units` names, and runs `changed` after each settled burst of changes. Reading a file never counts.
pub fn watch(paths: Vec<PathBuf>, units: Units, changed: impl Fn() + 'static) -> Rc<Live> {
    let live = Rc::new(Live {
        monitors: RefCell::new(Vec::new()),
        subscriptions: RefCell::new(Vec::new()),
        pending: RefCell::new(None),
        changed: Box::new(changed),
    });
    for path in paths {
        let file = gio::File::for_path(&path);
        let monitor = if path.is_dir() {
            file.monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        } else {
            file.monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        };
        let Ok(monitor) = monitor else { continue };
        let weak = Rc::downgrade(&live);
        monitor.connect_changed(move |_, _, _, event| {
            if matches!(event, gio::FileMonitorEvent::AttributeChanged | gio::FileMonitorEvent::ChangesDoneHint) {
                return;
            }
            if let Some(live) = weak.upgrade() {
                live.heard();
            }
        });
        live.monitors.borrow_mut().push(monitor);
    }
    for (wanted, kind) in [(units.system, gio::BusType::System), (units.user, gio::BusType::Session)] {
        if !wanted {
            continue;
        }
        let weak = Rc::downgrade(&live);
        glib::spawn_future_local(async move {
            let Ok(bus) = gio::bus_get_future(kind).await else { return };
            // systemd sends its manager's signals only once somebody has asked it to
            let _ = bus
                .call_future(
                    Some("org.freedesktop.systemd1"),
                    "/org/freedesktop/systemd1",
                    "org.freedesktop.systemd1.Manager",
                    "Subscribe",
                    None,
                    None,
                    gio::DBusCallFlags::NONE,
                    -1,
                )
                .await;
            let Some(live) = weak.upgrade() else { return };
            for signal in ["UnitFilesChanged", "Reloading"] {
                let heard = Rc::downgrade(&live);
                #[allow(deprecated)] // the stream form would need the app to poll it for a callback this small
                let id = bus.signal_subscribe(
                    Some("org.freedesktop.systemd1"),
                    Some("org.freedesktop.systemd1.Manager"),
                    Some(signal),
                    Some("/org/freedesktop/systemd1"),
                    None,
                    gio::DBusSignalFlags::NONE,
                    move |_, _, _, _, _, _| {
                        if let Some(live) = heard.upgrade() {
                            live.heard();
                        }
                    },
                );
                live.subscriptions.borrow_mut().push((bus.clone(), id));
            }
        });
    }
    live
}

/// The account's home, for the paths an app follows there
pub fn home(path: &str) -> PathBuf {
    glib::home_dir().join(path)
}
