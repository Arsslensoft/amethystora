// The top bar's peek leaves the way up to a screen above open (Amethystora UI's modules/bars/edge.js): its barriers
// cover only the parts of a screen's top edge that no other screen's bottom edge touches, as GNOME's hot corners leave
// an edge with a neighbour alone. Run by 20-tests.sh:
//   node test-bars-edge.mjs [EXTENSION_DIR]
import assert from 'node:assert/strict';
import path from 'node:path';
import {pathToFileURL} from 'node:url';

const dir = process.argv[2] ?? '/usr/share/gnome-shell/extensions/amethystora-ui@iarsslen.github.io';
const {freeTopEdge, freeBottomEdge} = await import(pathToFileURL(path.join(dir, 'modules/bars/edge.js')).href);
const m = (x, y, width, height) => ({x, y, width, height});

// Side by side: nothing above, the whole edge
const left = m(0, 0, 1920, 1080), right = m(1920, 0, 1280, 800);
assert.deepEqual(freeTopEdge(right, [left, right]), [[1920, 3200]]);
// A laptop under an external screen as wide: no barrier at all, and the screen above keeps its own
const above = m(0, 0, 1920, 1080), laptop = m(0, 1080, 1920, 1200);
assert.deepEqual(freeTopEdge(laptop, [above, laptop]), []);
assert.deepEqual(freeTopEdge(above, [above, laptop]), [[0, 1920]]);
// A narrower screen over the middle of a wider one: the two ends
const wide = m(0, 1080, 2560, 1440);
assert.deepEqual(freeTopEdge(wide, [m(320, 0, 1920, 1080), wide]), [[0, 320], [2240, 2560]]);
// Partly over it, from the left: the rest
assert.deepEqual(freeTopEdge(wide, [m(-1000, 0, 1500, 1080), wide]), [[500, 2560]]);
// Two screens over it side by side: nothing
assert.deepEqual(freeTopEdge(wide, [m(0, 0, 1280, 1080), m(1280, 0, 1280, 1080), wide]), []);
// A pixel apart is not touching
assert.deepEqual(freeTopEdge(wide, [m(0, 0, 2560, 1079), wide]), [[0, 2560]]);
// Mixed scales: GNOME gives logical rectangles, a 4K screen at 2x above a 1080p one at 1x is 1920x1080 over 1920x1080
const hidpi = m(0, 0, 1920, 1080), plain = m(0, 1080, 1920, 1080);
assert.deepEqual(freeTopEdge(plain, [hidpi, plain]), []);
// ...and a 2560 wide screen at 1.25x (2048 logical) centred above a 2560 wide one: the two uncovered ends
assert.deepEqual(freeTopEdge(m(0, 1152, 2560, 1440), [m(256, 0, 2048, 1152), m(0, 1152, 2560, 1440)]),
    [[0, 256], [2304, 2560]]);
// Mirrored screens are one logical monitor, so one rectangle: the whole edge
const mirrored = m(0, 0, 1920, 1080);
assert.deepEqual(freeTopEdge(mirrored, [mirrored]), [[0, 1920]]);
// A screen beside, its bottom level with this one's top, touches no part of it
assert.deepEqual(freeTopEdge(plain, [m(1920, 0, 1920, 1080), plain]), [[0, 1920]]);
// The bar at the bottom (R5-06): the same, mirrored. A screen below this one leaves the way down open
assert.deepEqual(freeBottomEdge(above, [above, laptop]), []);
assert.deepEqual(freeBottomEdge(laptop, [above, laptop]), [[0, 1920]]);
assert.deepEqual(freeBottomEdge(m(320, 0, 1920, 1080), [m(320, 0, 1920, 1080), wide]), []);
assert.deepEqual(freeBottomEdge(m(0, 0, 2560, 1080), [m(0, 0, 2560, 1080), m(320, 1080, 1920, 1080)]), [[0, 320], [2240, 2560]]);
assert.deepEqual(freeBottomEdge(left, [left, right]), [[0, 1920]]);
console.log('bars edge: ok');
