// Saving data: on a connection that counts data (NetworkManager's metered flag, yes or guessed yes: a phone's hotspot,
// mobile broadband, a network marked so in Settings), the big downloads wait, and the pill says so. What waits is each
// updater's own business (/usr/libexec/amethystora-metered, in the units' ExecCondition): the system update with the
// apps and Homebrew (uupd), the containers' upgrade and a backup to the network, never longer than a week. This only
// shows it, and Update anyway opens System Updates and starts its update, which a person starts whatever the connection.
//
// One proxy on NetworkManager's own object, told of its property changes: nothing is polled. The stamps of what waited
// are read when the card opens.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {buttonRow, cardButton, retire} from './clock.js';
import {plain, timeOfDay} from './text.js';

const NM = 'org.freedesktop.NetworkManager';
const NM_PATH = '/org/freedesktop/NetworkManager';
const ACTIVE = 'org.freedesktop.NetworkManager.Connection.Active';
// NMMetered: yes, and guessed yes
const METERED = [1, 3];
const UPDATES = '/usr/bin/amethystora-update';
const STATE = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const SYSTEM_STAMP = '/var/lib/amethystora/metered/system';
const CONTAINERS = GLib.build_filenamev([STATE, 'pkg', 'containers']);
const BACKUP = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora', 'backup.env']);
const DAY = 24 * 3600;

// When something first waited, from its stamp (seconds), or 0
function waitedSince(path) {
    try {
        const [, bytes] = GLib.file_get_contents(path);
        const seconds = Number.parseInt(new TextDecoder().decode(bytes).trim(), 10);
        return Number.isFinite(seconds) && seconds > 0 ? seconds : 0;
    } catch {
        return 0;
    }
}

function since(seconds) {
    const date = new Date(seconds * 1000);
    const today = new Date();
    if (date.toDateString() === today.toDateString())
        return _('Waiting since %s').format(timeOfDay(date));
    const dt = GLib.DateTime.new_from_unix_local(seconds);
    // Translators: a day, as GLib.DateTime.format takes it
    return _('Waiting since %s').format(dt.format(_('%A %e %B')).replace(/\s+/g, ' ').trim());
}

export class Saving {
    constructor(activities) {
        this.id = 'saving';
        this.plate = '';
        this._activities = activities;
        this._metered = false;
        this._name = '';
        this._forced = null;
    }

    enable() {
        this._cancellable = new Gio.Cancellable();
        Gio.DBusProxy.new_for_bus(Gio.BusType.SYSTEM, Gio.DBusProxyFlags.DO_NOT_AUTO_START, null, NM, NM_PATH, NM,
            this._cancellable, (o, result) => {
                try {
                    this._proxy = Gio.DBusProxy.new_for_bus_finish(result);
                } catch (e) {
                    if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        console.debug(`Amethystora UI: NetworkManager could not be asked about the connection: ${e.message}`);
                    return;
                }
                this._proxy.connectObject('g-properties-changed', () => this._sync(), this);
                this._sync();
            });
    }

    disable() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._proxy?.disconnectObject(this);
        this._proxy = null;
        this._end();
    }

    _property(name) {
        try {
            return this._proxy?.get_cached_property(name)?.unpack() ?? null;
        } catch {
            return null;
        }
    }

    _sync() {
        const metered = this._forced ?? METERED.includes(this._property('Metered'));
        if (!metered) {
            this._metered = false;
            this._end();
            return;
        }
        if (!this._metered)
            this._askName();
        this._metered = true;
        this.pill();
        this._activities.set('saving', this);
    }

    // The connection's name as NetworkManager has it: a phone's hotspot names itself, so it is plain text, cut short
    _askName() {
        const path = this._property('PrimaryConnection');
        this._name = '';
        if (!path || path === '/')
            return;
        Gio.DBus.system.call(NM, path, 'org.freedesktop.DBus.Properties', 'Get',
            new GLib.Variant('(ss)', [ACTIVE, 'Id']), new GLib.VariantType('(v)'), Gio.DBusCallFlags.NONE, 2000,
            this._cancellable, (connection, result) => {
                try {
                    const [value] = connection.call_finish(result).deepUnpack();
                    this._name = plain(value.unpack(), 40);
                } catch {}
            });
    }

    _end() {
        if (this._activities.get('saving') === this)
            this._activities.set('saving', null);
        retire(this._content);
        this._content = null;
    }

    get shown() {
        return this._activities.get('saving') === this;
    }

    get text() {
        return _('Saving data');
    }

    get accessible() {
        return _('Saving data: this connection counts data, so the big downloads wait');
    }

    get row() {
        return {icon: 'network-cellular-signal-good-symbolic', text: this.text};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-saving'});
            this._content.add_child(new St.Icon({icon_name: 'network-cellular-signal-good-symbolic', icon_size: 14,
                y_align: Clutter.ActorAlign.CENTER}));
            this._content.add_child(new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER}));
        }
        return this._content;
    }

    // What waits, each with when it first waited (0: not yet): [{icon, text, waited, note}]
    waiting() {
        const rows = [{icon: 'software-update-available-symbolic', text: _('The system update and app updates'),
            waited: waitedSince(SYSTEM_STAMP)}];
        if (GLib.file_test(CONTAINERS, GLib.FileTest.IS_DIR)) {
            rows.push({icon: 'package-x-generic-symbolic', text: _('The containers’ upgrade'),
                waited: waitedSince(GLib.build_filenamev([STATE, 'metered', 'containers']))});
        }
        if (GLib.file_test(BACKUP, GLib.FileTest.EXISTS)) {
            rows.push({icon: 'drive-harddisk-symbolic', text: _('The backup to the network'),
                waited: waitedSince(GLib.build_filenamev([STATE, 'metered', 'backup'])),
                note: _('A backup to a disk still runs')});
        }
        return rows;
    }

    updateAnyway() {
        try {
            Util.spawn([UPDATES, 'update']);
        } catch (e) {
            console.warn(`Amethystora UI: System Updates did not open: ${e.message}`);
        }
    }

    card(api) {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body amethystora-pill-news-card',
            orientation: Clutter.Orientation.VERTICAL});
        body.add_child(new St.Label({style_class: 'amethystora-pill-news-title', text: _('Saving data')}));
        const where = new St.Label({
            style_class: 'amethystora-pill-card-text',
            text: this._name ? _('On %s, which counts data. These wait for a network that does not:').format(this._name)
                : _('This connection counts data. These wait for a network that does not:'),
        });
        where.clutter_text.line_wrap = true;
        body.add_child(where);
        const now = Date.now() / 1000;
        for (const row of this.waiting()) {
            const line = new St.BoxLayout({style_class: 'amethystora-pill-news-row'});
            const plate = new St.Bin({style_class: 'amethystora-pill-news-icon', y_align: Clutter.ActorAlign.START});
            plate.set_child(new St.Icon({icon_name: row.icon, icon_size: 18}));
            line.add_child(plate);
            const text = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true});
            text.add_child(new St.Label({style_class: 'amethystora-pill-news-item', text: row.text}));
            const notes = [];
            if (row.waited)
                notes.push(now - row.waited >= 7 * DAY ? _('Runs now: it waited a week') : since(row.waited));
            if (row.note)
                notes.push(row.note);
            if (notes.length)
                text.add_child(new St.Label({style_class: 'amethystora-pill-news-area', text: notes.join(' · ')}));
            line.add_child(text);
            body.add_child(line);
        }
        const week = new St.Label({style_class: 'amethystora-pill-news-once',
            text: _('Never longer than a week: after that, they run on any connection.')});
        week.clutter_text.line_wrap = true;
        body.add_child(week);
        body.add_child(buttonRow([cardButton(_('Update anyway'), () => {
            api.close();
            this.updateAnyway();
        }, {primary: true})]));
        return body;
    }

    // For the suites: as on a metered connection (true), an unmetered one (false), or as NetworkManager says (null)
    forTest(metered) {
        this._forced = metered;
        this._sync();
        return this.shown;
    }
}
