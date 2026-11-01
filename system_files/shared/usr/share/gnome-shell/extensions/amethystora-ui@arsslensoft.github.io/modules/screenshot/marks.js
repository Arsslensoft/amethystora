// What a person draws over a screenshot, and how each mark is painted. The screen and the saved picture are painted
// by the same code, with the same numbering of the markers, so that what is saved is exactly what was seen.

import Cairo from 'cairo';
import Pango from 'gi://Pango';
import PangoCairo from 'gi://PangoCairo';

// The tools in the toolbar's order, which is also the order of the number keys
export const TOOLS = ['area', 'move', 'pen', 'box', 'fill', 'highlight', 'arrow', 'text', 'marker'];
// The tools that draw: with one of them chosen, the pointer draws instead of framing
export const DRAWING = new Set(['pen', 'box', 'fill', 'highlight', 'arrow', 'text', 'marker']);

// The swatches, left to right
export const COLOURS = ['#ffffff', '#000000', '#ff4444', '#ff8800', '#ffdd00', '#44cc44', '#4488ff', '#aa44ff'];

// The line width's range, in logical pixels
export const MIN_WIDTH = 1;
export const MAX_WIDTH = 16;

// How far around a mark a press still picks it up
const PICK_MARGIN = 8;
// The arrow's head: each stroke is this many line widths long, at a seventh of a half turn from the line
const HEAD_LENGTH = 5;
const HEAD_ANGLE = Math.PI / 7;
// The translucent pen: wider than the line width, and seen through
const HIGHLIGHT_SCALE = 4;
const HIGHLIGHT_ALPHA = 0.4;
// A marker's disc, in line widths, and its number's size against the disc
const MARKER_RADIUS = 5;
const MARKER_TEXT = 1.2;

export function parseColour(hex) {
    const value = parseInt(hex.slice(1), 16);
    return [(value >> 16 & 255) / 255, (value >> 8 & 255) / 255, (value & 255) / 255];
}

// Light colours get dark numbers and dark colours light ones, by how bright the eye sees them
function isLight(hex) {
    const [r, g, b] = parseColour(hex);
    return 0.299 * r + 0.587 * g + 0.114 * b > 0.5;
}

export function textSize(width) {
    return Math.max(8, 3 * width);
}

function layoutFor(cr, text, size, bold = false) {
    const layout = PangoCairo.create_layout(cr);
    const font = Pango.FontDescription.from_string(bold ? 'Sans Bold' : 'Sans');
    font.set_absolute_size(size * Pango.SCALE);
    layout.set_font_description(font);
    layout.set_text(text, -1);
    return layout;
}

// The size a line of text takes, in logical pixels, for picking it up
export function measureText(text, size) {
    const surface = new Cairo.ImageSurface(Cairo.Format.ARGB32, 1, 1);
    const cr = new Cairo.Context(surface);
    const [, logical] = layoutFor(cr, text, size).get_pixel_extents();
    cr.$dispose();
    surface.finish();
    return [logical.width, logical.height];
}

// A new mark. Strokes and boxes take more points as the pointer moves; text and markers have one.
export function newMark(tool, colour, width, point) {
    return {tool, colour, width, points: [point], text: '', extent: [0, 0]};
}

// A stroke or a box made of a single point, a click without moving, draws nothing and is dropped
export function isEmpty(mark) {
    if (mark.tool === 'text')
        return mark.text === '';
    if (mark.tool === 'marker')
        return false;
    const [x0, y0] = mark.points[0];
    return mark.points.every(([x, y]) => x === x0 && y === y0);
}

export function moveMark(mark, dx, dy) {
    mark.points = mark.points.map(([x, y]) => [x + dx, y + dy]);
}

// The rectangle round a mark inside which a press picks it up, as [x1, y1, x2, y2]
export function pickBox(mark) {
    const xs = mark.points.map(p => p[0]);
    const ys = mark.points.map(p => p[1]);
    let [x1, y1, x2, y2] = [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
    let grow = PICK_MARGIN;
    switch (mark.tool) {
    case 'pen':
    case 'box':
        grow += mark.width;
        break;
    case 'highlight':
        grow += HIGHLIGHT_SCALE * mark.width;
        break;
    case 'arrow':
        grow += 2 * mark.width;
        break;
    case 'text':
        x2 += mark.extent[0];
        y2 += mark.extent[1];
        break;
    case 'marker':
        grow += MARKER_RADIUS * mark.width;
        break;
    }
    return [x1 - grow, y1 - grow, x2 + grow, y2 + grow];
}

// The topmost mark under a point, or null
export function markAt(marks, x, y) {
    for (let i = marks.length - 1; i >= 0; i--) {
        const [x1, y1, x2, y2] = pickBox(marks[i]);
        if (x >= x1 && x <= x2 && y >= y1 && y <= y2)
            return marks[i];
    }
    return null;
}

function strokeThrough(cr, points) {
    cr.moveTo(...points[0]);
    for (const point of points.slice(1))
        cr.lineTo(...point);
    cr.stroke();
}

function paintMark(cr, mark, number) {
    const [r, g, b] = parseColour(mark.colour);
    const width = mark.width;
    const points = mark.points;
    cr.setSourceRGBA(r, g, b, 1);
    cr.setLineWidth(width);
    cr.setLineCap(Cairo.LineCap.ROUND);
    cr.setLineJoin(Cairo.LineJoin.ROUND);

    switch (mark.tool) {
    case 'pen':
        if (points.length > 1)
            strokeThrough(cr, points);
        break;
    case 'highlight':
        if (points.length < 2)
            break;
        cr.setSourceRGBA(r, g, b, HIGHLIGHT_ALPHA);
        cr.setLineWidth(HIGHLIGHT_SCALE * width);
        cr.setLineCap(Cairo.LineCap.SQUARE);
        strokeThrough(cr, points);
        break;
    case 'box':
    case 'fill': {
        if (points.length < 2)
            break;
        const [[x0, y0], [x1, y1]] = [points[0], points[points.length - 1]];
        cr.setLineJoin(Cairo.LineJoin.MITER);
        cr.rectangle(Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0));
        if (mark.tool === 'fill')
            cr.fill();
        else
            cr.stroke();
        break;
    }
    case 'arrow': {
        if (points.length < 2)
            break;
        const [[x0, y0], [x1, y1]] = [points[0], points[points.length - 1]];
        const angle = Math.atan2(y1 - y0, x1 - x0);
        const length = HEAD_LENGTH * width;
        cr.moveTo(x0, y0);
        cr.lineTo(x1, y1);
        cr.moveTo(x1 - length * Math.cos(angle - HEAD_ANGLE), y1 - length * Math.sin(angle - HEAD_ANGLE));
        cr.lineTo(x1, y1);
        cr.lineTo(x1 - length * Math.cos(angle + HEAD_ANGLE), y1 - length * Math.sin(angle + HEAD_ANGLE));
        cr.stroke();
        break;
    }
    case 'text': {
        if (!mark.text)
            break;
        cr.moveTo(...points[0]);
        PangoCairo.show_layout(cr, layoutFor(cr, mark.text, textSize(width)));
        break;
    }
    case 'marker': {
        const [x, y] = points[0];
        const radius = MARKER_RADIUS * width;
        cr.arc(x, y, radius, 0, 2 * Math.PI);
        cr.fill();
        const layout = layoutFor(cr, `${number}`, MARKER_TEXT * radius, true);
        const [ink] = layout.get_pixel_extents();
        if (isLight(mark.colour))
            cr.setSourceRGBA(0, 0, 0, 1);
        else
            cr.setSourceRGBA(1, 1, 1, 1);
        cr.moveTo(x - ink.x - ink.width / 2, y - ink.y - ink.height / 2);
        PangoCairo.show_layout(cr, layout);
        break;
    }
    }
}

// Every mark, in the order drawn, onto a context whose origin is the point (x, y) of the stage, at `scale` pixels
// to a logical pixel. Markers are numbered 1, 2, 3 in the order they were placed, counted again after a mark is
// taken away, on the screen and in the picture alike.
export function paintMarks(cr, marks, x, y, scale) {
    cr.save();
    cr.scale(scale, scale);
    cr.translate(-x, -y);
    let number = 0;
    for (const mark of marks) {
        if (mark.tool === 'marker')
            number++;
        paintMark(cr, mark, number);
    }
    cr.restore();
}

// The dashed frame round a picked-up mark, on the screen only: white dashes, over a faint dark line so that the frame
// shows on a white page as well
export function paintPicked(cr, mark, x, y) {
    const [x1, y1, x2, y2] = pickBox(mark);
    cr.save();
    cr.translate(-x, -y);
    cr.setLineWidth(1.5);
    cr.rectangle(x1, y1, x2 - x1, y2 - y1);
    cr.setSourceRGBA(0, 0, 0, 0.35);
    cr.strokePreserve();
    cr.setSourceRGBA(1, 1, 1, 0.9);
    cr.setDash([5, 4], 0);
    cr.stroke();
    cr.restore();
}
