// The pill's weight (R7-17): a mass on a spring, a little under critical damping, written out so that each frame is one
// sum and a change of mind keeps the speed the plate already has. A distance from rest and a speed go in, the distance
// and the speed t seconds later come out. Free of GNOME Shell's imports, so that it runs under node (test-pill-logic.mjs).

// Radians a second, and how little it swings: about 6 % past rest, then still
const STIFF = 14;
const DAMPING = 0.66;
// By now it is within half a percent of rest, in milliseconds before GNOME's slow-down factor
export const SETTLE_MS = 600;

export function springAt(from, speed, t) {
    const decay = DAMPING * STIFF;
    const swing = STIFF * Math.sqrt(1 - DAMPING * DAMPING);
    const b = (speed + decay * from) / swing;
    const fade = Math.exp(-decay * t);
    const [c, s] = [Math.cos(swing * t), Math.sin(swing * t)];
    return [fade * (from * c + b * s), fade * ((b * swing - from * decay) * c - (from * swing + b * decay) * s)];
}

// A ring round a stadium w wide and h high, from the top's middle clockwise: how far along its length, as pieces to draw.
// Each is ['line', x1, y1, x2, y2] or ['arc', cx, cy, r, from, to]. fraction is held to 0..1.
export function ringPieces(w, h, inset, fraction) {
    const r = Math.max(0, h / 2 - inset);
    const straight = Math.max(0, w - h);
    const left = h / 2;
    const right = left + straight;
    const [top, bottom] = [inset, h - inset];
    const middle = w / 2;
    const all = [
        ['line', middle, top, right, top, straight / 2],
        ['arc', right, h / 2, r, -Math.PI / 2, Math.PI / 2, Math.PI * r],
        ['line', right, bottom, left, bottom, straight],
        ['arc', left, h / 2, r, Math.PI / 2, 3 * Math.PI / 2, Math.PI * r],
        ['line', left, top, middle, top, straight / 2],
    ];
    const total = all.reduce((sum, piece) => sum + piece[piece.length - 1], 0);
    let room = Math.min(Math.max(fraction, 0), 1) * total;
    const pieces = [];
    for (const piece of all) {
        const length = piece[piece.length - 1];
        if (room <= 0 || length <= 0)
            continue;
        const part = Math.min(room / length, 1);
        room -= length * part;
        if (piece[0] === 'line') {
            const [, x1, y1, x2, y2] = piece;
            pieces.push(['line', x1, y1, x1 + (x2 - x1) * part, y1 + (y2 - y1) * part]);
        } else {
            const [, cx, cy, radius, from, to] = piece;
            pieces.push(['arc', cx, cy, radius, from, from + (to - from) * part]);
        }
    }
    return pieces;
}
