// What a profile wrote outside the shell's memory and must put back even if GNOME Shell stops in the middle: Night
// Light's two keys as they were before Sleep warmed them, and whether this module paused Night Light. One small file,
// written whole only when it changes, with a copy beside it for the day it is lost or cannot be read. Nothing in it is
// ever run, shown or used as a path; what does not have the shape below is left out.
//
//   {"version": 1, "night": {"before": {"enabled": B, "temperature": U}, "wrote": {"enabled": B, "temperature": U}},
//    "paused": true}

import GLib from 'gi://GLib';

const DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const FILE = GLib.build_filenamev([DIR, 'focus.json']);
const COPY = `${FILE}.bak`;

const isObject = value => !!value && typeof value === 'object' && !Array.isArray(value);
const isTemperature = value => Number.isInteger(value) && value >= 1000 && value <= 10000;
const isNightValues = value => isObject(value) && typeof value.enabled === 'boolean' && isTemperature(value.temperature);

function clean(data) {
    const result = {version: 1};
    if (isObject(data?.night) && isNightValues(data.night.before) && isNightValues(data.night.wrote)) {
        result.night = {
            before: {enabled: data.night.before.enabled, temperature: data.night.before.temperature},
            wrote: {enabled: data.night.wrote.enabled, temperature: data.night.wrote.temperature},
        };
    }
    if (data?.paused === true)
        result.paused = true;
    return result;
}

// A file's JSON object, or null when it is missing, cannot be read or is not an object
function read(path) {
    try {
        const [, bytes] = GLib.file_get_contents(path);
        if (bytes.length > 4096)
            return null;
        const data = JSON.parse(new TextDecoder().decode(bytes));
        return isObject(data) ? data : null;
    } catch {
        return null;
    }
}

export class FocusState {
    constructor() {
        const main = read(FILE);
        const copy = main ? null : read(COPY);
        this._data = clean(main ?? copy ?? {});
        // The file the copy stood in for is made again
        if (copy)
            this._save();
    }

    get(key) {
        return this._data[key];
    }

    set(key, value) {
        if (JSON.stringify(this._data[key]) === JSON.stringify(value))
            return;
        if (value === undefined)
            delete this._data[key];
        else
            this._data[key] = value;
        this._data = clean(this._data);
        this._save();
    }

    _save() {
        try {
            const text = JSON.stringify(this._data);
            GLib.mkdir_with_parents(DIR, 0o700);
            GLib.file_set_contents(FILE, text);
            GLib.file_set_contents(COPY, text);
        } catch (e) {
            console.warn(`Amethystora UI: focus could not write ${FILE}: ${e.message}`);
        }
    }
}
