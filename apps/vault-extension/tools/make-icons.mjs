// node apps/vault-extension/tools/make-icons.mjs: draws the toolbar's PNG icons from the gem's polygons, as the Extension board draws
// it, since the Chromium family reads no SVG in a manifest. Not part of what is packaged. PNG written here with node's zlib, four by
// four samples a pixel, so that the same polygons always give the same bytes.
import { writeFileSync } from 'node:fs';
import { deflateSync, crc32 } from 'node:zlib';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const GEM = [
    ['34,88.3 128,43.3 128,107.6 92,124.8', [0xdc, 0xbc, 0xff]],
    ['128,43.3 164,60.5 164,124.8 128,107.6', [0x91, 0x48, 0xf0]],
    ['164,141.7 164,206 128,223.3 128,159', [0x5d, 0x1a, 0xc4]],
    ['128,223.3 34,178.3 92,141.7 128,159', [0x74, 0x29, 0xe0]],
    ['34,178.3 34,88.3 92,124.8 92,141.7', [0xb3, 0x7d, 0xf9]],
    ['164,60.5 222,32.7 222,88.3 164,124.8', [0xb3, 0x7d, 0xf9]],
    ['164,124.8 222,88.3 222,214.3 164,214.3', [0x91, 0x48, 0xf0]],
].map(([points, colour]) => [points.split(' ').map((point) => point.split(',').map(Number)), colour]);
const VIEW = { x: 34, y: 32.7, size: 190.5 };
const SAMPLES = 4;

function inside(polygon, x, y) {
    let within = false;
    for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
        const [xi, yi] = polygon[i];
        const [xj, yj] = polygon[j];
        if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) {
            within = !within;
        }
    }
    return within;
}

function chunk(type, data) {
    const body = Buffer.concat([Buffer.from(type), data]);
    const out = Buffer.alloc(body.length + 8);
    out.writeUInt32BE(data.length, 0);
    body.copy(out, 4);
    out.writeUInt32BE(crc32(body), body.length + 4);
    return out;
}

function draw(size) {
    const stride = size * 4 + 1;
    const rows = Buffer.alloc(stride * size);
    for (let py = 0; py < size; py++) {
        for (let px = 0; px < size; px++) {
            const sum = [0, 0, 0];
            let hits = 0;
            for (let sy = 0; sy < SAMPLES; sy++) {
                for (let sx = 0; sx < SAMPLES; sx++) {
                    const x = VIEW.x + ((px + (sx + 0.5) / SAMPLES) / size) * VIEW.size;
                    const y = VIEW.y + ((py + (sy + 0.5) / SAMPLES) / size) * VIEW.size;
                    const hit = GEM.find(([polygon]) => inside(polygon, x, y));
                    if (hit) {
                        hit[1].forEach((value, channel) => (sum[channel] += value));
                        hits++;
                    }
                }
            }
            const at = py * stride + 1 + px * 4;
            for (let channel = 0; channel < 3; channel++) {
                rows[at + channel] = hits ? Math.round(sum[channel] / hits) : 0;
            }
            rows[at + 3] = Math.round((255 * hits) / (SAMPLES * SAMPLES));
        }
    }
    const header = Buffer.alloc(13);
    header.writeUInt32BE(size, 0);
    header.writeUInt32BE(size, 4);
    header[8] = 8;
    header[9] = 6;
    const signature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
    return Buffer.concat([signature, chunk('IHDR', header), chunk('IDAT', deflateSync(rows, { level: 9 })), chunk('IEND', Buffer.alloc(0))]);
}

const folder = join(dirname(fileURLToPath(import.meta.url)), '..', 'icons');
for (const size of [16, 32, 48, 128]) {
    writeFileSync(join(folder, `vault-${size}.png`), draw(size));
}
