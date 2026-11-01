// What the pointer module keeps across log-outs: the pointer size it wrote for a dense screen, so that it only ever
// takes back a size it set itself and never one the person chose. One small file, written only when that changes,
// with a copy beside it for the day the file is lost or cannot be read; a file that holds anything else counts as
// empty.

import GLib from 'gi://GLib';

const DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const FILE = GLib.build_filenamev([DIR, 'pointer.json']);
const COPY = `${FILE}.bak`;

const isSize = value => Number.isInteger(value) && value > 0 && value <= 256;

// A file's JSON object, or null when it is missing, cannot be read or is not an object
function read(path) {
    try {
        const [, bytes] = GLib.file_get_contents(path);
        const data = JSON.parse(new TextDecoder().decode(bytes));
        return data && typeof data === 'object' && !Array.isArray(data) ? data : null;
    } catch {
        return null;
    }
}

export class State {
    constructor() {
        const main = read(FILE);
        const copy = main ? null : read(COPY);
        const data = main ?? copy ?? {};
        this._wrote = isSize(data.wrote) ? data.wrote : null;
        if (copy)
            this._save();
    }

    // The size the module wrote, or null
    get wrote() {
        return this._wrote;
    }

    set wrote(value) {
        value = isSize(value) ? value : null;
        if (value === this._wrote)
            return;
        this._wrote = value;
        this._save();
    }

    _save() {
        try {
            const text = JSON.stringify(this._wrote === null ? {} : {wrote: this._wrote});
            GLib.mkdir_with_parents(DIR, 0o700);
            GLib.file_set_contents(FILE, text);
            GLib.file_set_contents(COPY, text);
        } catch (e) {
            console.warn(`Amethystora UI: the pointer could not write ${FILE}: ${e.message}`);
        }
    }
}
