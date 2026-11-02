// Click with the keyboard (plans/ui-ideas/spec-later.md, R5-36): Super+Alt+K puts a letter on every button, link, box
// and tab of the window in use; typing the letters clicks it, and a field gets the focus. The controls come from the
// window's accessibility tree, read by a program of its own (/usr/libexec/amethystora-letters), so that an app that
// does not answer never makes the desktop wait: it says where the controls are, and is told which one to press. It
// runs only while the letters show. While they do, the shell holds the keyboard (a modal grab, as a menu does);
// Escape, a click, a letter that matches nothing, or the window moving or going takes them away, and the window then
// has its keyboard and its input method back. Offered and off: the key `letters`, without which the shortcut is not
// even bound.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {giveBackTextInput} from '../emoji/paste.js';

const HELPER = '/usr/libexec/amethystora-letters';
// The home row first: the letters easiest to reach go on the first controls
const ALPHABET = 'asdfjklghqweruiopzxcvbnmty';
// The program has this long to say where the controls are
const ANSWER_MS = 8000;
const MOST_LINE = 256 * 1024;

// The labels for n controls: one letter each while they suffice, two otherwise (never a label that starts another)
export function labels(n) {
    if (n <= ALPHABET.length)
        return [...ALPHABET].slice(0, n);
    const out = [];
    for (const first of ALPHABET) {
        for (const second of ALPHABET) {
            if (out.length < n)
                out.push(first + second);
        }
    }
    return out;
}

export class Letters {
    constructor() {
        this._typed = '';
    }

    get showing() {
        return !!this._proc;
    }

    toggle() {
        if (this._proc)
            this.hide();
        else
            this.show();
    }

    show() {
        const window = global.display.focus_window;
        if (!window || window.window_type !== Meta.WindowType.NORMAL || Main.modalCount > 0 || Main.overview.visible)
            return;
        try {
            this._proc = Gio.Subprocess.new([HELPER, String(window.get_pid()), window.get_title() ?? ''],
                Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_PIPE);
        } catch (e) {
            console.warn(`Amethystora UI: the letters could not start: ${e.message}`);
            this._proc = null;
            return;
        }
        this._window = window;
        this._cancellable = new Gio.Cancellable();
        this._stdin = this._proc.get_stdin_pipe();
        this._stdout = new Gio.DataInputStream({base_stream: this._proc.get_stdout_pipe(), close_base_stream: true});
        // The window moving, changing size or going while the letters are worked out or shown: they would be wrong
        window.connectObject(
            'position-changed', () => this.hide(),
            'size-changed', () => this.hide(),
            'unmanaged', () => this.hide(), this);
        global.display.connectObject('notify::focus-window', () => {
            if (!this._grab && global.display.focus_window !== this._window)
                this.hide();
        }, this);
        this._answerId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ANSWER_MS, () => {
            this._answerId = 0;
            // The same words as a window with nothing to say: nothing came up, and the key is not left looking dead
            this._answered('{"error": "none"}');
            return GLib.SOURCE_REMOVE;
        });
        const stdout = this._stdout;
        stdout.read_line_async(GLib.PRIORITY_DEFAULT, this._cancellable, (stream, result) => {
            let line = null;
            try {
                [line] = stream.read_line_finish_utf8(result);
            } catch {}
            if (this._stdout !== stdout)
                return;
            if (this._answerId)
                GLib.source_remove(this._answerId);
            this._answerId = 0;
            this._answered(line);
        });
    }

    _answered(line) {
        let answer = null;
        try {
            answer = line && line.length <= MOST_LINE ? JSON.parse(line) : null;
        } catch {}
        const frame = answer?.frame;
        const items = answer?.items;
        if (!Array.isArray(frame) || frame.length !== 4 || !Array.isArray(items) || items.length === 0 ||
            !this._window || global.display.focus_window !== this._window) {
            if (answer?.error === 'none' || answer?.error === 'window')
                Main.notify(_('Click with the keyboard'), _('This window says nothing of its buttons. Apps made with GTK say the most.'));
            this.hide();
            return;
        }
        this._draw(frame, items);
    }

    // The letters over the window: each control's place in the window's own coordinates, from the window's origin as
    // its accessibility tree has it to the window's frame on the screen
    _draw(frame, items) {
        const rect = this._window.get_frame_rect();
        const [fx, fy] = frame.map(Number);
        this._overlay = new St.Widget({
            style_class: 'amethystora-letters', reactive: true, can_focus: true,
            x: rect.x, y: rect.y, width: rect.width, height: rect.height,
        });
        this._overlay.accessible_name = _('Letters to click with');
        this._labels = [];
        const names = labels(items.length);
        items.forEach((item, index) => {
            if (!Array.isArray(item) || item.length !== 4)
                return;
            const [x, y, w, h] = item.map(Number);
            const left = x - fx;
            const top = y - fy;
            // Only what lies inside the window
            if (![left, top, w, h].every(Number.isFinite) || left + w <= 0 || top + h <= 0 || left >= rect.width ||
                top >= rect.height)
                return;
            const label = new St.Label({style_class: 'amethystora-letters-label', text: names[index].toUpperCase()});
            label.clutter_text.use_markup = false;
            this._overlay.add_child(label);
            label.set_position(Math.max(0, Math.round(left)), Math.max(0, Math.round(top + (h - 22) / 2)));
            this._labels.push({label, name: names[index], index});
        });
        if (this._labels.length === 0) {
            this.hide();
            return;
        }
        Main.layoutManager.addTopChrome(this._overlay);
        this._grab = Main.pushModal(this._overlay, {actionMode: Shell.ActionMode.POPUP});
        this._overlay.grab_key_focus();
        this._overlay.connectObject(
            'key-press-event', (actor, event) => this._key(event),
            'button-press-event', () => {
                this.hide();
                return Clutter.EVENT_STOP;
            }, this);
    }

    _key(event) {
        const symbol = event.get_key_symbol();
        if (symbol === Clutter.KEY_Escape) {
            this.hide();
            return Clutter.EVENT_STOP;
        }
        if (symbol === Clutter.KEY_BackSpace) {
            this._typed = this._typed.slice(0, -1);
            this._mark();
            return Clutter.EVENT_STOP;
        }
        const letter = String.fromCodePoint(Clutter.keysym_to_unicode(symbol) || 32).toLowerCase();
        if (!ALPHABET.includes(letter)) {
            this.hide();
            return Clutter.EVENT_STOP;
        }
        this._typed += letter;
        const matches = this._labels.filter(entry => entry.name.startsWith(this._typed));
        if (matches.length === 0) {
            this.hide();
        } else if (matches.length === 1 && matches[0].name === this._typed) {
            const {index} = matches[0];
            // The grab goes first: the window takes the click with its keyboard back
            this._write(`press ${index}`);
            this.hide({keepHelper: true});
        } else {
            this._mark();
        }
        return Clutter.EVENT_STOP;
    }

    // The labels that still match what was typed stay; the others fade
    _mark() {
        for (const {label, name} of this._labels)
            label.opacity = name.startsWith(this._typed) ? 255 : 60;
    }

    _write(line) {
        try {
            this._stdin?.write_all(new TextEncoder().encode(`${line}\n`), null);
        } catch {}
    }

    // Takes the letters away; the program is told to end (closing its input), unless it was just told what to press,
    // after which it ends by itself
    hide({keepHelper = false} = {}) {
        if (this._answerId)
            GLib.source_remove(this._answerId);
        this._answerId = 0;
        this._window?.disconnectObject(this);
        this._window = null;
        global.display.disconnectObject(this);
        this._cancellable?.cancel();
        this._cancellable = null;
        const grabbed = !!this._grab;
        const grab = this._grab;
        this._grab = null;
        if (grab) {
            try {
                Main.popModal(grab);
            } catch (e) {
                console.warn(`Amethystora UI: the letters' grab was already gone: ${e.message}`);
            }
        }
        this._overlay?.destroy();
        this._overlay = null;
        this._labels = [];
        this._typed = '';
        const proc = this._proc;
        this._proc = null;
        try {
            this._stdin?.close(null);
        } catch {}
        this._stdin = null;
        this._stdout = null;
        if (proc && !keepHelper)
            proc.send_signal(15);
        // The window's text input back from the shell's grab, as after a menu
        if (grabbed)
            giveBackTextInput();
    }

    state() {
        return {
            helper: this._proc?.get_identifier() ?? null,
            labels: this._labels?.map(({label, name}) => ({name, x: label.x, y: label.y})) ?? [],
            grabbed: !!this._grab,
        };
    }
}
