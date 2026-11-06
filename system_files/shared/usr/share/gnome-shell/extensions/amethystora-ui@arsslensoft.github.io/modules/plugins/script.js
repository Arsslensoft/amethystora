// Any script can go live (R7-03): `ame pill start "Backup" --progress 0.1`, `update`, `end`. The helper (/usr/libexec/amethystora-pill)
// writes one small file for each pill in $XDG_RUNTIME_DIR/amethystora/pill and takes it away when the script ends, even if the script
// forgets (a waiter on the script's process). This only watches that folder: no timer, no polling, nothing at all while it is empty.
// The account's own scripts, so nothing is sandboxed and nothing is asked for; they get the plugins' rules all the same: text only, no
// system plate, at most MAX_SCRIPTS at once, and a script that updates more than MAX_SCRIPT_UPDATES times a minute has its pill ended.
// A file that is no plain file (a FIFO would hang the shell), or is larger than 4 KiB, is never read.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {MAX_SCRIPTS, MAX_SCRIPT_UPDATES, script} from './clamp.js';

const SETTLE_MS = 120;
const MOST_BYTES = 4096;
// A pill nobody ended is ended after this: nothing stays in the pill for good
const MOST_AGE = 12 * 3600;

export function pillFolder() {
    return GLib.build_filenamev([GLib.get_user_runtime_dir(), 'amethystora', 'pill']);
}

export class Scripts {
    constructor(changed, told) {
        this._changed = changed;
        this._told = told;
        this._pills = new Map();
        this._updates = new Map();
        this._serial = 0;
    }

    start() {
        this._folder = Gio.File.new_for_path(pillFolder());
        try {
            GLib.mkdir_with_parents(pillFolder(), 0o700);
            this._monitor = this._folder.monitor_directory(Gio.FileMonitorFlags.NONE, null);
            this._monitor.connect('changed', () => this._queue());
        } catch (e) {
            console.warn(`Amethystora UI: script pills cannot be watched: ${e.message}`);
            return;
        }
        this._cancellable = new Gio.Cancellable();
        this._queue();
    }

    stop() {
        this._serial++;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._monitor?.cancel();
        this._monitor = null;
        if (this._settle)
            GLib.source_remove(this._settle);
        this._settle = 0;
        this._pills.clear();
        this._updates.clear();
    }

    get pills() {
        return [...this._pills.values()];
    }

    _queue() {
        if (this._settle)
            return;
        this._settle = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE_MS, () => {
            this._settle = 0;
            this._rescan();
            return GLib.SOURCE_REMOVE;
        });
    }

    _rescan() {
        const serial = ++this._serial;
        const cancellable = this._cancellable;
        if (!cancellable)
            return;
        this._folder.enumerate_children_async('standard::name,standard::type,standard::size', Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS,
            GLib.PRIORITY_DEFAULT, cancellable, (folder, result) => {
                let children;
                try {
                    children = folder.enumerate_children_finish(result);
                } catch {
                    return;
                }
                children.next_files_async(64, GLib.PRIORITY_DEFAULT, cancellable, (e, more) => {
                    let infos;
                    try {
                        infos = e.next_files_finish(more);
                    } catch {
                        return;
                    }
                    if (serial !== this._serial)
                        return;
                    const files = infos.filter(i => i.get_file_type() === Gio.FileType.REGULAR && i.get_size() <= MOST_BYTES &&
                        /^[a-z0-9][a-z0-9-]{0,60}\.json$/.test(i.get_name())).slice(0, MAX_SCRIPTS * 2);
                    const names = new Set(files.map(i => i.get_name()));
                    let changed = false;
                    for (const name of [...this._pills.keys()]) {
                        if (!names.has(name)) {
                            this._pills.delete(name);
                            changed = true;
                        }
                    }
                    let waiting = files.length;
                    if (waiting === 0 && changed)
                        this._changed();
                    for (const info of files) {
                        this._read(info.get_name(), cancellable, serial, did => {
                            changed ||= did;
                            if (--waiting === 0 && changed && serial === this._serial)
                                this._changed();
                        });
                    }
                });
            });
    }

    // Opened once, without following a link and without waiting for a writer (a FIFO put there reads as empty), and read through that one
    // descriptor, at most MOST_BYTES + 1: what was checked when the folder was listed is not what is trusted, only what is opened here
    _read(name, cancellable, serial, done) {
        const path = GLib.build_filenamev([pillFolder(), name]);
        // O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC
        const fd = GLib.open(path, 0o400000 | 0o4000 | 0o2000000, 0);
        if (fd < 0) {
            this._finish(name, null, null, serial, done);
            return;
        }
        const stream = Gio.UnixInputStream.new(fd, true);
        stream.read_bytes_async(MOST_BYTES + 1, GLib.PRIORITY_DEFAULT, cancellable, (s, result) => {
            let pill = null;
            try {
                const bytes = s.read_bytes_finish(result);
                if (bytes.get_size() <= MOST_BYTES)
                    pill = script({...JSON.parse(new TextDecoder().decode(bytes.toArray())), file: name});
            } catch {}
            try {
                s.close(null);
            } catch {}
            this._finish(name, pill, path, serial, done);
        });
    }

    _finish(name, pill, path, serial, done) {
        if (serial !== this._serial)
            return;
        const file = Gio.File.new_for_path(path ?? GLib.build_filenamev([pillFolder(), name]));
        const now = Date.now() / 1000;
        if (!pill || (pill.started && now - pill.started > MOST_AGE)) {
            this._remove(name, file);
            done(this._pills.delete(name));
            return;
        }
        const before = JSON.stringify(this._pills.get(name) ?? null);
        if (before === JSON.stringify(pill)) {
            done(false);
            return;
        }
        // A flood: more than a person could read, so it is not a pill any more. The folder is read at most about eight times a second, so the limit
        // is a count of changes well under that. Only a change counts: every file is read at each event
        const seen = this._updates.get(name) ?? [];
        seen.push(now);
        while (seen.length && now - seen[0] > 60)
            seen.shift();
        this._updates.set(name, seen);
        if (seen.length > MAX_SCRIPT_UPDATES) {
            this._remove(name, file);
            this._told?.(pill.name);
            done(this._pills.delete(name));
            return;
        }
        if (!this._pills.has(name) && this._pills.size >= MAX_SCRIPTS) {
            done(false);
            return;
        }
        this._pills.set(name, pill);
        done(true);
    }

    _remove(name, file) {
        file.delete_async(GLib.PRIORITY_DEFAULT, null, (f, result) => {
            try {
                f.delete_finish(result);
            } catch {}
        });
    }
}
