// Charged by the time you leave (R7-70): the pattern and the plan (modules/power/chargedata.js of Amethystora UI), checked under node and gjs:
//   node build_files/shared/test-charge-by.mjs
// What is kept of an unplug, what counts as a night, the usual time and the next one, what the calendar moves, and when the hold comes off.

import {CHARGE_MS, KEEP, MIN_HOLD_MS, MIN_ON_MAINS_MS, clean, decode, encode, learn, merge, nextLeave, plan, usual, withCalendar} from
    '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/power/chargedata.js';

const say = globalThis.print ?? console.log;
const failures = [];
let count = 0;
function expect(what, got, wanted) {
    count++;
    const a = JSON.stringify(got), b = JSON.stringify(wanted);
    if (a !== b)
        failures.push(`${what}: expected ${b}, got ${a}`);
}

const H = 3600 * 1000;
// Wednesday 2026-11-04 08:30 local time, and the night before
const at = (day, hour, minute = 0) => new Date(2026, 10, day, hour, minute).getTime();

// Only an unplug after a night counts, and only its minute and kind are kept
expect('a night counts', learn([], at(4, 8, 30), at(3, 22)), [{m: 510, we: false}]);
expect('a short stay does not', learn([], at(4, 8, 30), at(4, 7)), []);
expect('exactly six hours counts', learn([], at(4, 8, 30), at(4, 8, 30) - MIN_ON_MAINS_MS), [{m: 510, we: false}]);
expect('a weekend', learn([], at(7, 10), at(6, 20)), [{m: 600, we: true}]);
expect('not a number', learn([], NaN, 0), []);
expect('nothing but the minute and the kind is kept', Object.keys(learn([], at(4, 8, 30), at(3, 22))[0]).sort(), ['m', 'we']);

// The last thirty
let list = [];
for (let n = 0; n < 40; n++)
    list = learn(list, at(4, 8, n), at(3, 20));
expect('the last thirty', [list.length, KEEP, list[0].m, list[29].m], [30, 30, 490, 519]);
expect('clean drops what does not fit', clean([{m: 5, we: false}, {m: 1440, we: false}, {m: -1, we: true}, {m: 1.5, we: true}, {m: 5}, null, 3, {m: 7, we: true, extra: 'x'}]),
    [{m: 5, we: false}, {m: 7, we: true}]);
expect('clean of nothing', clean('x'), []);
expect('round trip', decode(encode([{m: 510, we: false}])), [{m: 510, we: false}]);
for (const bad of ['', 'x', '[]', '{"version":2,"unplugs":[]}', '{"version":1}'])
    expect(`not a list: ${bad}`, decode(new TextEncoder().encode(bad)), null);
expect('merge', merge([{m: 1, we: false}], [{m: 2, we: false}]), [{m: 1, we: false}, {m: 2, we: false}]);

// The usual time is the median, and needs three of a kind
const week = [500, 510, 520, 530, 900].map(m => ({m, we: false}));
expect('median of five', usual(week, false), 520);
expect('median of four', usual(week.slice(0, 4), false), 515);
expect('too few', usual(week.slice(0, 2), false), null);
expect('a kind with none', usual(week, true), null);

// The next leaving: tonight's or tomorrow's, and over a weekend the weekend's
expect('this morning, before it', new Date(nextLeave(week, at(4, 7))).getHours(), 8);
expect('tomorrow, after today\'s time', new Date(nextLeave(week, at(4, 9))).getDate(), 5);
expect('nothing known', nextLeave([], at(4, 9)), null);
expect('Friday night: no weekend data, so nothing is known and it charges fully', nextLeave(week, at(6, 22)), null);
expect('Sunday night: Monday is known', new Date(nextLeave(week, at(8, 22))).getDate(), 9);

// The calendar moves it earlier, never later, never into the past
const leave = at(5, 8, 30);
expect('an earlier event', withCalendar(leave, [at(5, 8, 0)], at(4, 22)), at(5, 7, 30));
expect('a later event', withCalendar(leave, [at(5, 10)], at(4, 22)), leave);
expect('an event whose lead has passed', withCalendar(leave, [at(5, 8, 0)], at(5, 7, 45)), leave);
expect('no events', withCalendar(leave, [], at(4, 22)), leave);
expect('a bad start', withCalendar(leave, [NaN, 'x'], at(4, 22)), leave);

// The hold: with time to spare, off at the leaving time less the charge; otherwise charge fully
expect('overnight', plan(at(4, 22), leave), {hold: true, releaseAt: leave - CHARGE_MS});
expect('exactly four hours', plan(leave - MIN_HOLD_MS, leave), {hold: true, releaseAt: leave - CHARGE_MS});
expect('too soon', plan(leave - MIN_HOLD_MS + 1, leave), {hold: false, releaseAt: null});
expect('nothing known charges fully', plan(at(4, 22), null), {hold: false, releaseAt: null});
expect('not a time', plan(at(4, 22), NaN), {hold: false, releaseAt: null});
expect('the release is before the leaving', leave - CHARGE_MS < leave && CHARGE_MS === 75 * 60 * 1000 && H === 3600000, true);

if (failures.length) {
    say(`test-charge-by: ${failures.length} of ${count} failed`);
    for (const f of failures)
        say(`  ${f}`);
    throw new Error('test-charge-by failed');
}
say(`test-charge-by: ${count} checks passed`);
