// The shelf (round 4, R4-08; plans/beyond/spec-r4-shelf.md): drag files, text or a picture toward the right edge of the
// screen and put them down there, go where they belong, and drag them out again. Offered, and off for a new account.
//
// The shelf itself is a small window of the Amethystora apps (amethystora-shelf), because the shell cannot be the
// source of a drag into a Wayland app; GTK can. This module only does what the window cannot:
//   - while a drag is on its way, a glow at the edge (`edge`) or, after a shake of the pointer, a card under it
//     (`shake`), where the first drop is let go; it reads the drop while the drag hovers it (shelf/drop.js) and hands it
//     to the program on its standard input
//   - the window above every other and on every workspace, at its place: 10 px in from the edge and centred on the
//     screen's height, or as a row centred on where a shake dropped
//   - the keyboard given back to the window that had it when the shelf first takes it
// Later drops land on the window itself.
//
// Off (ame desktop shelf off, the image's default), the module holds its settings and one handler. On, with no drag,
// two handlers on GNOME's drag and one on new windows; no timer. A drag adds the glow and a drag monitor until it ends.
//
// For the suites and the harness: state(), owns(window), glow, card.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as DND from 'resource:///org/gnome/shell/ui/dnd.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {ShakeDetector, STEP} from './pointer/shake.js';
import {handOff, readDrag} from './shelf/drop.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.shelf';
const KEY = 'shelf';
export const APP_ID = 'org.amethystora.Shelf';
// The board's glow: 26 px wide, from 120/560 of the screen's height to 440/560
const GLOW_WIDTH = 26;
const GLOW_INSET = 120 / 560;
const GLOW_OPACITY = 200;
const GLOW_FADE = 150;
// The window 10 px in from the screen's edge
const EDGE_GAP = 10;
// A shake's card goes once the pointer has been away from it this long, and longer before the pointer first reached
// it: the hand is still shaking when it shows
const CARD_AWAY = 1000;
const CARD_FIRST = 3000;
// The keyboard is given back if the shelf takes it within this long of its first frame
const FOCUS_BACK = 1000;

// A shelf window may be given a drop that came for it a moment ago, never an older one
const DROP_FRESH = 5 * 1000 * 1000;
// The shelf's program: the app's own path, a hard link to the one program of the Amethystora apps
const PROGRAMS = ['/usr/lib/amethystora-shelf/amethystora-shelf', '/usr/lib/amethystora-apps/amethystora-apps'];
// The programs this module started, by process id, kept while GNOME Shell runs (a few at most)
const started = new Set();

// Whether a window is the shelf's. Any app can give its window the shelf's id, so the id alone is not enough: the
// window's process is also one this module started, or runs the shelf's own program outside any sandbox. Only then is
// a window kept above the others, on every workspace, and out of the dock.
export function isShelfWindow(window) {
    if (!window || (window.get_gtk_application_id?.() !== APP_ID && window.get_wm_class?.() !== APP_ID))
        return false;
    const pid = window.get_pid?.() ?? -1;
    if (window.get_sandboxed_app_id?.() || pid <= 0)
        return false;
    if (started.has(pid))
        return true;
    try {
        return PROGRAMS.includes(GLib.file_read_link(`/proc/${pid}/exe`));
    } catch {
        return false;
    }
}

export class Shelf {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject(`changed::${KEY}`, () => this._sync(), this);
        this._mode = 'off';
        this._windows = new Map();
        this._pending = new Set();
        this._sync();
    }

    disable() {
        this._stop();
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        this._windows = null;
        this._pending = null;
    }

    _sync() {
        const mode = this._settings.get_string(KEY);
        if (mode === this._mode)
            return;
        this._stop();
        if (mode === 'edge' || mode === 'shake')
            this._start(mode);
    }

    _start(mode) {
        this._mode = mode;
        Main.xdndHandler.connectObject(
            'drag-begin', () => this._begin(),
            'drag-end', () => this._end(), this);
        // The work area changes when the top bar or a dock that keeps its room comes or goes: the shelf follows it
        global.display.connectObject(
            'window-created', (d, window) => this._created(window),
            'workareas-changed', () => this._placeAll(), this);
        Main.layoutManager.connectObject('monitors-changed', () => this._placeAll(), this);
        // A shelf already open (the screen was locked, or the module started again) is kept at its place
        for (const actor of global.get_window_actors()) {
            const window = actor.meta_window;
            if (isShelfWindow(window))
                this._adopt(window, {fresh: false});
        }
    }

    _stop() {
        this._mode = 'off';
        this._fromShelf = false;
        this._dropAt = null;
        Main.xdndHandler.disconnectObject(this);
        global.display.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        this._forgetDrag();
        for (const window of [...(this._windows?.keys() ?? [])])
            this._release(window);
        for (const pending of [...(this._pending ?? [])])
            this._unwatch(pending);
    }

    // ---- A drag on its way ----

    _begin() {
        this._forgetDrag();
        // A drag that starts on the shelf is an item dragged out, which is only ever a copy (fromShelf()): the press on
        // the item gave the shelf the keyboard, and the pointer may have left it before the drag is said to begin
        const [x, y] = global.get_pointer();
        this._fromShelf = this._windows.has(global.display.focus_window) ||
            [...this._windows.keys()].some(w => this._inside(w.get_compositor_private(), x, y));
        // With the shelf open, the drag goes straight onto it
        if (this._openShelf())
            return;
        this._drag = {
            cancellable: new Gio.Cancellable(),
            read: null,
            over: false,
            last: [x, y],
            shake: this._mode === 'shake' ? new ShakeDetector() : null,
            sampled: 0,
        };
        if (this._mode === 'edge')
            this._showGlow(this._monitorAt(x, y));
        this._monitor = {dragMotion: event => this._motion(event)};
        DND.addDragMonitor(this._monitor);
    }

    _motion(event) {
        const drag = this._drag;
        if (!drag)
            return DND.DragMotionResult.CONTINUE;
        drag.last = [event.x, event.y];
        if (this._mode === 'edge') {
            const monitor = this._monitorAt(event.x, event.y);
            if (this._glow && monitor !== this._glowMonitor)
                this._placeGlow(monitor);
            this._hover(this._inside(this._glow, event.x, event.y));
        } else if (drag.shake) {
            const now = GLib.get_monotonic_time() / 1000;
            if (!this._card && now - drag.sampled >= STEP) {
                drag.sampled = now;
                if (drag.shake.sample(event.x, now))
                    this._showCard(event.x, event.y);
            }
            if (this._card) {
                const over = this._inside(this._card, event.x, event.y);
                this._hover(over);
                this._awayTimer(!over);
            }
        }
        return DND.DragMotionResult.CONTINUE;
    }

    // The drag over the glow or the card: brighter, and its data read once, while the drag still holds it
    _hover(over) {
        const drag = this._drag;
        if (!drag || drag.over === over)
            return;
        drag.over = over;
        if (over)
            drag.reached = true;
        const target = this._glow ?? this._card;
        target?.ease({opacity: over ? 255 : GLOW_OPACITY, duration: GLOW_FADE, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        if (over && !drag.read)
            drag.read = readDrag(drag.cancellable).catch(() => null);
    }

    async _end() {
        const drag = this._drag;
        if (this._monitor)
            DND.removeDragMonitor(this._monitor);
        this._monitor = null;
        const released = (global.get_pointer()[2] & Clutter.ModifierType.BUTTON1_MASK) === 0;
        const target = this._glow ?? this._card;
        const dropped = drag && released && drag.read && this._inside(target, ...drag.last);
        const at = drag ? {x: drag.last[0], y: drag.last[1], monitor: this._monitorAt(...drag.last)} : null;
        const layout = this._mode === 'shake' ? 'row' : 'column';
        this._hideTargets();
        if (!dropped) {
            this._forgetDrag();
            return;
        }
        const read = drag.read;
        this._drag = null;
        const drop = await read;
        if (!drop || this._mode === 'off')
            return;
        // Where the window goes when it comes, if it comes soon
        this._dropAt = {...at, layout, time: GLib.get_monotonic_time()};
        const pid = await handOff(drop, layout);
        if (pid > 0) {
            started.add(pid);
            if (started.size > 8)
                started.delete(started.values().next().value);
        }
    }

    _forgetDrag() {
        if (this._monitor)
            DND.removeDragMonitor(this._monitor);
        this._monitor = null;
        this._drag?.cancellable.cancel();
        this._drag = null;
        this._hideTargets();
    }

    _hideTargets() {
        this._glow?.destroy();
        this._glow = null;
        this._glowMonitor = null;
        if (this._away)
            GLib.source_remove(this._away);
        this._away = 0;
        const hadCard = !!this._card;
        this._card?.destroy();
        this._card = null;
        if (hadCard)
            this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    _monitorAt(x, y) {
        const index = Main.layoutManager.monitors.findIndex(m => x >= m.x && x < m.x + m.width &&
            y >= m.y && y < m.y + m.height);
        return index >= 0 ? index : Main.layoutManager.primaryIndex;
    }

    _inside(actor, x, y) {
        if (!actor)
            return false;
        const [ax, ay] = actor.get_transformed_position();
        const [w, h] = actor.get_transformed_size();
        return x >= ax && x < ax + w && y >= ay && y < ay + h;
    }

    _rtl() {
        return Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
    }

    // ---- The glow (edge) ----

    _showGlow(monitor) {
        const glow = new St.Widget({
            style_class: 'amethystora-shelf-glow',
            reactive: true,
            opacity: 0,
        });
        if (this._rtl())
            glow.add_style_class_name('amethystora-shelf-glow-start');
        // A drag over it is the shell's: no window under it gets the drop, and the pointer says copy
        glow._delegate = {
            handleDragOver: source => source === Main.xdndHandler
                ? DND.DragMotionResult.COPY_DROP : DND.DragMotionResult.CONTINUE,
        };
        Main.layoutManager.addTopChrome(glow);
        this._glow = glow;
        this._placeGlow(monitor);
        glow.ease({opacity: GLOW_OPACITY, duration: GLOW_FADE, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
    }

    _placeGlow(monitor) {
        const geometry = Main.layoutManager.monitors[monitor];
        if (!this._glow || !geometry)
            return;
        this._glowMonitor = monitor;
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const width = GLOW_WIDTH * scale;
        const top = Math.round(geometry.height * GLOW_INSET);
        const x = this._rtl() ? geometry.x : geometry.x + geometry.width - width;
        this._glow.set_position(x, geometry.y + top);
        this._glow.set_size(width, geometry.height - 2 * top);
    }

    // ---- The card (shake) ----

    _showCard(x, y) {
        const card = new St.Label({
            style_class: 'amethystora-shelf-card',
            text: _('Drop here to put it down'),
            reactive: true,
            opacity: 0,
        });
        card.clutter_text.use_markup = false;
        card._delegate = {
            handleDragOver: source => source === Main.xdndHandler
                ? DND.DragMotionResult.COPY_DROP : DND.DragMotionResult.CONTINUE,
        };
        Main.layoutManager.addTopChrome(card);
        this._card = card;
        // Centred on the pointer, so that letting go at once is letting go on it, and kept inside its screen
        const [, width] = card.get_preferred_width(-1);
        const [, height] = card.get_preferred_height(width);
        const area = Main.layoutManager.getWorkAreaForMonitor(this._monitorAt(x, y));
        const left = Math.max(area.x, Math.min(Math.round(x - width / 2), area.x + area.width - width));
        const top = Math.max(area.y, Math.min(Math.round(y - height / 2), area.y + area.height - height));
        card.set_position(left, top);
        card.ease({opacity: GLOW_OPACITY, duration: GLOW_FADE, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    // Away from the card for a moment, it goes, and the next shake brings it back
    _awayTimer(away) {
        if (!away) {
            if (this._away)
                GLib.source_remove(this._away);
            this._away = 0;
            return;
        }
        if (this._away)
            return;
        this._away = GLib.timeout_add(GLib.PRIORITY_DEFAULT, this._drag?.reached ? CARD_AWAY : CARD_FIRST, () => {
            this._away = 0;
            if (this._drag) {
                this._drag.over = false;
                this._drag.reached = false;
                this._drag.shake?.reset();
            }
            this._card?.destroy();
            this._card = null;
            this._ext.modules?.get('glass')?.surfacesChanged?.();
            return GLib.SOURCE_REMOVE;
        });
    }

    glassSurfaces() {
        return this._card ? [{actor: this._card, material: 'glass', radius: null}] : [];
    }

    // ---- The window ----

    // The shelf's window open and shown: a drag then goes straight onto it
    _openShelf() {
        return [...this._windows.keys()].some(window => !window.minimized);
    }

    _created(window) {
        if (isShelfWindow(window)) {
            this._adopt(window, {fresh: true});
            return;
        }
        // A Wayland window says its id just after it is made: asked again then, once
        const pending = {window};
        const check = () => {
            this._unwatch(pending);
            if (this._mode !== 'off' && this._windows && !this._windows.has(window) && isShelfWindow(window))
                this._adopt(window, {fresh: true});
        };
        window.connectObject(
            'notify::wm-class', check,
            'notify::gtk-application-id', check,
            'unmanaged', () => this._unwatch(pending), pending);
        this._pending.add(pending);
    }

    _unwatch(pending) {
        pending.window.disconnectObject(pending);
        this._pending?.delete(pending);
    }

    _adopt(window, {fresh}) {
        if (this._windows.has(window))
            return;
        const drop = fresh && GLib.get_monotonic_time() - (this._dropAt?.time ?? 0) < DROP_FRESH ? this._dropAt : null;
        if (fresh)
            this._dropAt = null;
        // A shelf open already (after the lock screen) stays where it is: a row where it was, a column at its edge
        const frame = window.get_frame_rect();
        const row = drop ? drop.layout === 'row' : !fresh && frame.width > frame.height;
        const record = {
            layout: row ? 'row' : 'column',
            monitor: drop?.monitor ?? window.get_monitor(),
            x: drop?.x ?? (row ? frame.x + frame.width / 2 : null),
            y: drop?.y ?? (row ? frame.y + frame.height / 2 : null),
            // The window that had the keyboard, given it back when the shelf takes it
            before: fresh && !isShelfWindow(global.display.focus_window) ? global.display.focus_window : null,
            focusTimer: 0,
        };
        this._windows.set(window, record);
        // On every workspace at once, before the tiling takes it in (a window on every workspace never joins a strip,
        // tiling/engine.js tileable()); above the others once Mutter has it in its stack, when it is shown
        if (!window.is_on_all_workspaces())
            window.stick();
        const above = () => {
            if (this._windows?.get(window) === record && !window.is_above())
                window.make_above();
        };
        if (!fresh)
            above();
        window.connectObject(
            'size-changed', () => this._place(window),
            'unmanaged', () => this._release(window), this);
        // Above, placed again and the keyboard given back once it is first shown, once
        if (fresh) {
            window.connectObject('shown', () => {
                window.disconnectObject(record);
                above();
                this._place(window);
                this._giveBackFocus(window);
            }, record);
        }
        this._place(window);
    }

    _release(window) {
        const record = this._windows?.get(window);
        if (!record)
            return;
        if (record.focusTimer)
            GLib.source_remove(record.focusTimer);
        global.display.disconnectObject(record);
        window.disconnectObject(record);
        window.disconnectObject(this);
        this._windows.delete(window);
    }

    _giveBackFocus(window) {
        const record = this._windows.get(window);
        const before = record?.before;
        if (!before || record.focusTimer)
            return;
        const back = () => {
            if (global.display.focus_window === window && before.get_compositor_private())
                before.activate(global.get_current_time());
        };
        back();
        global.display.connectObject('notify::focus-window', back, record);
        record.focusTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, FOCUS_BACK, () => {
            record.focusTimer = 0;
            record.before = null;
            global.display.disconnectObject(record);
            return GLib.SOURCE_REMOVE;
        });
    }

    _placeAll() {
        for (const window of this._windows.keys())
            this._place(window);
    }

    // At the edge: 10 px in, centred on the work area's height. As a row: centred on the drop, inside the work area.
    _place(window) {
        const record = this._windows?.get(window);
        if (!record)
            return;
        const monitor = record.monitor < Main.layoutManager.monitors.length ? record.monitor : window.get_monitor();
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor);
        const frame = window.get_frame_rect();
        if (!area || frame.width <= 0 || frame.height <= 0)
            return;
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        let x, y;
        if (record.layout === 'row' && record.x !== null) {
            x = record.x - frame.width / 2;
            y = record.y - frame.height / 2;
        } else {
            x = this._rtl() ? area.x + EDGE_GAP * scale : area.x + area.width - frame.width - EDGE_GAP * scale;
            y = area.y + (area.height - frame.height) / 2;
        }
        x = Math.round(Math.max(area.x, Math.min(x, area.x + area.width - frame.width)));
        y = Math.round(Math.max(area.y, Math.min(y, area.y + area.height - frame.height)));
        if (x !== frame.x || y !== frame.y)
            window.move_frame(true, x, y);
    }

    // ---- For the dock, the suites and the harness ----

    // The dock leaves the shelf out of its running apps
    owns(window) {
        return this._windows?.has(window) ?? false;
    }

    // Whether the drag under way started on the shelf: the dock's stacks then copy rather than move (dock/drops.js),
    // as the shelf promises
    fromShelf() {
        return !!this._fromShelf;
    }

    get glow() {
        return this._glow ?? null;
    }

    get card() {
        return this._card ?? null;
    }

    state() {
        const box = actor => {
            if (!actor)
                return null;
            const [x, y] = actor.get_transformed_position();
            const [width, height] = actor.get_transformed_size();
            return {x, y, width, height, opacity: actor.opacity};
        };
        return {
            mode: this._mode,
            connected: this._mode !== 'off',
            dragging: !!this._drag,
            over: this._drag?.over ?? false,
            reading: !!this._drag?.read,
            glow: box(this._glow),
            card: box(this._card),
            windows: [...(this._windows?.keys() ?? [])].map(window => {
                const frame = window.get_frame_rect();
                return {
                    x: frame.x, y: frame.y, width: frame.width, height: frame.height,
                    sticky: window.is_on_all_workspaces(), above: window.is_above(),
                    layout: this._windows.get(window).layout,
                };
            }),
        };
    }
}
