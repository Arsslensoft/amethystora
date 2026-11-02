// When a flat battery closes the computer (R7-54; modules/power/flatdata.js of Amethystora UI), checked under node by
// 20-tests.sh: never for a battery not seen with charge in hand, never while charging, not for a UPS, and held by Not yet
// until the charge falls further.

import {strict as assert} from 'node:assert';

import {FLAT, flatState, isAbove} from
    '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/power/flatdata.js';

const base = {present: true, type: 2, state: 2, percentage: 3, warning: 4, seenAbove: true, heldAt: null};
const flat = over => flatState({...base, ...over});

assert.equal(FLAT, 3);
assert.equal(flat({}), 'flat', 'at 3%, running down');
assert.equal(flat({percentage: 3.4}), 'none', 'above 3%');
assert.equal(flat({percentage: 1}), 'flat', 'below 3%');
assert.equal(flat({percentage: 12, warning: 5}), 'flat', 'UPower about to act, whatever the percent');
assert.equal(flat({seenAbove: false}), 'none', 'a gauge that was never above 3% closes nothing');
assert.equal(flat({percentage: 0, seenAbove: false}), 'none', 'a gauge that always reads 0% closes nothing');
for (const state of [0, 1, 3, 4, 5, 6])
    assert.equal(flat({state}), 'none', `state ${state} is not running down`);
assert.equal(flat({type: 3}), 'none', 'a UPS is not this');
assert.equal(flat({present: false}), 'none', 'no battery');
assert.equal(flat({present: undefined, type: undefined, state: undefined, percentage: undefined}), 'none', 'no reading yet');
assert.equal(flat({percentage: NaN}), 'none', 'no number');
assert.equal(flat({heldAt: 3}), 'none', 'Not yet at 3%');
assert.equal(flat({heldAt: 3, percentage: 3.9}), 'none', 'still the same percent');
assert.equal(flat({heldAt: 3, percentage: 2.9}), 'flat', 'Not yet lasts until it falls further');

assert.equal(isAbove({present: true, type: 2, percentage: 3.1}), true);
assert.equal(isAbove({present: true, type: 2, percentage: 3}), false);
assert.equal(isAbove({present: true, type: 3, percentage: 90}), false);
assert.equal(isAbove({present: false, type: 2, percentage: 90}), false);

console.log('test-flat-battery: ok');
