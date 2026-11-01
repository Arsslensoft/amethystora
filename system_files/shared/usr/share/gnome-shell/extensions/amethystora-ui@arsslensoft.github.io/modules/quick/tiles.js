// The tiles of quick settings by name, and what two of GNOME's own say beyond GNOME's words: the Wi-Fi network's
// strength and a Bluetooth device's battery. And the phone's tile, from the phone link.
//
// GNOME's tiles stay GNOME's: their subtitle is still made by GNOME's own method, which is wrapped on the tile itself so
// that a word is added after what GNOME says, and put back when the module stops. The words follow a change only while
// quick settings is open; it is worked out again each time it opens.

import Clutter from 'gi://Clutter';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';
import {QuickToggle} from 'resource:///org/gnome/shell/ui/quickSettings.js';

// GNOME's own tiles by their class, under the names the keys and commands use
const GNOME_IDS = {
    NMWirelessToggle: 'wifi',
    NMWiredToggle: 'wired',
    NMBluetoothToggle: 'tether',
    NMModemToggle: 'mobile',
    NMVpnToggle: 'vpn',
    BluetoothToggle: 'bluetooth',
    PowerProfilesToggle: 'power-mode',
    NightLightToggle: 'night-light',
    DarkModeToggle: 'dark-style',
    DoNotDisturbToggle: 'dnd',
    KeyboardBrightnessToggle: 'keyboard-light',
    RfkillToggle: 'airplane',
    RotationToggle: 'rotation',
};
export const ID = /^[a-z0-9-]{1,40}$/;
// The phone app, by its full path: the session's PATH may hold a folder of the account's own before the system's
const PHONE_APP = '/usr/bin/amethystora-phone';
// What the Wi-Fi tile's icon says about the signal (GNOME draws it from the strength), as the word it adds
const SIGNAL = /^network-wireless-signal-(excellent|good|ok|weak|none)-symbolic$/;
// GnomeBluetooth.BatteryType.PERCENTAGE: the device says its charge in percent (GnomeBluetooth itself is not loaded
// here, so that a system built without Bluetooth loses nothing)
const BATTERY_PERCENTAGE = 1;

const percent = new Intl.NumberFormat(undefined, {style: 'percent'});

export function formatPercent(value) {
    return percent.format(Math.max(0, Math.min(100, value)) / 100);
}

// The name a tile goes by. A tile of GNOME's that the table does not know is its class's name without "Toggle"; one of
// another extension is x- and its class's name, so that it can be ordered and put away like the rest.
export function tileId(item, gnome) {
    if (item._amethystoraQuickId)
        return item._amethystoraQuickId;
    const name = item.constructor?.name ?? '';
    if (GNOME_IDS[name])
        return GNOME_IDS[name];
    let id = name.toLowerCase().replace(/[^a-z0-9-]+/g, '-').replace(/^-+|-+$/g, '');
    if (gnome)
        id = id.replace(/-?toggle$/, '');
    id = `${gnome ? '' : 'x-'}${id || 'tile'}`.slice(0, 40);
    return ID.test(id) ? id : 'x-tile';
}

// The words GNOME's Wi-Fi and Bluetooth tiles say, with the strength or the battery added
export class Subtitles {
    constructor() {
        this._wrapped = new Map();
        this._watched = [];
    }

    // GNOME's tiles as they are now; a tile seen again is left as it is
    adopt(items) {
        for (const item of items) {
            if (this._wrapped.has(item))
                continue;
            const name = item.constructor?.name;
            if (name === 'NMWirelessToggle' && typeof item._transformSubtitle === 'function')
                this._wrapWifi(item);
            else if (name === 'BluetoothToggle' && typeof item._sync === 'function')
                this._wrapBluetooth(item);
        }
    }

    _wrapWifi(toggle) {
        const original = toggle._transformSubtitle;
        toggle._transformSubtitle = function (source) {
            const subtitle = original.call(this, source);
            const word = strength(this.icon_name);
            return subtitle && word ? `${subtitle} · ${word}` : subtitle;
        };
        this._wrapped.set(toggle, {name: '_transformSubtitle', refresh: () => refreshWifi(toggle)});
    }

    _wrapBluetooth(toggle) {
        const original = toggle._sync;
        toggle._sync = function () {
            original.call(this);
            const connected = [...this._client?.getDevices?.() ?? []].filter(device => device.connected);
            if (connected.length !== 1)
                return;
            const [device] = connected;
            if (device.battery_type === BATTERY_PERCENTAGE && device.battery_percentage > 0)
                this.subtitle = `${device.alias || device.name} · ${formatPercent(device.battery_percentage)}`;
        };
        this._wrapped.set(toggle, {name: '_sync', refresh: () => toggle._sync()});
    }

    // Quick settings opened: the words now, and their changes while it stays open
    opened() {
        this.closed();
        for (const [toggle, {name, refresh}] of this._wrapped) {
            refresh();
            if (name === '_transformSubtitle') {
                toggle.connectObject('notify::icon-name', () => refresh(), this);
                this._watched.push(toggle);
            } else {
                for (const device of toggle._client?.getDevices?.() ?? []) {
                    if (!device.connected)
                        continue;
                    device.connectObject('notify::battery-percentage', () => refresh(), this);
                    this._watched.push(device);
                }
            }
        }
    }

    closed() {
        for (const object of this._watched)
            object.disconnectObject(this);
        this._watched = [];
    }

    // GNOME's own methods back, and GNOME's own words once more
    destroy() {
        this.closed();
        for (const [toggle, {name}] of this._wrapped) {
            delete toggle[name];
            try {
                if (name === '_transformSubtitle')
                    refreshWifi(toggle);
                else
                    toggle._sync();
            } catch (e) {
                console.warn(`Amethystora UI: a tile's words were not put back: ${e.message}`);
            }
        }
        this._wrapped.clear();
    }
}

// The word for the strength GNOME's Wi-Fi icon shows: its thresholds are GNOME's (80, 50, 40)
function strength(iconName) {
    const level = SIGNAL.exec(iconName ?? '')?.[1];
    switch (level) {
    case 'excellent':
        return _('strong');
    case 'good':
        return _('good');
    case 'ok':
        return _('fair');
    case 'weak':
    case 'none':
        return _('weak');
    default:
        return null;
    }
}

// The Wi-Fi tile's words made again by its own method, from the network it shows
function refreshWifi(toggle) {
    const item = toggle._itemBinding?.source ?? null;
    toggle.subtitle = toggle._transformSubtitle(item?.name ?? null);
}

// The phone's tile: shown while a paired phone is connected, its name and its battery; a click opens the Phone app. It
// is never on or off, so it is never drawn as checked.
export class PhoneTile {
    constructor(connect) {
        this._connect = connect;
        this.tile = new QuickToggle({
            title: _('Phone'),
            iconName: 'smartphone-symbolic',
            toggleMode: false,
            visible: false,
        });
        this.tile._amethystoraQuickId = 'phone';
        this.tile.connect('clicked', () => {
            Main.panel.closeQuickSettings();
            Util.spawn([PHONE_APP]);
        });
        this._unlisten = connect?.onPhonesChanged?.(() => this._sync()) ?? null;
        this._sync();
    }

    _sync() {
        const phones = this._connect?.phones?.() ?? [];
        this.tile.visible = phones.length > 0;
        if (phones.length === 0)
            return;
        if (phones.length > 1) {
            this.tile.title = _('Phones');
            this.tile.subtitle = ngettext('%d connected', '%d connected', phones.length).format(phones.length);
            return;
        }
        const [phone] = phones;
        // The phone's own name is text, never markup (St.Label's default)
        this.tile.title = phone.name || _('Phone');
        if (phone.level < 0)
            this.tile.subtitle = phone.charging ? _('Charging') : _('Connected');
        else if (phone.charging)
            this.tile.subtitle = _('Charging · %s').format(formatPercent(phone.level));
        else
            this.tile.subtitle = _('Connected · %s').format(formatPercent(phone.level));
    }

    destroy() {
        this._unlisten?.();
        this._unlisten = null;
        this.tile.destroy();
    }
}

// A tile is one column of GNOME's grid
export function isTile(grid, child) {
    if (!(child instanceof Clutter.Actor))
        return false;
    try {
        return grid.layout_manager.get_child_meta(grid, child)?.column_span === 1;
    } catch {
        return false;
    }
}
