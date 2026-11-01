// One program's tray item, as it reads over D-Bus: its properties, which it changes by sending a signal that names
// what changed, and the calls a click or a scroll makes. It notices when its program leaves the bus, comes back, or
// takes the object away while staying connected, and says so; the tray decides what to do about it.
//
// Signals: 'changed' (a Set of what changed: icon, attention, overlay, title, status, menu, label, app, owner) and
// 'gone' (nothing answers for the item any more).

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';

import {EventEmitter} from 'resource:///org/gnome/shell/misc/signals.js';

import {
    ITEM_IFACE, busCall, call, clearSources, isCancelled, isGone, isUnknownMethod, remoteError, sleep,
} from './dbus.js';
import {hasUsablePixmap} from './icons.js';

const PROPERTIES = 'org.freedesktop.DBus.Properties';

// What each kind of change re-reads. The theme path goes with every icon, since it says where their names are found.
const GROUPS = {
    icon: ['IconName', 'IconPixmap', 'IconAccessibleDesc', 'IconThemePath'],
    attention: ['AttentionIconName', 'AttentionIconPixmap', 'AttentionAccessibleDesc', 'IconThemePath'],
    overlay: ['OverlayIconName', 'OverlayIconPixmap', 'IconThemePath'],
    title: ['Title'],
    status: ['Status'],
    menu: ['Menu'],
    label: ['XAyatanaLabel', 'XAyatanaLabelGuide'],
};
const ICON_GROUPS = ['icon', 'attention', 'overlay'];

// What of an item is kept. The properties of GROUPS and Id are read, as text (paths and object paths are text here
// too) or, for the pixmaps, as a(iiay); anything else the item sends, and any value of another type, is left out as
// if the item did not have it, so that nothing the tray does later has to ask what a property holds.
const PIXMAPS = new Set(['IconPixmap', 'AttentionIconPixmap', 'OverlayIconPixmap']);
const TEXTS = new Set([...Object.values(GROUPS).flat().filter(name => !PIXMAPS.has(name)), 'Id']);
// The longest text kept, in characters: a path is no longer, and a label that was is not going to be read
const TEXT_LENGTH = 4096;

function text(value) {
    return value.slice(0, TEXT_LENGTH);
}

// The item's signals and the group each names. NewToolTip is not here: the top bar shows no tooltips.
const SIGNALS = {
    NewIcon: 'icon',
    NewAttentionIcon: 'attention',
    NewOverlayIcon: 'overlay',
    NewTitle: 'title',
    NewStatus: 'status',
    NewMenu: 'menu',
    NewIconThemePath: 'theme-path',
    XAyatanaNewLabel: 'label',
};

// What a Passive item still reads at once; anything else waits until it is shown again
const WHILE_PASSIVE = ['status', 'menu'];

// Milliseconds: signals that come together are read together, and the panel hears of changes no oftener than this
const GATHER = 30;
const BATCH = 60;
// How long an item whose program left the bus keeps its button, for a program that restarts at once
const OWNER_GRACE = 500;
// How long after a change of visibility, or a failed read, the item is asked whether it is still there
const ALIVE_DELAY = 10000;
// A returning program is asked again this many times, a second apart, for an item it has not set up yet
const RETRIES = 3;
const RETRY_DELAY = 1000;

// A property the item does not have is no sign that the item is gone
const NOT_THERE_YET = ['org.freedesktop.DBus.Error.UnknownProperty'];

// The methods worth knowing about before calling them
const METHODS = ['Activate', 'SecondaryActivate', 'XAyatanaSecondaryActivate', 'ProvideXdgActivationToken'];

export class Item extends EventEmitter {
    /**
     * @param {object} target - where the item is
     * @param {string} target.bus - the bus name it is called on, unique or well-known
     * @param {string} target.path - its object path
     * @param {string} target.service - what it is listed as in the watcher
     */
    constructor({bus, path, service}) {
        super();
        this.bus = bus;
        this.path = path;
        this.service = service;
        // The unique name behind the item while it has one
        this.owner = null;
        this.props = {};
        // true or false once known, undefined until then (a call is tried)
        this.methods = {};
        this.appName = null;

        this._cancellable = new Gio.Cancellable();
        this._sources = new Set();
        this._toRead = new Set();
        this._changed = new Set();
        this._noted = new Set();
        this._dropped = new Set();
        this._lastBatch = 0;
        this._gatherId = 0;
        this._graceId = 0;
        this._aliveId = 0;
        this._destroyed = false;
    }

    /**
     * Reads the item once and starts following it. Throws when the item cannot be read.
     *
     * @param {string} owner - the unique name behind it now
     */
    async init(owner) {
        this.owner = owner;
        await this._readAll();
        if (this._destroyed)
            return;
        this._signalId = Gio.DBus.session.signal_subscribe(this.bus, ITEM_IFACE, null, this.path, null,
            Gio.DBusSignalFlags.NONE, (c, s, p, i, signal, params) => this._onSignal(signal, params));
        this._watchId = Gio.bus_watch_name_on_connection(Gio.DBus.session, this.bus,
            Gio.BusNameWatcherFlags.NONE, (c, n, newOwner) => this._ownerAppeared(newOwner),
            () => this._ownerVanished());
        Shell.AppSystem.get_default().connectObject(
            'installed-changed', () => this._updateApp(),
            'app-state-changed', () => this._updateApp(), this);
        this._introspect();
        this._findApp();
    }

    destroy() {
        this._destroyed = true;
        this._cancellable.cancel();
        if (this._signalId)
            Gio.DBus.session.signal_unsubscribe(this._signalId);
        this._signalId = 0;
        if (this._watchId)
            Gio.bus_unwatch_name(this._watchId);
        this._watchId = 0;
        Shell.AppSystem.get_default().disconnectObject(this);
        for (const id of [this._gatherId, this._graceId, this._aliveId]) {
            if (id)
                GLib.source_remove(id);
        }
        this._gatherId = this._graceId = this._aliveId = 0;
        clearSources(this._sources);
    }

    get needsAttention() {
        return this.props.Status?.toLowerCase?.() === 'needsattention';
    }

    get passive() {
        return this.props.Status?.toLowerCase?.() === 'passive';
    }

    // A program that registers the same item again: what it shows is read again, its icons too
    reregister() {
        for (const group of ['status', 'label', ...ICON_GROUPS])
            this._toRead.add(group);
        this._schedule();
    }

    // The icons again, from the bus, for a change the item did not make (the scale, the icon theme)
    reloadIcons() {
        for (const group of ICON_GROUPS)
            this._toRead.add(group);
        this._schedule();
    }

    /**
     * A pixmap property, read again if its data was let go after drawing.
     *
     * @param {string} name - IconPixmap, AttentionIconPixmap or OverlayIconPixmap
     * @returns {Promise<GLib.Variant|null>}
     */
    async pixmap(name) {
        if (this._dropped.has(name)) {
            this._dropped.delete(name);
            await this._read([name]);
        }
        return this.props[name] ?? null;
    }

    // Lets the data of a drawn pixmap go; pixmap() reads it again if it is needed
    dropPixmap(name) {
        if (!this.props[name])
            return;
        this.props[name] = null;
        this._dropped.add(name);
    }

    // Asks the item whether it is still there, once things have had time to settle
    checkAlive() {
        if (this._aliveId || this._destroyed)
            return;
        this._aliveId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ALIVE_DELAY, () => {
            this._aliveId = 0;
            this._getProperty('Status').then(variant => {
                const before = this.props.Status;
                this._store('Status', variant);
                if (this.props.Status !== before)
                    this._emitChanged(new Set(['status']));
            }).catch(e => {
                if (isGone(e))
                    this.emit('gone');
            });
            return GLib.SOURCE_REMOVE;
        });
    }

    // KDE's addition: a token that lets the program raise its window under Wayland, for the click at `time`
    provideToken(time) {
        if (this.methods.ProvideXdgActivationToken === false)
            return;
        let token;
        try {
            token = global.create_app_launch_context(time, -1).get_startup_notify_id(null, []);
        } catch {
            return;
        }
        if (!token)
            return;
        this._callItem('ProvideXdgActivationToken', new GLib.Variant('(s)', [token]));
    }

    activate(x, y, time) {
        this.provideToken(time);
        this._callItem('Activate', new GLib.Variant('(ii)', [Math.trunc(x), Math.trunc(y)]));
    }

    secondaryActivate(x, y, time) {
        this.provideToken(time);
        if (this.methods.XAyatanaSecondaryActivate) {
            this._callItem('XAyatanaSecondaryActivate', new GLib.Variant('(u)', [time]),
                () => this.secondaryActivate(x, y, time));
        } else {
            this._callItem('SecondaryActivate', new GLib.Variant('(ii)', [Math.trunc(x), Math.trunc(y)]));
        }
    }

    scroll(delta, orientation) {
        this._callItem('Scroll', new GLib.Variant('(is)', [delta, orientation]));
    }

    // A method the item may not have: an UnknownMethod answer says so for next time, and `otherwise` runs
    _callItem(method, params, otherwise = null) {
        call(this.bus, this.path, ITEM_IFACE, method, params, null, this._cancellable).catch(e => {
            if (isUnknownMethod(e)) {
                this.methods[method] = false;
                otherwise?.();
            }
        });
    }

    async _readAll() {
        const reply = await call(this.bus, this.path, PROPERTIES, 'GetAll', new GLib.Variant('(s)', [ITEM_IFACE]),
            '(a{sv})', this._cancellable);
        const dict = reply.get_child_value(0);
        this.props = {};
        this._dropped.clear();
        for (let i = 0; i < dict.n_children(); i++) {
            const entry = dict.get_child_value(i);
            const name = entry.get_child_value(0).get_string()[0];
            // The top bar shows no tooltips, and a tooltip can carry pictures of its own
            if (name !== 'ToolTip')
                this._store(name, entry.get_child_value(1).get_variant());
        }
    }

    _store(name, variant) {
        const type = variant.get_type_string();
        if (PIXMAPS.has(name))
            this.props[name] = type === 'a(iiay)' && hasUsablePixmap(variant) ? variant : null;
        else if (TEXTS.has(name) && (type === 's' || type === 'o'))
            this.props[name] = text(variant.get_string()[0]);
        else if (TEXTS.has(name))
            delete this.props[name];
    }

    _getProperty(name) {
        return call(this.bus, this.path, PROPERTIES, 'Get', new GLib.Variant('(ss)', [ITEM_IFACE, name]), '(v)',
            this._cancellable).then(reply => reply.get_child_value(0).get_variant());
    }

    // Reads properties one by one, since an item may lack any of them; one it lacks is taken as unset. An answer
    // that the object, or its program, is not there any more has the item checked.
    async _read(names) {
        await Promise.all(names.map(async name => {
            try {
                this._store(name, await this._getProperty(name));
            } catch (e) {
                if (isCancelled(e))
                    throw e;
                delete this.props[name];
                if (name === 'Status' || (isGone(e) && !NOT_THERE_YET.includes(remoteError(e))))
                    this.checkAlive();
            }
        }));
    }

    _onSignal(signal, params) {
        const group = SIGNALS[signal];
        if (!group)
            return;

        // The signals that carry their value need no read
        if (signal === 'NewStatus' && params.get_type_string() === '(s)') {
            const wasPassive = this.passive;
            this.props.Status = text(params.get_child_value(0).get_string()[0]);
            this._changed.add('status');
            if (wasPassive && !this.passive)
                this._unnote();
        } else if (signal === 'NewIconThemePath' && params.get_type_string() === '(s)') {
            this.props.IconThemePath = text(params.get_child_value(0).get_string()[0]);
            this._note(ICON_GROUPS, this._changed);
        } else if (signal === 'XAyatanaNewLabel' && params.get_type_string() === '(ss)') {
            this.props.XAyatanaLabel = text(params.get_child_value(0).get_string()[0]);
            this.props.XAyatanaLabelGuide = text(params.get_child_value(1).get_string()[0]);
            this._note(['label'], this._changed);
        } else if (group === 'theme-path') {
            this._note(ICON_GROUPS, this._toRead);
        } else {
            this._note([group], this._toRead);
        }
        this._schedule();
    }

    // While the item is Passive, what is not shown is only noted and read once the item is shown again
    _note(groups, into) {
        for (const group of groups) {
            if (this.passive && !WHILE_PASSIVE.includes(group))
                this._noted.add(group);
            else
                into.add(group);
        }
    }

    _unnote() {
        for (const group of this._noted)
            this._toRead.add(group);
        this._noted.clear();
    }

    _schedule() {
        if (this._gatherId || this._destroyed)
            return;
        const sinceLast = GLib.get_monotonic_time() / 1000 - this._lastBatch;
        this._gatherId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, Math.max(GATHER, BATCH - sinceLast), () => {
            this._gatherId = 0;
            this._flush().catch(e => {
                if (!isCancelled(e))
                    console.warn(`Amethystora UI: a tray item could not be read: ${e.message}`);
            });
            return GLib.SOURCE_REMOVE;
        });
    }

    async _flush() {
        const groups = new Set([...this._toRead, ...this._changed]);
        const names = new Set([...this._toRead].flatMap(group => GROUPS[group]));
        this._toRead.clear();
        this._changed.clear();
        if (names.size) {
            const wasPassive = this.passive;
            await this._read([...names]);
            // A status read while the item was hidden can show it again
            if (wasPassive && !this.passive) {
                this._unnote();
                this._schedule();
            }
        }
        if (this._destroyed)
            return;
        this._lastBatch = GLib.get_monotonic_time() / 1000;
        this._emitChanged(groups);
    }

    _emitChanged(groups) {
        if (groups.size && !this._destroyed)
            this.emit('changed', groups);
    }

    async _introspect() {
        try {
            const reply = await call(this.bus, this.path, 'org.freedesktop.DBus.Introspectable', 'Introspect', null,
                '(s)', this._cancellable);
            const info = Gio.DBusNodeInfo.new_for_xml(reply.get_child_value(0).get_string()[0]);
            const iface = info.lookup_interface(ITEM_IFACE);
            if (!iface)
                return;
            for (const method of METHODS)
                this.methods[method] = iface.lookup_method(method) !== null;
        } catch {
            // An item that cannot be introspected is called all the same, and learnt from its answers
        }
    }

    // The app the item's program is, for the button's accessible name
    async _findApp() {
        try {
            const reply = await busCall('GetConnectionUnixProcessID', new GLib.Variant('(s)', [this.bus]), '(u)',
                this._cancellable);
            this._pid = reply.get_child_value(0).get_uint32();
        } catch {
            this._pid = 0;
        }
        this._updateApp();
    }

    _updateApp() {
        if (this._destroyed)
            return;
        const app = this._pid ? Shell.WindowTracker.get_default().get_app_from_pid(this._pid) : null;
        const name = app?.get_name() ?? null;
        if (name !== this.appName) {
            this.appName = name;
            this._emitChanged(new Set(['app']));
        }
    }

    _ownerVanished() {
        this.owner = null;
        if (this._graceId || this._destroyed)
            return;
        this._graceId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, OWNER_GRACE, () => {
            this._graceId = 0;
            if (!this.owner)
                this.emit('gone');
            return GLib.SOURCE_REMOVE;
        });
    }

    async _ownerAppeared(owner) {
        if (this._graceId)
            GLib.source_remove(this._graceId);
        this._graceId = 0;
        if (owner === this.owner)
            return;

        // The same name, another program: everything about the item is read again
        this.owner = owner;
        this.methods = {};
        if (!await this._readReturning(owner, RETRIES))
            return;
        this._introspect();
        this._findApp();
        this._emitChanged(new Set(['owner', 'status', 'title', 'menu', 'label', ...ICON_GROUPS]));
        this.checkAlive();
    }

    // Reads a returning program's item, and again a second later while it has not set it up, `retries` times at
    // most. False when the item was given up meanwhile.
    async _readReturning(owner, retries) {
        let ready = false;
        try {
            await this._readAll();
            ready = ['Id', 'Menu', 'Status'].every(name => this.props[name] !== undefined);
        } catch (e) {
            if (isCancelled(e))
                return false;
        }
        if (this._destroyed || this.owner !== owner)
            return false;
        if (ready || retries === 0)
            return true;
        await sleep(this._sources, RETRY_DELAY);
        return this._readReturning(owner, retries - 1);
    }
}
