// Keeping text readable on the glass. Once for each new frosted picture, never per frame, the picture is read back
// small (frost.js) and its brightest and darkest parts found, as the glass would show them; for each surface the veil
// of the theme's background over the picture is then made just thick enough that its text reads at 4.5:1 (WCAG 2)
// against the glass behind it, wherever on the wallpaper that is. Where the variant's own veil is enough, it stays.
// Colours are [r, g, b] from 0 to 1, in sRGB, blended as the shader blends them.

const WHITE = [1, 1, 1];
// The share of the samples darker than the one taken as the brightest, and as the darkest
const HIGH = 0.95;
const LOW = 0.05;
const READABLE = 4.5;

export const mix = (x, y, t) => [x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t];

const linear = v => v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;

export function luminance(c) {
    return 0.2126 * linear(c[0]) + 0.7152 * linear(c[1]) + 0.0722 * linear(c[2]);
}

export function contrast(x, y) {
    const a = luminance(x);
    const b = luminance(y);
    return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

// The picture as a material shows it before its veil: saturation, brightness and white (lens.glsl's tone)
export function tone(c, material) {
    const l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    const toned = c.map(v => Math.min(Math.max(l + (v - l) * material.sat, 0), 1) * material.dim);
    return mix(toned, WHITE, material.tint);
}

// The brightest and the darkest of the samples (a flat array of r, g, b from 0 to 1), as the material shows them
export function extremes(samples, material) {
    const count = Math.floor(samples.length / 3);
    if (!count)
        return null;
    const toned = [];
    for (let i = 0; i < count; i++) {
        const c = tone([samples[3 * i], samples[3 * i + 1], samples[3 * i + 2]], material);
        toned.push({c, y: luminance(c)});
    }
    toned.sort((a, b) => a.y - b.y);
    return {
        hi: toned[Math.floor(HIGH * (count - 1))].c,
        lo: toned[Math.floor(LOW * (count - 1))].c,
    };
}

// The veil to draw: text, ground and the picture's extremes as above, base the variant's own veil, allowance the
// white light the text may count on. Light text is read against the brightest part, with the light over it; dark
// text against the darkest, where white light would only help. fill, [r, g, b, alpha], is what St draws over the
// glass when the surface has a background of its own (a bar piece of the islands or neon style): the veil under it
// is then only as thick as the fill leaves wanting.
export function veil({text, ground, hi, lo, base, allowance, fill = null}) {
    const light = luminance(text) > luminance(ground);
    const behind = light ? hi : lo;
    const composite = a => {
        const veiled = mix(behind, ground, a);
        const lit = light ? mix(veiled, WHITE, allowance) : veiled;
        return fill ? mix(lit, fill, fill[3]) : lit;
    };
    const readable = a => contrast(text, composite(a)) >= READABLE;
    if (readable(base))
        return base;
    if (!readable(1))
        return 1;
    let low = base;
    let high = 1;
    for (let step = 0; step < 24; step++) {
        const middle = (low + high) / 2;
        if (readable(middle))
            high = middle;
        else
            low = middle;
    }
    return Math.min(1, Math.ceil(high * 200 - 1e-9) / 200);
}
