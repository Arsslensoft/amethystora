// Where the windows were at a set of screens (R7-46; modules/desks/layoutsdata.js of Amethystora UI), checked under node by
// 20-tests.sh: what desks.json's `layouts` may hold of a tampered or hand-edited file (never a title, only app IDs and
// numbers), and which open window each kept entry means.

import {strict as assert} from 'node:assert';

import {MOST_LAYOUTS, MOST_WINDOWS, cleanLayout, cleanLayouts, match, strips, visit} from
    '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/desks/layoutsdata.js';

const screen = '0123456789ab';
const tile = {app: 'org.gnome.Nautilus.desktop', rank: 0, workspace: 1, screen, column: 0, row: 0, width: 900};
const place = {app: 'kitty.desktop', rank: 0, workspace: 0, screen, x: 10, y: 20, w: 800, h: 600, max: false};
const layout = (mode, windows, more = {}) => ({seen: 3, mode, shown: {[screen]: 1}, windows, ...more});

// A good record is kept as it is
assert.deepEqual(cleanLayout(layout('tiling', [tile])), layout('tiling', [tile]));
assert.deepEqual(cleanLayout(layout('classic', [place])), layout('classic', [place]));

// Nothing but what the shape names survives: no title, no path, no field of anybody's
const dirty = cleanLayout(layout('tiling', [{...tile, title: 'CANARY secret.pdf', file: '/home/a/CANARY'}],
    {title: 'CANARY', document: 'CANARY'}));
assert.ok(!JSON.stringify(dirty).includes('CANARY'), 'a title is never kept');

// What does not fit is left out, entry by entry
assert.equal(cleanLayout(layout('tiling', [{...tile, app: 'not-a-launcher'}])), null, 'an app with no launcher');
assert.equal(cleanLayout(layout('tiling', [{...tile, app: '../../etc/passwd.desktop'}])), null, 'a path as an app');
assert.equal(cleanLayout(layout('tiling', [{...tile, screen: 'DP-1'}])), null, 'a connector as a screen');
assert.equal(cleanLayout(layout('tiling', [{...tile, width: 'wide'}])), null, 'a width that is no number');
assert.equal(cleanLayout(layout('tiling', [place])), null, 'a place under the tiling');
assert.equal(cleanLayout(layout('classic', [tile])), null, 'a strip in the classic layout');
assert.equal(cleanLayout(layout('other', [tile])), null, 'another mode');
assert.equal(cleanLayout([]), null);
assert.equal(cleanLayout(null), null);
assert.deepEqual(cleanLayout(layout('tiling', [tile], {shown: {DP1: 1, [screen]: 'x'}})).shown, {}, 'a shown that does not fit');
assert.equal(cleanLayout(layout('tiling', [tile, {...tile, column: 3}])).windows.length, 1, 'a window twice is one window');

// The caps: windows, and the desks seen last
const many = Array.from({length: MOST_WINDOWS + 20}, (unused, rank) => ({...tile, rank: Math.min(rank, MOST_WINDOWS),
    app: `app${rank}.desktop`}));
assert.equal(cleanLayout(layout('tiling', many)).windows.length, MOST_WINDOWS);
const desks = Object.fromEntries(Array.from({length: MOST_LAYOUTS + 3}, (unused, i) =>
    [`${i}`.padStart(12, '0'), layout('tiling', [tile], {seen: i + 1})]));
const kept = cleanLayouts(desks);
assert.equal(Object.keys(kept).length, MOST_LAYOUTS);
assert.ok(!('000000000000' in kept), 'the one seen longest ago goes first');
assert.deepEqual(cleanLayouts('nothing'), {});
assert.deepEqual(cleanLayouts({'not-a-hash': layout('tiling', [tile])}), {});
assert.ok(!Object.prototype.polluted);
assert.deepEqual(cleanLayouts(JSON.parse('{"__proto__": {"polluted": true}}')), {});
assert.ok(!Object.prototype.polluted);

// The visits are a counter, never a time: the desk visited last has the highest, an unchanged desk visited again still moves
// up (so a desk used all the time is never the first to go), the one already last needs no write, and when there are more
// than fit the least recently visited goes
const base = n => `${n}`.padStart(12, '0');
let stored = {};
for (const n of [1, 2, 3]) {
    stored = visit(stored, base(n), layout('tiling', [tile], {seen: 0}));
}
assert.deepEqual([1, 2, 3].map(n => stored[base(n)].seen), [1, 2, 3], 'each visit takes the next number');
assert.equal(visit(stored, base(3), layout('tiling', [tile])), null, 'the desk visited last, unchanged: nothing to write');
stored = visit(stored, base(1), layout('tiling', [tile]));
assert.equal(stored[base(1)].seen, 4, 'a desk visited again unchanged moves up');
assert.notEqual(visit(stored, base(1), layout('tiling', [{...tile, width: 901}])), null, 'a changed layout is written');
assert.ok(Object.values(stored).every(l => l.seen < 1e6), 'no time of day is on disk');
for (let n = 4; n < 4 + MOST_LAYOUTS; n++) {
    stored = visit(stored, base(n), layout('tiling', [tile]));
    stored = visit(stored, base(1), layout('tiling', [tile]));
}
assert.ok(base(1) in stored, 'the desk used between all the others stayed');
assert.equal(Object.keys(stored).length, MOST_LAYOUTS);
assert.ok(!(base(2) in stored), 'the one visited longest ago went');
assert.deepEqual(visit('nothing', base(1), layout('tiling', [tile])), {[base(1)]: {...cleanLayout(layout('tiling', [tile])), seen: 1}});

// Which window each entry means: by app and rank, oldest first; closed since, left out; opened since, never moved
const open = [
    {window: 'b', app: 'kitty.desktop', seq: 9},
    {window: 'a', app: 'kitty.desktop', seq: 3},
    {window: 'new', app: 'org.gnome.TextEditor.desktop', seq: 12},
];
const held = layout('classic', [{...place, rank: 1}, place, {...place, rank: 2}, {...place, app: 'gone.desktop'}]);
assert.deepEqual(match(held, open).map(m => [m.entry.rank, m.window]), [[1, 'b'], [0, 'a']]);

// A strip's columns close up over what was closed, and stacked windows keep their order
const entry = (column, row, rank) => ({entry: {...tile, column, row, rank, width: 500 + column}, window: `w${column}.${row}`});
const [strip] = strips([entry(4, 0, 2), entry(0, 1, 1), entry(0, 0, 0), entry(2, 3, 3)]);
assert.deepEqual(strip.windows.map(w => [w.window, w.column, w.row, w.width]), [
    ['w0.0', 0, 0, 500], ['w0.1', 0, 1, 500], ['w2.3', 1, 0, 502], ['w4.0', 2, 0, 504],
]);
assert.equal(strips([entry(0, 0, 0), {...entry(1, 0, 1), entry: {...tile, workspace: 2}}]).length, 2, 'a strip per workspace');

console.log('test-desk-layouts: ok');
