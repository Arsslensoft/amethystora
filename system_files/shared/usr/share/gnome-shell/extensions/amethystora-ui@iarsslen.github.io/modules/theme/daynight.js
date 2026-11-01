// Light by day, dark by night (ame desktop day-night): a theme with a light and a dark pair (pair.theme) on its light side
// once the day starts and on its dark side once the night does. The theme module keeps one of these; it does nothing
// while the day-night key is off: no timeout, no bus subscription, no file read. On, it works out which part of the day
// it is and since when, and when that is not the part it last switched for (the state file), asks amethystora-theme for
// that side (`daynight day|night`, which leaves a theme without a pair alone), then sleeps until the next boundary in one
// timeout. It looks again when it wakes from suspend (logind's PrepareForSleep(false)), when the shell enables it again
// (every unlock), and when its keys or Night Light's change. A theme flipped by hand meanwhile stays until the next
// boundary, since the part of the day it last switched for is the same.
//
// The times: Night Light's own hours while Night Light is on with a schedule of its own; sunrise and sunset, which
// gnome-settings-daemon works out from the location, while it is on and automatic; else, or with hours chosen, the keys.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const STATE_DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const STATE = GLib.build_filenamev([STATE_DIR, 'day-night']);
const KEYS = ['day-night', 'day-night-times', 'day-starts', 'night-starts'];
const NIGHT_LIGHT_KEYS = ['night-light-enabled', 'night-light-schedule-automatic', 'night-light-schedule-from',
    'night-light-schedule-to'];
// Where Night Light knows neither hours nor a location
const DAY = 7;
const NIGHT = 19;
// Seconds past a boundary the timeout fires at, so that the clock has reached it
const SLACK = 2;

// The local date of a GLib.DateTime as YYYY-MM-DD
function day(dt) {
    return dt.format('%Y-%m-%d');
}

// The part of the day at `now` for a day and a night starting at these hours, the moment it started and the next
// boundary, each a GLib.DateTime: {side, since, next}
export function partOfDay(now, dayStarts, nightStarts) {
    const at = (offset, hours) => {
        const midnight = GLib.DateTime.new_local(now.get_year(), now.get_month(), now.get_day_of_month(), 0, 0, 0);
        return midnight.add_days(offset).add_seconds(Math.round(hours * 3600));
    };
    const boundaries = [];
    for (const offset of [-1, 0, 1]) {
        boundaries.push({side: 'day', at: at(offset, dayStarts)});
        boundaries.push({side: 'night', at: at(offset, nightStarts)});
    }
    boundaries.sort((a, b) => a.at.compare(b.at));
    let last = null, next = null;
    for (const b of boundaries) {
        if (b.at.compare(now) <= 0)
            last = b;
        else if (!next)
            next = b;
    }
    return {side: last.side, since: last.at, next: next.at};
}

export class DayNight {
    constructor(settings, nightLight, run) {
        this._settings = settings;
        this._nightLight = nightLight;
        this._run = run;
        this._timeout = 0;
        this._sleep = 0;
        this._cancellable = null;
        for (const key of KEYS)
            this._settings.connectObject(`changed::${key}`, () => this._update(), this);
        for (const key of NIGHT_LIGHT_KEYS)
            this._nightLight?.connectObject(`changed::${key}`, () => this._update(), this);
        this._update();
    }

    destroy() {
        this._settings?.disconnectObject(this);
        this._nightLight?.disconnectObject(this);
        this._stop();
        this._settings = null;
        this._nightLight = null;
    }

    // Whether it is on, and what it waits for, for the lab
    get state() {
        return {on: this._on(), timeout: this._timeout !== 0, sleep: this._sleep !== 0, next: this._next ?? null};
    }

    _on() {
        return this._settings?.get_boolean('day-night') ?? false;
    }

    _update() {
        if (!this._on()) {
            this._stop();
            return;
        }
        if (!this._sleep) {
            this._sleep = Gio.DBus.system.signal_subscribe('org.freedesktop.login1', 'org.freedesktop.login1.Manager',
                'PrepareForSleep', '/org/freedesktop/login1', null, Gio.DBusSignalFlags.NONE,
                (connection, sender, path, iface, signal, params) => {
                    // Waking: the timeout slept with the machine
                    if (!params.deepUnpack()[0])
                        this._check();
                });
        }
        this._check();
    }

    _stop() {
        if (this._timeout)
            GLib.source_remove(this._timeout);
        this._timeout = 0;
        if (this._sleep)
            Gio.DBus.system.signal_unsubscribe(this._sleep);
        this._sleep = 0;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._next = null;
    }

    // The hours the day and the night start at, to the callback: Night Light's, or the keys'
    _times(callback) {
        const nl = this._nightLight;
        if (this._settings.get_string('day-night-times') === 'hours') {
            callback(this._settings.get_double('day-starts'), this._settings.get_double('night-starts'));
            return;
        }
        if (!nl?.get_boolean('night-light-enabled')) {
            callback(DAY, NIGHT);
            return;
        }
        if (!nl.get_boolean('night-light-schedule-automatic')) {
            // Night Light's from is when its night starts, its to when the morning does
            callback(nl.get_double('night-light-schedule-to'), nl.get_double('night-light-schedule-from'));
            return;
        }
        this._cancellable?.cancel();
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        Gio.DBus.session.call('org.gnome.SettingsDaemon.Color', '/org/gnome/SettingsDaemon/Color',
            'org.freedesktop.DBus.Properties', 'GetAll', new GLib.Variant('(s)', ['org.gnome.SettingsDaemon.Color']),
            null, Gio.DBusCallFlags.NO_AUTO_START, 2000, cancellable, (connection, result) => {
                let sunrise = -1, sunset = -1;
                try {
                    const [props] = connection.call_finish(result).deepUnpack();
                    sunrise = props.Sunrise?.unpack() ?? -1;
                    sunset = props.Sunset?.unpack() ?? -1;
                } catch (e) {
                    if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        return;
                }
                if (cancellable.is_cancelled() || !this._settings)
                    return;
                this._cancellable = null;
                const known = sunrise >= 0 && sunrise < 24 && sunset >= 0 && sunset < 24 && sunrise !== sunset;
                callback(known ? sunrise : DAY, known ? sunset : NIGHT);
            });
    }

    _check() {
        if (!this._on())
            return;
        this._times((dayStarts, nightStarts) => {
            if (!this._on())
                return;
            // The same hour twice is no day at all: the usual hours instead
            if (Math.abs(dayStarts - nightStarts) < 1 / 60) {
                dayStarts = DAY;
                nightStarts = NIGHT;
            }
            const now = GLib.DateTime.new_now_local();
            const part = partOfDay(now, dayStarts, nightStarts);
            const id = `${part.side} ${day(part.since)}`;
            if (this._last() !== id) {
                // Remembered at once, so that a theme switch that fails is not asked again until the next boundary
                this._run('daynight', ['daynight', part.side]);
                this._remember(id);
            }
            this._arm(now, part.next);
        });
    }

    _arm(now, next) {
        if (this._timeout)
            GLib.source_remove(this._timeout);
        const seconds = Math.max(1, Math.ceil(next.difference(now) / 1e6) + SLACK);
        this._next = next.format_iso8601();
        this._timeout = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, seconds, () => {
            this._timeout = 0;
            this._check();
            return GLib.SOURCE_REMOVE;
        });
    }

    _last() {
        try {
            const [, bytes] = GLib.file_get_contents(STATE);
            return new TextDecoder().decode(bytes).trim();
        } catch {
            return null;
        }
    }

    _remember(id) {
        try {
            GLib.mkdir_with_parents(STATE_DIR, 0o700);
            GLib.file_set_contents(STATE, `${id}\n`);
        } catch (e) {
            console.warn(`Amethystora UI: ${STATE} could not be written: ${e.message}`);
        }
    }
}
