// A ring round the pill or its bubble that drains as something runs (R7-22) or fills as it moves (R7-20): one stroke
// along the outline, drawn only when its value changes. The part left is in the theme's accent, over a faint track.

import St from 'gi://St';
import GObject from 'gi://GObject';

import {ringPieces} from './spring.js';

// Not drawn again for less than this
const STEP = 0.002;

export const RoundRing = GObject.registerClass(
class AmethystoraPillRoundRing extends St.DrawingArea {
    _init() {
        super._init({style_class: 'amethystora-pill-round-ring', x_expand: true, y_expand: true});
        this._fraction = -1;
    }

    set fraction(value) {
        if (Math.abs(value - this._fraction) < STEP)
            return;
        this._fraction = value;
        this.queue_repaint();
    }

    get fraction() {
        return this._fraction;
    }

    vfunc_repaint() {
        const cr = this.get_context();
        const [width, height] = this.get_surface_size();
        const node = this.get_theme_node();
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const stroke = 2 * scale;
        cr.setLineWidth(stroke);
        cr.setLineCap(1); // round
        const draw = pieces => {
            for (const piece of pieces) {
                if (piece[0] === 'line') {
                    cr.moveTo(piece[1], piece[2]);
                    cr.lineTo(piece[3], piece[4]);
                } else {
                    cr.arc(piece[1], piece[2], piece[3], piece[4], piece[5]);
                }
                cr.stroke();
            }
        };
        const [found, track] = node.lookup_color('-amethystora-pill-track', false);
        if (found) {
            cr.setSourceColor(track);
            draw(ringPieces(width, height, stroke / 2, 1));
        }
        cr.setSourceColor(node.get_foreground_color());
        draw(ringPieces(width, height, stroke / 2, this._fraction));
        cr.$dispose();
    }
});
