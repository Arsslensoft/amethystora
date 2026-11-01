// A new network's zone (plans/ui-ideas/spec-restart.md 4): the first time this account's desktop joins a network, a
// notification names the firewall zone it is in and what that means, with Open Security. It switches nothing: naming a
// network home stays Security's (its home networks).
//
// One proxy on NetworkManager's own object, told when the primary connection changes; firewalld is asked, read-only,
// which zone the connection's interface is in, as the phone link asks it. The networks seen are kept by their
// NetworkManager UUID only, in ~/.local/state/amethystora/networks-seen.json (with a copy beside it); the first time
// this runs, every saved connection is recorded as seen, without a word.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {plain} from '../pill/text.js';

const NM = 'org.freedesktop.NetworkManager';
const NM_PATH = '/org/freedesktop/NetworkManager';
const NM_SETTINGS = '/org/freedesktop/NetworkManager/Settings';
const ACTIVE = 'org.freedesktop.NetworkManager.Connection.Active';
const DEVICE = 'org.freedesktop.NetworkManager.Device';
const FIREWALLD = 'org.fedoraproject.FirewallD1';
const FIREWALLD_PATH = '/org/fedoraproject/FirewallD1';
const SECURITY = '/usr/bin/amethystora-security';
// The kinds of connection a person joins: Wi-Fi, a cable, mobile broadband, a phone over Bluetooth
const TYPES = ['802-11-wireless', '802-3-ethernet', 'gsm', 'cdma', 'bluetooth'];
const STATE_DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const SEEN = GLib.build_filenamev([STATE_DIR, 'networks-seen.json']);
const MOST = 256;
const UUID = /^[0-9A-Fa-f-]{8,64}$/;
const TIMEOUT = 3000;

function readSeen() {
    for (const path of [SEEN, `${SEEN}.bak`]) {
        try {
            const [, bytes] = GLib.file_get_contents(path);
            const data = JSON.parse(new TextDecoder().decode(bytes));
            if (data?.version === 1 && Array.isArray(data.seen))
                return data.seen.filter(u => typeof u === 'string' && UUID.test(u)).slice(-MOST);
        } catch {}
    }
    return null;
}

// What the zone means here, in a sentence or two
export function zoneWords(zone) {
    switch (zone) {
    case 'amethystora':
        return _('In the default zone. Phone pairing is closed here and nothing on this computer is shared. If this is your home, name it home in Security.');
    case 'amethystora-home':
        return _('In the home zone. Phones can pair with this computer here.');
    case 'amethystora-away':
        return _('In the away zone. Nothing that starts on this network reaches this computer, and phone pairing is closed.');
    case null:
    case undefined:
    case '':
        return _('The firewall did not say which zone this network is in. Security shows how it stands.');
    default:
        return _('In the %s zone. Security says what each zone lets in.').format(plain(zone, 40));
    }
}

export class Zone {
    constructor(ext) {
        this._ext = ext;
    }

    start() {
        this._cancellable = new Gio.Cancellable();
        this._seen = readSeen();
        this._ready = this._seen !== null;
        this._seen ??= [];
        this._last = null;
        this._current = null;
        Gio.DBusProxy.new_for_bus(Gio.BusType.SYSTEM, Gio.DBusProxyFlags.DO_NOT_AUTO_START, null, NM, NM_PATH, NM,
            this._cancellable, (o, result) => {
                try {
                    this._proxy = Gio.DBusProxy.new_for_bus_finish(result);
                } catch (e) {
                    if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        console.debug(`Amethystora UI: NetworkManager could not be asked about networks: ${e.message}`);
                    return;
                }
                this._proxy.connectObject('g-properties-changed', (proxy, changed) => {
                    if (changed.lookup_value('PrimaryConnection', null))
                        this._primary();
                }, this);
                if (this._ready)
                    this._primary();
                else
                    this._firstRun();
            });
    }

    stop() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._proxy?.disconnectObject(this);
        this._proxy = null;
        this._source?.destroy();
        this._source = null;
    }

    async _call(name, path, iface, method, args, type) {
        return new Promise((resolve, reject) => {
            Gio.DBus.system.call(name, path, iface, method, args, type ? new GLib.VariantType(type) : null,
                Gio.DBusCallFlags.NO_AUTO_START, TIMEOUT, this._cancellable, (connection, result) => {
                    try {
                        resolve(connection.call_finish(result));
                    } catch (e) {
                        reject(e);
                    }
                });
        });
    }

    // The first run: every connection saved already is one this account knows, so nothing is said about it
    async _firstRun() {
        const cancellable = this._cancellable;
        try {
            const [paths] = (await this._call(NM, NM_SETTINGS, `${NM}.Settings`, 'ListConnections', null, '(ao)'))
                .deepUnpack();
            for (const path of paths.slice(0, MOST)) {
                try {
                    const [settings] = (await this._call(NM, path, `${NM}.Settings.Connection`, 'GetSettings', null,
                        '(a{sa{sv}})')).recursiveUnpack();
                    const uuid = settings?.connection?.uuid;
                    if (typeof uuid === 'string' && UUID.test(uuid) && !this._seen.includes(uuid))
                        this._seen.push(uuid);
                } catch {}
                if (cancellable.is_cancelled())
                    return;
            }
        } catch (e) {
            if (cancellable.is_cancelled())
                return;
            console.debug(`Amethystora UI: the saved networks could not be listed: ${e.message}`);
        }
        this._ready = true;
        this._write();
    }

    _write() {
        const text = new TextEncoder().encode(`${JSON.stringify({version: 1, seen: this._seen.slice(-MOST)})}\n`);
        try {
            GLib.mkdir_with_parents(STATE_DIR, 0o700);
            // The copy first: a crash between the two leaves one whole file
            GLib.file_set_contents_full(`${SEEN}.bak`, text, GLib.FileSetContentsFlags.CONSISTENT, 0o600);
            GLib.file_set_contents_full(SEEN, text, GLib.FileSetContentsFlags.CONSISTENT, 0o600);
        } catch (e) {
            console.warn(`Amethystora UI: the networks seen could not be written: ${e.message}`);
        }
    }

    async _primary() {
        const path = this._proxy?.get_cached_property('PrimaryConnection')?.unpack();
        if (!this._ready || !path || path === '/' || path === this._current)
            return;
        this._current = path;
        const cancellable = this._cancellable;
        try {
            const [props] = (await this._call(NM, path, 'org.freedesktop.DBus.Properties', 'GetAll',
                new GLib.Variant('(s)', [ACTIVE]), '(a{sv})')).recursiveUnpack();
            const uuid = props?.Uuid;
            if (typeof uuid !== 'string' || !UUID.test(uuid) || !TYPES.includes(props.Type) ||
                this._seen.includes(uuid))
                return;
            let zone = null;
            const device = Array.isArray(props.Devices) ? props.Devices[0] : null;
            if (device) {
                try {
                    const [value] = (await this._call(NM, device, 'org.freedesktop.DBus.Properties', 'Get',
                        new GLib.Variant('(ss)', [DEVICE, 'IpInterface']), '(v)')).deepUnpack();
                    zone = await this._zoneOf(value.unpack());
                } catch {}
            }
            if (!cancellable.is_cancelled())
                this.consider(uuid, props.Id, zone);
        } catch (e) {
            if (!cancellable.is_cancelled())
                console.debug(`Amethystora UI: the new connection could not be read: ${e.message}`);
        }
    }

    // The zone firewalld puts an interface in, its default zone when it names none, or null when it cannot be asked
    async _zoneOf(iface) {
        if (typeof iface !== 'string' || !iface)
            return null;
        try {
            const [zone] = (await this._call(FIREWALLD, FIREWALLD_PATH, `${FIREWALLD}.zone`, 'getZoneOfInterface',
                new GLib.Variant('(s)', [iface]), '(s)')).deepUnpack();
            if (zone)
                return zone;
            const [fallback] = (await this._call(FIREWALLD, FIREWALLD_PATH, FIREWALLD, 'getDefaultZone', null, '(s)'))
                .deepUnpack();
            return fallback || null;
        } catch {
            return null;
        }
    }

    // A network seen for the first time: recorded, then said once
    consider(uuid, name, zone) {
        if (this._seen.includes(uuid))
            return false;
        this._seen.push(uuid);
        this._seen = this._seen.slice(-MOST);
        this._write();
        const shown = plain(typeof name === 'string' ? name : '', 64) || _('This network');
        this._last = {name: shown, zone: zone ?? null};
        this._notify(shown, zoneWords(zone));
        return true;
    }

    _notify(name, body) {
        if (!this._source) {
            this._source = new MessageTray.Source({title: _('Networks'), iconName: 'network-wireless-symbolic'});
            this._source.connect('destroy', () => (this._source = null));
            Main.messageTray.add(this._source);
        }
        const notification = new MessageTray.Notification({
            source: this._source,
            title: _('%s, a new network').format(name),
            body,
            isTransient: false,
        });
        notification.addAction(_('Fine'), () => {});
        notification.addAction(_('Open Security'), () => {
            try {
                Util.spawn([SECURITY, 'settings/network']);
            } catch (e) {
                console.warn(`Amethystora UI: Security did not open: ${e.message}`);
            }
        });
        this._source.addNotification(notification);
    }

    state() {
        return {ready: this._ready, seen: this._seen.length, last: this._last};
    }
}
