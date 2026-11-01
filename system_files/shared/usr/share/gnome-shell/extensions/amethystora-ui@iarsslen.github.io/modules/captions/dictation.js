// Dictation (plans/ui-ideas/spec-later.md, R5-38): what is said into the microphone, typed into the text field in use,
// made on this computer by the captions' own recognizer (/usr/libexec/amethystora-captions dictate, Vosk) in the
// captions' language. Offered and off: while the key `dictation` of org.gnome.shell.extensions.amethystora-ui.captions
// is off, its shortcut is not even bound and nothing is made. On, Super+Ctrl+Shift+D starts listening for the window
// that has the focus, shows a small pill low on the screen ("Listening on this computer only", and the words while they
// form), and types each finished sentence into that window, as GNOME's on-screen keyboard types: through the input
// method when the field takes text from it, otherwise pasted (emoji/paste.js). The same key, the focus leaving that
// window, 15 seconds without a word, or two minutes in all end it. A password or PIN field is never typed into.
//
// Nothing is written anywhere: the sound goes from the microphone into the recognizer's memory, and the words into the
// pill and the field. The program runs only while the pill shows. GNOME's own microphone indicator shows meanwhile.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {paste} from '../emoji/paste.js';

// By its full path: a program of the same name earlier in the session's PATH is not started instead
const HELPER = '/usr/libexec/amethystora-captions';
const KEY = 'dictate';
// The most one dictation lasts, and how long it waits for a word before it stops by itself
const MOST_MS = 120000;
const QUIET_MS = 15000;
// After `stop`, the program has this long to say its last words and end
const GRACE_MS = 2000;
// A line from the program longer than this is not one of its own
const MOST_LINE = 4096;
// The words still forming that the pill shows, at most
const MOST_SHOWN = 120;
// The pill: 24 px above the work area's bottom, fading in over 150 ms
const BOTTOM = 24;
const SHOW_MS = 150;
const PULSE_MS = 700;
// Fields nothing is ever dictated into
const SECRET = [Clutter.InputContentPurpose.PASSWORD, Clutter.InputContentPurpose.PIN];

export class Dictation {
    constructor(settings, captions) {
        this._settings = settings;
        this._captions = captions;
        this._bound = false;
    }

    enable() {
        this._settings.connectObject('changed::dictation', () => this._sync(), this);
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this.stop();
        this._unbind();
    }

    _sync() {
        if (this._settings.get_boolean('dictation'))
            this._bind();
        else {
            this.stop();
            this._unbind();
        }
    }

    _bind() {
        if (this._bound)
            return;
        const action = Main.wm.addKeybinding(KEY, this._settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL, () => this.toggle());
        if (action === Meta.KeyBindingAction.NONE)
            console.warn('Amethystora UI: the dictation shortcut could not be set');
        this._bound = true;
    }

    _unbind() {
        if (this._bound)
            Main.wm.removeKeybinding(KEY);
        this._bound = false;
    }

    toggle() {
        if (this._proc)
            this._finish();
        else
            this.start();
    }

    start() {
        if (this._proc || Main.sessionMode.isLocked)
            return;
        const window = global.display.focus_window;
        if (!window) {
            Main.notify(_('Dictation'), _('Click into a text field first: dictation types into the window in use.'));
            return;
        }
        if (SECRET.includes(Main.inputMethod.content_purpose)) {
            Main.notify(_('Dictation'), _('Dictation never types into a password field.'));
            return;
        }
        const language = this._captions.language;
        if (!this._captions.modelHere(language)) {
            Main.notify(_('Dictation'), _('Dictation needs the speech model of its language: ame desktop dictation on'));
            return;
        }
        try {
            this._proc = Gio.Subprocess.new([HELPER, 'dictate', language],
                Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_PIPE);
        } catch (e) {
            console.warn(`Amethystora UI: dictation could not start: ${e.message}`);
            this._proc = null;
            return;
        }
        this._window = window;
        this._partial = '';
        this._typed = 0;
        this._cancellable = new Gio.Cancellable();
        this._stdin = this._proc.get_stdin_pipe();
        this._stdout = new Gio.DataInputStream({base_stream: this._proc.get_stdout_pipe(), close_base_stream: true});
        this._read();
        // The text goes to this window only: the focus leaving it, or the window going, ends dictation
        global.display.connectObject('notify::focus-window', () => {
            if (global.display.focus_window !== this._window)
                this.stop();
        }, this);
        window.connectObject('unmanaged', () => this.stop(), this);
        this._mostId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, MOST_MS, () => {
            this._mostId = 0;
            this._finish();
            return GLib.SOURCE_REMOVE;
        });
        this._heardSomething();
        this._showPill();
    }

    // Asked to end: what is being said is heard to its end, then the program ends by itself
    _finish() {
        if (!this._proc || this._finishing)
            return;
        this._finishing = true;
        this._write('stop');
        this._words?.set_text(_('Finishing…'));
        this._graceId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, GRACE_MS, () => {
            this._graceId = 0;
            this.stop();
            return GLib.SOURCE_REMOVE;
        });
    }

    // Ends at once: nothing more is typed
    stop() {
        for (const id of ['_mostId', '_quietId', '_graceId']) {
            if (this[id])
                GLib.source_remove(this[id]);
            this[id] = 0;
        }
        global.display.disconnectObject(this);
        this._window?.disconnectObject(this);
        this._window = null;
        this._cancellable?.cancel();
        this._cancellable = null;
        const proc = this._proc;
        this._proc = null;
        if (proc) {
            try {
                this._stdin?.close(null);
            } catch {}
            proc.send_signal(15);
        }
        this._stdin = null;
        this._stdout = null;
        this._finishing = false;
        this._partial = '';
        this._pulse?.remove_all_transitions();
        this._pill?.destroy();
        this._pill = null;
        this._pulse = null;
        this._words = null;
    }

    _write(word) {
        this._stdin?.write_bytes_async(new GLib.Bytes(new TextEncoder().encode(`${word}\n`)), GLib.PRIORITY_DEFAULT,
            this._cancellable, (stream, result) => {
                try {
                    stream.write_bytes_finish(result);
                } catch {}
            });
    }

    _read() {
        const stdout = this._stdout;
        stdout.read_line_async(GLib.PRIORITY_DEFAULT, this._cancellable, (stream, result) => {
            let line;
            try {
                [line] = stream.read_line_finish_utf8(result);
            } catch {
                line = null;
            }
            if (this._stdout !== stdout)
                return;
            if (line === null) {
                // The program ended (asked to, or by itself): the last words still forming were said, so they are typed
                if (this._finishing && this._partial)
                    this._type(this._partial);
                this.stop();
                return;
            }
            if (line.length <= MOST_LINE)
                this._heard(line);
            if (this._stdout === stdout)
                this._read();
        });
    }

    _heard(line) {
        let record;
        try {
            record = JSON.parse(line);
        } catch {
            return;
        }
        if (typeof record?.error === 'string') {
            Main.notify(_('Dictation stopped'), record.error === 'audio'
                ? _('No sound could be taken from the microphone.')
                : _('The speech recognizer stopped.'));
        } else if (typeof record?.text === 'string') {
            this._partial = '';
            this._type(record.text);
            this._heardSomething();
        } else if (typeof record?.partial === 'string') {
            this._partial = record.partial;
            if (!this._finishing)
                this._words?.set_text(record.partial.slice(-MOST_SHOWN));
            this._heardSomething();
        }
    }

    _heardSomething() {
        if (this._quietId)
            GLib.source_remove(this._quietId);
        this._quietId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, QUIET_MS, () => {
            this._quietId = 0;
            this._finish();
            return GLib.SOURCE_REMOVE;
        });
    }

    // A sentence into the field: only while that window still has the focus and the field is not a secret one; a
    // sentence after the first starts with a space
    _type(text) {
        const window = this._window;
        if (!window || global.display.focus_window !== window)
            return;
        if (SECRET.includes(Main.inputMethod.content_purpose)) {
            this.stop();
            Main.notify(_('Dictation'), _('Dictation never types into a password field.'));
            return;
        }
        const words = text.trim();
        if (!words)
            return;
        paste({text: `${this._typed > 0 ? ' ' : ''}${words}`, window});
        this._typed++;
        if (!this._finishing)
            this._words?.set_text('');
    }

    // --- The pill -----------------------------------------------------------------------------------------------

    _showPill() {
        this._pulse = new St.Widget({style_class: 'amethystora-dictation-dot', y_align: Clutter.ActorAlign.CENTER});
        const title = new St.Label({style_class: 'amethystora-dictation-title', text: _('Listening on this computer only')});
        this._words = new St.Label({style_class: 'amethystora-dictation-words', text: ''});
        // What the microphone heard is text, never markup
        for (const label of [title, this._words]) {
            label.clutter_text.use_markup = false;
            label.clutter_text.ellipsize = Pango.EllipsizeMode.START;
        }
        const text = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, y_align: Clutter.ActorAlign.CENTER});
        text.add_child(title);
        text.add_child(this._words);
        this._pill = new St.BoxLayout({style_class: 'amethystora-dictation-pill', reactive: false, opacity: 0});
        this._pill.add_child(this._pulse);
        this._pill.add_child(text);
        this._pill.accessible_name = _('Dictation: listening on this computer only');
        Main.layoutManager.addTopChrome(this._pill);
        const monitor = this._window?.get_monitor() ?? Main.layoutManager.primaryIndex;
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor);
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        this._pill.ensure_style();
        const [, width] = this._pill.get_preferred_width(-1);
        const [, height] = this._pill.get_preferred_height(width);
        this._pill.translation_x = Math.round(area.x + (area.width - width) / 2);
        this._pill.translation_y = Math.round(area.y + area.height - BOTTOM * scale - height);
        this._pill.ease({opacity: 255, duration: SHOW_MS, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        // The dot breathes while it listens: its opacity only, and not at all with animations off
        if (St.Settings.get().enable_animations) {
            this._pulse.ease({
                opacity: 90, duration: PULSE_MS, mode: Clutter.AnimationMode.EASE_IN_OUT_SINE,
                repeatCount: -1, autoReverse: true,
            });
        }
    }

    // For the VM suite: what runs, and what is shown
    state() {
        return {
            bound: this._bound,
            helper: this._proc?.get_identifier() ?? null,
            window: this._window?.get_id() ?? null,
            pill: !!this._pill,
            words: this._words?.text ?? null,
            typed: this._typed ?? 0,
        };
    }
}
