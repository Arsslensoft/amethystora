// The pointer while presenting (board 38, Present mode): a halo that follows it, or a torch that dims everything but a
// circle round it. Drawn over every window, a slide show in full screen included (that is where people look for the
// pointer), and always under the lock screen. Each is a few actors moved by translation, so a move of the mouse costs no relayout and no repaint of anything
// but them. The board's third variant, a ripple at every click, is not built: the shell never sees a press that a window
// takes (plans/beyond/spec-r4-capture.md, 0.3).

import Cairo from 'cairo';
import Clutter from 'gi://Clutter';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// The board's sizes: the halo 52 px across, the torch's circle 88 px round with a 4 px soft edge
const HALO = 52;
const SPOT = 88;
const SOFT = 4;
// The torch's shade
const SHADE = [10 / 255, 6 / 255, 18 / 255, 0.62];

// Over every window, a full screen's included, and under the lock screen whether or not the extension stops at a lock
export function addUnderLock(actor) {
    Main.layoutManager.uiGroup.insert_child_below(actor, Main.layoutManager.screenShieldGroup);
}

class Halo {
    constructor() {
        this.actor = new St.Widget({style_class: 'amethystora-present-halo', width: HALO, height: HALO, reactive: false});
        addUnderLock(this.actor);
    }

    moveTo(x, y) {
        this.actor.set_translation(Math.round(x - HALO / 2), Math.round(y - HALO / 2), 0);
    }

    destroy() {
        this.actor.destroy();
    }
}

// The circle is a texture drawn once (the hole, with its soft edge, in a square of shade); four flat panes of the same
// shade fill the rest of the stage around it, each as large as the stage and only moved
class Torch {
    constructor() {
        const side = 2 * (SPOT + SOFT);
        this._side = side;
        this.actor = new Clutter.Actor({name: 'amethystoraTorch', reactive: false, clip_to_allocation: true});
        this.actor.add_constraint(new Clutter.BindConstraint({source: global.stage, coordinate: Clutter.BindCoordinate.SIZE}));
        const [r, g, b, a] = SHADE;
        const colour = `rgba(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)}, ${a})`;
        this._panes = [0, 1, 2, 3].map(() => {
            const pane = new St.Widget({style: `background-color: ${colour};`, reactive: false});
            this.actor.add_child(pane);
            return pane;
        });
        this._hole = new St.DrawingArea({width: side, height: side, reactive: false});
        this._hole.connect('repaint', area => {
            const cr = area.get_context();
            const [w] = area.get_surface_size();
            const scale = w / side;
            cr.scale(scale, scale);
            const gradient = new Cairo.RadialGradient(side / 2, side / 2, SPOT - SOFT / 2, side / 2, side / 2, SPOT + SOFT / 2);
            gradient.addColorStopRGBA(0, r, g, b, 0);
            gradient.addColorStopRGBA(1, r, g, b, a);
            cr.setSource(gradient);
            cr.rectangle(0, 0, side, side);
            cr.fill();
            cr.$dispose();
        });
        this.actor.add_child(this._hole);
        addUnderLock(this.actor);
        this._sizePanes();
        global.stage.connectObject('notify::size', () => this._sizePanes(), this);
    }

    _sizePanes() {
        const [w, h] = [global.stage.width, global.stage.height];
        const side = this._side;
        const [top, bottom, left, right] = this._panes;
        top.set_size(2 * w + side, h);
        bottom.set_size(2 * w + side, h);
        left.set_size(w, side);
        right.set_size(w, side);
    }

    moveTo(x, y) {
        const side = this._side;
        const [x0, y0] = [Math.round(x - side / 2), Math.round(y - side / 2)];
        const [top, bottom, left, right] = this._panes;
        const w = global.stage.width;
        this._hole.set_translation(x0, y0, 0);
        top.set_translation(x0 - w, y0 - top.height, 0);
        bottom.set_translation(x0 - w, y0 + side, 0);
        left.set_translation(x0 - left.width, y0, 0);
        right.set_translation(x0 + side, y0, 0);
    }

    destroy() {
        global.stage.disconnectObject(this);
        this.actor.destroy();
    }
}

// The mark chosen, following the pointer through the cursor tracker; null for none
export class PointerMark {
    constructor(kind) {
        this.kind = kind;
        this._mark = kind === 'halo' ? new Halo() : kind === 'torch' ? new Torch() : null;
        this._tracker = global.backend.get_cursor_tracker();
        this._sprite = global.stage.context.get_backend().get_pointer_sprite(global.stage);
        if (this._mark) {
            this._tracker.connectObject('position-invalidated', () => this._follow(), this);
            this._follow();
        }
        // The fullscreen window is drawn by the compositor itself otherwise, and the mark would not show over it
        if (this._mark) {
            global.compositor.disable_unredirect();
            this._unredirected = true;
        }
    }

    get actor() {
        return this._mark?.actor ?? null;
    }

    _follow() {
        const point = this._sprite?.get_coords() ?? this._tracker.get_pointer()[0];
        this._mark?.moveTo(point.x, point.y);
    }

    destroy() {
        this._tracker?.disconnectObject(this);
        this._tracker = null;
        this._sprite = null;
        if (this._unredirected)
            global.compositor.enable_unredirect();
        this._unredirected = false;
        this._mark?.destroy();
        this._mark = null;
    }
}
