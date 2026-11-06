// Put this away for tomorrow (R7-34) as data: what the sealed list may hold, and what a refusal of the Vault means to the feature.
// Nothing here touches GNOME, so that the build checks it under node (build_files/shared/test-put-away.mjs); putaway.js does the rest.
//
// A put-away is a saved set (a name, a layout and windows as app IDs, ranks, places and tiles: never a title or a file), so it
// is checked by the same code as the sets' file (setsdata.js clean). The whole list is sealed as one record.

import {clean} from './setsdata.js';

export const MAX_ITEMS = 8;
// The Vault's purpose and record for the list, and the file the sealed bytes go to
export const PURPOSE = 'put-away';
export const RECORD = 'list';
export const FILE_NAME = 'put-away.avs';

// The items as the bytes that are sealed
export function encode(items) {
    return new TextEncoder().encode(JSON.stringify({version: 1, sets: items.slice(0, MAX_ITEMS)}));
}

// The items sealed bytes held, checked piece by piece, newest first; null when it is no list at all
export function decode(bytes) {
    let data;
    try {
        data = JSON.parse(new TextDecoder().decode(bytes));
    } catch {
        return null;
    }
    if (!data || typeof data !== 'object' || data.version !== 1 || !Array.isArray(data.sets))
        return null;
    return clean(data).sets.slice(0, MAX_ITEMS);
}

// `item` added to the list: one of the same name gives way, and past MAX_ITEMS the oldest go
export function add(items, item) {
    const kept = [item, ...items.filter(i => i.name !== item.name)];
    kept.sort((a, b) => b.saved - a.saved);
    return kept.slice(0, MAX_ITEMS);
}

export const without = (items, name) => items.filter(i => i.name !== name);

// What the file held and what this session put away: the session's wins where a name is in both
export function merge(fromFile, inMemory) {
    return inMemory.reduce((list, item) => add(list, item), [...fromFile]);
}

// What the Vault's answer means for the person. `kept` the list is sealed; the others say why it is only in memory, and what
// to do, which is never to prompt: `closed` (set up, closed: Open Vault), `never` (not set up: offered once), `refused`.
export function problemOf(why) {
    switch (why) {
    case 'closed': return 'closed';
    case 'not-set-up': return 'never';
    case 'refused': return 'refused';
    // Too large cannot be: the list is limited well under a megabyte. Anything else is as closed
    default: return 'closed';
    }
}
