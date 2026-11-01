// Touch and pen: touch mode, which sizes the shell for a fingertip while a 2-in-1 is folded back (modules/touch/mode.js),
// and the pen's tools, six of them around the pen's tip or at the screen's edge (ring.js). Both are off until the
// person chooses them (ame desktop touch, Control); the keys of org.gnome.shell.extensions.amethystora-ui.touch are read
// live, and a part that is off has nothing made and nothing connected.
//
//   Super+Ctrl+P   opens the pen's tools, while they are on; the pen's side button sends it once
//                  `ame desktop touch pen-button on` has set the pen up in GNOME's tablet settings
//
// For the suites: state(), setSeatForTest(true|false|null) in place of the hinge, ring for the open ring.

import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {TouchMode} from './touch/mode.js';
import {PenRing, TOOLS} from './touch/ring.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.touch';
const RING_KEY = 'pen-ring-key';

export class Touch {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this.settings = this._ext.getSettings(SCHEMA);
        this._bound = false;
        this.mode = new TouchMode(this);
        this.ring = new PenRing(this);
        this.settings.connectObject(
            'changed::touch-mode', () => this.mode.set(this.settings.get_string('touch-mode')),
            'changed::pen-ring', () => this._syncRing(),
            this);
        this.mode.set(this.settings.get_string('touch-mode'));
        this._syncRing();
    }

    disable() {
        this.settings?.disconnectObject(this);
        if (this._bound)
            Main.wm.removeKeybinding(RING_KEY);
        this._bound = false;
        this.ring?.destroy();
        this.ring = null;
        this.mode?.destroy();
        this.mode = null;
        // Disposed now rather than by the collector (dock.js says why)
        this.settings?.run_dispose();
        this.settings = null;
    }

    get modules() {
        return this._ext.modules ?? null;
    }

    get dock() {
        return this._ext.modules?.get('dock') ?? null;
    }

    get reduceMotion() {
        return this._ext.modules?.get('comfort')?.reduceMotion ?? false;
    }

    _syncRing() {
        const on = this.settings.get_string('pen-ring') !== 'off';
        if (on && !this._bound) {
            Main.wm.addKeybinding(RING_KEY, this.settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
                Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW, () => this.openRing());
            this._bound = true;
        } else if (!on && this._bound) {
            Main.wm.removeKeybinding(RING_KEY);
            this._bound = false;
            this.ring.destroy();
        } else if (on) {
            // Another variant: the next opening takes it
            this.ring.close();
        }
    }

    openRing() {
        const variant = this.settings?.get_string('pen-ring');
        if (!variant || variant === 'off')
            return;
        const tools = this.settings.get_strv('pen-ring-tools').filter((id, i, all) =>
            TOOLS.includes(id) && all.indexOf(id) === i);
        this.ring.open(variant, tools.length > 0 ? tools : TOOLS);
    }

    setSeatForTest(value) {
        this.mode?.setSeatForTest(value);
    }

    // The glass draws behind the hint and the open ring
    glassSurfaces() {
        return [...this.mode?.glassSurfaces() ?? [], ...this.ring?.glassSurfaces() ?? []];
    }

    surfacesChanged() {
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    state() {
        return {
            mode: this.mode?.state() ?? null,
            ring: this.ring?.state() ?? null,
            key: this._bound,
        };
    }
}
