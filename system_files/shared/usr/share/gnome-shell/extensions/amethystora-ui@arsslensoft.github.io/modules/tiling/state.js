// What the tiling keeps across log-outs that is the person's doing and not a setting: where new windows open,
// the workspaces whose top bar they hid, and what it took out of GNOME's settings while it runs, so that those
// are put back even if GNOME Shell stops in the middle. One small file, written only when one of them changes, with
// a copy beside it for the day the file is lost or cannot be read: nothing else knows what was taken out.

import GLib from 'gi://GLib';

const DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const FILE = GLib.build_filenamev([DIR, 'tiling.json']);
const COPY = `${FILE}.bak`;

const isText = value => typeof value === 'string';
const isIndex = value => Number.isInteger(value) && value >= 0;
const isObject = value => !!value && typeof value === 'object' && !Array.isArray(value);

// The shapes the records have when the tiling wrote them; a file made by hand, or by another version, may hold
// anything, and what does not fit is left out rather than trusted
const CHECKS = {
    taken: record => isObject(record) && isText(record.schema) && isText(record.key) &&
        Array.isArray(record.removed) && Array.isArray(record.wrote) && record.wrote.every(isText) &&
        record.removed.every(r => isObject(r) && isText(r.accelerator) && isIndex(r.index)),
    forced: record => isObject(record) && isText(record.schema) && isText(record.key) &&
        typeof record.was === 'boolean' && typeof record.wrote === 'boolean',
};

function clean(data) {
    const result = {...data};
    for (const [key, check] of Object.entries(CHECKS)) {
        const kept = Array.isArray(result[key]) ? result[key].filter(check) : [];
        if (kept.length)
            result[key] = kept;
        else
            delete result[key];
    }
    if ('hiddenTopBar' in result && !(Array.isArray(result.hiddenTopBar) && result.hiddenTopBar.every(isIndex)))
        delete result.hiddenTopBar;
    return result;
}

// A file's JSON object, or null when it is missing, cannot be read or is not an object
function read(path) {
    try {
        const [, bytes] = GLib.file_get_contents(path);
        const data = JSON.parse(new TextDecoder().decode(bytes));
        return isObject(data) ? data : null;
    } catch {
        return null;
    }
}

export class State {
    constructor() {
        const main = read(FILE);
        const copy = main ? null : read(COPY);
        this._data = clean(main ?? copy ?? {});
        // The file the copy stood in for is made again, for whatever reads it besides the tiling
        if (copy)
            this._save();
    }

    get(key) {
        return this._data[key];
    }

    set(key, value) {
        if (JSON.stringify(this._data[key]) === JSON.stringify(value))
            return;
        if (value === undefined || (Array.isArray(value) && !value.length))
            delete this._data[key];
        else
            this._data[key] = value;
        this._save();
    }

    _save() {
        try {
            const text = JSON.stringify(this._data);
            GLib.mkdir_with_parents(DIR, 0o700);
            GLib.file_set_contents(FILE, text);
            GLib.file_set_contents(COPY, text);
        } catch (e) {
            console.warn(`Amethystora UI: the tiling could not write ${FILE}: ${e.message}`);
        }
    }
}
