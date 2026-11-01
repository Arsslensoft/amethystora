// A slow computer is noticed: once a session, shortly after it starts, whether the screen is drawn without a graphics
// driver (in software, as GNOME itself tells by the same call) and how much memory the computer has. A computer that
// draws in software, or has under 8 GB, is offered the light desktop once, in a notification with its two answers;
// the offer is remembered for this computer's graphics and memory as it is made, answered, dismissed or left alone, so
// it is asked again only after they change (a new graphics card or driver). Nothing here runs after that: no timer, no
// watch.
//
// What was asked is kept in ~/.local/state/amethystora/light-desktop.json ({"version": 1, "asked": "<what was read>"}),
// written whole with a copy beside it; a missing or bad file only means asking once more.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.comfort';
const STATE_DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const STATE = GLib.build_filenamev([STATE_DIR, 'light-desktop.json']);
// MemTotal under this many kB counts as under 8 GB: an 8 GB computer reports about 7.6 million, after what the
// firmware and the graphics keep
const LITTLE_MEMORY_KB = 7000000;
// Seconds after the session has started: the first login's welcome has had its moment
const AFTER_START = 15;
const DRIVER = /^[A-Za-z0-9_-]{1,40}$/;

// Read once per shell, not at every unlock (the extension starts again then)
let checked = false;

const decoder = new TextDecoder();

function readAsked() {
    for (const path of [STATE, `${STATE}.bak`]) {
        try {
            const [ok, bytes] = GLib.file_get_contents(path);
            const state = ok ? JSON.parse(decoder.decode(bytes)) : null;
            if (typeof state?.asked === 'string' && state.asked.length <= 200)
                return state.asked;
        } catch {}
    }
    return null;
}

function writeAsked(asked) {
    const text = `${JSON.stringify({version: 1, asked})}\n`;
    try {
        GLib.mkdir_with_parents(STATE_DIR, 0o700);
        // The copy first: a crash between the two leaves one whole file
        GLib.file_set_contents(`${STATE}.bak`, text);
        GLib.file_set_contents(STATE, text);
    } catch (e) {
        console.warn(`Amethystora UI: the light desktop's question could not be remembered: ${e.message}`);
    }
}

// The computer's memory in kB, or 0 when it cannot be read
function memoryKb() {
    try {
        const [ok, bytes] = GLib.file_get_contents('/proc/meminfo');
        const match = ok ? /^MemTotal:\s+(\d+)\s+kB/m.exec(decoder.decode(bytes)) : null;
        return match ? Number(match[1]) : 0;
    } catch {
        return 0;
    }
}

// The kernel drivers of the graphics devices, by name, sorted: what changes when the graphics do
function drivers() {
    const names = new Set();
    try {
        const folder = Gio.File.new_for_path('/sys/class/drm');
        const children = folder.enumerate_children('standard::name', Gio.FileQueryInfoFlags.NONE, null);
        let info;
        while ((info = children.next_file(null))) {
            const card = info.get_name();
            if (!/^card\d+$/.test(card))
                continue;
            try {
                const link = folder.get_child(card).get_child('device').get_child('driver')
                    .query_info('standard::symlink-target', Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, null);
                const name = GLib.path_get_basename(link.get_symlink_target() ?? '');
                if (DRIVER.test(name))
                    names.add(name);
            } catch {}
        }
        children.close(null);
    } catch {}
    return [...names].sort();
}

// What the check found: software or a GPU, memory little or enough, and the drivers, as one line
export function fingerprint({accelerated, memory, drivers: list}) {
    const low = memory > 0 && memory < LITTLE_MEMORY_KB;
    return `${accelerated ? 'gpu' : 'software'}|${low ? 'low' : 'ok'}|${list.join(',')}`;
}

export class SlowCheck {
    constructor(ext, settings) {
        this._ext = ext;
        this._settings = settings;
        this._timer = 0;
        this._notification = null;
    }

    start() {
        if (checked)
            return;
        checked = true;
        const later = () => {
            this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_LOW, AFTER_START, () => {
                this._timer = 0;
                try {
                    this.check();
                } catch (e) {
                    console.warn(`Amethystora UI: the slow computer's check failed: ${e.message}`);
                }
                return GLib.SOURCE_REMOVE;
            });
        };
        if (Main.layoutManager._startingUp)
            Main.layoutManager.connectObject('startup-complete', () => {
                Main.layoutManager.disconnectObject(this);
                later();
            }, this);
        else
            later();
    }

    stop() {
        Main.layoutManager.disconnectObject(this);
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        // A question still showing stays; its answers write the key through a settings object of their own
        this._settings = null;
    }

    // What the computer is, read now, or the stand-ins a test gives
    _facts(facts) {
        return {
            accelerated: facts?.accelerated ?? global.backend.is_rendering_hardware_accelerated(),
            memory: facts?.memory ?? memoryKb(),
            drivers: facts?.drivers ?? drivers(),
        };
    }

    // Offers the light desktop if this computer is slow and was not asked as it is now. Returns what it found:
    // {slow, software, low, fingerprint, offered}
    check(facts = null, again = false) {
        const read = this._facts(facts);
        const print = fingerprint(read);
        const software = !read.accelerated;
        const low = read.memory > 0 && read.memory < LITTLE_MEMORY_KB;
        const found = {slow: software || low, software, low, fingerprint: print, offered: false};
        if (!found.slow || !this._settings || this._settings.get_boolean('light'))
            return found;
        if (!again && readAsked() === print)
            return found;
        this._offer(software, print);
        found.offered = true;
        return found;
    }

    _offer(software, print) {
        this._notification?.destroy();
        const source = MessageTray.getSystemSource();
        const title = software
            ? _('This computer draws the screen without a graphics driver')
            : _('This computer has little memory');
        const body = [
            _('The glass and the motion will feel slow. The light desktop keeps the look and drops the costly parts:'),
            _('solid glass in place of blur, short and simple motion, no live window previews.'),
        ].join(' ');
        const notification = new MessageTray.Notification({source, title, body, iconName: 'video-display-symbolic'});
        // Through a settings object of the answer's own: the module may have stopped and started again meanwhile
        const ext = this._ext;
        // Asked once: a notice dismissed or left in the list is not offered again at the next login
        writeAsked(print);
        const answer = light => {
            if (!light)
                return;
            const settings = ext.getSettings(SCHEMA);
            settings.set_boolean('light', true);
            Gio.Settings.sync();
            settings.run_dispose();
        };
        notification.addAction(_('Use the light desktop'), () => answer(true));
        notification.addAction(_('Keep it as it is'), () => answer(false));
        notification.connect('destroy', () => {
            if (this._notification === notification)
                this._notification = null;
        });
        this._notification = notification;
        source.addNotification(notification);
    }
}
