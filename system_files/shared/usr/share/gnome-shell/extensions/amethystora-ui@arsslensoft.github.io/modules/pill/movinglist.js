// What is moving, as the parts that already watch it say so (the dock's Downloads stack and its Flatpak watcher), and the
// sums the pill makes of it (R7-20, R7-56). While the pill has no listener a report costs one comparison. Free of GNOME
// Shell's imports, so that it runs under node (test-pill-logic.mjs).

let listener = null;

export function listenToMoving(callback) {
    listener = callback;
}

// kind: 'download' or 'flatpak'; key: its own name there; info: {...} while it moves, null when it has stopped
export function reportMoving(kind, key, info) {
    listener?.(kind, key, info);
}

// What a browser calls a download while it comes
export const PARTIAL = /\.(part|crdownload|download|partial|tmp)$/i;

export function partialName(name) {
    return String(name).replace(PARTIAL, '');
}

// How far it has all got: the share of the bytes of what says how many it has, or null when nothing says
export function fractionOf(items) {
    let [done, total] = [0, 0];
    for (const item of items) {
        if (item.total > 0) {
            done += Math.min(Math.max(item.done ?? 0, 0), item.total);
            total += item.total;
        }
    }
    return total > 0 ? done / total : null;
}

// The most a laptop's own battery is spent on staying awake: below this, off the mains, it gives way (R7-56). UPower's
// states: 2 discharging, 6 pending discharge
const LOWEST = 15;

export function givesWay(percent, state) {
    return (state === 2 || state === 6) && Number.isFinite(percent) && percent > 0 && percent < LOWEST;
}

// Seconds: a download nothing has been written to for this long is not coming any more
export const STALE_DOWNLOAD = 5 * 60;

// Nothing that moves stays for ever, whatever it is and whoever says it (R7-20, R7-56): an item is over when its source has
// said nothing for QUIET[kind] seconds, or when it has been there longer than LONGEST[kind]; and the machine is kept awake for
// at most HOLD_LONGEST seconds at a stretch, whatever comes after. Times are the monotonic clock's, in seconds.
export const QUIET = {download: STALE_DOWNLOAD, phone: 5 * 60};
export const LONGEST = {flatpak: 2 * 3600};
export const HOLD_LONGEST = 12 * 3600;

// item: {kind, since, seen}
export function agedOut(item, now) {
    const quiet = QUIET[item.kind];
    if (quiet !== undefined && now - item.seen > quiet)
        return true;
    return now - item.since > (LONGEST[item.kind] ?? HOLD_LONGEST);
}
