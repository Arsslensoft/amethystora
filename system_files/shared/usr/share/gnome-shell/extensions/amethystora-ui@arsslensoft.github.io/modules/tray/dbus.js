// The names the tray speaks on the session bus, from the public StatusNotifierItem and dbusmenu specifications,
// and the few calls every part of it makes.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

export const WATCHER_NAME = 'org.kde.StatusNotifierWatcher';
export const WATCHER_PATH = '/StatusNotifierWatcher';
export const ITEM_IFACE = 'org.kde.StatusNotifierItem';
export const ITEM_PATH = '/StatusNotifierItem';
export const MENU_IFACE = 'com.canonical.dbusmenu';
// What an item without a menu says its menu is
export const NO_MENU = '/NO_DBUSMENU';

// The watcher's side of the protocol, as programs expect to find it
export const WATCHER_XML = `<node>
  <interface name="${WATCHER_NAME}">
    <method name="RegisterStatusNotifierItem">
      <arg name="service" type="s" direction="in"/>
    </method>
    <method name="RegisterStatusNotifierHost">
      <arg name="service" type="s" direction="in"/>
    </method>
    <property name="RegisteredStatusNotifierItems" type="as" access="read"/>
    <property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
    <property name="ProtocolVersion" type="i" access="read"/>
    <signal name="StatusNotifierItemRegistered">
      <arg type="s"/>
    </signal>
    <signal name="StatusNotifierItemUnregistered">
      <arg type="s"/>
    </signal>
    <signal name="StatusNotifierHostRegistered"/>
    <signal name="StatusNotifierHostUnregistered"/>
  </interface>
</node>`;

/**
 * A method call on the session bus, answered as a promise.
 *
 * @param {string} bus - the bus name to call
 * @param {string} path - the object path
 * @param {string} iface - the interface
 * @param {string} method - the method
 * @param {GLib.Variant|null} params - its arguments, a tuple
 * @param {string|null} replyType - the reply's type, checked by GDBus; null takes any
 * @param {Gio.Cancellable|null} cancellable - stops the call
 * @param {number} timeout - milliseconds, -1 for the bus's own
 * @returns {Promise<GLib.Variant>}
 */
export function call(bus, path, iface, method, params = null, replyType = null, cancellable = null, timeout = -1) {
    return Gio.DBus.session.call(bus, path, iface, method, params,
        replyType ? new GLib.VariantType(replyType) : null, Gio.DBusCallFlags.NONE, timeout, cancellable);
}

/**
 * A call to the bus itself (ListNames, GetNameOwner and the like).
 *
 * @param {string} method - the bus's method
 * @param {GLib.Variant|null} params - its arguments
 * @param {string} replyType - the reply's type
 * @param {Gio.Cancellable|null} cancellable - stops the call
 * @returns {Promise<GLib.Variant>}
 */
export function busCall(method, params, replyType, cancellable = null) {
    return call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', method, params,
        replyType, cancellable);
}

/**
 * The unique name that owns a bus name now.
 *
 * @param {string} name - a well-known or unique name
 * @param {Gio.Cancellable|null} cancellable - stops the call
 * @returns {Promise<string>}
 */
export async function nameOwner(name, cancellable = null) {
    const reply = await busCall('GetNameOwner', new GLib.Variant('(s)', [name]), '(s)', cancellable);
    return reply.get_child_value(0).get_string()[0];
}

/**
 * A time for an event sent to a program: the current event's, else now on the clock Clutter's event times count
 * (milliseconds of the monotonic clock, as 32 bits).
 *
 * @returns {number}
 */
export function eventTime() {
    return global.get_current_time() || Math.floor(GLib.get_monotonic_time() / 1000) % 0x100000000;
}

/**
 * @param {Error} e - what a call threw
 * @returns {boolean} whether the call was stopped by its cancellable
 */
export function isCancelled(e) {
    return e instanceof GLib.Error && e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED);
}

/**
 * The D-Bus error name a failed call came back with, mapped or not, or null.
 *
 * @param {Error} e - what the call threw
 * @returns {string|null}
 */
export function remoteError(e) {
    if (!(e instanceof GLib.Error) || !Gio.DBusError.is_remote_error(e))
        return null;
    return Gio.DBusError.get_remote_error(e);
}

// The answers that mean nothing is there any more: the program, its object or its interface
const GONE = ['NameHasNoOwner', 'ServiceUnknown', 'UnknownObject', 'UnknownInterface', 'UnknownMethod',
    'UnknownProperty'].map(name => `org.freedesktop.DBus.Error.${name}`);

/**
 * @param {Error} e - what a call threw
 * @returns {boolean} whether the answer says nothing is there any more
 */
export function isGone(e) {
    return GONE.includes(remoteError(e));
}

/**
 * @param {Error} e - what a call threw
 * @returns {boolean} whether the object does not have the method called
 */
export function isUnknownMethod(e) {
    return remoteError(e) === 'org.freedesktop.DBus.Error.UnknownMethod';
}

/**
 * A GLib timeout as a promise, its source kept in `sources` so that whoever owns it can remove it.
 *
 * @param {Set<number>} sources - where the pending source is kept
 * @param {number} ms - how long
 * @param {number} priority - the GLib priority
 * @returns {Promise<void>}
 */
export function sleep(sources, ms, priority = GLib.PRIORITY_DEFAULT) {
    return new Promise(resolve => {
        const id = GLib.timeout_add(priority, ms, () => {
            sources.delete(id);
            resolve();
            return GLib.SOURCE_REMOVE;
        });
        sources.add(id);
    });
}

/**
 * Removes every GLib source kept by sleep().
 *
 * @param {Set<number>} sources - the sources
 */
export function clearSources(sources) {
    for (const id of sources)
        GLib.source_remove(id);
    sources.clear();
}
