// The shell as the session's tray: the StatusNotifierWatcher that programs look for on the session bus, and the only
// host it knows. A program registers its item in any of the forms programs send; the call is answered once the
// item's button is in the top bar. When another program holds the name already, the shell waits its turn behind it.
// Taking the tray away tells every program so and lets the name go, so that the next tray finds the items again.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {ITEM_PATH, WATCHER_NAME, WATCHER_PATH, WATCHER_XML, isCancelled, nameOwner, remoteError} from './dbus.js';
import {Item} from './item.js';

// The prefix some programs give their item's own bus name and match on, so that it is called by that name
const FREEDESKTOP_PREFIX = 'org.freedesktop.StatusNotifierItem-';
const REFUSED = 'org.freedesktop.DBus.Error.InvalidArgs';
// A program that registers more items than this, or all programs together, is answered with LIMITS: each item has
// a button, a menu, a signal subscription and a name watch, and every path of one connection is an item of its own
const LIMITS = 'org.freedesktop.DBus.Error.LimitsExceeded';
const MAX_PER_PROGRAM = 16;
const MAX_ITEMS = 64;

/**
 * Where an item is, from what a program registered.
 *
 * @param {string} service - what the program passed
 * @param {string|null} sender - the program's unique name, null for an item the scan found
 * @param {Gio.Cancellable} cancellable - stops the lookups
 * @returns {Promise<{bus: string, path: string, owner: string, identity: string, service: string}>}
 */
async function locate(service, sender, cancellable) {
    const refuse = () => new Error(`${service} cannot be registered as a tray item`);
    if (typeof service !== 'string' || !service)
        throw refuse();

    // An object path alone (libappindicator, Ayatana): the caller's
    if (service.startsWith('/')) {
        if (!sender || !GLib.Variant.is_object_path(service))
            throw refuse();
        return {bus: sender, path: service, owner: sender, identity: `${sender}${service}`, service: `${sender}${service}`};
    }

    // A bus name and a path (Electron)
    const slash = service.indexOf('/');
    if (slash > 0) {
        const name = service.slice(0, slash);
        const path = service.slice(slash);
        if (!Gio.dbus_is_name(name) || !GLib.Variant.is_object_path(path))
            throw refuse();
        const owner = await ownerOf(name, cancellable, refuse);
        return {bus: owner, path, owner, identity: `${owner}${path}`, service};
    }

    if (!Gio.dbus_is_name(service))
        throw refuse();
    const owner = await ownerOf(service, cancellable, refuse);
    // The freedesktop form keeps its name, the others are called on their owner and known by the name they gave
    if (service.startsWith(FREEDESKTOP_PREFIX))
        return {bus: service, path: ITEM_PATH, owner, identity: `${service}${ITEM_PATH}`, service};
    const identity = owner !== service ? service : `${owner}${ITEM_PATH}`;
    return {bus: owner, path: ITEM_PATH, owner, identity, service};
}

async function ownerOf(name, cancellable, refuse) {
    if (name.startsWith(':'))
        return name;
    try {
        return await nameOwner(name, cancellable);
    } catch (e) {
        if (isCancelled(e))
            throw e;
        throw refuse();
    }
}

export class Watcher {
    /**
     * @param {object} host - what shows the items
     * @param {function(Item): void} host.add - puts an item's button in the top bar
     * @param {function(Item): void} host.remove - takes it away
     */
    constructor(host) {
        this._host = host;
    }

    enable() {
        this._items = new Map();
        this._adding = new Map();
        this._invocations = new Set();
        this._cancellable = new Gio.Cancellable();
        this._enabled = true;

        const watcher = this;
        this._dbus = Gio.DBusExportedObject.wrapJSObject(WATCHER_XML, {
            RegisterStatusNotifierItemAsync: (params, invocation) => this._registerCall(params, invocation),
            RegisterStatusNotifierHostAsync: (params, invocation) => invocation.return_dbus_error(
                'org.freedesktop.DBus.Error.NotSupported', 'This tray is the only host it has'),
            get RegisteredStatusNotifierItems() {
                return watcher._listed();
            },
            IsStatusNotifierHostRegistered: true,
            ProtocolVersion: 0,
        });
        this._dbus.export(Gio.DBus.session, WATCHER_PATH);
        // No flags: the shell neither takes the name from another tray nor lets it be taken, and queues for it
        this._nameId = Gio.bus_own_name_on_connection(Gio.DBus.session, WATCHER_NAME, Gio.BusNameOwnerFlags.NONE,
            () => this._acquired(), () => this._lost());
    }

    disable() {
        this._enabled = false;
        this._cancellable?.cancel();
        for (const invocation of this._invocations ?? [])
            invocation.return_dbus_error('org.freedesktop.DBus.Error.Failed', 'The tray has stopped');
        this._invocations = null;
        for (const item of this._items?.values() ?? []) {
            this._host.remove(item);
            item.destroy();
        }
        this._items = null;
        this._adding = null;
        Main.layoutManager.disconnectObject(this);

        if (this._dbus) {
            this._dbus.emit_signal('StatusNotifierHostUnregistered', null);
            this._dbus.unexport();
        }
        this._dbus = null;
        // GDBus gives the name back before it returns, so that a tray started again at once gets it
        if (this._nameId)
            Gio.bus_unown_name(this._nameId);
        this._nameId = 0;
    }

    /**
     * Whether an object is shown already.
     *
     * @param {string} owner - its program's unique name
     * @param {string} path - its object path
     * @returns {boolean}
     */
    has(owner, path) {
        return [...this._items?.values() ?? []].some(item =>
            item.path === path && (item.owner === owner || item.bus === owner));
    }

    /**
     * Adds an item to the tray, or reads it again when it is there. Resolves once its button is in the top bar.
     *
     * @param {string} service - what the program registered
     * @param {string|null} sender - the program's unique name
     */
    async register(service, sender) {
        const target = await locate(service, sender, this._cancellable);
        if (!this._enabled)
            throw new Error('The tray has stopped');
        const known = this._items.get(target.identity);
        if (known) {
            known.reregister();
            return;
        }
        let adding = this._adding.get(target.identity)?.promise;
        if (!adding) {
            this._checkLimits(target.owner);
            adding = this._add(target).finally(() => this._adding?.delete(target.identity));
            this._adding.set(target.identity, {promise: adding, owner: target.owner});
        }
        await adding;
    }

    // Items shown or on their way, in all and for one program
    _checkLimits(owner) {
        const mine = [...this._items.values()].filter(item => item.registeredBy === owner).length +
            [...this._adding.values()].filter(adding => adding.owner === owner).length;
        if (this._items.size + this._adding.size >= MAX_ITEMS || mine >= MAX_PER_PROGRAM) {
            const e = new Error('The tray holds as many items as it takes');
            e.dbusName = LIMITS;
            throw e;
        }
    }

    async _registerCall([service], invocation) {
        this._invocations.add(invocation);
        try {
            await this.register(service, invocation.get_sender());
            if (this._invocations?.delete(invocation))
                invocation.return_value(null);
        } catch (e) {
            // An item that could not be read is refused with what reading it answered
            const name = e.dbusName ?? remoteError(e);
            if (this._invocations?.delete(invocation)) {
                invocation.return_dbus_error(name ?? REFUSED,
                    name ? e.message.replace(/^GDBus\.Error:[^:]+:\s*/, '') : e.message);
            }
        }
    }

    async _add(target) {
        await this._started();
        const item = new Item(target);
        item.registeredBy = target.owner;
        try {
            await item.init(target.owner);
        } catch (e) {
            item.destroy();
            throw isCancelled(e) ? new Error('The tray has stopped') : e;
        }
        if (!this._enabled) {
            item.destroy();
            throw new Error('The tray has stopped');
        }
        try {
            this._host.add(item);
        } catch (e) {
            item.destroy();
            throw e;
        }
        this._items.set(target.identity, item);
        item.connectObject('gone', () => this._remove(target.identity), this);
        this._dbus.emit_signal('StatusNotifierItemRegistered', new GLib.Variant('(s)', [item.service]));
        this._listChanged();
    }

    _remove(identity) {
        const item = this._items?.get(identity);
        if (!item)
            return;
        this._items.delete(identity);
        this._host.remove(item);
        item.destroy();
        this._dbus.emit_signal('StatusNotifierItemUnregistered', new GLib.Variant('(s)', [item.service]));
        this._listChanged();
    }

    _listed() {
        return [...this._items?.values() ?? []].map(item => item.service);
    }

    _listChanged() {
        this._dbus.emit_property_changed('RegisteredStatusNotifierItems',
            new GLib.Variant('as', this._listed()));
    }

    // While the shell starts, buttons wait until the top bar is up
    _started() {
        if (!Main.layoutManager._startingUp)
            return Promise.resolve();
        return new Promise(resolve => {
            Main.layoutManager.connectObject('startup-complete', () => resolve(), this);
        });
    }

    _acquired() {
        // Programs that were waiting for a tray register now
        this._dbus?.emit_signal('StatusNotifierHostRegistered', null);
    }

    _lost() {
        if (this._enabled)
            console.log(`Amethystora UI: another program is the tray (${WATCHER_NAME}); the shell waits behind it`);
    }
}
