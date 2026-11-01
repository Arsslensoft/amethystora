// What the dock's parts share: the edges and their axes, a layer that never changes a size, durations with GNOME's
// slow-down, and the dock's three ways of moving (full, reduced, none).

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import St from 'gi://St';

// Along which axis the row runs, and which way is away from the screen's edge
export const EDGES = {
    bottom: {long: 'x', short: 'y', vertical: false, away: -1, side: St.Side.BOTTOM},
    left: {long: 'y', short: 'x', vertical: true, away: 1, side: St.Side.LEFT},
    right: {long: 'y', short: 'x', vertical: true, away: -1, side: St.Side.RIGHT},
};

// The board's curves as Clutter's (spec 1.0)
export const EASE = {
    out: Clutter.AnimationMode.EASE_OUT_QUAD,
    inOut: Clutter.AnimationMode.EASE_IN_OUT_QUAD,
    sine: Clutter.AnimationMode.EASE_IN_OUT_SINE,
    quart: Clutter.AnimationMode.EASE_OUT_QUART,
    back: Clutter.AnimationMode.EASE_OUT_BACK,
    linear: Clutter.AnimationMode.LINEAR,
};

// The longest a crossfade is while motion is reduced (the comfort contract)
export const REDUCED_FADE = 150;

// The stage's pixels per logical pixel
export function scaleFactor() {
    return St.ThemeContext.get_for_stage(global.stage).scale_factor;
}

// A duration for a timeline or lag of the dock's own, as fast as GNOME's animations are
export function slowed(ms) {
    return ms * St.Settings.get().slow_down_factor;
}

// A widget that lays its children out where they are put and asks for no room of its own, so that what is added to
// an item (its mark, its number, a ring round its icon) never changes the size of anything around it, and that is
// never picked
export const Layer = GObject.registerClass(
class AmethystoraDockLayer extends St.Widget {
    _init(params = {}) {
        super._init({
            layout_manager: new Clutter.FixedLayout(),
            x_expand: true,
            y_expand: true,
            reactive: false,
            ...params,
        });
    }

    vfunc_get_preferred_width() {
        return [0, 0];
    }

    vfunc_get_preferred_height() {
        return [0, 0];
    }

    // Never under the pointer, not even for a drag, which picks every actor: what is under it is the item
    vfunc_pick(_context) {
    }
});

// Eases actor to props, or sets them at once with animations off. With reduced motion, what moves (translation,
// scale) is set at once and only the opacity fades, in at most REDUCED_FADE.
export function easeTo(actor, props, {level = 'full', duration = 0, mode = EASE.out, delay = 0, onComplete} = {}) {
    if (level === 'none' || duration <= 0) {
        actor.remove_all_transitions();
        actor.set(props);
        onComplete?.();
        return;
    }
    if (level === 'reduced') {
        const {opacity, ...moves} = props;
        actor.set(moves);
        if (opacity === undefined) {
            onComplete?.();
            return;
        }
        actor.ease({opacity, duration: Math.min(duration, REDUCED_FADE), mode: EASE.out, onComplete});
        return;
    }
    actor.ease({...props, duration, mode, delay, onComplete});
}

// A rectangle on the stage from an actor's transformed extents
export function stageRect(actor) {
    const extents = actor.get_transformed_extents();
    return {x: extents.origin.x, y: extents.origin.y, width: extents.size.width, height: extents.size.height};
}

export function inside(rect, x, y, grow = 0) {
    return x >= rect.x - grow && x <= rect.x + rect.width + grow &&
        y >= rect.y - grow && y <= rect.y + rect.height + grow;
}
