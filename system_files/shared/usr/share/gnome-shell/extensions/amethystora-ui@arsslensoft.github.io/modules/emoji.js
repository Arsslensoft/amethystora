// Emoji and symbols: Super+. opens a picker over the window being typed in, under its text cursor where the input
// method says where that is, with the keyboard in its search field. Its tabs are Recent, Smileys, Arrows, Maths,
// Money, Punctuation and the rest of Unicode's emoji groups; a search finds by Unicode's own names and search words, in
// the session's language and English. Picking one (a click, Enter) closes the picker and puts the character where the
// person was typing (emoji/paste.js), and keeps it first in Recent. Hands and people come in the skin tone of the key
// skin-tone; a right click, a long press or the Menu key on one offers the six. Escape, the same key again or a click
// elsewhere closes it without typing anything.
//
// Nothing is held while it is closed but its key: the lists and names are read when it opens and let go when it
// closes (emoji/data.js), and its actors are made then too.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Graphene from 'gi://Graphene';
import Meta from 'gi://Meta';
import Mtk from 'gi://Mtk';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {EmojiData, TONES} from './emoji/data.js';
import {Picker} from './emoji/picker.js';
import {addRecent, readRecent} from './emoji/recent.js';
import {cancelPastes, giveBackTextInput, paste} from './emoji/paste.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.emoji';
const KEY = 'emoji-picker';
// The card's width, its distance from the text cursor and from the work area's edges (logical pixels)
const WIDTH = 400;
const BELOW = 8;
const MARGIN = 16;
const DURATION = 100;
const SMALL = 0.96;

export class Emoji {
    constructor(ext) {
        this._ext = ext;
    }

    get open() {
        return !!this._picker;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject(
            'changed::skin-tone', () => this._picker?.setTone(this._tone()),
            `changed::${KEY}`, () => this._bind(),
            this);
        this._bind();
    }

    // The key, while it names one: an empty list turns the picker off, and nothing is bound then
    _bind() {
        const wanted = this._settings.get_strv(KEY).some(accelerator => accelerator !== '');
        if (wanted === !!this._bound)
            return;
        if (!wanted) {
            Main.wm.removeKeybinding(KEY);
            this._bound = false;
            this.close();
            return;
        }
        const action = Main.wm.addKeybinding(KEY, this._settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL, () => this.toggle());
        if (action === Meta.KeyBindingAction.NONE)
            console.warn('Amethystora UI: the emoji picker\'s key could not be set');
        this._bound = true;
    }

    disable() {
        this._settings?.disconnectObject(this);
        if (this._bound)
            Main.wm.removeKeybinding(KEY);
        this._bound = false;
        this.close();
        cancelPastes();
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    _tone() {
        const tone = this._settings.get_string('skin-tone');
        return TONES.includes(tone) ? tone : 'none';
    }

    toggle() {
        if (this._picker)
            this.close();
        else
            this.show();
    }

    // Where the person is typing: the text cursor's rectangle where the input method knows it, as GNOME's own
    // input method keeps it
    _caret() {
        const rect = Main.inputMethod.currentFocus ? Main.inputMethod._cursorRect : null;
        if (!rect || !Number.isFinite(rect.x) || !Number.isFinite(rect.y))
            return null;
        // Only a cursor inside the window being typed in: the input method keeps the last one it was told of, which
        // may be a field of the shell's own
        const frame = this._window?.get_frame_rect();
        if (!frame || rect.x < frame.x || rect.y < frame.y || rect.x > frame.x + frame.width ||
            rect.y > frame.y + frame.height)
            return null;
        return rect;
    }

    show() {
        if (this._picker || Main.overview.visible || Main.modalCount > 0)
            return;
        this._window = global.display.focus_window;
        const caret = this._caret();
        const recent = readRecent();
        const picker = new Picker(this, {tab: recent.length ? 'recent' : 'smileys', recent, tone: this._tone()});
        this._picker = picker;
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        picker.actor.width = WIDTH * scale;
        Main.layoutManager.addChrome(picker.actor);
        this._place(caret);
        this.glassChanged();

        // The keyboard leaving the picker (a click in another window) closes it, and so does a press anywhere else that
        // leaves the keyboard where it is (the desktop, the bar, the window being typed in); a screen change too
        global.stage.connectObject(
            'notify::key-focus', () => this._queueFocusCheck(),
            'captured-event', (stage, event) => this._onCaptured(event),
            this);
        Main.layoutManager.connectObject('monitors-changed', () => this.close(), this);
        global.display.connectObject('in-fullscreen-changed', () => this.close(), this);

        const reduce = this._ext.modules?.get('comfort')?.reduceMotion ?? false;
        picker.actor.opacity = 0;
        picker.actor.set_scale(reduce ? 1 : SMALL, reduce ? 1 : SMALL);
        picker.actor.ease({
            opacity: 255, scale_x: 1, scale_y: 1, duration: DURATION, mode: Clutter.AnimationMode.EASE_OUT_QUAD,
        });
        picker.entry.grab_key_focus();

        this._loading = new Gio.Cancellable();
        const loading = this._loading;
        EmojiData.load(loading).then(data => {
            if (loading.is_cancelled() || this._picker !== picker)
                return;
            this._loading = null;
            this._data = data;
            if (data)
                picker.setData(data);
        });
    }

    // Under the text cursor (above it when there is no room), else a third of the way down the window being typed in,
    // else the middle of the main screen; always inside the work area
    _place(caret) {
        const actor = this._picker.actor;
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const width = actor.width;
        const [, height] = actor.get_preferred_height(width);
        let monitor = Main.layoutManager.primaryIndex;
        let x, y, pivotY = 0;
        // What it was placed by, for the tests
        this._placedBy = caret ? 'caret' : this._window ? 'window' : 'screen';
        if (caret) {
            monitor = global.display.get_monitor_index_for_rect(new Mtk.Rectangle({
                x: Math.round(caret.x), y: Math.round(caret.y), width: 1, height: 1,
            }));
            const area = Main.layoutManager.getWorkAreaForMonitor(monitor);
            x = caret.x + caret.width / 2 - width / 2;
            y = caret.y + caret.height + BELOW * scale;
            if (y + height > area.y + area.height - MARGIN * scale) {
                y = caret.y - BELOW * scale - height;
                pivotY = 1;
            }
        } else if (this._window) {
            const frame = this._window.get_frame_rect();
            monitor = this._window.get_monitor();
            x = frame.x + frame.width / 2 - width / 2;
            y = frame.y + frame.height / 3;
        } else {
            const area = Main.layoutManager.getWorkAreaForMonitor(monitor);
            x = area.x + (area.width - width) / 2;
            y = area.y + (area.height - height) / 2;
        }
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor < 0 ? Main.layoutManager.primaryIndex : monitor);
        const margin = MARGIN * scale;
        x = Math.max(area.x + margin, Math.min(x, area.x + area.width - margin - width));
        y = Math.max(area.y + margin, Math.min(y, area.y + area.height - margin - height));
        actor.set_position(Math.round(x), Math.round(y));
        actor.pivot_point = new Graphene.Point({x: 0.5, y: pivotY});
    }

    close() {
        if (!this._picker)
            return;
        this._loading?.cancel();
        this._loading = null;
        this._removeFocusCheck();
        global.stage.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        global.display.disconnectObject(this);
        const picker = this._picker;
        this._picker = null;
        // The keyboard goes back to the windows first
        const focus = global.stage.get_key_focus();
        if (focus && picker.contains(focus)) {
            global.stage.set_key_focus(null);
            giveBackTextInput();
        }
        picker.destroy();
        this._data = null;
        this.glassChanged();
    }

    // A character picked (with the tone chosen in the strip, when it came from there)
    pick(character, tone = null) {
        const window = this._window;
        this.close();
        if (tone && TONES.includes(tone) && tone !== this._tone())
            this._settings.set_string('skin-tone', tone);
        addRecent(character);
        paste({text: character, window});
    }

    // The tone strip, over the cell it is for
    showStrip(strip, cell) {
        Main.layoutManager.addChrome(strip);
        const [cellX, cellY] = cell.get_transformed_position();
        const [, width] = strip.get_preferred_width(-1);
        const [, height] = strip.get_preferred_height(width);
        const monitor = Main.layoutManager.findIndexForActor(cell);
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor);
        const x = Math.max(area.x, Math.min(cellX + cell.width / 2 - width / 2, area.x + area.width - width));
        const y = Math.max(area.y, cellY + cell.height / 2 - height / 2);
        strip.set_position(Math.round(x), Math.round(y));
        this.glassChanged();
    }

    hideStrip() {
        this.glassChanged();
    }

    _queueFocusCheck() {
        if (this._focusLater)
            return;
        this._focusLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._focusLater = 0;
            const focus = global.stage.get_key_focus();
            if (this._picker && !(focus && focus !== global.stage && this._picker.contains(focus)))
                this.close();
            return GLib.SOURCE_REMOVE;
        });
    }

    _onCaptured(event) {
        const type = event.type();
        if (!this._picker || (type !== Clutter.EventType.BUTTON_PRESS && type !== Clutter.EventType.TOUCH_BEGIN))
            return Clutter.EVENT_PROPAGATE;
        const target = global.stage.get_event_actor(event);
        if (!target || !this._picker.contains(target))
            this.close();
        return Clutter.EVENT_PROPAGATE;
    }

    _removeFocusCheck() {
        if (this._focusLater)
            global.compositor.get_laters().remove(this._focusLater);
        this._focusLater = 0;
    }

    // ---- For the glass ----

    glassSurfaces() {
        const surfaces = [];
        if (this._picker) {
            surfaces.push({actor: this._picker.actor, material: 'glass', radius: null});
            if (this._picker.strip)
                surfaces.push({actor: this._picker.strip, material: 'glass', radius: null});
        }
        return surfaces;
    }

    glassChanged() {
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    // ---- For the tests (the lab's suite 33) ----

    snapshot() {
        const picker = this._picker;
        const focus = global.stage.get_key_focus();
        let focused = '';
        if (picker && focus) {
            if (picker.entry.contains(focus))
                focused = 'field';
            else if (picker.strip?.contains(focus))
                focused = 'tones';
            else if (focus.character)
                focused = focus.character;
        }
        return JSON.stringify({
            open: !!picker,
            loaded: !!this._data,
            x: picker?.actor.x ?? 0,
            y: picker?.actor.y ?? 0,
            width: picker?.actor.width ?? 0,
            height: picker?.actor.height ?? 0,
            tab: picker?.tab ?? '',
            query: picker?.query ?? '',
            cells: picker ? picker.shown.slice(0, 24).map(character => this._data?.toned(character,
                picker.tab === 'recent' ? 'none' : this._tone()) ?? character) : [],
            focus: focused,
            timeouts: this._focusLater ? 1 : 0,
            placed: picker ? this._placedBy : '',
        });
    }
}
