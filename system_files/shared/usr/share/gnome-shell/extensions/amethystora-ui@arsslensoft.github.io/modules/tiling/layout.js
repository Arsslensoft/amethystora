// The arithmetic of a strip, with no GNOME in it: where each window of a workspace goes, how far the strip
// scrolls to show one, and the steps the sizing keys go through. Every size is in pixels of the screen.

// The space between two windows, and between the windows and the edges of the screen: the settings' (config.js
// sets them through configure), the image's until then
export let GAP = 8;
export let MARGIN = 8;
export let MARGIN_TOP = 8;
export let MARGIN_BOTTOM = 8;
// A column never comes closer than this to both edges at once
export let MIN_MARGIN = Math.min(15, MARGIN);
// The least a window in a column of several keeps of its height
export const MIN_HEIGHT = 50;
// How much wider or narrower a window has to get for a step to count, for apps that size in increments
export const SLACK = 30;
// The widths and heights the cycle keys step through, as shares of what is available (above 1, in pixels)
export const DEFAULT_STEPS = [0.38195, 0.5, 0.61804];
export let WIDTH_STEPS = DEFAULT_STEPS;
export let HEIGHT_STEPS = DEFAULT_STEPS;

export function configure(values) {
    ({gap: GAP, margin: MARGIN, marginTop: MARGIN_TOP, marginBottom: MARGIN_BOTTOM} = values);
    MIN_MARGIN = Math.min(15, MARGIN);
    WIDTH_STEPS = values.widthSteps;
    HEIGHT_STEPS = values.heightSteps;
}

const pixels = (step, space) => step <= 1 ? Math.floor(step * space) : Math.round(step);

export const Mode = {PLAIN: 0, CENTRE: 1, EDGE: 2};

const clamp = (value, low, high) => Math.max(low, Math.min(high, value));

// The part of a monitor's work area the windows of a strip stand in
export function usable(area) {
    return {
        x: area.x,
        y: area.y + MARGIN_TOP,
        width: area.width,
        height: Math.max(MIN_HEIGHT, area.height - MARGIN_TOP - MARGIN_BOTTOM),
    };
}

// The widest a column can be
export function maxWidth(area) {
    return area.width - 2 * MIN_MARGIN;
}

// The width two windows of complementary steps share side by side
export function available(area) {
    return area.width - 2 * MARGIN - GAP;
}

// Heights of the windows of one column, top to bottom: `wishes` is each window's last height (or 0 for none),
// `keep` the index of the one that keeps its height, or -1
export function heights(total, wishes, keep) {
    const n = wishes.length;
    const space = total - GAP * (n - 1);
    if (n === 1)
        return [space];
    if (wishes.every(h => !h))
        return equal(space, n);
    const out = new Array(n).fill(0);
    let rest = space;
    if (keep >= 0 && wishes[keep]) {
        out[keep] = clamp(wishes[keep], MIN_HEIGHT, space - MIN_HEIGHT * (n - 1));
        rest -= out[keep];
    }
    const others = wishes.map((h, i) => i).filter(i => !(keep >= 0 && wishes[keep] && i === keep));
    const weight = others.reduce((sum, i) => sum + (wishes[i] || space / n), 0);
    let given = 0;
    others.forEach((i, k) => {
        const share = k === others.length - 1
            ? rest - given
            : Math.round(rest * (wishes[i] || space / n) / weight);
        out[i] = Math.max(MIN_HEIGHT, share);
        given += out[i];
    });
    // The shares kept at their least took from the window that keeps its height
    const over = out.reduce((a, b) => a + b, 0) - space;
    if (over > 0 && keep >= 0)
        out[keep] = Math.max(MIN_HEIGHT, out[keep] - over);
    return out;
}

function equal(space, n) {
    const each = Math.floor(space / n);
    return Array.from({length: n}, (_, i) => i === n - 1 ? space - each * (n - 1) : each);
}

// Where every column of a strip stands, relative to the strip's left end: [{x, width}], and the strip's width
export function columns(widths) {
    let x = GAP;
    const out = widths.map(width => {
        const column = {x, width};
        x += width + GAP;
        return column;
    });
    return {columns: out, width: x};
}

// The strip's scroll (the screen x of its left end, from the work area's left edge) that centres it when it is
// narrower than the screen or has a single column; null when it covers the screen and scrolls freely
export function centred(area, strip) {
    if (!strip.columns.length)
        return 0;
    if (strip.columns.length === 1)
        return Math.round((area.width - strip.columns[0].width) / 2) - GAP;
    if (strip.width <= area.width)
        return Math.round((area.width - strip.width) / 2);
    return null;
}

// Where a column's left edge lands on screen (from the work area's left) when it gets the focus, by the
// workspace's focus mode. `from` is where it is now, or where it was when last left; `index` and `count` place it
// in the strip.
export function placeFocused(area, column, from, mode, index, count) {
    const {width} = column;
    const middle = Math.round((area.width - width) / 2);
    if (mode === Mode.CENTRE)
        return middle;
    if (mode === Mode.EDGE) {
        if (count === 1)
            return middle;
        if (index === 0)
            return MARGIN;
        if (index === count - 1)
            return area.width - MARGIN - width;
        return from + width / 2 < area.width / 2 ? MARGIN : area.width - MARGIN - width;
    }
    // A window nearly as wide as the screen is centred
    if (width > 0.9 * (area.width - 2 * (MARGIN + GAP)))
        return middle;
    if (from + width + MARGIN > area.width)
        return area.width - MARGIN - width;
    if (from < MARGIN)
        return MARGIN;
    return from;
}

// The next width or height in the cycle: forward the smallest step more than SLACK beyond `now`, back the
// largest step more than SLACK below it, going round at either end
export function nextStep(now, space, forward, steps = WIDTH_STEPS) {
    const sizes = steps.map(step => pixels(step, space)).sort((a, b) => a - b);
    if (forward)
        return sizes.find(size => size > now + SLACK) ?? sizes[0];
    return [...sizes].reverse().find(size => size < now - SLACK) ?? sizes[sizes.length - 1];
}

// A size one tenth of `space` bigger or smaller, from the nearest whole tenth
export function stepSize(now, space, grow) {
    const step = Math.max(1, Math.floor(space / 10));
    const steps = Math.round(now / step) + (grow ? 1 : -1);
    return clamp(steps * step, step, space);
}
