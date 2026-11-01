// A second time zone beside the clock (the pill's `second-zone` key, ame desktop clock zone, Control): after GNOME's
// own clock, a thin line and the time in that zone with its city ("Tokyo 06:41"), on the 24- or 12-hour clock as the
// clock itself is. It sits in GNOME's clock box, so it goes wherever the clock goes (into the live pill, or a bar
// arranged otherwise). It is redrawn when GNOME's own wall clock ticks, so it never wakes the shell by itself, and it
// exists only while a zone is chosen. GNOME's clock and calendar keys (24 hours, seconds, weekday, date, week numbers)
// are GNOME's own and need nothing here.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const INTERFACE = 'org.gnome.desktop.interface';
// A zone's name as the system's time-zone data writes them: an area and a place, perhaps a part more
const NAME = /^[A-Za-z]+(\/[A-Za-z0-9_+-]{1,40}){1,2}$/;

// The city of a zone's name: its last part, its underscores as spaces ("America/New_York" is "New York")
export function cityOf(zone) {
    return zone.split('/').pop().replaceAll('_', ' ');
}

export class Zone {
    constructor(settings) {
        this._settings = settings;
        this._box = null;
        this._zone = null;
    }

    enable() {
        this._interface = new Gio.Settings({schema_id: INTERFACE});
        this._interface.connectObject('changed::clock-format', () => this._tick(), this);
        this._settings.connectObject('changed::second-zone', () => this._sync(), this);
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._settings = null;
        this._interface?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._interface?.run_dispose();
        this._interface = null;
        this._drop();
    }

    // For the tests: what it shows, or null
    get text() {
        return this._box ? this._label.text : null;
    }

    _sync() {
        const name = this._settings?.get_string('second-zone') ?? '';
        let zone = null;
        if (NAME.test(name)) {
            try {
                zone = GLib.TimeZone.new_identifier(name);
            } catch {
                zone = null;
            }
        }
        if (!zone) {
            this._drop();
            return;
        }
        this._zone = zone;
        this._city = cityOf(name);
        if (!this._build())
            return;
        this._tick();
    }

    // In GNOME's clock box, after its clock
    _build() {
        if (this._box)
            return true;
        const dateMenu = Main.panel.statusArea.dateMenu;
        const clock = dateMenu?._clockDisplay;
        const holder = clock?.get_parent();
        if (!holder)
            return false;
        this._box = new St.BoxLayout({style_class: 'amethystora-pill-zone', y_align: Clutter.ActorAlign.CENTER});
        this._box.add_child(new St.Widget({style_class: 'amethystora-pill-zone-line', y_align: Clutter.ActorAlign.CENTER}));
        this._label = new St.Label({style_class: 'amethystora-pill-zone-time', y_align: Clutter.ActorAlign.CENTER});
        this._label.clutter_text.use_markup = false;
        this._box.add_child(this._label);
        holder.insert_child_above(this._box, clock);
        this._wallClock = dateMenu._clock ?? null;
        this._wallClock?.connectObject('notify::clock', () => this._tick(), this);
        return true;
    }

    _drop() {
        this._wallClock?.disconnectObject(this);
        this._wallClock = null;
        this._box?.destroy();
        this._box = null;
        this._label = null;
        this._zone = null;
    }

    _tick() {
        if (!this._box || !this._zone)
            return;
        const now = GLib.DateTime.new_now(this._zone);
        const twelve = this._interface?.get_string('clock-format') === '12h';
        let time = now.format(twelve ? '%l:%M %p' : '%H:%M').trim();
        // GNOME's clock writes its colon as a ratio sign in some languages: the same here
        const shown = Main.panel.statusArea.dateMenu?._clockDisplay?.text ?? '';
        if (shown.includes('∶'))
            time = time.replace(':', '∶');
        const text = `${this._city} ${time}`;
        if (this._label.text !== text)
            this._label.text = text;
    }
}
