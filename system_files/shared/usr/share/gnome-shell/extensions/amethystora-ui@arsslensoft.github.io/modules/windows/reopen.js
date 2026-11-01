// Reopen the app you just closed: Super+Alt+T opens again the app whose window closed last, on its workspace and, in
// the classic layout, at its place and size; again for the one before, five back. Only the app is started, as its
// launcher starts it, with no file: what it had open is its own to restore.
//
// As windows close, the app (by its launcher's ID), the workspace and the place are kept in memory, five at most,
// gone at log-out and never written anywhere. Opening one again is a saved set of one (workspaces/sets.js): the app's
// next window, within ten seconds, is told its place before it draws, and put there when it shows if it took no notice.

import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {Osd} from './osd.js';
import {appOf, clampPlace, quickNote, screens} from './places.js';

export const DEPTH = 5;
const CLAIM = 10 * GLib.USEC_PER_SEC;

// What closed, the newest first; at module scope, so that it lives through a screen lock (which stops the extension)
let closed = [];

export class Reopen {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
    }

    enable() {
        this._watched = new Set();
        this._osd = new Osd();
        this._claim = null;
        Main.wm.addKeybinding('reopen-key', this._module.settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL, () => this.reopen());
        global.display.connectObject('window-created', (d, window) => this._created(window), this);
        for (const actor of global.get_window_actors())
            this._watch(actor.meta_window);
    }

    disable() {
        Main.wm.removeKeybinding('reopen-key');
        global.display.disconnectObject(this);
        for (const window of this._watched ?? [])
            window.disconnectObject(this);
        this._watched = null;
        this._endClaim();
        this._osd?.hide();
        this._osd = null;
    }

    // Turned off by the person: nothing is kept
    forget() {
        closed = [];
    }

    // For the tests: what would open, newest first
    get closed() {
        return closed.map(entry => ({...entry}));
    }

    // --- remembering --------------------------------------------------------------------------------------------

    _qualifies(window) {
        if (!window || window.get_window_type() !== Meta.WindowType.NORMAL || window.get_transient_for() ||
            window.skip_taskbar || quickNote(window) || this._module.holds(window))
            return false;
        const app = Shell.WindowTracker.get_default().get_window_app(window);
        return !!app && !app.is_window_backed();
    }

    _watch(window) {
        if (!window || this._watched.has(window) || window.get_window_type() !== Meta.WindowType.NORMAL)
            return;
        this._watched.add(window);
        window.connectObject('unmanaging', () => {
            window.disconnectObject(this);
            this._watched?.delete(window);
            this._record(window);
        }, this);
    }

    _record(window) {
        if (!this._qualifies(window))
            return;
        const app = Shell.WindowTracker.get_default().get_window_app(window);
        const monitor = window.get_monitor();
        const geometry = Main.layoutManager.monitors[monitor];
        const connector = screens().find(m => m.number === monitor)?.connector ?? null;
        const f = window.get_frame_rect();
        const place = geometry && connector ? {
            m: connector, x: f.x - geometry.x, y: f.y - geometry.y, w: f.width, h: f.height,
            max: window.maximized_horizontally && window.maximized_vertically,
        } : null;
        const workspace = window.is_on_all_workspaces() ? -1 : window.get_workspace()?.index() ?? -1;
        closed.unshift({app: app.get_id(), workspace, place});
        closed.length = Math.min(closed.length, DEPTH);
    }

    // --- opening again ------------------------------------------------------------------------------------------

    reopen() {
        const system = Shell.AppSystem.get_default();
        let entry = closed.shift();
        let app = entry ? system.lookup_app(entry.app) : null;
        // One that is not installed any more is passed over
        while (entry && !app) {
            entry = closed.shift();
            app = entry ? system.lookup_app(entry.app) : null;
        }
        if (!entry) {
            this._osd.show(_('Nothing closed to reopen'), {at: global.display.get_current_monitor()});
            return false;
        }
        const manager = global.workspace_manager;
        const workspace = entry.workspace >= 0 && entry.workspace < manager.n_workspaces
            ? manager.get_workspace_by_index(entry.workspace) : manager.get_active_workspace();
        if (workspace !== manager.get_active_workspace())
            workspace.activate(global.get_current_time());
        this._endClaim();
        this._claim = {app: entry.app, appInfo: app, place: entry.place, workspace, until: GLib.get_monotonic_time() + CLAIM};
        this._claimTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, CLAIM / 1000, () => {
            this._claimTimer = 0;
            this._endClaim();
            return GLib.SOURCE_REMOVE;
        });
        const index = workspace.index();
        try {
            if (app.state === Shell.AppState.RUNNING && app.can_open_new_window())
                app.open_new_window(index);
            else if (app.state === Shell.AppState.RUNNING)
                app.activate();
            else
                app.launch(0, index, Shell.AppLaunchGpu.APP_PREF);
        } catch (e) {
            console.warn(`Amethystora UI: ${entry.app} did not open again: ${e.message}`);
            this._endClaim();
            Main.notify(_('%s could not be started').format(app.get_name()), '');
            return false;
        }
        const words = this._module.classic && entry.place
            ? _('%s, back in its place').format(app.get_name())
            : _('%s, back on its workspace').format(app.get_name());
        this._osd.show(words, {at: global.display.get_current_monitor()});
        return true;
    }

    _endClaim() {
        if (this._claimTimer)
            GLib.source_remove(this._claimTimer);
        this._claimTimer = 0;
        this._claim = null;
    }

    // Whether a window is the one an opening waits for: by the app GNOME found for it, or by its names
    _matches(window, claim) {
        if (!claim || GLib.get_monotonic_time() > claim.until || window.get_window_type() !== Meta.WindowType.NORMAL ||
            window.get_transient_for())
            return false;
        if (appOf(window) === claim.app)
            return true;
        const wmClass = window.get_wm_class();
        for (const name of [window.get_sandboxed_app_id(), window.get_gtk_application_id(), wmClass]) {
            if (name && `${name}.desktop` === claim.app)
                return true;
        }
        const startup = claim.appInfo.get_app_info()?.get_startup_wm_class?.();
        return !!startup && startup === wmClass;
    }

    // Places asks, before it puts a new window where its app's was: this one goes where it was when it closed
    claimed(window) {
        const claim = this._claim;
        if (!claim || !window)
            return false;
        if (claim.window)
            return claim.window === window;
        return this._matches(window, claim);
    }

    _created(window) {
        this._watch(window);
        const claim = this._claim;
        if (!claim || claim.window || window.get_window_type() !== Meta.WindowType.NORMAL)
            return;
        // Its app is known for certain only once it has said its names: asked in its first configure, or when it shows
        const owner = {};
        window.connectObject('configure', (w, config) => {
            if (config.get_is_initial?.())
                this._configure(window, config);
        }, 'shown', () => {
            window.disconnectObject(owner);
            this._shown(window);
        }, 'unmanaging', () => window.disconnectObject(owner), owner);
    }

    _take(window) {
        const claim = this._claim;
        if (!claim || claim.window || !this._matches(window, claim))
            return null;
        claim.window = window;
        if (window.get_workspace() !== claim.workspace && !window.is_on_all_workspaces())
            window.change_workspace(claim.workspace);
        return claim;
    }

    _configure(window, config) {
        const claim = this._take(window) ?? (this._claim?.window === window ? this._claim : null);
        if (!claim || !this._module.classic || !claim.place)
            return;
        const rect = clampPlace(claim.place, screens(), true);
        if (!rect)
            return;
        config.set_position(rect.x, rect.y);
        config.set_size(rect.width, rect.height);
    }

    _shown(window) {
        const claim = this._take(window) ?? (this._claim?.window === window ? this._claim : null);
        if (!claim)
            return;
        if (this._module.classic && claim.place) {
            const rect = clampPlace(claim.place, screens(), true);
            const f = window.get_frame_rect();
            if (rect && (Math.abs(f.x - rect.x) > 2 || Math.abs(f.y - rect.y) > 2 ||
                Math.abs(f.width - rect.width) > 2 || Math.abs(f.height - rect.height) > 2))
                window.move_resize_frame(false, rect.x, rect.y, rect.width, rect.height);
            if (rect?.max)
                window.maximize();
        }
        this._endClaim();
    }
}
