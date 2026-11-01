// Sounds: the two of the desktop's own sounds that GNOME plays for nobody, from the sound theme the person has
// (Amethystora's Soft set, /usr/share/sounds/amethystora, unless they chose another). A slow chord once the desktop
// has started, and a rising note when the computer is plugged in. Everything else in the set (notifications,
// screenshots, the volume, the bell) GNOME and the apps play themselves.
//
// The sounds go through GNOME's own sound player, which follows Settings: with event sounds off (ame desktop sounds
// off) neither is heard. The module has no setting of its own and no timer that repeats: it waits for UPower to say
// the power changed, on the system bus, and plays the chord once per session, since the extension is turned off at
// every lock and on again at every unlock.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

// UPower itself, whose OnBattery says whether the computer runs from its batteries: true to false is the cable going
// in, however the battery then behaves (charging, full, held at a charge limit, or not charging at all)
const UPOWER = 'org.freedesktop.UPower';
const UPOWER_PATH = '/org/freedesktop/UPower';
// How long after the desktop starts the chord plays, so that it is not lost in what the session does first
const LOGIN_DELAY_MS = 1500;

// Once per shell, which is once per session: the module is started again at every unlock
let loginPlayed = false;

export class Sounds {
    constructor(ext) {
        this._ext = ext;
        this._proxy = null;
    }

    enable() {
        this._state = null;
        if (!loginPlayed) {
            this._loginId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, LOGIN_DELAY_MS, () => {
                this._loginId = 0;
                loginPlayed = true;
                this._play('desktop-login', _('Login sound'));
                return GLib.SOURCE_REMOVE;
            });
        }

        // No battery, or no UPower: nothing is ever played for the power
        this._cancellable = new Gio.Cancellable();
        Gio.DBusProxy.new_for_bus(Gio.BusType.SYSTEM, Gio.DBusProxyFlags.NONE, null,
            UPOWER, UPOWER_PATH, UPOWER, this._cancellable, (source, result) => {
                let proxy;
                try {
                    proxy = Gio.DBusProxy.new_for_bus_finish(result);
                } catch {
                    return;
                }
                if (!this._cancellable)
                    return;
                this._proxy = proxy;
                this._state = this._readState();
                proxy.connectObject('g-properties-changed', () => this._powerChanged(), this);
            });
    }

    disable() {
        if (this._loginId) {
            GLib.source_remove(this._loginId);
            this._loginId = 0;
        }
        this._cancellable?.cancel();
        this._cancellable = null;
        this._proxy?.disconnectObject(this);
        this._proxy = null;
        this._state = null;
    }

    // Whether the computer runs from its batteries, or null when UPower does not say
    _readState() {
        const value = this._proxy?.get_cached_property('OnBattery');
        return value?.get_type_string() === 'b' ? value.unpack() : null;
    }

    // Only the step from the battery to the mains: a battery that fills up while plugged in says nothing
    _powerChanged() {
        const state = this._readState();
        const before = this._state;
        this._state = state;
        if (before === true && state === false)
            this._play('power-plug', _('Power connected sound'));
    }

    _play(name, description) {
        global.display.get_sound_player().play_from_theme(name, description, null);
    }
}
