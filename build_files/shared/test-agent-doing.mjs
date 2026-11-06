// What the agent's tool server says it is doing (R7-13): the session file's `now` and `recent` are read for the tools' own names
// and nothing else, the pill's line picks the running step or the one that just finished, and every step has words. No screen:
//   node build_files/shared/test-agent-doing.mjs
// A failure throws, which makes node exit with an error.

const UI = '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules';
const {LINGER, STALE, TOOLS, TOPICS, lingerLeft, parse, phrase, pick, rows} = await import(`${UI}/agent/doingmodel.js`);

const say = globalThis.print ?? console.log;
const failures = [];
let count = 0;

function same(what, got, want) {
    count++;
    if (JSON.stringify(got) !== JSON.stringify(want))
        failures.push(`${what}: expected ${JSON.stringify(want)}, got ${JSON.stringify(got)}`);
}

// The id passes the words through unchanged, so that a missing phrase shows as the empty string and every one is one of ours
const _ = text => text;

// --- Reading a file: only fixed names ---------------------------------------------------------------------------------------
same('a broken file says nothing', parse('{nope'), {now: null, recent: []});
same('an array says nothing', parse('[1]'), {now: null, recent: []});
same('a tool of the list', parse('{"now":{"tool":"search","at":10},"recent":[]}').now, {tool: 'search', topic: null, at: 10});
same('a tool that changes things is left out', parse('{"now":{"tool":"software","at":10}}').now, null);
same('a topic outside the list is dropped', parse('{"now":{"tool":"state","topic":"drivers; rm","at":10}}').now.topic, null);
same('a topic of another tool is dropped', parse('{"now":{"tool":"read","topic":"updates","at":10}}').now.topic, null);
same('a topic of the list', parse('{"now":{"tool":"state","topic":"updates","at":10}}').now.topic, 'updates');
same('only the last three finished steps', parse(JSON.stringify({recent: [1, 2, 3, 4, 5].map(at => ({tool: 'read', at}))})).recent.map(s => s.at), [3, 4, 5]);
same('free text in a step is never kept', JSON.stringify(parse('{"now":{"tool":"read","at":1,"query":"my password","text":"secret"}}')).includes('secret'), false);

// --- What the pill says -------------------------------------------------------------------------------------------------------
const run = (tool, at, topic = null) => ({now: {tool, topic, at}, recent: []});
const done = (...steps) => ({now: null, recent: steps.map(([tool, at, topic = null]) => ({tool, topic, at}))});
same('nothing running, nothing said', pick([{now: null, recent: []}], 100), null);
same('a running tool', pick([run('search', 99)], 100).running, true);
same('the newest of two servers', pick([run('search', 90), run('state', 99, 'disks')], 100).step.tool, 'state');
same('a step that finished a moment ago keeps its tick', pick([done(['read', 98])], 100), {running: false, step: {tool: 'read', topic: null, at: 98}});
same('and loses it after the linger', pick([done(['read', 100 - LINGER - 1])], 100), null);
same('a server that died while running is not still running', pick([run('search', 100 - STALE - 1)], 100), null);
same('the tick ends by one timer', lingerLeft(pick([done(['read', 98])], 100), 100), LINGER - 2);
same('a running step has no timer', lingerLeft(pick([run('read', 99)], 100), 100), 0);
const card = rows([done(['search', 90], ['read', 91]), run('state', 95, 'backups')], 100);
same('the card lists what is done, oldest first, and what runs now', [card.done.map(s => s.tool), card.now.topic], [['search', 'read'], 'backups']);

// --- Every tool and topic has words, present and past ----------------------------------------------------------------------
for (const tool of TOOLS) {
    for (const present of [true, false])
        same(`${tool} has words (present ${present})`, phrase({tool, topic: null}, present, _).length > 0, true);
}
for (const topic of TOPICS) {
    same(`state ${topic} has its own words`, phrase({tool: 'state', topic}, true, _) !== phrase({tool: 'state', topic: null}, true, _), true);
    same(`state ${topic} says what is done differently`, phrase({tool: 'state', topic}, true, _) !== phrase({tool: 'state', topic}, false, _), true);
}
same('an unknown tool has none', phrase({tool: 'rm', topic: null}, true, _), '');

if (failures.length) {
    for (const failure of failures)
        say(`not ok: ${failure}`);
    throw new Error(`${failures.length} of ${count} checks of what the agent is doing failed`);
}
say(`ok: ${count} checks of what the agent is doing`);
