// Saved sets: a workspace's apps and where their windows are, kept under the workspace's name and opened again from
// the workspace menu. Opening a set starts its apps, as their launchers do and with no file, on the workspace you
// are on if it is empty, else on the next empty one; each window goes to its place as it first appears, and windows
// of the set's apps already there are put in place rather than started twice. Nothing is ever closed.
//
// A set holds only desktop file IDs, numbers and the name: never a title, a file or a command. The IDs are only
// looked up in GNOME's list of apps, and an app is only ever started through it. The sets are kept in
// ~/.local/state/amethystora/sets.json with a copy beside it; what does not fit the shape this writes is left out
// piece by piece, and a file that cannot be read never stops the shell.
//
// Nothing is connected between openings. While one runs (30 seconds at most) it watches for new windows, and each
// new window until it is placed. The apps open before an update's restart (workspaces/resume.js) come back the same
// way, one workspace after another (openQueue).

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {appOf, clampPlace, quickNote, screens} from '../windows/places.js';
import {MAX_BYTES, MAX_WINDOWS, addSet, clean, cleanName, isPlace, slot, tileOrder} from './setsdata.js';

const DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const FILE = GLib.build_filenamev([DIR, 'sets.json']);
const COPY = `${FILE}.bak`;
const WRITE_DELAY = 500;
// How long an opening waits for its windows, in seconds
const GIVE_UP = 30;

const now = () => Math.floor(Date.now() / 1000);

// A file's JSON, or null when it is missing, too large, or not JSON
function read(path) {
    try {
        const info = Gio.File.new_for_path(path).query_info('standard::size', Gio.FileQueryInfoFlags.NONE, null);
        if (info.get_size() > MAX_BYTES)
            return null;
        const [, bytes] = GLib.file_get_contents(path);
        const data = JSON.parse(new TextDecoder().decode(bytes));
        return data && typeof data === 'object' && !Array.isArray(data) ? data : null;
    } catch {
        return null;
    }
}

function hasWindows(workspace) {
    return workspace.list_windows().some(w => !w.is_on_all_workspaces());
}

// Whether a workspace is still one of GNOME's: with dynamic workspaces an empty one can go at any time, and a
// workspace that went must never be asked anything again
export function alive(workspace) {
    const manager = global.workspace_manager;
    for (let i = 0; i < manager.n_workspaces; i++) {
        if (manager.get_workspace_by_index(i) === workspace)
            return true;
    }
    return false;
}

// One saved window of an opening, waiting for its window
class Claim {
    constructor(opening, saved, app, tile) {
        this.opening = opening;
        this.app = app;
        this.appId = saved.app;
        this.rank = saved.rank;
        this.place = saved.place ?? null;
        this.tile = tile;
        this.workspace = opening.target;
        // The window it holds, whether that window is new, and whether it is placed, risen, taken by the tiling
        this.window = null;
        this.fresh = false;
        this.configured = false;
        this.done = false;
        this.rose = false;
        this.taken = false;
    }

    // Where its window goes in a strip, among the opening's windows already there (setsdata.js slot)
    slotIn(strip, except = null) {
        const columns = strip.columns
            .map(column => column.filter(w => w !== except))
            .filter(column => column.length)
            .map(column => column.map(w => this.opening.byWindow.get(w)?.tile).filter(Boolean));
        return slot(columns, this.tile);
    }
}

export class Sets {
    constructor(module) {
        this._module = module;
        this._ext = module._ext;
    }

    enable() {
        const main = read(FILE);
        const copy = main ? null : read(COPY);
        this._data = clean(main ?? copy ?? {});
        this._writeId = 0;
        this._dirty = false;
        this._opening = null;
        // The workspace of an opening that ended with apps still on their way, for a while: their windows may still
        // come there, so the next set does not open there too
        this._recent = null;
        // The windows openings claimed, for the tiling's and the motion's questions after an opening is over
        this._claims = new WeakMap();
        // How many signal connections an opening holds on the display now (0 between openings), for the tests
        this.connections = 0;
        // The workspaces still to open after an update's restart (openQueue), and the one shown at the end
        this._queue = null;
        this._queueActive = null;
        this._queueId = 0;
        if (copy)
            this._write();
    }

    disable() {
        this._clearQueue();
        this._endOpening();
        if (this._writeId)
            GLib.source_remove(this._writeId);
        this._writeId = 0;
        if (this._dirty)
            this._write();
        this._data = null;
        this._claims = null;
    }

    // ---- The list ----

    get list() {
        return this._data?.sets ?? [];
    }

    // The tests' view: never a window's title, only what the menu shows and how an opening stands
    snapshot() {
        const opening = this._opening;
        return {
            sets: this.list.map(set => ({
                name: set.name, layout: set.layout, count: set.windows.length,
                apps: [...new Set(set.windows.map(w => w.app))],
            })),
            opening: opening ? {
                claims: opening.claims.length,
                claimed: opening.claims.filter(c => c.window).length,
                connected: opening.connected,
            } : null,
            connections: this.connections,
            queued: this._queue?.length ?? 0,
            risen: this._ext.modules?.get('motion')?.risen ?? 0,
        };
    }

    forget(name) {
        if (!this._data)
            return;
        const sets = this._data.sets.filter(s => s.name !== name);
        if (sets.length === this._data.sets.length)
            return;
        this._data = {version: 1, sets};
        this._queueWrite();
    }

    _queueWrite() {
        this._dirty = true;
        if (this._writeId)
            return;
        this._writeId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, WRITE_DELAY, () => {
            this._writeId = 0;
            this._write();
            return GLib.SOURCE_REMOVE;
        });
    }

    // The file and its copy, each written whole under another name and moved into place, readable by the account
    // alone
    _write() {
        if (!this._data)
            return;
        if (this._writeId)
            GLib.source_remove(this._writeId);
        this._writeId = 0;
        try {
            const bytes = new TextEncoder().encode(JSON.stringify(this._data));
            GLib.mkdir_with_parents(DIR, 0o700);
            for (const path of [FILE, COPY])
                GLib.file_set_contents_full(path, bytes, GLib.FileSetContentsFlags.CONSISTENT, 0o600);
            this._dirty = false;
        } catch (e) {
            console.warn(`Amethystora UI: the saved sets could not be written to ${FILE}: ${e.message}`);
        }
    }

    // ---- Saving ----

    _tiling() {
        return this._ext.modules?.get('tiling')?.engine ?? null;
    }

    // A window a set can hold: a normal window of its own, of an app with a launcher, that is not Notes' quick note
    _savable(window) {
        if (!window || window.get_window_type() !== Meta.WindowType.NORMAL || window.get_transient_for() ||
            window.skip_taskbar || window.is_on_all_workspaces() || quickNote(window))
            return false;
        const app = Shell.WindowTracker.get_default().get_window_app(window);
        return !!app && !app.is_window_backed() && !!app.get_app_info();
    }

    // The windows of a workspace a set would hold, from the bottom of the stack to the top
    windowsOf(workspace) {
        const windows = workspace.list_windows().filter(w => this._savable(w));
        return global.display.sort_windows_by_stacking(windows);
    }

    // The name a set of this workspace takes: the workspace's own, else the one the top bar shows for it, else its
    // number
    nameFor(index, workspace) {
        const name = cleanName(this._module.titleOf(index, workspace));
        return name || cleanName(_('Workspace %d').format(index + 1));
    }

    // {name, count, left}: the set saved, how many windows it holds and how many were left out
    save(workspace) {
        if (!this._data || !workspace)
            return null;
        const captured = this.capture(workspace);
        if (!captured)
            return null;
        this._data = addSet(this._data, captured.set);
        this._queueWrite();
        return {name: captured.set.name, count: captured.set.windows.length, left: captured.left};
    }

    // A workspace's windows as a set would hold them, checked as the file would be ({set, left}), or null when it has none
    capture(workspace) {
        if (!workspace)
            return null;
        const index = workspace.index();
        const all = this.windowsOf(workspace);
        // Beyond the limit, the windows used last are kept: the top of the stack
        const windows = all.slice(-MAX_WINDOWS);
        const tracker = Shell.WindowTracker.get_default();
        const engine = this._tiling();
        const list = screens();
        // An app's windows are ranked oldest first, which is the order an opening uses them in again
        const ranks = new Map();
        const byAge = [...windows].sort((a, b) => a.get_stable_sequence() - b.get_stable_sequence());
        const seen = new Map();
        for (const window of byAge) {
            const id = tracker.get_window_app(window).get_id();
            const rank = seen.get(id) ?? 0;
            seen.set(id, rank + 1);
            ranks.set(window, rank);
        }
        // Under the tiling: the strip's columns that hold a window of the set, numbered from 0 left to right
        const strip = engine ? windows.map(w => engine.stripOf(w)).find(Boolean) ?? null : null;
        const used = strip ? strip.columns.map(column => column.filter(w => windows.includes(w))) : [];
        const columns = used.filter(column => column.length);

        const saved = [];
        for (const window of windows) {
            const entry = {app: tracker.get_window_app(window).get_id(), rank: ranks.get(window)};
            const place = this._placeOf(window, engine, list);
            if (place)
                entry.place = place;
            const at = strip && engine.stripOf(window) === strip ? columns.findIndex(c => c.includes(window)) : -1;
            if (at >= 0) {
                const width = engine.info(window)?.width || window.get_frame_rect().width;
                entry.tile = {column: at, row: columns[at].indexOf(window), width: Math.max(50, Math.min(16384, width))};
            }
            saved.push(entry);
        }
        const name = this.nameFor(index, workspace);
        const set = {name, saved: now(), layout: engine ? 'tiling' : 'classic', windows: saved};
        // What the file would hold is what is kept: an entry the file would refuse is not kept either
        const checked = clean({version: 1, sets: [set]}).sets[0];
        return checked ? {set: checked, left: all.length - windows.length} : null;
    }

    // Where a window is on its screen: where it is drawn, under the tiling, which may be beside its frame
    _placeOf(window, engine, list) {
        const monitor = list.find(m => m.number === window.get_monitor());
        const geometry = Main.layoutManager.monitors[window.get_monitor()];
        if (!monitor || !geometry)
            return null;
        const vis = engine?.info(window)?.vis;
        const f = window.get_frame_rect();
        const r = vis ?? {x: f.x, y: f.y, width: f.width, height: f.height};
        const place = {
            m: monitor.connector, x: Math.round(r.x - geometry.x), y: Math.round(r.y - geometry.y),
            w: Math.round(r.width), h: Math.round(r.height),
            max: window.maximized_horizontally && window.maximized_vertically,
        };
        return isPlace(place) ? place : null;
    }

    // ---- Opening ----

    // The workspace a set opens on: the one you are on when it is empty, else the next empty one after it, going
    // round, else the one you are on
    _targetFor(active) {
        const recent = this._recent && GLib.get_monotonic_time() < this._recent.until ? this._recent.target : null;
        const empty = workspace => !hasWindows(workspace) && workspace !== recent;
        if (empty(active))
            return active;
        const manager = global.workspace_manager;
        const count = manager.n_workspaces;
        for (let step = 1; step < count; step++) {
            const workspace = manager.get_workspace_by_index((active.index() + step) % count);
            if (workspace && empty(workspace))
                return workspace;
        }
        return active;
    }

    open(name) {
        const set = this.list.find(s => s.name === name);
        if (!set)
            return;
        // A set opened from the menu ends what the queue below still had to open
        this._clearQueue();
        this._openSet(set, null, true);
    }

    // A set that is not in the list (a put-away's, workspaces/putaway.js): opened as the menu opens a saved one
    // `named` whether an unnamed workspace takes the set's name: a put-away's never does, so that its name is written nowhere in the clear
    openGiven(set, named = true) {
        if (!this._data)
            return;
        this._clearQueue();
        this._openSet(set, null, named);
    }

    // The apps open before an update's restart (workspaces/resume.js): each entry ({index, layout, windows}) on the
    // workspace at its own place, one after another, and the workspace that was in front shown at the end
    openQueue(entries, active) {
        this._clearQueue();
        this._endOpening();
        this._queue = [...entries];
        this._queueActive = active;
        this._nextQueued();
    }

    _clearQueue() {
        if (this._queueId)
            GLib.source_remove(this._queueId);
        this._queueId = 0;
        this._queue = null;
        this._queueActive = null;
    }

    // The workspace at a place, made when dynamic workspaces have fewer; null when GNOME keeps a fixed number short of it
    _workspaceAt(index) {
        const manager = global.workspace_manager;
        while (manager.n_workspaces <= index && Meta.prefs_get_dynamic_workspaces() && manager.n_workspaces < 36)
            manager.append_new_workspace(false, global.get_current_time());
        return manager.get_workspace_by_index(index);
    }

    _nextQueued() {
        const queue = this._queue;
        if (!queue || !this._data)
            return;
        const entry = queue.shift();
        if (!entry) {
            const front = this._queueActive !== null ? global.workspace_manager.get_workspace_by_index(this._queueActive) : null;
            this._clearQueue();
            if (front && front !== global.workspace_manager.get_active_workspace())
                front.activate(global.get_current_time());
            return;
        }
        const set = {name: `${entry.index + 1}`, saved: 0, layout: entry.layout, windows: entry.windows};
        const target = this._workspaceAt(entry.index);
        this._openSet(set, target, false, true);
    }

    // `target` the workspace to open on (null: the one you are on when empty, else the next empty one), `named` whether
    // an unnamed workspace takes the set's name, `queued` whether the queue goes on when this opening ends
    _openSet(set, target, named, queued = false) {
        this._endOpening();
        const manager = global.workspace_manager;
        const active = manager.get_active_workspace();
        target ??= this._targetFor(active);
        const appSystem = Shell.AppSystem.get_default();
        const order = tileOrder(set);
        const since = Math.max(0, ...global.display.list_all_windows().map(w => w.get_stable_sequence()));
        const opening = {
            name: set.name, target, since, began: GLib.get_monotonic_time(), claims: [], byWindow: new Map(),
            apps: new Map(), owners: new Map(), connected: false, timer: 0, later: 0, queued,
        };
        set.windows.forEach((saved, index) => {
            // An app that is not installed any more is left out
            const app = appSystem.lookup_app(saved.app);
            if (!app)
                return;
            const claim = new Claim(opening, saved, app, order[index]);
            opening.claims.push(claim);
            let group = opening.apps.get(saved.app);
            if (!group) {
                group = {app, claims: [], waiting: false};
                opening.apps.set(saved.app, group);
            }
            group.claims.push(claim);
        });
        for (const group of opening.apps.values())
            group.claims.sort((a, b) => a.rank - b.rank);
        this._opening = opening;

        if (target !== active)
            target.activate(global.get_current_time());
        if (named)
            this._module.nameIfUnnamed(target.index(), set.name);

        // New windows are watched from now on, before any app is asked for one
        global.display.connectObject('window-created', (d, window) => this._created(window), this);
        // The workspace going away (dynamic workspaces) ends the opening
        manager.connectObject('workspace-removed', () => {
            if (!alive(opening.target))
                this._endOpening();
        }, this);
        this.connections += 2;
        opening.connected = true;
        opening.timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, GIVE_UP, () => {
            opening.timer = 0;
            this._endOpening();
            return GLib.SOURCE_REMOVE;
        });

        this._useWhatIsThere(opening);
        for (const group of opening.apps.values())
            this._start(opening, group);
        this._check(opening);
    }

    // Windows of the set's apps already on the workspace are its windows, oldest first
    _useWhatIsThere(opening) {
        for (const [id, group] of opening.apps) {
            const there = opening.target.list_windows()
                .filter(w => this._savable(w) && !opening.byWindow.has(w) && this._matches(w, id, group.app))
                .sort((a, b) => a.get_stable_sequence() - b.get_stable_sequence());
            for (const claim of group.claims) {
                const window = there.shift();
                if (!window)
                    break;
                this._take(claim, window, false);
                this._placeNow(claim, window);
            }
        }
    }

    // The app is asked for the windows that are still missing: started when it is not running, else for a new window;
    // an app that keeps one window brings that window over
    _start(opening, group) {
        const open = group.claims.filter(c => !c.window && !c.done);
        if (!open.length)
            return;
        const {app} = group;
        if (!alive(opening.target))
            return;
        const index = opening.target.index();
        if (app.state === Shell.AppState.STOPPED) {
            try {
                app.launch(0, index, Shell.AppLaunchGpu.APP_PREF);
            } catch (e) {
                console.warn(`Amethystora UI: the set ${opening.name} could not start ${group.app.get_id()}: ${e.message}`);
                for (const claim of open)
                    claim.done = true;
                Main.notify(_('%s could not be started').format(app.get_name()), opening.queued
                    ? _('The apps that were open before the restart came back without it.')
                    : _('The set %s opened without it.').format(opening.name));
            }
            return;
        }
        if (app.can_open_new_window()) {
            app.open_new_window(index);
            return;
        }
        // One window only: the one used last, from wherever it is
        const window = app.get_windows().find(w => this._savable(w) && !opening.byWindow.has(w) &&
            w.get_workspace() !== opening.target);
        if (window) {
            this._take(open[0], window, false);
            window.change_workspace(opening.target);
            this._placeNow(open[0], window);
            open.shift();
        } else if (app.state === Shell.AppState.STARTING) {
            // Its window is on its way
            return;
        }
        for (const claim of open)
            claim.done = true;
    }

    // Whether a window is an app's: by the app GNOME found for it, or by the names the window gives itself
    _matches(window, id, app) {
        if (appOf(window) === id)
            return true;
        const wmClass = window.get_wm_class();
        for (const name of [window.get_sandboxed_app_id(), window.get_gtk_application_id(), wmClass]) {
            if (name && `${name}.desktop` === id)
                return true;
        }
        const startup = app.get_app_info()?.get_startup_wm_class?.();
        return !!startup && startup === wmClass;
    }

    // A window that appeared after the opening began, matched to the first open claim of its app; asked when the
    // window is first configured or shown, by the tiling when it settles the window, by places and by the motion.
    _match(window) {
        const opening = this._opening;
        if (!opening || !window)
            return this._claims?.get(window) ?? null;
        const known = opening.byWindow.get(window);
        if (known)
            return known;
        if (window.get_stable_sequence() <= opening.since || !this._savable(window))
            return null;
        // Never a window the person opened on another workspace meanwhile: only one that appears on the set's
        // workspace, or while that is the one shown
        if (window.get_workspace() !== opening.target &&
            global.workspace_manager.get_active_workspace() !== opening.target)
            return null;
        for (const [id, group] of opening.apps) {
            const claim = group.claims.find(c => !c.window && !c.done);
            if (claim && this._matches(window, id, group.app)) {
                this._take(claim, window, true);
                this._askNext(opening, group);
                return claim;
            }
        }
        return null;
    }

    // The app's next window is asked for once the one before is here. An app only says whether it can open another
    // once it is running: until then this waits for it.
    _askNext(opening, group) {
        if (!group.claims.some(c => !c.window && !c.done) || group.waiting)
            return;
        const {app} = group;
        if (app.state === Shell.AppState.STARTING) {
            group.waiting = true;
            app.connectObject('notify::state', () => {
                if (app.state === Shell.AppState.STARTING)
                    return;
                app.disconnectObject(group);
                group.waiting = false;
                if (this._opening === opening)
                    this._askNext(opening, group);
            }, group);
            return;
        }
        if (!alive(opening.target)) {
            this._endOpening();
            return;
        }
        if (app.state === Shell.AppState.RUNNING && app.can_open_new_window()) {
            app.open_new_window(opening.target.index());
            return;
        }
        // It keeps one window, or it quit: the rest of its windows are not coming
        for (const claim of group.claims) {
            if (!claim.window)
                claim.done = true;
        }
        this._check(opening);
    }

    _take(claim, window, fresh) {
        claim.window = window;
        claim.fresh = fresh;
        claim.opening.byWindow.set(window, claim);
        this._claims.set(window, claim);
        const opening = claim.opening;
        // Matched inside another module's window-created handler, before ours ran for it: watched from here
        if (fresh && !opening.owners.has(window))
            this._created(window);
        // Every claim has its window: nothing more needs watching for new ones. Not from inside the very signal this
        // may be called from: just before the next frame.
        if (opening.connected && !opening.later && opening.claims.every(c => c.window || c.done)) {
            opening.later = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
                opening.later = 0;
                this._unwatch(opening);
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    _unwatch(opening) {
        if (!opening.connected)
            return;
        global.display.disconnectObject(this);
        global.workspace_manager.disconnectObject(this);
        this.connections -= 2;
        opening.connected = false;
    }

    _created(window) {
        const opening = this._opening;
        if (!opening || window.get_window_type() !== Meta.WindowType.NORMAL || opening.owners.has(window))
            return;
        // Only a window that may still be claimed, or is
        if (!opening.byWindow.has(window) && opening.claims.every(c => c.window || c.done))
            return;
        const owner = {};
        opening.owners.set(window, owner);
        const forget = () => {
            window.disconnectObject(owner);
            opening.owners.delete(window);
        };
        window.connectObject('configure', (w, config) => {
            if (config.get_is_initial?.())
                this._configure(window, config);
        }, 'shown', () => {
            forget();
            this._shown(window);
        }, 'unmanaging', () => {
            forget();
            const claim = opening.byWindow.get(window);
            if (claim)
                claim.done = true;
            this._check(opening);
        }, owner);
    }

    // A Wayland window is told its first place before it draws anything: the set's place, in the classic layout
    _configure(window, config) {
        const claim = this._match(window);
        if (!claim || this._tiling() || !claim.place || !alive(claim.workspace))
            return;
        if (window.get_workspace() !== claim.workspace)
            window.change_workspace(claim.workspace);
        const rect = clampPlace(claim.place, screens(), true);
        if (!rect)
            return;
        config.set_position(rect.x, rect.y);
        config.set_size(rect.width, rect.height);
        claim.configured = true;
    }

    _shown(window) {
        const claim = this._match(window);
        if (!claim)
            return;
        if (!this._tiling() && alive(claim.workspace)) {
            if (window.get_workspace() !== claim.workspace)
                window.change_workspace(claim.workspace);
            // An X11 window is never configured that way, and some apps take a size from the first configure but
            // not its place: either is moved now, before it is first drawn
            const rect = claim.place ? clampPlace(claim.place, screens(), true) : null;
            const f = window.get_frame_rect();
            if (rect && (Math.abs(f.x - rect.x) > 2 || Math.abs(f.y - rect.y) > 2 ||
                Math.abs(f.width - rect.width) > 2 || Math.abs(f.height - rect.height) > 2))
                window.move_resize_frame(false, rect.x, rect.y, rect.width, rect.height);
            if (claim.place?.max)
                window.maximize();
        }
        claim.done = true;
        this._check(claim.opening);
    }

    // A window already open, used by the set: put in its place at once
    _placeNow(claim, window) {
        claim.done = true;
        if (!alive(claim.workspace))
            return;
        if (window.minimized)
            window.unminimize();
        const engine = this._tiling();
        if (engine) {
            if (engine.tileable(window)) {
                claim.taken = true;
                engine.placeClaimed(window, claim, {appear: false});
            }
        } else if (claim.place) {
            const rect = clampPlace(claim.place, screens(), true);
            if (rect) {
                if (window.maximized_horizontally || window.maximized_vertically)
                    window.unmaximize();
                window.move_resize_frame(false, rect.x, rect.y, rect.width, rect.height);
                if (rect.max)
                    window.maximize();
            }
        }
    }

    // Over once every claim has its window placed, or was let go
    _check(opening) {
        if (opening === this._opening && opening.claims.every(c => c.done))
            this._endOpening();
    }

    _endOpening() {
        const opening = this._opening;
        if (!opening)
            return;
        this._opening = null;
        if (opening.later)
            global.compositor.get_laters().remove(opening.later);
        opening.later = 0;
        this._unwatch(opening);
        // Apps still on their way may yet open their windows there, until the opening's 30 s are up: one that gave up
        // leaves nothing to keep clear
        const until = opening.began + GIVE_UP * GLib.USEC_PER_SEC;
        if (opening.claims.some(c => !c.window && !c.done) && alive(opening.target) &&
            GLib.get_monotonic_time() < until)
            this._recent = {target: opening.target, until};
        else if (this._recent?.target === opening.target)
            this._recent = null;
        if (opening.timer)
            GLib.source_remove(opening.timer);
        opening.timer = 0;
        for (const [window, owner] of opening.owners)
            window.disconnectObject(owner);
        opening.owners.clear();
        for (const group of opening.apps.values())
            group.app.disconnectObject(group);
        // Windows that were not there in time are not waited for any more
        for (const claim of opening.claims) {
            if (!claim.window)
                claim.done = true;
        }
        // The next workspace of the queue, once this signal or timer has returned
        if (opening.queued && this._queue && !this._queueId) {
            this._queueId = GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
                this._queueId = 0;
                this._nextQueued();
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    // ---- What the other modules ask ----

    // The tiling, settling a new window: the claim it places, once
    claim(window) {
        const claim = this._match(window);
        if (!claim || claim.taken || !claim.fresh || !alive(claim.workspace))
            return null;
        claim.taken = true;
        return claim;
    }

    // Places: whether a set is placing this window
    claimed(window) {
        return !!this._match(window)?.fresh;
    }

    // The motion: whether this window is one of a set appearing for the first time, asked once
    arriving(window) {
        const claim = this._match(window);
        if (!claim || !claim.fresh || claim.rose)
            return false;
        claim.rose = true;
        return true;
    }
}
