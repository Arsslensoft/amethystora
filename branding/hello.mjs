// The first login's greeting (Amethystora UI's hello module): the word for hello in each language Amethystora speaks,
// drawn in code as one pen line after another, so that the shell can write it stroke by stroke with one small shader and
// no drawing per frame.
//
//   node branding/hello.mjs              writes the extension's hello/<language>.png
//   node branding/hello.mjs --preview D  also writes each word as it looks when finished into folder D, to look at
//
// Each picture holds no colour but three measures, one to a channel: red, when the pen gets here (0 to 1 along the word);
// green, how much of the ink is here; blue, how much of its glow. The strokes are written here, in order, the way a
// hand writes them (Arabic from the right, a j's dot last), never traced from a font. A new language is a new entry in
// WORDS, from the letters below or strokes of its own, and a line in the module's table.

import {deflateSync} from 'node:zlib';
import {mkdirSync, writeFileSync} from 'node:fs';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';

const OUT = join(dirname(fileURLToPath(import.meta.url)), '..', 'system_files/shared/usr/share/gnome-shell/extensions',
    'amethystora-ui@arsslensoft.github.io/hello');

// The picture, which the module draws at this logical size on a large screen
export const WIDTH = 900;
export const HEIGHT = 420;
const SCALE = 2.25; // Picture pixels per unit of the letters below
const BASELINE = 268; // From the top of the picture
const PEN = 8; // The line's width, in picture pixels
const GLOW = 30; // How far its glow reaches, in picture pixels
const LIFT = 0.05; // Lifting the pen takes this much of the word's time

// One letter in units, y up, the baseline at 0, x-height 40, a tall letter 96. `in` is the rise from the line that a
// letter starts on when the one before it ends low, left out when it ends high; a null in a list is a corner, where the
// pen stops and turns. `high` says that it ends high. `out` is its width.
const RISE = [[0, 8], [8, 18]];
// A dot, as the pen makes one: a tiny round
const dot = (x, y) => [0, 1, 2, 3, 4, 5, 6].map(k => [x + 1.5 * Math.cos(k * Math.PI / 3), y + 1.5 * Math.sin(k * Math.PI / 3)]);
const TALL = [[0, 8], [12, 24], [24, 58], [27, 84], [21, 96], [14, 88], [12, 58]];
const LETTERS = {
    a: {in: [[0, 8]], body: [[12, 28], [27, 38], [16, 40], [7, 28], [7, 10], [17, 1], [27, 10], [31, 38], null, [30, 14], [36, 2], [44, 1], [52, 9]], out: 52},
    b: {in: [], body: [...TALL, [12, 12], [21, 1], [33, 6], [36, 22], [27, 37], [18, 32], [28, 29], [44, 31]], out: 44, high: true},
    e: {in: [], body: [[0, 8], [14, 16], [26, 28], [23, 40], [12, 38], [6, 22], [12, 6], [24, 0], [36, 9]], out: 36},
    h: {in: [], body: [...TALL, [11, 0], null, [14, 20], [24, 37], [33, 38], [37, 26], [37, 10], [44, 1], [52, 9]], out: 52},
    j: {in: RISE, body: [[16, 39], null, [16, 8], [13, -24], [6, -42], [0, -36], [7, -14], [24, 2], [36, 10]], out: 36, dots: [dot(16, 58)]},
    l: {in: [], body: [...TALL, [12, 22], [19, 3], [29, 0], [39, 9]], out: 39},
    n: {in: RISE, body: [[14, 38], null, [13, 0], null, [16, 20], [26, 37], [34, 37], [37, 26], [37, 10], [44, 1], [52, 9]], out: 52},
    o: {in: [[0, 9], [13, 29]], body: [[27, 38], [16, 40], [6, 28], [6, 12], [16, 1], [28, 4], [33, 20], [27, 37], [21, 32], [30, 29], [44, 31]], out: 44, high: true},
    r: {in: RISE, body: [[13, 39], null, [13, 0], null, [15, 20], [23, 35], [33, 36], [37, 30]], out: 38},
    u: {in: RISE, body: [[12, 38], null, [11, 14], [18, 2], [29, 5], [37, 38], null, [37, 12], [43, 2], [52, 9]], out: 52},
};

// Two strokes joined right to left, x growing to the right as everywhere, with the dot below the tooth last
const MARHABA = {
    width: 196,
    strokes: [
        // Meem's small loop, then ra's tail going below the line: they join, and ra joins nothing after it
        [[176, 2], [186, 6], [193, 18], [186, 30], [172, 27], [166, 14], [176, 2], [160, 1], [148, 3], [145, 8], null, [142, -8], [131, -26], [110, -36]],
        // Hah's cap, from its left end to the right and back down onto the line, the tooth of ba, and alef going up
        [[84, 37], [104, 35], [122, 30], null, [104, 15], [84, 3], [72, 1], [66, 4], [62, 15], null, [58, 3], [44, 1], [24, 1], [10, 1], null, [9, 88]],
        // Ba's dot
        dot(61, -15),
    ],
};

// The word's pen lines, each a list of points in units with its corners, from its letters
function writing(word) {
    if (typeof word !== 'string')
        return word;
    const line = [];
    const lifted = [];
    let x = 0;
    let high = false;
    for (const name of word) {
        const letter = LETTERS[name];
        const place = ([px, py]) => [x + px, py];
        // The new letter's first point takes over from the last one's end when they nearly meet, which would make a kink
        for (const point of [...(high ? [] : letter.in), ...letter.body]) {
            const end = line[line.length - 1];
            const next = point && place(point);
            if (next && end && Math.hypot(next[0] - end[0], next[1] - end[1]) < 4)
                line.pop();
            line.push(next);
        }
        for (const dot of letter.dots ?? [])
            lifted.push(dot.map(place));
        high = !!letter.high;
        x += letter.out;
    }
    return {width: x, strokes: [line, ...lifted]};
}

// One span of a Catmull-Rom curve through its points, sampled finely, so that a stroke passes through every point written
function curve(points, out) {
    for (let i = 0; i + 1 < points.length; i++) {
        const [p0, p1, p2, p3] = [points[Math.max(0, i - 1)], points[i], points[i + 1], points[Math.min(points.length - 1, i + 2)]];
        const span = Math.hypot(p2[0] - p1[0], p2[1] - p1[1]);
        const steps = Math.max(2, Math.ceil(span));
        for (let s = i === 0 ? 0 : 1; s <= steps; s++) {
            const t = s / steps;
            const c = (a, b, c1, d) => 0.5 * (2 * b + (c1 - a) * t + (2 * a - 5 * b + 4 * c1 - d) * t * t + (3 * b - a - 3 * c1 + d) * t * t * t);
            out.push([c(p0[0], p1[0], p2[0], p3[0]), c(p0[1], p1[1], p2[1], p3[1])]);
        }
    }
}

// A stroke as a fine line of picture pixels: its curves apart at each corner
function sampled({width, strokes}) {
    const left = (WIDTH - width * SCALE) / 2;
    return strokes.map(stroke => {
        const line = [];
        let run = [];
        for (const point of stroke) {
            if (point) {
                run.push(point);
            } else {
                curve(run, line);
                run = [run[run.length - 1]];
            }
        }
        curve(run, line);
        return line.map(([x, y]) => [left + x * SCALE, BASELINE - y * SCALE]);
    });
}

// The mean of the values within a square around each pixel, leaving out those below 0, which stay as they are where no value
// is near: a sliding sum along the rows and then the columns
function spread(values, radius) {
    const [sum, count] = [Float32Array.from(values, v => Math.max(v, 0)), Float32Array.from(values, v => v >= 0 ? 1 : 0)];
    for (const [lines, along, step, next] of [[HEIGHT, WIDTH, 1, WIDTH], [WIDTH, HEIGHT, WIDTH, 1]]) {
        for (const field of [sum, count]) {
            const row = new Float32Array(along);
            for (let line = 0; line < lines; line++) {
                for (let i = 0; i < along; i++)
                    row[i] = field[line * next + i * step];
                let total = 0;
                for (let i = -radius; i < along; i++) {
                    total += (row[i + radius] ?? 0) - (row[i - radius - 1] ?? 0);
                    if (i >= 0)
                        field[line * next + i * step] = total;
                }
            }
        }
    }
    return sum.map((total, k) => count[k] >= 0.5 ? total / count[k] : 0);
}

// The three measures of one word. Going along the pen's line, a pixel keeps the first moment the ink itself passes over
// it, so that a stroke crossing an earlier one leaves that one whole; outside the ink, the moment of the nearest line.
function measures(lines) {
    const size = WIDTH * HEIGHT;
    const nearest = new Float32Array(size).fill(1e9);
    const glowTime = new Float32Array(size);
    const inkTime = new Float32Array(size).fill(-1);

    const lengths = lines.map(line => line.slice(1).reduce((sum, p, i) => sum + Math.hypot(p[0] - line[i][0], p[1] - line[i][1]), 0));
    const whole = lengths.reduce((a, b) => a + b, 0);
    const pauses = (lines.length - 1) * LIFT;
    let at = 0;
    const reach = GLOW * 2.2;
    lines.forEach((line, index) => {
        let along = 0;
        line.forEach((point, i) => {
            if (i > 0)
                along += Math.hypot(point[0] - line[i - 1][0], point[1] - line[i - 1][1]);
            const time = at + (along / whole) * (1 - pauses);
            const x0 = Math.max(0, Math.floor(point[0] - reach));
            const x1 = Math.min(WIDTH - 1, Math.ceil(point[0] + reach));
            const y0 = Math.max(0, Math.floor(point[1] - reach));
            const y1 = Math.min(HEIGHT - 1, Math.ceil(point[1] + reach));
            for (let y = y0; y <= y1; y++) {
                for (let x = x0; x <= x1; x++) {
                    const k = y * WIDTH + x;
                    const d = Math.hypot(x + 0.5 - point[0], y + 0.5 - point[1]);
                    if (d < nearest[k]) {
                        nearest[k] = d;
                        glowTime[k] = time;
                    }
                    if (d < PEN / 2 + 2 && inkTime[k] < 0)
                        inkTime[k] = time;
                }
            }
        });
        at += (lengths[index] / whole) * (1 - pauses) + LIFT;
    });

    // Outside the ink the nearest line changes from pixel to pixel at a jump, where two strokes written at different moments
    // meet, and the glow would show that edge while the second waits to be written: the moments are spread over the glow
    for (const k of glowTime.keys())
        glowTime[k] = nearest[k] < reach ? glowTime[k] : -1;
    const glowTimes = spread(glowTime, Math.round(GLOW * 0.6));

    const data = Buffer.alloc(size * 3);
    for (let k = 0; k < size; k++) {
        const d = nearest[k];
        const ink = Math.min(1, Math.max(0, PEN / 2 + 0.5 - d));
        const glow = Math.exp(-((d / GLOW) ** 2)) * (d < reach ? 1 : 0);
        // Close to a line, its own moment: spread, a pixel beside a stroke still to come would take the moment of an earlier
        // one, and the glow would run ahead of the pen. Farther out, the spread one, which has no edge where two lines meet
        const own = Math.exp(-((d / (GLOW * 0.5)) ** 2));
        const time = inkTime[k] >= 0 ? inkTime[k] : glowTime[k] * own + glowTimes[k] * (1 - own);
        data[k * 3] = Math.round(Math.min(1, time) * 255);
        data[k * 3 + 1] = Math.round(ink * 255);
        data[k * 3 + 2] = Math.round(glow * 255);
    }
    return data;
}

// A PNG of three 8-bit channels, written here so that drawing needs nothing but Node
function png(data, width = WIDTH, height = HEIGHT) {
    const table = Array.from({length: 256}, (_, n) => {
        for (let k = 0; k < 8; k++)
            n = n & 1 ? 0xedb88320 ^ (n >>> 1) : n >>> 1;
        return n >>> 0;
    });
    const crc = buffer => (buffer.reduce((c, byte) => table[(c ^ byte) & 255] ^ (c >>> 8), 0xffffffff) ^ 0xffffffff) >>> 0;
    const chunk = (type, body) => {
        const named = Buffer.concat([Buffer.from(type), body]);
        const length = Buffer.alloc(4);
        length.writeUInt32BE(body.length);
        const sum = Buffer.alloc(4);
        sum.writeUInt32BE(crc(named));
        return Buffer.concat([length, named, sum]);
    };
    const header = Buffer.alloc(13);
    header.writeUInt32BE(width, 0);
    header.writeUInt32BE(height, 4);
    header.set([8, 2, 0, 0, 0], 8);
    const stride = width * 3;
    const rows = Buffer.alloc((stride + 1) * height);
    // Each row as its difference from the one above (PNG's filter 2): the measures change slowly, which packs far better
    for (let y = 0; y < height; y++) {
        const at = y * (stride + 1);
        rows[at] = 2;
        for (let x = 0; x < stride; x++)
            rows[at + 1 + x] = (data[y * stride + x] - (y ? data[(y - 1) * stride + x] : 0)) & 255;
    }
    return Buffer.concat([
        Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
        chunk('IHDR', header),
        chunk('IDAT', deflateSync(rows, {level: 9})),
        chunk('IEND', Buffer.alloc(0)),
    ]);
}

// The finished word on the greeting's dim ground, as the module's shader draws it
function preview(data) {
    const out = Buffer.alloc(data.length);
    for (let k = 0; k < WIDTH * HEIGHT; k++) {
        const [ink, glow] = [data[k * 3 + 1] / 255, data[k * 3 + 2] / 255];
        const shine = 0.45 * glow;
        const base = [20, 13, 34];
        const light = [179, 125, 249];
        const white = [250, 244, 255];
        for (let c = 0; c < 3; c++)
            out[k * 3 + c] = Math.min(255, (base[c] + (light[c] - base[c]) * shine) * (1 - ink) + white[c] * ink);
    }
    return png(out);
}

const WORDS = {en: 'hello', fr: 'bonjour', de: 'hallo', es: 'hola', ar: MARHABA};

const previewAt = process.argv.indexOf('--preview');
mkdirSync(OUT, {recursive: true});
if (previewAt >= 0)
    mkdirSync(process.argv[previewAt + 1], {recursive: true});
for (const [language, word] of Object.entries(WORDS)) {
    const data = measures(sampled(writing(word)));
    writeFileSync(join(OUT, `${language}.png`), png(data));
    if (previewAt >= 0)
        writeFileSync(join(process.argv[previewAt + 1], `${language}.png`), preview(data));
}
