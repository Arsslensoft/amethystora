// Amethystora's Soft sounds: the desktop's own sound theme, lower and rounder, for a quiet room. Each sound is the
// design board's Soft set written out as arithmetic (plans/beyond/spec-look.md 1.7): soft triangle tones an octave
// down, each with its own swell and fade and a gentle low-pass, and for the screenshot and the trash a short burst of
// filtered noise. Nothing is recorded and nothing is random: the noise comes from a seeded generator, so two runs
// write byte-identical files.
//
// Three more sets play the same events from the same notes in other voices (round 6's R6-04, plans/ui-ideas/R6Sound):
// Glass, bright short chimes (sine partials at a bell's inharmonic ratios, each fading fast); Wood, warm knocks and taps
// (a struck bar: a low sine and its fourth partial, a quick drop of pitch at the strike and a click of filtered noise);
// Retro, eight-bit blips (square and narrow pulse waves, held flat and cut off, the notes stepped). Each is a theme of
// its own beside Soft's, amethystora-glass, -wood and -retro, falling through to the freedesktop theme the same way.
//
// The files are 48 kHz, 16-bit, mono WAV, which every sound server plays without a decoder. No file is normalised on
// its own, so that the quiet ones stay quiet beside the loud ones; the script fails if any peak goes above -1 dBFS.
// Every other event name falls through Inherits=freedesktop to the freedesktop theme.
//
//   node branding/sounds.mjs    writes system_files/shared/usr/share/sounds/amethystora (index.theme and stereo/),
//                               amethystora-glass, -wood and -retro, and prints each file's length, peak and RMS
import {mkdirSync, rmSync, writeFileSync} from 'node:fs';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const OUT = join(ROOT, 'system_files/shared/usr/share/sounds/amethystora');

const RATE = 48000;
// The board plays the Soft set an octave below the notes it names
const LOW = 0.5;
// The board's output gain at full volume
const OUTPUT = 0.5;
// -1 dBFS
const MOST = 0.891;

// A tone: frequency, start and length in seconds, and where it glides to (if it does)
const T = (f, at, dur, glide = 0) => ({kind: 'tone', f, at, dur, glide});
// A burst of noise: start, length and the middle of its band
const N = (at, dur, freq) => ({kind: 'noise', at, dur, freq});

// In the board's order, which seeds the noise
const EVENTS = [
    ['message-new-instant', [T(1318.5, 0, 0.35), T(1975.5, 0.09, 0.5)]],
    ['message-new-instant-phone', [T(1046.5, 0, 0.22), T(1318.5, 0.08, 0.22), T(1568, 0.16, 0.42)]],
    ['power-plug', [T(587, 0, 0.5, 1175), T(1175, 0.32, 0.45)]],
    ['screen-capture', [N(0, 0.09, 3200), N(0.1, 0.12, 2200), T(2349, 0.02, 0.12)]],
    ['trash-empty', [T(880, 0, 0.45, 294), N(0.12, 0.35, 900)]],
    ['audio-volume-change', [T(1760, 0, 0.07)]],
    ['dialog-error', [T(440, 0, 0.22), T(349, 0.16, 0.38)]],
    ['desktop-login', [T(523, 0, 0.6), T(659, 0.12, 0.6), T(784, 0.24, 0.6), T(1046.5, 0.36, 0.9)]],
];
// The same bytes under a second name: GTK's error bell rings Mutter's, which plays this one
const COPIES = [['bell-window-system', 'dialog-error']];

// A biquad filter from the RBJ cookbook, run over a whole buffer from rest
function biquad(input, b0, b1, b2, a0, a1, a2) {
    const out = new Float64Array(input.length);
    let x1 = 0, x2 = 0, y1 = 0, y2 = 0;
    for (let i = 0; i < input.length; i++) {
        const x = input[i];
        const y = (b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2) / a0;
        x2 = x1;
        x1 = x;
        y2 = y1;
        y1 = y;
        out[i] = y;
    }
    return out;
}

// A low-pass whose Q is in decibels, as a web browser's is
function lowpass(input, f0, qDb) {
    const w0 = 2 * Math.PI * f0 / RATE;
    const cos = Math.cos(w0);
    const alpha = Math.sin(w0) / (2 * Math.pow(10, qDb / 20));
    return biquad(input, (1 - cos) / 2, 1 - cos, (1 - cos) / 2, 1 + alpha, -2 * cos, 1 - alpha);
}

// A band-pass with a peak of 0 dB, Q as a plain number
function bandpass(input, f0, q) {
    const w0 = 2 * Math.PI * f0 / RATE;
    const alpha = Math.sin(w0) / (2 * q);
    return biquad(input, alpha, 0, -alpha, 1 + alpha, -2 * Math.cos(w0), 1 - alpha);
}

// An exponential ramp from v0 at t0 to v1 at t1
const ramp = (v0, v1, t0, t1, t) => v0 * Math.pow(v1 / v0, (t - t0) / (t1 - t0));

function tone(part, length) {
    const signal = new Float64Array(length);
    const start = Math.round(part.at * RATE);
    const f = part.f * LOW;
    const g = part.glide * LOW;
    const bend = 0.8 * part.dur;
    let phase = 0;
    for (let i = start; i < length; i++) {
        const t = (i - start) / RATE;
        if (t >= part.dur)
            break;
        // A naive triangle that starts at 0, rising
        const wave = 4 * Math.abs(((phase + 0.75) % 1) - 0.5) - 1;
        const envelope = t < 0.018 ? ramp(0.0001, 0.42, 0, 0.018, t) : ramp(0.42, 0.0001, 0.018, part.dur, t);
        signal[i] = wave * envelope;
        const freq = g ? f * Math.pow(g / f, Math.min(t, bend) / bend) : f;
        phase = (phase + freq / RATE) % 1;
    }
    return lowpass(signal, 1600, 0.4);
}

// The noise generator: mulberry32, a few lines of 32-bit arithmetic that give the same numbers everywhere
function mulberry32(seed) {
    let a = seed | 0;
    return () => {
        a = (a + 0x6D2B79F5) | 0;
        let t = Math.imul(a ^ (a >>> 15), 1 | a);
        t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
        return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
}

function noise(part, length, seed) {
    const signal = new Float64Array(length);
    const start = Math.round(part.at * RATE);
    const n = Math.floor(RATE * part.dur);
    const random = mulberry32(seed);
    for (let i = 0; i < n && start + i < length; i++)
        signal[start + i] = (random() * 2 - 1) * Math.pow(1 - i / n, 3);
    return bandpass(signal, part.freq, 0.9).map(x => x * 0.35);
}

// --- The other sets' voices ----------------------------------------------------------------------------------------

// A plain exponential fade from a short rise: the strike of a bell or a bar
function struck(t, dur, rise, top) {
    return t < rise ? top * t / rise : top * Math.pow(0.0001 / top, (t - rise) / (dur - rise));
}

// Glass: the note an octave above Soft's, sine partials at 1, 2.76 and 5.40 times it, the higher ones fading faster
function glassTone(part, length) {
    const signal = new Float64Array(length);
    const start = Math.round(part.at * RATE);
    const partials = [[1, 1, 1], [2.76, 0.32, 0.55], [5.4, 0.12, 0.3]];
    const dur = Math.min(part.dur, 0.6);
    for (let i = start; i < length; i++) {
        const t = (i - start) / RATE;
        if (t >= dur)
            break;
        const f = part.glide ? part.f * Math.pow(part.glide / part.f, Math.min(t / (0.8 * dur), 1)) : part.f;
        let x = 0;
        for (const [ratio, gain, life] of partials)
            x += gain * Math.sin(2 * Math.PI * f * ratio * t) * struck(t, dur * life, 0.002, 1);
        signal[i] = 0.26 * x;
    }
    return signal;
}

function glassNoise(part, length, seed) {
    // A glassy tick: noise high up, very short
    return noise({...part, dur: Math.min(part.dur, 0.05), freq: part.freq * 2}, length, seed).map(x => x * 0.8);
}

// Wood: a struck bar at Soft's notes, a low sine with a fourth partial, the pitch dropping a little as it is struck
function woodTone(part, length) {
    const signal = new Float64Array(length);
    const start = Math.round(part.at * RATE);
    const dur = Math.min(part.dur, 0.32);
    let phase = 0, phase4 = 0;
    for (let i = start; i < length; i++) {
        const t = (i - start) / RATE;
        if (t >= dur)
            break;
        const glide = part.glide ? Math.pow(part.glide / part.f, Math.min(t / dur, 1)) : 1;
        const f = part.f * LOW * glide * (1 + 0.06 * Math.exp(-t / 0.004));
        phase = (phase + f / RATE) % 1;
        phase4 = (phase4 + 3.93 * f / RATE) % 1;
        signal[i] = 0.5 * Math.sin(2 * Math.PI * phase) * struck(t, dur, 0.001, 1) +
            0.16 * Math.sin(2 * Math.PI * phase4) * struck(t, dur * 0.35, 0.001, 1);
    }
    // The strike itself: a click of noise round the bar's note
    const click = noise({at: part.at, dur: 0.012, freq: Math.min(part.f * 2, 4000)}, length,
        Math.round(part.f * 10)).map(x => x * 1.4);
    return lowpass(signal.map((x, i) => x + click[i]), 3200, 0.2);
}

function woodNoise(part, length, seed) {
    // A knock: lower and shorter than Soft's swish
    return noise({...part, dur: Math.min(part.dur, 0.08), freq: part.freq / 3}, length, seed).map(x => x * 1.6);
}

// Retro: square waves at Soft's notes, an octave up, notes stepped in sixteenths, a narrow pulse for glides
function retroTone(part, length) {
    const signal = new Float64Array(length);
    const start = Math.round(part.at * RATE);
    const dur = Math.min(part.dur, 0.3);
    const step = 0.04;
    let phase = 0;
    for (let i = start; i < length; i++) {
        const t = (i - start) / RATE;
        if (t >= dur)
            break;
        // A glide goes in steps, as an old sound chip's did
        const n = Math.floor(t / step);
        const f = part.glide ? part.f * Math.pow(part.glide / part.f, Math.min(n * step / dur, 1)) : part.f;
        phase = (phase + f / RATE) % 1;
        const duty = part.glide ? 0.25 : 0.5;
        const level = t < 0.003 ? t / 0.003 : t > dur - 0.01 ? Math.max(0, (dur - t) / 0.01) : 1 - 0.5 * t / dur;
        signal[i] = (phase < duty ? 1 : -1) * 0.14 * level;
    }
    return lowpass(signal, 6000, 0);
}

// Noise as an old chip made it: a seeded value held for a few samples, a short burst
function retroNoise(part, length, seed) {
    const signal = new Float64Array(length);
    const start = Math.round(part.at * RATE);
    const n = Math.floor(RATE * Math.min(part.dur, 0.08));
    const random = mulberry32(seed);
    let held = 0;
    for (let i = 0; i < n && start + i < length; i++) {
        if (i % 12 === 0)
            held = random() < 0.5 ? -1 : 1;
        signal[start + i] = held * 0.12 * (1 - i / n);
    }
    return signal;
}

const SOFT = {tone, noise};
const SETS = [
    {dir: 'amethystora-glass', name: 'Amethystora Glass', comment: 'Bright, short chimes', voice: {tone: glassTone, noise: glassNoise}},
    {dir: 'amethystora-wood', name: 'Amethystora Wood', comment: 'Warm knocks and taps', voice: {tone: woodTone, noise: woodNoise}},
    {dir: 'amethystora-retro', name: 'Amethystora Retro', comment: 'Eight-bit blips', voice: {tone: retroTone, noise: retroNoise}},
];

function render(index, parts, voice = SOFT) {
    const end = Math.max(...parts.map(p => p.at + p.dur)) + 0.05;
    // A whole sample, past what floating point adds to an exact number of them
    const length = Math.ceil(end * RATE - 1e-6);
    const mix = new Float64Array(length);
    let noises = 0;
    for (const part of parts) {
        const signal = part.kind === 'tone' ? voice.tone(part, length)
            : voice.noise(part, length, 1000 * index + noises++ + 1);
        for (let i = 0; i < length; i++)
            mix[i] += signal[i];
    }
    const samples = new Int16Array(length);
    let peak = 0, square = 0;
    for (let i = 0; i < length; i++) {
        const x = Math.max(-1, Math.min(1, mix[i] * OUTPUT));
        peak = Math.max(peak, Math.abs(x));
        square += x * x;
        samples[i] = Math.round(x * 32767);
    }
    return {samples, peak, rms: Math.sqrt(square / length)};
}

// The canonical 44-byte header, then the samples, little-endian
function wav(samples) {
    const data = samples.length * 2;
    const file = Buffer.alloc(44 + data);
    file.write('RIFF', 0, 'ascii');
    file.writeUInt32LE(36 + data, 4);
    file.write('WAVE', 8, 'ascii');
    file.write('fmt ', 12, 'ascii');
    file.writeUInt32LE(16, 16);
    file.writeUInt16LE(1, 20);
    file.writeUInt16LE(1, 22);
    file.writeUInt32LE(RATE, 24);
    file.writeUInt32LE(RATE * 2, 28);
    file.writeUInt16LE(2, 32);
    file.writeUInt16LE(16, 34);
    file.write('data', 36, 'ascii');
    file.writeUInt32LE(data, 40);
    for (let i = 0; i < samples.length; i++)
        file.writeInt16LE(samples[i], 44 + 2 * i);
    return file;
}

const INDEX = `[Sound Theme]
Name=Amethystora
Comment=Amethystora's Soft sounds, synthesised by branding/sounds.mjs
Inherits=freedesktop
Directories=stereo

[stereo]
OutputProfile=stereo
`;

let failed = false;
const dB = x => (20 * Math.log10(x)).toFixed(1);

// One set: its folder, its index.theme, the eight events and the copies
function writeSet(out, index, voice) {
    rmSync(out, {recursive: true, force: true});
    mkdirSync(join(out, 'stereo'), {recursive: true});
    writeFileSync(join(out, 'index.theme'), index);
    const written = new Map();
    for (const [i, [name, parts]] of EVENTS.entries()) {
        const {samples, peak, rms} = render(i, parts, voice);
        const file = wav(samples);
        writeFileSync(join(out, 'stereo', `${name}.wav`), file);
        written.set(name, file);
        console.log(`${name.padEnd(26)} ${(samples.length / RATE).toFixed(3)} s  peak ${peak.toFixed(3)} (${dB(peak)} dBFS)` +
            `  RMS ${rms.toFixed(4)} (${dB(rms)} dBFS)  ${file.length} bytes`);
        if (peak > MOST) {
            console.error(`${name}: its peak is above -1 dBFS`);
            failed = true;
        }
    }
    for (const [name, of] of COPIES) {
        writeFileSync(join(out, 'stereo', `${name}.wav`), written.get(of));
        console.log(`${name.padEnd(26)} the same as ${of}`);
    }
}

writeSet(OUT, INDEX, SOFT);
for (const set of SETS) {
    console.log(`-- ${set.dir}`);
    writeSet(join(OUT, '..', set.dir), INDEX.replace('Name=Amethystora', `Name=${set.name}`)
        .replace(/^Comment=.*\n/m, `Comment=Amethystora's ${set.name.split(' ')[1]} sounds (${set.comment.toLowerCase()}), synthesised by branding/sounds.mjs\n`),
    set.voice);
}
// What `ame desktop sounds off` points the theme at: GNOME's volume keys play their step whatever event-sounds says,
// so only a theme whose step is silent stops it
const SILENT = join(OUT, '..', 'amethystora-silent');
rmSync(SILENT, {recursive: true, force: true});
mkdirSync(join(SILENT, 'stereo'), {recursive: true});
writeFileSync(join(SILENT, 'index.theme'), INDEX.replace('Name=Amethystora', 'Name=Amethystora silent')
    .replace(/^Comment=.*\n/m, "Comment=The volume keys' step, silent, for ame desktop sounds off\n"));
writeFileSync(join(SILENT, 'stereo', 'audio-volume-change.wav'), wav(new Array(480).fill(0)));
if (failed)
    process.exit(1);
