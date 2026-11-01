// The battery's tile in quick settings, in place of GNOME's battery button: the charge and when it runs out or is full,
// and a menu with "Keep the battery at 80%", GNOME's own charge limit, read and set through UPower's charge threshold.
//
// The limit is set only through UPower's own method on UPower's own battery objects, which asks polkit itself (the
// person at the machine may, nobody else); nothing here writes a file or runs anything as root, and what the switch
// shows is always what UPower then says, never a guess.

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';
import UPower from 'gi://UPowerGlib';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import {formatTime} from 'resource:///org/gnome/shell/misc/dateUtils.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {QuickMenuToggle} from 'resource:///org/gnome/shell/ui/quickSettings.js';

import {formatPercent} from './tiles.js';

const BUS_NAME = 'org.freedesktop.UPower';
const DISPLAY_DEVICE = '/org/freedesktop/UPower/devices/DisplayDevice';
const DEVICE_XML = `<node>
  <interface name="org.freedesktop.UPower.Device">
    <property name="Type" type="u" access="read"/>
    <property name="State" type="u" access="read"/>
    <property name="Percentage" type="d" access="read"/>
    <property name="TimeToEmpty" type="x" access="read"/>
    <property name="TimeToFull" type="x" access="read"/>
    <property name="IsPresent" type="b" access="read"/>
    <property name="IconName" type="s" access="read"/>
    <property name="PowerSupply" type="b" access="read"/>
    <property name="ChargeThresholdEnabled" type="b" access="read"/>
    <property name="ChargeThresholdSupported" type="b" access="read"/>
    <property name="ChargeEndThreshold" type="u" access="read"/>
    <property name="ChargeThresholdSettingsSupported" type="u" access="read"/>
    <method name="EnableChargeThreshold">
      <arg name="charge_threshold" type="b" direction="in"/>
    </method>
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
// UPower's device type for a battery
const TYPE_BATTERY = 2;
// ChargeThresholdSettingsSupported: the end threshold can be set (bit 1); without it the firmware decides alone
const END_SETTABLE = 2;
// How long UPower may take to set the limit
const CALL_TIMEOUT_MS = 25000;
// UPower answers the call before it says the property changed: the switch waits this long for the new value
const SETTLE_MS = 2000;

// The 80% row: a row of the battery's menu whose click (or Space, or Enter) turns the switch in place, so that the
// switch, what UPower then says and an error stay in sight; a menu item would close the menu first
const LimitRow = GObject.registerClass(
class AmethystoraQuickLimitRow extends PopupMenu.PopupBaseMenuItem {
    activate() {
        this.toggle?.();
    }
});

export class BatteryTile {
    constructor() {
        this._batteries = new Map();
        // The lowest of the other devices' batteries ({label, percent}), which shows the tile on a computer without one
        this._others = null;
        this._cancellable = new Gio.Cancellable();

        this.tile = new QuickMenuToggle({
            title: _('Battery'),
            iconName: 'battery-level-100-symbolic',
            toggleMode: false,
            visible: false,
        });
        this.tile._amethystoraQuickId = 'battery';
        // The whole tile opens its menu: there is nothing to switch on or off
        this.tile.connect('clicked', () => this.tile.menu.open());
        this.tile.menu.setHeader('battery-level-100-symbolic', _('Battery'));

        // A switch to a screen reader, checked as the switch is
        this._row = new LimitRow({style_class: 'amethystora-quick-limit'});
        this._row.accessible_role = Atk.Role.CHECK_MENU_ITEM;
        const words = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true,
            y_align: Clutter.ActorAlign.CENTER});
        this._limitTitle = new St.Label({style_class: 'amethystora-quick-limit-title'});
        this._limitText = new St.Label({style_class: 'amethystora-quick-limit-text'});
        this._limitText.clutter_text.line_wrap = true;
        words.add_child(this._limitTitle);
        words.add_child(this._limitText);
        this._row.add_child(words);
        this._switch = new PopupMenu.Switch(false);
        this._row.add_child(this._switch);
        this._switch.connect('notify::state', () => {
            if (this._switch.state)
                this._row.add_accessible_state(Atk.StateType.CHECKED);
            else
                this._row.remove_accessible_state(Atk.StateType.CHECKED);
        });
        this._row.label_actor = this._limitTitle;
        this._row.toggle = () => this.toggleLimit();
        this.tile.menu.addMenuItem(this._row);

        this._error = new St.Label({style_class: 'amethystora-quick-error', visible: false,
            text: _('The charge limit could not be changed.')});
        this._error.clutter_text.line_wrap = true;
        this.tile.menu.box.add_child(this._error);

        this.tile.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this.tile.menu.addSettingsAction(_('Power Settings'), 'gnome-power-panel.desktop');
        this._row.visible = false;

        this._display = new DeviceProxy(Gio.DBus.system, BUS_NAME, DISPLAY_DEVICE, (proxy, error) => {
            if (error) {
                console.debug(`Amethystora UI: UPower's display device: ${error.message}`);
                return;
            }
            this.sync();
        }, this._cancellable);
        this._manager = new UPowerProxy(Gio.DBus.system, BUS_NAME, '/org/freedesktop/UPower', (proxy, error) => {
            if (!error)
                this._enumerate();
        }, this._cancellable);
    }

    // Quick settings opened: the charge now, and its changes while it stays open
    opened() {
        this._open = true;
        this._display?.connectObject('g-properties-changed', () => this.sync(), this);
        for (const battery of this._batteries.values())
            battery.connectObject('g-properties-changed', () => this.sync(), this);
        this._enumerate();
        this.sync();
    }

    closed() {
        this._open = false;
        this._display?.disconnectObject(this);
        for (const battery of this._batteries.values())
            battery.disconnectObject(this);
        this._error.hide();
    }

    // The batteries UPower knows, each read once and followed while quick settings is open
    _enumerate() {
        this._manager?.EnumerateDevicesAsync().then(([paths]) => {
            if (this._cancellable.is_cancelled())
                return;
            for (const [path, battery] of this._batteries) {
                if (!paths.includes(path)) {
                    battery.disconnectObject(this);
                    this._batteries.delete(path);
                }
            }
            for (const path of paths) {
                if (this._batteries.has(path) || !path.includes('/battery'))
                    continue;
                const battery = new DeviceProxy(Gio.DBus.system, BUS_NAME, path, (proxy, error) => {
                    if (error || this._cancellable.is_cancelled())
                        return;
                    if (this._open)
                        proxy.connectObject('g-properties-changed', () => this.sync(), this);
                    this.sync();
                }, this._cancellable);
                this._batteries.set(path, battery);
            }
            this.sync();
        }).catch(e => {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                console.debug(`Amethystora UI: UPower's devices: ${e.message}`);
        });
    }

    // The batteries that can keep their charge below full
    _limitable() {
        return [...this._batteries.values()].filter(battery => battery.Type === TYPE_BATTERY &&
            battery.PowerSupply === true && battery.ChargeThresholdSupported === true);
    }

    get present() {
        return this._display?.IsPresent === true;
    }

    // The end of the charge as a percentage, or null when the laptop decides it alone
    _end() {
        const [first] = this._limitable();
        if (!first)
            return null;
        const end = first.ChargeEndThreshold;
        const settings = first.ChargeThresholdSettingsSupported;
        if (!Number.isInteger(end) || end < 1 || end > 99)
            return null;
        if (Number.isInteger(settings) && (settings & END_SETTABLE) === 0)
            return null;
        return end;
    }

    subtitle() {
        const display = this._display;
        if (!this.present)
            return '';
        const p = formatPercent(display.Percentage ?? 0);
        const end = this._end();
        if (display.ChargeThresholdEnabled === true) {
            return end === null
                ? _('%s · charging stops early').format(p)
                : _('%s · stops at %s when plugged in').format(p, formatPercent(end));
        }
        const state = display.State;
        const at = seconds => formatTime(new Date(Date.now() + seconds * 1000), {timeOnly: true});
        if (state === UPower.DeviceState.DISCHARGING && display.TimeToEmpty > 0)
            return _('%s · until %s').format(p, at(display.TimeToEmpty));
        if (state === UPower.DeviceState.CHARGING && display.TimeToFull > 0)
            return _('%s · full at %s').format(p, at(display.TimeToFull));
        if (state === UPower.DeviceState.FULLY_CHARGED)
            return _('%s · charged').format(p);
        return p;
    }

    // GNOME's name for the icon of this charge
    _iconName() {
        const display = this._display;
        const level = 10 * Math.floor((display?.Percentage ?? 100) / 10);
        if (display?.State === UPower.DeviceState.FULLY_CHARGED ||
            (display?.State === UPower.DeviceState.CHARGING && level === 100))
            return 'battery-level-100-charged-symbolic';
        let state = '';
        if (display?.State === UPower.DeviceState.CHARGING)
            state = '-charging';
        else if (display?.State === UPower.DeviceState.PENDING_CHARGE && display?.ChargeThresholdEnabled === true)
            state = '-plugged-in';
        return `battery-level-${level}${state}-symbolic`;
    }

    sync() {
        if (!this._display)
            return;
        const wasPresent = this.tile.visible;
        this.tile.visible = this.present || this._others !== null;
        if (!this.present) {
            if (wasPresent && !this.tile.visible)
                this.tile.menu.close();
            if (this._others)
                this._syncOthers();
            return;
        }
        this.tile.title = _('Battery');
        const subtitle = this.subtitle();
        const names = [this._iconName(), this._display.IconName].filter(name => typeof name === 'string' && name);
        const gicon = new Gio.ThemedIcon({names, use_default_fallbacks: false});
        this.tile.set({subtitle, gicon});
        this.tile.menu.setHeader(gicon, _('Battery'), subtitle);

        const limitable = this._limitable().length > 0;
        this._row.visible = limitable;
        if (!limitable)
            return;
        const end = this._end();
        if (end === null) {
            this._limitTitle.text = _('Keep the battery from charging fully');
            this._limitText.text = _('Charging stops early, as the laptop decides, so the battery lasts more years');
        } else {
            const e = formatPercent(end);
            this._limitTitle.text = _('Keep the battery at %s').format(e);
            this._limitText.text = _('Charging stops at %s, so the battery lasts more years').format(e);
        }
        if (!this._pending)
            this._switch.state = this._display.ChargeThresholdEnabled === true;
        this._row.accessible_name = this._limitTitle.text;
    }

    setOthers(lowest) {
        this._others = lowest && typeof lowest.label === 'string' && typeof lowest.percent === 'number' ? lowest : null;
        this.sync();
    }

    // A computer without a battery of its own: the tile is the devices' batteries, saying the lowest
    _syncOthers() {
        const {label, percent} = this._others;
        const subtitle = `${label} · ${formatPercent(percent)}`;
        const gicon = new Gio.ThemedIcon({name: `battery-level-${10 * Math.floor(percent / 10)}-symbolic`});
        this.tile.set({title: _('Batteries'), subtitle, gicon});
        this.tile.menu.setHeader(gicon, _('Batteries'), subtitle);
        this._row.visible = false;
    }

    // The 80% row, as the person's switch: UPower's own method on every battery that has a limit
    limitRow() {
        if (!this._row.visible)
            return null;
        return {title: this._limitTitle.text, active: this._switch.state,
            upower: this._display?.ChargeThresholdEnabled === true, error: this._error.visible};
    }

    toggleLimit() {
        if (this._pending || !this._row.visible)
            return;
        const want = !(this._display?.ChargeThresholdEnabled === true);
        this._switch.state = want;
        this._error.hide();
        this._pending = true;
        const calls = this._limitable().map(battery => new Promise((resolve, reject) => {
            battery.call('EnableChargeThreshold', new GLib.Variant('(b)', [want]), Gio.DBusCallFlags.NONE,
                CALL_TIMEOUT_MS, this._cancellable, (proxy, result) => {
                    try {
                        proxy.call_finish(result);
                        resolve();
                    } catch (e) {
                        reject(e);
                    }
                });
        }));
        Promise.all(calls).catch(e => {
            if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                return;
            console.debug(`Amethystora UI: the charge limit was refused: ${e.message}`);
            if (this._open)
                this._error.show();
            this._settled();
        }).then(() => {
            if (this._cancellable.is_cancelled() || !this._pending)
                return;
            if (this._display?.ChargeThresholdEnabled === want) {
                this._settled();
                return;
            }
            this._display?.connectObject('g-properties-changed', () => {
                if (this._display?.ChargeThresholdEnabled === want)
                    this._settled();
            }, this._wait = {});
            this._settleId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE_MS, () => {
                this._settleId = 0;
                this._settled();
                return GLib.SOURCE_REMOVE;
            });
        });
    }

    // The call is over: what UPower says, whatever was asked
    _settled() {
        if (this._settleId)
            GLib.source_remove(this._settleId);
        this._settleId = 0;
        if (this._wait)
            this._display?.disconnectObject(this._wait);
        this._wait = null;
        this._pending = false;
        if (!this._cancellable.is_cancelled())
            this.sync();
    }

    destroy() {
        this._cancellable.cancel();
        this._settled();
        this.closed();
        this._batteries.clear();
        this._display = null;
        this._manager = null;
        this.tile.menu.destroy();
        this.tile.destroy();
    }
}
