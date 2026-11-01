// The battery, explained: the battery tile's menu in quick settings opens on how full it is and when it will be empty
// or full, the watts it takes or gives now, the apps that used the most since it was plugged in or out, and the
// battery's health. All of it is what UPower already knows, but the apps, which are worked out from the processor time
// each used (an estimate, and the menu says so: the screen, the graphics and apps that came and went are not seen).
//
// The processor time is looked at twice: once when the power is plugged in or out (UPower's state, a D-Bus signal),
// and again when the menu opens. The first look is kept outside the class, so that the lock, which turns the
// extension off and on again, keeps it. Nothing is made while the menu is closed.

import Cairo from 'cairo';
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import St from 'gi://St';
import UPower from 'gi://UPowerGlib';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {hoursMinutes, plain, timeOfDay} from '../pill/text.js';
import {Walker, usedBetween} from './apps.js';

const BUS_NAME = 'org.freedesktop.UPower';
const DISPLAY_DEVICE = '/org/freedesktop/UPower/devices/DisplayDevice';
const DEVICE_XML = `<node>
  <interface name="org.freedesktop.UPower.Device">
    <property name="Type" type="u" access="read"/>
    <property name="State" type="u" access="read"/>
    <property name="Percentage" type="d" access="read"/>
    <property name="TimeToEmpty" type="x" access="read"/>
    <property name="TimeToFull" type="x" access="read"/>
    <property name="EnergyRate" type="d" access="read"/>
    <property name="Energy" type="d" access="read"/>
    <property name="EnergyFull" type="d" access="read"/>
    <property name="EnergyFullDesign" type="d" access="read"/>
    <property name="Capacity" type="d" access="read"/>
    <property name="ChargeCycles" type="i" access="read"/>
    <property name="IsPresent" type="b" access="read"/>
    <property name="PowerSupply" type="b" access="read"/>
    <property name="ChargeThresholdEnabled" type="b" access="read"/>
    <property name="ChargeEndThreshold" type="u" access="read"/>
  </interface>
</node>`;
const UPOWER_XML = `<node>
  <interface name="org.freedesktop.UPower">
    <method name="EnumerateDevices">
      <arg name="devices" type="ao" direction="out"/>
    </method>
  </interface>
</node>`;
const DeviceProxy = Gio.DBusProxy.makeProxyWrapper(DEVICE_XML);
const UPowerProxy = Gio.DBusProxy.makeProxyWrapper(UPOWER_XML);
const TYPE_BATTERY = 2;
// Apps listed before "Everything else"
const APPS = 3;

// The first look, kept across the lock: {charging, since (ms), walk (Map)}
let kept = null;

function charging(state) {
    return state === UPower.DeviceState.CHARGING || state === UPower.DeviceState.PENDING_CHARGE ||
        state === UPower.DeviceState.FULLY_CHARGED;
}

// What the menu says, from UPower's values alone (a pure function, for the suites): {percent, when, how, health}
export function batteryWords({state, percentage, timeToEmpty = 0, timeToFull = 0, rate = 0, since = null,
    limit = null, capacity = 0, full = 0, design = 0, cycles = -1}) {
    const percent = `${Math.round(percentage ?? 0)}%`;
    let when = '';
    if (state === UPower.DeviceState.DISCHARGING && timeToEmpty > 0)
        when = _('%s left').format(hoursMinutes(timeToEmpty));
    else if (state === UPower.DeviceState.CHARGING && timeToFull > 0)
        when = _('Full at %s').format(timeOfDay(new Date(Date.now() + timeToFull * 1000)));
    else if (state === UPower.DeviceState.FULLY_CHARGED)
        when = _('Fully charged');
    // In the session's way of writing numbers (9,8 in French)
    const watts = rate > 0 ? rate.toLocaleString(undefined, {maximumFractionDigits: rate < 10 ? 1 : 0}) : null;
    let how = '';
    if (state === UPower.DeviceState.DISCHARGING) {
        if (watts && since)
            how = _('Using %s W now · unplugged at %s').format(watts, timeOfDay(new Date(since)));
        else if (watts)
            how = _('Using %s W now').format(watts);
    } else if (state === UPower.DeviceState.CHARGING && watts) {
        how = limit
            ? _('Charging at %s W · stops at %d%%').format(watts, limit)
            : _('Charging at %s W').format(watts);
    }
    const health = capacity > 0 && capacity <= 150 ? Math.round(Math.min(capacity, 100))
        : full > 0 && design > 0 ? Math.round(Math.min(100 * full / design, 100)) : null;
    let healthText = null;
    if (health !== null) {
        healthText = cycles > 0
            ? ngettext('%d%% of new · %d cycle', '%d%% of new · %d cycles', cycles).format(health, cycles)
            : _('%d%% of new').format(health);
    }
    return {percent, when, how, health: healthText};
}

export class BatteryDetails {
    constructor(quick) {
        this._quick = quick;
    }

    enable() {
        this._menu = this._quick?.batteryMenu?.() ?? null;
        if (!this._menu)
            return;
        this._cancellable = new Gio.Cancellable();
        this._walker = new Walker();
        this._display = new DeviceProxy(Gio.DBus.system, BUS_NAME, DISPLAY_DEVICE, (proxy, error) => {
            if (error || !this._cancellable || this._cancellable.is_cancelled())
                return;
            proxy.connectObject('g-properties-changed', () => this._stateChanged(), this);
            this._stateChanged();
        }, this._cancellable, Gio.DBusProxyFlags.DO_NOT_AUTO_START);
        this._menu.connectObject('open-state-changed', (menu, open) => (open ? this._open() : this._close()), this);
    }

    disable() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._display?.disconnectObject(this);
        this._display = null;
        this._menu?.disconnectObject(this);
        this._close();
        this._menu = null;
        this._walker?.clear();
        this._walker = null;
        // Kept across the lock only: turned off and on again, the first look is made again
        if (!Main.sessionMode.isLocked)
            kept = null;
    }

    get _present() {
        return this._display?.IsPresent === true;
    }

    // Plugged in or out (or the first time it is known): the first look, kept until the next change
    _stateChanged() {
        if (!this._present)
            return;
        const now = charging(this._display.State);
        if (kept && kept.charging === now)
            return;
        const since = Date.now();
        // The first look the module makes is not a plug or unplug: its time is not said as one
        const plugged = kept !== null;
        kept = {charging: now, since, plugged, walk: null};
        const mine = kept;
        this._walker.walk(this._cancellable).then(walk => {
            if (kept === mine)
                mine.walk = walk;
        }).catch(e => {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                console.warn(`Amethystora UI: the battery could not look at the processes: ${e.message}`);
        });
    }

    // --- The menu's section ---------------------------------------------------------------------------------------

    _open() {
        if (!this._present || this._item)
            return;
        this._item = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false,
            style_class: 'amethystora-power-battery-item'});
        this._box = new St.BoxLayout({style_class: 'amethystora-power-battery', orientation: Clutter.Orientation.VERTICAL,
            x_expand: true});
        this._item.add_child(this._box);
        this._menu.addMenuItem(this._item, 0);
        this._fill(null);
        this._health = null;
        this._readHealth();
        // The second look, now
        if (kept?.walk) {
            const before = kept.walk;
            this._walker.walk(this._cancellable).then(after => {
                if (this._item)
                    this._fill(usedBetween(before, after));
            }).catch(() => {});
        }
    }

    _close() {
        this._healthCancellable?.cancel();
        this._healthCancellable = null;
        this._item?.destroy();
        this._item = null;
        this._box = null;
    }

    // The laptop's own batteries (not the display device, which sums them, nor a mouse's, which powers no machine) say
    // their health: with two, their energies together and no count of cycles, which each counts apart
    _readHealth() {
        const cancellable = this._healthCancellable = new Gio.Cancellable();
        const proxy = (path, made) => new Promise(resolve => {
            const device = made(path, (p, error) => resolve(error ? null : device), cancellable);
        });
        proxy('/org/freedesktop/UPower', (path, ready, c) =>
            new UPowerProxy(Gio.DBus.system, BUS_NAME, path, ready, c, Gio.DBusProxyFlags.DO_NOT_AUTO_START))
            .then(manager => manager?.EnumerateDevicesAsync())
            .then(([paths] = [[]]) => Promise.all(paths.filter(p => p.includes('/battery')).map(path => proxy(path,
                (p, ready, c) => new DeviceProxy(Gio.DBus.system, BUS_NAME, p, ready, c,
                    Gio.DBusProxyFlags.DO_NOT_AUTO_START)))))
            .then(devices => {
                const own = (devices ?? []).filter(b => b?.Type === TYPE_BATTERY && b.PowerSupply === true);
                if (cancellable.is_cancelled() || own.length === 0)
                    return;
                const sum = key => own.reduce((total, b) => total + (b[key] ?? 0), 0);
                this._health = own.length === 1
                    ? {capacity: own[0].Capacity ?? 0, full: own[0].EnergyFull ?? 0,
                        design: own[0].EnergyFullDesign ?? 0, cycles: own[0].ChargeCycles ?? -1}
                    : {capacity: 0, full: sum('EnergyFull'), design: sum('EnergyFullDesign'), cycles: -1};
                this._fill(this._used ?? null);
            }).catch(() => {});
    }

    reading() {
        const d = this._display;
        return {
            state: d?.State, percentage: d?.Percentage ?? 0, timeToEmpty: Number(d?.TimeToEmpty ?? 0),
            timeToFull: Number(d?.TimeToFull ?? 0), rate: d?.EnergyRate ?? 0,
            since: kept?.plugged ? kept.since : null,
            limit: d?.ChargeThresholdEnabled === true ? d?.ChargeEndThreshold ?? null : null,
            ...this._health ?? {},
        };
    }

    _fill(used) {
        const box = this._box;
        if (!box)
            return;
        this._used = used;
        box.destroy_all_children();
        const words = batteryWords(this.reading());

        const head = new St.BoxLayout({style_class: 'amethystora-power-battery-head', orientation: Clutter.Orientation.VERTICAL});
        box.add_child(head);
        const top = new St.BoxLayout({style_class: 'amethystora-power-battery-top'});
        top.add_child(new St.Label({style_class: 'amethystora-power-battery-percent', text: words.percent}));
        top.add_child(new St.Label({style_class: 'amethystora-power-battery-when', text: words.when,
            y_align: Clutter.ActorAlign.END}));
        head.add_child(top);
        if (words.how)
            head.add_child(new St.Label({style_class: 'amethystora-power-battery-how', text: words.how}));

        const shown = this._usedRows(used);
        if (shown) {
            box.add_child(new St.Widget({style_class: 'amethystora-power-line'}));
            for (const actor of shown)
                box.add_child(actor);
        }
        if (words.health) {
            box.add_child(new St.Widget({style_class: 'amethystora-power-line'}));
            const row = new St.BoxLayout({style_class: 'amethystora-power-kv'});
            row.add_child(new St.Label({text: _('Battery health'), x_expand: true}));
            row.add_child(new St.Label({style_class: 'amethystora-power-kv-value', text: words.health}));
            box.add_child(row);
        }
    }

    // The heading, the apps and the note; null when there is nothing to say
    _usedRows(used) {
        if (!kept)
            return null;
        const at = timeOfDay(new Date(kept.since));
        const head = new St.Label({
            style_class: 'amethystora-power-head',
            // A plug or unplug the module saw, else the start of the session, when it first looked
            text: (!kept.plugged ? _('Used the most since you logged in at %s')
                : kept.charging ? _('Used the most since it was plugged in at %s') : _('Used the most since %s'))
                .format(at)
                .toLocaleUpperCase(),
        });
        head.clutter_text.line_wrap = true;
        if (!used) {
            const later = new St.Label({style_class: 'amethystora-power-note',
                text: kept.walk ? _('Measuring…') : _('Measured from the next time you open this')});
            return [head, later];
        }
        if (used.total <= 0 || used.list.length === 0)
            return null;
        const rows = [head];
        const top = used.list.slice(0, APPS);
        const rest = used.total - top.reduce((sum, a) => sum + a.ticks, 0);
        const entries = top.map(a => [plain(a.name, 40), a.ticks / used.total]);
        if (rest > 0)
            entries.push([_('Everything else'), rest / used.total]);
        for (const [name, share] of entries) {
            const row = new St.BoxLayout({style_class: 'amethystora-power-use', orientation: Clutter.Orientation.VERTICAL});
            const line = new St.BoxLayout();
            line.add_child(new St.Label({text: name, x_expand: true}));
            line.add_child(new St.Label({style_class: 'amethystora-power-use-share', text: `${Math.round(share * 100)}%`}));
            row.add_child(line);
            row.add_child(shareBar(share));
            rows.push(row);
        }
        rows.push(new St.Label({style_class: 'amethystora-power-note',
            text: _('Worked out from processor time; the screen is not counted.')}));
        rows[rows.length - 1].clutter_text.line_wrap = true;
        return rows;
    }

    // --- For the suites -------------------------------------------------------------------------------------------

    state() {
        return {
            present: this._present,
            kept: kept ? {charging: kept.charging, since: kept.since, measured: !!kept.walk} : null,
            open: !!this._item,
            texts: this._box ? collect(this._box) : [],
        };
    }

    // As if the power was plugged in or out now
    replugForTest() {
        kept = null;
        this._stateChanged();
        return true;
    }

    // Fills the open menu as UPower would say `reading` (a test needs no battery)
    showForTest(reading, used = null) {
        this.reading = () => reading;
        kept ??= {charging: false, since: reading.since ?? Date.now(), plugged: true, walk: new Map()};
        if (!this._item) {
            this._item = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false,
                style_class: 'amethystora-power-battery-item'});
            this._box = new St.BoxLayout({style_class: 'amethystora-power-battery',
                orientation: Clutter.Orientation.VERTICAL, x_expand: true});
            this._item.add_child(this._box);
            this._menu.addMenuItem(this._item, 0);
        }
        this._fill(used);
        return collect(this._box);
    }
}

// A share as a round bar: the track and, from the start, the fill from the accent to the accent's text colour, drawn
// at the size the bar is given (no relayout for it). The colours are the style's: `color` the fill's end,
// -amethystora-power-from its start, -amethystora-power-track the track.
export function shareBar(share) {
    const bar = new St.DrawingArea({style_class: 'amethystora-power-bar', x_expand: true});
    bar.connect('repaint', area => {
        const cr = area.get_context();
        const [w, h] = area.get_surface_size();
        const node = area.get_theme_node();
        const end = node.get_foreground_color();
        const [hasFrom, from] = node.lookup_color('-amethystora-power-from', false);
        const [hasTrack, track] = node.lookup_color('-amethystora-power-track', false);
        const pill = width => {
            const r = Math.min(h / 2, width / 2);
            cr.newSubPath();
            cr.arc(r, h / 2, r, Math.PI / 2, 3 * Math.PI / 2);
            cr.arc(width - r, h / 2, r, -Math.PI / 2, Math.PI / 2);
            cr.closePath();
        };
        const rgba = c => [c.red / 255, c.green / 255, c.blue / 255, c.alpha / 255];
        if (hasTrack)
            cr.setSourceRGBA(...rgba(track));
        else
            cr.setSourceRGBA(1, 1, 1, 0.08);
        pill(w);
        cr.fill();
        const width = Math.max(Math.round(w * Math.min(Math.max(share, 0), 1)), share > 0 ? h : 0);
        if (width > 0) {
            const gradient = new Cairo.LinearGradient(0, 0, width, 0);
            gradient.addColorStopRGBA(0, ...rgba(hasFrom ? from : end));
            gradient.addColorStopRGBA(1, ...rgba(end));
            cr.setSource(gradient);
            pill(width);
            cr.fill();
        }
        cr.$dispose();
    });
    return bar;
}

function collect(actor) {
    const texts = [];
    const visit = a => {
        if (a instanceof St.Label && a.text)
            texts.push(a.text);
        for (const child of a.get_children())
            visit(child);
    };
    visit(actor);
    return texts;
}
