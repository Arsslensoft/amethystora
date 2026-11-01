// Comfort: what the desktop does for people who need it calmer or clearer. Reduce motion is GNOME's own reduced
// motion setting, which GTK and libadwaita apps follow as well (ame desktop comfort reduce-motion, Control); GNOME's
// Animations switch turned off counts as reduce motion too. Increase contrast is GNOME's own High Contrast, so that
// the desktop and the apps agree. The power saver is the profile power-profiles-daemon's interface says is active
// (tuned-ppd answers it on this image): while it is on, the glass is solid and animations take half their time.
// Nothing here decides when any of them is on: it only listens, to one of GNOME's keys, two of its shell settings and
// one property on the system bus, and costs nothing in between. Its one setting is the light desktop (`light` in
// org.gnome.shell.extensions.amethystora-ui.comfort, ame desktop light-desktop, Control): what the power saver does
// to the glass and the motion, kept on, and no live window previews in the dock. A computer that draws the screen
// without a graphics driver, or has little memory, is offered it once (comfort/slow.js).
//
// GNOME turns its animations off by itself where the screen is drawn without a GPU (a virtual machine without
// acceleration, some remote desktops) and while the screen is shared remotely: there, reduce motion is on whatever
// the switch says, since nothing is animated anyway.
//
// For the other modules (the dock, the glass, the windows, the motion, the animations' speed in tweaks):
//
//   const comfort = this._ext.modules.get('comfort');
//   comfort?.reduceMotion   true while GNOME's reduced motion is 'reduce' or its animations are off: an animation
//                           that moves an actor (translation, scale, a path) becomes a crossfade in place of at most
//                           150 ms
//   comfort?.highContrast   true while GNOME's High Contrast is on
//   comfort?.powerSaver     true while the power-saver profile is active
//   comfort?.lighter        true while the power saver or the light desktop is on or a focus profile asks for what
//                           they do (solid glass, shorter motion): what the glass and the animations' speed follow
//   comfort?.light          true while the light desktop is on (the dock's previews follow it too)
//   comfort?.setLighter(b)  the focus module's ask, in memory
//   const unlisten = comfort?.listen(c => ...);
//                           the callback runs once for each change of any of these, with this object; the
//                           function returned takes it away again. Call it in your disable(). Calling it after
//                           comfort itself has stopped is harmless.
//
// Comfort starts before tweaks and every module with motion, and stops after them.

import Gio from 'gi://Gio';
import St from 'gi://St';

import {SlowCheck} from './comfort/slow.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.comfort';
// GNOME's accessibility settings: reduced-motion is 'no-preference' or 'reduce'
const A11Y = 'org.gnome.desktop.a11y.interface';
const REDUCED_MOTION = 'reduced-motion';
// power-profiles-daemon's interface, where GNOME's own quick settings find it too
const PROFILES = 'org.freedesktop.UPower.PowerProfiles';
const PROFILES_PATH = '/org/freedesktop/UPower/PowerProfiles';
const POWER_SAVER = 'power-saver';

export class Comfort {
    constructor(ext) {
        this._ext = ext;
        this._proxy = null;
    }

    enable() {
        this._listeners = new Set();
        this._powerSaver = false;
        this._settings = new Gio.Settings({schema_id: A11Y});
        // A GNOME without the key (before 50) has only its Animations switch
        this._hasKey = this._settings.settings_schema.has_key(REDUCED_MOTION);
        if (this._hasKey)
            this._settings.connectObject(`changed::${REDUCED_MOTION}`, () => this._changed(), this);
        St.Settings.get().connectObject(
            'notify::enable-animations', () => this._changed(),
            'notify::high-contrast', () => this._changed(),
            this);
        this._own = this._ext.getSettings(SCHEMA);
        this._own.connectObject('changed::light', () => this._changed(), this);
        this._state = this._now();
        this._slow = new SlowCheck(this._ext, this._own);
        this._slow.start();

        // A daemon that is not running means no power saver, and nothing is said about it
        this._cancellable = new Gio.Cancellable();
        Gio.DBusProxy.new_for_bus(Gio.BusType.SYSTEM, Gio.DBusProxyFlags.DO_NOT_AUTO_START, null,
            PROFILES, PROFILES_PATH, PROFILES, this._cancellable, (source, result) => {
                let proxy;
                try {
                    proxy = Gio.DBusProxy.new_for_bus_finish(result);
                } catch {
                    return;
                }
                if (!this._listeners)
                    return;
                this._proxy = proxy;
                proxy.connectObject(
                    'g-properties-changed', () => this._readProfile(),
                    'notify::g-name-owner', () => this._readProfile(),
                    this);
                this._readProfile();
            });
    }

    disable() {
        this._slow?.stop();
        this._slow = null;
        this._own?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._own?.run_dispose();
        this._own = null;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._proxy?.disconnectObject(this);
        this._proxy = null;
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        St.Settings.get().disconnectObject(this);
        this._listeners?.clear();
        this._listeners = null;
        this._powerSaver = false;
        this._lighter = false;
        this._state = null;
    }

    get reduceMotion() {
        return (this._hasKey && this._settings?.get_string(REDUCED_MOTION) === 'reduce') ||
            !St.Settings.get().enable_animations;
    }

    get highContrast() {
        return St.Settings.get().high_contrast;
    }

    get powerSaver() {
        return this._powerSaver;
    }

    get lighter() {
        return this._powerSaver || !!this._lighter || this.light;
    }

    get light() {
        return this._own?.get_boolean('light') ?? false;
    }

    // For the tests: the slow computer's check, run now with what it would read replaced ({accelerated, memory,
    // drivers}), whether it was asked before or not
    checkSlow(facts = null) {
        return this._slow?.check(facts, true) ?? null;
    }

    setLighter(on) {
        if (!!on === !!this._lighter)
            return;
        this._lighter = !!on;
        this._changed();
    }

    listen(callback) {
        this._listeners?.add(callback);
        return () => this._listeners?.delete(callback);
    }

    // The profile as the daemon has it now; with nobody owning the name, the cached value is stale and counts for
    // nothing
    _readProfile() {
        const proxy = this._proxy;
        const value = proxy?.g_name_owner ? proxy.get_cached_property('ActiveProfile') : null;
        const saver = value?.get_type_string() === 's' && value.unpack() === POWER_SAVER;
        if (saver === this._powerSaver)
            return;
        this._powerSaver = saver;
        this._changed();
    }

    _now() {
        return [this.reduceMotion, this.highContrast, this.powerSaver, this.lighter, this.light].join();
    }

    // Each listener hears once per change, however many signals brought it
    _changed() {
        if (!this._listeners)
            return;
        const state = this._now();
        if (state === this._state)
            return;
        this._state = state;
        for (const listener of [...this._listeners]) {
            try {
                listener(this);
            } catch (e) {
                console.error(`Amethystora UI: a comfort listener failed: ${e.message}\n${e.stack}`);
            }
        }
    }
}
