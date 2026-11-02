// Get me through this: when the battery runs low, the pill's question offers one button that says what it changes. It
// holds the power saver (power-profiles-daemon's own hold, which ends by itself if the shell does and puts the profile
// back), dims the built-in screen (GNOME's own brightness, not while a call or a recording is on), and tells
// /usr/libexec/amethystora-metered to keep the backup and the containers' upgrade waiting (never longer than a week). The
// system update waits on its own only under 20% on battery (uupd's hardware check). The moment the machine is plugged
// in, or at Back to normal, everything goes back, a brightness the person has moved since excepted. What a person starts
// still runs.
//
// It gives everything back whatever happens. The brightness is kept in a small plain file while it is dimmed (a share of
// the screen is not private), since GNOME saves the dimmed value as the screen's own: a session that ends before the
// plug-in puts it back at the next start. Plugging in is UPower's OnBattery, followed through a screen lock (a lock stops
// the extension, and the hold stays), and so is the power saver being released by somebody else (ProfileReleased).
//
// The key get-through of org.gnome.shell.extensions.amethystora-ui.pill (ame desktop get-through, Control), read live;
// with it off the battery's question is what it always was. Nothing is listened to until the button is pressed, and then
// only UPower's and the power profiles' own signals; no timer.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import {loadInterfaceXML} from 'resource:///org/gnome/shell/misc/fileUtils.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {buttonRow, cardButton, retire} from './clock.js';
import {hoursMinutes, timeOfDay} from './text.js';

Gio._promisify(Gio.File.prototype, 'make_directory_async', 'make_directory_finish');
Gio._promisify(Gio.File.prototype, 'replace_contents_bytes_async', 'replace_contents_finish');
Gio._promisify(Gio.File.prototype, 'load_contents_async', 'load_contents_finish');
Gio._promisify(Gio.File.prototype, 'delete_async', 'delete_finish');

const UPOWER = 'org.freedesktop.UPower';
const UPOWER_PATH = '/org/freedesktop/UPower';
const DISPLAY_DEVICE = '/org/freedesktop/UPower/devices/DisplayDevice';
const PROFILES = 'org.freedesktop.UPower.PowerProfiles';
const PROFILES_PATH = '/org/freedesktop/UPower/PowerProfiles';
const APP_ID = 'org.amethystora.UI';
const METERED = '/usr/libexec/amethystora-metered';
const STATE_DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const STATE_FILE = GLib.build_filenamev([STATE_DIR, 'get-through.json']);
const MARK = GLib.build_filenamev([GLib.get_user_runtime_dir(), 'amethystora', 'get-through']);
// The screen at this share of what it was, and never darker than the floor by doing so
const DIM_BY = 0.6;
const DIM_FLOOR = 0.3;
// A brightness this near the one it was set to is still the one set here (any step of GNOME's scale is wider)
const SAME = 0.011;

let DeviceProxy = null;
// While it is on, kept outside the class: a screen lock stops the extension, and the hold and the dimmed screen stay.
// {cookie, before, dimmedTo, left}: the hold's number, the brightness to put back and what it was set to, the time left
// when the button was pressed (seconds)
let held = null;
// The instance that is running, for the words; none while the extension is stopped (a lock)
let active = null;
// The signals followed while it is on: UPower's OnBattery and the power profiles' ProfileReleased
let watches = [];
// A brightness to put back as soon as the machine has a built-in screen again, and the order of the state file's jobs
let backlight = null;
let disk = Promise.resolve();
const WAITING = {};

// What the time left is, for the words: 0 until the battery has said
export function endsAt(seconds, now = Date.now()) {
    return seconds > 0 ? new Date(now + seconds * 1000) : null;
}

// --- The brightness's file ------------------------------------------------------------------------------------------

// One job at a time on the file, so that a delete never overtakes the write before it
function later(job) {
    disk = disk.then(job).catch(e => console.debug(`Amethystora UI: get-through.json: ${e.message}`));
    return disk;
}

function saveBrightness(record) {
    later(async () => {
        try {
            await Gio.File.new_for_path(STATE_DIR).make_directory_async(GLib.PRIORITY_DEFAULT, null);
        } catch (e) {
            if (!e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.EXISTS))
                throw e;
        }
        await Gio.File.new_for_path(STATE_FILE).replace_contents_bytes_async(
            new GLib.Bytes(new TextEncoder().encode(JSON.stringify(record))), null, false,
            Gio.FileCreateFlags.REPLACE_DESTINATION, null);
    });
}

function forgetBrightness() {
    later(async () => {
        try {
            await Gio.File.new_for_path(STATE_FILE).delete_async(GLib.PRIORITY_DEFAULT, null);
        } catch (e) {
            if (!e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.NOT_FOUND))
                throw e;
        }
    });
}

// What an earlier session left, or null
async function savedBrightness() {
    try {
        const [bytes] = await Gio.File.new_for_path(STATE_FILE).load_contents_async(null);
        const {before, dimmedTo} = JSON.parse(new TextDecoder().decode(bytes));
        const fine = value => Number.isFinite(value) && value >= 0 && value <= 1;
        return fine(before) && fine(dimmedTo) ? {before, dimmedTo} : null;
    } catch {
        return null;
    }
}

// Puts the brightness back, unless somebody has moved it since. A machine with no built-in screen at the moment (docked
// with the lid closed) has no scale to set: the brightness is put back when one comes, and GNOME's own copy of the
// dimmed value is not left to come back with the lid.
function putBack(record) {
    const manager = Main.brightnessManager;
    const scale = manager?.globalScale;
    if (!scale) {
        if (manager && !backlight) {
            backlight = record;
            manager.connectObject('changed', () => {
                if (manager.globalScale)
                    putBack(backlight);
            }, WAITING);
        }
        return;
    }
    stopWaiting();
    if (Math.abs(scale.value - record.dimmedTo) <= SAME)
        scale.value = record.before;
    forgetBrightness();
}

function stopWaiting() {
    Main.brightnessManager?.disconnectObject(WAITING);
    backlight = null;
}

// --- Following the machine ------------------------------------------------------------------------------------------

function release(cookie) {
    Gio.DBus.system.call(PROFILES, PROFILES_PATH, PROFILES, 'ReleaseProfile', new GLib.Variant('(u)', [cookie]),
        null, Gio.DBusCallFlags.NONE, -1, null, (connection, result) => {
            try {
                connection.call_finish(result);
            } catch {
                // Somebody chose a profile by hand, which ends every hold: nothing left to give back
            }
        });
}

function mark(word) {
    try {
        Gio.Subprocess.new([METERED, 'through', word], Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE)
            .wait_async(null, null);
    } catch (e) {
        console.warn(`Amethystora UI: ${METERED} did not start: ${e.message}`);
    }
}

// All of it back. Called with nothing held, it does nothing.
function undo() {
    if (!held)
        return;
    const mine = held;
    held = null;
    unwatch();
    if (mine.cookie)
        release(mine.cookie);
    if (mine.before !== null)
        putBack({before: mine.before, dimmedTo: mine.dimmedTo});
    mark('off');
}

const OVER = {
    plugged: () => _('Plugged in: everything is back'),
    released: () => _('The power mode was changed: everything is back'),
    failed: () => _('The power saver could not be held: nothing was changed'),
};

// It ends for a reason of the machine's: the words are said if the pill is there to say them
function finish(why) {
    if (!held)
        return;
    const shown = active;
    undo();
    if (shown) {
        shown._hide();
        shown._notice(OVER[why](), 'battery-full-charging-symbolic', 4, 'saver');
    }
}

// UPower's OnBattery as it is now: true, false, or null when it cannot be read (no change)
function checkMains() {
    Gio.DBus.system.call(UPOWER, UPOWER_PATH, 'org.freedesktop.DBus.Properties', 'Get',
        new GLib.Variant('(ss)', [UPOWER, 'OnBattery']), new GLib.VariantType('(v)'), Gio.DBusCallFlags.NO_AUTO_START,
        -1, null, (connection, result) => {
            try {
                const [value] = connection.call_finish(result).deepUnpack();
                if (value.unpack() === false)
                    finish('plugged');
            } catch {
                // UPower cannot be asked: the machine is taken to be as it was
            }
        });
}

function watch() {
    if (watches.length > 0)
        return;
    watches.push(Gio.DBus.system.signal_subscribe(UPOWER, 'org.freedesktop.DBus.Properties', 'PropertiesChanged',
        UPOWER_PATH, UPOWER, Gio.DBusSignalFlags.NONE, (c, s, p, i, name, parameters) => {
            const [, changed] = parameters.recursiveUnpack();
            if (changed?.OnBattery === false)
                finish('plugged');
        }));
    watches.push(Gio.DBus.system.signal_subscribe(PROFILES, PROFILES, 'ProfileReleased', PROFILES_PATH, null,
        Gio.DBusSignalFlags.NONE, (c, s, p, i, name, parameters) => {
            const [cookie] = parameters.deepUnpack();
            if (held?.cookie && cookie === held.cookie)
                finish('released');
        }));
}

function unwatch() {
    for (const id of watches)
        Gio.DBus.system.signal_unsubscribe(id);
    watches = [];
}

// For the pill, when the part is not wanted (its key went off) or the pill stops: what a lock kept goes back now, and a
// lock itself keeps it
export function settleThrough(stopping) {
    if (stopping)
        stopWaiting();
    if (held && !(stopping && Main.sessionMode.isLocked))
        undo();
}

export class Through {
    constructor(activities, notice, capture, privacy = () => null) {
        this.id = 'through';
        this.plate = 'saver';
        this._activities = activities;
        this._notice = notice;
        this._capture = capture;
        this._privacy = privacy;
    }

    enable() {
        this._cancellable = new Gio.Cancellable();
        active = this;
        if (held) {
            // Kept through a lock: the machine may have been plugged in and unplugged meanwhile
            watch();
            checkMains();
            this._follow();
            return;
        }
        // A mark or a dimmed screen that an earlier session left behind is taken away
        if (GLib.file_test(MARK, GLib.FileTest.EXISTS))
            mark('off');
        const cancellable = this._cancellable;
        savedBrightness().then(record => {
            if (record && !held && !cancellable.is_cancelled())
                putBack(record);
        });
    }

    disable() {
        if (active === this)
            active = null;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._proxy?.disconnectObject(this);
        this._proxy = null;
        // Only a screen lock keeps it on: turned off or stopped any other way, all of it goes back now
        if (held && !Main.sessionMode.isLocked)
            undo();
        this._hide();
    }

    get on() {
        return !!held;
    }

    // --- What it does -----------------------------------------------------------------------------------------------

    // The button. `left` is the time the battery says it has now (seconds).
    start(left = 0) {
        if (held)
            return;
        held = {cookie: 0, before: null, dimmedTo: null, left};
        active = this;
        watch();
        this._hold();
        this._dim();
        mark('on');
        this._follow();
    }

    stop() {
        if (!held)
            return;
        undo();
        this._hide();
    }

    _hold() {
        const mine = held;
        Gio.DBus.system.call(PROFILES, PROFILES_PATH, PROFILES, 'HoldProfile',
            new GLib.Variant('(sss)', ['power-saver', _('Making the battery last until you plug in'), APP_ID]),
            new GLib.VariantType('(u)'), Gio.DBusCallFlags.NONE, -1, null, (connection, result) => {
                let cookie = 0;
                try {
                    [cookie] = connection.call_finish(result).deepUnpack();
                } catch (e) {
                    console.warn(`Amethystora UI: the power saver could not be held: ${e.message}`);
                }
                if (held !== mine) {
                    // Over before the answer came: the hold made now is given back
                    if (cookie)
                        release(cookie);
                } else if (cookie) {
                    mine.cookie = cookie;
                } else {
                    // Without the hold nothing of it is true: no card goes on saying it
                    finish('failed');
                }
            });
    }

    // Whether somebody is in a call or showing the screen: the phone's call, a recording or a share, the microphone or
    // the camera (the privacy dots know the last two; with them off, a call on the computer cannot be told)
    _busy() {
        if (this._activities.get('call') || this._capture?.active)
            return true;
        const state = this._privacy()?.state;
        return !!(state?.microphone || state?.camera);
    }

    // Not during a call or a recording: the person is showing or looking at something. Only a built-in screen has a scale.
    _dim() {
        const scale = Main.brightnessManager?.globalScale;
        if (!scale || this._busy())
            return;
        const dimmed = Math.max(DIM_FLOOR, scale.value * DIM_BY);
        if (dimmed >= scale.value)
            return;
        held.before = scale.value;
        scale.value = dimmed;
        held.dimmedTo = scale.value;
        saveBrightness({before: held.before, dimmedTo: held.dimmedTo});
    }

    // --- Following the battery --------------------------------------------------------------------------------------

    _follow() {
        this._show();
        if (this._proxy || !this._cancellable)
            return;
        DeviceProxy ??= Gio.DBusProxy.makeProxyWrapper(loadInterfaceXML('org.freedesktop.UPower.Device'));
        new DeviceProxy(Gio.DBus.system, UPOWER, DISPLAY_DEVICE, (proxy, error) => {
            if (error || !this._cancellable)
                return;
            this._proxy = proxy;
            proxy.connectObject('g-properties-changed', () => this._changed(), this);
            this._changed();
        }, this._cancellable, Gio.DBusProxyFlags.DO_NOT_AUTO_START);
    }

    // The percentage and the time, for the words only: whether it is over is the machine's (OnBattery), not this
    _reading() {
        if (this._forced)
            return this._forced;
        return this._proxy
            ? {percentage: this._proxy.Percentage ?? 0, left: Number(this._proxy.TimeToEmpty ?? 0)}
            : null;
    }

    _changed() {
        if (!this._reading() || !held)
            return;
        const text = this.text;
        if (this._label && this._label.text !== text) {
            this._label.text = text;
            this._activities.update('through');
        }
        this._fill();
    }

    _show() {
        this.pill();
        if (this._activities.get('through') !== this)
            this._activities.set('through', this);
    }

    _hide() {
        if (this._activities.get('through') === this)
            this._activities.set('through', null);
        retire(this._content);
        this._content = null;
        this._label = null;
    }

    get shown() {
        return this._activities.get('through') === this;
    }

    // --- What it says -----------------------------------------------------------------------------------------------

    get text() {
        const end = endsAt(this._reading()?.left ?? 0);
        return end ? _('Lasting until about %s').format(timeOfDay(end)) : _('Making the battery last');
    }

    get accessible() {
        return this.text;
    }

    get row() {
        return {icon: 'power-profile-power-saver-symbolic', text: this.text};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-through'});
            this._content.add_child(new St.Icon({icon_name: 'power-profile-power-saver-symbolic', icon_size: 14,
                y_align: Clutter.ActorAlign.CENTER}));
            this._label = new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER});
            this._content.add_child(this._label);
        }
        return this._content;
    }

    // The battery in a line: "Battery: 24%, about 1 h 40 min", the time only once the battery has said
    _title(percentage, left) {
        return left > 0
            ? _('Battery: %d%%, about %s').format(Math.round(percentage), hoursMinutes(left))
            : _('Battery: %d%%').format(Math.round(percentage));
    }

    _changes() {
        const line = new St.BoxLayout({style_class: 'amethystora-pill-news-row'});
        const text = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        const what = new St.Label({style_class: 'amethystora-pill-news-item',
            text: _('Dimmer, slower, and the backup and the containers’ upgrade wait')});
        what.clutter_text.line_wrap = true;
        text.add_child(what);
        text.add_child(new St.Label({style_class: 'amethystora-pill-news-area', text: _('Until you plug in')}));
        line.add_child(text);
        return line;
    }

    // The notes under the changes, each a whole sentence and each true of what is done
    _notes(body) {
        const say = text => {
            const label = new St.Label({style_class: 'amethystora-pill-news-once', text});
            label.clutter_text.line_wrap = true;
            body.add_child(label);
        };
        // The screen: left alone only for what can be told. With the dots off, a call on the computer cannot.
        say(this._privacy()
            ? _('The screen is not dimmed while a call, a recording, the microphone or the camera is on.')
            : _('The screen is dimmed even during a call on this computer: the privacy dots, which could tell, are off.'));
        say(_('The system update waits on its own only under 20%, and an update or a backup you start yourself still runs.'));
    }

    // The offer, the battery's card while the key is on. `answer` says what the battery does with each button.
    offerCard(api, reading, answer) {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body amethystora-pill-news-card',
            orientation: Clutter.Orientation.VERTICAL});
        body.add_child(new St.Label({style_class: 'amethystora-pill-news-title',
            text: this._title(reading.percentage, reading.timeToEmpty)}));
        body.add_child(this._changes());
        const note = new St.Label({style_class: 'amethystora-pill-news-once',
            text: _('The time is the battery’s own estimate.')});
        note.clutter_text.line_wrap = true;
        body.add_child(note);
        this._notes(body);
        body.add_child(buttonRow([
            cardButton(_('Get me through'), () => {
                api.close();
                answer(true);
            }, {primary: true}),
            cardButton(_('Not now'), () => {
                api.close();
                answer(false);
            }),
        ]));
        return body;
    }

    // While it is on: how long now, and how long when the button was pressed, both the battery's own estimates
    card(api) {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body amethystora-pill-news-card',
            orientation: Clutter.Orientation.VERTICAL});
        this._cardTitle = new St.Label({style_class: 'amethystora-pill-news-title'});
        body.add_child(this._cardTitle);
        body.add_child(this._changes());
        this._cardNote = new St.Label({style_class: 'amethystora-pill-news-once'});
        this._cardNote.clutter_text.line_wrap = true;
        body.add_child(this._cardNote);
        this._notes(body);
        body.add_child(buttonRow([cardButton(_('Back to normal'), () => {
            api.close();
            this.stop();
        })]));
        body.connect('destroy', () => {
            this._cardTitle = null;
            this._cardNote = null;
        });
        this._fill();
        return body;
    }

    _fill() {
        if (!this._cardTitle)
            return;
        const reading = this._reading();
        this._cardTitle.text = reading ? this._title(reading.percentage, reading.left) : _('Making the battery last');
        this._cardNote.text = held?.left > 0
            ? _('It was about %s when you pressed the button. Both times are the battery’s own estimates.').format(hoursMinutes(held.left))
            : _('The time is the battery’s own estimate.');
    }

    // --- For the suites and the harness -----------------------------------------------------------------------------

    state() {
        return {
            on: !!held,
            shown: this.shown,
            text: this.shown ? this.text : '',
            cookie: held?.cookie ?? 0,
            before: held?.before ?? null,
            dimmedTo: held?.dimmedTo ?? null,
            waiting: !!backlight,
            watching: watches.length,
        };
    }

    // As if UPower read {percentage, left}
    readingForTest(reading) {
        this._forced = reading;
        this._changed();
        return this.state();
    }

    // As if UPower said OnBattery, or the power saver were released by somebody else
    mainsForTest(onBattery) {
        if (onBattery === false)
            finish('plugged');
        return this.state();
    }

    releasedForTest() {
        finish('released');
        return this.state();
    }
}
