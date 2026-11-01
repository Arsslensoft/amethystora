// Night Light while a profile is on. "Paused" is gnome-settings-daemon's own "until tomorrow", which it keeps in memory
// and lets go of by itself at the next sunrise: set only if it was not already, and taken back at the end only if it is
// still set. "Warmest" writes Night Light's switch and its temperature (1700 K, the warm end of Settings' slider): what
// they were is put in the state file first, and at the end each key that still holds what was written gets its old
// value back, so that a key the person changed meanwhile stays as they set it. The schedule is never written: quiet at
// night, Daylight and the Sleep trigger all read it.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const COLOR_SCHEMA = 'org.gnome.settings-daemon.plugins.color';
const COLOR_BUS = 'org.gnome.SettingsDaemon.Color';
const COLOR_PATH = '/org/gnome/SettingsDaemon/Color';
export const WARMEST = 1700;

// gnome-settings-daemon's colour plugin: its pause, and when the sun sets and rises where the person is
export const ColorIface = `
<node>
  <interface name="org.gnome.SettingsDaemon.Color">
    <property name="DisabledUntilTomorrow" type="b" access="readwrite"/>
    <property name="Sunrise" type="d" access="read"/>
    <property name="Sunset" type="d" access="read"/>
  </interface>
</node>`;
let ColorProxy = null;

// A proxy of the colour plugin, handed to `ready` once it is there, or `failed` called (without the daemon, or when
// it does not answer)
export function colorProxy(cancellable, ready, failed = null) {
    ColorProxy ??= Gio.DBusProxy.makeProxyWrapper(ColorIface);
    new ColorProxy(Gio.DBus.session, COLOR_BUS, COLOR_PATH, (proxy, error) => {
        if (cancellable.is_cancelled())
            return;
        if (error || !proxy.g_name_owner)
            failed?.();
        else
            ready(proxy);
    }, cancellable, Gio.DBusProxyFlags.DO_NOT_AUTO_START);
}

// Made once by the focus module, which disposes of it when it stops (never one per call: a settings object left to
// the collector can deadlock the shell against dconf's worker)
export function colorSettings() {
    if (!Gio.SettingsSchemaSource.get_default().lookup(COLOR_SCHEMA, true))
        return null;
    return new Gio.Settings({schema_id: COLOR_SCHEMA});
}

export class NightLight {
    // color: Night Light's settings, the focus module's (null where gnome-settings-daemon is not installed)
    constructor(state, color) {
        this._state = state;
        this._color = color;
        this._value = 'as-set';
    }

    get value() {
        return this._value;
    }

    // At the start: a record of a run that did not put Night Light back (a crash, a log-out, the lock screen) is taken
    // over as it is when the profile on now wants the same, so that a value the person changed meanwhile stays theirs
    // (the record's own comparison puts back only what still holds what was written); else it is put back. A pause is
    // asked for again: the daemon forgets it when the session ends, and does nothing when it is paused already.
    start(wanted) {
        if (this._state.get('night')) {
            if (wanted === 'warmest')
                this._value = 'warmest';
            else
                this._putBack();
        }
        if (this._state.get('paused')) {
            if (wanted === 'paused') {
                this._value = 'paused';
                this._setPaused(true);
            } else {
                this._setPaused(false);
            }
        }
    }

    set(value) {
        if (value === this._value)
            return;
        if (this._value === 'warmest')
            this._putBack();
        else if (this._value === 'paused')
            this._setPaused(false);
        this._value = value;
        if (value === 'warmest')
            this._warm();
        else if (value === 'paused')
            this._setPaused(true);
    }

    // The module stops: everything back, unless `keep` (the lock screen, with the profile still on), when the record
    // stays for the next start
    stop(keep) {
        // A pause still on its way is not made; taking one back is left to finish on its own
        this._cancellable?.cancel();
        this._cancellable = null;
        if (!keep) {
            if (this._value === 'warmest')
                this._putBack();
            else if (this._value === 'paused')
                this._setPaused(false);
        }
        this._cancellable = null;
        this._value = 'as-set';
    }

    _warm() {
        const settings = this._color;
        if (!settings)
            return;
        const wrote = {enabled: true, temperature: WARMEST};
        this._state.set('night', {
            before: {enabled: settings.get_boolean('night-light-enabled'),
                temperature: settings.get_uint('night-light-temperature')},
            wrote,
        });
        settings.set_uint('night-light-temperature', wrote.temperature);
        settings.set_boolean('night-light-enabled', wrote.enabled);
    }

    _putBack() {
        const record = this._state.get('night');
        this._state.set('night', undefined);
        const settings = record ? this._color : null;
        if (!settings)
            return;
        if (settings.get_boolean('night-light-enabled') === record.wrote.enabled)
            settings.set_boolean('night-light-enabled', record.before.enabled);
        if (settings.get_uint('night-light-temperature') === record.wrote.temperature)
            settings.set_uint('night-light-temperature', record.before.temperature);
    }

    // The daemon's pause: on only where it was off, recorded first; off only where this module turned it on
    _setPaused(on) {
        this._cancellable?.cancel();
        this._cancellable = null;
        if (!on && !this._state.get('paused'))
            return;
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        colorProxy(cancellable, proxy => {
            if (this._cancellable === cancellable)
                this._cancellable = null;
            // No daemon running: nothing to pause, and nothing recorded
            if (!proxy.g_name_owner)
                return;
            const now = proxy.DisabledUntilTomorrow;
            if (on) {
                if (now === true)
                    return;
                this._state.set('paused', true);
            } else {
                this._state.set('paused', undefined);
                if (now !== true)
                    return;
            }
            proxy.call('org.freedesktop.DBus.Properties.Set',
                new GLib.Variant('(ssv)', [COLOR_BUS, 'DisabledUntilTomorrow', GLib.Variant.new_boolean(on)]),
                Gio.DBusCallFlags.NONE, -1, null, (p, result) => {
                    try {
                        p.call_finish(result);
                    } catch (e) {
                        console.warn(`Amethystora UI: focus could not ${on ? 'pause' : 'resume'} Night Light: ${e.message}`);
                    }
                });
        });
    }
}
