// Network by app, offered and off (network-apps): the network meter's menu ends with the apps that have a connection
// open now, and to where. Read only while that menu is open, as it opens and with the meters' own reading every five
// seconds, and never while it is closed. Only this account's own sockets and processes can be read, which is all the
// list needs; how much each app sends would need root, so the list counts connections, not bytes.
//
// A place is a name only when the machine already knows one for the address: systemd-resolved is asked with its
// "no network" flag, so it answers from its cache, /etc/hosts and the local network's own names, and never asks a
// server. Nothing leaves the machine to make the list.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {plain} from '../pill/text.js';

const SECURITY = '/usr/bin/amethystora-security';
// The most apps listed, places named in a row, and addresses asked about in one reading
const MOST_APPS = 6;
const PLACES_SHOWN = 2;
const MOST_NAMES = 24;
// systemd-resolved's SD_RESOLVED_NO_NETWORK (src/shared/resolved-def.h): answer only from what it already knows
const NO_NETWORK = 1 << 15;
const AF_INET = 2;
const AF_INET6 = 10;
// TCP connected or connecting; a UDP socket counts when it has a peer
const TCP_OPEN = new Set(['01', '02', '03']);
// Processes read at once
const BATCH = 16;
// The phone link, by its user unit
const UNIT_NAMES = {'amethystora-connect.service': () => _('Phone link')};

function readLater(path, cancellable) {
    return new Promise(resolve => {
        Gio.File.new_for_path(path).load_contents_async(cancellable, (file, result) => {
            try {
                resolve(new TextDecoder().decode(file.load_contents_finish(result)[1]));
            } catch {
                resolve(null);
            }
        });
    });
}

// The links in a process's fd folder: the socket inodes it holds
function socketsOf(pid, cancellable) {
    return new Promise(resolve => {
        const dir = Gio.File.new_for_path(`/proc/${pid}/fd`);
        dir.enumerate_children_async('standard::name,standard::symlink-target',
            Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, GLib.PRIORITY_LOW, cancellable, (d, result) => {
                let enumerator;
                try {
                    enumerator = d.enumerate_children_finish(result);
                } catch {
                    resolve([]);
                    return;
                }
                const inodes = [];
                const next = () => enumerator.next_files_async(64, GLib.PRIORITY_LOW, cancellable, (e, res) => {
                    let infos;
                    try {
                        infos = e.next_files_finish(res);
                    } catch {
                        infos = [];
                    }
                    for (const info of infos) {
                        const match = /^socket:\[(\d+)\]$/.exec(info.get_symlink_target() ?? '');
                        if (match)
                            inodes.push(match[1]);
                    }
                    if (infos.length > 0) {
                        next();
                    } else {
                        enumerator.close_async(GLib.PRIORITY_LOW, null, null);
                        resolve(inodes);
                    }
                });
                next();
            });
    });
}

// An address from /proc/net: four bytes, or sixteen in four words, each word in the machine's order (little-endian)
function address(hex) {
    const bytes = [];
    for (let word = 0; word < hex.length; word += 8) {
        const chunk = hex.slice(word, word + 8);
        for (let i = 6; i >= 0; i -= 2)
            bytes.push(parseInt(chunk.slice(i, i + 2), 16));
    }
    return bytes;
}

// Sockets of this account with a peer, by inode: {bytes, family}
function parseTable(text, tcp, uid, found) {
    for (const line of (text ?? '').split('\n').slice(1)) {
        const fields = line.trim().split(/\s+/);
        if (fields.length < 10 || Number(fields[7]) !== uid)
            continue;
        const [remoteHex, port] = fields[2].split(':');
        if (!remoteHex || /^0+$/.test(remoteHex) || /^0+$/.test(port ?? ''))
            continue;
        if (tcp && !TCP_OPEN.has(fields[3]))
            continue;
        const bytes = address(remoteHex);
        const family = bytes.length === 4 ? AF_INET : AF_INET6;
        found.set(fields[9], {bytes, family});
    }
}

function ownedBy(path, uid) {
    try {
        return Gio.File.new_for_path(path).query_info('unix::uid', Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, null)
            .get_attribute_uint32('unix::uid') === uid;
    } catch {
        return false;
    }
}

// What the place is: loopback (left out), on this network, or out there
function kind(bytes) {
    // An IPv4 address carried in IPv6 is judged as the IPv4 address
    if (bytes.length === 16 && bytes.slice(0, 10).every(b => b === 0) && bytes[10] === 0xff && bytes[11] === 0xff)
        bytes = bytes.slice(12);
    if (bytes.length === 4) {
        const [a, b] = bytes;
        if (a === 127)
            return 'loopback';
        if (a === 10 || (a === 172 && b >= 16 && b < 32) || (a === 192 && b === 168) || (a === 169 && b === 254))
            return 'local';
        return 'internet';
    }
    if (bytes.every((v, i) => v === (i === 15 ? 1 : 0)))
        return 'loopback';
    if ((bytes[0] & 0xfe) === 0xfc || (bytes[0] === 0xfe && (bytes[1] & 0xc0) === 0x80))
        return 'local';
    return 'internet';
}

function textOf(place) {
    const family = place.family === AF_INET ? Gio.SocketFamily.IPV4 : Gio.SocketFamily.IPV6;
    return Gio.InetAddress.new_from_bytes(place.bytes, family)?.to_string() ?? '';
}

// The app a process belongs to, from its systemd unit: GNOME starts apps in app-gnome-ID-PID.scope, Flatpak in
// app-flatpak-ID-N.scope, autostart in app-gnome-ID@autostart.service (systemd escapes a hyphen in an ID as \x2d)
function appOfUnit(unit) {
    const match = /^app-[a-z0-9]+-(.+?)(?:@[^.]*)?(?:-\d+)?\.(?:scope|service)$/.exec(unit);
    if (!match)
        return null;
    const id = match[1].replace(/\\x([0-9a-f]{2})/g, (m, hex) => String.fromCharCode(parseInt(hex, 16)));
    return Shell.AppSystem.get_default().lookup_app(`${id}.desktop`);
}

export class Connections {
    constructor(ext) {
        this._ext = ext;
        this._names = new Map();
        this.reading = false;
    }

    start() {
        this.actor = new St.BoxLayout({style_class: 'amethystora-privacy-apps', orientation: Clutter.Orientation.VERTICAL});
        const top = new St.BoxLayout({style_class: 'amethystora-privacy-apps-top'});
        const heading = new St.Label({style_class: 'amethystora-privacy-apps-heading', text: _('Talking to the network'),
            x_expand: true});
        top.add_child(heading);
        top.add_child(new St.Label({style_class: 'amethystora-privacy-apps-now', text: _('now'),
            y_align: Clutter.ActorAlign.END}));
        this.actor.add_child(top);
        this._list = new St.BoxLayout({style_class: 'amethystora-privacy-apps-list',
            orientation: Clutter.Orientation.VERTICAL});
        this.actor.add_child(this._list);
        this._empty = new St.Label({style_class: 'amethystora-privacy-apps-empty',
            text: _('No app is talking to the network now.')});
        this._empty.clutter_text.line_wrap = true;
        this.actor.add_child(this._empty);
        const link = new St.Button({style_class: 'amethystora-privacy-link amethystora-privacy-apps-link',
            label: _('Network per app, in Security'), can_focus: true, x_align: Clutter.ActorAlign.END});
        link.connect('clicked', () => {
            this._menu?.close();
            Util.spawn([SECURITY, 'apps']);
        });
        this.actor.add_child(link);
        this._ext.modules?.get('meters')?.setNetworkSection?.(this);
    }

    stop() {
        this._ext.modules?.get('meters')?.setNetworkSection?.(null);
        this._cancellable?.cancel();
        this._cancellable = null;
        this.actor?.destroy();
        this.actor = null;
        this._names.clear();
        this._menu = null;
        this.reading = false;
    }

    // The meters say the network menu opened (with its menu) or closed
    shown(open, menu) {
        this._menu = menu;
        if (open) {
            this.reading = true;
            this.tick();
        } else {
            this.reading = false;
            this._cancellable?.cancel();
            this._cancellable = null;
            this._names.clear();
        }
    }

    // The meters' reading, while the menu is open
    tick() {
        if (!this.reading || this._busy)
            return;
        this._busy = true;
        const cancellable = this._cancellable ??= new Gio.Cancellable();
        this._read(cancellable).then(apps => {
            if (!cancellable.is_cancelled() && this.actor)
                this._show(apps);
        }).catch(e => {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                console.warn(`Amethystora UI: network by app could not read the connections: ${e.message}`);
        }).finally(() => {
            this._busy = false;
        });
    }

    async _read(cancellable) {
        const uid = new Gio.Credentials().get_unix_user();
        const sockets = new Map();
        const tables = await Promise.all(['tcp', 'tcp6', 'udp', 'udp6'].map(t => readLater(`/proc/net/${t}`, cancellable)));
        tables.forEach((text, i) => parseTable(text, i < 2, uid, sockets));
        for (const [inode, place] of sockets) {
            if (kind(place.bytes) === 'loopback')
                sockets.delete(inode);
        }
        if (sockets.size === 0)
            return [];

        const pids = [];
        const dir = GLib.Dir.open('/proc', 0);
        // Only this account's processes, by the owner of their folder: another account's cannot be read anyway
        for (let name = dir.read_name(); name; name = dir.read_name()) {
            if (/^\d+$/.test(name) && ownedBy(`/proc/${name}`, uid))
                pids.push(name);
        }
        dir.close();

        // The account's processes that hold one of those sockets
        const holders = new Map();
        for (let i = 0; i < pids.length && holders.size < sockets.size; i += BATCH) {
            if (cancellable.is_cancelled())
                return [];
            const batch = pids.slice(i, i + BATCH);
            // eslint-disable-next-line no-await-in-loop
            const lists = await Promise.all(batch.map(pid => socketsOf(pid, cancellable)));
            lists.forEach((inodes, k) => {
                for (const inode of inodes) {
                    if (sockets.has(inode) && !holders.has(inode))
                        holders.set(inode, batch[k]);
                }
            });
        }

        // Grouped by app
        const groups = new Map();
        const units = new Map();
        for (const [inode, pid] of holders) {
            if (!units.has(pid))
                // eslint-disable-next-line no-await-in-loop
                units.set(pid, await this._who(pid, cancellable));
            const who = units.get(pid);
            let group = groups.get(who.key);
            if (!group) {
                group = {name: who.name, count: 0, places: []};
                groups.set(who.key, group);
            }
            group.count++;
            const place = sockets.get(inode);
            const text = textOf(place);
            if (!group.places.some(p => p.text === text))
                group.places.push({...place, text});
        }
        const apps = [...groups.values()].sort((a, b) => b.count - a.count || a.name.localeCompare(b.name))
            .slice(0, MOST_APPS);
        await this._name(apps, cancellable);
        return apps;
    }

    // Who a process is: its app, the phone link, or its program's name
    async _who(pid, cancellable) {
        const cgroup = await readLater(`/proc/${pid}/cgroup`, cancellable) ?? '';
        const unit = cgroup.trim().split('/').pop() ?? '';
        const app = appOfUnit(unit);
        if (app)
            return {key: `app:${app.get_id()}`, name: plain(app.get_name(), 40)};
        if (UNIT_NAMES[unit])
            return {key: `unit:${unit}`, name: UNIT_NAMES[unit]()};
        const comm = plain((await readLater(`/proc/${pid}/comm`, cancellable) ?? '').trim(), 40);
        return {key: `program:${comm}`, name: comm || _('A program')};
    }

    // Names the machine already knows for the places shown, asked of resolved without the network
    async _name(apps, cancellable) {
        const wanted = [];
        for (const app of apps) {
            for (const place of app.places.slice(0, PLACES_SHOWN)) {
                if (!this._names.has(place.text) && !wanted.includes(place) && wanted.length < MOST_NAMES)
                    wanted.push(place);
            }
        }
        await Promise.all(wanted.map(place => new Promise(resolve => {
            Gio.DBus.system.call('org.freedesktop.resolve1', '/org/freedesktop/resolve1',
                'org.freedesktop.resolve1.Manager', 'ResolveAddress',
                new GLib.Variant('(iiayt)', [0, place.family, place.bytes, NO_NETWORK]),
                new GLib.VariantType('(a(is)t)'), Gio.DBusCallFlags.NO_AUTO_START, 2000, cancellable,
                (bus, result) => {
                    let name = '';
                    try {
                        const [names] = bus.call_finish(result).deepUnpack();
                        name = plain(names?.[0]?.[1] ?? '', 60);
                    } catch {}
                    this._names.set(place.text, name);
                    resolve();
                });
        })));
    }

    _placeText(place) {
        const shown = this._names.get(place.text) || place.text;
        return kind(place.bytes) === 'local' ? _('%s on this network').format(shown) : shown;
    }

    _show(apps) {
        this._list.destroy_all_children();
        this._empty.visible = apps.length === 0;
        for (const app of apps) {
            const row = new St.BoxLayout({style_class: 'amethystora-privacy-app', orientation: Clutter.Orientation.VERTICAL});
            const top = new St.BoxLayout({style_class: 'amethystora-privacy-app-top'});
            const name = new St.Label({style_class: 'amethystora-privacy-app-name', text: app.name, x_expand: true});
            const count = new St.Label({style_class: 'amethystora-privacy-app-count',
                text: ngettext('%d connection', '%d connections', app.count).format(app.count)});
            top.add_child(name);
            top.add_child(count);
            row.add_child(top);
            const shown = app.places.slice(0, PLACES_SHOWN).map(p => this._placeText(p));
            const more = app.places.length - shown.length;
            if (more > 0)
                shown.push(ngettext('%d more', '%d more', more).format(more));
            const hosts = new St.Label({style_class: 'amethystora-privacy-app-hosts', text: shown.join(' · ')});
            hosts.clutter_text.line_wrap = true;
            hosts.set_text_direction(Clutter.TextDirection.LTR);
            row.add_child(hosts);
            for (const labelActor of [name, count, hosts])
                labelActor.clutter_text.use_markup = false;
            this._list.add_child(row);
        }
        this._shown = apps.map(a => ({name: a.name, count: a.count, places: a.places.map(p => this._placeText(p))}));
    }

    // --- For the VM suite ---------------------------------------------------------------------------------------------

    state() {
        return {reading: this.reading, shown: this._shown ?? [], names: this._names.size, inMenu: !!this.actor?.get_parent()};
    }
}
