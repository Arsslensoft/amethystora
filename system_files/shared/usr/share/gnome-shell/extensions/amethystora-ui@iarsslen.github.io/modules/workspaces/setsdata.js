// The saved sets as data: what a set file may hold, and where a set's window goes in a strip. Nothing here touches
// GNOME, so that the build checks it under node (build_files/shared/test-sets.mjs); sets.js does the rest.

export const MAX_SETS = 12;
export const MAX_WINDOWS = 16;
export const MAX_NAME = 24;
// A file larger than this was not written here, and is not read
export const MAX_BYTES = 256 * 1024;

const isObject = v => !!v && typeof v === 'object' && !Array.isArray(v);
const isInt = (v, lo, hi) => Number.isInteger(v) && v >= lo && v <= hi;
// Control characters, the line breaks among them, never belong in a name
const CONTROL = /[\u0000-\u001f\u007f-\u009f\u2028\u2029]/u;

// Invisible format characters (the ones that turn text round, among others), which could make a name or an app's ID
// read as something else
const FORMAT = /\p{Cf}/u;
// ...but the two joiners belong to words and emoji: a name keeps them
const JOINERS = ['\u200c', '\u200d'];

// A name as it may be kept: its control and format characters taken out (the joiners kept), cut to MAX_NAME
// characters, and the spaces at its ends trimmed; '' when nothing is left
export function cleanName(text) {
    if (typeof text !== 'string')
        return '';
    const kept = Array.from(text).filter(c => !CONTROL.test(c) && (!FORMAT.test(c) || JOINERS.includes(c)));
    return kept.slice(0, MAX_NAME).join('').trim();
}

export const isName = v => typeof v === 'string' && v !== '__proto__' && cleanName(v) === v && v !== '';
// A desktop file ID as GLib allows one (a launcher's name under applications/, its folders joined by -), spaces, +
// and @ included: never a slash, a control or a format character, never starting with a dot. Whether it names an
// installed app is for GNOME's list of apps to say, where an opening looks it up.
export const isAppId = v => typeof v === 'string' && new TextEncoder().encode(v).length <= 255 &&
    v.endsWith('.desktop') && v.length > '.desktop'.length && !v.startsWith('.') &&
    Array.from(v).every(c => c !== '/' && !CONTROL.test(c) && !FORMAT.test(c));

// A window's place on its screen, as windows/places.js keeps them
export const isPlace = p => isObject(p) && typeof p.m === 'string' && p.m.length > 0 && p.m.length <= 256 &&
    isInt(p.x, -32768, 32768) && isInt(p.y, -32768, 32768) && isInt(p.w, 50, 16384) && isInt(p.h, 50, 16384) &&
    typeof p.max === 'boolean';

export const isTile = t => isObject(t) && isInt(t.column, 0, MAX_WINDOWS - 1) && isInt(t.row, 0, MAX_WINDOWS - 1) &&
    isInt(t.width, 50, 16384);

function cleanWindow(w) {
    if (!isObject(w) || !isAppId(w.app) || !isInt(w.rank, 0, MAX_WINDOWS - 1))
        return null;
    const window = {app: w.app, rank: w.rank};
    if (isPlace(w.place))
        window.place = {m: w.place.m, x: w.place.x, y: w.place.y, w: w.place.w, h: w.place.h, max: w.place.max};
    if (isTile(w.tile))
        window.tile = {column: w.tile.column, row: w.tile.row, width: w.tile.width};
    return window;
}

function cleanSet(s) {
    if (!isObject(s) || !isName(s.name) || !Number.isFinite(s.saved) || !['classic', 'tiling'].includes(s.layout) ||
        !Array.isArray(s.windows))
        return null;
    const windows = s.windows.slice(0, MAX_WINDOWS).map(cleanWindow).filter(Boolean);
    // Two windows of one app with the same rank: the second is dropped, so that each claim is one window
    const seen = new Set();
    const kept = windows.filter(w => {
        const key = `${w.app} ${w.rank}`;
        if (seen.has(key))
            return false;
        seen.add(key);
        return true;
    });
    if (!kept.length)
        return null;
    return {name: s.name, saved: Math.floor(s.saved), layout: s.layout, windows: kept};
}

// What fits the shape this module writes, and nothing else: newest first, one set per name, at most MAX_SETS
export function clean(data) {
    const result = {version: 1, sets: []};
    if (!isObject(data) || data.version !== 1 || !Array.isArray(data.sets))
        return result;
    const sets = data.sets.slice(0, 4 * MAX_SETS).map(cleanSet).filter(Boolean);
    sets.sort((a, b) => b.saved - a.saved);
    const names = new Set();
    for (const set of sets) {
        if (names.has(set.name))
            continue;
        names.add(set.name);
        result.sets.push(set);
    }
    result.sets.length = Math.min(result.sets.length, MAX_SETS);
    return result;
}

// The apps open before an update's restart (workspaces/resume.js), as resume.json may hold them: each workspace's
// windows in a saved set's shape, by the workspace's place, at most MAX_SETS workspaces; `from` is the system that wrote
// it (IMAGE_VERSION and BUILD_ID), `kind` who asked ('restart', or 'locked' for the snapshot a lock takes), `active` the
// workspace that was in front. null when nothing fits.
const VERSION_ID = /^[A-Za-z0-9._:+-]{1,96}$/;
export function cleanResume(data) {
    if (!isObject(data) || data.version !== 1 || !Number.isFinite(data.saved) || typeof data.from !== 'string' ||
        !VERSION_ID.test(data.from) || !['restart', 'locked'].includes(data.kind) || !Array.isArray(data.workspaces))
        return null;
    const seen = new Set();
    const workspaces = [];
    for (const entry of data.workspaces.slice(0, 4 * MAX_SETS)) {
        if (!isObject(entry) || !isInt(entry.index, 0, 63) || seen.has(entry.index))
            continue;
        const set = cleanSet({name: `${entry.index + 1}`, saved: data.saved, layout: entry.layout,
            windows: entry.windows});
        if (!set)
            continue;
        seen.add(entry.index);
        workspaces.push({index: entry.index, layout: set.layout, windows: set.windows});
        if (workspaces.length === MAX_SETS)
            break;
    }
    if (!workspaces.length)
        return null;
    workspaces.sort((a, b) => a.index - b.index);
    return {
        version: 1, saved: Math.floor(data.saved), from: data.from, kind: data.kind,
        active: isInt(data.active, 0, 63) ? data.active : workspaces[0].index, workspaces,
    };
}

// A set added to the list: one of the same name gives way to it, and past MAX_SETS the oldest go
export function addSet(data, set) {
    // First, so that it stays ahead of sets saved in the same second
    const sets = [set, ...data.sets.filter(s => s.name !== set.name)];
    sets.sort((a, b) => b.saved - a.saved);
    return {version: 1, sets: sets.slice(0, MAX_SETS)};
}

// Each window's place in a strip, in the set's order: the saved one when it has one; a window saved without one (in
// the classic layout, or floating) takes a column of its own after them, by the left-to-right order of its centre,
// at its saved width
export function tileOrder(set) {
    const tiled = set.windows.filter(w => w.tile);
    let next = tiled.length ? Math.max(...tiled.map(w => w.tile.column)) + 1 : 0;
    const loose = set.windows
        .map((w, index) => ({w, index}))
        .filter(({w}) => !w.tile)
        .sort((a, b) => centre(a.w) - centre(b.w) || a.index - b.index);
    const order = set.windows.map(w => w.tile ? {...w.tile} : null);
    for (const {w, index} of loose)
        order[index] = {column: next++, row: 0, width: w.place?.w ?? 0};
    return order;
}

function centre(w) {
    return w.place ? w.place.x + w.place.w / 2 : 0;
}

// Where a window of an opening goes in a strip: `columns` holds, for each column of the strip from the left, the
// saved tiles of the opening's windows already in it ([] for a column of other windows), and `tile` is the new
// window's saved tile. It joins the column of its saved column when one is there, under the windows saved above it;
// else it takes a column of its own before the first column saved to its right, or after the opening's last column,
// or, while none of the opening's windows is there yet, after every window of the workspace.
export function slot(columns, tile) {
    const same = columns.findIndex(tiles => tiles.some(t => t.column === tile.column));
    if (same >= 0)
        return {column: same, row: columns[same].filter(t => t.row < tile.row).length};
    const right = columns.findIndex(tiles => tiles.some(t => t.column > tile.column));
    if (right >= 0)
        return {column: right};
    let last = -1;
    columns.forEach((tiles, index) => {
        if (tiles.length)
            last = index;
    });
    return {column: last >= 0 ? last + 1 : columns.length};
}
