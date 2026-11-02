// How a window arrives and leaves (the motion's `open-close` key): pop, slide or fade, closing playing the opening
// backwards, in place of GNOME's own for a plain window; dialogs and everything else keep GNOME's. With Reduce motion on
// it fades in place. The durations go through ease(), so the animation speed and GNOME's switch apply.
//
// GNOME's own map and destroy handlers run for every window as they always do, told to skip their effect and handed a
// stand-in for Mutter's object, so that whatever else they do (following a window's type, dimming its parent) is still
// GNOME's; the window is then in GNOME's own set of windows being mapped or destroyed, which GNOME's own ending takes it
// out of when the motion is over, as it does when Mutter cuts an effect short. Nothing runs between windows.

import Clutter from 'gi://Clutter';
import Meta from 'gi://Meta';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {scaleFactor} from './flight.js';

// Logical px the slide travels, and the scale a pop starts from
const SLIDE = 24;
const POP = 0.92;
// Milliseconds before the speed: the opening, the closing, and either one with Reduce motion on
const OPEN = {pop: 200, slide: 220, fade: 180};
const CLOSE = {pop: 160, slide: 160, fade: 140};
const REDUCED = 150;

export const STYLES = ['pop', 'slide', 'fade'];

export class Arrivals {
    // rest(actor, t0): the translation a window rests at when it has arrived (the tiling's strip may draw it off its frame)
    constructor(rest) {
        this._rest = rest;
        this._opening = new Map();
        this._closing = new Set();
    }

    // For the tests: whether a window is arriving or leaving through this
    moving(actor) {
        return this._opening.has(actor) || this._closing.has(actor);
    }

    get count() {
        return this._opening.size + this._closing.size;
    }

    // style: pop, slide or fade; reduced: Reduce motion. Returns false when GNOME's own map is to run instead.
    open(shellwm, actor, style, reduced) {
        if (Main.wm._getAnimationWindowType(actor) !== Meta.WindowType.NORMAL)
            return false;
        const ok = Main.wm._shouldAnimateActor(actor, [Meta.WindowType.NORMAL]);
        Main.wm.skipNextEffect(actor);
        if (!ok) {
            Main.wm._mapWindow(shellwm, actor);
            return true;
        }
        Main.wm._mapWindow({completed_map() {}}, actor);
        Main.wm._mapping.add(actor);
        const kind = reduced ? 'fade' : style;
        const t0 = {x: actor.translation_x, y: actor.translation_y};
        this._opening.set(actor, t0);
        actor.set_pivot_point(0.5, kind === 'slide' ? 1 : 0.5);
        actor.opacity = 0;
        if (kind === 'pop')
            actor.set_scale(POP, POP);
        if (kind === 'slide')
            actor.translation_y = t0.y + SLIDE * scaleFactor();
        actor.show();
        const duration = reduced ? Math.min(REDUCED, OPEN.fade) : OPEN[kind];
        // Never the pop's overshoot on the opacity: past 255 it wraps round to nearly nothing, a flash as it ends
        actor.ease({opacity: 255, duration, mode: Clutter.AnimationMode.EASE_OUT_CUBIC});
        actor.ease({
            scale_x: 1, scale_y: 1, translation_y: t0.y,
            duration,
            mode: kind === 'pop' ? Clutter.AnimationMode.EASE_OUT_BACK : Clutter.AnimationMode.EASE_OUT_CUBIC,
            onStopped: () => this._opened(shellwm, actor),
        });
        return true;
    }

    _opened(shellwm, actor) {
        const t0 = this._opening.get(actor);
        if (!t0)
            return;
        this._opening.delete(actor);
        // GNOME's ending, unless Mutter's cut it short already (GNOME's own handler then ended it)
        Main.wm._mapWindowDone(shellwm, actor);
        const rest = this._rest(actor, t0);
        actor.set_translation(rest.x, rest.y, 0);
    }

    // Returns false when GNOME's own destroy is to run instead
    close(shellwm, actor, style, reduced) {
        if (Main.wm._getAnimationWindowType(actor) !== Meta.WindowType.NORMAL)
            return false;
        const ok = Main.wm._shouldAnimateActor(actor, [Meta.WindowType.NORMAL]);
        Main.wm.skipNextEffect(actor);
        if (!ok) {
            Main.wm._destroyWindow(shellwm, actor);
            return true;
        }
        Main.wm._destroyWindow({completed_destroy() {}}, actor);
        Main.wm._destroying.add(actor);
        this._closing.add(actor);
        const kind = reduced ? 'fade' : style;
        actor.set_pivot_point(0.5, kind === 'slide' ? 1 : 0.5);
        const to = {opacity: 0};
        if (kind === 'pop')
            Object.assign(to, {scale_x: POP, scale_y: POP});
        if (kind === 'slide')
            to.translation_y = actor.translation_y + SLIDE * scaleFactor();
        actor.ease({
            ...to,
            duration: reduced ? Math.min(REDUCED, CLOSE.fade) : CLOSE[kind],
            mode: kind === 'pop' ? Clutter.AnimationMode.EASE_IN_QUAD : Clutter.AnimationMode.EASE_IN_CUBIC,
            onStopped: () => this._closed(shellwm, actor),
        });
        return true;
    }

    _closed(shellwm, actor) {
        if (!this._closing.delete(actor))
            return;
        Main.wm._destroyWindowDone(shellwm, actor);
    }

    // Every motion under way ends where it was going, so that Mutter hears the end of every effect
    stop() {
        const wm = global.window_manager;
        for (const actor of [...this._opening.keys()]) {
            try {
                actor.remove_all_transitions();
                this._opened(wm, actor);
            } catch (e) {
                console.error(`Amethystora UI: motion: a window's opening did not end cleanly: ${e.message}`);
            }
        }
        for (const actor of [...this._closing]) {
            try {
                actor.remove_all_transitions();
                this._closed(wm, actor);
            } catch (e) {
                console.error(`Amethystora UI: motion: a window's closing did not end cleanly: ${e.message}`);
            }
        }
        this._opening.clear();
        this._closing.clear();
    }
}
