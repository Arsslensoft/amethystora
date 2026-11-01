// What marks an item (spec 1.4 and 1.14): a running app's bar, dots or glow at the pill's edge on the screen's side,
// and, while Super is held, the number Alt+N opens on the icon's far top corner. Both are in a layer over the item's
// container that asks for no room, beside the item's picture rather than in it: the mark follows the picture along the
// row as it swells, but never grows with it.

import Clutter from 'gi://Clutter';
import St from 'gi://St';

import {EASE, Layer, scaleFactor} from './util.js';

const MAX_DOTS = 4;
// The number's disc: its centre this far outside the icon's corner on both axes
const DISC_OUT = 6;
const DISC = {fade: 160, grow: 200, stagger: 18, from: 0.6};
// The glow sits 1 px in from the pill's edge, the bar 3 px: 2 px nearer the edge than the tile's
const GLOW_NEARER = 2;

// Where actor's allocation lies in ancestor's own coordinates, through the allocations between them
function boxIn(actor, ancestor) {
    const box = actor.get_allocation_box();
    let [x, y] = [box.x1, box.y1];
    for (let parent = actor.get_parent(); parent && parent !== ancestor; parent = parent.get_parent()) {
        const around = parent.get_allocation_box();
        x += around.x1;
        y += around.y1;
    }
    return {x, y, width: box.get_width(), height: box.get_height()};
}

export class ItemMarks {
    // state: the dash's record of the item (item, button, key, dash)
    constructor(state) {
        this._state = state;
        this.layer = new Layer();
        state.item.add_child(this.layer);
        this.layer.connect('notify::allocation', () => this.place());
        // The item may go first, and take the layer with it
        this.layer.connect('destroy', () => {
            this._gone = true;
            this.mark = this.disc = null;
        });
        this.kind = null;
        this.count = 0;
        this.mark = null;
        this.disc = null;
        this.number = 0;
    }

    destroy() {
        if (!this._gone)
            this.layer.destroy();
        this.mark = this.disc = null;
    }

    // kind: bar, dot or glow; windows: how many the item counts
    sync(kind, windows) {
        if (this._gone)
            return;
        if (kind !== this.kind || (kind === 'dot' && Math.min(windows, MAX_DOTS) !== Math.min(this.count, MAX_DOTS))) {
            this.mark?.destroy();
            this.mark = this._make(kind, windows);
            this.layer.add_child(this.mark);
            this.kind = kind;
        }
        this.count = windows;
        if (kind === 'bar') {
            if (windows >= 2)
                this.mark.add_style_class_name('amethystora-dock-bar-wide');
            else
                this.mark.remove_style_class_name('amethystora-dock-bar-wide');
        }
        this.mark.visible = windows > 0;
        this._catchUp();
        this.place();
    }

    _make(kind, windows) {
        if (kind === 'glow')
            return new St.Widget({style_class: 'amethystora-dock-glow'});
        if (kind === 'bar')
            return new St.Widget({style_class: 'amethystora-dock-bar'});
        const vertical = this._state.dash.edge.vertical;
        const dots = new St.BoxLayout({
            style_class: 'amethystora-dock-dots',
            orientation: vertical ? Clutter.Orientation.VERTICAL : Clutter.Orientation.HORIZONTAL,
        });
        const size = this._state.dash.iconSize + 12;
        const dot = Math.max(2, Math.round(size * 0.09));
        dots.set_style(`spacing: ${Math.max(1, Math.round(dot / 2))}px;`);
        for (let i = 0; i < Math.min(windows, MAX_DOTS); i++) {
            const child = new St.Widget({style_class: 'app-grid-running-dot amethystora-dock-dot'});
            child.set_style(`width: ${dot}px; height: ${dot}px; border-radius: ${dot}px;`);
            dots.add_child(child);
        }
        return dots;
    }

    // The mark at the tile's edge on the screen's side, centred on the tile along the row
    place() {
        if (this._gone)
            return;
        const {button, dash} = this._state;
        if (!button.has_allocation())
            return;

        const tile = button.get_allocation_box();
        const sf = scaleFactor();
        if (this.mark) {
            const [, , width, height] = this.mark.get_preferred_size();
            const edge = dash.edgeName;
            const nearer = ({glow: GLOW_NEARER, dot: -1}[this.kind] ?? 0) * sf;
            let x, y;
            if (edge === 'bottom') {
                x = (tile.x1 + tile.x2 - width) / 2;
                y = tile.y2 - height + nearer;
            } else if (edge === 'left') {
                x = tile.x1 - nearer;
                y = (tile.y1 + tile.y2 - height) / 2;
            } else {
                x = tile.x2 - width + nearer;
                y = (tile.y1 + tile.y2 - height) / 2;
            }
            this.mark.set_position(Math.round(x), Math.round(y));
        }
        if (this.disc)
            this._placeDisc();
    }

    // Copied from the picture's place along the row on every frame of the swell; only written while something in the
    // layer shows, since most items have no mark and every write costs a frame time
    follow(along, long) {
        this._along = along;
        this._long = long;
        if (this._gone || !(this.mark?.visible || this.disc?.visible))
            return;
        if (long === 'x')
            this.layer.translation_x = along;
        else
            this.layer.translation_y = along;
    }

    // Where the swell left the item, when the mark or the number shows again
    _catchUp() {
        if (this._gone || this._along === undefined)
            return;
        if (this._long === 'x')
            this.layer.translation_x = this._along;
        else
            this.layer.translation_y = this._along;
    }

    get markBox() {
        if (!this.mark)
            return null;
        const [x, y] = this.mark.get_transformed_position();
        const [width, height] = this.mark.get_transformed_size();
        return {kind: this.kind, x, y, width, height, visible: this.mark.visible && this.mark.mapped};
    }

    // ---- The number ----

    showNumber(number, order, level) {
        this.number = number;
        if (!this.disc) {
            this.disc = new St.Label({style_class: 'amethystora-dock-number', opacity: 0, visible: false});
            this.disc.clutter_text.use_markup = false;
            this.disc.clutter_text.x_align = Clutter.ActorAlign.CENTER;
            this.disc.clutter_text.y_align = Clutter.ActorAlign.CENTER;
            this.disc.set_pivot_point(0.5, 0.5);
            this.layer.add_child(this.disc);
        }
        this.disc.text = number.toLocaleString();
        this.disc.visible = true;
        this._catchUp();
        this._placeDisc();
        this.disc.remove_all_transitions();
        if (level !== 'full') {
            this.disc.set({scale_x: 1, scale_y: 1});
            this.disc.ease({opacity: 255, duration: level === 'none' ? 0 : DISC.fade, mode: EASE.out});
            return;
        }
        this.disc.set({opacity: 0, scale_x: DISC.from, scale_y: DISC.from});
        const delay = DISC.stagger * order;
        this.disc.ease({opacity: 255, duration: DISC.fade, delay, mode: EASE.out});
        this.disc.ease({scale_x: 1, scale_y: 1, duration: DISC.grow, delay, mode: EASE.back});
    }

    hideNumber() {
        const disc = this.disc;
        if (!disc?.visible)
            return;
        disc.remove_all_transitions();
        disc.ease({
            opacity: 0,
            duration: DISC.fade,
            mode: EASE.out,
            onComplete: () => (disc.visible = false),
        });
    }

    get numberShown() {
        return Boolean(this.disc?.visible && this.disc.opacity > 0);
    }

    // The icon's square inside the container, and the disc's centre beyond its far top corner
    iconBox() {
        const {button, item} = this._state;
        const icon = button.icon?.icon;
        return icon?.has_allocation() ? boxIn(icon, item) : null;
    }

    _placeDisc() {
        const box = this.iconBox();
        if (!box)
            return;
        const sf = scaleFactor();
        const out = DISC_OUT * sf;
        const [, , width, height] = this.disc.get_preferred_size();
        const edge = this._state.dash.edgeName;
        const rtl = this._state.dash.get_text_direction() === Clutter.TextDirection.RTL;
        let cx, cy;
        if (edge === 'bottom') {
            cx = rtl ? box.x - out : box.x + box.width + out;
            cy = box.y - out;
        } else {
            cx = edge === 'left' ? box.x + box.width + out : box.x - out;
            cy = box.y + box.height + out;
        }
        this.disc.set_position(Math.round(cx - width / 2), Math.round(cy - height / 2));
    }

    // The disc's centre and box on the stage
    discBox() {
        if (!this.disc?.visible)
            return null;
        const [x, y] = this.disc.get_transformed_position();
        const [width, height] = this.disc.get_transformed_size();
        return {x, y, width, height};
    }
}
