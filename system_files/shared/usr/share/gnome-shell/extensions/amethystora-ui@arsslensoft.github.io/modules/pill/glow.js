// The recording glow: while the screen is recorded or shared, a thin red line runs round every screen's edges with a
// glow inside it, over everything, fullscreen windows included, so that nobody forgets it is on. It breathes for its
// first three beats and then stays still. It takes no input, and it shows in a recording of the whole screen: the
// shell cannot leave one of its own pieces out of what it records.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const NAME = 'amethystora-pill-glow';
// The glow's depth inside the line (logical pixels), and its timings in ms: in, out, and one breath
const DEPTH = 26;
const FADE = 300;
const BREATH = 2400;
const BREATHS = 3;
// The breath's low point: the board's two keyframes' ratio
const LOW = Math.round(0.72 * 255);

// reactive: false only keeps events off an actor. GNOME's drag and drop picks with PickMode.ALL, which sees every mapped
// actor, so a layer the size of the screen would take every drop in the shell. This one, and all in it, is never picked.
const GlowLayer = GObject.registerClass(class AmethystoraPillGlowLayer extends Clutter.Actor {
    vfunc_pick(_context) {
    }
});

export class Glow {
    constructor(capture, reduceMotion) {
        this._capture = capture;
        this._reduce = reduceMotion;
    }

    enable() {
        this._unlisten = this._capture.listen(() => this._sync());
        Main.layoutManager.connectObject('monitors-changed', () => this._rebuild(), this);
        // The glow's depth is in logical pixels: a new scale draws it again
        St.ThemeContext.get_for_stage(global.stage).connectObject('notify::scale-factor', () => this._rebuild(), this);
        this._sync();
    }

    disable() {
        this._unlisten?.();
        this._unlisten = null;
        Main.layoutManager.disconnectObject(this);
        St.ThemeContext.get_for_stage(global.stage).disconnectObject(this);
        this._remove(false);
    }

    get monitors() {
        return this._actor ? this._actor.get_n_children() : 0;
    }

    get breathing() {
        return !!this._actor?.get_transition('opacity') && this._breathing === true;
    }

    _sync() {
        if (this._capture.active)
            this._show();
        else
            this._remove(true);
    }

    _show() {
        if (this._actor && !this._leaving)
            return;
        if (this._actor)
            this._remove(false);
        this._actor = new GlowLayer({name: NAME, reactive: false, opacity: 0});
        this._build();
        Main.uiGroup.add_child(this._actor);
        Main.uiGroup.set_child_above_sibling(this._actor, null);
        // A fullscreen window is drawn past the compositor unless something is drawn over it
        global.compositor.disable_unredirect();
        this._unredirectOff = true;
        this._actor.ease({
            opacity: 255,
            duration: FADE,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onComplete: () => this._breathe(),
        });
    }

    _build() {
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const depth = Math.round(DEPTH * scale);
        for (const monitor of Main.layoutManager.monitors) {
            const frame = new Clutter.Actor({reactive: false});
            frame.set_position(monitor.x, monitor.y);
            frame.set_size(monitor.width, monitor.height);
            const strips = {
                top: [0, 0, monitor.width, depth],
                bottom: [0, monitor.height - depth, monitor.width, depth],
                left: [0, 0, depth, monitor.height],
                right: [monitor.width - depth, 0, depth, monitor.height],
            };
            for (const [side, [x, y, w, h]] of Object.entries(strips)) {
                const strip = new St.Widget({style_class: `amethystora-pill-glow-strip amethystora-pill-glow-${side}`, reactive: false});
                strip.set_position(x, y);
                strip.set_size(w, h);
                frame.add_child(strip);
            }
            this._actor.add_child(frame);
        }
    }

    _rebuild() {
        if (!this._actor || this._leaving)
            return;
        this._actor.destroy_all_children();
        this._build();
    }

    // Three breaths, then still
    _breathe() {
        if (!this._actor || this._leaving || this._reduce())
            return;
        this._breathing = true;
        this._actor.ease({
            opacity: LOW,
            duration: BREATH / 2,
            repeatCount: 2 * BREATHS - 1,
            autoReverse: true,
            mode: Clutter.AnimationMode.EASE_IN_OUT,
            onStopped: () => {
                this._breathing = false;
                if (this._actor && !this._leaving)
                    this._actor.opacity = 255;
            },
        });
    }

    _remove(animate) {
        const actor = this._actor;
        if (!actor)
            return;
        let gone = false;
        const done = () => {
            if (gone)
                return;
            gone = true;
            if (this._actor === actor)
                this._actor = null;
            this._leaving = false;
            actor.destroy();
            if (this._unredirectOff) {
                global.compositor.enable_unredirect();
                this._unredirectOff = false;
            }
        };
        if (!animate) {
            this._leaving = true;
            // A fade out that was running ends here, and finishes the job itself
            actor.remove_all_transitions();
            if (this._actor === actor)
                done();
            return;
        }
        if (this._leaving)
            return;
        this._leaving = true;
        actor.remove_all_transitions();
        actor.ease({
            opacity: 0,
            duration: FADE,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onStopped: done,
        });
    }
}
