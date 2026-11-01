// A frame in the theme's colours (offered, off): in the classic layout the window with the keyboard has a still line
// round it, a gradient from the theme's warm colour through its accent to its cool one, as the tiling's focus frame
// marks the window in use on the strip. Never animated: it is drawn once, with Cairo, when the window it frames, its
// size or the theme changes, and otherwise only moves with the window. None round a maximized, fullscreen or kept-on-
// top window (whose ring says it is in use), nor round the drop-down terminal.

import Cairo from 'cairo';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

// How far it stands out of the frame, how thick, and its corners (the board's)
const OUT = 2;
const LINE = 2;
const RADIUS = 16;
const TYPES = [Meta.WindowType.NORMAL, Meta.WindowType.DIALOG, Meta.WindowType.MODAL_DIALOG];
// The board's orange, purple and teal, until the theme says otherwise
const STAND_INS = [[0xe0, 0x8a, 0x5a], [0x91, 0x48, 0xf0], [0x6f, 0xd6, 0xd0]];
const STOPS = [0, 0.55, 1];
const PROPERTIES = ['-amethystora-frame-start', '-amethystora-frame-middle', '-amethystora-frame-end'];

const scale = () => St.ThemeContext.get_for_stage(global.stage).scale_factor;

export class Frame {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
    }

    enable() {
        this._area = new St.DrawingArea({style_class: 'amethystora-windows-frame', reactive: false, visible: false});
        this._area.connect('repaint', area => this._paint(area));
        this._area.connect('style-changed', () => this._area.queue_repaint());
        global.display.connectObject('notify::focus-window', () => this._follow(), 'restacked', () => this._restack(),
            this);
        this._follow();
    }

    disable() {
        global.display.disconnectObject(this);
        this._unfollow();
        this._area?.destroy();
        this._area = null;
    }

    // For tests
    get area() {
        return this._area ?? null;
    }

    get window() {
        return this._window ?? null;
    }

    _wanted(window) {
        return !!window && TYPES.includes(window.get_window_type()) && !window.is_override_redirect() &&
            !this._module.holds(window) && !!window.get_compositor_private()?.get_parent();
    }

    // The focus moved: the frame goes to the new window, or out of sight
    _follow() {
        const window = global.display.focus_window;
        if (window === this._window)
            return;
        this._unfollow();
        if (!this._wanted(window)) {
            this._area.hide();
            return;
        }
        this._window = window;
        this._actor = window.get_compositor_private();
        const parent = this._actor.get_parent();
        if (this._area.get_parent() !== parent) {
            this._area.get_parent()?.remove_child(this._area);
            parent.add_child(this._area);
        }
        this._placed = null;
        const sync = () => this._sync();
        window.connectObject('position-changed', sync, 'size-changed', sync, 'notify::fullscreen', sync,
            'notify::maximized-horizontally', sync, 'notify::maximized-vertically', sync, 'notify::above', sync,
            'notify::minimized', sync, 'unmanaging', () => this._unfollow(), this);
        this._actor.connectObject('notify::visible', sync, 'effects-completed', sync,
            'notify::translation-x', sync, 'notify::translation-y', sync, this);
        this._restack();
        this._sync();
    }

    _unfollow() {
        this._window?.disconnectObject(this);
        this._actor?.disconnectObject(this);
        this._window = null;
        this._actor = null;
        this._area?.hide();
    }

    _sync() {
        const window = this._window;
        if (!window || !this._area)
            return;
        const shown = this._actor.visible && !window.minimized && !window.is_fullscreen() && !window.is_above() &&
            !(window.maximized_horizontally && window.maximized_vertically) &&
            !this._ext.modules?.get('motion')?.flying?.(this._actor);
        if (!shown) {
            this._area.hide();
            return;
        }
        const out = OUT * scale();
        const f = window.get_frame_rect();
        const [width, height] = [Math.round(f.width + 2 * out), Math.round(f.height + 2 * out)];
        const [x, y] = [Math.round(f.x - out), Math.round(f.y - out)];
        // A new size is painted again; a move only moves what was painted: St paints again whenever an actor's own
        // position changes, so the place is set with the size and a move is a translation from it
        if (!this._placed || this._area.width !== width || this._area.height !== height) {
            this._area.set_position(x, y);
            this._placed = [x, y];
            this._area.set_size(width, height);
        }
        this._area.translation_x = this._actor.translation_x + x - this._placed[0];
        this._area.translation_y = this._actor.translation_y + y - this._placed[1];
        this._area.show();
    }

    // Directly over the window: it lies outside its frame, over only the shadow the app draws there, which would
    // hide most of it from below
    _restack() {
        if (this._actor && this._area?.get_parent() === this._actor.get_parent())
            this._actor.get_parent().set_child_above_sibling(this._area, this._actor);
    }

    _colours(area) {
        const node = area.get_theme_node();
        return PROPERTIES.map((name, i) => {
            const [found, colour] = node.lookup_color(name, false);
            return found ? [colour.red, colour.green, colour.blue] : STAND_INS[i];
        });
    }

    // The line, once: a rounded rectangle stroked with a gradient at 135 degrees (top left to bottom right)
    _paint(area) {
        const t0 = GLib.get_monotonic_time();
        const cr = area.get_context();
        const [width, height] = area.get_surface_size();
        const s = scale();
        const line = LINE * s;
        const half = line / 2;
        const r = Math.max(0, Math.min(RADIUS * s, width / 2, height / 2) - half);
        const gradient = new Cairo.LinearGradient(0, 0, width, height);
        this._colours(area).forEach(([red, green, blue], i) =>
            gradient.addColorStopRGBA(STOPS[i], red / 255, green / 255, blue / 255, 1));
        const [x0, y0, x1, y1] = [half, half, width - half, height - half];
        cr.newSubPath();
        cr.arc(x1 - r, y0 + r, r, -Math.PI / 2, 0);
        cr.arc(x1 - r, y1 - r, r, 0, Math.PI / 2);
        cr.arc(x0 + r, y1 - r, r, Math.PI / 2, Math.PI);
        cr.arc(x0 + r, y0 + r, r, Math.PI, 3 * Math.PI / 2);
        cr.closePath();
        cr.setSource(gradient);
        cr.setLineWidth(line);
        cr.stroke();
        cr.$dispose();
        this.paints = (this.paints ?? 0) + 1;
        this._module.note('frame.paint', t0);
    }
}

