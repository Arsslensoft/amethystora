// Text shortcuts, at the one place in the shell that sees what is typed into apps: GNOME's own input method hands every
// key back to the app with notify_key_event() once IBus has answered for it. The observer wraps that method on GNOME's
// one input method object (an own property; the prototype is not touched), always calls GNOME's first, unchanged, so
// that the key reaches the app exactly as it would have, and only then looks at the key.
//
// It keeps the last characters typed into the focused window, at most one more than the longest shortcut, in memory
// only: never logged, written, shown or handed on. They are forgotten when another window or a field of the shell takes
// the keyboard, when the field's kind changes (to a password, say), on every key that is not text, every chord, every
// key the input method engine took, and at each expansion. A click or a middle-click paste inside the same window is
// not seen: Mutter gives the input method one focus for every app and field, and tells it nothing of a click. That is
// why nothing is replaced on what was typed alone: where the app tells the input method its text, only once that text
// is seen to end with the shortcut and the space where a word starts; where it does not, only in a terminal (whose line
// a click does not move), and nowhere else. Password, PIN, number, phone and date fields, the shell's own fields, a
// modal dialog or the overview, and an engine's unfinished text (Pinyin's, Mozc's) are never looked at.
//
// On a space after a shortcut, the shortcut and the space are taken away and its text typed in their place, both
// through the input method as an engine would, and only into the window that had the keyboard when the space was typed;
// in a terminal with BackSpace keys of the input method's own. Any key pressed meanwhile gives the expansion up.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {endsWithTyped, fillText, find, isPrintable, localeTag} from './match.js';

// How long the app has to tell its text after the space, before the expansion is given up
const WAIT_TEXT = 300;
// How long an app that does not say its text has to take the BackSpace keys before the text is typed
const KEYS_FIRST = 60;
// XKB's keycode for BackSpace (evdev's 14 and 8), as the input method forwards keys
const BACKSPACE_KEYCODE = 22;

const Purpose = Clutter.InputContentPurpose;
const Hint = Clutter.InputContentHintFlags;
// The fields that take words; any other purpose (digits, numbers, phone numbers, dates, times, passwords, PINs) is
// never looked at
const WORDS = new Set([Purpose.NORMAL, Purpose.ALPHA, Purpose.URL, Purpose.EMAIL, Purpose.NAME, Purpose.TERMINAL]);
const SECRET = Hint.HIDDEN_TEXT | Hint.SENSITIVE_DATA;
// A chord with one of these is a command, not text
const CHORD = Clutter.ModifierType.CONTROL_MASK | Clutter.ModifierType.MOD1_MASK | Clutter.ModifierType.MOD4_MASK |
    Clutter.ModifierType.SUPER_MASK | Clutter.ModifierType.HYPER_MASK | Clutter.ModifierType.META_MASK;
const MODIFIERS = new Set([
    Clutter.KEY_Shift_L, Clutter.KEY_Shift_R, Clutter.KEY_Control_L, Clutter.KEY_Control_R, Clutter.KEY_Alt_L,
    Clutter.KEY_Alt_R, Clutter.KEY_Super_L, Clutter.KEY_Super_R, Clutter.KEY_Meta_L, Clutter.KEY_Meta_R,
    Clutter.KEY_Hyper_L, Clutter.KEY_Hyper_R, Clutter.KEY_ISO_Level3_Shift, Clutter.KEY_ISO_Level5_Shift,
    Clutter.KEY_Caps_Lock, Clutter.KEY_Num_Lock, Clutter.KEY_Shift_Lock,
]);

// Whether GNOME's input method is still what the observer was written for: the method it calls on itself for every key,
// and the signal it gives when an app has told it its text
export function inputMethodFits() {
    const im = Main.inputMethod;
    if (typeof im?.notify_key_event !== 'function' || typeof im.getSurroundingText !== 'function' ||
        typeof im.hasPreedit !== 'function')
        return false;
    try {
        return GObject.signal_lookup('surrounding-text-set', im.constructor.$gtype) !== 0;
    } catch {
        return false;
    }
}

export class Expander {
    // counts: the module's counters, which the observer adds to (never any text)
    constructor(counts) {
        this._counts = counts;
        this._buffer = [];
        this._map = new Map();
        this._lengths = [];
        this._maxLength = 0;
        this._expanding = null;
        this._wrapper = null;
    }

    get wrapped() {
        return !!this._wrapper && Main.inputMethod.notify_key_event === this._wrapper;
    }

    get bufferLength() {
        return this._buffer.length;
    }

    get count() {
        return this._map.size;
    }

    setShortcuts({map, lengths, maxLength}) {
        this._map = map;
        this._lengths = lengths;
        this._maxLength = maxLength;
        this.clear();
    }

    start() {
        const im = Main.inputMethod;
        this._im = im;
        // The session's language, for {date}: it does not change while the session lasts
        this._locale = localeTag(GLib.get_language_names()[0]);
        // What the object answers now: the prototype's method, or another wrapper's own property under ours
        const below = im.notify_key_event;
        this._below = Object.prototype.hasOwnProperty.call(im, 'notify_key_event') ? below : null;
        const self = this;
        const wrapper = function (event, filtered) {
            const result = below.call(this, event, filtered);
            if (self._wrapper === wrapper)
                self._watch(event, filtered);
            return result;
        };
        this._wrapper = wrapper;
        im.notify_key_event = wrapper;
        global.display.connectObject('notify::focus-window', () => this.forget(), this);
        // What a key is checked against, kept as it changes rather than asked of GNOME at every key: asking cost most of
        // the time a key took
        this._readField();
        this._readStage();
        im.connectObject('notify::content-purpose', () => this._readField(),
            'notify::content-hints', () => this._readField(), this);
        global.stage.connectObject('notify::key-focus', () => this._readStage(), this);
    }

    // Another kind of field (a password field after a name field, say) clears what was kept, and gives up an expansion
    _readField() {
        this._purpose = this._im.content_purpose;
        this._hints = this._im.content_hints;
        this.forget();
    }

    // A field of the shell's own has the stage's key focus; a client window's field leaves it empty
    _readStage() {
        const focus = global.stage.key_focus;
        this._shellFocus = focus !== null && focus !== global.stage;
        if (this._shellFocus)
            this.forget();
    }

    stop() {
        global.display.disconnectObject(this);
        global.stage.disconnectObject(this);
        this._im?.disconnectObject(this);
        this._cancelExpansion();
        const im = this._im;
        const wrapper = this._wrapper;
        // Ours lets everything through from now on, whatever happens to it below
        this._wrapper = null;
        if (im && wrapper && im.notify_key_event === wrapper) {
            if (this._below)
                im.notify_key_event = this._below;
            else
                delete im.notify_key_event;
        }
        // Another wrapper put on top of ours keeps calling ours, which now only passes the key on
        this._im = null;
        this._below = null;
        this.forget();
        this._map = new Map();
        this._lengths = [];
        this._maxLength = 0;
    }

    clear() {
        this._buffer.length = 0;
    }

    // Another window or kind of field: nothing typed before counts, and an expansion on its way is given up
    forget() {
        this.clear();
        this._cancelExpansion();
    }

    _watch(event, filtered) {
        const counts = this._counts;
        counts.observed++;
        const started = GLib.get_monotonic_time();
        let expanded = false;
        try {
            expanded = this._observe(event, filtered);
        } catch (e) {
            // Never into GNOME's input method, and never the text: only that it happened
            this.clear();
            counts.errors++;
            if (counts.errors === 1)
                console.warn(`Amethystora UI: text shortcuts: a key could not be looked at (${e.name})`);
        }
        const took = GLib.get_monotonic_time() - started;
        // A key, and apart from those the space that starts an expansion, which does the replacing too
        if (expanded) {
            counts.expandMaxUs = Math.max(counts.expandMaxUs, took);
        } else {
            counts.timed++;
            counts.totalUs += took;
            counts.maxUs = Math.max(counts.maxUs, took);
        }
    }

    _skip(reason) {
        this._counts.skipped[reason]++;
        this.clear();
    }

    // Returns true when the key started an expansion
    _observe(event, filtered) {
        if (event.type() !== Clutter.EventType.KEY_PRESS)
            return false;
        // A key pressed while an expansion waits: the person typed on, and the expansion would land after it
        if (this._expanding) {
            this._cancelExpansion();
            this.clear();
        }
        const im = this._im;
        if (this._shellFocus || Main.modalCount > 0) {
            this._skip('shell');
            return false;
        }
        // No field of an app has the keyboard. (Which field has it is not known here: Mutter gives the input method one
        // focus for every app and field, so a change of window is what notify::focus-window tells)
        if (!im.currentFocus) {
            this.clear();
            return false;
        }
        const purpose = this._purpose;
        // Mutter has no purpose for a PIN: GTK marks PIN fields, as it does passwords, hidden and sensitive
        if (purpose === Purpose.PASSWORD || (this._hints & SECRET) !== 0) {
            this._skip('password');
            return false;
        }
        if (!WORDS.has(purpose)) {
            this._skip('purpose');
            return false;
        }
        if (im.hasPreedit()) {
            this._skip('preedit');
            return false;
        }
        if (filtered) {
            this._skip('filtered');
            return false;
        }
        const keyval = event.get_key_symbol();
        if (MODIFIERS.has(keyval))
            return false;
        if ((event.get_state() & CHORD) !== 0) {
            this.clear();
            return false;
        }
        const buffer = this._buffer;
        if (keyval === Clutter.KEY_BackSpace) {
            buffer.pop();
            return false;
        }
        if (keyval === Clutter.KEY_space) {
            const abbr = this._lengths.length ? find(buffer, this._lengths, this._map) : null;
            if (abbr) {
                this.clear();
                this._expand(abbr);
                return true;
            }
            buffer.push(' ');
            if (buffer.length > this._maxLength + 1)
                buffer.shift();
            return false;
        }
        const ch = event.get_key_unicode();
        if (!isPrintable(ch)) {
            this.clear();
            return false;
        }
        buffer.push(ch);
        if (buffer.length > this._maxLength + 1)
            buffer.shift();
        return false;
    }

    _isTerminal() {
        if (this._im.content_purpose === Purpose.TERMINAL)
            return true;
        const window = global.display.focus_window;
        const app = window ? Shell.WindowTracker.get_default().get_window_app(window) : null;
        return (app?.get_app_info()?.get_categories() ?? '').split(';').includes('TerminalEmulator');
    }

    _expand(abbr) {
        const im = this._im;
        // Only into the window that had the keyboard when the space was typed, and only while no field of the shell or
        // modal dialog has it. (The input method's focus is one object for every app, so it proves nothing)
        const window = global.display.focus_window;
        const still = () => window !== null && global.display.focus_window === window && !this._shellFocus &&
            Main.modalCount === 0 && im.currentFocus !== null;
        const text = fillText(this._map.get(abbr), this._locale);
        // A line break in a terminal would run what is on the line
        if (text.includes('\n') && this._isTerminal()) {
            this._counts.skipped.terminal++;
            return;
        }
        const n = [...abbr].length;
        const expansion = {};
        const done = () => {
            if (this._expanding === expansion)
                this._cancelExpansion();
        };
        if (im.getSurroundingText()[0] === null) {
            // The app does not say its text, so nothing confirms what is there. In a terminal a click does not move the
            // line being typed, and a text of one line presses no Enter; anywhere else a click or a paste may have put
            // other text where the shortcut seems to be, and nothing is replaced
            if (!this._isTerminal()) {
                this._counts.skipped.unseen++;
                return;
            }
            if (text.includes('\n')) {
                this._counts.skipped.terminal++;
                return;
            }
            // As many BackSpace keys as the shortcut and its space, from the input method itself, so that neither IBus
            // nor this observer sees them. The text follows once the app has had them: an app may take the text,
            // which comes another way, before keys sent ahead of it
            const time = global.display.get_current_time_roundtrip();
            for (let i = 0; i < n + 1; i++) {
                im.forward_key(Clutter.KEY_BackSpace, BACKSPACE_KEYCODE, 0, time, true);
                im.forward_key(Clutter.KEY_BackSpace, BACKSPACE_KEYCODE, 0, time, false);
            }
            this._expanding = expansion;
            expansion.timeout = GLib.timeout_add(GLib.PRIORITY_DEFAULT, KEYS_FIRST, () => {
                expansion.timeout = 0;
                done();
                if (still()) {
                    im.commit(`${text} `);
                    this._counts.expansions++;
                }
                return GLib.SOURCE_REMOVE;
            });
            return;
        }
        // The app says its text: wait for it to say the space was typed, and replace only what is seen to be there
        this._expanding = expansion;
        expansion.signal = im.connect('surrounding-text-set', () => {
            if (!still()) {
                done();
                return;
            }
            const [surrounding, cursor, anchor] = im.getSurroundingText();
            if (surrounding === null || cursor !== anchor) {
                done();
                return;
            }
            const seen = endsWithTyped([...surrounding].slice(0, cursor), abbr);
            if (seen === 'not-yet')
                return;
            done();
            if (seen !== 'yes')
                return;
            im.delete_surrounding(-(n + 1), n + 1);
            im.commit(`${text} `);
            this._counts.expansions++;
        });
        expansion.timeout = GLib.timeout_add(GLib.PRIORITY_DEFAULT, WAIT_TEXT, () => {
            expansion.timeout = 0;
            done();
            return GLib.SOURCE_REMOVE;
        });
    }

    _cancelExpansion() {
        const expansion = this._expanding;
        if (!expansion)
            return;
        this._expanding = null;
        if (expansion.signal)
            this._im?.disconnect(expansion.signal);
        if (expansion.timeout)
            GLib.source_remove(expansion.timeout);
    }
}
