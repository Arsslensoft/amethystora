// What a desk sets, read and written through what the desktop already has: the sound output through GNOME's own mixer
// (as its output menu and quick settings choose one), the dock through the dock module's settings, Focus through the
// focus module's pick (a profile picked by hand), and the power mode through power-profiles-daemon's ActiveProfile,
// which tuned-ppd answers on this image. Nothing here is a security setting, and nothing starts an app.
//
// Each part changes only what differs, and says what it wrote; putting back changes only what is still as it was
// written, so that whatever the person changed meanwhile stays theirs.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import * as Volume from 'resource:///org/gnome/shell/ui/status/volume.js';

import {POWER} from './state.js';

const PROFILES = 'org.freedesktop.UPower.PowerProfiles';
const PROFILES_PATH = '/org/freedesktop/UPower/PowerProfiles';
const PROPERTIES = 'org.freedesktop.DBus.Properties';
// The output ids looked up: the mixer counts them from one, a few for each card and port it has seen
const MOST_OUTPUT_ID = 255;

function systemCall(method, parameters, type) {
    return new Promise(resolve => {
        Gio.DBus.system.call(PROFILES, PROFILES_PATH, PROPERTIES, method, parameters,
            type ? new GLib.VariantType(type) : null, Gio.DBusCallFlags.NONE, 2000, null, (bus, result) => {
                try {
                    resolve(bus.call_finish(result));
                } catch {
                    resolve(null);
                }
            });
    });
}

// An output as GNOME's own list names it
function outputName(device) {
    return device.origin ? `${device.description} – ${device.origin}` : device.description;
}

const same = (a, b) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);

export class Applier {
    constructor(ext) {
        this._ext = ext;
    }

    get _mixer() {
        return Volume.getMixerControl();
    }

    // ---- Sound ----

    sound() {
        const mixer = this._mixer;
        const sink = mixer?.get_default_sink();
        if (!sink)
            return null;
        const device = mixer.lookup_device_from_stream(sink);
        return {
            sink: sink.get_name() ?? '',
            port: device?.get_port?.() ?? sink.get_port?.()?.port ?? '',
            label: device ? outputName(device) : sink.get_description() ?? '',
        };
    }

    // The output device of that sound card and port, among the mixer's outputs: their ids are small numbers it gives
    // from one up, looked up rather than taken from quick settings' private list
    _device(sound) {
        const mixer = this._mixer;
        for (let id = 1; id <= MOST_OUTPUT_ID; id++) {
            const device = mixer.lookup_output_id(id);
            const stream = device ? mixer.get_stream_from_device(device) : null;
            if (stream?.get_name() === sound.sink && (!sound.port || device.get_port() === sound.port))
                return device;
        }
        return null;
    }

    _setSound(sound) {
        const mixer = this._mixer;
        const device = this._device(sound);
        if (device) {
            mixer.change_output(device);
            return true;
        }
        // An output GNOME lists under no device of its own: the card's stream itself
        const stream = mixer.get_sinks().find(s => s.get_name() === sound.sink);
        if (stream) {
            mixer.set_default_sink(stream);
            return true;
        }
        return false;
    }

    // ---- The dock ----

    get _dock() {
        return this._ext.modules?.get('dock')?.settings ?? null;
    }

    dock() {
        const settings = this._dock;
        return settings ? {edge: settings.get_string('edge'), screens: settings.get_string('screens')} : null;
    }

    _setDock(dock) {
        const settings = this._dock;
        if (!settings)
            return false;
        // Two plain writes: the dock's own settings object is never put in delay mode, which would hold its later writes
        if (settings.get_string('edge') !== dock.edge)
            settings.set_string('edge', dock.edge);
        if (settings.get_string('screens') !== dock.screens)
            settings.set_string('screens', dock.screens);
        return true;
    }

    // ---- Focus ----

    get _focus() {
        return this._ext.modules?.get('focus') ?? null;
    }

    focus() {
        const focus = this._focus;
        return focus?.state ? focus.state().hand ?? 'off' : null;
    }

    _setFocus(id) {
        const focus = this._focus;
        if (!focus?.pick)
            return false;
        focus.pick(id);
        return true;
    }

    // ---- Power ----

    // A program's hold (a Focus profile's, a game's) lays a mode over the person's own: while one is on, the mode shown is
    // not theirs to record, and writing one would make the daemon let go of every hold
    async _held() {
        const reply = await systemCall('Get', new GLib.Variant('(ss)', [PROFILES, 'ActiveProfileHolds']), '(v)');
        return (reply?.recursiveUnpack()[0] ?? []).length > 0;
    }

    async power() {
        if (await this._held())
            return null;
        const reply = await systemCall('Get', new GLib.Variant('(ss)', [PROFILES, 'ActiveProfile']), '(v)');
        const value = reply?.deepUnpack()[0]?.deepUnpack?.();
        return POWER.includes(value) ? value : null;
    }

    async _setPower(profile) {
        if (await this._held())
            return false;
        const reply = await systemCall('Get', new GLib.Variant('(ss)', [PROFILES, 'Profiles']), '(v)');
        const listed = reply?.recursiveUnpack()[0] ?? [];
        if (!listed.some(p => p.Profile === profile))
            return false;
        const set = await systemCall('Set', new GLib.Variant('(ssv)', [PROFILES, 'ActiveProfile',
            new GLib.Variant('s', profile)]), null);
        return set !== null;
    }

    // ---- All of it ----

    // How things are now, as a desk records them
    async read() {
        const now = {};
        const sound = this.sound();
        if (sound?.sink)
            now.sound = sound;
        const dock = this.dock();
        if (dock)
            now.dock = dock;
        const focus = this.focus();
        if (focus)
            now.focus = focus;
        const power = await this.power();
        if (power)
            now.power = power;
        return now;
    }

    // Sets what differs; returns {before, wrote}, each only the parts written, and says them to `wrote` as it goes, so
    // that what was set is recorded even if the module stops in the middle. The power mode first: a Focus profile picked
    // after it may then hold its own mode over it.
    async apply(set, now, written = () => {}) {
        const before = {}, wrote = {};
        if (set.power && now.power && set.power !== now.power && await this._setPower(set.power)) {
            before.power = now.power;
            wrote.power = set.power;
            written(before, wrote);
        }
        if (set.sound && !(now.sound?.sink === set.sound.sink && now.sound?.port === set.sound.port) &&
            this._setSound(set.sound)) {
            before.sound = now.sound;
            wrote.sound = set.sound;
        }
        if (set.dock && !same(set.dock, now.dock) && this._setDock(set.dock)) {
            before.dock = now.dock;
            wrote.dock = set.dock;
        }
        if (set.focus && set.focus !== now.focus && this._setFocus(set.focus)) {
            before.focus = now.focus;
            wrote.focus = set.focus;
        }
        written(before, wrote);
        return {before, wrote};
    }

    // Puts back each part still as it was written
    async putBack(before, wrote) {
        const now = await this.read();
        if (wrote.sound && before.sound && now.sound?.sink === wrote.sound.sink && now.sound?.port === wrote.sound.port)
            this._setSound(before.sound);
        if (wrote.dock && before.dock && same(now.dock, wrote.dock))
            this._setDock(before.dock);
        if (wrote.focus && before.focus && now.focus === wrote.focus)
            this._setFocus(before.focus);
        if (!wrote.power || !before.power)
            return;
        // The profile just ended lets go of its hold over the bus: a moment for that, then the desk's mode goes back if it
        // is still the desk's (read while nothing holds one)
        for (let i = 0; i < 5 && await this._held(); i++)
            await new Promise(resolve => GLib.timeout_add(GLib.PRIORITY_DEFAULT, 200, () => resolve() ?? GLib.SOURCE_REMOVE));
        if (await this.power() === wrote.power)
            await this._setPower(before.power);
    }
}
