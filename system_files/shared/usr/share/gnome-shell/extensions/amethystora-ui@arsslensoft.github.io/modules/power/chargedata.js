// Charged by the time you leave (R7-70) as data: when a person usually unplugs, and when the battery's hold has to come off for it to be
// full by then. Nothing here touches GNOME, so that the build checks it under node (build_files/shared/test-charge-by.mjs); charge.js
// does the rest.
//
// The pattern is only ever the minute of the day and whether it was a weekend of each unplug that followed a night on mains (at least
// six hours), the last thirty: no date, no place, no device. It is sealed in the Vault (purpose `charge-pattern`); the leaving time it
// works out is only ever held in memory.

export const KEEP = 30;
export const PURPOSE = 'charge-pattern';
export const RECORD = 'list';
export const FILE_NAME = 'charge-pattern.avs';
// An unplug counts only after this long on mains: a trip to another desk is no leaving
export const MIN_ON_MAINS_MS = 6 * 3600 * 1000;
// At least this many unplugs of a kind before one is guessed at
export const MIN_SAMPLES = 3;
// A hold is only worth it with this long left, and is taken off this long before the person leaves, for a charge from the limit to full
export const MIN_HOLD_MS = 4 * 3600 * 1000;
export const CHARGE_MS = 75 * 60 * 1000;
// An event of the calendar earlier than the usual time moves it, by this much before it
export const EVENT_LEAD_MS = 30 * 60 * 1000;

export const minuteOf = date => date.getHours() * 60 + date.getMinutes();
export const isWeekend = date => date.getDay() === 0 || date.getDay() === 6;

const isMinute = v => Number.isInteger(v) && v >= 0 && v < 1440;

// What fits the shape this keeps, and nothing else: the last KEEP
export function clean(list) {
    if (!Array.isArray(list))
        return [];
    return list.filter(e => e && typeof e === 'object' && isMinute(e.m) && typeof e.we === 'boolean')
        .map(e => ({m: e.m, we: e.we})).slice(-KEEP);
}

export const encode = list => new TextEncoder().encode(JSON.stringify({version: 1, unplugs: clean(list)}));

// The list sealed bytes held, or null when they are no list
export function decode(bytes) {
    try {
        const data = JSON.parse(new TextDecoder().decode(bytes));
        if (data && data.version === 1 && Array.isArray(data.unplugs))
            return clean(data.unplugs);
    } catch {}
    return null;
}

// The list with an unplug at `unpluggedMs`, after `pluggedMs` on mains, when that was a night; otherwise as it was
export function learn(list, unpluggedMs, pluggedMs) {
    if (!Number.isFinite(unpluggedMs) || !Number.isFinite(pluggedMs) || unpluggedMs - pluggedMs < MIN_ON_MAINS_MS)
        return list;
    const at = new Date(unpluggedMs);
    return clean([...list, {m: minuteOf(at), we: isWeekend(at)}]);
}

// What was known from the file and what this session saw: the file's first, then this session's, the last KEEP
export const merge = (stored, fresh) => clean([...stored, ...fresh]);

// The usual minute of leaving on a weekday or a weekend: the median of the unplugs of that kind, or null with too few
export function usual(list, weekend) {
    const minutes = list.filter(e => e.we === weekend).map(e => e.m).sort((a, b) => a - b);
    if (minutes.length < MIN_SAMPLES)
        return null;
    const mid = Math.floor(minutes.length / 2);
    return minutes.length % 2 ? minutes[mid] : Math.round((minutes[mid - 1] + minutes[mid]) / 2);
}

// The next time the person is expected to leave after `nowMs`, in milliseconds, or null while nothing is known
export function nextLeave(list, nowMs) {
    const now = new Date(nowMs);
    for (let ahead = 0; ahead <= 2; ahead++) {
        const day = new Date(now.getFullYear(), now.getMonth(), now.getDate() + ahead);
        const minute = usual(list, isWeekend(day));
        if (minute === null)
            continue;
        const at = new Date(day.getFullYear(), day.getMonth(), day.getDate(), Math.floor(minute / 60), minute % 60).getTime();
        if (at > nowMs)
            return at;
    }
    return null;
}

// An event that starts before the usual time (a list of start times in milliseconds, nothing else of it) moves the leaving time to
// EVENT_LEAD_MS before it, when that is still ahead
export function withCalendar(leaveMs, starts, nowMs) {
    let best = leaveMs;
    for (const start of starts) {
        const wanted = start - EVENT_LEAD_MS;
        if (Number.isFinite(start) && wanted > nowMs && wanted < best)
            best = wanted;
    }
    return best;
}

// What to do now that the computer is on mains and the person is expected to leave at `leaveMs`: hold at the limit until
// `releaseAt`, or charge fully (hold false) when there is not enough time left to make a hold worth it, or nothing is known
export function plan(nowMs, leaveMs) {
    if (leaveMs === null || !Number.isFinite(leaveMs) || leaveMs - nowMs < MIN_HOLD_MS)
        return {hold: false, releaseAt: null};
    return {hold: true, releaseAt: leaveMs - CHARGE_MS};
}
