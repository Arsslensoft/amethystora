// Hold a key for accents: a letter held down, in an app's text field, shows its accented forms in a strip over the
// text cursor (hold e: é è ê ë ē ė ę); a number, or the arrows and Enter, types the one chosen in place of the letter,
// and Escape or any other key leaves the letter as it was and goes on typing. Offered and off (the key accents).
//
// It sits where the text shortcuts do, on GNOME's own input method's notify_key_event() (an own property on its one
// object, never the prototype), and calls GNOME's method for every key. The only keys it keeps from the app are the
// letter's auto-repeats once the strip is open, and the keys that pick in it: those are handed on as an input method
// engine hands on a key it took (filtered), so that the app never sees them.
//
// The strip opens at GNOME's repeat delay: at the letter's first auto-repeat, or when that time has passed with the key
// still down (an app that repeats keys itself). Picking takes away what was typed since the letter was pressed (the
// letter, and the repeats an app made itself), as the app's own text says, and types the form through the input method.
// So it works only where the app tells the input method its text; terminals, fullscreen windows (games), password and
// number fields, an engine's unfinished text and the shell's own fields are left alone. Nothing typed is kept but the
// letter held and where the cursor was, until the strip closes.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Graphene from 'gi://Graphene';
import Mtk from 'gi://Mtk';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// The accented forms of each letter, the commonest first (board 53's for e). Upper case follows the letter held
export const FORMS = {
    a: 'àáâäæãåā', c: 'çćč', e: 'éèêëēėę', g: 'ğ', i: 'îïíīįì', l: 'ł', n: 'ñń', o: 'ôöòóœøōõ', s: 'ßśšş',
    u: 'ûüùúū', y: 'ÿý', z: 'žźż',
};
const UPPER = {'ß': 'ẞ'};
// How far above the text cursor the strip stands, and from the work area's edges (logical pixels)
const ABOVE = 8;
const MARGIN = 8;
// The most characters a pick takes away: the letter and the repeats an app made itself in the time it was held
const MOST = 64;

const Purpose = Clutter.InputContentPurpose;
const Hint = Clutter.InputContentHintFlags;
const WORDS = new Set([Purpose.NORMAL, Purpose.ALPHA, Purpose.URL, Purpose.EMAIL, Purpose.NAME]);
const SECRET = Hint.HIDDEN_TEXT | Hint.SENSITIVE_DATA;
const CHORD = Clutter.ModifierType.CONTROL_MASK | Clutter.ModifierType.MOD1_MASK | Clutter.ModifierType.MOD4_MASK |
    Clutter.ModifierType.SUPER_MASK | Clutter.ModifierType.HYPER_MASK | Clutter.ModifierType.META_MASK;
const DIGITS = [Clutter.KEY_1, Clutter.KEY_2, Clutter.KEY_3, Clutter.KEY_4, Clutter.KEY_5, Clutter.KEY_6, Clutter.KEY_7,
    Clutter.KEY_8];
const KP_DIGITS = [Clutter.KEY_KP_1, Clutter.KEY_KP_2, Clutter.KEY_KP_3, Clutter.KEY_KP_4, Clutter.KEY_KP_5,
    Clutter.KEY_KP_6, Clutter.KEY_KP_7, Clutter.KEY_KP_8];

// The forms of a character typed, in its case, or null when it has none
export function formsOf(ch) {
    if (typeof ch !== 'string' || [...ch].length !== 1)
        return null;
    const lower = ch.toLowerCase();
    const forms = FORMS[lower];
    if (!forms)
        return null;
    const list = [...forms];
    if (ch === lower)
        return list;
    if (ch !== ch.toUpperCase())
        return null;
    return list.map(form => UPPER[form] ?? form.toUpperCase());
}

export class Accents {
    // counts: the module's counters, which this adds to (never any text)
    constructor(counts) {
        this._counts = counts;
        this._wrapper = null;
        this._hold = null;
        this._strip = null;
        this._swallowed = new Set();
    }

    get wrapped() {
        return !!this._wrapper && Main.inputMethod.notify_key_event === this._wrapper;
    }

    get open() {
        return !!this._strip;
    }

    start() {
        const im = Main.inputMethod;
        this._im = im;
        this._keyboard = new Gio.Settings({schema_id: 'org.gnome.desktop.peripherals.keyboard'});
        const below = im.notify_key_event;
        this._below = Object.prototype.hasOwnProperty.call(im, 'notify_key_event') ? below : null;
        const self = this;
        const wrapper = function (event, filtered) {
            let take = false;
            if (self._wrapper === wrapper) {
                try {
                    take = self._key(event, filtered);
                } catch (e) {
                    take = false;
                    self._close();
                    self._counts.errors++;
                    if (self._counts.errors === 1)
                        console.warn(`Amethystora UI: accents: a key could not be looked at (${e.name})`);
                }
            }
            return below.call(this, event, take ? true : filtered);
        };
        this._wrapper = wrapper;
        im.notify_key_event = wrapper;
        global.display.connectObject('notify::focus-window', () => {
            this._close();
            this._swallowed.clear();
        }, this);
        global.stage.connectObject('notify::key-focus', () => this._close(), this);
        im.connectObject('notify::content-purpose', () => this._close(), 'notify::content-hints', () => this._close(),
            this);
    }

    stop() {
        global.display.disconnectObject(this);
        global.stage.disconnectObject(this);
        this._im?.disconnectObject(this);
        this._close();
        this._swallowed.clear();
        const im = this._im;
        const wrapper = this._wrapper;
        this._wrapper = null;
        if (im && wrapper && im.notify_key_event === wrapper) {
            if (this._below)
                im.notify_key_event = this._below;
            else
                delete im.notify_key_event;
        }
        this._im = null;
        this._below = null;
        this._keyboard?.run_dispose();
        this._keyboard = null;
    }

    // Whether a key may start a hold: a letter alone, into a field of the kinds that take words, in an app that tells
    // its text, in a window that is not a terminal or fullscreen
    _eligible(event, filtered) {
        const im = this._im;
        if (filtered || !im.currentFocus || Main.modalCount > 0)
            return false;
        const focus = global.stage.key_focus;
        if (focus !== null && focus !== global.stage)
            return false;
        if ((event.get_state() & CHORD) !== 0)
            return false;
        if (!WORDS.has(im.content_purpose) || (im.content_hints & SECRET) !== 0 || im.hasPreedit())
            return false;
        const window = global.display.focus_window;
        if (!window || window.is_fullscreen())
            return false;
        const app = Shell.WindowTracker.get_default().get_window_app(window);
        if ((app?.get_app_info()?.get_categories() ?? '').split(';').includes('TerminalEmulator'))
            return false;
        return im.getSurroundingText()[0] !== null;
    }

    // Returns true for a key the app must not see
    _key(event, filtered) {
        const type = event.type();
        const code = event.get_key_code();
        if (type === Clutter.EventType.KEY_RELEASE) {
            // The release of a key the strip took goes with its press
            if (this._swallowed.delete(code))
                return true;
            if (this._hold?.code === code) {
                this._hold.down = false;
                this._stopTimer();
                // Let go before the strip opened: an ordinary key
                if (!this._strip)
                    this._hold = null;
            }
            return false;
        }
        if (type !== Clutter.EventType.KEY_PRESS)
            return false;
        const repeated = (event.get_flags() & Clutter.EventFlags.FLAG_REPEATED) !== 0;
        if (this._strip)
            return this._inStrip(event, code, repeated);
        if (repeated) {
            if (this._hold?.code === code && this._hold.down) {
                this._show();
                return !!this._strip;
            }
            return false;
        }
        // A release that never came (the window went away meanwhile) is not waited for any more
        this._swallowed.delete(code);
        this._stopTimer();
        this._hold = null;
        if (!this._eligible(event, filtered))
            return false;
        const forms = formsOf(event.get_key_unicode());
        if (!forms)
            return false;
        const [text, cursor, anchor] = this._im.getSurroundingText();
        if (text === null || cursor !== anchor)
            return false;
        this._hold = {code, forms, ch: event.get_key_unicode(), cursor, down: true, window: global.display.focus_window};
        // An app that repeats keys itself sends no repeat through here: the strip opens when the repeat would start
        const delay = Math.max(150, Math.min(this._keyboard.get_uint('delay'), 2000));
        this._timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, delay, () => {
            this._timer = 0;
            if (this._hold?.down)
                this._show();
            return GLib.SOURCE_REMOVE;
        });
        return false;
    }

    _inStrip(event, code, repeated) {
        const hold = this._hold;
        // The letter still held keeps repeating: none of it reaches the app
        if (repeated && hold?.code === code)
            return true;
        const keyval = event.get_key_symbol();
        let index = DIGITS.indexOf(keyval);
        if (index < 0)
            index = KP_DIGITS.indexOf(keyval);
        if (index >= 0 && index < hold.forms.length && (event.get_state() & CHORD) === 0) {
            this._swallowed.add(code);
            this._pick(index);
            return true;
        }
        switch (keyval) {
        case Clutter.KEY_Left:
        case Clutter.KEY_Right:
        case Clutter.KEY_Up:
        case Clutter.KEY_Down: {
            const step = keyval === Clutter.KEY_Left || keyval === Clutter.KEY_Up ? -1 : 1;
            const rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
            const n = hold.forms.length;
            this._select((this._selected + (rtl && (keyval === Clutter.KEY_Left || keyval === Clutter.KEY_Right) ? -step : step) + n) % n);
            this._swallowed.add(code);
            return true;
        }
        case Clutter.KEY_Return:
        case Clutter.KEY_KP_Enter:
        case Clutter.KEY_ISO_Enter:
            this._swallowed.add(code);
            this._pick(this._selected);
            return true;
        case Clutter.KEY_Escape:
            this._swallowed.add(code);
            this._close();
            return true;
        default:
            // Any other key: the letter stays, and the key is typed as it is
            this._close();
            return false;
        }
    }

    _caret() {
        const rect = this._im.currentFocus ? this._im._cursorRect : null;
        if (!rect || !Number.isFinite(rect.x) || !Number.isFinite(rect.y))
            return null;
        const frame = this._hold?.window?.get_frame_rect();
        if (!frame || rect.x < frame.x || rect.y < frame.y || rect.x > frame.x + frame.width ||
            rect.y > frame.y + frame.height)
            return null;
        return rect;
    }

    _show() {
        this._stopTimer();
        const hold = this._hold;
        if (this._strip || !hold || global.display.focus_window !== hold.window)
            return;
        const caret = this._caret();
        if (!caret) {
            this._counts.accentsNoCaret++;
            this._hold = null;
            return;
        }
        const strip = new St.BoxLayout({
            style_class: 'amethystora-emoji-picker amethystora-accents',
            reactive: false,
        });
        this._cells = hold.forms.map((form, i) => {
            const cell = new St.BoxLayout({
                style_class: 'amethystora-emoji-cell amethystora-accents-cell',
                orientation: Clutter.Orientation.VERTICAL,
            });
            const glyph = new St.Label({text: form, style_class: 'amethystora-accents-glyph', x_align: Clutter.ActorAlign.CENTER});
            glyph.clutter_text.use_markup = false;
            const number = new St.Label({text: `${i + 1}`, style_class: 'amethystora-accents-number', x_align: Clutter.ActorAlign.CENTER});
            cell.add_child(glyph);
            cell.add_child(number);
            strip.add_child(cell);
            return cell;
        });
        this._strip = strip;
        Main.layoutManager.addChrome(strip);
        this._place(caret);
        this._select(0);
        this._counts.accentsOpened++;
    }

    // Above the text cursor, below it when there is no room, inside the work area
    _place(caret) {
        const strip = this._strip;
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const [, width] = strip.get_preferred_width(-1);
        const [, height] = strip.get_preferred_height(width);
        const monitor = global.display.get_monitor_index_for_rect(new Mtk.Rectangle({
            x: Math.round(caret.x), y: Math.round(caret.y), width: 1, height: 1,
        }));
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor < 0 ? Main.layoutManager.primaryIndex : monitor);
        let x = caret.x - width / 2;
        let y = caret.y - ABOVE * scale - height;
        if (y < area.y + MARGIN * scale)
            y = caret.y + caret.height + ABOVE * scale;
        x = Math.max(area.x + MARGIN * scale, Math.min(x, area.x + area.width - MARGIN * scale - width));
        y = Math.max(area.y + MARGIN * scale, Math.min(y, area.y + area.height - MARGIN * scale - height));
        strip.set_position(Math.round(x), Math.round(y));
        strip.pivot_point = new Graphene.Point({x: 0.5, y: 1});
    }

    _select(index) {
        this._selected = index;
        this._cells?.forEach((cell, i) => {
            if (i === index)
                cell.add_style_pseudo_class('selected');
            else
                cell.remove_style_pseudo_class('selected');
        });
    }

    // The form typed in place of what the hold typed: the letter held and its repeats, which the app's text shows just
    // before the cursor, and no more than was typed since the press. (What the app said of its text at the press may
    // lag the keys typed just before it, a space say: those are not the letter, and the run stops at them)
    _pick(index) {
        const hold = this._hold;
        const im = this._im;
        const form = hold?.forms[index];
        this._close();
        if (!form || !im || global.display.focus_window !== hold.window || !im.currentFocus) {
            this._counts.accentsGaveUp++;
            return;
        }
        const [text, cursor, anchor] = im.getSurroundingText();
        const before = text === null ? [] : [...text].slice(0, cursor);
        let typed = 0;
        while (typed < before.length && typed < cursor - hold.cursor && before[before.length - 1 - typed] === hold.ch)
            typed++;
        if (text === null || cursor !== anchor || typed < 1 || typed > MOST) {
            this._counts.accentsGaveUp++;
            return;
        }
        im.delete_surrounding(-typed, typed);
        im.commit(form);
        this._counts.accentsPicked++;
    }

    _stopTimer() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
    }

    _close() {
        this._stopTimer();
        this._hold = null;
        this._cells = null;
        if (this._strip) {
            this._strip.destroy();
            this._strip = null;
        }
    }
}
