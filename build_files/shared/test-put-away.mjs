// Put this away for tomorrow (R7-34): the list that is sealed (modules/workspaces/putawaydata.js of Amethystora UI), checked under node and gjs:
//   node build_files/shared/test-put-away.mjs
// What a tampered or swapped list may hold, the limit of eight, which of two lists wins, and what each answer of the Vault means to the feature.
// A failure throws.

import {FILE_NAME, MAX_ITEMS, PURPOSE, RECORD, add, decode, encode, merge, problemOf, without} from
    '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/workspaces/putawaydata.js';

const say = globalThis.print ?? console.log;
const failures = [];
let count = 0;

function expect(what, got, wanted) {
    count++;
    const a = JSON.stringify(got), b = JSON.stringify(wanted);
    if (a !== b)
        failures.push(`${what}: expected ${b}, got ${a}`);
}

const win = {app: 'org.gnome.TextEditor.desktop', rank: 0, place: {m: 'Virtual-1', x: 16, y: 54, w: 900, h: 700, max: false}};
const item = (name, saved) => ({name, saved, layout: 'classic', windows: [win]});

expect('names', [PURPOSE, RECORD, FILE_NAME], ['put-away', 'list', 'put-away.avs']);

// What is sealed comes back the same
const two = [item('Tax return', 200), item('Slides', 100)];
expect('round trip', decode(encode(two)), two);

// What is not a list is nothing, and a tampered one is kept piece by piece
for (const bad of ['', 'x', '[]', '{"version":2,"sets":[]}', '{"version":1}', '{"version":1,"sets":3}'])
    expect(`not a list: ${bad}`, decode(new TextEncoder().encode(bad)), null);
expect('a bad piece', decode(new TextEncoder().encode(JSON.stringify({version: 1, sets: [
    item('Good', 5), {...item('x\nb', 4)}, {...item('Empty', 3), windows: []}, {...item('Path', 2), windows: [{app: '../x.desktop', rank: 0}]},
]}))).map(i => i.name), ['Good']);

// At most eight, the newest kept, and one name once
let list = [];
for (let n = 1; n <= 10; n++)
    list = add(list, item(`Set ${n}`, n));
expect('limit', [list.length, MAX_ITEMS, list[0].name, list[7].name], [8, 8, 'Set 10', 'Set 3']);
list = add(list, item('Set 5', 99));
expect('same name gives way', [list.length, list[0].name, list[0].saved], [8, 'Set 5', 99]);
expect('without', without(list, 'Set 5').length, 7);
expect('encoded list is cut at eight', decode(encode([...list, item('A', 1), item('B', 2)])).length, 8);

// The file and this session: the session's wins
expect('merge', merge([item('Tax return', 100), item('Old', 50)], [item('Tax return', 300)]).map(i => [i.name, i.saved]),
    [['Tax return', 300], ['Old', 50]]);

// What the Vault's answers mean
expect('problems', ['closed', 'not-set-up', 'refused', 'failed', 'too-large'].map(problemOf), ['closed', 'never', 'refused', 'closed', 'closed']);

// The sealed bytes never carry a name: a stand-in for sealing is checked by the helper's test; here, that encode() is JSON of the list only
expect('encode is the list', JSON.parse(new TextDecoder().decode(encode([item('Canary-4821', 1)]))).sets[0].name, 'Canary-4821');

if (failures.length) {
    say(`test-put-away: ${failures.length} of ${count} failed`);
    for (const f of failures)
        say(`  ${f}`);
    throw new Error('test-put-away failed');
}
say(`test-put-away: ${count} checks passed`);
