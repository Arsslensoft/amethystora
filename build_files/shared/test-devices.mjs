// Round 7's devices, what needs no screen (plans/ui-ideas/r7/spec-devices.md): what CUPS says of a print job and what the
// sound server says of a stream. Run under node, as test-search-calc.mjs is:
//   node build_files/shared/test-devices.mjs
// The answers below are what ipptool -c and pactl -f json printed on Fedora 45 against a stand-in printer and a stand-in
// player, cut down to what matters.

import {MAX_JOBS, OVER, admit, askArgv, byPrinter, parseState, parseStates, retryCommand, trouble, validJob} from
    '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/pill/printwords.js';
import {parseSinkInputs, sorted} from
    '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/quick/sounding.js';

const failures = [];
let count = 0;

function same(what, got, want) {
    count++;
    const [a, b] = [JSON.stringify(got), JSON.stringify(want)];
    if (a !== b)
        failures.push(`${what}: expected ${b}, got ${a}`);
}

const HEADER = 'job-state,job-state-reasons,job-impressions,job-impressions-completed,job-media-sheets,job-media-sheets-completed';
const answer = (job, printer) => `${HEADER}\n${job}\nprinter-state,printer-state-reasons\n${printer}\n`;

// Printing, and how far
let view = parseState(answer('processing,job-printing,,2,,2', 'processing,none'));
same('pages without a total', [view.state, view.done, view.total], ['processing', 2, null]);
same('nothing wrong', trouble(view), null);
view = parseState(answer('processing,job-printing,12,3,,', 'processing,none'));
same('pages and a total', [view.done, view.total], [3, 12]);
view = parseState(answer('processing,job-printing,,,,', 'processing,none'));
same('a printer that says nothing', [view.state, view.done, view.total, trouble(view)], ['processing', null, null, null]);
view = parseState(answer('processing,job-printing,2,5,,', 'processing,none'));
same('a count past the total says nothing of the total', [view.done, view.total], [5, null]);

// What is wrong, in the printer's keywords
const wrong = {
    'media-empty-error': 'paper',
    'media-needed': 'paper',
    'media-jam-error': 'jam',
    'toner-empty-error': 'ink',
    'marker-supply-empty-error': 'ink',
    'door-open-report': null,
    'cover-open': 'cover',
    'offline-report': 'offline',
    // The network backends set it during every normal connect: it says nothing is wrong
    'connecting-to-device': null,
    'timed-out-report': 'offline',
    'toner-low-warning': null,
    'media-low-warning': null,
    'cups-waiting-for-job-completed': null,
    'something-new-error': null,
    'none': null,
};
for (const [reason, want]of Object.entries(wrong))
    same(reason, trouble(parseState(answer('processing,job-printing,,,,', `processing,${reason}`))), want);
view = parseState(answer('processing,job-printing,,,,', 'processing,"toner-low-warning,media-empty-error"'));
same('a list of reasons, quoted', [view.printerReasons, trouble(view)], [['toner-low-warning', 'media-empty-error'], 'paper']);

// Words from a printer never get through as words
view = parseState(answer('processing,job-printing,,,,', 'processing,"<b>out</b> of paper,media-empty-error"'));
same('only keywords are kept', view.printerReasons, ['media-empty-error']);

// A stopped queue, a stopped job, a held job: what Try again runs
view = parseState(answer('pending,printer-stopped,,3,,3', 'stopped,paused'));
same('stopped printer', trouble(view), 'stopped');
same('it starts the printer', retryCommand(view, 7, 'TV'), ['/usr/bin/cupsenable', 'TV']);
view = parseState(answer('pending-held,job-hold-until-specified,,,,', 'idle,none'));
same('held', trouble(view), 'held');
same('it lets the job go', retryCommand(view, 7, 'TV'), ['/usr/bin/lp', '-i', '7', '-H', 'resume']);
view = parseState(answer('processing-stopped,job-stopped,,,,', 'idle,none'));
same('a stopped job', retryCommand(view, 7, 'TV'), ['/usr/bin/lp', '-i', '7', '-H', 'resume']);
view = parseState(answer('processing,job-printing,,,,', 'processing,media-empty-error'));
same('paper added carries on by itself: nothing to try', retryCommand(view, 7, 'TV'), null);

// The end of a job, and no answer at all
for (const state of ['canceled', 'aborted', 'completed'])
    same(state, OVER.has(parseState(answer(`${state},x,,,,`, 'idle,none')).state), true);
same('no job', parseState('ipptool: client-error-not-found\n'), null);
same('empty', parseState(''), null);
same('nothing', parseState(null), null);

// What may reach the address and the command line
same('a name', validJob('Living_Room-2', 3), true);
// A leading dash would be an option of cupsenable
for (const [printer, id] of [['a/b', 3], ['a b', 3], ['a#b', 3], ['../x', 3], ['', 3], ['TV', 0], ['TV', 1.5], [3, 3], ['-E', 3], ['--release', 3]])
    same(`refused ${printer} ${id}`, validJob(printer, id), false);

same('a dash inside is a name', validJob('Living-Room', 3), true);

// Jobs of one printer in one request: the answer of Get-Jobs has a row for each job
const JOBS_HEADER = 'job-id,job-state,job-state-reasons,job-impressions,job-impressions-completed,job-media-sheets,job-media-sheets-completed';
const many = parseStates(`${JOBS_HEADER}
1,canceled,none,,0,,0
4,pending-held,job-hold-until-specified,,0,,0
4,pending-held,job-hold-until-specified,,0,,0
2,processing,job-printing,12,3,,
printer-state,printer-state-reasons
stopped,paused
`);
same('one row each, the repeated number once', [...many.keys()], [1, 4, 2]);
same('the held job', trouble(many.get(4)), 'stopped');
same('its pages', [many.get(2).done, many.get(2).total], [3, 12]);
same('the printer is the same for all', many.get(1).printerState, 'stopped');
same('the answer for a job CUPS no longer has', parseStates('Job #9 does not exist.').size, 0);
same('nothing', parseStates(null).size, 0);
same('not CSV', parseStates('<html>').size, 0);

// One request for all the jobs of a printer: the cap, the grouping and the command
const jobs = new Map();
const user = 'alice';
same('another accounts job is not followed', admit(jobs, 'TV', 5, 'bob', user), false);
same('a name that is an option is not followed', admit(jobs, '-E', 5, user, user), false);
for (let id = 1; id <= MAX_JOBS; id++)
    admit(jobs, id % 2 ? 'TV' : 'Lab', id, user, user);
same('up to the cap', jobs.size, MAX_JOBS);
same('over the cap is not followed', [admit(jobs, 'TV', 1000, user, user), jobs.size], [false, MAX_JOBS]);
same('a job already followed is still accepted at the cap', admit(jobs, 'TV', 1, user, user), true);
const groups = byPrinter(jobs);
same('one group for each printer', [...groups.keys()], ['TV', 'Lab']);
same('every job in one of them', [...groups.values()].flat().length, MAX_JOBS);
// 200 queued jobs are a request for each printer of at most the cap, never a process for each job
const flood = new Map();
for (let id = 1; id <= 200; id++)
    admit(flood, 'TV', id, user, user);
same('a flood of jobs is capped', flood.size, MAX_JOBS);
same('and asked for in one request', byPrinter(flood).size, 1);
const argv = askArgv('/usr/bin/ipptool', '/q.test', user, 'Living Room', [7, 9]);
same('the tool, its time limit and the account', argv.slice(0, 6), ['/usr/bin/ipptool', '-c', '-T', '10', '-d', 'user=alice']);
same('every place carries a number', argv.filter(word => /^j[0-9]+=/.test(word)).length, MAX_JOBS);
same('the numbers, the last repeating', argv.filter(word => /^j[0-9]+=/.test(word)).slice(0, 4), ['j1=7', 'j2=9', 'j3=7', 'j4=7']);
same('the address and the question', argv.slice(-2), ['ipp://localhost/printers/Living%20Room', '/q.test']);

// The sound server: three facts of a stream, and what is playing is dropped
const CANARY = 'Secret film title';
const said = parseSinkInputs(JSON.stringify([
    {index: 58, corked: false, properties: {'media.name': CANARY, 'application.name': 'WEBRTC VoiceEngine',
        'pipewire.access.portal.app_id': 'com.example.Chat', 'application.process.binary': 'chat'}},
    {index: 69, corked: true, properties: {'media.name': CANARY, 'application.process.binary': 'firefox'}},
    {index: 70, corked: true, properties: {'pipewire.access.portal.app_id': '$(rm -rf)', 'application.process.binary': 'a b/c'}},
    {index: 'x', corked: true},
    null,
]));
same('streams', [...said.keys()], [58, 69, 70]);
same('a sandboxed app', said.get(58), {corked: false, app: 'com.example.Chat', binary: 'chat'});
same('a paused program', said.get(69), {corked: true, app: null, binary: 'firefox'});
same('names that are not names', said.get(70), {corked: true, app: null, binary: null});
same('nothing of what plays is kept', JSON.stringify([...said.values()]).includes(CANARY), false);
same('not pactl\'s list', parseSinkInputs('Connection failure').size, 0);
same('not a list', parseSinkInputs('{}').size, 0);

const apps = [{name: 'Zed', key: 'z', paused: false}, {name: 'Alpha', key: 'a', paused: true}, {name: 'Mid', key: 'm', paused: false}];
same('sounding first', sorted(apps, true).map(app => app.name), ['Mid', 'Zed', 'Alpha']);
same('with the key off, by name as before', sorted(apps, false).map(app => app.name), ['Alpha', 'Mid', 'Zed']);

if (failures.length > 0) {
    console.log(failures.join('\n'));
    throw new Error(`${failures.length} of ${count} failed`);
}
console.log(`test-devices: ${count} passed`);
