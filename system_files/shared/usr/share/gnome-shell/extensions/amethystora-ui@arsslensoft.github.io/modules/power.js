// Power you can see: "why is the fan loud, why did the battery go so fast?" answered where people look. The battery
// tile's menu in quick settings explains the battery (modules/power/battery.js), and a chip beside the meters names an
// app that has kept the processor busy for two minutes (modules/power/busy.js). The same menu lists the batteries of the
// mouse, the keyboard, the headphones and the rest, with a notice once at 15 % (modules/power/devices.js).
//
// Three keys of org.gnome.shell.extensions.amethystora-ui.power (ame desktop power battery|busy|devices, Control), read
// live; a part that is off has nothing connected and nothing made. Nothing new runs on a timer: the battery looks at the
// processes when it is plugged in or out and when its menu opens, and the busy chip rides the meters' own reading
// (without the meters there is no chip).
//
// For the suites: battery, busy and devices are the three parts, with their state() and test helpers.

import {BatteryDetails} from './power/battery.js';
import {Busy} from './power/busy.js';
import {DeviceBatteries} from './power/devices.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.power';

export class Power {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject(
            'changed::battery-details', () => this._sync(),
            'changed::busy-app', () => this._sync(),
            'changed::device-batteries', () => this._sync(),
            this);
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._stop('_devices');
        this._stop('_battery');
        this._stop('_busy');
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    _sync() {
        const modules = this._ext.modules;
        this._toggle('_battery', this._settings.get_boolean('battery-details'),
            () => new BatteryDetails(modules?.get('quick')));
        this._toggle('_busy', this._settings.get_boolean('busy-app'),
            () => new Busy(modules?.get('meters'), () => modules?.get('glass') ?? null));
        this._toggle('_devices', this._settings.get_boolean('device-batteries'),
            () => new DeviceBatteries(modules?.get('quick'), modules?.get('connect')));
    }

    _toggle(name, wanted, make) {
        if (wanted && !this[name]) {
            this[name] = make();
            try {
                this[name].enable();
            } catch (e) {
                console.error(`Amethystora UI: the power module's ${name.slice(1)} did not start: ${e.message}\n${e.stack}`);
                this._stop(name);
            }
        } else if (!wanted && this[name]) {
            this._stop(name);
        }
    }

    _stop(name) {
        const part = this[name];
        this[name] = null;
        try {
            part?.disable();
        } catch (e) {
            console.error(`Amethystora UI: the power module's ${name.slice(1)} did not stop cleanly: ${e.message}`);
        }
    }

    get battery() {
        return this._battery;
    }

    get busy() {
        return this._busy;
    }

    get devices() {
        return this._devices;
    }
}
