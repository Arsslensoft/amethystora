// The arithmetic of the droplet, with nothing of GNOME's in it, so that it can be checked on its own (node imports
// this file): the curves, the arc a drop flies along, and the shape of a window at any moment of its way into the
// dock and back. Every length is in stage pixels, every moment `u` a share of a flight from 0 to 1.
//
// The moments are the design board's, written as its percentages of a 2.4 s loop: the window goes out between 0 % and
// 38 % and comes back between 62 % and 100 %.

// A curve as CSS writes it, cubic-bezier(x1, y1, x2, y2): its value at t, from 0 to 1
export function cubicBezier(x1, y1, x2, y2) {
    const cx = 3 * x1;
    const bx = 3 * (x2 - x1) - cx;
    const ax = 1 - cx - bx;
    const cy = 3 * y1;
    const by = 3 * (y2 - y1) - cy;
    const ay = 1 - cy - by;
    const xAt = s => ((ax * s + bx) * s + cx) * s;
    const slope = s => (3 * ax * s + 2 * bx) * s + cx;
    const yAt = s => ((ay * s + by) * s + cy) * s;
    return t => {
        if (t <= 0)
            return 0;
        if (t >= 1)
            return 1;
        // Newton's steps first; where the curve is too flat for them, halving the interval
        let s = t;
        for (let i = 0; i < 8; i++) {
            const error = xAt(s) - t;
            if (Math.abs(error) < 1e-7)
                return yAt(s);
            const d = slope(s);
            if (Math.abs(d) < 1e-6)
                break;
            s -= error / d;
        }
        let lo = 0;
        let hi = 1;
        s = t;
        for (let i = 0; i < 40; i++) {
            const x = xAt(s);
            if (Math.abs(x - t) < 1e-7)
                break;
            if (x < t)
                lo = s;
            else
                hi = s;
            s = (lo + hi) / 2;
        }
        return yAt(s);
    };
}

// CSS's ease and ease-in-out, which are also Clutter's EASE and EASE_IN_OUT
export const ease = cubicBezier(0.25, 0.1, 0.25, 1);
export const easeInOut = cubicBezier(0.42, 0, 0.58, 1);
// The workspace switch's curve
export const glide = cubicBezier(0.3, 0.7, 0.2, 1);

const board = percent => percent / 38;
const back = percent => (percent - 62) / 38;

// The moments of the way out
export const OUT = {
    circle: board(12),      // the window has gathered into a circle
    small: board(19),       // the circle has shrunk, and leaves
    middle: board(27),      // half way down
    landed: board(35),      // at the icon
    gloss: board(14),       // the gloss is full
    ripple: board(34),      // the ring starts round the icon
};
// The moments of the way back
export const IN = {
    shown: back(66),        // the drop is there, on the icon
    middle: back(76),       // half way up
    glossFrom: back(74),    // the gloss starts to go
    over: back(86),         // a window again, a little larger than itself
    under: back(93),        // a little smaller
};

// A flight's whole time before GNOME's slow-down factor, in milliseconds, and the ring's two parts
export const FLIGHT_MS = 912;
export const RIPPLE_IN_MS = 96;
export const RIPPLE_OUT_MS = 336;
// A fade, while motion is reduced
export const FADE_MS = 150;

const lerp = (a, b, t) => a + (b - a) * t;
const clamp01 = t => Math.min(1, Math.max(0, t));
const along = (u, from, to) => clamp01((u - from) / (to - from));
// A length that shrinks or grows by the same share each moment
const geometric = (a, b, t) => a > 0 && b > 0 ? a ** (1 - t) * b ** t : lerp(a, b, t);

// The arc from p0 to p2, bowed to the left of the way the window falls (with y down), as the board draws it; ways
// up and down bow the same side. `bow` and `reach` place the control point in the window-to-icon frame.
export function arc(p0, p2, window, icon, reach, bow) {
    const dx = icon.x - window.x;
    const dy = icon.y - window.y;
    const length = Math.hypot(dx, dy);
    if (length < 1)
        return () => ({x: p2.x, y: p2.y});
    const ux = dx / length;
    const uy = dy / length;
    const c = {
        x: window.x + reach * length * ux - bow * length * uy,
        y: window.y + reach * length * uy + bow * length * ux,
    };
    return s => {
        const a = (1 - s) * (1 - s);
        const b = 2 * s * (1 - s);
        const d = s * s;
        return {x: a * p0.x + b * c.x + d * p2.x, y: a * p0.y + b * c.y + d * p2.y};
    };
}

// The arcs' control points, from the board's waypoints: (-64, 160) at 27 % on the way down and (-48, 130) at 76 % on
// the way up, for an icon 293 px straight below the window's centre
export const DOWN = {reach: 0.592, bow: 0.437};
export const UP = {reach: 0.387, bow: 0.328};

// Sizes that follow from the window and the icon
export function sizes(geo) {
    const d1 = 0.72 * Math.min(geo.W, geo.H);
    return {
        d1,
        d2: 0.65 * d1,
        dm: 0.513 * Math.min(geo.W, geo.H),
        de: 0.72 * geo.S,
    };
}

// A box never has corners rounder than half its shorter side
function box(centre, w, h, r, g, o) {
    w = Math.max(w, 0);
    h = Math.max(h, 0);
    return {centre, w, h, r: Math.min(Math.max(r, 0), w / 2, h / 2), g: clamp01(g), o: clamp01(o)};
}

// One part of a flight, from moment a to b: how far through it u is, and whether it starts from where a flight
// that turned round left off (its part is the one u0 falls in) rather than from the table
function part(u, a, b, from) {
    const u0 = from?.u0 ?? 0;
    const resumed = !!from && u0 >= a && u0 < b;
    return {t: along(u, resumed ? u0 : a, b), resumed};
}

// How far along an arc that runs over [a, b] a flight is, counted from where it joined the arc
function arcShare(u, a, b, from) {
    const u0 = from?.u0 ?? 0;
    const s = easeInOut(along(u, a, b));
    if (!from || u0 <= a)
        return s;
    const s0 = easeInOut(along(u0, a, b));
    return s0 < 1 ? clamp01((s - s0) / (1 - s0)) : 1;
}

// A share that goes from v0 to v1 over [a, b], starting from the resumed value where u0 falls inside
function fadeTrack(u, a, b, v0, v1, from, key) {
    const u0 = from?.u0 ?? 0;
    if (from && u0 >= a) {
        if (u0 >= b)
            return from[key];
        return lerp(from[key], v1, ease(along(u, u0, b)));
    }
    return lerp(v0, v1, ease(along(u, a, b)));
}

// The way out. geo: {W, H, R0, S, Fc, Ic}, the window's frame size, its corner radius, the icon's size and the two
// centres. from (optional) is where a flight that turned round starts, {u0, centre, w, h, r, g, o}: the parts before
// u0 are not drawn, and the part u0 falls in starts from that state instead of the table's.
function minimizeShape(u, geo, from) {
    const {W, H, R0, Fc, Ic} = geo;
    const {d1, d2, de} = sizes(geo);
    const u0 = from?.u0 ?? 0;
    // The gloss comes up as the window gathers; a flight that turned round brings it up from where it was
    let g;
    if (!from)
        g = ease(along(u, 0, OUT.gloss));
    else
        g = lerp(from.g, 1, ease(along(u, u0, u0 < OUT.gloss ? OUT.gloss : Math.max(OUT.landed, u0 + 1e-6))));

    if (u < OUT.circle) {
        // Gathering into a circle where the window is
        const {t, resumed} = part(u, 0, OUT.circle, from);
        const start = resumed ? from : {centre: Fc, w: W, h: H, r: R0};
        const k = ease(t);
        return box(lerpPoint(start.centre, Fc, k), lerp(start.w, d1, k), lerp(start.h, d1, k),
            lerp(start.r, d1 / 2, k), g, 1);
    }
    if (u < OUT.small) {
        const {t, resumed} = part(u, OUT.circle, OUT.small, from);
        const start = resumed ? from : {centre: Fc, w: d1, h: d1, r: d1 / 2};
        const k = ease(t);
        return box(lerpPoint(start.centre, Fc, k), lerp(start.w, d2, k), lerp(start.h, d2, k),
            lerp(start.r, d2 / 2, k), g, 1);
    }
    if (u < OUT.landed) {
        // Down the arc into the icon, shrinking by the same share all the way
        const {resumed} = part(u, OUT.small, OUT.landed, from);
        const start = resumed ? from : {centre: Fc, w: d2, h: d2, r: d2 / 2};
        const s = arcShare(u, OUT.small, OUT.landed, from);
        // A drop that turned round bows as much as the way it still has to go
        const path = arc(start.centre, Ic, start.centre, Ic, DOWN.reach, DOWN.bow);
        return box(path(s), geometric(start.w, de, s), geometric(start.h, de, s), lerp(start.r, de / 2, s), g, 1);
    }
    // Landed: it fades into the icon
    const o = fadeTrack(u, OUT.landed, 1, 1, 0, from && u0 >= OUT.landed ? from : null, 'o');
    return box(Ic, de, de, de / 2, from && u0 >= OUT.landed ? from.g : g, o);
}

function lerpPoint(a, b, t) {
    return {x: lerp(a.x, b.x, t), y: lerp(a.y, b.y, t)};
}

// The way back, and an app opening from its icon. from as for minimizeAt.
function unminimizeShape(u, geo, from) {
    const {W, H, R0, Fc, Ic} = geo;
    const {dm, de} = sizes(geo);
    const u0 = from?.u0 ?? 0;
    // The gloss stays until the drop opens, and is gone when it is a window again
    let g;
    if (!from)
        g = 1 - ease(along(u, IN.glossFrom, IN.over));
    else if (u0 < IN.glossFrom)
        g = from.g * (1 - ease(along(u, IN.glossFrom, IN.over)));
    else
        g = from.g * (1 - ease(along(u, u0, Math.max(IN.over, Math.min(1, u0 + 0.2)))));

    if (u < IN.shown) {
        // The drop appears on the icon
        const o = fadeTrack(u, 0, IN.shown, 0, 1, from, 'o');
        const start = from ? from : {centre: Ic};
        const k = ease(along(u, u0, IN.shown));
        return box(lerpPoint(start.centre, Ic, k), de, de, de / 2, g, o);
    }
    if (u < IN.over) {
        const resumedArc = !!from && u0 >= IN.shown && u0 < IN.over;
        const s = arcShare(u, IN.shown, IN.over, from);
        const p0 = resumedArc ? from.centre : Ic;
        const path = arc(p0, Fc, Fc, p0, UP.reach, UP.bow);
        if (u < IN.middle) {
            // Rising, the drop swells
            const {t, resumed} = part(u, IN.shown, IN.middle, from);
            const start = resumed ? from : {w: de, h: de, r: de / 2};
            const k = ease(t);
            return box(path(s), lerp(start.w, dm, k), lerp(start.h, dm, k), lerp(start.r, dm / 2, k), g, 1);
        }
        // It opens into the window's shape, a little larger than the window
        const {t, resumed} = part(u, IN.middle, IN.over, from);
        const start = resumed ? from : {w: dm, h: dm, r: dm / 2};
        const k = ease(t);
        return box(path(s), lerp(start.w, 1.04 * W, k), lerp(start.h, 1.04 * H, k), lerp(start.r, 1.04 * R0, k),
            g, 1);
    }
    // Settling: a little smaller, then itself
    if (u < IN.under) {
        const {t, resumed} = part(u, IN.over, IN.under, from);
        const start = resumed ? from : {centre: Fc, w: 1.04 * W, h: 1.04 * H, r: 1.04 * R0};
        const k = ease(t);
        return box(lerpPoint(start.centre, Fc, k), lerp(start.w, 0.99 * W, k), lerp(start.h, 0.99 * H, k),
            lerp(start.r, 0.99 * R0, k), g, 1);
    }
    const {t, resumed} = part(u, IN.under, 1, from);
    const start = resumed ? from : {centre: Fc, w: 0.99 * W, h: 0.99 * H, r: 0.99 * R0};
    const k = ease(t);
    return box(lerpPoint(start.centre, Fc, k), lerp(start.w, W, k), lerp(start.h, H, k), lerp(start.r, R0, k),
        g, 1);
}

// How much of the window is drawn as itself (its own corners and its shadow) rather than as the drop: all of it at
// rest, so that neither end of a flight shows a step. It goes in the first moments of the way out and comes back as
// the window settles.
const PLAIN_OUT = OUT.circle / 2;

export function minimizeAt(u, geo, from = null) {
    const state = minimizeShape(u, geo, from);
    const u0 = from?.u0 ?? 0;
    const k0 = from ? from.plain ?? 0 : 1;
    state.plain = k0 * (1 - ease(along(u, u0, u0 + PLAIN_OUT)));
    return state;
}

export function unminimizeAt(u, geo, from = null) {
    const state = unminimizeShape(u, geo, from);
    const u0 = from?.u0 ?? 0;
    if (from && u0 >= IN.under) {
        state.plain = lerp(from.plain ?? 0, 1, ease(along(u, u0, 1)));
    } else {
        // What a flight that turned round still showed of the window goes first
        const left = from ? (from.plain ?? 0) * (1 - ease(along(u, u0, Math.min(IN.under, u0 + PLAIN_OUT)))) : 0;
        state.plain = Math.max(left, ease(along(u, IN.under, 1)));
    }
    return state;
}

// A fade in place, while motion is reduced: the opacity at u, from o0
export function fadeAt(u, out, o0 = out ? 1 : 0) {
    const t = ease(clamp01(u));
    return out ? o0 * (1 - t) : o0 + (1 - o0) * t;
}

// How far along its arc a drop is at u (0 at the window, 1 at the icon), for a flight that turns round part way
export function downShare(u) {
    return easeInOut(along(u, OUT.small, OUT.landed));
}

export function upShare(u) {
    return easeInOut(along(u, IN.shown, IN.over));
}

// Where a flight that is turned round at `last` (the state it last drew, with its kind and u) starts again, as
// {kind, u0, from, scale}: scale multiplies the time left of the new flight
export function turnRound(last) {
    const {kind, u} = last;
    const state = {centre: last.centre, w: last.w, h: last.h, r: last.r, g: last.g, o: last.o, plain: last.plain ?? 0};
    if (kind === 'minimize') {
        if (u >= OUT.landed)
            return {kind: 'unminimize', u0: 0, from: {...state, u0: 0}, scale: 1};
        if (u >= OUT.small) {
            return {kind: 'unminimize', u0: IN.shown, from: {...state, u0: IN.shown},
                scale: Math.max(0.4, downShare(u))};
        }
        // Still gathering: straight to the settle
        return {kind: 'unminimize', u0: IN.middle, from: {...state, u0: IN.middle},
            scale: Math.max(0.3, u / OUT.small)};
    }
    if (kind === 'unminimize' || kind === 'open') {
        if (u < IN.shown)
            return {kind: 'minimize', u0: OUT.landed, from: {...state, u0: OUT.landed}, scale: 1};
        if (u < IN.over) {
            return {kind: 'minimize', u0: OUT.small, from: {...state, u0: OUT.small},
                scale: Math.max(0.4, 1 - upShare(u))};
        }
        return {kind: 'minimize', u0: 0, from: {...state, u0: 0}, scale: 1};
    }
    if (kind === 'fade-out')
        return {kind: 'fade-in', u0: 0, from: {...state, u0: 0}, scale: Math.max(0.05, 1 - state.o)};
    return {kind: 'fade-out', u0: 0, from: {...state, u0: 0}, scale: Math.max(0.05, state.o)};
}
