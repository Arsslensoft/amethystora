// Force quit: Super+Ctrl+Shift+Escape lists the running apps with what each uses now, one that does not answer first,
// and ends the one picked after asking once more (quit/list.js). GNOME's own "not responding" dialog is shown in the
// image's look, with Wait first, and Ask the agent why while the agentic features are on (quit/frozen.js).
//
// Keys of org.gnome.shell.extensions.amethystora-ui.quit, read live: force-quit (the list's key; an empty list turns it
// off) and frozen-dialog (GNOME's dialog as it was when false). Closed, the list holds nothing and reads nothing: the
// processes are read only while it is open.

import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {FrozenDialog} from './quit/frozen.js';
import {ForceQuitList, keyLabel} from './quit/list.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.quit';
const KEY = 'force-quit';

export class Quit {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject(
            `changed::${KEY}`, () => this._bind(),
            'changed::frozen-dialog', () => this._syncFrozen(),
            this);
        this._bind();
        this._syncFrozen();
    }

    disable() {
        this._settings?.disconnectObject(this);
        if (this._bound)
            Main.wm.removeKeybinding(KEY);
        this._bound = false;
        this._list?.destroy();
        this._list = null;
        this._frozen?.disable();
        this._frozen = null;
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    // Whether the agentic features are on, as the agent module knows (off when it does not run)
    _agentOn() {
        return this._ext.modules?.get('agent')?.on ?? false;
    }

    // The agent module says when that changes; the dialog asks each time it is made, so nothing is kept here
    agentChanged() {}

    // The key, while it names one
    _bind() {
        const wanted = this._settings.get_strv(KEY).some(accelerator => accelerator !== '');
        if (wanted === !!this._bound)
            return;
        if (!wanted) {
            Main.wm.removeKeybinding(KEY);
            this._bound = false;
            this._list?.close();
            return;
        }
        const action = Main.wm.addKeybinding(KEY, this._settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW, () => this.toggle());
        if (action === Meta.KeyBindingAction.NONE)
            console.warn('Amethystora UI: the force quit key could not be set');
        this._bound = true;
    }

    _syncFrozen() {
        const wanted = this._settings.get_boolean('frozen-dialog');
        if (wanted && !this._frozen) {
            this._frozen = new FrozenDialog(() => this._agentOn());
            this._frozen.enable();
        } else if (!wanted && this._frozen) {
            this._frozen.disable();
            this._frozen = null;
        }
    }

    // Opens the list, or closes it
    toggle() {
        if (this._list) {
            this._list.close();
            return;
        }
        const accelerator = this._settings.get_strv(KEY).find(a => a !== '') ?? '';
        this._list = new ForceQuitList(keyLabel(accelerator), () => {
            this._list = null;
        });
        this._list.open();
    }

    get list() {
        return this._list ?? null;
    }

    // For the suites
    state() {
        return {bound: !!this._bound, frozen: !!this._frozen, list: this._list?.state ?? null};
    }
}
