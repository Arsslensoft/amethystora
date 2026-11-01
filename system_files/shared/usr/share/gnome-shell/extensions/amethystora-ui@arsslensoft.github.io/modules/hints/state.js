// What the tips remember for the account: when the module first ran (no tip in the first hour after that), when the
// last tip came (none within half an hour of it), and which tips have been shown, each once ever. One small file,
// written only when a tip shows (a few times in an account's life), with a copy beside it for the day the file is lost
// or cannot be read. `ame desktop hints reset` empties what was shown, and the module reads the file again when it
// changes.

import GLib from 'gi://GLib';

const DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
export const FILE = GLib.build_filenamev([DIR, 'hints.json']);
const COPY = `${FILE}.bak`;

const isTime = value => Number.isInteger(value) && value > 0;
const isObject = value => !!value && typeof value === 'object' && !Array.isArray(value);

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

export class HintState {
    // known: the ids a tip may have; anything else in the file is left out
    constructor(known) {
        this._known = known;
        this.load();
    }

    load() {
        const main = read(FILE);
        const copy = main ? null : read(COPY);
        const data = main ?? copy ?? {};
        this.since = isTime(data.since) ? data.since : 0;
        this.last = isTime(data.last) ? data.last : 0;
        this.shown = {};
        if (isObject(data.shown)) {
            for (const [id, at] of Object.entries(data.shown)) {
                if (this._known.includes(id) && isTime(at))
                    this.shown[id] = at;
            }
        }
        // The file the copy stood in for is made again
        if (copy)
            this._save();
    }

    // The first time the module runs for the account
    begin(now) {
        if (this.since)
            return;
        this.since = now;
        this._save();
    }

    record(id, now) {
        this.shown[id] = now;
        this.last = now;
        this._save();
    }

    _save() {
        try {
            const text = JSON.stringify({version: 1, since: this.since, last: this.last, shown: this.shown});
            GLib.mkdir_with_parents(DIR, 0o700);
            // Each written whole under another name and moved into place
            GLib.file_set_contents(FILE, text);
            GLib.file_set_contents(COPY, text);
        } catch (e) {
            console.warn(`Amethystora UI: the tips could not write ${FILE}: ${e.message}`);
        }
    }
}
