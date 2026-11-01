// The places on the dock that are not apps: each drive that is plugged in and mounted, and the trash. A Files window
// that shows a place belongs to it rather than to Files, so the place has the window's dot and its click goes back
// to it; Files says which window shows what through the file manager interface it publishes on the session bus.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import {EventEmitter} from 'resource:///org/gnome/shell/misc/signals.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';
import * as ShellMountOperation from 'resource:///org/gnome/shell/ui/shellMountOperation.js';

const FILES = 'org.gnome.Nautilus.desktop';
const FILES_BUS = 'org.gnome.Nautilus';
const TRASH = 'trash:///';
// The trash is looked at again this long after it changed, so that emptying it is one look and not thousands
const TRASH_SETTLE = 1000;

// Whether uri is the place itself or somewhere inside it
const inside = (uri, root) => uri === root || uri.startsWith(root.endsWith('/') ? root : `${root}/`);

export class Places extends EventEmitter {
    start() {
        this._cancellable = new Gio.Cancellable();
        this._mounts = [];
        this._locations = {};
        this.trash = {
            uri: TRASH,
            name: _('Trash'),
            gicon: Gio.ThemedIcon.new('user-trash'),
            fallback: 'user-trash',
            full: false,
        };
        this._watchTrash();

        this._volumes = Gio.VolumeMonitor.get();
        this._volumes.connectObject(
            'mount-added', (monitor, mount) => this._addMount(mount),
            'mount-removed', (monitor, mount) => this._removeMount(mount),
            'mount-changed', (monitor, mount) => this._changeMount(mount),
            this);
        for (const mount of this._volumes.get_mounts())
            this._addMount(mount);

        // Only while Files runs; asking does not start it
        Gio.DBusProxy.new_for_bus(Gio.BusType.SESSION, Gio.DBusProxyFlags.DO_NOT_AUTO_START, null,
            'org.freedesktop.FileManager1', '/org/freedesktop/FileManager1', 'org.freedesktop.FileManager1',
            this._cancellable, (object, result) => {
                try {
                    this._fileManager = Gio.DBusProxy.new_for_bus_finish(result);
                } catch (e) {
                    if (!e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        console.warn(`Amethystora UI: the dock cannot ask Files what it shows: ${e.message}`);
                    return;
                }
                this._fileManager.connectObject('g-properties-changed', () => this._readLocations(), this);
                this._readLocations();
            });

        const apps = Shell.AppSystem.get_default();
        apps.connectObject('installed-changed', () => this._watchFiles(), this);
        this._watchFiles();
    }

    stop() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._volumes?.disconnectObject(this);
        this._volumes = null;
        this._fileManager?.disconnectObject(this);
        this._fileManager = null;
        this._files?.disconnectObject(this);
        this._files = null;
        Shell.AppSystem.get_default().disconnectObject(this);
        this._trashMonitor?.cancel();
        this._trashMonitor = null;
        if (this._trashId)
            GLib.source_remove(this._trashId);
        this._trashId = 0;
        this._mounts = [];
        this._locations = {};
    }

    // Drives in the order they appeared, then the trash
    list() {
        return [...this._mounts, this.trash];
    }

    // The place a window shows, or null
    owner(window) {
        const uris = this._locations[window.get_gtk_window_object_path?.() ?? ''];
        if (!uris)
            return null;
        return this.list().find(place => uris.some(uri => inside(uri, place.uri))) ?? null;
    }

    windowsOf(place) {
        const windows = this._files?.get_windows() ?? [];
        return windows.filter(window => !window.skip_taskbar && this.owner(window) === place);
    }

    // In the program that opens it, which finds a Files window already showing it
    open(place) {
        try {
            Gio.AppInfo.launch_default_for_uri(place.uri, global.create_app_launch_context(0, -1));
        } catch (e) {
            Main.notifyError(_('%s could not be opened').format(place.name), e.message);
        }
    }

    // A new Files window, even when one shows the place already
    openNew(place) {
        const app = Shell.AppSystem.get_default().lookup_app(FILES);
        const platform = {};
        if (app) {
            const context = global.create_app_launch_context(0, -1);
            const token = context.get_startup_notify_id(app.get_app_info(), []);
            if (token)
                platform['activation-token'] = new GLib.Variant('s', token);
        }
        Gio.DBus.session.call(FILES_BUS, '/org/gnome/Nautilus', 'org.gtk.Application', 'Open',
            new GLib.Variant('(assa{sv})', [[place.uri], 'new-window', platform]), null,
            Gio.DBusCallFlags.NONE, -1, null, (connection, result) => {
                try {
                    connection.call_finish(result);
                } catch (e) {
                    Main.notifyError(_('%s could not be opened').format(place.name), e.message);
                }
            });
    }

    // What the place's menu offers beside its windows: [{label, run()}]
    actions(place) {
        if (place === this.trash)
            return place.full ? [{label: _('Empty trash'), run: () => this._emptyTrash()}] : [];
        const actions = [];
        if (place.mount.can_unmount())
            actions.push({label: _('Unmount'), run: () => this._unmount(place, false)});
        if (place.mount.can_eject())
            actions.push({label: _('Eject'), run: () => this._unmount(place, true)});
        return actions;
    }

    _watchFiles() {
        const files = Shell.AppSystem.get_default().lookup_app(FILES);
        if (files === this._files)
            return;
        this._files?.disconnectObject(this);
        this._files = files;
        this._files?.connectObject('windows-changed', () => this.emit('windows-changed'), this);
    }

    _readLocations() {
        const value = this._fileManager?.get_cached_property('OpenWindowsWithLocations');
        try {
            this._locations = value?.deepUnpack() ?? {};
        } catch {
            this._locations = {};
        }
        this.emit('windows-changed');
    }

    _watchTrash() {
        try {
            this._trashMonitor = Gio.File.new_for_uri(TRASH).monitor_directory(Gio.FileMonitorFlags.NONE,
                this._cancellable);
            this._trashMonitor.connect('changed', () => this._queueTrash());
        } catch (e) {
            // No trash to watch here (no GVfs): its plain icon stays
            console.warn(`Amethystora UI: the trash cannot be watched: ${e.message}`);
        }
        this._readTrash();
    }

    _queueTrash() {
        if (this._trashId)
            return;
        this._trashId = GLib.timeout_add(GLib.PRIORITY_LOW, TRASH_SETTLE, () => {
            this._trashId = 0;
            this._readTrash();
            return GLib.SOURCE_REMOVE;
        });
    }

    _readTrash() {
        Gio.File.new_for_uri(TRASH).query_info_async('standard::icon,trash::item-count',
            Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_LOW, this._cancellable, (file, result) => {
                let info;
                try {
                    info = file.query_info_finish(result);
                } catch {
                    return;
                }
                this.trash.full = info.get_attribute_uint32('trash::item-count') > 0;
                this.trash.gicon = info.get_icon() ??
                    Gio.ThemedIcon.new(this.trash.full ? 'user-trash-full' : 'user-trash');
                this.emit('place-changed', this.trash);
            });
    }

    // A drive the person can take away, as GIO's volumes list them: a stick, a disk, a phone or a camera on its cable
    // (MTP, PTP, an iPhone's AFC), but not a network share, not one hidden behind another, and not the system's own,
    // which has no volume
    _wanted(mount) {
        const volume = mount.get_volume();
        return Boolean(volume) && !mount.is_shadowed() && (mount.can_unmount() || mount.can_eject()) &&
            volume.get_identifier(Gio.VOLUME_IDENTIFIER_KIND_CLASS) !== 'network';
    }

    _addMount(mount) {
        if (this._mounts.some(place => place.mount === mount) || !this._wanted(mount))
            return;
        const place = {mount, fallback: 'media-removable'};
        this._describe(place);
        this._mounts.push(place);
        this.emit('places-changed');
    }

    _removeMount(mount) {
        const count = this._mounts.length;
        this._mounts = this._mounts.filter(place => place.mount !== mount);
        if (this._mounts.length !== count)
            this.emit('places-changed');
    }

    _changeMount(mount) {
        const place = this._mounts.find(p => p.mount === mount);
        if (!place) {
            this._addMount(mount);
            return;
        }
        if (!this._wanted(mount)) {
            this._removeMount(mount);
            return;
        }
        this._describe(place);
        this.emit('place-changed', place);
    }

    _describe(place) {
        place.uri = place.mount.get_root().get_uri();
        place.name = place.mount.get_name();
        place.gicon = place.mount.get_icon() ?? Gio.ThemedIcon.new('media-removable');
    }

    _emptyTrash() {
        // Files asks first, in its own dialog
        const platform = {timestamp: new GLib.Variant('u', global.get_current_time())};
        Gio.DBus.session.call(FILES_BUS, '/org/gnome/Nautilus/FileOperations2', 'org.gnome.Nautilus.FileOperations2',
            'EmptyTrash', new GLib.Variant('(ba{sv})', [true, platform]), null, Gio.DBusCallFlags.NONE, -1, null,
            (connection, result) => {
                try {
                    connection.call_finish(result);
                } catch (e) {
                    Main.notifyError(_('The trash could not be emptied'), e.message);
                }
            });
    }

    // Never forced: a forced unmount skips the check for programs still using the drive, and with it GNOME's own
    // dialog that names them, so a copy under way would be cut off with the drive reported safe to pull
    _unmount(place, eject) {
        if (place.busy) {
            Main.osdWindowManager.showAll(Gio.ThemedIcon.new('media-eject-symbolic'), place.busy === 'eject'
                ? _('%s is already being ejected').format(place.name)
                : _('%s is already being unmounted').format(place.name));
            return;
        }
        place.busy = eject ? 'eject' : 'unmount';
        const {mount} = place;
        const operation = new ShellMountOperation.ShellMountOperation(mount);
        const done = (object, result) => {
            place.busy = null;
            operation.close();
            try {
                if (eject)
                    mount.eject_with_operation_finish(result);
                else
                    mount.unmount_with_operation_finish(result);
            } catch (e) {
                if (e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.BUSY) && this._busyNotice(place, mount, eject))
                    return;
                if (!e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.FAILED_HANDLED)) {
                    Main.notifyError(eject
                        ? _('%s could not be ejected').format(place.name)
                        : _('%s could not be unmounted').format(place.name), e.message);
                }
            }
        };
        if (eject)
            mount.eject_with_operation(Gio.MountUnmountFlags.NONE, operation.mountOp, null, done);
        else
            mount.unmount_with_operation(Gio.MountUnmountFlags.NONE, operation.mountOp, null, done);
    }

    // A drive kept busy: the notice offers to show what keeps it so (the tools module, plans/ui-ideas/spec-tools.md
    // R5-18); false when that module is not running, and the plain error is said instead
    _busyNotice(place, mount, eject) {
        const tools = Main.extensionManager.lookup('amethystora-ui@arsslensoft.github.io')?.stateObj?.modules?.get('tools');
        if (!tools?.showUsersOfMount)
            return false;
        const source = MessageTray.getSystemSource();
        const notification = new MessageTray.Notification({
            source,
            title: eject ? _('Cannot eject %s').format(place.name) : _('Cannot unmount %s').format(place.name),
            body: _('It is busy.'),
            iconName: 'media-eject-symbolic',
            isTransient: true,
        });
        notification.addAction(_('Show what'), () => tools.showUsersOfMount(mount));
        source.addNotification(notification);
        return true;
    }
}
