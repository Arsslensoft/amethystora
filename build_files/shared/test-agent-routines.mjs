// Little routines in the shell (R7-15), the parts that need no screen: what the helper says, which routines an event wakes, when a time
// trigger is next due, and what a run reads as. The routines' own rules (never root, never a shell line, no secret) are the helper's, held
// by `cargo test -p amethystora-agent-tools routines`. No screen:
//   node build_files/shared/test-agent-routines.mjs
// A failure throws, which makes node exit with an error.

const UI = '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules';
const {COOL_OFF, actionsRan, nextDue, onDay, parseRan, parseTriggers, routinesFor, sourcesNeeded, stateOf, wakes} = await import(`${UI}/agent/routinesmodel.js`);

const say = globalThis.print ?? console.log;
const failures = [];
let count = 0;

function same(what, got, want) {
    count++;
    if (JSON.stringify(got) !== JSON.stringify(want))
        failures.push(`${what}: expected ${JSON.stringify(want)}, got ${JSON.stringify(got)}`);
}

const routine = (id, when, more = {}) => ({id, name: id, when, paused: false, whileLocked: false, ...more});

// --- What the helper says ------------------------------------------------------------------------------------------------------
same('broken output is an error', parseTriggers('{nope').state, 'error');
same('a closed vault waits', parseTriggers('{"state":"closed","said":"The Vault is closed"}'), {state: 'closed', routines: [], said: 'The Vault is closed'});
same('no store', parseTriggers('{"state":"none","routines":[]}'), {state: 'none', routines: [], said: ''});
const listed = parseTriggers(JSON.stringify({state: 'ok', routines: [
    {id: 'office-morning', name: 'Office morning', when: {network: 'Office Wi-Fi'}, paused: false, when_words: 'When I join Office Wi-Fi', undoable: true},
    {id: '../x', name: 'bad id', when: {network: 'a'}}, {id: 'no-when', name: 'x'}, 'junk']}));
same('only well-formed routines are kept', listed.routines.map(r => r.id), ['office-morning']);
same('and their words', listed.routines[0].whenWords, 'When I join Office Wi-Fi');

// --- Only what a routine names is listened for ----------------------------------------------------------------------------------
same('nothing, nothing', [...sourcesNeeded([])], []);
same('a network and a time', [...sourcesNeeded([routine('a', {network: 'x'}), routine('b', {time: '18:00'})])].sort(), ['network', 'time']);
same('a paused routine needs nothing', [...sourcesNeeded([routine('a', {network: 'x'}, {paused: true})])], []);

// --- Which routines an event wakes ------------------------------------------------------------------------------------------------
same('the network by its exact name', wakes({network: 'Office Wi-Fi'}, {kind: 'network', name: 'Office Wi-Fi'}), true);
same('not another network', wakes({network: 'Office Wi-Fi'}, {kind: 'network', name: 'Office Wi-Fi 5G'}), false);
same('an app opening is the default', wakes({app: 'a.desktop'}, {kind: 'app', id: 'a.desktop', state: 'opens'}), true);
same('not closing', wakes({app: 'a.desktop'}, {kind: 'app', id: 'a.desktop', state: 'closes'}), false);
same('fullscreen', wakes({app: 'a.desktop', state: 'fullscreen'}, {kind: 'app', id: 'a.desktop', state: 'fullscreen'}), true);
same('the screen', [wakes({screen: 'unlocks'}, {kind: 'screen', state: 'unlocks'}), wakes({screen: 'unlocks'}, {kind: 'screen', state: 'locks'})], [true, false]);
same('unplugging', wakes({battery: 'on-battery'}, {kind: 'battery', onBattery: true, percent: 80, was: {onBattery: false, percent: 80}}), true);
same('not while it was on the battery already', wakes({battery: 'on-battery'}, {kind: 'battery', onBattery: true, percent: 79, was: {onBattery: true, percent: 80}}), false);
same('plugging in', wakes({battery: 'plugged-in'}, {kind: 'battery', onBattery: false, percent: 20, was: {onBattery: true, percent: 20}}), true);
same('crossing down through 20', wakes({battery: 'below 20'}, {kind: 'battery', onBattery: true, percent: 19, was: {onBattery: true, percent: 20}}), true);
same('not again below it', wakes({battery: 'below 20'}, {kind: 'battery', onBattery: true, percent: 18, was: {onBattery: true, percent: 19}}), false);
same('not while charging', wakes({battery: 'below 20'}, {kind: 'battery', onBattery: false, percent: 15, was: {onBattery: false, percent: 21}}), false);
same('a phone near', wakes({phone: 'near'}, {kind: 'phone', state: 'near', name: 'Pixel'}), true);
same('a named phone only', [wakes({phone: 'near', name: 'Pixel'}, {kind: 'phone', state: 'near', name: 'Other'}), wakes({phone: 'away', name: 'Pixel'}, {kind: 'phone', state: 'away', name: 'Pixel'})], [false, true]);
same('a time is no event', wakes({time: '18:00'}, {kind: 'network', name: '18:00'}), false);

const list = [routine('a', {network: 'x'}), routine('b', {network: 'x'}, {paused: true}), routine('c', {network: 'y'})];
same('paused and other routines stay asleep', routinesFor(list, {kind: 'network', name: 'x'}, new Map(), 100).map(r => r.id), ['a']);
same('once for each time the trigger fires', routinesFor(list, {kind: 'network', name: 'x'}, new Map([['a', 100 - COOL_OFF + 1]]), 100).map(r => r.id), []);
same('and again after the cool-off', routinesFor(list, {kind: 'network', name: 'x'}, new Map([['a', 100 - COOL_OFF]]), 100).map(r => r.id), ['a']);

// --- Time: one timer, to the next due moment --------------------------------------------------------------------------------------
const friday = new Date(2026, 10, 6, 10, 0, 0);
same('weekdays include a Friday', onDay({time: '18:00', days: 'weekdays'}, friday), true);
same('weekends do not', onDay({time: '18:00', days: 'weekends'}, friday), false);
same('named days', [onDay({days: ['fri', 'sat']}, friday), onDay({days: ['mon']}, friday)], [true, false]);
const today = nextDue([routine('a', {time: '18:00', days: 'weekdays'})], friday);
same('this evening', [today.at.getDate(), today.at.getHours(), today.ids], [6, 18, ['a']]);
const monday = nextDue([routine('a', {time: '07:30', days: 'weekdays'})], new Date(2026, 10, 6, 8, 0, 0));
same('after the weekend, on the Monday', [monday.at.getDay(), monday.at.getDate()], [1, 9]);
const two = nextDue([routine('a', {time: '18:00'}), routine('b', {time: '18:00'}), routine('c', {time: '19:00'})], friday);
same('two at the same moment come together', two.ids, ['a', 'b']);
same('none, none', nextDue([routine('a', {network: 'x'}), routine('b', {time: '18:00'}, {paused: true})], friday), null);
same('a moment that has passed is not due', nextDue([routine('a', {time: '10:00', days: ['fri']})], friday).at.getDate(), 13);

// --- A run -------------------------------------------------------------------------------------------------------------------------
const ran = parseRan(JSON.stringify({state: 'ran', id: 'office-morning', name: 'Office morning', failed: null, undoable: true,
    results: [{kind: 'open', what: 'com.slack.Slack.desktop', ok: true, undo: false}, {kind: 'ame', what: 'ame desktop volume mute', ok: true, undo: true}, {kind: 'sh', what: 'x'}]}));
same('what ran, and nothing of another kind', ran.results.map(r => r.kind), ['open', 'ame']);
same('and that it can be undone', ran.undoable, true);
same('a paused routine did not run', parseRan('{"state":"paused","id":"a"}'), null);
same('a failure says why', parseRan('{"state":"ran","id":"a","name":"A","failed":"ame x did not work","results":[]}').failed, 'ame x did not work');

same('a closed Vault is its own answer, not a run', [stateOf('{"state":"closed"}'), stateOf('{"state":"ran"}'), stateOf('{"state":"paused"}'), stateOf('{nope'), stateOf('{"state":"x"}')], ['closed', 'ran', 'paused', '', '']);
const partly = parseRan(JSON.stringify({state: 'ran', id: 'a', name: 'A', failed: 'x', results: [{kind: 'ame', what: 'a', ok: true}, {kind: 'ame', what: 'b', ok: false}]}));
same('a way back is asked only for what ran', actionsRan(partly), 1);

if (failures.length) {
    for (const failure of failures)
        say(`not ok: ${failure}`);
    throw new Error(`${failures.length} of ${count} checks of the routines failed`);
}
say(`ok: ${count} checks of the routines`);
