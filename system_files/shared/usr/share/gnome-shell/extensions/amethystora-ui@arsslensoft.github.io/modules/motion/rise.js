// A window of a saved set rising into its place as it first appears: from a little smaller and a little lower,
// transparent, to itself, about its centre. With Reduce motion on it only fades in. Only the transform and opacity
// of the window's own actor change; the place it rests at is asked each frame, since the tiling's strip may still be
// gliding it there.

import Clutter from 'gi://Clutter';
import Graphene from 'gi://Graphene';

import {adjustAnimationTime} from 'resource:///org/gnome/shell/misc/animationUtils.js';

// The board's: 0.9 and 14 px below, rising over 450 ms on a curve that settles softly, fading in over 350 ms
const FROM_SCALE = 0.9;
const FROM_Y = 14;
const MOVE_MS = 450;
const FADE_MS = 350;
const MOVE_CURVE = [0.2, 0.8, 0.2, 1];
const FADE_CURVE = [0.25, 0.1, 0.25, 1];

function timeline(actor, ms, curve) {
    const t = new Clutter.Timeline({actor, duration: Math.max(1, Math.round(adjustAnimationTime(ms)))});
    t.set_progress_mode(Clutter.AnimationMode.CUBIC_BEZIER);
    t.set_cubic_bezier_progress(new Graphene.Point({x: curve[0], y: curve[1]}),
        new Graphene.Point({x: curve[2], y: curve[3]}));
    return t;
}

export class Rise {
    // rest: () => {x, y}, the translation the window rests at; onEnd: called once, when both parts are over
    constructor(actor, {reduced, rest, onEnd}) {
        this.actor = actor;
        this._reduced = reduced;
        this._rest = rest;
        this._onEnd = onEnd;
        this._move = null;
        this._fade = null;
        this._open = 0;
    }

    start() {
        const actor = this.actor;
        actor.set_pivot_point(0.5, 0.5);
        actor.opacity = 0;
        this._place(this._reduced ? 1 : 0);
        actor.show();
        this._fade = timeline(actor, FADE_MS, FADE_CURVE);
        this._fade.connect('new-frame', t => {
            actor.opacity = Math.round(255 * t.get_progress());
            if (this._reduced)
                this._place(1);
        });
        this._run(this._fade);
        if (!this._reduced) {
            this._move = timeline(actor, MOVE_MS, MOVE_CURVE);
            this._move.connect('new-frame', t => this._place(t.get_progress()));
            this._run(this._move);
        }
    }

    _run(t) {
        this._open++;
        t.connect('completed', () => {
            if (--this._open === 0)
                this._onEnd?.(this);
        });
        t.start();
    }

    // p from 0 (below, smaller) to 1 (in place)
    _place(p) {
        const actor = this.actor;
        const scale = FROM_SCALE + (1 - FROM_SCALE) * p;
        const rest = this._rest();
        actor.set_scale(scale, scale);
        actor.set_translation(rest.x, rest.y + FROM_Y * (1 - p), 0);
    }

    // Stopped where it is; the caller puts the window as GNOME leaves it
    stop() {
        this._onEnd = null;
        for (const t of [this._move, this._fade])
            t?.stop();
        this._move = this._fade = null;
    }

    restTranslation() {
        return this._rest();
    }
}
