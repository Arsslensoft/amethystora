// Everything moving, one place (R7-20, the key moving): downloads, Flatpak updates, Files' copies, backups and the phone's
// transfers as one activity with one ring, and a card listing each. Nothing new is watched for it: the dock already
// follows the Downloads folder and the Flatpak pulls and says so here (movinglist.js), Files holds an inhibitor for as long
// as it copies (through the portal on GNOME 51, which gnome-session's signals say), with no number; the backup is a unit of the account's own manager, and
// the phone link says Transfer. The names are in memory, for the card, and are not said in the pill while the screen is
// recorded or shared. A `cp` in a terminal says nothing to anybody and is not seen.
//
// It doesn't sleep on your copy (R7-56, the key stay-awake): while something moves, a suspend inhibitor with gnome-session.
// The screen can still go dark, closing the lid still sleeps (logind ignores inhibitors for the lid unless told otherwise,
// and the image does not tell it), and on battery it gives way below 15 %. The point is walking away, and the extension is
// off while the screen is locked, so then a small keeper outside the class holds on, asks once a minute whether anything
// it knew of is still going, and lets go when nothing is.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {retire} from './clock.js';
import {HOLD_LONGEST, STALE_DOWNLOAD, agedOut, fractionOf, givesWay, listenToMoving, partialName} from './movinglist.js';
import {subscribeUnits} from './subscribe.js';
import {plain} from './text.js';

const SESSION = 'org.gnome.SessionManager';
const SESSION_PATH = '/org/gnome/SessionManager';
const INHIBITOR = 'org.gnome.SessionManager.Inhibitor';
// gnome-session's flag for suspending, and what it is told about who asks
const SUSPEND = 4;
const IDLE = 8;
const APP_ID = 'org.gnome.Shell';
const FILES = ['org.gnome.Nautilus', 'org.gnome.Nautilus.desktop'];
const SYSTEMD = 'org.freedesktop.systemd1';
const MANAGER = '/org/freedesktop/systemd1';
const BACKUP = 'amethystora-backup.service';
const UPOWER = 'org.freedesktop.UPower';
const DISPLAY = '/org/freedesktop/UPower/devices/DisplayDevice';
const BUS_TIMEOUT = 2000;
// The pill says a change at most once a second
const COALESCE_MS = 1000;
// A look at the downloads this often, only while there are any; the keeper's look, while locked
const LOOK_SECONDS = 60;
// After an unlock, this long for the parts to say again what still moves
const GRACE_SECONDS = 90;
// While locked: how long each kind nothing can ask about is taken at its word, and the longest of all
const LOCKED_FLATPAK = 30 * 60;
const LOCKED_PHONE = 10 * 60;
const LOCKED_LONGEST = HOLD_LONGEST;
const MOST_ROWS = 6;
// Files' inhibitors whose removal was heard, so that one gone before it was asked about is not shown afterwards
const MOST_GONE = 32;

// The monotonic clock, in seconds
function mono() {
    return GLib.get_monotonic_time() / 1e6;
}

function locked() {
    return Main.sessionMode.isLocked || Main.screenShield?.locked === true;
}

function call(bus, name, path, iface, method, args, type, cancellable, done) {
    bus.call(name, path, iface, method, args, type ? new GLib.VariantType(type) : null, Gio.DBusCallFlags.NO_AUTO_START,
        BUS_TIMEOUT, cancellable, (connection, result) => {
            let value = null;
            try {
                value = connection.call_finish(result).deepUnpack();
            } catch {}
            done?.(value);
        });
}

function letGo(cookie) {
    call(Gio.DBus.session, SESSION, SESSION_PATH, SESSION, 'Uninhibit', new GLib.Variant('(u)', [cookie]), null, null, null);
}

// The account's backup unit's state, or null
function askBackup(done) {
    call(Gio.DBus.session, SYSTEMD, MANAGER, `${SYSTEMD}.Manager`, 'LoadUnit', new GLib.Variant('(s)', [BACKUP]), '(o)', null, loaded => {
        if (!loaded) {
            done(null, null);
            return;
        }
        call(Gio.DBus.session, SYSTEMD, loaded[0], 'org.freedesktop.DBus.Properties', 'Get',
            new GLib.Variant('(ss)', [`${SYSTEMD}.Unit`, 'ActiveState']), '(v)', null, value => {
                done(loaded[0], value ? value[0].deepUnpack() : null);
            });
    });
}

// The computer's own battery says it must give way, or null when it cannot say
function askBattery(done) {
    call(Gio.DBus.system, UPOWER, DISPLAY, 'org.freedesktop.DBus.Properties', 'GetAll',
        new GLib.Variant('(s)', [`${UPOWER}.Device`]), '(a{sv})', null, value => {
            if (!value) {
                done(false);
                return;
            }
            const props = value[0];
            done(givesWay(props.Percentage?.deepUnpack(), props.State?.deepUnpack()));
        });
}

// What holds on while the screen is locked: {cookie, items: [{kind, path}], since, started, timer}. since is when the lock
// began and started when the machine began to be kept awake, both on the monotonic clock in microseconds
let keeper = null;

function endKeeper() {
    if (!keeper)
        return;
    if (keeper.timer)
        GLib.source_remove(keeper.timer);
    letGo(keeper.cookie);
    keeper = null;
}

// For the pill: the keeper has nobody to give it to (moving or stay-awake went off during the lock), so it lets go
export function releaseKeeper() {
    endKeeper();
}

function keeperLook() {
    const kept = keeper;
    if (!kept)
        return;
    const age = (GLib.get_monotonic_time() - kept.since) / 1e6;
    // At most this long in all, counted from when the machine began to be kept awake
    if (age > LOCKED_LONGEST || (GLib.get_monotonic_time() - kept.started) / 1e6 > LOCKED_LONGEST) {
        endKeeper();
        return;
    }
    askBattery(gives => {
        if (kept !== keeper)
            return;
        if (gives) {
            endKeeper();
            return;
        }
        const looks = kept.items.map(item => new Promise(resolve => {
            switch (item.kind) {
            case 'download':
                Gio.File.new_for_path(item.path).query_info_async('time::modified', Gio.FileQueryInfoFlags.NONE,
                    GLib.PRIORITY_LOW, null, (file, result) => {
                        try {
                            const modified = file.query_info_finish(result).get_attribute_uint64('time::modified');
                            resolve(Date.now() / 1000 - modified < STALE_DOWNLOAD);
                        } catch {
                            resolve(false);
                        }
                    });
                break;
            case 'backup':
                askBackup((path, state) => resolve(state === 'active' || state === 'activating'));
                break;
            case 'flatpak':
                resolve(age < LOCKED_FLATPAK);
                break;
            case 'phone':
                resolve(age < LOCKED_PHONE);
                break;
            default:
                // Files holds its own inhibitor for as long as it copies
                resolve(false);
            }
        }));
        Promise.all(looks).then(alive => {
            if (kept === keeper && !alive.some(Boolean))
                endKeeper();
        });
    });
}

export class Moving {
    // awake: whether stay-awake is on now; capture: says whether the screen is recorded or shared
    constructor(activities, {capture, connect, flatpaks, awake}) {
        this._activities = activities;
        this._capture = capture;
        this._connect = connect;
        this._flatpaks = flatpaks;
        this._awake = awake;
        this.id = 'moving';
        this.plate = '';
    }

    enable() {
        this._items = new Map();
        this._gone = new Set();
        this._cancellable = new Gio.Cancellable();
        // What held on through a lock is this one's now, until the parts have had time to say what still moves
        if (keeper) {
            if (keeper.timer)
                GLib.source_remove(keeper.timer);
            this._cookie = keeper.cookie;
            this._runStart = keeper.started / 1e6;
            keeper = null;
            this._grace = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, GRACE_SECONDS, () => {
                this._grace = 0;
                this._syncAwake();
                return GLib.SOURCE_REMOVE;
            });
        }
        listenToMoving((kind, key, info) => this._reported(kind, key, info));
        for (const id of this._flatpaks?.() ?? [])
            this._reported('flatpak', id, {});
        this._watchFiles();
        this._watchBackup();
        // Heard only while this runs: the phone link connects to no phone's Transfer otherwise
        this._unwatchTransfers = this._connect?.watchTransfers?.((...args) => this._transfer(...args)) ?? null;
        // A phone link that stops, or a phone that goes, ends what was moving to it
        this._unlistenPhones = this._connect?.onPhonesChanged?.(() => this._phonesChanged()) ?? null;
        // A name is not said once the screen is shared, nor kept in the pill when the sharing starts
        this._unlistenCapture = this._capture?.listen(() => this._show()) ?? null;
    }

    disable() {
        listenToMoving(null);
        this._unwatchTransfers?.();
        this._unwatchTransfers = null;
        this._unlistenPhones?.();
        this._unlistenPhones = null;
        this._unlistenCapture?.();
        this._unlistenCapture = null;
        this._unsubscribe?.();
        this._unsubscribe = null;
        this._cancellable?.cancel();
        this._cancellable = null;
        for (const id of [this._filesAdded, this._filesRemoved, this._backupSignal]) {
            if (id)
                Gio.DBus.session.signal_unsubscribe(id);
        }
        this._filesAdded = this._filesRemoved = this._backupSignal = 0;
        for (const name of ['_soon', '_look', '_grace']) {
            if (this[name])
                GLib.source_remove(this[name]);
            this[name] = 0;
        }
        this._stopBattery();
        // Locked with something still on its way: the keeper holds on
        if (this._cookie && locked() && this._items.size > 0 && !this._tooLong) {
            keeper = {
                cookie: this._cookie,
                since: GLib.get_monotonic_time(),
                started: (this._runStart || mono()) * 1e6,
                items: [...this._items.values()].map(({kind, path}) => ({kind, path})),
                timer: 0,
            };
            keeper.timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, LOOK_SECONDS, () => {
                keeperLook();
                return keeper ? GLib.SOURCE_CONTINUE : GLib.SOURCE_REMOVE;
            });
        } else if (this._cookie) {
            letGo(this._cookie);
        }
        this._cookie = 0;
        this._asking = false;
        this._items = null;
        this._gone = null;
        this._runStart = 0;
        this._tooLong = false;
        this._gaveWay = false;
        this._end();
    }

    // --- What is moving -------------------------------------------------------------------------------------------

    // item: what the source says, or null when it is over. Every item has when it first came (since) and when its source last
    // said anything (seen), which is how it ages out (movinglist.js agedOut)
    _set(key, item) {
        const old = this._items.get(key);
        const now = mono();
        if (item) {
            item.since = old?.since ?? now;
            item.seen = now;
            this._items.set(key, item);
            this._arm();
        } else {
            this._items.delete(key);
        }
        // The pill says a change at most once a second, an arrival or an end included: a program that makes and removes files
        // in Downloads all day redraws the bar once a second, not once for each
        if (mono() - (this._shownAt ?? 0) >= COALESCE_MS / 1000)
            this._show();
        else
            this._showSoon();
        this._syncAwake();
    }

    _reported(kind, key, info) {
        if (!this._items)
            return;
        const id = `${kind}:${key}`;
        if (!info) {
            // The dock stops before the pill when the screen locks, and says as it does that nothing updates any more: the
            // update goes on, and the keeper is told so
            if (kind === 'flatpak' && locked())
                return;
            this._set(id, null);
            return;
        }
        if (kind === 'download') {
            // One written to again is only one that still comes: nothing to say
            const known = this._items.get(id);
            if (known)
                known.seen = mono();
            else
                this._set(id, {kind, path: info.path, name: plain(partialName(key), 60)});
        } else if (kind === 'flatpak') {
            const app = Shell.AppSystem.get_default().lookup_app(key);
            this._set(id, {kind, name: plain(app?.get_name() ?? key.replace(/\.desktop$/, ''), 60)});
        }
    }

    // Once a minute while anything moves: what has gone quiet or has been there too long is over, and the machine is kept
    // awake for no more than HOLD_LONGEST in all, whatever is added meanwhile
    _arm() {
        if (this._look)
            return;
        this._look = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, LOOK_SECONDS, () => {
            if (!this._items) {
                this._look = 0;
                return GLib.SOURCE_REMOVE;
            }
            const now = mono();
            for (const [key, item] of [...this._items]) {
                if (agedOut(item, now))
                    this._set(key, null);
            }
            if (this._runStart && now - this._runStart > HOLD_LONGEST && !this._tooLong) {
                this._tooLong = true;
                this._syncAwake();
            }
            if (this._items.size === 0) {
                this._look = 0;
                return GLib.SOURCE_REMOVE;
            }
            return GLib.SOURCE_CONTINUE;
        });
    }

    // The phone link stopped or a phone went: what was moving to one that is not there any more is over
    _phonesChanged() {
        if (!this._items)
            return;
        const here = new Set((this._connect?.sendable ?? []).map(device => device.id));
        for (const [key, item] of [...this._items]) {
            if (item.kind === 'phone' && !here.has(item.device))
                this._set(key, null);
        }
    }

    // Files holds an inhibitor for as long as it copies, moves or empties the trash
    _watchFiles() {
        const bus = Gio.DBus.session;
        this._filesAdded = bus.signal_subscribe(null, SESSION, 'InhibitorAdded', SESSION_PATH, null, Gio.DBusSignalFlags.NONE,
            (c, s, p, i, n, params) => this._inhibitor(params.deepUnpack()[0]));
        this._filesRemoved = bus.signal_subscribe(null, SESSION, 'InhibitorRemoved', SESSION_PATH, null, Gio.DBusSignalFlags.NONE,
            (c, s, p, i, n, params) => {
                const path = params.deepUnpack()[0];
                if (!this._items)
                    return;
                // One removed while it is still being asked about must not be shown when the answers come
                this._gone.add(path);
                if (this._gone.size > MOST_GONE)
                    this._gone.delete(this._gone.values().next().value);
                if (this._items.has(`files:${path}`))
                    this._set(`files:${path}`, null);
            });
        call(bus, SESSION, SESSION_PATH, SESSION, 'GetInhibitors', null, '(ao)', this._cancellable, value => {
            for (const path of value?.[0] ?? [])
                this._inhibitor(path);
        });
    }

    _inhibitor(path) {
        if (!this._items || this._items.has(`files:${path}`))
            return;
        this._gone.delete(path);
        const ask = (method, type, done) => call(Gio.DBus.session, SESSION, path, INHIBITOR, method, null, type, this._cancellable, done);
        ask('GetAppId', '(s)', app => {
            // On GNOME 51 Files asks through the portal, whose backend passes it on with the app's id. Only that id is Files:
            // what some other program, or one the portal could not name, says as its reason is never shown under Files' name
            if (!app || !FILES.includes(app[0]) || !this._items)
                return;
            ask('GetFlags', '(u)', flags => {
                if (!flags || !(flags[0] & SUSPEND) || (flags[0] & IDLE) || !this._items)
                    return;
                ask('GetReason', '(s)', reason => {
                    // An inhibitor that is gone by now answers nothing: it is no copy that goes on
                    if (!reason || !this._items || this._gone.has(path))
                        return;
                    this._set(`files:${path}`, {kind: 'files', name: plain(reason[0], 60) || _('Copying files')});
                });
            });
        });
    }

    // The backup, by its unit's own signals: asking once, and then told, without a timer
    _watchBackup() {
        // The manager says a unit's changes only to a connection that subscribed: shared, and given back when this stops
        this._unsubscribe = subscribeUnits();
        askBackup((path, state) => {
            if (!path || !this._items)
                return;
            this._backupState(state);
            this._backupSignal = Gio.DBus.session.signal_subscribe(SYSTEMD, 'org.freedesktop.DBus.Properties', 'PropertiesChanged', path,
                `${SYSTEMD}.Unit`, Gio.DBusSignalFlags.NONE, (c, s, p, i, n, params) => {
                    const state2 = params.deepUnpack()[1]?.ActiveState?.deepUnpack();
                    if (state2)
                        this._backupState(state2);
                });
        });
    }

    _backupState(state) {
        const going = state === 'active' || state === 'activating';
        if (going !== this._items.has('backup'))
            this._set('backup', going ? {kind: 'backup', name: _('Backup')} : null);
    }

    // The phone link: a transfer's own signal. Its name is a file's, never a path, and it is cleaned as text.
    // The program tells the shell of files going out only (a file coming in is the app's alone, apps/connect dbus.rs), so
    // everything here is going to a phone
    _transfer(device, id, name, incoming, done, total, state) {
        if (!this._items)
            return;
        const key = `phone:${device?.id}:${id}`;
        if (state === 'started' || state === 'progress') {
            this._set(key, {
                kind: 'phone',
                device: device?.id,
                name: plain(name, 60),
                done: Number(done),
                total: Number(total),
            });
        } else {
            this._set(key, null);
        }
    }

    // --- Staying awake ---------------------------------------------------------------------------------------------

    get awake() {
        return this._holding() && !!this._cookie;
    }

    _holding() {
        return !!this._items && this._items.size > 0 && this._awake() && !this._gaveWay && !this._tooLong;
    }

    _syncAwake() {
        if (!this._items)
            return;
        // A new run asks the battery afresh, and counts its twelve hours from here
        if (this._items.size === 0) {
            this._gaveWay = false;
            this._tooLong = false;
            this._runStart = 0;
        } else if (!this._runStart) {
            this._runStart = mono();
        }
        if (this._items.size > 0)
            this._startBattery();
        else
            this._stopBattery();
        if (this._holding() && !this._cookie && !this._asking) {
            this._asking = true;
            call(Gio.DBus.session, SESSION, SESSION_PATH, SESSION, 'Inhibit',
                new GLib.Variant('(susu)', [APP_ID, 0, _('Something is still being copied or downloaded'), SUSPEND]), '(u)', null, value => {
                    // The module may have gone meanwhile: what was given is given back
                    if (!this._items) {
                        if (value)
                            letGo(value[0]);
                        return;
                    }
                    this._asking = false;
                    if (!value)
                        return;
                    this._cookie = value[0];
                    this._syncAwake();
                    this._showSoon();
                });
        } else if (!this._holding() && this._cookie && !this._grace) {
            letGo(this._cookie);
            this._cookie = 0;
            this._showSoon();
        }
    }

    // The computer's own battery, followed only while something moves
    _startBattery() {
        if (this._battery || !this._cancellable)
            return;
        this._battery = true;
        Gio.DBusProxy.new(Gio.DBus.system, Gio.DBusProxyFlags.DO_NOT_AUTO_START, null, UPOWER, DISPLAY, `${UPOWER}.Device`, this._cancellable,
            (source, result) => {
                let proxy;
                try {
                    proxy = Gio.DBusProxy.new_finish(result);
                } catch {
                    return;
                }
                if (this._battery !== true)
                    return;
                this._battery = proxy;
                proxy.connectObject('g-properties-changed', () => this._batteryChanged(), this);
                this._batteryChanged();
            });
    }

    _stopBattery() {
        if (this._battery && this._battery !== true)
            this._battery.disconnectObject(this);
        this._battery = null;
    }

    _batteryChanged() {
        const read = name => this._battery?.get_cached_property?.(name)?.deepUnpack();
        // Both ways: it gives way when the battery runs low, and takes hold again once the computer is charging
        const gives = givesWay(read('Percentage'), read('State'));
        if (gives !== !!this._gaveWay) {
            this._gaveWay = gives;
            this._syncAwake();
        }
    }

    // --- In the pill and the card ---------------------------------------------------------------------------------

    _label(item) {
        switch (item.kind) {
        case 'download': return _('Downloads');
        case 'flatpak': return _('Flatpak update');
        case 'files': return _('Files');
        case 'backup': return _('Your backup');
        case 'phone': return _('To your phone');
        }
        return '';
    }

    // What an item is called in the pill and the card: its name, but not while the screen is shared or recorded
    shownName(item) {
        return this._capture?.active ? this._label(item) : item.name;
    }

    get _fraction() {
        return this._items ? fractionOf(this._items.values()) : null;
    }

    get _percent() {
        const fraction = this._fraction;
        return fraction === null ? '' : `${Math.round(fraction * 100)}%`;
    }

    // A name only while nobody else is watching the screen
    get text() {
        const count = this._items?.size ?? 0;
        const percent = this._percent;
        let words;
        if (count === 1 && !this._capture?.active)
            words = this.shownName([...this._items.values()][0]);
        else
            words = ngettext('%d moving', '%d moving', count).format(count);
        return percent ? `${words} · ${percent}` : words;
    }

    get short() {
        return this._percent || `${this._items?.size ?? 0}`;
    }

    get accessible() {
        const count = this._items?.size ?? 0;
        return ngettext('%d thing moving', '%d things moving', count).format(count);
    }

    get row() {
        return {icon: 'folder-download-symbolic', text: this.text};
    }

    // The ring: how far it has got, or whole while nothing can say
    get progress() {
        return this._fraction ?? 1;
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-moving'});
            this._content.add_child(new St.Icon({icon_name: 'folder-download-symbolic', icon_size: 14, y_align: Clutter.ActorAlign.CENTER}));
            this._labelActor = new St.Label({style_class: 'amethystora-pill-moving-text', y_align: Clutter.ActorAlign.CENTER});
            this._labelActor.clutter_text.ellipsize = Pango.EllipsizeMode.MIDDLE;
            this._content.add_child(this._labelActor);
            this._awakeActor = new St.Label({style_class: 'amethystora-pill-moving-awake', y_align: Clutter.ActorAlign.CENTER});
            this._content.add_child(this._awakeActor);
        }
        return this._content;
    }

    _showSoon() {
        if (this._soon)
            return;
        this._soon = GLib.timeout_add(GLib.PRIORITY_DEFAULT, COALESCE_MS, () => {
            this._soon = 0;
            this._show();
            return GLib.SOURCE_REMOVE;
        });
    }

    _show() {
        if (!this._items)
            return;
        this._shownAt = mono();
        if (this._items.size === 0) {
            this._end();
            return;
        }
        this.pill();
        this._labelActor.text = this.text;
        this._awakeActor.text = this.awake ? _('· staying awake') : '';
        this._awakeActor.visible = this.awake;
        this._activities.set('moving', this);
        this._activities.update('moving');
        this._card?.update();
    }

    _end() {
        this._card = null;
        if (this._activities.get('moving') === this)
            this._activities.set('moving', null);
        retire(this._content);
        this._content = null;
        this._labelActor = this._awakeActor = null;
    }

    card() {
        const card = new MovingCard(this);
        this._card = card;
        card.connect('destroy', () => {
            if (this._card === card)
                this._card = null;
        });
        return card;
    }

    get items() {
        return [...this._items?.values() ?? []];
    }

    // What the card says of one, in words
    detail(item) {
        const label = this._label(item);
        if (!(item.total > 0))
            return label;
        return `${label} · ${Math.round(Math.min(item.done / item.total, 1) * 100)}%`;
    }

    state() {
        return {count: this._items?.size ?? 0, awake: this.awake, kinds: this.items.map(item => item.kind), fraction: this._fraction};
    }

    // For the harness: as if a part said this
    reportForTest(kind, key, info) {
        this._reported(kind, key, info);
    }
}

// The card: each thing moving, in two lines, and whether the computer stays awake for it
const MovingCard = GObject.registerClass(
class AmethystoraPillMovingCard extends St.BoxLayout {
    _init(moving) {
        super._init({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        this._moving = moving;
        this.add_child(new St.Label({style_class: 'amethystora-pill-card-label', text: _('Moving now')}));
        this._rows = new St.BoxLayout({style_class: 'amethystora-pill-moving-rows', orientation: Clutter.Orientation.VERTICAL});
        this.add_child(this._rows);
        this._note = new St.Label({style_class: 'amethystora-pill-card-text'});
        this._note.clutter_text.line_wrap = true;
        this.add_child(this._note);
        this.update();
    }

    update() {
        this._rows.destroy_all_children();
        const items = this._moving.items;
        for (const item of items.slice(0, MOST_ROWS)) {
            const row = new St.BoxLayout({style_class: 'amethystora-pill-moving-row', orientation: Clutter.Orientation.VERTICAL});
            const name = new St.Label({style_class: 'amethystora-pill-moving-name', text: this._moving.shownName(item)});
            name.clutter_text.ellipsize = Pango.EllipsizeMode.MIDDLE;
            row.add_child(name);
            row.add_child(new St.Label({style_class: 'amethystora-pill-card-small', text: this._moving.detail(item)}));
            this._rows.add_child(row);
        }
        if (items.length > MOST_ROWS) {
            const more = items.length - MOST_ROWS;
            this._rows.add_child(new St.Label({
                style_class: 'amethystora-pill-card-small',
                text: ngettext('and %d more', 'and %d more', more).format(more),
            }));
        }
        this._note.text = _('The screen can go dark. The computer stays awake until this is done, then sleeps as usual.');
        this._note.visible = this._moving.awake;
    }
});
