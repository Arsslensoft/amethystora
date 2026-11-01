// Which app a process belongs to, and how much of the processor each app used between two looks. A process's app is
// the systemd scope or service the session started it in (GNOME starts every app in one of its own, Flatpak too):
// /proc/PID/cgroup names it, and the app system knows the app by that id. A process outside any app (a system
// service, the shell itself, a program started from a terminal) keeps the name the meters show it by.
//
// The processes are walked with the meters' own reader (meters/sources.js), off the frame, a few at a time.

import Gio from 'gi://Gio';
import Shell from 'gi://Shell';

import * as Sources from '../meters/sources.js';

// app-gnome-org.gnome.TextEditor-1234.scope, app-flatpak-org.mozilla.firefox-1234.scope,
// app-gnome-org.gnome.Nautilus@12.service, app-org.gnome.Nautilus@12.service
const UNIT = /^app-(?:gnome-|flatpak-)?(.+?)(?:-\d+\.scope|@[^/]*\.service|\.service)$/;

const decoder = new TextDecoder();

// systemd writes a character it cannot keep in a unit's name as \xNN ('-' becomes \x2d)
function unescape(text) {
    return text.replace(/\\x([0-9a-fA-F]{2})/g, (m, hex) => String.fromCharCode(parseInt(hex, 16)));
}

// The app id a /proc/PID/cgroup names, or null
export function appIdOf(cgroup) {
    const line = (cgroup ?? '').split('\n').find(l => l.startsWith('0::')) ?? '';
    for (const part of line.slice(3).split('/').reverse()) {
        const match = UNIT.exec(part);
        if (match)
            return unescape(match[1]);
    }
    return null;
}

function readLater(path, cancellable) {
    return new Promise(resolve => {
        Gio.File.new_for_path(path).load_contents_async(cancellable, (file, result) => {
            try {
                const [, bytes] = file.load_contents_finish(result);
                resolve(decoder.decode(bytes));
            } catch {
                resolve(null);
            }
        });
    });
}

// The app system's app for an id, or null
export function lookup(id) {
    if (!id)
        return null;
    return Shell.AppSystem.get_default().lookup_app(id.endsWith('.desktop') ? id : `${id}.desktop`);
}

// Walks the processes: each process (by its key, pid and start) with its ticks so far, its name and its app's id
export class Walker {
    constructor() {
        this._processes = new Sources.Processes();
        this._ids = new Map();
    }

    async walk(cancellable) {
        const found = await this._processes.walk(cancellable, true);
        const missing = [...found.keys()].filter(key => !this._ids.has(key));
        for (let i = 0; i < missing.length; i += 16) {
            if (cancellable.is_cancelled())
                break;
            // eslint-disable-next-line no-await-in-loop
            await Promise.all(missing.slice(i, i + 16).map(async key => {
                const text = await readLater(`/proc/${key.split(':')[0]}/cgroup`, cancellable);
                this._ids.set(key, appIdOf(text));
            }));
        }
        for (const key of this._ids.keys()) {
            if (!found.has(key))
                this._ids.delete(key);
        }
        const result = new Map();
        for (const [key, process] of found)
            result.set(key, {ticks: process.ticks, name: process.name, id: this._ids.get(key) ?? null});
        return result;
    }

    clear() {
        this._processes.clear();
        this._ids.clear();
    }
}

// What each app used between two walks, most first: [{id, app, name, ticks, top}] (top: its busiest process's name) and
// the ticks every process used. A
// process that started since counts from nothing; one that ended is not seen.
export function usedBetween(before, after) {
    const apps = new Map();
    let total = 0;
    for (const [key, now] of after) {
        const used = Math.max(now.ticks - (before.get(key)?.ticks ?? 0), 0);
        if (used === 0)
            continue;
        total += used;
        const app = lookup(now.id);
        const group = app ? `app:${app.get_id()}` : `name:${now.name}`;
        if (!apps.has(group))
            apps.set(group, {id: app?.get_id() ?? null, app, name: app?.get_name() ?? now.name, ticks: 0, top: null, most: 0});
        const entry = apps.get(group);
        entry.ticks += used;
        if (used > entry.most) {
            entry.most = used;
            entry.top = now.name;
        }
    }
    return {list: [...apps.values()].sort((a, b) => b.ticks - a.ticks), total};
}
