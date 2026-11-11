// What Amethystora Clipboard records of a copy (pictures.js): a picture's header is read before anything decodes it,
// so a small file that declares gigabytes is skipped, as is one past the byte ceilings:
//   node build_files/shared/test-clipboard-pictures.mjs
// A failure throws, which makes the run exit with an error.

import {pictureSize, pictureFits, copyFits, MAX_PICTURE_BYTES, MAX_TEXT_BYTES} from
    '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-clipboard@arsslensoft.github.io/pictures.js';

const fail = what => { throw new Error(`test-clipboard-pictures: ${what}`); };
const eq = (got, want, what) => { if (JSON.stringify(got) !== JSON.stringify(want)) fail(`${what}: ${JSON.stringify(got)}, wanted ${JSON.stringify(want)}`); };
const bytes = (...parts) => Uint8Array.from(parts.flat());
const be32 = n => [n >>> 24, (n >>> 16) & 255, (n >>> 8) & 255, n & 255];
const le16 = n => [n & 255, n >> 8];
const text = s => [...s].map(c => c.charCodeAt(0));

const png = (w, h) => bytes([0x89], text('PNG\r\n'), [0x1a, 0x0a], be32(13), text('IHDR'), be32(w), be32(h), [8, 6, 0, 0, 0], new Array(8).fill(0));
const gif = (w, h) => bytes(text('GIF89a'), le16(w), le16(h), new Array(8).fill(0));
const jpeg = (w, h) => bytes([0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc4, 0, 3, 0, 0xff, 0xc0, 0, 17, 8],
    [h >> 8, h & 255, w >> 8, w & 255], [3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1], new Array(8).fill(0));
const webpX = (w, h) => bytes(text('RIFF'), [0, 0, 0, 0], text('WEBPVP8X'), [10, 0, 0, 0, 0, 0, 0, 0],
    [(w - 1) & 255, ((w - 1) >> 8) & 255, (w - 1) >> 16, (h - 1) & 255, ((h - 1) >> 8) & 255, (h - 1) >> 16]);
const webpL = (w, h) => {
    const bits = ((w - 1) | ((h - 1) << 14)) >>> 0;
    return bytes(text('RIFF'), [0, 0, 0, 0], text('WEBPVP8L'), [5, 0, 0, 0, 0x2f], [bits & 255, (bits >>> 8) & 255, (bits >>> 16) & 255, bits >>> 24], new Array(8).fill(0));
};
const svg = attrs => new TextEncoder().encode(`<?xml version="1.0"?>\n<svg xmlns="http://www.w3.org/2000/svg" ${attrs}><rect/></svg>`);

eq(pictureSize(png(30000, 20)), {width: 30000, height: 20}, 'png');
eq(pictureSize(gif(640, 480)), {width: 640, height: 480}, 'gif');
eq(pictureSize(jpeg(1920, 1080)), {width: 1920, height: 1080}, 'jpeg (after a DHT segment)');
eq(pictureSize(webpX(4000, 3000)), {width: 4000, height: 3000}, 'webp extended');
eq(pictureSize(webpL(800, 600)), {width: 800, height: 600}, 'webp lossless');
eq(pictureSize(svg('width="200" height="100"')), {width: 200, height: 100}, 'svg');
eq(pictureSize(svg('width="1in" height="1in"')), {width: 96, height: 96}, 'svg in inches');
eq(pictureSize(svg('viewBox="0 0 24 24"')), {width: 24, height: 24}, 'svg viewBox');
eq(pictureSize(bytes([0xef, 0xbb, 0xbf], text('\n<!-- a comment -->\n<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "x.dtd">\n<svg width="30" height="40">'))),
    {width: 30, height: 40}, 'svg after a BOM, a comment and a DOCTYPE');
eq(pictureSize(bytes(text('not a picture at all, just text that goes on'))), null, 'text');
eq(pictureSize(bytes([0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3, 1, 2, 3, 4, 5])), null, 'gzip');

// Fits: an ordinary picture, and each way to be too large
for (const [what, data] of [['png', png(1920, 1080)], ['gif', gif(300, 300)], ['jpeg', jpeg(4000, 3000)],
    ['webp', webpX(3840, 2160)], ['svg', svg('width="64" height="64"')]])
    if (!pictureFits(data)) fail(`${what} should fit`);
for (const [what, data] of [
    ['a PNG declaring 30000 x 30000', png(30000, 30000)],
    ['a PNG one side over 16384', png(16385, 10)],
    ['a PNG of zero width', png(0, 100)],
    ['a JPEG of 65535 x 65535', jpeg(65535, 65535)],
    ['a GIF of 65535 x 65535', gif(65535, 65535)],
    ['a WebP of 16384 x 16384', webpX(16384, 16384)],
    ['an SVG of 100000 px', svg('width="100000" height="100000"')],
    ['an SVG with no size', svg('')],
    ['a BMP holding a small <svg> early', bytes(text('BM'), new Array(60).fill(0), text('<svg width="1" height="1">'))],
    ['a TIFF holding a small <svg> early', bytes(text('II*'), [0, 8, 0, 0, 0], text('<svg width="1" height="1">'))],
    ['an unknown binary format with an <svg> in a text run', bytes([0, 1, 2, 3], text('<svg width="1" height="1"></svg>'))],
    ['an SVG-looking line after other text', bytes(text('hello\n<svg width="1" height="1">'))],
    ['a root that is not svg, with an <svg> inside', bytes(text('<html><svg width="1" height="1"></svg></html>'))],
    ['invalid UTF-8 before the <svg>', bytes([0xff, 0xfe, 0x80], text('<svg width="1" height="1">'))],
    ['a gzipped SVG', bytes([0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3, 1, 2, 3, 4, 5])],
    ['data that is no picture', bytes(text('hello, this is text, not a picture'))],
    ['nothing', new Uint8Array()],
])
    if (pictureFits(data)) fail(`${what} should not fit`);

// The byte ceilings, whatever the header says
const heavy = new Uint8Array(MAX_PICTURE_BYTES + 1);
heavy.set(png(10, 10));
if (pictureFits(heavy)) fail('a picture over the byte ceiling should not fit');
if (!copyFits(false, new Uint8Array(MAX_TEXT_BYTES))) fail('text at the ceiling should fit');
if (copyFits(false, new Uint8Array(MAX_TEXT_BYTES + 1))) fail('text over the ceiling should not fit');
if (!copyFits(true, png(100, 100))) fail('a small picture should fit');

(globalThis.print ?? console.log)('test-clipboard-pictures: ok');
