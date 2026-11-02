// Music you can see (R7-21): the sums, free of GNOME Shell's imports so that they run under node (test-pill-logic.mjs). The
// album's colour from a tiny copy of its cover, the same colour moved until the theme's text reads on it (or nothing, and
// the theme's own plate stays), and the height of four bars from a slice of sound.

function luminance([r, g, b]) {
    const part = v => {
        const c = v / 255;
        return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    };
    return 0.2126 * part(r) + 0.7152 * part(g) + 0.0722 * part(b);
}

export function contrast(a, b) {
    const [high, low] = [luminance(a), luminance(b)].sort((x, y) => y - x);
    return (high + 0.05) / (low + 0.05);
}

// The colour of a cover: the average of its pixels, each counted by how colourful it is, so that a black sleeve with a red
// title is red. pixels: bytes, rowstride bytes a row, channels 3 or 4. Null for a grey picture, which has no colour to take.
export function albumColour(pixels, width, height, rowstride, channels) {
    let [r, g, b, weight] = [0, 0, 0, 0];
    for (let y = 0; y < height; y++) {
        for (let x = 0; x < width; x++) {
            const at = y * rowstride + x * channels;
            const [pr, pg, pb] = [pixels[at], pixels[at + 1], pixels[at + 2]];
            const most = Math.max(pr, pg, pb);
            const chroma = most - Math.min(pr, pg, pb);
            const w = chroma * chroma;
            r += pr * w;
            g += pg * w;
            b += pb * w;
            weight += w;
        }
    }
    // Under this the picture is grey, whatever its average says
    if (weight / (width * height) < 400)
        return null;
    return [r / weight, g / weight, b / weight].map(Math.round);
}

// The colour moved toward black (light text) or white (dark text) until the text reads on it at 4.5:1, with the plate's
// own see-through counted as the worst case; null when that takes it too far from the album's colour
const MOST_STEPS = 12;

export function plateTint(colour, text) {
    if (!colour)
        return null;
    const darken = luminance(text) > 0.4;
    let now = colour;
    for (let step = 0; step <= MOST_STEPS; step++) {
        if (contrast(now, text) >= 4.5)
            return now.map(Math.round);
        now = now.map(v => (darken ? v * 0.88 : v + (255 - v) * 0.12));
    }
    return null;
}

// How high four bars stand for a slice of sound (signed 16-bit samples), each its quarter of the slice, 0.15 to 1. The
// square root is how a meter looks right to the eye.
export function barsOf(samples) {
    const quarter = Math.floor(samples.length / 4);
    const bars = [];
    for (let i = 0; i < 4; i++) {
        let sum = 0;
        for (let j = i * quarter; j < (i + 1) * quarter; j++)
            sum += samples[j] * samples[j];
        const rms = quarter > 0 ? Math.sqrt(sum / quarter) / 32768 : 0;
        bars.push(Math.min(1, 0.15 + Math.sqrt(rms) * 1.2));
    }
    return bars;
}
