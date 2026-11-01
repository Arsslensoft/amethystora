// Keys: two things the keyboard does that GNOME has no setting for.
//
// Paste as plain text (plain-paste, Super+Alt+V): what is copied is typed into the window in use as text alone, without
// the fonts, colours and links a web page or a document copied with it, whatever the app's own paste would do; Ctrl+V
// is left as it is. The clipboard's text is asked for when the key is pressed (St.Clipboard hands over text only) and
// typed in once Super and Alt are let go, into the window that had the keyboard then and only while it still has it
// (emoji/paste.js, as the emoji picker types). A password a password manager marks secret is never put back on the
// clipboard, where the clipboard's history would keep it. Nothing is held between two pastes.
//
// What Super alone opens (super-alone): GNOME's overview, the search box, or nothing. GNOME opens the overview when
// Super is let go with no other key (its overlay-key handler calls Main.overview.toggle()); with search or nothing, the
// overview's toggle answers that one caller differently, told by the event being handled, the release of Super, and
// every other caller (Super+S, the hot corner, a gesture) gets GNOME's toggle. With overview, nothing is wrapped.
//
// Idle, nothing runs: a keybinding and the settings' signals.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {cancelPastes, paste} from './emoji/paste.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.keys';
const KEY = 'plain-paste';
// What a password manager offers beside a password it marks secret, as the clipboard's history reads it
const PASSWORD_HINT = 'x-kde-passwordManagerHint';
// Super and Alt are waited for this long at most, looked at this often meanwhile
const RELEASE = 1500;
const LOOK = 20;
const HELD = Clutter.ModifierType.SUPER_MASK | Clutter.ModifierType.MOD4_MASK | Clutter.ModifierType.MOD1_MASK |
    Clutter.ModifierType.CONTROL_MASK | Clutter.ModifierType.SHIFT_MASK | Clutter.ModifierType.HYPER_MASK |
    Clutter.ModifierType.META_MASK;
const SUPERS = [Clutter.KEY_Super_L, Clutter.KEY_Super_R];
const MODES = ['overview', 'search', 'nothing'];

export class Keys {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        // For the tests: how many pastes, and how each ended (never the text)
        this.pastes = {asked: 0, typed: 0, empty: 0, secret: 0, moved: 0};
        this.superAlone = {search: 0, nothing: 0};
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject(`changed::${KEY}`, () => this._bind(), 'changed::super-alone',
            () => this._syncSuper(), this);
        this._bind();
        this._syncSuper();
    }

    disable() {
        this._settings?.disconnectObject(this);
        if (this._bound)
            Main.wm.removeKeybinding(KEY);
        this._bound = false;
        this._stopWaiting();
        cancelPastes();
        this._unwrapToggle();
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    // --- Paste as plain text ---------------------------------------------------------------------------------------

    // The key, while it names one: an empty list turns it off, and nothing is bound then
    _bind() {
        const wanted = this._settings.get_strv(KEY).some(accelerator => accelerator !== '');
        if (wanted === !!this._bound)
            return;
        if (!wanted) {
            Main.wm.removeKeybinding(KEY);
            this._bound = false;
            return;
        }
        const action = Main.wm.addKeybinding(KEY, this._settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL, () => this.pastePlain());
        if (action === Meta.KeyBindingAction.NONE)
            console.warn('Amethystora UI: the key to paste as plain text could not be set');
        this._bound = true;
    }

    pastePlain() {
        // Only into an app's window: a field of the shell's own, a menu or the overview has the keyboard otherwise
        const window = global.display.focus_window;
        if (!window || Main.modalCount > 0 || Main.overview.visible || global.stage.key_focus)
            return;
        this.pastes.asked++;
        const clipboard = St.Clipboard.get_default();
        const keep = clipboard.get_mimetypes(St.ClipboardType.CLIPBOARD).includes(PASSWORD_HINT);
        const settings = this._settings;
        clipboard.get_text(St.ClipboardType.CLIPBOARD, (_clipboard, text) => {
            // The module stopped meanwhile, or there is no text (a picture only)
            if (this._settings !== settings)
                return;
            if (!text) {
                this.pastes.empty++;
                return;
            }
            if (keep)
                this.pastes.secret++;
            this._whenReleased(() => {
                if (global.display.focus_window !== window) {
                    this.pastes.moved++;
                    return;
                }
                paste({text, window, keep});
                this.pastes.typed++;
                Main.osdWindowManager.showAll(Gio.ThemedIcon.new('edit-paste-symbolic'), _('Pasted as plain text'));
            });
        });
    }

    // done() once no modifier is held any more (keys pressed for the paste would otherwise come with Super and Alt),
    // or after RELEASE at most; a new paste replaces the one waiting
    _whenReleased(done) {
        this._stopWaiting();
        const started = GLib.get_monotonic_time();
        const look = () => {
            this._wait = 0;
            const [, , mods] = global.get_pointer();
            if ((mods & HELD) === 0 || GLib.get_monotonic_time() - started > RELEASE * 1000) {
                done();
                return;
            }
            this._wait = GLib.timeout_add_once(GLib.PRIORITY_DEFAULT, LOOK, look);
        };
        look();
    }

    _stopWaiting() {
        if (this._wait)
            GLib.source_remove(this._wait);
        this._wait = 0;
    }

    // --- What Super alone opens ----------------------------------------------------------------------------------------

    _mode() {
        const mode = this._settings.get_string('super-alone');
        return MODES.includes(mode) ? mode : 'overview';
    }

    _syncSuper() {
        if (this._mode() === 'overview')
            this._unwrapToggle();
        else
            this._wrapToggle();
    }

    // Whether the overview is being toggled by Super let go on its own: the event GNOME is handling is its release
    _bySuper() {
        const event = Clutter.get_current_event();
        return !!event && event.type() === Clutter.EventType.KEY_RELEASE && SUPERS.includes(event.get_key_symbol());
    }

    _wrapToggle() {
        if (this._toggle)
            return;
        const overview = Main.overview;
        this._ownToggle = Object.getOwnPropertyDescriptor(overview, 'toggle') ?? null;
        const below = overview.toggle;
        const module = this;
        const wrapper = function (...args) {
            if (module._toggle === wrapper && module._bySuper()) {
                module._superReleased();
                return undefined;
            }
            return below.apply(this, args);
        };
        this._toggle = wrapper;
        overview.toggle = wrapper;
    }

    _unwrapToggle() {
        const wrapper = this._toggle;
        this._toggle = null;
        if (!wrapper)
            return;
        // Another own toggle put on top of ours (the Hold Super card's, for a moment) keeps calling ours, which now only
        // passes the call on, and puts ours back when it ends; then it is taken away at the next change
        if (Main.overview.toggle === wrapper) {
            if (this._ownToggle)
                Object.defineProperty(Main.overview, 'toggle', this._ownToggle);
            else
                delete Main.overview.toggle;
        }
        this._ownToggle = null;
    }

    _superReleased() {
        // The overview open already (a gesture, Super+S): Super closes it, as GNOME's does
        if (Main.overview.visible) {
            Main.overview.hide();
            return;
        }
        if (this._mode() === 'nothing') {
            this.superAlone.nothing++;
            return;
        }
        this.superAlone.search++;
        const search = this._ext.modules?.get('search');
        if (search?._toggle)
            search._toggle();
        else
            Main.overview.show();
    }

    // For the tests: counts and state only
    snapshot() {
        return JSON.stringify({
            bound: !!this._bound, mode: this._settings ? this._mode() : null, wrapped: !!this._toggle,
            toggleIsOurs: !!this._toggle && Main.overview.toggle === this._toggle,
            pastes: this.pastes, superAlone: this.superAlone, waiting: !!this._wait,
        });
    }
}
