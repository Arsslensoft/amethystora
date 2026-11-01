// The phone's battery outline and signal bars, drawn in the colour of the text beside them. Each is a drawing area
// repainted only when its value changes or the style does (the theme's text colour, the screen's scale); nothing
// here runs on a timer. Sizes are the board's (32, second card), in the units the stylesheet gives the area: a
// battery 10 by 16, the bars 16 by 12.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import St from 'gi://St';

// The bars' left edges and heights, bottom-aligned on 12; each 3 wide with a corner of 1
const BAR_X = [0, 4.3, 8.6, 12.9];
const BAR_HEIGHT = [4, 6.5, 9, 12];
const BAR_WIDTH = 3;
// A bar above the phone's level is drawn this faint
const FAINT = 0.35;

function roundedRect(cr, x, y, width, height, radius) {
    cr.newSubPath();
    cr.arc(x + width - radius, y + radius, radius, -Math.PI / 2, 0);
    cr.arc(x + width - radius, y + height - radius, radius, 0, Math.PI / 2);
    cr.arc(x + radius, y + height - radius, radius, Math.PI / 2, Math.PI);
    cr.arc(x + radius, y + radius, radius, Math.PI, 1.5 * Math.PI);
    cr.closePath();
}

// The cairo context of an area, scaled so that one unit is one board pixel, and the text colour of its theme node
function begin(area, units) {
    const cr = area.get_context();
    const [width, height] = area.get_surface_size();
    cr.scale(width / units[0], height / units[1]);
    return [cr, area.get_theme_node().get_foreground_color()];
}

export const BatteryGlyph = GObject.registerClass(
class PhoneBatteryGlyph extends St.DrawingArea {
    _init() {
        super._init({style_class: 'amethystora-connect-battery-glyph', y_align: Clutter.ActorAlign.CENTER});
    }

    // An outline, 8 by 14 at (1, 1), corner 2, a line of 1.5, no fill
    vfunc_repaint() {
        const [cr, colour] = begin(this, [10, 16]);
        cr.setSourceRGBA(colour.red / 255, colour.green / 255, colour.blue / 255, colour.alpha / 255);
        cr.setLineWidth(1.5);
        roundedRect(cr, 1, 1, 8, 14, 2);
        cr.stroke();
        cr.$dispose();
    }
});

export const SignalBars = GObject.registerClass(
class PhoneSignalBars extends St.DrawingArea {
    _init() {
        super._init({style_class: 'amethystora-connect-bars', y_align: Clutter.ActorAlign.CENTER});
        this._level = 0;
    }

    // 0 to 4: the bars up to it are full, the rest faint (0 is four faint bars: a SIM with no signal)
    set level(level) {
        if (level === this._level)
            return;
        this._level = level;
        this.queue_repaint();
    }

    get level() {
        return this._level;
    }

    vfunc_repaint() {
        const [cr, colour] = begin(this, [16, 12]);
        for (let bar = 0; bar < 4; bar++) {
            const strength = bar < this._level ? 1 : FAINT;
            cr.setSourceRGBA(colour.red / 255, colour.green / 255, colour.blue / 255, strength * colour.alpha / 255);
            roundedRect(cr, BAR_X[bar], 12 - BAR_HEIGHT[bar], BAR_WIDTH, BAR_HEIGHT[bar], 1);
            cr.fill();
        }
        cr.$dispose();
    }
});
