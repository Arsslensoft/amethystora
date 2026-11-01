// The apps used lately (spec 1.5): the order in which apps had the focus in this session, newest first, kept in
// memory and written nowhere. Before any app has had the focus, GNOME's own record of app use fills it. With GNOME's
// usage history off there is none. Each dock shows the first three that are neither pinned nor running, wherever their
// windows are (on another screen or, with this workspace's apps only, another workspace).

import Gio from 'gi://Gio';
import Shell from 'gi://Shell';

import {EventEmitter} from 'resource:///org/gnome/shell/misc/signals.js';
import * as AppFavorites from 'resource:///org/gnome/shell/ui/appFavorites.js';
import * as ParentalControlsManager from 'resource:///org/gnome/shell/misc/parentalControlsManager.js';

export const SHOWN = 3;
// The most remembered, which is more than shown since the ones running or pinned are passed over
const KEPT = 32;
// The order kept while the module is off, in this shell only, so that turning it off and on again shows the same apps
// (spec 7.1.35); a new session starts from GNOME's record again
let remembered = [];

export class Recent extends EventEmitter {
    start() {
        this._ids = [...remembered];
        this._privacy = new Gio.Settings({schema_id: 'org.gnome.desktop.privacy'});
        this._privacy.connectObject('changed::remember-app-usage', () => {
            this._ids = [];
            this._seed();
            this.emit('changed');
        }, this);
        Shell.WindowTracker.get_default().connectObject('notify::focus-app', () => this._focused(), this);
        this._seed();
        this.connections = 2;
    }

    stop() {
        remembered = this.remembering ? this._ids : [];
        this._privacy?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._privacy?.run_dispose();
        this._privacy = null;
        Shell.WindowTracker.get_default().disconnectObject(this);
        this._ids = [];
        this.connections = 0;
    }

    get remembering() {
        return this._privacy?.get_boolean('remember-app-usage') ?? false;
    }

    get ids() {
        return this.remembering ? [...this._ids] : [];
    }

    _seed() {
        if (!this.remembering || this._ids.length)
            return;
        this._ids = Shell.AppUsage.get_default().get_most_used().map(app => app.get_id()).slice(0, KEPT);
    }

    _focused() {
        if (!this.remembering)
            return;
        const app = Shell.WindowTracker.get_default().focus_app;
        const id = app?.get_id();
        if (!id || app.is_window_backed())
            return;
        if (this._ids[0] === id)
            return;
        this._ids = [id, ...this._ids.filter(other => other !== id)].slice(0, KEPT);
        this.emit('changed');
    }

    // The apps a dock shows: installed, shown in menus, allowed by Parental Controls, neither pinned nor running
    // (running: the dock's own running group, which also holds an app still starting)
    shownFor(running) {
        const system = Shell.AppSystem.get_default();
        const favorites = AppFavorites.getAppFavorites();
        const parental = ParentalControlsManager.getDefault();
        const apps = [];
        for (const id of this.ids) {
            if (apps.length >= SHOWN)
                break;
            const app = system.lookup_app(id);
            const info = app?.get_app_info();
            if (!app || !info || app.is_window_backed() || !info.should_show() || favorites.isFavorite(id) ||
                running.includes(app) || app.state === Shell.AppState.RUNNING || !parental.shouldShowApp(info))
                continue;
            apps.push(app);
        }
        return apps;
    }
}
