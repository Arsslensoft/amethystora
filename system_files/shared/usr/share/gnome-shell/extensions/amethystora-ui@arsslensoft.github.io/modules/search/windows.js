// Open windows, found by their title or their app's name, on every workspace, the one used last first; Enter switches
// to it. The windows are asked for at each search, and nothing is kept between searches but which window each result
// of the last one was. A window's title is shown as the window gives it, as plain text.

import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {makeEntry, queryWords, rank, score} from '../fuzzy/matcher.js';

// How much evidence each word typed needs: about a word's start, or a typo in it; a letter or two in common is not a
// window anyone asked for
const ENOUGH = 0.4;

export class WindowsProvider {
    constructor() {
        this.id = 'amethystora-windows';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._found = new Map();
        this._previous = new Map();
    }

    destroy() {
        this._found = new Map();
        this._previous = new Map();
    }

    getInitialResultSet(terms) {
        // The last search's results stay known until the next one is in: GNOME may open one of them meanwhile
        this._previous = this._found;
        this._found = new Map();
        // clip: is the clipboard's
        if (/^clip:/iu.test(terms.join(' ')))
            return Promise.resolve([]);
        const tracker = Shell.WindowTracker.get_default();
        const windows = global.display.get_tab_list(Meta.TabList.NORMAL_ALL, null)
            .filter(window => !window.skip_taskbar && window.get_title());
        const entries = new Map();
        const order = new Map();
        windows.forEach((window, index) => {
            const id = `win:${window.get_stable_sequence()}`;
            const app = tracker.get_window_app(window);
            entries.set(id, makeEntry(id, {names: [window.get_title()], keywords: [app?.get_name() ?? '']}));
            order.set(id, index);
            this._found.set(id, new WeakRef(window));
        });
        const query = queryWords(terms);
        const ids = rank(entries, terms, [], (a, b) => order.get(a) - order.get(b))
            .filter(id => score(entries.get(id), query) >= ENOUGH * query.length);
        for (const id of this._found.keys()) {
            if (!ids.includes(id))
                this._found.delete(id);
        }
        return Promise.resolve(ids);
    }

    getSubsearchResultSet(previous, terms) {
        return this.getInitialResultSet(terms);
    }

    _window(id) {
        const window = (this._found.get(id) ?? this._previous.get(id))?.deref();
        return window && window.get_compositor_private() ? window : null;
    }

    getResultMetas(ids) {
        const tracker = Shell.WindowTracker.get_default();
        const several = global.workspace_manager.n_workspaces > 1;
        return Promise.resolve(ids.map(id => {
            const window = this._window(id);
            if (!window)
                return {id, name: id};
            const app = tracker.get_window_app(window);
            const workspace = window.get_workspace();
            return {
                id,
                name: window.get_title(),
                description: several && workspace && !window.is_on_all_workspaces()
                    ? Meta.prefs_get_workspace_name(workspace.index()) : '',
                createIcon: size => app?.create_icon_texture(size) ?? null,
                hint: _('Enter switches'),
            };
        }));
    }

    filterResults(ids, max) {
        return ids.slice(0, max);
    }

    // As GNOME's activateWindow does, without asking the overview to go: GNOME's result does that itself, which the
    // search box answers by closing
    activateResult(id) {
        const window = this._window(id);
        if (!window)
            return;
        const time = global.get_current_time();
        const workspace = window.get_workspace();
        if (workspace && workspace.index() !== global.workspace_manager.get_active_workspace_index())
            workspace.activate_with_focus(window, time);
        else
            window.activate(time);
    }
}
