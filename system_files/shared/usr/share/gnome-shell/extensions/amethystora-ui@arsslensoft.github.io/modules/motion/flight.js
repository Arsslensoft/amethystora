// One window's way into the dock or back: a timeline on the window's own actor that, every frame, works out the
// drop's place and shape (path.js) and writes it as the actor's scale and translation and the shader's uniforms,
// nothing else. A fade in place, while motion is reduced, writes only the opacity. The flight keeps what it last
// drew, so that one turned round part way starts again from exactly there.

import Clutter from 'gi://Clutter';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {adjustAnimationTime} from 'resource:///org/gnome/shell/misc/animationUtils.js';

import {DropletEffect, LiftedCopy} from './droplet.js';
import {FADE_MS, FLIGHT_MS, IN, OUT, fadeAt, minimizeAt, unminimizeAt} from './path.js';

export const EFFECT = 'amethystora-motion-droplet';

const DROPLETS = new Set(['minimize', 'unminimize', 'open']);

// The centre of a window's frame, where its frame is (no translation)
export function frameCentre(window) {
    const f = window.get_frame_rect();
    return {x: f.x + f.width / 2, y: f.y + f.height / 2};
}

// The window's frame as its actor draws it now, translation aside: the actor's own place and size, less the margins
// round the frame (its shadow). Mutter moves the actor and gives it a new picture a little after the window's
// rectangles change; going by the actor keeps every frame of a flight on what is actually drawn.
// The margins round the frame are read again only when the actor's size changes (cache: an object kept by the caller).
export function measure(actor, cache = null) {
    const [ax, ay] = actor.get_position();
    const [aw, ah] = actor.get_size();
    let margins = cache?.margins;
    if (!margins || cache.aw !== aw || cache.ah !== ah) {
        const window = actor.meta_window;
        const f = window.get_frame_rect();
        const b = window.get_buffer_rect();
        margins = {left: f.x - b.x, top: f.y - b.y, right: b.x + b.width - f.x - f.width,
            bottom: b.y + b.height - f.y - f.height};
        if (cache)
            Object.assign(cache, {aw, ah, margins, frame: null});
    }
    const {left, top} = margins;
    const W = Math.max(1, aw - left - margins.right);
    const H = Math.max(1, ah - top - margins.bottom);
    return {
        W, H, left, top, aw: Math.max(1, aw), ah: Math.max(1, ah),
        x: ax + left, y: ay + top,
        centre: {x: ax + left + W / 2, y: ay + top + H / 2},
    };
}

export class Flight {
    // kind: minimize, unminimize, open, fade-out or fade-in. options:
    //   geo       {W, H, R0, S, Ic}: the frame's size and corners, the icon's size and centre
    //   rest      (m) => {x, y}, the translation the window rests at when it is itself: the one it had (a way out), or
    //             where the tiling's strip draws it now (a way back), read every frame from the frame measured (m)
    //   from      where a flight that turned round left off (path.js turnRound), or null
    //   u0, scale the moment the flight starts at, and a share of its usual time
    //   shader, accent, ripple   the droplet's shader, the theme's accent, and what starts the ring (or null)
    //   onEnd     (flight) => ..., when it has run to its end
    constructor(kind, actor, options) {
        this.kind = kind;
        this.actor = actor;
        this.window = actor.meta_window;
        this._options = options;
        this._geo = options.geo;
        this._from = options.from ?? null;
        this._u0 = options.u0 ?? 0;
        this._rest = options.rest;
        this._rippled = !options.ripple;
        this.timeline = null;
        this.effect = null;
        this._copy = null;
        this.last = null;
        // What measure() keeps between frames, and the pivot last set
        this._measured = {};
        this._pivot = null;
        // Set when the window's actor is destroyed: nothing may touch it after that
        this.gone = false;
        actor.connectObject('destroy', () => {
            this.gone = true;
            options.onGone?.(this);
        }, this);
    }

    // Lets go of the window's actor, once the window manager has had its ending
    detach() {
        if (!this.gone)
            this.actor.disconnectObject(this);
    }

    get droplet() {
        return DROPLETS.has(this.kind);
    }

    start() {
        const actor = this.actor;
        if (this.droplet) {
            this.effect = new DropletEffect(this._options.shader, this._options.accent);
            actor.add_effect_with_name(EFFECT, this.effect);
        }
        const base = this.droplet ? FLIGHT_MS : FADE_MS;
        const duration = Math.max(1, Math.round(adjustAnimationTime(base * (1 - this._u0) * (this._options.scale ?? 1))));
        this.step(this._u0);
        if (this.kind !== 'minimize' && this.kind !== 'fade-out')
            actor.show();
        // A window that is not on the screen has no frames to fly in: it is where it was going at once
        if (!actor.mapped) {
            this.step(1);
            this._options.onEnd?.(this);
            return;
        }
        this.timeline = new Clutter.Timeline({actor, duration});
        this.timeline.connect('new-frame', () => this.step(this._at()));
        this.timeline.connect('completed', () => {
            if (this.gone)
                return;
            this.step(1);
            this._options.onEnd?.(this);
        });
        this.timeline.start();
    }

    _at() {
        return this._u0 + (1 - this._u0) * this.timeline.get_progress();
    }

    // The moment u of the flight, drawn
    step(u) {
        const actor = this.actor;
        const m = measure(actor, this._measured);
        const rest = this._rest(m);
        const fc = m.centre;
        const centre = {x: fc.x + rest.x, y: fc.y + rest.y};
        if (!this.droplet) {
            const out = this.kind === 'fade-out';
            const o = fadeAt(u <= this._u0 ? 0 : (u - this._u0) / (1 - this._u0), out, this._from?.o ?? (out ? 1 : 0));
            actor.opacity = Math.round(255 * o);
            this.last = {kind: this.kind, u, centre, w: this._geo.W, h: this._geo.H, r: this._geo.R0, g: 0, o};
            return;
        }
        // The window may change size on its way back (the tiling gives it the strip's height): the drop opens into
        // the size it has now. Scaled about the frame's centre.
        const geo = {...this._geo, W: m.W, H: m.H, Fc: centre};
        const state = this.kind === 'minimize' ? minimizeAt(u, geo, this._from) : unminimizeAt(u, geo, this._from);
        // Each write costs a call into Clutter and its notifications: only what changed is written
        const px = (m.left + m.W / 2) / m.aw;
        const py = (m.top + m.H / 2) / m.ah;
        if (this._pivot?.[0] !== px || this._pivot?.[1] !== py) {
            actor.set_pivot_point(px, py);
            this._pivot = [px, py];
        }
        actor.set_scale(state.w / m.W, state.h / m.H);
        actor.set_translation(state.centre.x - fc.x, state.centre.y - fc.y, 0);
        this.effect.state = state;
        this._measured.frame ??= [m.left / m.aw, m.top / m.ah, (m.left + m.W) / m.aw, (m.top + m.H) / m.ah];
        this.effect.frame = this._measured.frame;
        // A drop is drawn above the dock and the top bar, where it lands or leaves from; the window, at either end,
        // in its own layer. On the way back it goes down to its layer as soon as it is clear of them, before it opens
        // out over them.
        if (this.kind === 'minimize')
            this._lift(u >= OUT.circle);
        else
            this._lift(u < 1 && !this._clear(u, state));
        // A frame where the box did not change would not be drawn again by itself
        actor.queue_redraw();
        this.last = {kind: this.kind, u, ...state};
        if (!this._rippled && u >= this._options.rippleAt) {
            this._rippled = true;
            this._options.ripple(this._copy);
        }
    }

    // Whether a drop on its way back has left the top bar and the dock behind (then for good)
    _clear(u, state) {
        if (this._cleared)
            return true;
        if (u < IN.shown)
            return false;
        const x1 = state.centre.x - state.w / 2;
        const y1 = state.centre.y - state.h / 2;
        const x2 = x1 + state.w;
        const y2 = y1 + state.h;
        this._cleared = !(this._options.chrome ?? []).some(c => x1 < c.x2 && x2 > c.x1 && y1 < c.y2 && y2 > c.y1);
        return this._cleared;
    }

    _lift(on) {
        const actor = this.actor;
        if (on && !this._copy) {
            this._copy = new LiftedCopy(this.effect, actor);
            // Over the windows, the top bar and the dock; under the shell's top layer (apps' popups, prompts, the
            // screenshot tool)
            Main.layoutManager.uiGroup.insert_child_below(this._copy, global.top_window_group);
        } else if (!on && this._copy) {
            this._copy.destroy();
            this._copy = null;
        }
        this.effect.lifted = on;
        const copy = this._copy;
        if (!copy)
            return;
        // Where the window is drawn, there: the windows' layer and the top one both start at the stage's corner
        const m = this._measured;
        const [ax, ay] = actor.get_position();
        if (copy._x !== ax || copy._y !== ay) {
            copy.set_position(ax, ay);
            copy._x = ax;
            copy._y = ay;
        }
        if (copy._aw !== m.aw || copy._ah !== m.ah) {
            copy.set_size(m.aw, m.ah);
            copy._aw = m.aw;
            copy._ah = m.ah;
        }
        if (this._pivot && copy._pivot !== this._pivot) {
            copy.set_pivot_point(...this._pivot);
            copy._pivot = this._pivot;
        }
        copy.set_scale(actor.scale_x, actor.scale_y);
        copy.set_translation(actor.translation_x, actor.translation_y, 0);
    }

    // Stops where it is and takes everything of its own off the window but the transforms, which the window
    // manager's own ending resets; returns what it last drew
    stop() {
        this.timeline?.stop();
        this.timeline = null;
        this._copy?.destroy();
        this._copy = null;
        if (this.effect) {
            if (!this.gone)
                this.actor.remove_effect(this.effect);
            this.effect.release();
            this.effect = null;
        }
        return this.last;
    }

    // The translation the window rests at, for the window manager's ending to leave it there
    restTranslation() {
        return this.gone ? {x: 0, y: 0} : this._rest(measure(this.actor, this._measured));
    }
}

// The size of a stage pixel, in the stylesheet's pixels
export function scaleFactor() {
    return St.ThemeContext.get_for_stage(global.stage).scale_factor;
}
