// Keep a copy in view: from a window's menu, the window goes out of sight and a small live copy of it stays in a
// corner, on top of every workspace. It can be dragged to another corner; a click brings the real window back. The
// copy is a clone of the window's own picture, never a copy in memory: an app that stops drawing while out of sight
// shows its last frame. One copy at a time; it goes when the window closes or comes back any other way.

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import Graphene from 'gi://Graphene';
import Meta from 'gi://Meta';
import Mtk from 'gi://Mtk';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {GLIDE, ease, glide} from './util.js';

const WIDTH = 320;
const MAX_HEIGHT = 360;
const INSET = 14;
const DRAG = 8;
const FADE = 300;
const GLIDE_TIME = 420;

// Across a screen lock the extension stops and starts again: the kept window and its corner wait here
let waiting = null;
// The last corner used, for the session
let lastCorner = null;

const scale = () => St.ThemeContext.get_for_stage(global.stage).scale_factor;
const rtl = () => Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;

export class Keep {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
    }

    enable() {
        // Its line in the window menu comes through the module's one hook (menu.js): menuItems() below
        // A window kept before the screen locked comes back as a copy, if it is still out of sight
        if (waiting) {
            const {id, corner} = waiting;
            waiting = null;
            const window = global.get_window_actors().map(a => a.meta_window).find(w => w?.get_id() === id);
            if (window?.minimized)
                this._show(window, corner, false);
        }
    }

    disable() {
        if (this._window && Main.sessionMode.isLocked)
            waiting = {id: this._window.get_id(), corner: this._corner};
        this._giveBack(!Main.sessionMode.isLocked, true);
    }

    get copy() {
        return this._clone ?? null;
    }

    get frame() {
        return this._frame ?? null;
    }

    isKept(window) {
        return !!window && window === this._window;
    }

    // Its line in a window menu GNOME has just built, for the module's hook to place
    menuItems(menu, window) {
        if (window.get_window_type() !== Meta.WindowType.NORMAL || window.is_fullscreen())
            return null;
        return menu.addAction(_('Keep a copy in view'), () => this.keep(window));
    }

    // Keeps a copy of the window in view and puts the window out of sight
    keep(window) {
        if (!window || window.get_window_type() !== Meta.WindowType.NORMAL || window.is_fullscreen())
            return false;
        if (this._window === window)
            return true;
        this._giveBack(true);
        const actor = window.get_compositor_private();
        this._window = window;
        if (actor) {
            actor.set_pivot_point(0.5, 0.5);
            ease(this._ext, actor, {opacity: 0, scale_x: 0.9, scale_y: 0.9}, {
                duration: FADE, mode: Clutter.AnimationMode.EASE,
                onComplete: () => this._minimize(window, actor),
            });
        } else {
            window.minimize();
        }
        this._show(window, lastCorner ?? (rtl() ? 'top-left' : 'top-right'), true);
        return true;
    }

    _minimize(window, actor) {
        if (this._window !== window)
            return;
        window.minimize();
        const reset = () => {
            actor.set({opacity: 255, scale_x: 1, scale_y: 1});
            actor.set_pivot_point(0, 0);
        };
        if (!actor.visible)
            reset();
        else
            actor.connectObject('notify::visible', () => {
                if (actor.visible)
                    return;
                actor.disconnectObject(this._resetOwner);
                reset();
            }, this._resetOwner = {});
    }

    _size(window) {
        const s = scale();
        const f = window.get_frame_rect();
        const aspect = f.height / Math.max(1, f.width);
        let width = WIDTH * s;
        let height = width * aspect;
        // Too tall: shorter, and narrower with it. Too flat for the height's floor: the floor would take a width
        // beyond the copy's, which only ever shrinks, so a very flat window keeps a flat copy (the picture is never
        // shown inside empty space, which the ring would fill)
        if (height > MAX_HEIGHT * s) {
            height = MAX_HEIGHT * s;
            width = height / aspect;
        }
        return [Math.round(width), Math.round(height)];
    }

    // Where the copy sits in a corner of a monitor's work area
    _cornerPosition(monitor, corner, width, height) {
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor);
        const inset = INSET * scale();
        const x = corner.endsWith('left') ? area.x + inset : area.x + area.width - inset - width;
        const y = corner.startsWith('top') ? area.y + inset : area.y + area.height - inset - height;
        return [x, y];
    }

    _show(window, corner, animate) {
        const actor = window.get_compositor_private();
        if (!actor)
            return;
        this._window = window;
        this._corner = corner;
        this._monitor = window.get_monitor();
        const [width, height] = this._size(window);
        const f = window.get_frame_rect();
        const b = window.get_buffer_rect();
        // The picture fits the frame, centred in it when the window is flatter than the frame allows
        const k = Math.min(width / Math.max(1, f.width), height / Math.max(1, f.height));
        const [dx, dy] = [(width - f.width * k) / 2, (height - f.height * k) / 2];
        const frame = new St.Widget({
            style_class: 'amethystora-windows-keep', reactive: true, track_hover: true,
            width, height, pivot_point: new Graphene.Point({x: 0.5, y: 0.5}),
            accessible_role: Atk.Role.PUSH_BUTTON,
        });
        // The title is plain text in the name the screen reader says
        frame.accessible_name = _('Copy of {title}').replace('{title}', () => window.get_title() ?? '');
        this._clone = new Clutter.Clone({source: actor, scale_x: k, scale_y: k,
            x: Math.round(dx - (f.x - b.x) * k), y: Math.round(dy - (f.y - b.y) * k)});
        // The ring and the shadow are drawn outside the frame, which a clip would cut: the picture alone is clipped
        const ring = new St.Widget({style_class: 'amethystora-windows-keep-ring', width, height});
        const clip = new St.Widget({clip_to_allocation: true, width, height});
        clip.add_child(this._clone);
        ring.add_child(clip);
        frame.add_child(ring);
        this._frame = frame;
        Main.layoutManager.addChrome(frame, {trackFullscreen: true});
        const [x, y] = this._cornerPosition(this._monitor, corner, width, height);
        frame.set_position(x, y);

        frame.connectObject('button-press-event', (a, event) => this._press(event), this);
        // A screen unplugged: the copy goes to its corner on the screen the window is on now
        Main.layoutManager.connectObject('monitors-changed', () => {
            const monitor = Math.min(this._window?.get_monitor() ?? 0, Main.layoutManager.monitors.length - 1);
            this._moveTo(Math.max(0, monitor), this._corner);
        }, this);
        window.connectObject('unmanaging', () => this._giveBack(false),
            'notify::minimized', () => {
                if (!window.minimized)
                    this._giveBack(false);
            }, this);
        Main.overview.connectObject('showing', () => frame.hide(), 'hidden', () => frame.show(), this);
        if (Main.overview.visible)
            frame.hide();
        if (animate) {
            frame.opacity = 0;
            frame.set_scale(0.6, 0.6);
            ease(this._ext, frame, {opacity: 255, scale_x: 1, scale_y: 1}, {duration: FADE});
        }
    }

    // A press on the copy: a drag moves it to another corner, a click brings the window back. The pointer is the
    // copy's only while the button is down; a system dialog opening meanwhile ends the drag where it is.
    _press(event) {
        if (event.get_button() !== Clutter.BUTTON_PRIMARY || this._grab)
            return Clutter.EVENT_PROPAGATE;
        const [x, y] = event.get_coords();
        this._start = [x, y];
        this._dragging = false;
        this._glide?.stop();
        this._glide = null;
        this._grab = global.stage.grab(this._frame);
        this._frame.connectObject('motion-event', (a, e) => this._motion(e),
            'button-release-event', (a, e) => this._release(e), this._pressOwner = {});
        Main.layoutManager.connectObject('system-modal-opened', () => this._endPress(null), this._pressOwner);
        return Clutter.EVENT_STOP;
    }

    _motion(event) {
        const [x, y] = event.get_coords();
        const [sx, sy] = this._start;
        if (!this._dragging && Math.hypot(x - sx, y - sy) > DRAG * scale())
            this._dragging = true;
        if (this._dragging) {
            this._frame.translation_x = x - sx;
            this._frame.translation_y = y - sy;
        }
        return Clutter.EVENT_STOP;
    }

    _release(event) {
        const [x, y] = event.get_coords();
        this._endPress({x, y});
        return Clutter.EVENT_STOP;
    }

    // The press ends: a click gives the window back, a drag settles where it was let go (or, with no point, in its
    // own corner again)
    _endPress(point) {
        if (this._pressOwner) {
            this._frame?.disconnectObject(this._pressOwner);
            Main.layoutManager.disconnectObject(this._pressOwner);
        }
        this._pressOwner = null;
        this._grab?.dismiss();
        this._grab = null;
        if (!this._dragging && point) {
            const window = this._window;
            this._giveBack(false);
            if (window)
                Main.activateWindow(window);
            return;
        }
        this._dragging = false;
        this._settle(point);
    }

    // To the nearest corner of the screen where it was let go, or its own corner again
    _settle(point) {
        const frame = this._frame;
        if (!frame)
            return;
        let corner = this._corner;
        let monitor = this._monitor;
        if (point) {
            monitor = global.display.get_monitor_index_for_rect(new Mtk.Rectangle({x: Math.round(point.x),
                y: Math.round(point.y), width: 1, height: 1}));
            const m = Main.layoutManager.monitors[monitor];
            corner = `${point.y < m.y + m.height / 2 ? 'top' : 'bottom'}-${point.x < m.x + m.width / 2 ? 'left' : 'right'}`;
            lastCorner = corner;
        }
        this._moveTo(monitor, corner);
    }

    // Glides the copy into a corner of a monitor, from wherever it is drawn now
    _moveTo(monitor, corner) {
        const frame = this._frame;
        const [fromX, fromY] = [frame.x + frame.translation_x, frame.y + frame.translation_y];
        const [toX, toY] = this._cornerPosition(monitor, corner, frame.width, frame.height);
        frame.set_position(fromX, fromY);
        frame.set_translation(0, 0, 0);
        this._corner = corner;
        this._monitor = monitor;
        this._glide?.stop();
        this._glide = glide(frame, GLIDE_TIME, GLIDE, p => {
            frame.set_position(Math.round(fromX + (toX - fromX) * p), Math.round(fromY + (toY - fromY) * p));
        }, () => (this._glide = null));
    }

    // The copy goes; the window comes back out of sight's way, unless it closed or came back already. now: at once
    _giveBack(unminimize, now = false) {
        const window = this._window;
        const frame = this._frame;
        this._window = null;
        this._frame = null;
        this._clone = null;
        this._glide?.stop();
        this._glide = null;
        if (this._pressOwner) {
            frame?.disconnectObject(this._pressOwner);
            Main.layoutManager.disconnectObject(this._pressOwner);
        }
        this._pressOwner = null;
        this._grab?.dismiss();
        this._grab = null;
        window?.disconnectObject(this);
        Main.overview.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        if (this._resetOwner)
            window?.get_compositor_private()?.disconnectObject(this._resetOwner);
        this._resetOwner = null;
        // A window still fading out when it was given back is drawn whole again
        const actor = window?.get_compositor_private();
        if (actor && !window.minimized) {
            actor.remove_all_transitions();
            actor.set({opacity: 255, scale_x: 1, scale_y: 1});
        }
        if (unminimize && window?.minimized)
            window.unminimize();
        if (!frame)
            return;
        frame.disconnectObject(this);
        const destroy = () => {
            Main.layoutManager.removeChrome(frame);
            frame.destroy();
        };
        if (now) {
            destroy();
            return;
        }
        frame.reactive = false;
        ease(this._ext, frame, {opacity: 0, scale_x: 0.6, scale_y: 0.6}, {duration: FADE, onComplete: destroy});
    }
}
