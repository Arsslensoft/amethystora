// The saved sets' data (modules/workspaces/setsdata.js of Amethystora UI), checked under node and gjs alike:
//   node build_files/shared/test-sets.mjs
//   gjs -m build_files/shared/test-sets.mjs
// What a tampered sets.json may hold and what is kept of it, and where an opening's windows go in a strip. A failure
// throws, which makes either exit with an error.

import {MAX_SETS, addSet, clean, cleanName, cleanResume, isAppId, isName, slot, tileOrder} from
    '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@iarsslen.github.io/modules/workspaces/setsdata.js';

const say = globalThis.print ?? console.log;
const failures = [];
let count = 0;

function expect(what, got, wanted) {
    count++;
    const a = JSON.stringify(got), b = JSON.stringify(wanted);
    if (a !== b)
        failures.push(`${what}: expected ${b}, got ${a}`);
}

const place = {m: 'Virtual-1', x: 16, y: 54, w: 900, h: 700, max: false};
const good = {name: 'Writing', saved: 1761645900, layout: 'classic', windows: [
    {app: 'org.gnome.TextEditor.desktop', rank: 0, place},
    {app: 'org.gnome.Nautilus.desktop', rank: 0, place: {...place, x: 930, w: 660, h: 420}},
    {app: 'kitty.desktop', rank: 0, tile: {column: 2, row: 0, width: 660}},
]};

// A good file is kept as it is
expect('good file', clean({version: 1, sets: [good]}), {version: 1, sets: [good]});

// Anything else is nothing
for (const bad of [null, 3, 'x', [], {}, {version: 2, sets: [good]}, {version: 1}, {version: 1, sets: {}}])
    expect(`not a file: ${JSON.stringify(bad)}`, clean(bad), {version: 1, sets: []});

// Tampered sets are dropped piece by piece
const tampered = {version: 1, sets: [
    good,
    {...good, name: 'Writing', saved: 1}, // the same name, older: dropped
    {...good, name: ''}, // no name
    {...good, name: 'x'.repeat(25)}, // too long
    {...good, name: 'a\nb'}, // a control character
    {...good, name: '__proto__'},
    {...good, name: ' padded'}, // not as it would be kept
    {...good, name: 'Layout', layout: 'grid'},
    {...good, name: 'Saved', saved: 'yesterday'},
    {...good, name: 'Apps', windows: [
        {app: '../x.desktop', rank: 0}, // a slash: not a desktop file ID
        {app: '/usr/bin/x.desktop', rank: 0},
        {app: '.hidden.desktop', rank: 0},
        {app: 'a\nb.desktop', rank: 0},
        {app: '.desktop', rank: 0},
        {app: 'x'.repeat(250) + '.desktop', rank: 0},
        {app: 'kitty', rank: 0},
        {app: 'wine-Programs-Some App+Tools@2.desktop', rank: 0}, // as GLib names Wine's launchers: kept
        {app: 'kitty.desktop', rank: 16},
        {app: 'kitty.desktop', rank: 1, place: {...place, w: 10}}, // a place that does not fit is left out
        {app: 'kitty.desktop', rank: 1}, // the same app and rank again
        {app: 'kitty.desktop', rank: 2, tile: {column: 16, row: 0, width: 600}},
        {app: 'kitty.desktop', rank: 3, place, extra: 'title', tile: {column: 1, row: 0, width: 600, x: 1}},
    ]},
    {...good, name: 'Empty', windows: [{app: 'bad', rank: 0}]}, // nothing left: dropped
]};
const kept = clean(tampered);
expect('tampered: names kept', kept.sets.map(s => s.name), ['Writing', 'Apps']);
expect('tampered: windows kept', kept.sets[1].windows, [
    {app: 'wine-Programs-Some App+Tools@2.desktop', rank: 0},
    {app: 'kitty.desktop', rank: 1},
    {app: 'kitty.desktop', rank: 2},
    {app: 'kitty.desktop', rank: 3, place, tile: {column: 1, row: 0, width: 600}},
]);
// The prototype cannot be reached through a name
expect('no prototype', Object.prototype.polluted, undefined);

// At most 16 windows, at most 12 sets, newest first
const many = {...good, windows: Array.from({length: 20}, (_, rank) => ({app: 'kitty.desktop', rank: rank % 16, place}))};
expect('16 windows', clean({version: 1, sets: [many]}).sets[0].windows.length, 16);
let data = {version: 1, sets: []};
for (let i = 0; i < 13; i++)
    data = addSet(data, {...good, name: `Set ${i}`, saved: 1000 + i});
expect('12 sets', data.sets.length, MAX_SETS);
expect('oldest gone', data.sets.map(s => s.name).includes('Set 0'), false);
expect('newest first', data.sets[0].name, 'Set 12');
// Saved in the same second: the newest still comes first and stays
data = addSet(data, {...good, name: 'Same second', saved: 2000});
expect('same second', [data.sets[0].name, data.sets.length], ['Same second', 12]);
data = addSet(data, {...good, name: 'Set 5', saved: 2000});
expect('same name replaces', [data.sets.length, data.sets[0].name, data.sets.filter(s => s.name === 'Set 5').length],
    [12, 'Set 5', 1]);

// Names as they are kept
expect('name cut', cleanName('Abcdefghijklmnopqrstuvwxyz'), 'Abcdefghijklmnopqrstuvwx');
expect('name cleaned', cleanName(' a\tb\u2028c '), 'abc');
expect('name of emoji', cleanName('\u{1F4DD}'.repeat(30)), '\u{1F4DD}'.repeat(24));

// Format characters that turn text round are refused in names and IDs; the joiners of words and emoji stay in names
expect('name without format characters', cleanName('a\u202eb\u2066c'), 'abc');
expect('name keeps the joiners', cleanName('\u{1F469}\u200d\u{1F4BB} x\u200cy'), '\u{1F469}\u200d\u{1F4BB} x\u200cy');
expect('a name with a right-to-left override is no name', isName('a\u202eb'), false);
expect('an ID with a format character is no ID', [isAppId('x\u202e.desktop'), isAppId('x\u200d.desktop'), isAppId('Some App+1@x.desktop')],
    [false, false, true]);

// The order in a strip: saved tiles, or the classic places' centres left to right
expect('tiles', tileOrder(good), [{column: 3, row: 0, width: 900}, {column: 4, row: 0, width: 660}, {column: 2, row: 0, width: 660}]);
const classic = {...good, windows: [
    {app: 'a.desktop', rank: 0, place: {...place, x: 900, w: 400}},
    {app: 'b.desktop', rank: 0, place: {...place, x: 0, w: 400}},
    {app: 'c.desktop', rank: 0, place: {...place, x: 400, w: 600}},
]};
expect('centres', tileOrder(classic), [{column: 2, row: 0, width: 400}, {column: 0, row: 0, width: 400}, {column: 1, row: 0, width: 600}]);

// Where a window goes: after every other window while none of the set is there; before the first set column to its
// right; under the windows saved above it in its own column
const t = (column, row = 0) => ({column, row, width: 600});
expect('empty strip', slot([], t(1)), {column: 0});
expect('after the others', slot([[], []], t(1)), {column: 2});
expect('after the set', slot([[], [t(0)]], t(1)), {column: 2});
expect('before a later one', slot([[], [t(0)], [t(2)]], t(1)), {column: 2});
expect('first of the set', slot([[], [t(2)]], t(0)), {column: 1});
expect('into its column', slot([[t(0)], [t(1, 0), t(1, 2)]], t(1, 1)), {column: 1, row: 1});
expect('top of its column', slot([[t(1, 1)]], t(1, 0)), {column: 0, row: 0});
// Two windows of one app around another: kitty, Files, kitty
expect('kitty files kitty', slot([[t(0)], [t(2)]], t(1)), {column: 1});

// What an update's restart keeps (resume.json, workspaces/resume.js): each workspace by its place, with a set's windows
const resume = {version: 1, saved: 1761900000, from: 'latest-45.20261031:abc1234', kind: 'restart', active: 2,
    workspaces: [{index: 2, layout: 'tiling', windows: good.windows}, {index: 0, layout: 'classic', windows: [good.windows[0]]}]};
expect('resume kept, by place', cleanResume(resume)?.workspaces.map(w => [w.index, w.windows.length]), [[0, 1], [2, 3]]);
expect('resume: the workspace in front', cleanResume(resume)?.active, 2);
for (const [what, bad] of [['version', {...resume, version: 2}], ['kind', {...resume, kind: 'x'}],
    ['from', {...resume, from: 'a b'}], ['saved', {...resume, saved: 'now'}], ['not an object', []],
    ['nothing left', {...resume, workspaces: [{index: 1, layout: 'classic', windows: [{app: 'bad', rank: 0}]}]}]])
    expect(`resume refused: ${what}`, cleanResume(bad), null);
expect('resume: a place twice, a place out of range', cleanResume({...resume, workspaces: [
    {index: 1, layout: 'classic', windows: good.windows}, {index: 1, layout: 'classic', windows: good.windows},
    {index: 64, layout: 'classic', windows: good.windows}]})?.workspaces.map(w => w.index), [1]);
expect('resume: no title survives', JSON.stringify(cleanResume({...resume, workspaces: [{index: 0, layout: 'classic',
    windows: [{app: 'kitty.desktop', rank: 0, title: 'secret'}]}]})).includes('secret'), false);

if (failures.length) {
    for (const f of failures)
        say(`FAIL ${f}`);
    throw new Error(`${failures.length} of ${count} checks of the saved sets failed`);
}
say(`saved sets: ${count} checks passed`);
