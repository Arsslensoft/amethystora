// What happened while it slept. Open the lid to a much emptier battery and a notice says how much went, what woke the
// computer (from /usr/libexec/amethystora-sleep-report, only once the loss is large) and offers to stop that device
// doing it again. Said once for each device, or once a day when nothing can be named. It needs a battery.
//
// A screen lock stops the extension, and the computer sleeps after it, so the signal that says it is going to sleep and
// the one that says it woke are heard at module scope, where they outlive the lock, and take a reading of the battery
// each (only in memory) a moment before and a few seconds after. Silence unless the loss is unusual. What it names is
// only what the helper could tell from the journal: never the key or the mouse that opened the lid. The mark of what was
// said (~/.local/state/amethystora/sleep-report.json) holds salted checksums only, never a device's name or ID.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';

import {judge, listTimes, reading, remember, suspendedSeconds, wasSaid} from './sleptlogic.js';

Gio._promisify(Gio.File.prototype, 'load_contents_async');
Gio._promisify(Gio.File.prototype, 'replace_contents_async');
Gio._promisify(Gio.File.prototype, 'make_directory_async');
Gio._promisify(Gio.Subprocess.prototype, 'communicate_utf8_async');
Gio._promisify(Gio.Subprocess.prototype, 'wait_check_async');

const HELPER = '/usr/libexec/amethystora-sleep-report';
const PKEXEC = '/usr/bin/pkexec';
const UPOWER = 'org.freedesktop.UPower';
const DISPLAY_DEVICE = '/org/freedesktop/UPower/devices/DisplayDevice';
const STATE_DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const STATE = GLib.build_filenamev([STATE_DIR, 'sleep-report.json']);
// UPower refreshes the battery as the computer wakes: its number is read this many seconds after
const SETTLE = 3;
const decoder = new TextDecoder();

// What the battery held as the computer went away and as it came back (in memory only), and who tells the person. All of
// it outlives the extension across a lock
let before = null;
let after = null;
let turnedOff = 0;
let watch = 0;
let settling = 0;
let active = null;
let source = null;

// What UPower says of the battery now, or null without one: one call, no proxy that would hear each of its changes
async function batteryNow() {
    const reply = await Gio.DBus.system.call(UPOWER, DISPLAY_DEVICE, 'org.freedesktop.DBus.Properties', 'GetAll',
        new GLib.Variant('(s)', [`${UPOWER}.Device`]), new GLib.VariantType('(a{sv})'), Gio.DBusCallFlags.NO_AUTO_START, 2000, null);
    const properties = reply.get_child_value(0).deepUnpack();
    const value = name => properties[name]?.deepUnpack?.();
    return value('IsPresent') === true ? {percent: value('Percentage')} : null;
}

async function suspendedNow() {
    const [bytes] = await Gio.File.new_for_path('/proc/uptime').load_contents_async(null);
    return suspendedSeconds(decoder.decode(bytes), GLib.get_monotonic_time());
}

async function readNow() {
    const battery = await batteryNow();
    return battery ? {...battery, suspended: await suspendedNow(), mono: GLib.get_monotonic_time(), real: GLib.get_real_time()} : null;
}

// Kept as `before` or `after`, unless the key was turned off meanwhile
async function take(which) {
    const asked = turnedOff;
    const kept = await readNow();
    if (asked !== turnedOff)
        return;
    if (which === 'before')
        before = kept;
    else
        after = kept;
}

function onSleep(parameters) {
    if (parameters.deepUnpack()[0]) {
        after = null;
        take('before').catch(e => console.debug(`Amethystora UI: the battery could not be read: ${e.message}`));
        return;
    }
    if (settling)
        GLib.source_remove(settling);
    settling = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, SETTLE, () => {
        settling = 0;
        take('after').then(() => active?._woke()).catch(e => console.debug(`Amethystora UI: the battery could not be read: ${e.message}`));
        return GLib.SOURCE_REMOVE;
    });
}

function startWatching() {
    if (watch)
        return;
    watch = Gio.DBus.system.signal_subscribe('org.freedesktop.login1', 'org.freedesktop.login1.Manager',
        'PrepareForSleep', '/org/freedesktop/login1', null, Gio.DBusSignalFlags.NONE, (c, s, p, i, n, parameters) => onSleep(parameters));
}

// Its key turned off: nothing stays held or heard, and a later night is not told from today's reading
export function forgetSleep() {
    turnedOff++;
    before = null;
    after = null;
    if (watch)
        Gio.DBus.system.signal_unsubscribe(watch);
    watch = 0;
    if (settling)
        GLib.source_remove(settling);
    settling = 0;
    source?.destroy();
    source = null;
}

async function readState() {
    for (const path of [STATE, `${STATE}.bak`]) {
        try {
            const [bytes] = await Gio.File.new_for_path(path).load_contents_async(null);
            return JSON.parse(decoder.decode(bytes));
        } catch {}
    }
    return {};
}

// The folder, made one level at a time without blocking the shell
async function makeFolder(path) {
    try {
        await Gio.File.new_for_path(path).make_directory_async(GLib.PRIORITY_DEFAULT, null);
    } catch (e) {
        if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.EXISTS))
            return;
        if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.NOT_FOUND))
            throw e;
        await makeFolder(GLib.path_get_dirname(path));
        await makeFolder(path);
    }
}

async function writeState(state) {
    const bytes = new GLib.Bytes(new TextEncoder().encode(`${JSON.stringify(state)}\n`));
    await makeFolder(STATE_DIR);
    for (const path of [`${STATE}.bak`, STATE]) {
        await Gio.File.new_for_path(path).replace_contents_async(bytes, null, false,
            Gio.FileCreateFlags.PRIVATE | Gio.FileCreateFlags.REPLACE_DESTINATION, null);
    }
}

function checksum(text) {
    return GLib.compute_checksum_for_string(GLib.ChecksumType.SHA256, text, -1).slice(0, 32);
}

// The notice. It belongs to the shell, not to the extension, so that a lock does not take it away unread
function notify(title, body, actions) {
    if (!source) {
        source = new MessageTray.Source({title: _('Battery'), iconName: 'battery-caution-symbolic'});
        source.connect('destroy', () => (source = null));
        Main.messageTray.add(source);
    }
    const notification = new MessageTray.Notification({source, title, body});
    for (const [label, callback] of actions)
        notification.addAction(label, callback);
    source.addNotification(notification);
    return {title, body, actions: actions.map(([label]) => label)};
}

// pkexec asks for an administrator's password: the setting is the machine's. Undo is the same command the other way.
async function wake(word, id) {
    const process = Gio.Subprocess.new([PKEXEC, HELPER, word, id], Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
    await process.wait_check_async(null);
}

async function stopWaking(device) {
    try {
        await wake('stop', device.id);
    } catch {
        notify(_('It can still wake the computer'), _('The setting was not changed.'), []);
        return;
    }
    notify(_('%s will not wake it now').format(device.name), _('Undo it here, or in Control, under System.'),
        [[_('Undo'), () => wake('allow', device.id).catch(() => {})]]);
}

export class SleptReport {
    enable() {
        this._stopped = false;
        startWatching();
        active = this;
        // The computer slept while the screen was locked: its readings are waiting
        if (before && after)
            this._woke();
    }

    disable() {
        this._stopped = true;
        if (active === this)
            active = null;
    }

    _woke() {
        if (this._stopped || !before || !after)
            return;
        const pair = {before, after};
        before = null;
        after = null;
        this._look(pair).then(finished => {
            // Stopped halfway (the screen locked): the next start looks again
            if (!finished && !before && !after) {
                before = pair.before;
                after = pair.after;
            }
        }).catch(e => console.warn(`Amethystora UI: what happened while it slept: ${e.message}`));
    }

    // True when it got to the end
    async _look(pair) {
        const {large, drop} = judge(pair.before, pair.after);
        if (!large)
            return true;
        const why = await this._why(pair.before);
        if (this._stopped)
            return false;
        await this.report(drop, why);
        return true;
    }

    // Monotonic time when it went away: a device counts only if it woke the machine after that
    async _why(kept) {
        const process = Gio.Subprocess.new([HELPER, 'why', '--json', '--after-ms', `${Math.floor(kept.mono / 1000)}`,
            '--since', `${Math.floor(kept.real / 1e6)}`], Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
        try {
            const [stdout] = await process.communicate_utf8_async(null, null);
            return JSON.parse(stdout);
        } catch {
            return null;
        }
    }

    // The notice, unless this device or (with nothing to name) this day was said already
    async report(drop, why) {
        const seen = reading(why);
        const key = seen.culprit ? seen.culprit.id : new Date().toISOString().slice(0, 10);
        const state = await readState();
        const salt = typeof state.salt === 'string' && state.salt ? state.salt : GLib.uuid_string_random();
        const mark = checksum(`${salt}:${key}`);
        if (wasSaid(state, mark))
            return false;
        writeState({...remember(state, mark), salt}).catch(e => console.warn(`Amethystora UI: the sleep report's mark: ${e.message}`));
        const actions = [[_('Leave it'), () => {}]];
        const device = seen.culprit;
        if (device?.can_stop)
            actions.push([_('Stop it waking the computer'), () => stopWaking(device).catch(e => console.warn(`Amethystora UI: a device's wake setting: ${e.message}`))]);
        this.shown = notify(_('%d%% gone while it slept').format(drop), this._body(seen), actions);
        return true;
    }

    _body(seen) {
        const count = seen.times.length;
        const listed = listTimes(seen.times);
        let wokeText;
        if (listed) {
            const clock = listed.map(time => GLib.DateTime.new_from_unix_local(time).format('%H:%M')).join(', ');
            // Translators: how many times the computer woke by itself, then at what times
            const woke = ngettext('It woke once, at %s.', 'It woke %d times, at %s.', count);
            wokeText = count === 1 ? woke.format(clock) : woke.format(count, clock);
        } else {
            wokeText = _('It woke %d times.').format(count);
        }
        switch (seen.say) {
        case 'woken':
            return `${wokeText}\n${this._suspect(seen.culprit)}`;
        case 'woken-unnamed':
            return `${wokeText}\n${_('It did not say what woke it.')}`;
        case 'asleep':
            return _('Nothing woke it, so the battery went on sleep itself. Some computers use more than others while asleep.');
        default:
            return _('It did not say why.');
        }
    }

    // Only the last thing to fire is known, so it is named as what it looks like. A device names itself: plain text
    _suspect(device) {
        switch (device?.kind) {
        case 'bluetooth': return _('It looks like a Bluetooth device: %s.').format(device.name);
        case 'usb': return _('It looks like a USB device: %s.').format(device.name);
        case 'network': return _('It looks like the network card.');
        default: return _('It looks like a timer.');
        }
    }

    // For the suites and the harness: a night, as the two readings the signals would have taken
    fakeForTest(pastBefore, pastAfter) {
        before = pastBefore;
        after = pastAfter;
        this._woke();
    }

    get before() {
        return before;
    }

    get after() {
        return after;
    }
}
