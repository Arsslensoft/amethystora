// Small line drawings in the colour of the text around them, from the boards' own path data on a square of 24: the
// speaker of reading aloud and the pen's six tools. They are drawn with Cairo rather than loaded as symbolic icons,
// since the shell fills a symbolic icon and a drawing made of lines would come out solid.

import Cairo from 'cairo';
import Clutter from 'gi://Clutter';
import St from 'gi://St';

const PIECE = /[MmLlHhVvCcSsAaZz]|[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?/g;
// How many numbers each command takes
const COUNT = {m: 2, l: 2, h: 1, v: 1, c: 6, s: 4, a: 7, z: 0};

// An elliptical arc as SVG writes it, here only ever circular: from (x0, y0) to (x, y) with radius r
function arcTo(cr, x0, y0, r, largeArc, clockwise, x, y) {
    const mx = (x0 - x) / 2;
    const my = (y0 - y) / 2;
    const d2 = mx * mx + my * my;
    if (d2 === 0)
        return;
    // A radius too small for the two points grows until it reaches
    r = Math.max(r, Math.sqrt(d2));
    const k = (largeArc === clockwise ? -1 : 1) * Math.sqrt(Math.max(0, r * r - d2) / d2);
    const cx = k * my + (x0 + x) / 2;
    const cy = -k * mx + (y0 + y) / 2;
    let from = Math.atan2(y0 - cy, x0 - cx);
    let to = Math.atan2(y - cy, x - cx);
    if (clockwise) {
        if (to < from)
            to += 2 * Math.PI;
        cr.arc(cx, cy, r, from, to);
    } else {
        if (to > from)
            from += 2 * Math.PI;
        cr.arcNegative(cx, cy, r, from, to);
    }
}

// Follows the path data d with Cairo's pen
export function tracePath(cr, d) {
    const pieces = d.match(PIECE) ?? [];
    let x = 0, y = 0, startX = 0, startY = 0;
    // The second control point of the last curve, for a smooth one after it
    let cx2 = null, cy2 = null;
    let command = '';
    let i = 0;
    while (i < pieces.length) {
        if (/^[A-Za-z]$/.test(pieces[i]))
            command = pieces[i++];
        const kind = command.toLowerCase();
        const rel = command === kind;
        const n = COUNT[kind];
        if (n === undefined)
            return;
        const v = pieces.slice(i, i + n).map(Number);
        if (v.length < n || v.some(Number.isNaN))
            return;
        i += n;
        const ox = rel ? x : 0;
        const oy = rel ? y : 0;
        let curve = false;
        switch (kind) {
        case 'm':
            x = ox + v[0];
            y = oy + v[1];
            cr.moveTo(x, y);
            startX = x;
            startY = y;
            // Further pairs are lines
            command = rel ? 'l' : 'L';
            break;
        case 'l':
            x = ox + v[0];
            y = oy + v[1];
            cr.lineTo(x, y);
            break;
        case 'h':
            x = ox + v[0];
            cr.lineTo(x, y);
            break;
        case 'v':
            y = oy + v[0];
            cr.lineTo(x, y);
            break;
        case 'c':
            cr.curveTo(ox + v[0], oy + v[1], ox + v[2], oy + v[3], ox + v[4], oy + v[5]);
            cx2 = ox + v[2];
            cy2 = oy + v[3];
            x = ox + v[4];
            y = oy + v[5];
            curve = true;
            break;
        case 's': {
            // The first control point mirrors the last curve's second one
            const ax = cx2 === null ? x : 2 * x - cx2;
            const ay = cy2 === null ? y : 2 * y - cy2;
            cr.curveTo(ax, ay, ox + v[0], oy + v[1], ox + v[2], oy + v[3]);
            cx2 = ox + v[0];
            cy2 = oy + v[1];
            x = ox + v[2];
            y = oy + v[3];
            curve = true;
            break;
        }
        case 'a': {
            const tx = ox + v[5];
            const ty = oy + v[6];
            arcTo(cr, x, y, v[0], !!v[3], !!v[4], tx, ty);
            x = tx;
            y = ty;
            break;
        }
        case 'z':
            cr.closePath();
            x = startX;
            y = startY;
            break;
        }
        if (!curve)
            cx2 = cy2 = null;
    }
}

// A drawing of the paths, stroked `width` wide on a square of 24, the size its style class gives it
export function lineIcon(paths, styleClass, width = 2.2) {
    const area = new St.DrawingArea({style_class: styleClass, y_align: Clutter.ActorAlign.CENTER, reactive: false});
    area.connect('repaint', () => {
        const cr = area.get_context();
        const [w, h] = area.get_surface_size();
        const color = area.get_theme_node().get_foreground_color();
        cr.setSourceRGBA(color.red / 255, color.green / 255, color.blue / 255, color.alpha / 255);
        const size = Math.min(w, h);
        cr.translate((w - size) / 2, (h - size) / 2);
        cr.scale(size / 24, size / 24);
        cr.setLineWidth(width);
        cr.setLineCap(Cairo.LineCap.ROUND);
        cr.setLineJoin(Cairo.LineJoin.ROUND);
        for (const d of paths) {
            tracePath(cr, d);
            cr.stroke();
        }
        cr.$dispose();
    });
    // Its colour is the text's, which a theme, a hover or a plate changes
    area.connect('style-changed', () => area.queue_repaint());
    return area;
}
