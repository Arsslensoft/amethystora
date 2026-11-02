// A flat battery loses nothing (R7-54, plans/ui-ideas/r7/spec-sturdy.md 1): at 3% it does not die mid-sentence. A card says
// it closes in 60 seconds, the apps and their places are kept at once (workspaces/resume.js, kind `battery`), and when the
// time is up the computer powers off through logind; they are offered back at the next start. It closes rather than
// hibernates, since kernel lockdown refuses hibernation. Plugging in ends it, and Not yet holds it until the charge falls
// further. It cannot save an app's work for it: the 60 seconds are for the person.
//
// The card takes no keyboard: a menu would swallow what the person is typing, and Space could press its button. Not yet has
// a key of its own, bound only while the card shows (flat-hold), so that nobody needs a pointer to stop it.
// While the screen is locked the extension is off, so a lock on battery keeps the apps (Resume.beforeDisable) and UPower's
// own action at 2% stays what closes the computer. Off (ame desktop power flat off): nothing is connected or made.
//
// One UPower proxy's property changes, and a timer of one second only while the card shows.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import UPower from 'gi://UPowerGlib';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {makeCard} from '../desks/card.js';
import {FlatMark} from '../pill/flat.js';
import {COUNTDOWN, TYPE_BATTERY, flatState, isAbove} from './flatdata.js';

const BUS_NAME = 'org.freedesktop.UPower';
const DISPLAY_DEVICE = '/org/freedesktop/UPower/devices/DisplayDevice';
const DEVICE_XML = `<node>
  <interface name="org.freedesktop.UPower.Device">
    <property name="Type" type="u" access="read"/>
    <property name="State" type="u" access="read"/>
    <property name="Percentage" type="d" access="read"/>
    <property name="WarningLevel" type="u" access="read"/>
    <property name="IsPresent" type="b" access="read"/>
  </interface>
</node>`;
const DeviceProxy = Gio.DBusProxy.makeProxyWrapper(DEVICE_XML);

// Kept while GNOME Shell runs, across the disable and enable of every lock: whether the battery has been seen with charge
// in hand, the percent Not yet was said at, and whether a closing that was refused left the apps' places kept (they go
// when the computer is plugged in)
let seenAbove = false;
let heldAt = null;
let keptAfterRefusal = false;

const HOLD_KEY = 'flat-hold';

export class FlatBattery {
    constructor(ext, settings) {
        this._ext = ext;
        this._settings = settings;
    }

    enable() {
        this._cancellable = new Gio.Cancellable();
        this._proxy = new DeviceProxy(Gio.DBus.system, BUS_NAME, DISPLAY_DEVICE, (proxy, error) => {
            if (error || !this._cancellable || this._cancellable.is_cancelled())
                return;
            proxy.connectObject('g-properties-changed', () => this.sync(), this);
            this.sync();
        }, this._cancellable, Gio.DBusProxyFlags.DO_NOT_AUTO_START);
    }

    disable() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._proxy?.disconnectObject(this);
        this._proxy = null;
        // A lock only takes the card away: what it kept stays until the unlock (Resume); else it is forgotten
        this._end({forget: !Main.sessionMode.isLocked});
    }

    get _resume() {
        return this._ext.modules?.get('workspaces')?.resume ?? null;
    }

    get _pill() {
        return this._ext.modules?.get('pill') ?? null;
    }

    reading() {
        const proxy = this._proxy;
        return {present: proxy?.IsPresent, type: proxy?.Type, state: proxy?.State, percentage: proxy?.Percentage,
            warning: proxy?.WarningLevel ?? 0};
    }

    // Running on the computer's own battery now, for what a lock keeps (a desktop on a UPS has none to run flat)
    get discharging() {
        return this._proxy?.IsPresent === true && this._proxy.Type === TYPE_BATTERY &&
            this._proxy.State === UPower.DeviceState.DISCHARGING;
    }

    sync(reading = this.reading()) {
        if (isAbove(reading))
            seenAbove = true;
        const {state, percentage} = reading;
        this._percentage = percentage;
        // A closing that was refused or cancelled kept the places for a battery that might die: plugging in ends that
        if (keptAfterRefusal && state !== UPower.DeviceState.DISCHARGING) {
            keptAfterRefusal = false;
            this._resume?.forgetKept();
        }
        if (state !== UPower.DeviceState.DISCHARGING || Math.floor(percentage) > heldAt)
            heldAt = null;
        if (flatState({...reading, seenAbove, heldAt}) === 'flat')
            this._begin(Math.round(percentage));
        else if (this._card && !this._closing)
            this._end({forget: true});
    }

    _begin(percent) {
        if (this._card) {
            if (this._mark?.setPercent(percent))
                this._pill?.flatChanged();
            return;
        }
        this._left = COUNTDOWN;
        this._closing = false;
        keptAfterRefusal = false;
        // At once: the battery may not last the minute
        this._resume?.keep('battery');
        this._card = makeCard({
            title: this._title(),
            has: '',
            text: this._resume
                ? _('Save anything you need. Your apps come back, each in its place, when you plug in and start up.')
                : _('Save anything you need, or plug in to stop this.'),
            rows: [],
            buttons: [
                [_('Not yet'), false, () => this._notYet()],
                [_('Close now'), true, () => this.close()],
            ],
            note: null,
        });
        const monitor = Main.layoutManager.primaryMonitor;
        const [, width] = this._card.get_preferred_width(-1);
        this._card.x = Math.round(monitor.x + (monitor.width - width) / 2);
        // A game or a video full screen must show it too
        global.compositor.disable_unredirect();
        this._redirectOff = true;
        this._mark = new FlatMark(percent);
        this._pill?.setFlat(this._mark);
        this._glass();
        this._bindHold();
        this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 1, () => this._tick());
    }

    // Not yet from the keyboard, only while the card shows
    _bindHold() {
        if (this._holdBound || !this._settings)
            return;
        Main.wm.addKeybinding(HOLD_KEY, this._settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW | Shell.ActionMode.POPUP, () => this._notYet());
        this._holdBound = true;
    }

    _unbindHold() {
        if (!this._holdBound)
            return;
        Main.wm.removeKeybinding(HOLD_KEY);
        this._holdBound = false;
    }

    _title() {
        return ngettext('Closing safely in %d second', 'Closing safely in %d seconds', this._left).format(this._left);
    }

    _tick() {
        this._left--;
        if (this._left <= 0) {
            this._timer = 0;
            this.close();
            return GLib.SOURCE_REMOVE;
        }
        this._card?.setTitle(this._title());
        return GLib.SOURCE_CONTINUE;
    }

    _end({forget}) {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        this._closing = false;
        this._unbindHold();
        if (this._redirectOff)
            global.compositor.enable_unredirect();
        this._redirectOff = false;
        this._pill?.setFlat(null);
        this._mark?.destroy();
        this._mark = null;
        const had = !!this._card;
        this._card?.destroy();
        this._card = null;
        if (!had)
            return;
        this._glass();
        if (forget) {
            keptAfterRefusal = false;
            this._resume?.forgetKept();
        }
    }

    // Not yet: until the charge falls further. UPower's own action at 2% is not ours to hold.
    _notYet() {
        heldAt = Math.floor(this._percentage);
        this._end({forget: true});
    }

    // What is open now is what is kept, then logind powers off. It does not save an app's own work: that is what the
    // minute was for. Refused (an app holding the logout, somebody else logged in), GNOME's own dialog names what holds it
    // and offers to go on; what was kept then stays only until the computer is plugged in (keptAfterRefusal).
    async close() {
        if (this._closing)
            return;
        this._closing = true;
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        this._card?.setTitle(_('Closing'));
        await this._resume?.keep('battery');
        try {
            await (this._powerOff ?? powerOff)();
        } catch {
            heldAt = Math.floor(this._percentage);
            keptAfterRefusal = true;
            this._end({forget: false});
            (this._ask ?? askSession)().catch(e => console.warn(`Amethystora UI: the computer could not be closed: ${e.message}`));
        }
    }

    glassSurfaces() {
        return this._card ? [{actor: this._card, material: 'glass', radius: 22}] : [];
    }

    _glass() {
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    // --- For the harness and the real machine's checks ------------------------------------------------------------

    state() {
        return {card: !!this._card, left: this._card ? this._left : null, closing: !!this._closing, seenAbove, heldAt,
            keptAfterRefusal, holdBound: !!this._holdBound};
    }

    // Stand-ins for the two ways out; forgets what the module-scope state holds
    testWith({off = null, ask = null} = {}) {
        this._powerOff = off;
        this._ask = ask;
        seenAbove = false;
        heldAt = null;
        keptAfterRefusal = false;
    }

    get card() {
        return this._card ?? null;
    }
}

function powerOff() {
    return callAsync(Gio.DBus.system, 'org.freedesktop.login1', '/org/freedesktop/login1',
        'org.freedesktop.login1.Manager', 'PowerOff', new GLib.Variant('(b)', [false]));
}

function askSession() {
    return callAsync(Gio.DBus.session, 'org.gnome.SessionManager', '/org/gnome/SessionManager',
        'org.gnome.SessionManager', 'Shutdown', null);
}

function callAsync(bus, name, path, iface, method, args) {
    return new Promise((resolve, reject) => {
        bus.call(name, path, iface, method, args, null, Gio.DBusCallFlags.NONE, -1, null, (connection, result) => {
            try {
                connection.call_finish(result);
                resolve();
            } catch (e) {
                reject(e);
            }
        });
    });
}
