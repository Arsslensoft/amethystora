// The live pill's sums (round 7), which need no screen: the spring, the ring's path, who sits beside whom, the words of
// a timer, what is moving and for how long, the album's colour and the sound meter's slice. Checked under node and gjs alike:
//   node build_files/shared/test-pill-logic.mjs
// A failure throws, which makes either exit with an error.

const UI = '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules';
const {ORDER, Activities} = await import(`${UI}/pill/activities.js`);
const {SETTLE_MS, ringPieces, springAt} = await import(`${UI}/pill/spring.js`);
const {readTimer} = await import(`${UI}/search/timer.js`);
const {HOLD_LONGEST, LONGEST, QUIET, agedOut, fractionOf, givesWay, listenToMoving, partialName, reportMoving} = await import(`${UI}/pill/movinglist.js`);
const {albumColour, barsOf, contrast, plateTint} = await import(`${UI}/pill/tint.js`);

const say = globalThis.print ?? console.log;
const failures = [];
let count = 0;

function same(what, got, want) {
    count++;
    if (JSON.stringify(got) !== JSON.stringify(want))
        failures.push(`${what}: expected ${JSON.stringify(want)}, got ${JSON.stringify(got)}`);
}

function near(what, got, want, within = 0.01) {
    count++;
    if (Math.abs(got - want) > within)
        failures.push(`${what}: expected about ${want}, got ${got}`);
}

// --- R7-16: who sits beside whom ------------------------------------------------------------------------------------
const act = (id, more = {}) => ({id, ...more});
const list = (...ids) => {
    const a = new Activities(null);
    for (const id of ids)
        a.set(id, act(id));
    return a;
};
same('one activity has nothing beside it', list('media').second, null);
same('nothing has nothing', list().second, null);
same('the second in ORDER sits beside the first', list('media', 'timer').second.id, 'media');
same('...and the first is the one first in ORDER', list('media', 'timer').top.id, 'timer');
for (const alone of ['call', 'recording', 'present'])
    same(`${alone} keeps the place alone`, list(alone, 'timer', 'media').second, null);
for (const mark of ['saving', 'quiet', 'focus'])
    same(`${mark} is never the bubble`, list('media', mark).second, null);
same('a mark is passed over for what happens', list('timer', 'focus', 'media').second.id, 'media');
{
    const a = new Activities(null);
    a.set('timer', act('timer'));
    a.set('hello', act('hello', {beside: false}));
    a.set('media', act('media'));
    same('beside: false is passed over', a.top.id, 'hello');
    same('...for the next', a.second.id, 'timer');
}
same('the others stay what they were', list('timer', 'media', 'update', 'focus', 'quiet').others.map(a => a.id), ['media', 'update', 'quiet']);
for (const [first, then] of [['call', 'recording'], ['recording', 'present'], ['present', 'meeting'], ['meeting', 'link'],
    ['link', 'code'], ['code', 'battery'], ['waiting', 'drop'], ['drop', 'hello'], ['hello', 'reading'], ['timer', 'timer2'],
    ['media', 'moving'], ['moving', 'update'], ['focus', null]]) {
    // What the sturdy group adds (a flat battery's 'flat', right after a call and a recording) is not in these pairs
    const order = ORDER.filter(id => id !== 'flat');
    const at = order.indexOf(first);
    same(`${first} comes right before ${then}`, order[at + 1] ?? null, then);
}
// ...and with it in the union: after a call and a recording, never before them, and it is no ALONE
{
    const flat = ORDER.includes('flat');
    if (!flat)
        ORDER.splice(ORDER.indexOf('recording') + 1, 0, 'flat');
    same('a call comes before flat', ORDER.indexOf('call') < ORDER.indexOf('flat'), true);
    same('a recording comes before flat', ORDER.indexOf('recording') < ORDER.indexOf('flat'), true);
    same('flat comes before anything else', ORDER.indexOf('flat') < ORDER.indexOf('present'), true);
    same('a call keeps the place against flat', list('flat', 'call').top.id, 'call');
    same('...alone', list('flat', 'call').second, null);
    same('a recording keeps the place against flat', list('flat', 'recording', 'battery').top.id, 'recording');
    same('...alone', list('flat', 'recording', 'battery').second, null);
    same('flat is not alone: the next sits beside it', list('flat', 'battery').second.id, 'battery');
    same('...Present too, behind it', list('flat', 'present').second.id, 'present');
    same('something added last never comes before a call or a recording', ORDER.indexOf('call') < ORDER.indexOf('quiet'), true);
    if (!flat)
        ORDER.splice(ORDER.indexOf('flat'), 1);
}

// --- R7-17: the spring ----------------------------------------------------------------------------------------------
same('the spring starts where it is', springAt(100, 0, 0).map(Math.round), [100, 0]);
near('...and is within half a percent of rest when it says', Math.abs(springAt(100, 0, SETTLE_MS / 1000)[0]), 0, 0.6);
{
    let lowest = 0;
    for (let t = 0; t < 1; t += 0.005)
        lowest = Math.min(lowest, springAt(100, 0, t)[0]);
    count++;
    if (lowest > -2 || lowest < -12)
        failures.push(`the spring overshoots a little: ${lowest}`);
    // A change of mind keeps its speed: the speed it says is the slope of where it goes
    const [x1, v] = springAt(100, 300, 0.1);
    const [x2] = springAt(100, 300, 0.101);
    near('its speed is its slope', v, (x2 - x1) / 0.001, 3);
    const [again, speedAgain] = springAt(x1, v, 0.05);
    const [direct, speedDirect] = springAt(100, 300, 0.15);
    near('carrying on from where it was is the same motion', again, direct, 0.001);
    near('...at the same speed', speedAgain, speedDirect, 0.01);
}

// --- R7-22: the ring round the pill ---------------------------------------------------------------------------------
{
    const lengthOf = pieces => pieces.reduce((sum, p) => sum + (p[0] === 'line' ? Math.hypot(p[3] - p[1], p[4] - p[2]) : p[3] * Math.abs(p[5] - p[4])), 0);
    const whole = lengthOf(ringPieces(200, 28, 1, 1));
    near('a whole ring is the outline', whole, 2 * (200 - 28) + 2 * Math.PI * 13, 0.01);
    near('half is half', lengthOf(ringPieces(200, 28, 1, 0.5)), whole / 2, 0.01);
    same('nothing left draws nothing', ringPieces(200, 28, 1, 0), []);
    same('it starts at the top of the middle', ringPieces(200, 28, 1, 0.01)[0].slice(1, 3), [100, 1]);
    near('beyond whole is whole', lengthOf(ringPieces(200, 28, 1, 7)), whole, 0.01);
}

// --- R7-22: "tea 4 min" -------------------------------------------------------------------------------------------
for (const [text, want] of [
    ['tea 4 min', {name: 'Tea', seconds: 240}],
    ['Tea 4 minutes', {name: 'Tea', seconds: 240}],
    ['oven 1 hour', {name: 'Oven', seconds: 3600}],
    ['eggs 90 secs', {name: 'Eggs', seconds: 90}],
    ['5 min', {name: '', seconds: 300}],
    ['5 min timer', {name: '', seconds: 300}],
    ['timer 5 min', {name: '', seconds: 300}],
    ['timer tea 4 min', {name: 'Tea', seconds: 240}],
    ['green tea 3 min', {name: 'Green tea', seconds: 180}],
    ['tea', null],
    ['4', null],
    ['tea 4', null],
    ['tea 4 km', null],
    ['tea 0 min', null],
    ['tea 100 min', null],
    ['tea 2 hours', null],
    ['remind me 4 min', null],
    ['/etc/passwd 4 min', null],
    ['https://example.org 4 min', null],
    ['2+2 4 min', null],
    ['$(id) 4 min', null],
])
    same(`the words ${JSON.stringify(text)}`, readTimer(text), want);
// Only the words of the table are units: the object's own names are not
for (const word of ['constructor', 'toString', 'hasOwnProperty', '__proto__', 'valueOf'])
    same(`${word} is no unit`, readTimer(`tea 4 ${word}`), null);
same('a long name is cut', readTimer(`${'a'.repeat(40)} 4 min`).name.length, 24);

// --- R7-20 and R7-56: what is moving --------------------------------------------------------------------------------
same('a partial name without its end', partialName('ubuntu.iso.part'), 'ubuntu.iso');
same('...and a download of Chromium\'s', partialName('Unconfirmed 123.crdownload'), 'Unconfirmed 123');
same('nothing says how far', fractionOf([{kind: 'download'}, {kind: 'flatpak'}]), null);
near('how far is by the bytes', fractionOf([{total: 100, done: 50}, {total: 300, done: 0}, {kind: 'backup'}]), 0.125);
near('...and never past the end', fractionOf([{total: 100, done: 500}]), 1);
{
    const heard = [];
    listenToMoving((...a) => heard.push(a));
    reportMoving('download', 'a.part', {});
    listenToMoving(null);
    reportMoving('download', 'b.part', {});
    same('a report with nobody listening goes nowhere', heard.length, 1);
}
same('on the mains it stays awake however low', givesWay(5, 1), false);
same('on battery above 15% it stays awake', givesWay(15, 2), false);
same('on battery below 15% it gives way', givesWay(14, 2), true);
same('...pending discharge too', givesWay(14, 6), true);
same('no battery says nothing', givesWay(0, 0), false);
// Nothing that moves stays for ever, whatever says so (and the machine is held awake twelve hours at most)
{
    const at = (kind, since, seen) => ({kind, since, seen});
    same('a download quiet for five minutes is over', agedOut(at('download', 0, 0), QUIET.download + 1), true);
    same('...one written to a minute ago is not', agedOut(at('download', 0, 200), 260), false);
    same('a phone transfer quiet for five minutes is over', agedOut(at('phone', 0, 0), QUIET.phone + 1), true);
    same('...one that says progress is not', agedOut(at('phone', 0, 299), 560), false);
    same('a Flatpak update is over after its longest', agedOut(at('flatpak', 0, 0), LONGEST.flatpak + 1), true);
    same('...not before', agedOut(at('flatpak', 0, 0), LONGEST.flatpak - 1), false);
    for (const kind of ['files', 'backup', 'phone', 'download', 'flatpak', 'anything'])
        same(`${kind} is over after twelve hours whatever its source says`, agedOut(at(kind, 0, HOLD_LONGEST), HOLD_LONGEST + 1), true);
    same('a copy of Files, which says nothing as it goes, stays within twelve hours', agedOut(at('files', 0, 0), HOLD_LONGEST - 1), false);
}

// --- R7-21: the album's colour --------------------------------------------------------------------------------------
{
    const picture = (...pixels) => new Uint8Array(pixels.flat());
    same('red on black is red', albumColour(picture([0, 0, 0], [200, 20, 20], [0, 0, 0], [200, 20, 20]), 2, 2, 6, 3), [200, 20, 20]);
    same('a grey picture has no colour', albumColour(picture([90, 90, 90], [91, 90, 90], [90, 90, 91], [90, 90, 90]), 2, 2, 6, 3), null);
    const white = [255, 255, 255];
    const dark = [20, 16, 30];
    same('a dark colour under light text stays', plateTint([122, 58, 31], white), [122, 58, 31]);
    for (const colour of [[255, 240, 0], [250, 250, 200], [0, 200, 0]]) {
        const tint = plateTint(colour, white);
        count++;
        if (tint && contrast(tint, white) < 4.5)
            failures.push(`light text reads on ${colour}: ${tint}`);
    }
    const onDark = plateTint([60, 20, 90], dark);
    count++;
    if (onDark && contrast(onDark, dark) < 4.5)
        failures.push(`dark text reads on it: ${onDark}`);
    same('no colour is no tint', plateTint(null, white), null);
}
{
    const silent = barsOf(new Int16Array(400));
    same('silence stands low', silent.map(b => Math.round(b * 100)), [15, 15, 15, 15]);
    const loud = barsOf(new Int16Array(400).fill(30000));
    same('loud stands full', loud, [1, 1, 1, 1]);
    const half = new Int16Array(400);
    half.fill(20000, 0, 100);
    const bars = barsOf(half);
    count++;
    if (!(bars[0] > 0.5 && bars[1] < 0.2 && bars[3] < 0.2))
        failures.push(`each bar is its own quarter: ${bars}`);
}

// --- R7-23: a code in a text is found by the phone link's program (apps/connect/src/codes.rs, cargo test), never by the shell ----
// Here only that this module's own sums no longer hold a rule that reads a text: the shell is told the digits alone

if (failures.length) {
    for (const failure of failures)
        say(`not ok: ${failure}`);
    throw new Error(`${failures.length} of ${count} checks of the pill's sums failed`);
}
say(`ok: ${count} checks of the pill's sums`);
