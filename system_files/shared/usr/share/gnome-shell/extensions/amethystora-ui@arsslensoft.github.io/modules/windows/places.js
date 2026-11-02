// Windows that go back to their places, in the classic layout. For each arrangement of screens (which screens, by
// their connectors, at which sizes and positions), each app's windows are remembered where they last were, and an
// app's new window opens where that app's window was; plugging a screen in or out moves the open windows to their
// places in the new arrangement. Apps cannot place themselves on Wayland, so this keeps the places, in
// ~/.local/state/amethystora/places.json with a copy beside it. A file that cannot be read, or holds what this never
// writes, is left out piece by piece and never stops the shell; nothing in it is ever run or shown.

import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const FILE = GLib.build_filenamev([DIR, 'places.json']);
const COPY = `${FILE}.bak`;
const MAX_ARRANGEMENTS = 8;
const MAX_APPS = 100;
const MAX_PLACES = 4;
const WRITE_DELAY = 2000;
const SETTLE = 500;
// After the screens change, and after a move of its own, what Mutter does to a window is not anybody's choice
const QUIET = 1000;

const isText = v => typeof v === 'string' && v.length > 0 && v.length <= 256 && v !== '__proto__';
// A map from names, with no prototype a name from the file could reach
const table = () => Object.create(null);
const isInt = (v, lo, hi) => Number.isInteger(v) && v >= lo && v <= hi;
const isObject = v => !!v && typeof v === 'object' && !Array.isArray(v);
const now = () => Math.floor(Date.now() / 1000);
// Notes' quick note (amethystora-notes capture) opens in the middle of the screen, as GNOME places it, not where
// Notes' own window was: it is Notes' one window that is not its application window, which GTK alone gives an object
// path. Known by the time the window is first configured.
export const quickNote = w => w?.get_wm_class() === 'org.amethystora.Notes' && !w.get_gtk_window_object_path();

const checkPlace = p => isObject(p) && isText(p.m) && isInt(p.x, -32768, 32768) && isInt(p.y, -32768, 32768) &&
    isInt(p.w, 50, 16384) && isInt(p.h, 50, 16384) && typeof p.max === 'boolean';

// What fits the shape this writes, and nothing else
export function clean(data) {
    const result = {version: 1, arrangements: table()};
    if (!isObject(data) || data.version !== 1 || !isObject(data.arrangements))
        return result;
    for (const [key, arrangement] of Object.entries(data.arrangements)) {
        if (!isText(key) || !isObject(arrangement) || !isObject(arrangement.apps))
            continue;
        const apps = table();
        for (const [id, app] of Object.entries(arrangement.apps)) {
            if (!isText(id) || !isObject(app) || !Array.isArray(app.places))
                continue;
            const places = app.places.slice(0, MAX_PLACES).map(p => checkPlace(p) ? {...p} : null);
            if (!places.some(Boolean))
                continue;
            apps[id] = {t: Number.isFinite(app.t) ? app.t : 0, places};
        }
        result.arrangements[key] = {used: Number.isFinite(arrangement.used) ? arrangement.used : 0, apps};
    }
    return trim(result);
}

// The caps: the least recently used arrangements and apps go first
function trim(data) {
    const keys = Object.keys(data.arrangements)
        .sort((a, b) => data.arrangements[b].used - data.arrangements[a].used);
    for (const key of keys.slice(MAX_ARRANGEMENTS))
        delete data.arrangements[key];
    for (const arrangement of Object.values(data.arrangements)) {
        const ids = Object.keys(arrangement.apps).sort((a, b) => arrangement.apps[b].t - arrangement.apps[a].t);
        for (const id of ids.slice(MAX_APPS))
            delete arrangement.apps[id];
    }
    return data;
}

function read(path) {
    try {
        const [, bytes] = GLib.file_get_contents(path);
        const data = JSON.parse(new TextDecoder().decode(bytes));
        return isObject(data) ? data : null;
    } catch {
        return null;
    }
}

// Every screen by its connector and its number now
export function screens() {
    const list = [];
    for (const logical of global.backend.get_monitor_manager().get_logical_monitors()) {
        const connector = logical.get_monitors()[0]?.get_connector();
        if (connector)
            list.push({connector, number: logical.get_number()});
    }
    return list;
}

// A place, moved and if need be shrunk into its screen's work area now; null when too little of it is left. With
// `fallback`, a screen that is not there any more is the main one, at the same offsets.
export function clampPlace(place, monitors, fallback = false) {
    let monitor = monitors.find(m => m.connector === place.m);
    if (!monitor && fallback)
        monitor = {number: Main.layoutManager.primaryIndex};
    if (!monitor || !Main.layoutManager.monitors[monitor.number])
        return null;
    const area = Main.layoutManager.getWorkAreaForMonitor(monitor.number);
    const geometry = Main.layoutManager.monitors[monitor.number];
    const w = Math.min(place.w, area.width);
    const h = Math.min(place.h, area.height);
    if (w * h < place.w * place.h / 2)
        return null;
    const x = Math.max(area.x, Math.min(geometry.x + place.x, area.x + area.width - w));
    const y = Math.max(area.y, Math.min(geometry.y + place.y, area.y + area.height - h));
    return {x, y, width: w, height: h, max: place.max};
}

// The app a window belongs to, by the most lasting name it has; null for a window that has none
export function appOf(window) {
    const app = Shell.WindowTracker.get_default().get_window_app(window);
    const id = app?.get_id();
    if (id && !id.startsWith('window:'))
        return id;
    return window.get_sandboxed_app_id() || window.get_gtk_application_id() ||
        (window.get_wm_class() ? `wm_class:${window.get_wm_class()}` : null);
}

export class Places {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
        this.lastApplied = null;
    }

    enable() {
        const main = read(FILE);
        const copy = main ? null : read(COPY);
        this._data = clean(main ?? copy ?? {});
        this._moved = new WeakMap();
        this._pending = new Map();
        // The place each new window was given in configure, for its first show
        this._given = new WeakMap();
        // The windows created and not yet shown, which are never recorded
        this._unplaced = new WeakSet();
        this._watched = new Set();
        // Windows whose move or new size waits a moment to be recorded (_recordSoon)
        this._soon = new Set();
        this._soonId = 0;
        this._grabbed = null;
        this._quietUntil = 0;
        this._heldKey = null;
        if (this._ext.modules?.get('desks')?.layouts)
            this.hold();
        if (copy)
            this._write();
        global.display.connectObject('window-created', (d, window) => this._created(window),
            // A window being moved or resized is recorded once, when it is let go: not at every step
            'grab-op-begin', (d, window) => (this._grabbed = window),
            'grab-op-end', (d, window) => {
                this._grabbed = null;
                if (window)
                    this._record(window);
            },
            'window-entered-monitor', (d, monitor, window) => this._recordSoon(window), this);
        Main.layoutManager.connectObject('monitors-changed', () => this._screensChanged(), this);
        for (const actor of global.get_window_actors())
            this._watch(actor.meta_window);
    }

    // save: false when the person turned places off, and nothing more may be written
    disable(save = true) {
        global.display.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        for (const window of this._watched ?? [])
            window.disconnectObject(this);
        this._watched = null;
        for (const [window, owner] of this._pending ?? [])
            window.disconnectObject(owner);
        this._pending = null;
        if (this._settleOwner)
            global.display.disconnectObject(this._settleOwner);
        this._settleOwner = null;
        this._soon = null;
        for (const id of [this._writeId, this._settleId, this._soonId])
            if (id)
                GLib.source_remove(id);
        this._writeId = 0;
        this._settleId = 0;
        this._soonId = 0;
        if (save && this._dirty)
            this._write();
        this._data = null;
    }

    // For tests
    get data() {
        return this._data;
    }

    // Where an app's window was in the arrangement last settled, which the windows themselves no longer say when the
    // screens have just changed (desks/layouts.js)
    leftPlace(id, rank) {
        return this._data?.arrangements[this._heldKey]?.apps[id]?.places[rank] ?? null;
    }

    // The arrangement now is the one leftPlace speaks of, until the screens change. Asked only while desks keep the
    // windows' places (key `desk-windows`): with it off nothing here looks at the arrangement for it.
    hold() {
        this._heldKey = this._arrangement().key;
    }

    // --- the arrangement ------------------------------------------------------------------------------------------

    // Every screen by its connector and where it is, in logical pixels, sorted by connector
    _arrangement() {
        const monitors = [];
        for (const logical of global.backend.get_monitor_manager().get_logical_monitors()) {
            const number = logical.get_number();
            const connector = logical.get_monitors()[0]?.get_connector();
            const m = Main.layoutManager.monitors[number];
            if (connector && m)
                monitors.push({connector, number, text: `${connector}:${m.width}x${m.height}@${m.x},${m.y}`});
        }
        monitors.sort((a, b) => a.connector < b.connector ? -1 : a.connector > b.connector ? 1 : 0);
        return {key: monitors.map(m => m.text).join(' '), monitors};
    }

    // --- remembering ----------------------------------------------------------------------------------------------

    _qualifies(window) {
        if (quickNote(window))
            return false;
        return window && window.get_window_type() === Meta.WindowType.NORMAL && !window.get_transient_for() &&
            !window.skip_taskbar && !this._module.isKept(window);
    }

    _watch(window) {
        if (!this._qualifies(window))
            return;
        const record = () => this._recordSoon(window);
        this._watched.add(window);
        // A move counts as well as a change of size: a window moved without one (Super+Z's cells, the keyboard, an app)
        // was otherwise kept where it had been, and a size changed on its way to a new place was kept with the place
        // it had before the move (a drag is recorded once, when it is let go)
        window.connectObject('size-changed', record, 'position-changed', record,
            'notify::maximized-horizontally', record, 'notify::maximized-vertically', record,
            'unmanaging', () => {
                this._record(window);
                window.disconnectObject(this);
                this._watched?.delete(window);
            }, this);
    }

    // The windows of an app that count, in the order they were opened
    _siblings(window, id) {
        return global.get_window_actors().map(a => a.meta_window)
            .filter(w => w && this._qualifies(w) && appOf(w) === id)
            .sort((a, b) => a.get_stable_sequence() - b.get_stable_sequence());
    }

    // A move or a change of size is recorded a moment later (SETTLE), once it is known whether the screens changed:
    // Mutter moves and shrinks windows to fit a new arrangement before the shell hears of it, and those places belong
    // to nobody. Recorded at once, they replaced the person's own place in the arrangement being left.
    _recordSoon(window) {
        if (!this._soon)
            return;
        this._soon.add(window);
        this._soonId ||= GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE, () => {
            this._soonId = 0;
            const windows = [...this._soon ?? []];
            this._soon?.clear();
            for (const each of windows) {
                if (this._watched?.has(each))
                    this._record(each);
            }
            return GLib.SOURCE_REMOVE;
        });
    }

    _record(window) {
        if (!this._data || !this._qualifies(window) || window.is_fullscreen() || window === this._grabbed)
            return;
        const time = GLib.get_monotonic_time();
        // A window not yet shown has not been placed: what Mutter gives it first is nobody's choice
        if (this._unplaced.has(window) || time < this._quietUntil || time < (this._moved.get(window) ?? 0))
            return;
        const id = appOf(window);
        if (!id)
            return;
        this._module.count('placesRecord');
        const rank = this._siblings(window, id).indexOf(window);
        if (rank < 0 || rank >= MAX_PLACES)
            return;
        const {key, monitors} = this._arrangement();
        const monitor = monitors.find(m => m.number === window.get_monitor());
        if (!key || !monitor)
            return;
        const geometry = Main.layoutManager.monitors[monitor.number];
        const max = window.maximized_horizontally && window.maximized_vertically;
        // The caps are only applied when something new was added
        const added = !this._data.arrangements[key] || !this._data.arrangements[key].apps[id];
        const arrangement = this._data.arrangements[key] ??= {used: now(), apps: table()};
        arrangement.used = now();
        const app = arrangement.apps[id] ??= {t: now(), places: []};
        app.t = now();
        const old = app.places[rank];
        const f = window.get_frame_rect();
        // A maximised window keeps the place it had before, for when it is not maximised any more
        const base = max && old?.m === monitor.connector ? old
            : {x: f.x - geometry.x, y: f.y - geometry.y, w: f.width, h: f.height};
        const place = {m: monitor.connector, x: base.x, y: base.y, w: base.w, h: base.h, max};
        if (!checkPlace(place))
            return;
        while (app.places.length < rank)
            app.places.push(null);
        app.places[rank] = place;
        if (added)
            trim(this._data);
        this._dirty = true;
        this._queueWrite();
    }

    _queueWrite() {
        if (this._writeId)
            return;
        this._writeId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, WRITE_DELAY, () => {
            this._writeId = 0;
            this._write();
            return GLib.SOURCE_REMOVE;
        });
    }

    _write() {
        if (!this._data)
            return;
        if (this._writeId)
            GLib.source_remove(this._writeId);
        this._writeId = 0;
        try {
            const text = JSON.stringify(this._data);
            GLib.mkdir_with_parents(DIR, 0o700);
            GLib.file_set_contents(FILE, text);
            GLib.file_set_contents(COPY, text);
            this._dirty = false;
        } catch (e) {
            console.warn(`Amethystora UI: the windows' places could not be written to ${FILE}: ${e.message}`);
        }
    }

    // --- putting back ---------------------------------------------------------------------------------------------

    _placeFor(window, rank) {
        const id = appOf(window);
        if (!id)
            return null;
        const {key, monitors} = this._arrangement();
        const place = this._data?.arrangements[key]?.apps[id]?.places[rank];
        return place ? clampPlace(place, monitors) : null;
    }

    _created(window) {
        if (!this._qualifies(window))
            return;
        this._watch(window);
        this._unplaced.add(window);
        // Asked once, as Mutter is about to tell the app its first size and place; an X11 window is never configured
        // this way, and is placed when it shows
        const owner = {};
        this._pending.set(window, owner);
        window.connectObject('configure', (w, config) => {
            if (config.get_is_initial?.())
                this._configure(window, config);
        }, 'shown', () => {
            window.disconnectObject(owner);
            this._pending.delete(window);
            this._shown(window);
        }, 'unmanaging', () => {
            window.disconnectObject(owner);
            this._pending.delete(window);
        }, owner);
    }

    _rank(window) {
        const id = appOf(window);
        return id ? this._siblings(window, id).filter(w => w !== window).length : -1;
    }

    // A window a saved set is opening goes where the set says (workspaces/sets.js), not where its app's was; so do the
    // drop-down terminal and an app opened again by Super+Alt+T (the module's claims)
    _claimed(window) {
        return (this._ext.modules?.get('workspaces')?.claimed?.(window) ?? false) || this._module.claims(window);
    }

    _configure(window, config) {
        if (quickNote(window) || this._claimed(window))
            return;
        const rank = this._rank(window);
        const place = rank >= 0 ? this._placeFor(window, rank) : null;
        if (!place)
            return;
        this._moved.set(window, GLib.get_monotonic_time() + QUIET * 1000);
        config.set_position(place.x, place.y);
        config.set_size(place.width, place.height);
        this._given.set(window, place);
        this.lastApplied = {path: 'configure', app: appOf(window), rect: [place.x, place.y, place.width, place.height]};
    }

    _shown(window) {
        this._unplaced.delete(window);
        if (this._claimed(window))
            return;
        let place = this._given.get(window);
        this._given.delete(window);
        const f = window.get_frame_rect();
        if (place) {
            if (this.lastApplied)
                this.lastApplied.shown = [f.x, f.y, f.width, f.height];
            // Mutter may take the size given in configure and not the place (GNOME 51 puts such a window at 0,0):
            // moved there now, when it is more than 2 px off
            if (Math.abs(f.x - place.x) > 2 || Math.abs(f.y - place.y) > 2) {
                this._moved.set(window, GLib.get_monotonic_time() + QUIET * 1000);
                window.move_frame(false, place.x, place.y);
                if (this.lastApplied)
                    this.lastApplied.moved = true;
            }
        } else {
            // Not placed in configure (an X11 window): moved now
            const rank = this._rank(window);
            place = rank >= 0 ? this._placeFor(window, rank) : null;
            if (!place)
                return;
            this._moved.set(window, GLib.get_monotonic_time() + QUIET * 1000);
            // The place first: a size below the window's smallest makes Mutter take nothing of the request
            window.move_frame(false, place.x, place.y);
            window.move_resize_frame(false, place.x, place.y, place.width, place.height);
            this.lastApplied = {path: 'shown', app: appOf(window), rect: [place.x, place.y, place.width, place.height],
                shown: [f.x, f.y, f.width, f.height]};
        }
        if (place.max)
            window.maximize();
    }

    // The screens changed: once their work areas are known, each window with a place in the new arrangement goes
    // there
    _screensChanged() {
        // What is held for the arrangement being left, before anything is put back (desks/layouts.js)
        const layouts = this._ext.modules?.get('desks')?.layouts;
        layouts?.leaving();
        if (layouts)
            this.hold();
        this._quietUntil = GLib.get_monotonic_time() + QUIET * 1000;
        // Hotplug says so more than once: only the last one waits
        if (this._settleId)
            GLib.source_remove(this._settleId);
        this._settleId = 0;
        if (this._settleOwner)
            global.display.disconnectObject(this._settleOwner);
        const owner = this._settleOwner = {};
        const settle = () => {
            if (this._settleOwner !== owner)
                return;
            global.display.disconnectObject(owner);
            this._settleOwner = null;
            if (this._settleId)
                GLib.source_remove(this._settleId);
            this._settleId = 0;
            this._applyAll();
        };
        global.display.connectObject('workareas-changed', settle, owner);
        this._settleId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE, () => {
            this._settleId = 0;
            settle();
            return GLib.SOURCE_REMOVE;
        });
    }

    _applyAll() {
        this._quietUntil = GLib.get_monotonic_time() + QUIET * 1000;
        const byApp = new Map();
        for (const actor of global.get_window_actors()) {
            const window = actor.meta_window;
            if (!this._qualifies(window) || window.is_fullscreen())
                continue;
            const id = appOf(window);
            if (id)
                byApp.set(id, [...byApp.get(id) ?? [], window]);
        }
        for (const windows of byApp.values()) {
            windows.sort((a, b) => a.get_stable_sequence() - b.get_stable_sequence());
            windows.forEach((window, rank) => {
                const place = this._placeFor(window, rank);
                if (!place)
                    return;
                if (place.max) {
                    window.maximize();
                    return;
                }
                if (window.maximized_horizontally || window.maximized_vertically)
                    window.unmaximize();
                // The place first: a size below the window's smallest makes Mutter take nothing of the request
                window.move_frame(false, place.x, place.y);
                window.move_resize_frame(false, place.x, place.y, place.width, place.height);
            });
        }
    }
}
