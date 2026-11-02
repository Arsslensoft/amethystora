// Your devices' batteries: the battery tile's menu in quick settings lists every device that says how full its battery
// is (a mouse, a keyboard, headphones, a game controller, a pen, and the paired phone from the phone link) beside the
// computer's own, and a notice says once when one is at 15 % or less, so that a mouse does not die in the middle of a
// meeting. On a computer without a battery of its own the tile shows while such a device is there, as Batteries.
//
// All of it is what UPower already knows, followed by UPower's own signals (a device added or removed, a device's
// properties changed); nothing is asked on a timer. The phone's battery is the phone link's (modules/connect.js). The
// menu's rows are made while it is open. Which devices were already said is kept outside the class, so that the lock,
// which turns the extension off and on again, does not say them twice.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import St from 'gi://St';
import UPower from 'gi://UPowerGlib';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {plain} from '../pill/text.js';
import {formatPercent} from '../quick/tiles.js';
import {shareBar} from './battery.js';

const BUS_NAME = 'org.freedesktop.UPower';
const UPOWER_PATH = '/org/freedesktop/UPower';
const DISPLAY_DEVICE = '/org/freedesktop/UPower/devices/DisplayDevice';
const DEVICE_XML = `<node>
  <interface name="org.freedesktop.UPower.Device">
    <property name="Type" type="u" access="read"/>
    <property name="Model" type="s" access="read"/>
    <property name="State" type="u" access="read"/>
    <property name="Percentage" type="d" access="read"/>
    <property name="BatteryLevel" type="u" access="read"/>
    <property name="IsPresent" type="b" access="read"/>
    <property name="PowerSupply" type="b" access="read"/>
    <property name="IconName" type="s" access="read"/>
  </interface>
</node>`;
const UPOWER_XML = `<node>
  <interface name="org.freedesktop.UPower">
    <method name="EnumerateDevices">
      <arg name="devices" type="ao" direction="out"/>
    </method>
    <signal name="DeviceAdded">
      <arg name="device" type="o"/>
    </signal>
    <signal name="DeviceRemoved">
      <arg name="device" type="o"/>
    </signal>
  </interface>
</node>`;
const DeviceProxy = Gio.DBusProxy.makeProxyWrapper(DEVICE_XML);
const UPowerProxy = Gio.DBusProxy.makeProxyWrapper(UPOWER_XML);
// At this charge or less a device is said, once; it is said again only after it charged above AGAIN
const LOW = 15;
const AGAIN = 20;
// A row says "charge it soon" under this
const SOON = 20;
const PATH = /^\/org\/freedesktop\/UPower\/devices\/[A-Za-z0-9_]+$/;

// The devices already said to be low, by their object path (or phone:NAME), kept across the lock
const told = new Set();

// The kind of a device, in words
function kindName(kind) {
    switch (kind) {
    case UPower.DeviceKind.MOUSE: return _('Mouse');
    case UPower.DeviceKind.KEYBOARD: return _('Keyboard');
    case UPower.DeviceKind.PHONE: return _('Phone');
    case UPower.DeviceKind.MEDIA_PLAYER: return _('Media player');
    case UPower.DeviceKind.TABLET: return _('Tablet');
    case UPower.DeviceKind.COMPUTER: return _('Computer');
    case UPower.DeviceKind.GAMING_INPUT: return _('Controller');
    case UPower.DeviceKind.PEN: return _('Pen');
    case UPower.DeviceKind.TOUCHPAD: return _('Touchpad');
    case UPower.DeviceKind.HEADSET: return _('Headset');
    case UPower.DeviceKind.SPEAKERS: return _('Speaker');
    case UPower.DeviceKind.HEADPHONES: return _('Headphones');
    case UPower.DeviceKind.OTHER_AUDIO: return _('Audio device');
    case UPower.DeviceKind.REMOTE_CONTROL: return _('Remote');
    case UPower.DeviceKind.WEARABLE: return _('Wearable');
    case UPower.DeviceKind.UPS: return _('Power backup');
    default: return _('Device');
    }
}

function kindIcon(kind) {
    switch (kind) {
    case UPower.DeviceKind.MOUSE: return 'input-mouse-symbolic';
    case UPower.DeviceKind.KEYBOARD: return 'input-keyboard-symbolic';
    case UPower.DeviceKind.PHONE: return 'smartphone-symbolic';
    case UPower.DeviceKind.GAMING_INPUT: return 'input-gaming-symbolic';
    case UPower.DeviceKind.PEN: return 'input-tablet-symbolic';
    case UPower.DeviceKind.TOUCHPAD: return 'input-touchpad-symbolic';
    case UPower.DeviceKind.HEADSET: return 'audio-headset-symbolic';
    case UPower.DeviceKind.HEADPHONES: return 'audio-headphones-symbolic';
    case UPower.DeviceKind.SPEAKERS: return 'audio-speakers-symbolic';
    default: return 'battery-symbolic';
    }
}

// A device worth a row: one with a battery that is not the computer's own (those are the tile's), present, with a
// percentage it reports
export function reading(path, d) {
    const kind = d.Type;
    if (!Number.isInteger(kind) || kind === UPower.DeviceKind.LINE_POWER || kind === UPower.DeviceKind.UNKNOWN)
        return null;
    if (kind === UPower.DeviceKind.BATTERY && d.PowerSupply === true)
        return null;
    if (d.IsPresent === false || typeof d.Percentage !== 'number' || d.Percentage <= 0)
        return null;
    return {
        id: path,
        kind,
        label: kindName(kind),
        // A device's model is its maker's text, said as text only, and short
        model: plain(d.Model ?? '', 40),
        percent: Math.max(0, Math.min(100, d.Percentage)),
        charging: d.State === UPower.DeviceState.CHARGING || d.State === UPower.DeviceState.FULLY_CHARGED,
        icon: kindIcon(kind),
    };
}

export class DeviceBatteries {
    constructor(quick, connect) {
        this._quick = quick;
        this._connect = connect;
    }

    enable() {
        this._menu = this._quick?.batteryMenu?.() ?? null;
        if (!this._menu)
            return;
        this._devices = new Map();
        this._cancellable = new Gio.Cancellable();
        // The proxy may finish after the module stopped and disposed it: only the one still kept is used
        this._manager = new UPowerProxy(Gio.DBus.system, BUS_NAME, UPOWER_PATH, (proxy, error) => {
            if (error || proxy !== this._manager)
                return;
            proxy.connectSignal('DeviceAdded', (p, sender, [path]) => this._add(path));
            proxy.connectSignal('DeviceRemoved', (p, sender, [path]) => this._remove(path));
            proxy.EnumerateDevicesAsync().then(([paths]) => {
                if (proxy === this._manager)
                    paths.forEach(path => this._add(path));
            }).catch(() => {});
        }, this._cancellable, Gio.DBusProxyFlags.DO_NOT_AUTO_START);
        this._display = new DeviceProxy(Gio.DBus.system, BUS_NAME, DISPLAY_DEVICE, () => {}, this._cancellable,
            Gio.DBusProxyFlags.DO_NOT_AUTO_START);
        this._unlisten = this._connect?.onPhonesChanged?.(() => this._changed()) ?? null;
        this._menu.connectObject('open-state-changed', (menu, open) => (open ? this._open() : this._close()), this);
        this._changed();
    }

    disable() {
        this._cancellable?.cancel();
        this._cancellable = null;
        // The proxies' signal handlers go with them
        for (const device of this._devices?.values() ?? [])
            device.disconnectObject(this);
        this._devices?.clear();
        this._devices = null;
        this._manager?.run_dispose();
        this._manager = null;
        this._display = null;
        this._unlisten?.();
        this._unlisten = null;
        this._menu?.disconnectObject(this);
        this._close();
        this._menu = null;
        this._quick?.setOtherBatteries?.(null);
    }

    _add(path) {
        if (!PATH.test(path) || path === DISPLAY_DEVICE || this._devices?.has(path))
            return;
        const device = new DeviceProxy(Gio.DBus.system, BUS_NAME, path, (proxy, error) => {
            if (error || !this._devices?.has(path))
                return;
            proxy.connectObject('g-properties-changed', () => this._changed(), this);
            this._changed();
        }, this._cancellable, Gio.DBusProxyFlags.DO_NOT_AUTO_START);
        this._devices.set(path, device);
    }

    _remove(path) {
        const device = this._devices?.get(path);
        if (!device)
            return;
        device.disconnectObject(this);
        this._devices.delete(path);
        told.delete(path);
        this._changed();
    }

    // Every device's reading, the phones' after UPower's (a phone UPower also knows is the phone link's)
    readings() {
        if (!this._devices)
            return [];
        const phones = (this._connect?.phones?.() ?? []).filter(phone => phone.level >= 0);
        const list = [];
        for (const [path, device] of this._devices) {
            const r = reading(path, device);
            if (r && !(r.kind === UPower.DeviceKind.PHONE && phones.length > 0))
                list.push(r);
        }
        for (const phone of phones) {
            list.push({id: `phone:${phone.name}`, kind: UPower.DeviceKind.PHONE, label: _('Phone'),
                model: plain(phone.name ?? '', 40), percent: Math.max(0, Math.min(100, phone.level)),
                charging: phone.charging === true, icon: 'smartphone-symbolic'});
        }
        return list.sort((a, b) => a.label.localeCompare(b.label) || a.model.localeCompare(b.model));
    }

    _changed() {
        const list = this.readings();
        // The tile, on a computer without a battery: the lowest device
        const lowest = list.reduce((low, r) => (!low || r.percent < low.percent ? r : low), null);
        this._quick?.setOtherBatteries?.(lowest ? {label: lowest.label, percent: lowest.percent} : null);
        for (const r of list) {
            if (r.percent > AGAIN || r.charging)
                told.delete(r.id);
            else if (r.percent <= LOW && !told.has(r.id))
                this._tell(r);
        }
        if (this._box)
            this._fill();
    }

    _tell(r) {
        told.add(r.id);
        const source = MessageTray.getSystemSource();
        const notification = new MessageTray.Notification({
            source,
            title: _('%s battery at %s').format(r.label, formatPercent(r.percent)),
            body: r.model || _('Charge it soon.'),
            isTransient: false,
        });
        source.addNotification(notification);
    }

    // --- The menu's section ---------------------------------------------------------------------------------------

    _open() {
        if (this._item || this.readings().length === 0)
            return;
        this._item = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false,
            style_class: 'amethystora-power-devices-item'});
        this._box = new St.BoxLayout({style_class: 'amethystora-power-devices', orientation: Clutter.Orientation.VERTICAL,
            x_expand: true});
        this._item.add_child(this._box);
        // Before the menu's line and Power Settings
        const items = this._menu._getMenuItems();
        const line = items.findIndex(item => item instanceof PopupMenu.PopupSeparatorMenuItem);
        this._menu.addMenuItem(this._item, line < 0 ? items.length : line);
        this._fill();
    }

    _close() {
        this._item?.destroy();
        this._item = null;
        this._box = null;
    }

    _fill() {
        const box = this._box;
        box.destroy_all_children();
        box.add_child(new St.Widget({style_class: 'amethystora-power-line'}));
        box.add_child(new St.Label({style_class: 'amethystora-power-head', text: _('Batteries').toLocaleUpperCase()}));
        const rows = [];
        if (this._display?.IsPresent === true && typeof this._display.Percentage === 'number') {
            rows.push({id: 'computer', label: _('This computer'), model: '', percent: this._display.Percentage,
                charging: false, icon: 'computer-symbolic'});
        }
        rows.push(...this.readings());
        for (const r of rows) {
            const row = new St.BoxLayout({style_class: 'amethystora-power-device', orientation: Clutter.Orientation.VERTICAL});
            const line = new St.BoxLayout({style_class: 'amethystora-power-device-line'});
            line.add_child(new St.Icon({icon_name: r.icon, style_class: 'amethystora-power-device-icon',
                y_align: Clutter.ActorAlign.CENTER}));
            const words = new St.BoxLayout({x_expand: true, y_align: Clutter.ActorAlign.CENTER});
            words.add_child(new St.Label({text: r.label}));
            if (r.model)
                words.add_child(new St.Label({style_class: 'amethystora-power-device-model', text: r.model, opacity: 180}));
            if (r.id !== 'computer' && r.percent < SOON && !r.charging)
                words.add_child(new St.Label({style_class: 'amethystora-power-device-soon', text: _('charge it soon')}));
            line.add_child(words);
            line.add_child(new St.Label({style_class: 'amethystora-power-use-share', text: formatPercent(r.percent),
                y_align: Clutter.ActorAlign.CENTER}));
            row.add_child(line);
            row.add_child(shareBar(r.percent / 100));
            box.add_child(row);
        }
    }

    // --- For the suites -------------------------------------------------------------------------------------------

    state() {
        return {
            devices: this.readings().map(({id, label, model, percent, charging}) => ({id, label, model, percent, charging})),
            told: [...told],
            open: !!this._item,
        };
    }

    // As if UPower said a device had this reading (a test needs no mouse): {kind name, model, percent}
    readingForTest({type = UPower.DeviceKind.MOUSE, model = '', percentage = 40, state = UPower.DeviceState.DISCHARGING}) {
        const path = '/org/freedesktop/UPower/devices/amethystora_test';
        const stand = {Type: type, Model: model, Percentage: percentage, State: state, IsPresent: true, PowerSupply: false,
            disconnectObject() {}};
        this._devices?.set(path, stand);
        this._changed();
        return this.state();
    }

    forgetTest() {
        this._remove('/org/freedesktop/UPower/devices/amethystora_test');
        return this.state();
    }
}
