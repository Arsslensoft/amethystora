// Tray items that never told this tray about themselves: a program that registered with the tray before (before a
// lock or a restart of the shell) and does not register again. Every program on the session bus is asked which
// objects it has, from its root down, and every object that is a tray item is reported. The work is all calls to
// other programs, a few at a time, and the shell waits on none of them.
//
// A program decides how big its tree is, so the walk has ceilings: a few levels down, a few hundred objects for each
// program, and a time for the whole scan. A program is walked once: the shell leaves the extension at every lock and
// scans again at every unlock, and what was found about a program still on the bus is kept for that.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {ITEM_IFACE, busCall, call, isCancelled, nameOwner} from './dbus.js';

// Programs asked at the same time, and how long one may take to answer
const PARALLEL = 4;
const TIMEOUT = 5000;
// Levels below a program's root that are walked, objects asked of one program, the most an answer may be (the
// shell reads it on its own thread), and milliseconds the whole scan may last, before the last answers
const MAX_DEPTH = 6;
const MAX_OBJECTS = 200;
const MAX_ANSWER = 256 * 1024;
const BUDGET = 20000;

// The item objects found in each program by an earlier scan, by the program's unique name, which the bus never
// gives to another program. It lives as long as the shell does, not as long as the tray.
const walked = new Map();

// Runs `work` on every value, `PARALLEL` at a time
async function eachAtMost(values, work) {
    const queue = [...values];
    const worker = async () => {
        while (queue.length) {
            // eslint-disable-next-line no-await-in-loop
            await work(queue.shift());
        }
    };
    await Promise.all(Array.from({length: Math.min(PARALLEL, queue.length)}, worker));
}

// Looks at one object and, below it, at what it lists. `state` is {found, objects, deadline, cut}: the items so far,
// the asks left for this program, when the scan's time is over and whether it was.
async function walk(owner, path, depth, state, cancellable) {
    if (state.objects <= 0)
        return;
    if (GLib.get_monotonic_time() > state.deadline) {
        state.cut = true;
        return;
    }
    state.objects--;
    let info;
    try {
        const reply = await call(owner, path, 'org.freedesktop.DBus.Introspectable', 'Introspect', null, '(s)',
            cancellable, TIMEOUT);
        const xml = reply.get_child_value(0).get_string()[0];
        if (xml.length > MAX_ANSWER)
            return;
        info = Gio.DBusNodeInfo.new_for_xml(xml);
    } catch (e) {
        if (isCancelled(e))
            throw e;
        return;
    }
    if (info.lookup_interface(ITEM_IFACE))
        state.found.push(path);
    if (depth >= MAX_DEPTH)
        return;
    for (const child of info.nodes ?? []) {
        if (state.objects <= 0)
            break;
        const name = child.path;
        if (!name || name.includes('/'))
            continue;
        // eslint-disable-next-line no-await-in-loop
        await walk(owner, path === '/' ? `/${name}` : `${path}/${name}`, depth + 1, state, cancellable);
    }
}

/**
 * Every tray item on the session bus, by the program that has it.
 *
 * @param {Gio.Cancellable} cancellable - stops the scan
 * @returns {Promise<Array<{owner: string, path: string, names: string[]}>>} each item, its program's unique name
 *   and its program's well-known names
 */
export async function findItems(cancellable) {
    const deadline = GLib.get_monotonic_time() + BUDGET * 1000;
    const own = Gio.DBus.session.get_unique_name();
    const reply = await busCall('ListNames', null, '(as)', cancellable);
    const names = reply.get_child_value(0).get_strv();

    // Each program by its unique name, with the well-known names it holds
    const programs = new Map();
    for (const name of names) {
        if (name.startsWith(':') && name !== own)
            programs.set(name, []);
    }
    await eachAtMost(names.filter(name => !name.startsWith(':') && name !== 'org.freedesktop.DBus'), async name => {
        try {
            programs.get(await nameOwner(name, cancellable))?.push(name);
        } catch (e) {
            if (isCancelled(e))
                throw e;
        }
    });
    // A program that left the bus is not kept
    for (const owner of [...walked.keys()]) {
        if (!programs.has(owner))
            walked.delete(owner);
    }

    const items = [];
    await eachAtMost([...programs.keys()], async owner => {
        let paths = walked.get(owner);
        if (!paths) {
            const state = {found: [], objects: MAX_OBJECTS, deadline, cut: false};
            await walk(owner, '/', 0, state, cancellable);
            paths = state.found;
            // A walk the time ran out on is made again next time
            if (!state.cut)
                walked.set(owner, paths);
        }
        for (const path of paths)
            items.push({owner, path, names: programs.get(owner)});
    });
    return items;
}

/**
 * What to register a found item as: a /StatusNotifierItem object by its program's name (one starting
 * org.kde.StatusNotifierItem first), any other by the name and its path.
 *
 * @param {{owner: string, path: string, names: string[]}} found - an item findItems reported
 * @returns {string}
 */
export function serviceFor({owner, path, names}) {
    const name = names.find(n => n.startsWith('org.kde.StatusNotifierItem')) ?? names[0];
    if (path === '/StatusNotifierItem' && name)
        return name;
    return `${name ?? owner}${path}`;
}
