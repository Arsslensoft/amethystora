// Easier to see and read: colour filters over the whole screen (modules/access/filter.js), a reading ruler that follows
// the pointer (ruler.js) and the selected text read aloud on this computer (speech.js). Each is off until the person
// chooses it (ame desktop access, Control), and one that is off has nothing made and nothing connected; its shortcut
// is not even bound. The keys of org.gnome.shell.extensions.amethystora-ui.access are read live.
//
//   Super+Ctrl+R   shows or hides the ruler, while a ruler is chosen (the board's Super+Alt+R is the tiling's)
//   Super+Ctrl+S   reads the selection aloud, or stops, while reading aloud is on (Super+Alt+S is GNOME's screen
//                  reader switch)
//   Super+Alt+K    letters on the window's buttons and links, to click them with the keyboard, while that is on
//                  (letters.js; plans/ui-ideas/spec-later.md, R5-36)
//
// For the suites: state() says what is made and connected.

import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {Filter} from './access/filter.js';
import {Ruler} from './access/ruler.js';
import {Speech} from './access/speech.js';
import {Letters} from './access/letters.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.access';
const RULER_KEY = 'ruler-key';
const READ_KEY = 'read-aloud-key';
const LETTERS_KEY = 'letters-key';
// The shortcuts work wherever the person reads: windows, the overview, a menu left open
const MODES = Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW | Shell.ActionMode.POPUP;

export class Access {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this.settings = this._ext.getSettings(SCHEMA);
        this.enabled = true;
        this._bound = new Set();
        this.filter = new Filter(this.settings);
        this.filter.enable();
        this.ruler = new Ruler();
        this.speech = new Speech(this);
        this.settings.connectObject(
            'changed::ruler', () => this._syncRuler(),
            'changed::ruler-shown', () => this._syncRuler(),
            'changed::read-aloud', () => this._syncSpeech(),
            'changed::read-aloud-speed', () => this.speech?.speedChanged(),
            'changed::letters', () => this._syncLetters(),
            this);
        this._syncRuler();
        this._syncSpeech();
        this._syncLetters();
    }

    disable() {
        this.enabled = false;
        this.settings?.disconnectObject(this);
        for (const name of this._bound ?? [])
            Main.wm.removeKeybinding(name);
        this._bound = null;
        this.letters?.hide();
        this.letters = null;
        this.speech?.destroy();
        this.speech = null;
        this.ruler?.hide();
        this.ruler = null;
        this.filter?.disable();
        this.filter = null;
        // Disposed now rather than by the collector (dock.js says why)
        this.settings?.run_dispose();
        this.settings = null;
    }

    // The live pill, for reading aloud
    get pill() {
        return this._ext.modules?.get('pill') ?? null;
    }

    _bind(name, wanted, handler) {
        if (wanted && !this._bound.has(name)) {
            Main.wm.addKeybinding(name, this.settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT, MODES, handler);
            this._bound.add(name);
        } else if (!wanted && this._bound.has(name)) {
            Main.wm.removeKeybinding(name);
            this._bound.delete(name);
        }
    }

    _syncRuler() {
        const style = this.settings.get_string('ruler');
        const chosen = style !== 'off';
        this._bind(RULER_KEY, chosen, () => this.settings.set_boolean('ruler-shown', !this.settings.get_boolean('ruler-shown')));
        this.ruler.show(chosen && this.settings.get_boolean('ruler-shown') ? style : null);
    }

    _syncSpeech() {
        const on = this.settings.get_boolean('read-aloud');
        this._bind(READ_KEY, on, () => this.speech.toggle());
        if (!on)
            this.speech.stop();
    }

    // Click with the keyboard: its shortcut while on; nothing is made until it is pressed
    _syncLetters() {
        const on = this.settings.get_boolean('letters');
        this._bind(LETTERS_KEY, on, () => {
            this.letters ??= new Letters();
            this.letters.toggle();
        });
        if (!on) {
            this.letters?.hide();
            this.letters = null;
        }
    }

    // Another module's colour matrix (9 numbers, row by row, in linear light) laid over the colour filter in the same
    // pass, or null to take it away: drawn whether a filter is on or not (a focus profile's grey)
    setExtraMatrix(id, matrix) {
        this.filter?.setExtra(id, matrix);
    }

    state() {
        return {
            filter: this.filter?.state() ?? null,
            ruler: this.ruler?.state() ?? null,
            speech: this.speech?.state() ?? null,
            letters: this.letters?.state() ?? null,
            keys: [...this._bound ?? []],
        };
    }
}
