// What the battery lost while the computer slept, and what to say of it: nothing of the shell here, so that
// build_files/shared/test-truth.mjs runs it under node.

// Unusual: at least this many points gone, faster than a sleeping laptop loses them, over at least half an hour
const LARGE_DROP = 10;
const LARGE_RATE = 1.5;
const LEAST_SLEEP = 1800;
// The two readings are taken as the computer goes to sleep and as it wakes, so the time awake between them is seconds.
// More than this and something is off (a reading that came late), and the loss cannot be put down to sleep (s)
const MOST_AWAKE = 120;
// The last wake is the person's own only when it was just now; an older one cannot be told from a wake of the computer's (s)
const FRESH = 300;
// A night that woke more often than this is told by its number, not by its times
const MOST_TIMES = 6;

// Seconds asleep from two readings of (time since boot with sleep) minus (time since boot without it), and whether the
// drop is worth a sentence. `before` and `now` are {percent, suspended (s), mono (µs)}; a battery that charged says nothing.
export function judge(before, now) {
    const drop = Math.round(before.percent - now.percent);
    const slept = now.suspended - before.suspended;
    if (!(slept >= LEAST_SLEEP) || drop < LARGE_DROP || (now.mono - before.mono) / 1e6 > MOST_AWAKE)
        return {large: false, drop, slept};
    return {large: drop / (slept / 3600) >= LARGE_RATE, drop, slept};
}

// /proc/uptime's first number, less the monotonic clock's: the seconds this boot has spent asleep
export function suspendedSeconds(uptimeText, monotonicMicroseconds) {
    const uptime = Number.parseFloat(String(uptimeText).split(/\s/)[0]);
    return Number.isFinite(uptime) ? Math.max(0, uptime - monotonicMicroseconds / 1e6) : 0;
}

// The kinds of source worth naming: a device a person can recognise and, for most, stop. A hub, a USB controller, the lid, the
// power button and the charger are never the culprit
const NAMEABLE = ['usb', 'bluetooth', 'network', 'timer'];

// From what amethystora-sleep-report says ({readable, wakes: [epoch...], last_wake_ago_s, sources: [{id, kind, name, can_stop}]}):
// what is to be said. The helper has already left out every source that fired at or after the last wake, which is the
// person's own (a key, the mouse, the lid): what remains fired earlier. Where the journal cannot be read or says nothing, or
// the last wake was not just now, nothing is known and nothing is claimed.
//   culprit: the source to name, or null; times: the wakes before the person's; say: which sentence;
//   mark: what makes this "said" (the device, or the day for a loss with nothing to name)
export function reading(why) {
    const nothing = {culprit: null, times: [], say: 'silent'};
    const wakes = Array.isArray(why?.wakes) ? why.wakes.filter(Number.isFinite) : [];
    if (why?.readable !== true || wakes.length === 0)
        return nothing;
    if (!Number.isFinite(why.last_wake_ago_s) || why.last_wake_ago_s > FRESH)
        return nothing;
    const times = wakes.slice(0, -1);
    if (times.length === 0)
        return {culprit: null, times: [], say: 'asleep'};
    const sources = (Array.isArray(why.sources) ? why.sources : [])
        .filter(source => typeof source?.id === 'string' && NAMEABLE.includes(source.kind));
    const culprit = sources[0] ?? null;
    return {culprit, times, say: culprit ? 'woken' : 'woken-unnamed'};
}

// How many times are listed, and whether the number says it better
export function listTimes(times) {
    return times.length <= MOST_TIMES ? times : null;
}

// The marks kept: at most this many, the newest last. The file also holds a random salt, which keeps its checksums from being
// looked up in a table of device IDs
const MOST_MARKS = 32;

export function remember(state, mark) {
    const said = (Array.isArray(state?.said) ? state.said : []).filter(entry => typeof entry === 'string' && entry !== mark);
    said.push(mark);
    const salt = typeof state?.salt === 'string' ? state.salt : undefined;
    return {version: 1, ...(salt ? {salt} : {}), said: said.slice(-MOST_MARKS)};
}

export function wasSaid(state, mark) {
    return Array.isArray(state?.said) && state.said.includes(mark);
}
