// The desks the account set up, and what the desk it is at changed: ~/.config/amethystora/desks.json, written whole
// with a copy beside it (desks.json.bak) for the day it is lost or cannot be read, in a folder only the account can
// read. `ame desktop desks rename|saved|forget|ask` edits it the same way. Nothing in it is ever run, shown as markup or
// used as a path; what does not have the shape below is left out, and a file that cannot be read stops nothing.
//
//   {"version": 1,
//    "desks": [{"id": "0123456789ab", "base": "0123456789ab", "name": "Office desk" | null, "seen": 1761650000,
//               "builtin": true, "screens": [{"vendor", "product", "serial", "connector", "name"}], "thunderbolt": 0,
//               "set": SET}],
//    "declined": ["0123456789ab"],
//    "current": {"desk": "0123456789ab", "before": SET, "wrote": SET}}
//
//   SET: {"sound": {"sink", "port", "label"}, "dock": {"edge", "screens"}, "focus": "work", "power": "balanced",
//         "saved": "Writing"}, every part optional
//
// `declined` are the desks Not now was said to: the card does not ask there again until `ame desktop desks ask`.
// The recipe and this module each write the file whole; before changing it, this reads it again if somebody else wrote
// it since, so that neither loses the other's change (the recipe also holds a lock while it writes).

import GLib from 'gi://GLib';

const DIR = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora']);
export const FILE = GLib.build_filenamev([DIR, 'desks.json']);
const COPY = `${FILE}.bak`;
const MOST_BYTES = 64 * 1024;
export const MOST_DESKS = 24;
// A desk's name, in characters (as the recipe counts them)
export const MOST_NAME = 40;

export const EDGES = ['bottom', 'left', 'right'];
export const DOCK_SCREENS = ['every', 'primary'];
export const FOCUS = ['off', 'work', 'games', 'sleep'];
export const POWER = ['balanced', 'performance', 'power-saver'];

const isObject = value => !!value && typeof value === 'object' && !Array.isArray(value);
// Text as the file may hold it: no control or format characters (a right-to-left override would show a name
// reversed), at most `most` characters
const isText = (value, most, empty = true) => typeof value === 'string' && [...value].length <= most &&
    (empty || value.length > 0) && !/[\p{Cc}\p{Cf}]/u.test(value);
const isId = value => typeof value === 'string' && /^[0-9a-f]{12}$/.test(value);

export function cleanSet(set) {
    const out = {};
    if (!isObject(set))
        return out;
    const sound = set.sound;
    if (isObject(sound) && isText(sound.sink, 256, false) && isText(sound.port ?? '', 128) &&
        isText(sound.label ?? '', 128))
        out.sound = {sink: sound.sink, port: sound.port ?? '', label: sound.label ?? ''};
    const dock = set.dock;
    if (isObject(dock) && EDGES.includes(dock.edge) && DOCK_SCREENS.includes(dock.screens))
        out.dock = {edge: dock.edge, screens: dock.screens};
    if (FOCUS.includes(set.focus))
        out.focus = set.focus;
    if (POWER.includes(set.power))
        out.power = set.power;
    if (isText(set.saved, 24, false))
        out.saved = set.saved;
    return out;
}

function cleanScreen(screen) {
    if (!isObject(screen))
        return null;
    const out = {};
    for (const key of ['vendor', 'product', 'serial', 'connector', 'name']) {
        if (!isText(screen[key] ?? '', 64))
            return null;
        out[key] = screen[key] ?? '';
    }
    return out;
}

function cleanDesk(desk) {
    if (!isObject(desk) || !isId(desk.id))
        return null;
    const screens = Array.isArray(desk.screens) ? desk.screens.slice(0, 8).map(cleanScreen) : [];
    if (screens.includes(null))
        return null;
    return {
        id: desk.id,
        base: isId(desk.base) ? desk.base : desk.id,
        name: isText(desk.name, MOST_NAME, false) && desk.name.trim() ? desk.name : null,
        seen: Number.isSafeInteger(desk.seen) && desk.seen > 0 ? desk.seen : 0,
        builtin: desk.builtin !== false,
        screens,
        thunderbolt: Number.isInteger(desk.thunderbolt) && desk.thunderbolt >= 0 && desk.thunderbolt < 16
            ? desk.thunderbolt : 0,
        set: cleanSet(desk.set),
    };
}

export function clean(data) {
    const result = {version: 1, desks: []};
    const seen = new Set();
    for (const desk of Array.isArray(data?.desks) ? data.desks : []) {
        const good = cleanDesk(desk);
        if (good && !seen.has(good.id)) {
            seen.add(good.id);
            result.desks.push(good);
        }
    }
    // The ones seen last are kept when there are too many
    result.desks.sort((a, b) => b.seen - a.seen);
    result.desks = result.desks.slice(0, MOST_DESKS);
    const declined = Array.isArray(data?.declined) ? data.declined.filter(isId) : [];
    if (declined.length)
        result.declined = [...new Set(declined)].slice(-MOST_DESKS);
    const current = data?.current;
    if (isObject(current) && isId(current.desk))
        result.current = {desk: current.desk, before: cleanSet(current.before), wrote: cleanSet(current.wrote)};
    return result;
}

// The text of a file as a JSON object, or null when it is missing, too large, cannot be read or is not an object
function read(path) {
    try {
        const [, bytes] = GLib.file_get_contents(path);
        if (bytes.length > MOST_BYTES)
            return null;
        const data = JSON.parse(new TextDecoder().decode(bytes));
        return isObject(data) ? data : null;
    } catch {
        return null;
    }
}

export class DesksState {
    constructor() {
        this._written = null;
        this.load();
    }

    // Read again, after the recipe changed the file
    load() {
        const main = read(FILE);
        const copy = main ? null : read(COPY);
        this._data = clean(main ?? copy ?? {});
        // The file the copy stood in for is made again
        if (copy)
            this.save();
    }

    // Whether the file says what this module last wrote: its own change, heard back
    isOwnWrite() {
        try {
            const [, bytes] = GLib.file_get_contents(FILE);
            return new TextDecoder().decode(bytes) === this._written;
        } catch {
            return false;
        }
    }

    // Before a change: the file as it is now, if somebody else wrote it since this module did
    _fresh() {
        if (this._written !== null && !this.isOwnWrite())
            this.load();
    }

    get desks() {
        return this._data.desks;
    }

    get current() {
        return this._data.current ?? null;
    }

    set current(value) {
        this._fresh();
        if (value)
            this._data.current = {desk: value.desk, before: cleanSet(value.before), wrote: cleanSet(value.wrote)};
        else
            delete this._data.current;
        this.save();
    }

    // The desk the computer is at: the one of its screens and Thunderbolt devices, else the only one of its screens
    find(here) {
        const desks = this._data.desks;
        const exact = desks.find(desk => desk.id === here.id);
        if (exact || !here.base)
            return exact ?? null;
        const alike = desks.filter(desk => desk.base === here.base);
        return alike.length === 1 ? alike[0] : null;
    }

    byId(id) {
        return this._data.desks.find(desk => desk.id === id) ?? null;
    }

    declined(id) {
        return this._data.declined?.includes(id) ?? false;
    }

    decline(id) {
        this._fresh();
        this._data.declined = [...(this._data.declined ?? []).filter(d => d !== id), id];
        this._data = clean(this._data);
        this.save();
    }

    put(desk) {
        const good = cleanDesk(desk);
        if (!good)
            return;
        this._fresh();
        // A desk kept is no longer one Not now was said to
        this._data.declined = (this._data.declined ?? []).filter(d => d !== good.id);
        this._data.desks = [good, ...this._data.desks.filter(d => d.id !== good.id)];
        this._data = clean(this._data);
        this.save();
    }

    save() {
        try {
            // Never larger than a read takes: the desks seen longest ago go first
            let text = JSON.stringify(this._data);
            while (new TextEncoder().encode(text).length > MOST_BYTES && this._data.desks.length > 1) {
                this._data.desks.pop();
                text = JSON.stringify(this._data);
            }
            GLib.mkdir_with_parents(DIR, 0o700);
            GLib.file_set_contents_full(FILE, text, GLib.FileSetContentsFlags.CONSISTENT, 0o600);
            GLib.file_set_contents_full(COPY, text, GLib.FileSetContentsFlags.CONSISTENT, 0o600);
            this._written = text;
        } catch (e) {
            console.warn(`Amethystora UI: desks could not write ${FILE}: ${e.message}`);
        }
    }
}
