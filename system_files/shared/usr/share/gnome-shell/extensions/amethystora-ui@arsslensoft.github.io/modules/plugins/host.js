// The plugin host as a client sees it (apps/plugin-host/dbus/org.amethystora.Plugins.xml). The host runs only while a plugin is on, so
// this does nothing but watch for its bus name; when the name appears the shell checks who owns it, by the file the owner runs,
// and listens only to that connection (anything running as the account could own the name first and feed the pill). The host holds
// every state, so a screen lock that disables the extension loses nothing: the next enable asks List again.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const NAME = 'org.amethystora.Plugins';
const PATH = '/org/amethystora/Plugins';
const IFACE = 'org.amethystora.Plugins';
export const HOST_EXE = '/usr/libexec/amethystora-plugin-host';

export class HostClient {
    // handlers: {list(array), updated(id, plugin), paused(id, reason), asked(id, kind, text), over(id, host), gone()}
    constructor(handlers) {
        this._h = handlers;
        this._owner = null;
        this._subs = [];
        this._serial = 0;
    }

    start() {
        this._cancellable = new Gio.Cancellable();
        this._watch = Gio.bus_watch_name(Gio.BusType.SESSION, NAME, Gio.BusNameWatcherFlags.NONE,
            (connection, name, owner) => this._appeared(owner), () => this._vanished());
    }

    stop() {
        this._serial++;
        this._cancellable?.cancel();
        this._cancellable = null;
        if (this._watch)
            Gio.bus_unwatch_name(this._watch);
        this._watch = 0;
        this._unsubscribe();
        this._owner = null;
    }

    get connected() {
        return this._owner !== null;
    }

    _unsubscribe() {
        for (const id of this._subs)
            Gio.DBus.session.signal_unsubscribe(id);
        this._subs = [];
    }

    // The owner's file, from its process: only the host's own program is believed
    _appeared(owner) {
        const serial = ++this._serial;
        this._unsubscribe();
        this._owner = null;
        Gio.DBus.session.call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionCredentials',
            new GLib.Variant('(s)', [owner]), null, Gio.DBusCallFlags.NONE, 2000, this._cancellable, (connection, result) => {
                if (serial !== this._serial)
                    return;
                let pid;
                try {
                    const credentials = connection.call_finish(result).recursiveUnpack()[0];
                    pid = credentials.ProcessID;
                } catch {
                    return;
                }
                if (!Number.isInteger(pid) || pid < 2)
                    return;
                Gio.File.new_for_path(`/proc/${pid}/exe`).query_info_async('standard::symlink-target', Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS,
                    GLib.PRIORITY_DEFAULT, this._cancellable, (file, done) => {
                        if (serial !== this._serial)
                            return;
                        let exe = null;
                        try {
                            exe = file.query_info_finish(done).get_symlink_target();
                        } catch {}
                        if (exe !== HOST_EXE) {
                            console.warn(`Amethystora UI: ${NAME} is owned by ${exe ?? 'something unreadable'}, not the plugin host: ignored`);
                            return;
                        }
                        this._trust(owner);
                    });
            });
    }

    _trust(owner) {
        this._owner = owner;
        const on = (member, handle) => this._subs.push(Gio.DBus.session.signal_subscribe(owner, IFACE, member, PATH, null,
            Gio.DBusSignalFlags.NONE, (c, sender, path, iface, signal, parameters) => {
                // Only a signal from the connection that was checked
                if (sender !== this._owner)
                    return;
                try {
                    handle(...parameters.deepUnpack());
                } catch (e) {
                    console.warn(`Amethystora UI: a plugin signal was not understood: ${e.message}`);
                }
            }));
        on('Updated', (id, text) => this._h.updated(id, parse(text)));
        on('Paused', (id, reason) => this._h.paused?.(id, reason));
        on('Asked', (id, kind, text) => this._h.asked?.(id, kind, text));
        on('OverLimit', (id, host) => this._h.over?.(id, host));
        this._call('List', null, '(s)').then(([text]) => {
            const list = parse(text);
            if (this._owner === owner && Array.isArray(list))
                this._h.list(list);
        }).catch(() => {});
    }

    _vanished() {
        this._serial++;
        this._unsubscribe();
        const had = this._owner !== null;
        this._owner = null;
        if (had)
            this._h.gone?.();
    }

    _call(method, args, reply = null) {
        return new Promise((resolve, reject) => {
            if (!this._owner || !this._cancellable) {
                reject(new Error('the plugin host is not running'));
                return;
            }
            Gio.DBus.session.call(this._owner, PATH, IFACE, method, args, reply ? new GLib.VariantType(reply) : null,
                Gio.DBusCallFlags.NONE, 5000, this._cancellable, (connection, result) => {
                    try {
                        resolve(connection.call_finish(result).deepUnpack());
                    } catch (e) {
                        reject(e);
                    }
                });
        });
    }

    action(id, action) {
        return this._call('Action', new GLib.Variant('(ss)', [id, action])).catch(e => console.debug(`Amethystora UI: ${id} ${action}: ${e.message}`));
    }

    pause(id) {
        return this._call('Pause', new GLib.Variant('(s)', [id])).catch(() => {});
    }

    // The app in front, for the plugins granted events=focused-app (the module asks only while one is on)
    focused(app) {
        return this._call('Focused', new GLib.Variant('(s)', [app])).catch(() => {});
    }

    resume(id) {
        return this._call('Resume', new GLib.Variant('(s)', [id])).catch(() => {});
    }
}

function parse(text) {
    try {
        return JSON.parse(text);
    } catch {
        return null;
    }
}
