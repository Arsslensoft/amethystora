// Pick a look in front: Control's welcome page, which user-setup.hooks.d/25-first-look.sh starts a few seconds after
// the Manual's welcome page at a new account's first login, is given the keyboard once the person is free to use it.
// Started from a systemd unit, it comes without an activation token, and a first login starts in the overview, with
// GNOME's own welcome dialog (which offers the Tour) over it, so Mutter maps it behind. With nothing modal up, Mutter gives
// a window without a token the focus: the window that had it before the page showed gets it back.
// The page's window is activated once, when it shows or as soon as GNOME's dialog, or any other modal one, has
// closed. The overview counts as free while it shows its windows (no search, no app grid): activating the page leaves
// it, as Take Tour does. Never when the Tour was started, when a window other than the Manual has the focus, or when
// the person launched something from the overview's search or app grid. A Control already running takes the hook's
// `welcome` in its own window: no new window, so nothing is brought forward then.
//
// Armed only where the hook opens the page: a home folder less than a day old (the hook's own rule) with no
// first-look.json, which the page writes when it shows. Anywhere else nothing is connected; armed, one handler on
// window-created and one on each new window's first showing, until the page has been seen.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const CONTROL = 'org.amethystora.Control';
const MANUAL = 'org.amethystora.Manual';
const TOUR = 'org.gnome.Tour.desktop';
const PAGE = 'welcome';
const DAY = 86400;

// The overview's search and its button to the app grid, where the session has an overview
function overviewParts() {
    try {
        return {search: Main.overview.searchController, apps: Main.overview.dash.showAppsButton};
    } catch {
        return {};
    }
}

export class FirstLook {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._watched = new Set();
        this._window = null;
        this._idleId = 0;
        this._armed = this._fresh();
        if (!this._armed)
            return;
        global.display.connectObject('window-created', (display, window) => this._watch(window), this);
    }

    disable() {
        this._stop();
        this._watched = null;
    }

    // For the suites: 'off' (nothing connected), 'watching' (for the page's window) or 'waiting' (to activate it)
    get state() {
        return this._window ? 'waiting' : this._armed ? 'watching' : 'off';
    }

    _fresh() {
        const state = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora', 'first-look.json']);
        if (GLib.file_test(state, GLib.FileTest.EXISTS))
            return false;
        try {
            const born = Gio.File.new_for_path(GLib.get_home_dir())
                .query_info('time::created', Gio.FileQueryInfoFlags.NONE, null)
                .get_attribute_uint64('time::created');
            // A home folder whose birth the file system does not keep (0) counts as an existing account, as in the hook
            return born > 0 && GLib.get_real_time() / 1e6 - born < DAY;
        } catch {
            return false;
        }
    }

    // A window's first showing, when its class and its process are known
    _watch(window) {
        this._watched.add(window);
        const before = global.display.focus_window;
        const forget = () => {
            window.disconnectObject(this);
            this._watched?.delete(window);
        };
        window.connectObject(
            'shown', () => {
                forget();
                if (this._isPage(window))
                    this._found(window, before);
            },
            'unmanaged', forget,
            this);
    }

    // Control started as `amethystora-control welcome`: /usr/bin's wrapper execs the program with its arguments
    _isPage(window) {
        if (window.get_wm_class() !== CONTROL)
            return false;
        const pid = window.get_pid();
        if (!(pid > 0))
            return false;
        try {
            const argv = new TextDecoder().decode(GLib.file_get_contents(`/proc/${pid}/cmdline`)[1]).split('\0');
            return argv[1] === PAGE;
        } catch {
            return false;
        }
    }

    _found(window, before) {
        // Once: nothing else is watched from here on
        global.display.disconnectObject(this);
        for (const other of this._watched)
            other.disconnectObject(this);
        this._watched.clear();
        this._window = window;
        this._before = before;
        this._picked = false;
        const queue = () => this._queue();
        window.connectObject('unmanaged', () => this._stop(), this);
        global.display.connectObject('notify::focus-window', queue, this);
        // A modal dialog's end gives the stage's key focus back
        global.stage.connectObject('notify::key-focus', queue, this);
        const {search, apps} = overviewParts();
        // The overview leaving from its search or its app grid is the person launching what they picked: theirs wins
        Main.overview.connectObject('hiding', () => {
            this._picked ||= !!(search?.search_active || apps?.checked);
        }, 'hidden', queue, this);
        search?.connectObject('notify::search-active', queue, this);
        apps?.connectObject('notify::checked', queue, this);
        queue();
    }

    _queue() {
        this._idleId ||= GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
            this._idleId = 0;
            this._try();
            return GLib.SOURCE_REMOVE;
        });
    }

    _try() {
        const window = this._window;
        if (!window)
            return;
        const focus = global.display.focus_window;
        // Given the focus by Mutter, and in view
        if (focus === window && !Main.overview.visible) {
            const before = this._before;
            if (before?.get_compositor_private() && before.get_wm_class() !== MANUAL)
                Main.activateWindow(before, global.display.get_current_time_roundtrip());
            this._stop();
            return;
        }
        if (!this._free())
            return;
        try {
            const tour = Shell.AppSystem.get_default().lookup_app(TOUR);
            // Any window but the page and the Manual is one the person chose, before the page showed or since
            const moved = focus && focus !== window && focus.get_wm_class() !== MANUAL;
            if (!moved && !this._picked && (!tour || tour.state === Shell.AppState.STOPPED))
                Main.activateWindow(window, global.display.get_current_time_roundtrip());
        } finally {
            this._stop();
        }
    }

    // No dialog holds the keyboard; the overview only while it shows the windows
    _free() {
        if (Main.actionMode === Shell.ActionMode.NORMAL)
            return Main.modalCount === 0;
        if (Main.actionMode !== Shell.ActionMode.OVERVIEW)
            return false;
        const {search, apps} = overviewParts();
        return !search?.search_active && !apps?.checked;
    }

    _stop() {
        if (this._idleId)
            GLib.source_remove(this._idleId);
        this._idleId = 0;
        global.display.disconnectObject(this);
        global.stage.disconnectObject(this);
        Main.overview.disconnectObject(this);
        const {search, apps} = overviewParts();
        search?.disconnectObject(this);
        apps?.disconnectObject(this);
        this._window?.disconnectObject(this);
        this._window = null;
        this._before = null;
        this._armed = false;
        this._picked = false;
        for (const window of this._watched ?? [])
            window.disconnectObject(this);
        this._watched?.clear();
    }
}
