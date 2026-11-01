// The emoji picker's recent picks: the 24 last, newest first, kept on this computer only in
// ~/.local/state/amethystora/emoji.json, with a copy beside it for the day the file is lost or cannot be read. A file
// that is missing or not as written is an empty list, never an error in the shell.

import GLib from 'gi://GLib';

const DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const FILE = GLib.build_filenamev([DIR, 'emoji.json']);
const COPY = `${FILE}.bak`;
export const KEPT = 24;
// The most a pick can be: one emoji, a flag, a family with its joiners
const LONGEST = 40;

function read(path) {
    try {
        const [, bytes] = GLib.file_get_contents(path);
        const data = JSON.parse(new TextDecoder().decode(bytes));
        return data && Array.isArray(data.recent) ? data.recent : null;
    } catch {
        return null;
    }
}

const clean = list => [...new Set(list.filter(item => typeof item === 'string' && item.length > 0 &&
    item.length <= LONGEST))].slice(0, KEPT);

// The picks, newest first
export function readRecent() {
    return clean(read(FILE) ?? read(COPY) ?? []);
}

// A pick goes first; the file and its copy are written whole, each by a rename
export function addRecent(pick) {
    const list = clean([pick, ...readRecent()]);
    try {
        const text = JSON.stringify({version: 1, recent: list});
        GLib.mkdir_with_parents(DIR, 0o700);
        GLib.file_set_contents(FILE, text);
        GLib.file_set_contents(COPY, text);
    } catch (e) {
        console.warn(`Amethystora UI: the emoji picker could not write ${FILE}: ${e.message}`);
    }
    return list;
}
