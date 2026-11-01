// The power mode while a profile is on: a hold with the power daemon (power-profiles-daemon's interface, which tuned-ppd
// answers on this image), never a change of the mode the person chose. The daemon takes holds only for performance and
// power saver, puts the person's own mode back when the last hold goes, and drops a hold by itself when the person picks
// a mode in GNOME's menu or when the shell's connection to the bus ends (a crash included). A new hold is taken before
// the old one is let go, so the daemon never falls back to the person's mode in between.
//
// Nothing is connected while no profile wants a hold.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const PROFILES = 'org.freedesktop.UPower.PowerProfiles';
const PROFILES_PATH = '/org/freedesktop/UPower/PowerProfiles';
const APP_ID = 'org.amethystora.Focus';
const HOLDABLE = ['performance', 'power-saver'];

export class PowerHold {
    constructor() {
        this._wanted = 'as-set';
        this._reason = '';
        this._cookie = 0;
        this._held = null;
        this._pending = null;
        this._dropped = false;
    }

    // What is held now, for the suites: {profile, cookie} or null
    get held() {
        return this._cookie ? {profile: this._held, cookie: this._cookie} : null;
    }

    get connected() {
        return !!this._proxy || !!this._cancellable;
    }

    set(wanted, reason) {
        if (!HOLDABLE.includes(wanted))
            wanted = 'as-set';
        if (wanted === this._wanted) {
            this._reason = reason;
            return;
        }
        this._wanted = wanted;
        this._reason = reason;
        // The person's own pick while a hold was on is kept until the profile asks for something else
        this._dropped = false;
        this._sync();
    }

    stop() {
        this._wanted = 'as-set';
        this._release();
        this._cancellable?.cancel();
        this._cancellable = null;
        this._proxy?.disconnectObject(this);
        this._proxy = null;
        this._pending = null;
    }

    _sync() {
        if (this._wanted === 'as-set') {
            this._release();
            this._disconnectWhenIdle();
            return;
        }
        if (this._held === this._wanted || this._pending === this._wanted || this._dropped)
            return;
        if (!this._proxy) {
            this._connect();
            return;
        }
        // Only a mode the machine has (performance is missing where the hardware has no such mode)
        const profiles = this._proxy.get_cached_property('Profiles')?.recursiveUnpack() ?? [];
        if (!profiles.some(p => p.Profile === this._wanted)) {
            this._release();
            return;
        }
        const asked = this._wanted;
        this._pending = asked;
        this._proxy.call('HoldProfile', new GLib.Variant('(sss)', [asked, this._reason, APP_ID]),
            Gio.DBusCallFlags.NONE, -1, null, (proxy, result) => {
                let cookie = 0;
                try {
                    [cookie] = proxy.call_finish(result).deepUnpack();
                } catch (e) {
                    console.warn(`Amethystora UI: focus could not hold ${asked}: ${e.message}`);
                }
                if (this._pending === asked)
                    this._pending = null;
                if (!cookie)
                    return;
                if (this._wanted !== asked || !this._proxy) {
                    // Asked for something else meanwhile, or stopped: this one goes at once
                    this._releaseCookie(proxy, cookie);
                    this._sync();
                    return;
                }
                const old = this._cookie;
                this._cookie = cookie;
                this._held = asked;
                if (old)
                    this._releaseCookie(proxy, old);
            });
    }

    _connect() {
        if (this._cancellable)
            return;
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        Gio.DBusProxy.new_for_bus(Gio.BusType.SYSTEM, Gio.DBusProxyFlags.NONE, null, PROFILES, PROFILES_PATH,
            PROFILES, cancellable, (source, result) => {
                if (this._cancellable === cancellable)
                    this._cancellable = null;
                let proxy;
                try {
                    proxy = Gio.DBusProxy.new_for_bus_finish(result);
                } catch {
                    return;
                }
                if (cancellable.is_cancelled())
                    return;
                this._proxy = proxy;
                // The daemon let our hold go: the person picked a mode, or it had to
                proxy.connectObject('g-signal', (p, sender, signal, parameters) => {
                    if (signal !== 'ProfileReleased')
                        return;
                    const [cookie] = parameters.deepUnpack();
                    if (cookie && cookie === this._cookie) {
                        this._cookie = 0;
                        this._held = null;
                        this._dropped = true;
                    }
                }, this);
                this._sync();
            });
    }

    _release() {
        if (this._cookie && this._proxy)
            this._releaseCookie(this._proxy, this._cookie);
        this._cookie = 0;
        this._held = null;
    }

    _releaseCookie(proxy, cookie) {
        proxy.call('ReleaseProfile', new GLib.Variant('(u)', [cookie]), Gio.DBusCallFlags.NONE, -1, null,
            (p, result) => {
                try {
                    p.call_finish(result);
                } catch {
                    // Already gone (the daemon dropped it, or restarted)
                }
            });
    }

    _disconnectWhenIdle() {
        if (this._cookie || this._pending)
            return;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._proxy?.disconnectObject(this);
        this._proxy = null;
    }
}
