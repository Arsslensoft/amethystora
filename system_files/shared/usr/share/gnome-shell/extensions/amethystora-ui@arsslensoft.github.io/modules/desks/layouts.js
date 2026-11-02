// Screens come back as you left them (R7-46, plans/ui-ideas/r7/spec-sturdy.md 2): plug the dock back in and every window
// goes back to its screen, place and size, instead of a pile on one display. Runs apart from Desks' own key
// (`desk-windows`, on; ame desktop desks windows on|off), and knows a set of screens as Desks does: by maker, model and
// serial number, so that a dock that names its ports differently today changes nothing.
//
// Mutter moves the windows before the shell hears that the screens changed (display.c on_monitors_changed_internal), so
// what a window's frame says then is nobody's choice. What was there before is read from what already knows: the tiling's
// strips, as its _monitorsChanged is about to merge them, and in the classic layout what the windows' places recorded for
// the arrangement being left (windows/places.js). Each calls leaving() first. That leaves nothing to watch: nothing runs
// but on a change of screens or a lock.
//
// Kept in desks.json's `layouts` (layoutsdata.js has the shape): app IDs, numbers and a hash of each screen, never a title.
// Read and written without blocking; each write goes through the file as it is then, so that it leaves the rest alone, and
// is refused by the file's etag if somebody else (the recipe) wrote it meanwhile, when it reads again. Windows are only
// moved, and never started or closed. A window opened since stays where it is. Turning the key off forgets the layouts
// (forgetLayouts), however it was turned off.
//
// One signal and, for two seconds after the screens change, one timeout. Off, nothing is made.

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {appOf, clampPlace, quickNote, screens} from '../windows/places.js';
import {fadeIn} from './card.js';
import {screenSet} from './identity.js';
import {cleanLayout, cleanLayouts, match, strips, visit} from './layoutsdata.js';
import {FILE, MOST_BYTES} from './state.js';

const COPY = `${FILE}.bak`;
// The screens settle for this long before the desk is worked out, as Desks waits
const SETTLE = 2000;
const NOTE_FOR = 4000;

// The last desk this GNOME Shell knew, across the disable and enable of every lock
let lastBase = null;
// Each write after the last, across the disable and enable too, so that one does not lose what another has under way
let queue = Promise.resolve();
const TRIES = 3;

// {text, etag}, or {text: null, etag: null} when the file is not there, unreadable or too big
function readText(path) {
    return new Promise(resolve => {
        Gio.File.new_for_path(path).load_contents_async(null, (file, result) => {
            try {
                const [, bytes, etag] = file.load_contents_finish(result);
                resolve({text: bytes.length <= MOST_BYTES ? new TextDecoder().decode(bytes) : null, etag});
            } catch {
                resolve({text: null, etag: null});
            }
        });
    });
}

// Whether the write is over: true when it wrote or failed for good, false when the file changed since `etag` was read
function writeText(path, text, etag = null) {
    return new Promise(resolve => {
        Gio.File.new_for_path(path).replace_contents_bytes_async(new GLib.Bytes(new TextEncoder().encode(text)), etag, false,
            Gio.FileCreateFlags.REPLACE_DESTINATION | Gio.FileCreateFlags.PRIVATE, null, (file, result) => {
                try {
                    file.replace_contents_finish(result);
                } catch (e) {
                    if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.WRONG_ETAG)) {
                        resolve(false);
                        return;
                    }
                    console.warn(`Amethystora UI: desks could not write ${path}: ${e.message}`);
                }
                resolve(true);
            });
    });
}

function parse(text) {
    try {
        const data = JSON.parse(text);
        return data && typeof data === 'object' && !Array.isArray(data) ? data : null;
    } catch {
        return null;
    }
}

// The file as it is, the copy standing in as for Desks (state.js), or a new one; with the file's etag for a write
async function readFile() {
    const main = await readText(FILE);
    const data = parse(main.text) ?? parse((await readText(COPY)).text) ?? {version: 1, desks: []};
    return {data, etag: main.etag};
}

// DATA changed by CHANGE, written through the file as it is then: CHANGE gets the file's content and returns the text to
// write, or null to leave it. Read again, up to three times, when somebody wrote the file meanwhile. After the writes
// queued before it.
function update(change) {
    queue = queue.then(async () => {
        for (let attempt = 0; attempt < TRIES; attempt++) {
            const {data, etag} = await readFile();
            const text = change(data);
            if (text === null)
                return;
            if (new TextEncoder().encode(text).length > MOST_BYTES)
                return;
            if (await writeText(FILE, text, etag)) {
                await writeText(COPY, text);
                return;
            }
        }
    }).catch(e => console.warn(`Amethystora UI: desks: ${e.message}`));
    return queue;
}

// Turned off: what was kept of the windows' places goes from the file (desks.json keeps its other contents)
export function forgetLayouts() {
    return update(data => {
        if (!data.layouts)
            return null;
        const {layouts, ...rest} = data;
        return JSON.stringify(rest);
    });
}

export class Layouts {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._here = (this._screenSet ?? screenSet)();
        // The classic layout's places are read for the arrangement as it is now (switched on while the module runs)
        this._ext.modules?.get('windows')?.places?.hold?.();
        // Until the screens have settled, the strips and the places are what the change made of them: nothing is kept
        this._settled = true;
        global.backend.get_monitor_manager().connectObject('monitors-changed', () => this._changed(), this);
        // Brought to another desk while the screen was locked, with the extension off
        if (lastBase !== null && lastBase !== this._here.base) {
            this._cameFrom = lastBase;
            this._changed();
        }
        lastBase = this._here.base;
    }

    disable() {
        global.backend.get_monitor_manager().disconnectObject(this);
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        this._closeNote();
        this._here = null;
    }

    // ---- Keeping ----

    // The screens have changed and the tiling or the places are about to act on it: what they still hold is kept under
    // the desk the screens were. Only the first of a run of changes counts.
    leaving() {
        if (!this._settled || !this._here)
            return;
        this._settled = false;
        this._keep();
        // Whichever hears of the change first: what is kept is never left unsettled
        this._changed();
    }

    // A screen lock: the computer may be taken from the desk while the extension is off
    beforeLock() {
        if (this._settled && this._here)
            this._keep();
    }

    _keep() {
        const layout = cleanLayout(this._capture());
        if (!layout)
            return;
        const base = this._here.base;
        update(data => {
            const layouts = visit(data.layouts, base, layout);
            return layouts ? JSON.stringify({...data, layouts}) : null;
        });
    }

    // What is on the screens, as desks.json would keep it
    _capture() {
        const keys = this._here.keys;
        const engine = this._ext.modules?.get('tiling')?.engine ?? null;
        const places = this._ext.modules?.get('windows')?.places ?? null;
        const ranks = this._ranks();
        const windows = [];
        const shown = {};
        if (engine) {
            for (const strip of engine.strips()) {
                const screen = keys.get(engine.connectorOf(strip.monitor));
                if (!screen)
                    continue;
                const workspace = strip.workspace.index();
                if (engine.screens?.monitorShowing(strip.workspace) === strip.monitor)
                    shown[screen] = workspace;
                strip.columns.forEach((column, at) => column.forEach((window, row) => {
                    const held = ranks.get(window);
                    if (held) {
                        const width = engine.info(window)?.width || window.get_frame_rect().width;
                        windows.push({...held, workspace, screen, column: at, row, width});
                    }
                }));
            }
        } else if (places) {
            for (const [window, held] of ranks) {
                const place = places.leftPlace(held.app, held.rank);
                const screen = place ? keys.get(place.m) : null;
                const workspace = window.get_workspace();
                if (screen && workspace) {
                    windows.push({...held, workspace: workspace.index(), screen, x: place.x, y: place.y, w: place.w,
                        h: place.h, max: place.max});
                }
            }
        }
        return {seen: 0, mode: engine ? 'tiling' : 'classic', shown, windows};
    }

    // Every window that can be kept, with its app and rank: a launcher's, oldest first in its app
    _ranks() {
        const seen = new Map();
        const ranks = new Map();
        for (const {window, app} of this._openWindows()) {
            const rank = seen.get(app) ?? 0;
            seen.set(app, rank + 1);
            ranks.set(window, {app, rank});
        }
        return ranks;
    }

    _openWindows() {
        return global.display.list_all_windows()
            .filter(w => w.get_window_type() === Meta.WindowType.NORMAL && !w.get_transient_for() && !w.skip_taskbar &&
                !quickNote(w) && appOf(w)?.endsWith('.desktop'))
            .map(window => ({window, app: appOf(window), seq: window.get_stable_sequence()}))
            .sort((a, b) => a.seq - b.seq);
    }

    // ---- Coming back ----

    _changed() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE, () => {
            this._timer = 0;
            this._settle().catch(e => console.warn(`Amethystora UI: desks: ${e.message}`));
            return GLib.SOURCE_REMOVE;
        });
    }

    async _settle() {
        const here = (this._screenSet ?? screenSet)();
        const before = this._cameFrom ?? this._here?.base;
        this._cameFrom = null;
        this._here = here;
        lastBase = here.base;
        this._settled = true;
        await queue;
        const {data} = await readFile();
        // Another change, or the module stopped, while this read
        if (this._here !== here || this._timer)
            return;
        const layout = cleanLayouts(data.layouts)[here.base];
        // Under the tiling, the same set of screens as before is a screen that woke: nothing moved a strip that kept its
        // screen, and putting windows in place again would only shuffle those the tiling holds apart
        const engine = this._ext.modules?.get('tiling')?.engine ?? null;
        if (!layout || (engine && before === here.base))
            return;
        // The note says the desk is back: only when there are screens to be back at, not on the way to the train
        if (this.restore(layout, here) > 0 && before !== here.base && here.external > 0)
            this._showNote();
    }

    // Each window kept and still open goes back; says how many moved. The layout that kept it must be the layout now.
    restore(layout, here) {
        const engine = this._ext.modules?.get('tiling')?.engine ?? null;
        if (layout.mode !== (engine ? 'tiling' : 'classic'))
            return 0;
        const connectorOf = new Map([...here.keys].map(([connector, key]) => [key, connector]));
        const matched = match(layout, this._openWindows());
        return engine ? this._restoreTiling(engine, layout, matched, connectorOf) : this._restoreClassic(matched, connectorOf);
    }

    _restoreTiling(engine, layout, matched, connectorOf) {
        let moved = 0;
        const manager = global.workspace_manager;
        for (const strip of strips(matched)) {
            const monitor = engine.indexOf(connectorOf.get(strip.screen));
            const workspace = strip.workspace < manager.n_workspaces ? manager.get_workspace_by_index(strip.workspace) : null;
            if (monitor >= 0 && workspace)
                moved += engine.arrange(workspace, monitor, strip.windows);
        }
        for (const [screen, index] of Object.entries(layout.shown ?? {})) {
            const monitor = engine.indexOf(connectorOf.get(screen));
            const workspace = index < manager.n_workspaces ? manager.get_workspace_by_index(index) : null;
            if (monitor >= 0 && workspace)
                engine.showOn(workspace, monitor);
        }
        return moved;
    }

    _restoreClassic(matched, connectorOf) {
        let moved = 0;
        const list = screens();
        const manager = global.workspace_manager;
        for (const {entry, window} of matched) {
            const m = connectorOf.get(entry.screen);
            const rect = m ? clampPlace({m, x: entry.x, y: entry.y, w: entry.w, h: entry.h, max: entry.max}, list) : null;
            if (!rect || window.is_fullscreen())
                continue;
            let changed = false;
            if (!window.is_on_all_workspaces() && entry.workspace < manager.n_workspaces &&
                window.get_workspace()?.index() !== entry.workspace) {
                window.change_workspace_by_index(entry.workspace, false);
                changed = true;
            }
            const f = window.get_frame_rect();
            const maximized = window.maximized_horizontally && window.maximized_vertically;
            const number = list.find(s => s.connector === m)?.number;
            if (rect.max ? !maximized || window.get_monitor() !== number
                : maximized || Math.abs(f.x - rect.x) > 2 || Math.abs(f.y - rect.y) > 2 ||
                    Math.abs(f.width - rect.width) > 2 || Math.abs(f.height - rect.height) > 2) {
                if (maximized)
                    window.unmaximize();
                // The place first: a size below the window's smallest makes Mutter take nothing of the request
                window.move_frame(false, rect.x, rect.y);
                window.move_resize_frame(false, rect.x, rect.y, rect.width, rect.height);
                if (rect.max)
                    window.maximize();
                changed = true;
            }
            if (changed)
                moved++;
        }
        return moved;
    }

    // ---- What it says ----

    _showNote() {
        this._closeNote();
        const note = new St.BoxLayout({style_class: 'amethystora-desks-toast amethystora-desks-note-card',
            orientation: Clutter.Orientation.VERTICAL});
        note.accessible_role = Atk.Role.NOTIFICATION;
        const title = _('Back at your desk');
        note.accessible_name = title;
        const head = new St.Label({style_class: 'amethystora-desks-title', text: title});
        head.clutter_text.use_markup = false;
        note.add_child(head);
        const words = new St.Label({style_class: 'amethystora-desks-note', text: _('Every window is where you left it.')});
        words.clutter_text.use_markup = false;
        note.add_child(words);
        Main.layoutManager.addTopChrome(note);
        const area = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
        const [, width] = note.get_preferred_width(-1);
        const [, height] = note.get_preferred_height(width);
        note.set_position(Math.round(area.x + (area.width - width) / 2), Math.round(area.y + area.height - height - 18 *
            St.ThemeContext.get_for_stage(global.stage).scale_factor));
        fadeIn(note);
        this._note = note;
        this._noteTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, NOTE_FOR, () => {
            this._noteTimer = 0;
            this._closeNote();
            return GLib.SOURCE_REMOVE;
        });
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    _closeNote() {
        if (this._noteTimer)
            GLib.source_remove(this._noteTimer);
        this._noteTimer = 0;
        const had = !!this._note;
        this._note?.destroy();
        this._note = null;
        if (had)
            this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    glassSurfaces() {
        return this._note ? [{actor: this._note, material: 'glass', radius: null}] : [];
    }

    // ---- For the harness ----

    state() {
        return {here: this._here?.base ?? null, settled: this._settled, timer: !!this._timer, note: !!this._note};
    }

    capture() {
        return cleanLayout(this._capture());
    }
}
