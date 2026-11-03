// Screens come back as you left them (R7-46): what desks.json's `layouts` may hold, and which window of those open now
// each kept entry is. Nothing here touches GNOME, so that the build checks it under node
// (build_files/shared/test-desk-layouts.mjs); desks/layouts.js does the rest.
//
//   "layouts": {"<the desk's base>": {"seen": 3, "mode": "tiling" | "classic",
//                                     "shown": {"<a screen>": 1}, "windows": [WINDOW]}}
//   WINDOW: {"app": "org.gnome.Nautilus.desktop", "rank": 0, "workspace": 1, "screen": "0123456789ab", then
//            in the tiling "column", "row", "width", and in the classic layout "x", "y", "w", "h", "max"}
//
// `shown` is the workspace each screen showed, with several screens under the tiling. `seen` is the order of the visits to
// a desk, a counter that is never a time: the desk visited last has the highest, and the lowest go when more than fit.
//
// A launcher's ID, numbers and the hash of a screen: never a window's title, a file or a page. An entry with anything else
// loses it here, and so does a window whose app does not end in `.desktop`. (That is the shell's own test of a launcher;
// a program can name itself so, and nothing is lost by that: a program can write the file itself.)

import {isAppId} from '../workspaces/setsdata.js';

export const MOST_LAYOUTS = 6;
export const MOST_WINDOWS = 48;

const isObject = v => !!v && typeof v === 'object' && !Array.isArray(v);
const isInt = (v, lo, hi) => Number.isInteger(v) && v >= lo && v <= hi;
const isHash = v => typeof v === 'string' && /^[0-9a-f]{12}$/.test(v);

function cleanWindow(w, mode) {
    if (!isObject(w) || !isAppId(w.app) || !isInt(w.rank, 0, MOST_WINDOWS) || !isInt(w.workspace, 0, 63) || !isHash(w.screen))
        return null;
    const window = {app: w.app, rank: w.rank, workspace: w.workspace, screen: w.screen};
    if (mode === 'tiling') {
        if (!isInt(w.column, 0, MOST_WINDOWS) || !isInt(w.row, 0, MOST_WINDOWS) || !isInt(w.width, 50, 16384))
            return null;
        return {...window, column: w.column, row: w.row, width: w.width};
    }
    if (!isInt(w.x, -32768, 32768) || !isInt(w.y, -32768, 32768) || !isInt(w.w, 50, 16384) || !isInt(w.h, 50, 16384) ||
        typeof w.max !== 'boolean')
        return null;
    return {...window, x: w.x, y: w.y, w: w.w, h: w.h, max: w.max};
}

export function cleanLayout(layout) {
    if (!isObject(layout) || !['tiling', 'classic'].includes(layout.mode) || !Array.isArray(layout.windows))
        return null;
    const seen = new Set();
    const windows = layout.windows.slice(0, 4 * MOST_WINDOWS).map(w => cleanWindow(w, layout.mode)).filter(w => {
        const key = w && `${w.app} ${w.rank}`;
        if (!w || seen.has(key))
            return false;
        seen.add(key);
        return true;
    }).slice(0, MOST_WINDOWS);
    if (!windows.length)
        return null;
    const shown = {};
    for (const [screen, workspace] of Object.entries(isObject(layout.shown) ? layout.shown : {}).slice(0, 8)) {
        if (isHash(screen) && isInt(workspace, 0, 63))
            shown[screen] = workspace;
    }
    return {seen: Number.isSafeInteger(layout.seen) && layout.seen > 0 ? layout.seen : 0, mode: layout.mode, shown, windows};
}

// What fits, keyed by the desk's base, the ones seen last when there are too many; {} when nothing does
export function cleanLayouts(layouts) {
    if (!isObject(layouts))
        return {};
    const kept = Object.entries(layouts).filter(([base]) => isHash(base))
        .map(([base, layout]) => [base, cleanLayout(layout)]).filter(([, layout]) => layout)
        .sort(([, a], [, b]) => b.seen - a.seen).slice(0, MOST_LAYOUTS);
    return Object.fromEntries(kept);
}

// The layouts with this desk's layout in as the visit of the moment, or null when nothing would change (the same windows,
// and already the desk visited last). `seen` is a counter of visits: the desk's takes the highest, so that one visited
// often is never the first to go, and nothing on disk says when.
export function visit(layouts, base, layout) {
    const all = cleanLayouts(layouts);
    const old = all[base];
    const top = Math.max(0, ...Object.values(all).map(l => l.seen));
    if (old && old.seen === top && JSON.stringify(old.windows) === JSON.stringify(layout.windows) &&
        JSON.stringify(old.shown) === JSON.stringify(layout.shown))
        return null;
    const seen = old?.seen === top && top > 0 ? top : top + 1;
    return cleanLayouts({...(isObject(layouts) ? layouts : {}), [base]: {...layout, seen}});
}

// Each kept entry with the window open now that it means: `open` is [{window, app, seq}], and a kept entry is the
// window of its app and rank, oldest first. What was closed since is left out, and a window opened since is not moved.
// `seqs`, when this GNOME Shell kept the layout itself, holds each entry's window by its stable sequence ("app rank" to
// seq): a window of an app closed meanwhile then shifts no other window of that app onto its place.
export function match(layout, open, seqs = null) {
    if (seqs) {
        const bySeq = new Map(open.map(item => [item.seq, item]));
        return layout.windows.map(entry => {
            const item = bySeq.get(seqs.get(`${entry.app} ${entry.rank}`));
            return {entry, window: item?.app === entry.app ? item.window : null};
        }).filter(({window}) => window);
    }
    const byApp = new Map();
    for (const item of open)
        byApp.set(item.app, [...byApp.get(item.app) ?? [], item]);
    for (const list of byApp.values())
        list.sort((a, b) => a.seq - b.seq);
    return layout.windows.map(entry => ({entry, window: byApp.get(entry.app)?.[entry.rank]?.window ?? null}))
        .filter(({window}) => window);
}

// Under the tiling: the entries of one workspace on one screen as columns in their order, each window with its place in
// the strip. A column that lost its windows closes up, so that the columns stay next to each other.
export function strips(matched) {
    const byStrip = new Map();
    for (const item of matched) {
        const key = `${item.entry.workspace} ${item.entry.screen}`;
        byStrip.set(key, [...byStrip.get(key) ?? [], item]);
    }
    return [...byStrip.values()].map(items => {
        const columns = [...new Set(items.map(({entry}) => entry.column))].sort((a, b) => a - b);
        return {
            workspace: items[0].entry.workspace,
            screen: items[0].entry.screen,
            windows: items.slice().sort((a, b) => a.entry.column - b.entry.column || a.entry.row - b.entry.row)
                .map(({entry, window}, index, sorted) => ({
                    window,
                    width: entry.width,
                    column: columns.indexOf(entry.column),
                    // The row among those of its column still open
                    row: sorted.slice(0, index).filter(other => other.entry.column === entry.column).length,
                })),
        };
    });
}
