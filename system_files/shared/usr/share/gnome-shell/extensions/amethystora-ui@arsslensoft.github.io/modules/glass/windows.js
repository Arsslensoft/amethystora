// Application windows on glass. Every window but the one being used shows the frosted desktop faintly through it:
// a pane of glass cut to its frame, with the window drawn over it at 84 %, and so do the window switchers' pictures
// of it. The window with the keyboard focus, a fullscreen window, and every window in the overview are fully opaque,
// with nothing drawn behind them. A maximized or tiled window's glass has square corners, so that it meets the
// screen's edges. Docks, desktop icons and desktop widgets are left alone; a window that has not said what it is yet
// has glass until it does. This part also tells the top bar when windows move.
//
// A window whose glass nobody can see does not carry it. The effect makes Mutter paint the window in full, and
// everything under it as well, however much of that is covered: Mutter's culling skips an actor with an effect, and
// takes no opaque pixels from it for what lies below. A window wholly behind another window with glass is one such.
// Its glass goes once it has stayed covered for a moment, and comes back in the frame it is seen again.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// The kinds of window that have glass, and are counted as touching the top bar
export const TYPES = new Set([Meta.WindowType.NORMAL, Meta.WindowType.DIALOG, Meta.WindowType.MODAL_DIALOG]);
// Left alone, by their class: a dock, the desktop icons and desktop widgets
const LEFT_ALONE = new Set(['plank', 'com.desktop.ding', 'conky']);
// The window over its glass, out of 255
const OPACITY = 215;
// How long a window has to stay covered before its glass is taken off, in milliseconds. Giving it back is never
// waited for.
const COVER_DELAY = 300;
// What a window is doing while it comes in, goes out or is carried along: it hides nothing for sure meanwhile
const ANIMATED = ['opacity', 'scale-x', 'scale-y', 'translation-x', 'translation-y'];

// The glass under a window is its frame, inside the actor, which also holds the window's own shadow. A copy of the
// window may be drawn smaller than the window (a switcher's picture), and its frame with it.
const frame = actor => {
    const window = actor.meta_window ?? actor.source?.meta_window;
    if (!window)
        return [];
    const f = window.get_frame_rect();
    const b = window.get_buffer_rect();
    const k = actor.source ? actor.width / Math.max(1, actor.source.width) : 1;
    return [[(f.x - b.x) * k, (f.y - b.y) * k, f.width * k, f.height * k]];
};
const content = actor => Math.round(actor.get_paint_opacity() * OPACITY / 255);

// A pane's corners are round: what lies this far inside its box (and a pixel more, for the edge's softness) is under
// the glass for sure
const inset = radius => radius > 0 ? Math.ceil(radius * (1 - Math.SQRT1_2)) + 1 : 0;

// A window whose class is not known yet is not one of them until it says so; one that says it has none is left alone
export function leftAlone(window) {
    const name = window.get_wm_class();
    return name === '' || LEFT_ALONE.has(name?.toLowerCase());
}

export class Windows {
    constructor(glass) {
        this._glass = glass;
        // Kept while the windows' glass rests (the glass solid), so that the top bar still hears when it is back
        this._listeners = new Set();
    }

    enable() {
        this._windows = new Set();
        this._actors = new Set();
        this._queued = new Set();
        // The windows whose glass is off because they are covered
        this._covered = new Set();
        this._recount = false;
        this._coverTimer = 0;
        this._laterId = 0;
        global.display.connectObject(
            'window-created', (display, window) => this._watch(window),
            'notify::focus-window', () => this._queueAll(),
            'restacked', () => this._changed(),
            this);
        global.workspace_manager.connectObject('active-workspace-changed', () => this._changed(), this);
        Main.layoutManager.connectObject('monitors-changed', () => this._changed(), this);
        for (const actor of global.get_window_actors())
            this._watch(actor.meta_window);
    }

    disable() {
        global.display.disconnectObject(this);
        global.workspace_manager.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        if (this._laterId)
            global.compositor.get_laters().remove(this._laterId);
        this._laterId = 0;
        if (this._coverTimer)
            GLib.source_remove(this._coverTimer);
        this._coverTimer = 0;
        for (const window of this._windows)
            window.disconnectObject(this);
        for (const actor of this._actors) {
            actor.disconnectObject(this);
            this._glass.detach(actor);
        }
        this._windows = this._actors = this._queued = this._covered = null;
    }

    // The desktop style changed the windows' corners
    restyle() {
        for (const window of this._windows ?? [])
            this._sync(window);
    }

    // Called whenever a window is added, goes, moves, changes size, shows or hides, or the workspace changes
    onChange(listener) {
        this._listeners.add(listener);
    }

    get windows() {
        return this._windows;
    }

    // The windows that have no glass because another window with glass hides all of them
    get covered() {
        return this._covered;
    }

    // The look of a copy of a window that has glass, as the workspace switch slides it in
    cloneLook() {
        return {material: 'pane', radius: this._glass.windowRadius, areas: frame, content, inClones: false,
            stageOnly: true};
    }

    // The window switchers' pictures of windows under actor: one of a window that is on glass while it is not in use
    // has glass too, with the corners as much smaller as the picture is
    glassClones(actor) {
        for (const child of actor.get_children()) {
            const window = child instanceof Clutter.Clone ? child.source?.meta_window : null;
            if (!window) {
                this.glassClones(child);
                continue;
            }
            if (!this._windows?.has(window) || !this._wantsGlass(window))
                continue;
            const square = window.maximized_horizontally || window.maximized_vertically;
            const k = child.width / Math.max(1, child.source.width);
            this._glass.attach(child, {...this.cloneLook(), radius: square ? 0 : this._glass.windowRadius * k});
        }
    }

    _watch(window) {
        if (!window || this._windows.has(window))
            return;
        this._windows.add(window);
        const queue = () => this._queue(window);
        window.connectObject(
            'notify::fullscreen', queue,
            'notify::maximized-horizontally', queue,
            'notify::maximized-vertically', queue,
            'notify::wm-class', queue,
            'notify::minimized', () => this._changed(),
            'position-changed', () => this._changed(),
            'size-changed', () => this._changed(),
            'workspace-changed', () => this._changed(),
            'shown', queue,
            // Before the shell begins its farewell to the window, what it covered is in view
            'unmanaging', () => this._uncover(window),
            'unmanaged', () => {
                window.disconnectObject(this);
                this._windows.delete(window);
                this._queued.delete(window);
                this._covered.delete(window);
                this._changed();
            },
            this);
        queue();
        this._changed();
    }

    _queueAll() {
        for (const window of this._windows)
            this._queue(window);
    }

    // Done once before the next frame, however many changes come in
    _queue(window) {
        this._queued.add(window);
        this._recount = true;
        this._schedule();
    }

    _schedule() {
        if (this._laterId)
            return;
        this._laterId = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._laterId = 0;
            for (const queued of this._queued)
                this._sync(queued);
            this._queued.clear();
            if (this._recount) {
                this._recount = false;
                this._cover(false);
            }
            return false;
        });
    }

    _changed() {
        this._recountSoon();
        for (const listener of this._listeners)
            listener();
    }

    // Who covers whom is worked out once before a frame at most, and only when something moved
    // A window on its way into the dock or back moves every frame: what it covers is worked out once it lands (the
    // motion queues the window again then)
    _moved(actor) {
        if (!this._glass._ext.modules?.get('motion')?.flying?.(actor))
            this._recountSoon();
    }

    _recountSoon() {
        this._recount = true;
        this._schedule();
    }

    // ---- Glass nobody can see ----

    // A window of this workspace that is on screen
    _shown(window, active) {
        const actor = window.get_compositor_private();
        return !!actor && actor.visible && actor.mapped && !window.minimized && window.located_on_workspace(active);
    }

    // The part of the screen that a window with glass hides for sure, or null while it hides nothing for sure: its
    // glass is drawn in full there (a finished picture, no animation, no cut edge), inside one screen
    _hides(window, actor) {
        const monitor = Main.layoutManager.monitors[window.get_monitor()];
        if (!monitor || !this._glass.frost?.picture(monitor.index, 'glass')?.ready)
            return null;
        if (actor.has_clip || actor.get_paint_opacity() < 255 || actor.scale_x !== 1 || actor.scale_y !== 1)
            return null;
        const f = window.get_frame_rect();
        const b = window.get_buffer_rect();
        if (f.x < monitor.x || f.y < monitor.y || f.x + f.width > monitor.x + monitor.width ||
            f.y + f.height > monitor.y + monitor.height)
            return null;
        const [x, y] = actor.get_transformed_position();
        const radius = (window.maximized_horizontally || window.maximized_vertically ? 0 : this._glass.windowRadius) *
            St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const edge = inset(radius);
        const left = x + f.x - b.x + edge;
        const top = y + f.y - b.y + edge;
        return {x1: left, y1: top, x2: left + f.width - 2 * edge, y2: top + f.height - 2 * edge};
    }

    // The windows hidden whole behind a window with glass, going down the stack from the top: one is hidden when all
    // of it, its shadow included, lies inside what a window above it that is not itself hidden covers. The window
    // with the keyboard focus covers nothing, since it may be see-through. `busy` is true while a window that might
    // hide another is still on its way in or out.
    _hidden(leaving = null) {
        const hidden = new Set();
        let busy = false;
        const active = global.workspace_manager.get_active_workspace();
        const windows = [...this._windows].filter(w => w !== leaving && this._wantsGlass(w) && this._shown(w, active));
        // The panes are made a moment after the glass starts, or a screen changes: nothing is hidden before then
        const waiting = m => !this._glass.frost?.picture(m.index, 'glass')?.ready;
        if (windows.length && Main.layoutManager.monitors.some(waiting))
            busy = true;
        const hides = [];
        for (const window of global.display.sort_windows_by_stacking(windows).reverse()) {
            const actor = window.get_compositor_private();
            const [seen, box] = actor.get_paint_box();
            if (!seen)
                continue;
            if (hides.some(r => r.x1 <= box.x1 && r.y1 <= box.y1 && box.x2 <= r.x2 && box.y2 <= r.y2)) {
                hidden.add(window);
                continue;
            }
            if (ANIMATED.some(name => actor.get_transition(name))) {
                busy = true;
                continue;
            }
            const rect = this._hides(window, actor);
            if (rect)
                hides.push(rect);
        }
        return {hidden, busy};
    }

    // Puts the glass in line with who covers whom. A window seen again has its glass at once. One newly covered
    // loses it only once it has stayed covered for COVER_DELAY (settled), however often things move meanwhile.
    _cover(settled) {
        if (!this._windows)
            return;
        const {hidden, busy} = this._hidden();
        for (const window of [...this._covered]) {
            if (!hidden.has(window)) {
                this._covered.delete(window);
                this._sync(window);
            }
        }
        if (this._coverTimer)
            GLib.source_remove(this._coverTimer);
        this._coverTimer = 0;
        const fresh = [...hidden].filter(w => !this._covered.has(w));
        if (settled) {
            for (const window of fresh) {
                this._covered.add(window);
                this._sync(window);
            }
        }
        if ((!settled && fresh.length) || busy) {
            this._coverTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, COVER_DELAY, () => {
                this._coverTimer = 0;
                this._cover(true);
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    // A window is going: what it covered is about to be seen
    _uncover(window) {
        if (!this._covered?.size)
            return;
        const {hidden} = this._hidden(window);
        for (const other of [...this._covered]) {
            if (!hidden.has(other)) {
                this._covered.delete(other);
                this._sync(other);
            }
        }
    }

    // A window on its way into the dock or back is a drop of glass of its own (modules/motion.js), with no pane, and one
    // the share guard covers has none (modules/privacy/guard.js)
    _wantsGlass(window) {
        const modules = this._glass._ext.modules;
        return !this._glass.solid && this.glassWithoutFocus(window) && window !== global.display.focus_window &&
            !modules?.get('motion')?.flying?.(window.get_compositor_private()) &&
            !modules?.get('privacy')?.covers?.(window);
    }

    // Whether the window is on glass whenever it does not have the keyboard, as in the tiling's pictures of the
    // workspaces of the other screens (tiling/screens.js), which never have it
    glassWithoutFocus(window) {
        return TYPES.has(window.get_window_type()) && !window.is_override_redirect() &&
            !window.is_attached_dialog() && !window.is_fullscreen() && !leftAlone(window);
    }

    _sync(window) {
        if (!this._windows.has(window))
            return;
        const actor = window.get_compositor_private();
        if (!actor)
            return;
        if (!this._actors.has(actor)) {
            this._actors.add(actor);
            actor.connectObject(
                'notify::visible', () => this._changed(),
                // What the actor shows lags behind what the window says (a client's new picture comes after), and
                // the tiling moves it by translation: what covers what is worked out again when it arrives
                'notify::allocation', () => this._recountSoon(),
                'notify::translation-x', () => this._moved(actor),
                'notify::translation-y', () => this._moved(actor),
                'destroy', () => this._actors?.delete(actor),
                this);
        }
        if (!this._wantsGlass(window) || this._covered.has(window)) {
            this._glass.detach(actor);
            return;
        }
        const square = window.maximized_horizontally || window.maximized_vertically;
        this._glass.attach(actor, {
            material: 'pane',
            radius: square ? 0 : this._glass.windowRadius,
            areas: frame,
            content,
            inClones: false,
            stageOnly: true,
        });
    }
}
