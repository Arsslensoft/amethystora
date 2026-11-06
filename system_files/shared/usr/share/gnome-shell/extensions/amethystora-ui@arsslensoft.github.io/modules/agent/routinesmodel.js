// Little routines in the shell (R7-15), the parts that need no screen: what the helper says about the routines, which of them an
// event wakes, when a time trigger is next due, and what the pill says of a run. No imports: Node reads this file in
// build_files/shared/test-agent-routines.mjs. The routines themselves are the helper's (apps/agent-tools/src/routines.rs): sealed in
// the Vault, checked, and run there; the shell only listens for the triggers they name and asks the helper to fire.

// Seconds a routine is left alone after it fired, so that a trigger that flaps (a network that drops and joins) runs it once
export const COOL_OFF = 10;
// Seconds a run stays in the pill, with Undo and Pause
export const SHOWN = 60;

const WEEKDAYS = ['mon', 'tue', 'wed', 'thu', 'fri'];
const WEEKEND = ['sat', 'sun'];
const DAYS = ['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'];

// What `routines triggers` printed, checked: {state, routines: [{id, name, when, paused, while_locked, undoable, when_words, do}]}.
// Anything else is no routines, said as `error`.
export function parseTriggers(text) {
    let body;
    try {
        body = JSON.parse(text);
    } catch {
        return {state: 'error', routines: []};
    }
    if (!body || typeof body !== 'object' || typeof body.state !== 'string')
        return {state: 'error', routines: []};
    if (body.state !== 'ok')
        return {state: body.state, routines: [], said: typeof body.said === 'string' ? body.said : ''};
    const routines = (Array.isArray(body.routines) ? body.routines : []).filter(r => r && typeof r.id === 'string' &&
        /^[a-z0-9-]{1,40}$/.test(r.id) && r.when && typeof r.when === 'object').map(r => ({
        id: r.id, name: String(r.name ?? r.id).slice(0, 60), when: r.when, paused: r.paused === true, whileLocked: r.while_locked === true,
        undoable: r.undoable === true, whenWords: String(r.when_words ?? ''), failed: typeof r.failed === 'string' ? r.failed : null,
    }));
    return {state: 'ok', routines};
}

// The sources a set of routines needs, and only those: `time`, `network`, `app`, `screen`, `battery`, `phone`
export function sourcesNeeded(routines) {
    const need = new Set();
    for (const routine of routines) {
        if (routine.paused)
            continue;
        for (const kind of ['time', 'network', 'app', 'screen', 'battery', 'phone']) {
            if (kind in routine.when)
                need.add(kind);
        }
    }
    return need;
}

// Whether `event` wakes `when`. Events: {kind: 'network', name}, {kind: 'app', id, state: 'opens'|'closes'|'fullscreen'},
// {kind: 'screen', state: 'locks'|'unlocks'}, {kind: 'battery', percent, onBattery, was: {percent, onBattery}}, {kind: 'phone', state:
// 'near'|'away', name}. A time trigger is woken by its own timer, never by an event.
export function wakes(when, event) {
    switch (event.kind) {
    case 'network':
        return typeof when.network === 'string' && when.network === event.name;
    case 'app':
        return typeof when.app === 'string' && when.app === event.id && (when.state ?? 'opens') === event.state;
    case 'screen':
        return when.screen === event.state;
    case 'battery': {
        const kind = when.battery;
        if (kind === 'on-battery')
            return event.onBattery === true && event.was?.onBattery === false;
        if (kind === 'plugged-in')
            return event.onBattery === false && event.was?.onBattery === true;
        const below = /^below ([0-9]{1,2})$/.exec(kind ?? '')?.[1];
        // Crossing down through the line while running on the battery, once
        return below !== undefined && event.onBattery === true && event.percent < Number(below) && (event.was?.percent ?? 100) >= Number(below);
    }
    case 'phone':
        return when.phone === event.state && (when.name === undefined || when.name === event.name);
    default:
        return false;
    }
}

// The routines an event wakes: not paused, not within COOL_OFF of their last run. `last` maps an id to the seconds it last ran.
export function routinesFor(routines, event, last, at) {
    return routines.filter(r => !r.paused && wakes(r.when, event) && !(last.get(r.id) !== undefined && at - last.get(r.id) < COOL_OFF));
}

// Whether a time trigger's days include the day `date` falls on
export function onDay(when, date) {
    const days = when.days ?? 'daily';
    const day = DAYS[date.getDay()];
    if (days === 'daily')
        return true;
    if (days === 'weekdays')
        return WEEKDAYS.includes(day);
    if (days === 'weekends')
        return WEEKEND.includes(day);
    return Array.isArray(days) && days.includes(day);
}

// The next moment (a Date) some time trigger is due after `now`, with the routines due then, or null. Looks a week ahead.
export function nextDue(routines, now) {
    let best = null;
    for (const routine of routines) {
        const match = /^([01][0-9]|2[0-3]):([0-5][0-9])$/.exec(routine.when.time ?? '');
        if (routine.paused || !match)
            continue;
        for (let ahead = 0; ahead < 8; ahead++) {
            const due = new Date(now.getFullYear(), now.getMonth(), now.getDate() + ahead, Number(match[1]), Number(match[2]), 0, 0);
            if (due.getTime() <= now.getTime() || !onDay(routine.when, due))
                continue;
            if (!best || due.getTime() < best.at.getTime())
                best = {at: due, ids: [routine.id]};
            else if (due.getTime() === best.at.getTime())
                best.ids.push(routine.id);
            break;
        }
    }
    return best;
}

// A run's result as the helper printed it, checked: {id, name, results: [{kind, what, ok, undo}], failed, undoable} or null
export function parseRan(text) {
    let body;
    try {
        body = JSON.parse(text);
    } catch {
        return null;
    }
    if (!body || body.state !== 'ran' || typeof body.id !== 'string' || !Array.isArray(body.results))
        return null;
    return {
        id: body.id, name: String(body.name ?? body.id).slice(0, 60), failed: typeof body.failed === 'string' ? body.failed : null,
        undoable: body.undoable === true,
        results: body.results.filter(r => r && ['open', 'ame'].includes(r.kind)).map(r => ({kind: r.kind, what: String(r.what ?? '').slice(0, 120), ok: r.ok === true})),
    };
}
