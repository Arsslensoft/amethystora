// What apps say about themselves on their dock icon, through the launcher protocol many of them speak
// (com.canonical.Unity.LauncherEntry): a count of unread items, a progress bar, a call for attention, that an update
// is under way, and a short menu of their own. An app only says it when somebody owns the protocol's bus name, so
// the dock owns it while it runs. What an app said is forgotten when it leaves the bus. While notifications are
// silenced (Do Not Disturb) none of it shows, anywhere: no count, no bar, no dimming for an update, no call for
// attention and no menu of its own, as if the app had said nothing.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';

import {EventEmitter} from 'resource:///org/gnome/shell/misc/signals.js';

const BUS_NAME = 'com.canonical.Unity';
const INTERFACE = 'com.canonical.Unity.LauncherEntry';
const SCHEME = 'application://';
// A launcher ID is a desktop file's name; anything else is not looked up
const LAUNCHER_ID = /^[\w.+-]+\.desktop$/;
// The most apps that are remembered at once, apart from needing to be ones the system knows
const MAX_ENTRIES = 128;

// The protocol's keys, and what each is called here
const KEYS = {
    'count': 'count',
    'count-visible': 'countVisible',
    'progress': 'progress',
    'progress-visible': 'progressVisible',
    'urgent': 'urgent',
    'updating': 'updating',
    'quicklist': 'quicklist',
};

export class LauncherEntries extends EventEmitter {
    start() {
        // By the launcher's ID, without the protocol's scheme
        this._entries = new Map();
        this._watches = new Map();
        // Silencing notifications, or letting them through again, changes what every app shows (null: every app)
        this._banners = new Gio.Settings({schema_id: 'org.gnome.desktop.notifications'});
        this._banners.connectObject('changed::show-banners', () => this.emit('changed', null), this);
        this._owner = Gio.bus_own_name(Gio.BusType.SESSION, BUS_NAME,
            Gio.BusNameOwnerFlags.ALLOW_REPLACEMENT | Gio.BusNameOwnerFlags.REPLACE, null, null, null);
        this._subscription = Gio.DBus.session.signal_subscribe(null, INTERFACE, 'Update', null, null,
            Gio.DBusSignalFlags.NONE, (connection, sender, path, iface, signal, parameters) =>
                this._update(sender, parameters));

        // The apps' own menus need libdbusmenu's bindings; without them the menus are left out and nothing else
        this._running = true;
        import('gi://Dbusmenu?version=0.4').then(module => {
            if (this._running)
                this._menus = module.default;
        }).catch(() => {});
    }

    stop() {
        this._running = false;
        this._banners?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._banners?.run_dispose();
        this._banners = null;
        if (this._subscription)
            Gio.DBus.session.signal_unsubscribe(this._subscription);
        this._subscription = 0;
        if (this._owner)
            Gio.bus_unown_name(this._owner);
        this._owner = 0;
        for (const id of this._watches?.values() ?? [])
            Gio.bus_unwatch_name(id);
        this._watches = null;
        for (const entry of this._entries?.values() ?? [])
            this._dropMenu(entry);
        this._entries = null;
        this._menus = null;
    }

    // Whether notifications are silenced
    get quiet() {
        return this._banners ? !this._banners.get_boolean('show-banners') : false;
    }

    // What the app with this launcher ID last said, or null
    get(id) {
        if (this.quiet)
            return null;
        const entry = this._entries?.get(id) ?? null;
        // Grey only: unlike an app saying it is updating, this does not stop it starting (dock/flatpak.js)
        return this.updates?.has(id) ? {...entry, greyed: true} : entry;
    }

    // The app's own menu entries as they are now: [{label, enabled, separator, activate()}]
    quicklist(id) {
        const root = this.get(id)?.client?.get_root();
        if (!root)
            return [];
        const items = [];
        for (const child of root.get_children()) {
            if (!child.property_get_bool('visible'))
                continue;
            if (child.property_get('type') === 'separator') {
                items.push({separator: true});
                continue;
            }
            items.push({
                // A mnemonic's underscore marks a key the dock does not offer
                label: (child.property_get('label') ?? '').replace(/_(.)/g, '$1'),
                enabled: child.property_get_bool('enabled'),
                activate: time => child.handle_event('clicked', GLib.Variant.new_int32(0), time),
            });
        }
        return items;
    }

    _update(sender, parameters) {
        if (!this._entries)
            return;
        let uri, properties;
        try {
            [uri, properties] = parameters.recursiveUnpack();
        } catch {
            return;
        }
        if (typeof uri !== 'string')
            return;
        const id = uri.startsWith(SCHEME) ? uri.slice(SCHEME.length) : uri;
        // Any program on the bus can say this about any launcher: only the apps this machine has get an entry, and
        // only so many
        let entry = this._entries.get(id);
        if (!entry) {
            if (this._entries.size >= MAX_ENTRIES || !LAUNCHER_ID.test(id) ||
                !Shell.AppSystem.get_default().lookup_app(id))
                return;
            entry = {};
        }
        let changed = entry.sender !== sender;
        entry.sender = sender;
        for (const [key, name] of Object.entries(KEYS)) {
            if (key in properties && entry[name] !== properties[key]) {
                entry[name] = properties[key];
                changed = true;
            }
        }
        this._syncMenu(entry);
        this._entries.set(id, entry);
        this._watch(sender);
        // The dock redraws every icon for a change, so one that says what it said before costs nothing
        if (changed)
            this.emit('changed', id);
    }

    _syncMenu(entry) {
        const path = this._menus && typeof entry.quicklist === 'string' && entry.quicklist.startsWith('/')
            ? entry.quicklist : null;
        if (entry.client && entry.clientPath === path && entry.clientSender === entry.sender)
            return;
        this._dropMenu(entry);
        if (!path)
            return;
        entry.client = this._menus.Client.new(entry.sender, path);
        entry.clientPath = path;
        entry.clientSender = entry.sender;
    }

    _dropMenu(entry) {
        // The client watches the bus until it is disposed, which the garbage collector may do much later
        entry.client?.run_dispose();
        entry.client = null;
    }

    _watch(sender) {
        if (this._watches.has(sender))
            return;
        this._watches.set(sender, Gio.bus_watch_name_on_connection(Gio.DBus.session, sender,
            Gio.BusNameWatcherFlags.NONE, null, () => this._vanished(sender)));
    }

    _vanished(sender) {
        const id = this._watches?.get(sender);
        if (!id)
            return;
        this._watches.delete(sender);
        Gio.bus_unwatch_name(id);
        for (const [appId, entry] of [...this._entries]) {
            if (entry.sender !== sender)
                continue;
            this._dropMenu(entry);
            this._entries.delete(appId);
            this.emit('changed', appId);
        }
    }
}
