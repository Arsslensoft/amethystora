// Windows tiled in a strip per workspace, wider than the screen: each new window opens beside the focused one,
// none overlaps another, and the strip scrolls to keep the focused one in view. Super and the arrows move
// around it, Super+Ctrl and the arrows move windows along it, and the windows that should not tile (dialogs, and
// those lifted out with Super+Ctrl+Escape) float above it. "enabled" is the classic layout's switch (ame desktop
// layout): off, every window is GNOME's own again and every GNOME shortcut the tiling took is given back, at once,
// with no logging out. Its other settings (the gaps, the steps, the speeds, the gestures, the window rules) are in
// the preferences window (prefs.js), and each applies as soon as it is changed.

import GLib from 'gi://GLib';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {Actions} from './tiling/actions.js';
import {Config, config} from './tiling/config.js';
import {Engine, Open} from './tiling/engine.js';
import {Keys, giveBackLeftovers} from './tiling/keys.js';
import {Sessions} from './tiling/session.js';
import {State} from './tiling/state.js';
import {TopBar} from './tiling/topbar.js';
import {GnomeSettings} from './tiling/settings.js';
import {Pointer} from './tiling/pointer.js';
import {ShellPatches} from './tiling/patches.js';

export class Tiling {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this.state = new State();
        this._settings = this._ext.getSettings('org.gnome.shell.extensions.amethystora-ui.tiling');
        this._keySettings = this._ext.getSettings('org.gnome.shell.extensions.amethystora-ui.tiling.keybindings');
        this._settings.connectObject('changed::enabled', () => this._sync(), this);
        this._config = new Config(this._settings);
        this._config.enable(key => this._configChanged(key));
        // Where new windows open was kept in the state file before it was a setting
        const kept = this.state.get('openPosition');
        if (kept !== undefined) {
            if (Number.isInteger(kept) && this._settings.get_user_value('open-window-position') === null)
                this._settings.set_int('open-window-position', kept);
            this.state.set('openPosition', undefined);
        }
        this._history = [];
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._config?.disable();
        this._config = null;
        if (this._gapsTimer)
            GLib.source_remove(this._gapsTimer);
        this._gapsTimer = 0;
        // At a screen lock the tiling comes back as it was at unlock
        this._stop(Main.sessionMode.isLocked);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        this._keySettings?.run_dispose();
        this._keySettings = null;
        this.state = null;
    }

    get settings() {
        return this._settings;
    }

    // A setting changed: what is on the screen follows it. The gaps wait for a moment of quiet, so that a slider
    // dragged across lays the windows out once and not at every step.
    _configChanged(key) {
        if (!this.engine)
            return;
        if (/margin|gap|steps|selection-border|maximize/.test(key)) {
            if (this._gapsTimer)
                GLib.source_remove(this._gapsTimer);
            this._gapsTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 500, () => {
                this._gapsTimer = 0;
                this.engine?.layoutAll();
                this.pointer?.rebuild();
                return GLib.SOURCE_REMOVE;
            });
        } else if (key === 'default-focus-mode') {
            for (const strip of this.engine.strips())
                this.actions.setFocusMode(strip, config.defaultMode);
        } else if (/^(show-|open-window-position|default-show-top-bar)/.test(key)) {
            this.topBar?.sync();
            this.topBar?.applyTopBar();
        } else if (/^(gesture|edge-preview|topbar-mouse)/.test(key)) {
            this.pointer?.rebuild();
        } else if (/^(workspace-|default-background)/.test(key)) {
            this.engine.backgrounds?.sync();
        }
    }

    _sync() {
        if (this._settings.get_boolean('enabled')) {
            this._start();
        } else {
            this._stop(false);
            // What an earlier session took from GNOME and could not give back
            giveBackLeftovers(this.state);
            GnomeSettings.giveBackLeftovers(this.state);
        }
    }

    get running() {
        return !!this.engine;
    }

    // Where new windows open, the person's choice, kept across log-outs
    get openPosition() {
        return Object.values(Open).includes(config.openPosition) ? config.openPosition : Open.RIGHT;
    }

    set openPosition(value) {
        this._settings.set_int('open-window-position', value);
    }

    _start() {
        if (this.engine)
            return;
        // Each part is started in turn, and a failure stops the whole tiling cleanly rather than half of it
        try {
            this.gnome = new GnomeSettings(this.state);
            this.gnome.enable();
            this.engine = new Engine(this);
            this.engine.enable();
            this.actions = new Actions(this);
            this.sessions = new Sessions(this);
            this.keys = new Keys(this._keySettings, this.actions.table(), this.state);
            this.keys.enable();
            this.topBar = new TopBar(this);
            this.topBar.enable();
            this.pointer = new Pointer(this);
            this.pointer.enable();
            this.patches = new ShellPatches(this);
            this.patches.enable();
            global.workspace_manager.connectObject('active-workspace-changed',
                () => this.noteWorkspace(global.workspace_manager.get_active_workspace()), this);
            this.noteWorkspace(global.workspace_manager.get_active_workspace());
        } catch (e) {
            console.error(`Amethystora UI: the tiling did not start: ${e.message}\n${e.stack}`);
            this._stop(false);
        }
    }

    _stop(suspend) {
        global.workspace_manager.disconnectObject(this);
        for (const part of ['patches', 'pointer', 'topBar', 'sessions', 'keys', 'engine', 'gnome']) {
            const object = this[part];
            this[part] = null;
            if (!object)
                continue;
            try {
                if (part === 'sessions')
                    object.destroy();
                else
                    object.disable(suspend);
            } catch (e) {
                console.error(`Amethystora UI: the tiling's ${part} did not stop cleanly: ${e.message}\n${e.stack}`);
            }
        }
        this.actions = null;
    }

    // The workspaces in the order they were last shown, for going back through them
    noteWorkspace(workspace) {
        this._history = [workspace, ...this._history.filter(w => w !== workspace)];
    }

    workspaceHistory() {
        const manager = global.workspace_manager;
        const all = Array.from({length: manager.n_workspaces}, (_, i) => manager.get_workspace_by_index(i));
        this._history = this._history.filter(w => all.includes(w));
        return [...this._history, ...all.filter(w => !this._history.includes(w))];
    }

    monitorsChanged() {
        this.gnome?.monitorsChanged();
    }
}
