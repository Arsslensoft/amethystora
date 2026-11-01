// The three profiles' marks, as the board draws them: a briefcase and a game pad in lines 2.4 wide with round ends, and
// a moon filled, each on a square of 24. They are drawn with Cairo in the text's colour rather than loaded as symbolic
// icons, since a symbolic icon is filled by the shell and a drawing in lines would come out as solid shapes.

import Cairo from 'cairo';
import Clutter from 'gi://Clutter';
import St from 'gi://St';

// The board's paths, in its own units (viewBox 0 0 24 24)
const MARKS = {
    work: {stroke: ['M3 10A3 3 0 0 1 6 7H18A3 3 0 0 1 21 10V17A3 3 0 0 1 18 20H6A3 3 0 0 1 3 17Z',
        'M9 7V5.5A1.5 1.5 0 0 1 10.5 4h3A1.5 1.5 0 0 1 15 5.5V7M3 13h18']},
    games: {stroke: ['M7 8h10a4 4 0 0 1 4 4.5l-.6 3.5a2.5 2.5 0 0 1-4.4 1.1L14.5 15h-5L8 17.1a2.5 2.5 0 0 1-4.4-1.1L3 12.5A4 4 0 0 1 7 8z',
        'M8 10.5v3M6.5 12h3']},
    sleep: {fill: ['M20 14.5A8.5 8.5 0 0 1 9.5 4 8.5 8.5 0 1 0 20 14.5z']},
};
const LINE = 2.4;

const TOKEN = /[MmLlHhVvAaZz]|[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?/g;
const ARGS = {m: 2, l: 2, h: 1, v: 1, a: 7, z: 0};

// An SVG arc from the current point, as a circle (the marks have no ellipses and no rotation)
function arc(cr, x1, y1, r, large, sweep, x2, y2) {
    const dx = (x1 - x2) / 2;
    const dy = (y1 - y2) / 2;
    const scale = Math.max(1, Math.sqrt((dx * dx + dy * dy) / (r * r)));
    r *= scale;
    const sign = large === sweep ? -1 : 1;
    const d2 = dx * dx + dy * dy;
    const coef = sign * Math.sqrt(Math.max(0, (r * r - d2) / d2));
    const cx = coef * dy + (x1 + x2) / 2;
    const cy = -coef * dx + (y1 + y2) / 2;
    const a1 = Math.atan2(y1 - cy, x1 - cx);
    let a2 = Math.atan2(y2 - cy, x2 - cx);
    if (sweep && a2 < a1)
        a2 += 2 * Math.PI;
    if (!sweep && a2 > a1)
        a2 -= 2 * Math.PI;
    if (sweep)
        cr.arc(cx, cy, r, a1, a2);
    else
        cr.arcNegative(cx, cy, r, a1, a2);
}

// The path `d` into Cairo, for the commands the marks use
function trace(cr, d) {
    const tokens = d.match(TOKEN);
    let x = 0, y = 0, sx = 0, sy = 0, i = 0, command = null;
    while (i < tokens.length) {
        if (/[A-Za-z]/.test(tokens[i]))
            command = tokens[i++];
        const lower = command.toLowerCase();
        const relative = command === lower;
        const n = ARGS[lower];
        const v = tokens.slice(i, i + n).map(Number);
        i += n;
        switch (lower) {
        case 'm':
            x = relative ? x + v[0] : v[0];
            y = relative ? y + v[1] : v[1];
            cr.moveTo(x, y);
            [sx, sy] = [x, y];
            // Pairs after a move are lines
            command = relative ? 'l' : 'L';
            break;
        case 'l':
            x = relative ? x + v[0] : v[0];
            y = relative ? y + v[1] : v[1];
            cr.lineTo(x, y);
            break;
        case 'h':
            x = relative ? x + v[0] : v[0];
            cr.lineTo(x, y);
            break;
        case 'v':
            y = relative ? y + v[0] : v[0];
            cr.lineTo(x, y);
            break;
        case 'a': {
            const tx = relative ? x + v[5] : v[5];
            const ty = relative ? y + v[6] : v[6];
            arc(cr, x, y, v[0], v[3], v[4], tx, ty);
            [x, y] = [tx, ty];
            break;
        }
        case 'z':
            cr.closePath();
            [x, y] = [sx, sy];
            break;
        }
    }
}

// A profile's mark, the size its style class gives it, in the colour of the text around it
export function markIcon(profile, styleClass) {
    const area = new St.DrawingArea({style_class: styleClass, y_align: Clutter.ActorAlign.CENTER});
    area.connect('repaint', () => {
        const mark = MARKS[profile];
        const cr = area.get_context();
        const [width, height] = area.get_surface_size();
        const color = area.get_theme_node().get_foreground_color();
        cr.setSourceRGBA(color.red / 255, color.green / 255, color.blue / 255, color.alpha / 255);
        const size = Math.min(width, height);
        cr.translate((width - size) / 2, (height - size) / 2);
        cr.scale(size / 24, size / 24);
        cr.setLineWidth(LINE);
        cr.setLineCap(Cairo.LineCap.ROUND);
        cr.setLineJoin(Cairo.LineJoin.ROUND);
        for (const d of mark?.stroke ?? []) {
            trace(cr, d);
            cr.stroke();
        }
        for (const d of mark?.fill ?? []) {
            trace(cr, d);
            cr.fill();
        }
        cr.$dispose();
    });
    // The text's colour changes with the theme and the plate
    area.connect('style-changed', () => area.queue_repaint());
    return area;
}
