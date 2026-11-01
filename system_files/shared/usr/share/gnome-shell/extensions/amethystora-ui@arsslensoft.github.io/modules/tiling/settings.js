// The GNOME settings the tiling cannot do without, set while it runs and put back as they were when it stops:
// dragging a window to an edge of the screen does not snap it to half the screen (GNOME's edge tiling would fight
// the strip), modal dialogs are not attached to their parent, and with more than one screen every screen has
// workspaces, so that each screen shows one of its own, given back when one screen is left. Dynamic or fixed
// workspaces stay the person's choice: screens.js keeps the ones the screens show. What these were is written down
// first (state.js), so that a crash cannot lose it.

import Gio from 'gi://Gio';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const MUTTER = 'org.gnome.mutter';

export class GnomeSettings {
    constructor(state) {
        this._state = state;
    }

    enable() {
        this._settings = new Gio.Settings({schema_id: MUTTER});
        this._settings.connectObject('changed', (s, key) => this._changed(key), this);
        this._force('edge-tiling', false);
        // A modal dialog is a window of its own: glued to a parent the strip moves, it would be left where the parent
        // was
        this._force('attach-modal-dialogs', false);
        // An earlier version made the workspaces fixed while there were several screens, and wrote down as what was
        // there before GNOME's dynamic ones, which beat the image's fixed six until the image set them for GNOME's
        // own session too: the key goes back to its default, once, rather than to that
        const old = this._records().find(r => r.key === 'dynamic-workspaces');
        if (old) {
            this._state.set('forced', this._records().filter(r => r !== old));
            if (this._settings.get_boolean('dynamic-workspaces') === old.wrote)
                this._settings.reset('dynamic-workspaces');
        }
        this.monitorsChanged();
    }

    disable(suspend = false) {
        this._settings?.disconnectObject(this);
        // At a screen lock nothing is written: the tiling is back at unlock
        if (!suspend)
            GnomeSettings.giveBackLeftovers(this._state);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    monitorsChanged() {
        if (!this._settings)
            return;
        if (Main.layoutManager.monitors.length > 1)
            this._force('workspaces-only-on-primary', false);
        else
            this._giveBack('workspaces-only-on-primary');
    }

    _giveBack(key) {
        const record = this._records().find(r => r.key === key);
        if (!record)
            return;
        this._state.set('forced', this._records().filter(r => r !== record));
        if (this._settings.get_boolean(key) === record.wrote)
            this._settings.set_boolean(key, record.was);
    }

    _records() {
        return this._state.get('forced') ?? [];
    }

    _force(key, value) {
        const current = this._settings.get_boolean(key);
        if (current === value || this._records().some(r => r.key === key))
            return;
        this._state.set('forced', [...this._records(), {schema: MUTTER, key, was: current, wrote: value}]);
        this._settings.set_boolean(key, value);
    }

    // The person set one of them again while the tiling runs: theirs from now on
    _changed(key) {
        const record = this._records().find(r => r.key === key);
        if (record && this._settings.get_boolean(key) !== record.wrote)
            this._state.set('forced', this._records().filter(r => r !== record));
    }

    static giveBackLeftovers(state) {
        for (const record of state.get('forced') ?? []) {
            let settings = null;
            try {
                settings = new Gio.Settings({schema_id: record.schema});
                if (settings.get_boolean(record.key) === record.wrote)
                    settings.set_boolean(record.key, record.was);
            } catch (e) {
                console.warn(`Amethystora UI: ${record.schema} ${record.key} was not put back: ${e.message}`);
            } finally {
                // Disposed now rather than by the collector (dock.js says why)
                settings?.run_dispose();
            }
        }
        state.set('forced', undefined);
    }
}
