// Put this away for tomorrow (plans/ui-ideas/r7/spec-keep.md, R7-34): a whole workspace put away by name and brought back days later
// with every window in place. The workspace's windows are captured as a saved set is (sets.js capture: app IDs, ranks, places and tiles,
// never a title or a file), the windows are closed the way the close button closes them (an app with unsaved work asks first), and
// the list is sealed under the account's key in Amethystora Vault (purpose `put-away`, record `list`) into
// ~/.local/state/amethystora/put-away.avs. No name or app is ever in a plain file.
//
// The Vault is asked only after a person's own click (Put away, Show, Keep it): at login nothing is read, so a sealed list shows only as
// "Put away" until a click opens it. With the vault never set up or closed, what was put away stays in memory (module scope, which a
// screen lock does not disturb) and the card says so; nothing is ever kept in the clear. A list that is on disk and not read yet is never
// overwritten: sealing waits until it can be read and merged.
//
// Nothing is connected while nothing is put away; the pill's two cards (pill/putaway.js) are the only view of this.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import * as Vault from '../vault.js';
import {MAX_BYTES} from './setsdata.js';
import {FILE_NAME, PURPOSE, RECORD, add, decode, encode, merge, problemOf, without} from './putawaydata.js';

const DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const FILE = GLib.build_filenamev([DIR, FILE_NAME]);
// How long after closing a workspace's windows the card is told which ones are still open, in seconds
const CHECK_AFTER = 3;
const VAULT_APP = 'amethystora-vault.desktop';

// Kept outside the class, since a lock turns the extension off: what is put away (in memory), whether a sealed list is on disk
// that this shell has not read, whether what is in memory is not sealed yet, and why
let items = [];
let onDisk = false;
let unread = false;
let dirty = false;
let problem = null;
// What a person is asked now ({workspace, set, count, name}), and what the last close left open
let pending = null;
let stillOpen = 0;
// What sealing and deleting have under way, one after the other: a write is never cancelled and a delete never overtakes one
let tail = Promise.resolve();

function queued(task) {
    const next = tail.then(task, task);
    tail = next.then(() => {}, () => {});
    return next;
}

function fileSize(path) {
    return new Promise(resolve => {
        Gio.File.new_for_path(path).query_info_async('standard::size', Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_DEFAULT, null,
            (file, result) => {
                try {
                    resolve(file.query_info_finish(result).get_size());
                } catch {
                    resolve(-1);
                }
            });
    });
}

function readFile() {
    return new Promise(resolve => {
        Gio.File.new_for_path(FILE).load_contents_async(null, (file, result) => {
            try {
                resolve(file.load_contents_finish(result)[1]);
            } catch {
                resolve(null);
            }
        });
    });
}

function writeFile(bytes) {
    return new Promise(resolve => {
        const flags = Gio.FileCreateFlags.REPLACE_DESTINATION | Gio.FileCreateFlags.PRIVATE;
        const attempt = again => Gio.File.new_for_path(FILE).replace_contents_bytes_async(
            new GLib.Bytes(bytes), null, false, flags, null, (file, result) => {
                try {
                    file.replace_contents_finish(result);
                    resolve(true);
                } catch (e) {
                    if (again && e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.NOT_FOUND)) {
                        GLib.mkdir_with_parents(DIR, 0o700);
                        attempt(false);
                        return;
                    }
                    console.warn(`Amethystora UI: what is put away could not be kept: ${e.message}`);
                    resolve(false);
                }
            });
        attempt(true);
    });
}

function deleteFile() {
    return new Promise(resolve => {
        Gio.File.new_for_path(FILE).delete_async(GLib.PRIORITY_DEFAULT, null, (file, result) => {
            try {
                file.delete_finish(result);
            } catch {}
            resolve(true);
        });
    });
}

export class PutAway {
    constructor(module) {
        this._module = module;
    }

    enable() {
        this._dead = false;
        this._check = 0;
        this._askTimer = 0;
        this._watched = [];
        // One stat of one file, once: whether a sealed list is there to be offered. It is not read, so no Vault is asked.
        fileSize(FILE).then(size => {
            if (this._dead || size < 0)
                return;
            onDisk = true;
            if (!items.length)
                unread = true;
            this._changed();
        });
    }

    disable() {
        this._dead = true;
        this._clearAsk();
        if (this._check)
            GLib.source_remove(this._check);
        this._check = 0;
        for (const window of this._watched ?? [])
            window.disconnectObject(this);
        this._watched = [];
        pending = null;
    }

    // What the pill's cards draw
    get items() {
        return items;
    }

    get unreadOnDisk() {
        return onDisk && unread;
    }

    get problem() {
        return dirty ? problem : null;
    }

    get pending() {
        return pending;
    }

    get stillOpen() {
        return stillOpen;
    }

    get anything() {
        return items.length > 0 || (onDisk && unread);
    }

    _changed() {
        if (!this._dead)
            this._module._ext.modules?.get('pill')?.putAway?.refresh();
    }

    // The workspace's windows as a set would hold them, to be asked about: {name, count}, or null when there is nothing to put away
    ask(workspace) {
        const sets = this._module.sets;
        const captured = sets?.capture(workspace);
        if (!captured) {
            pending = null;
            return null;
        }
        // The windows the set holds are the ones that close: kept here, since the card may stay up while others come and go on the workspace
        const windows = sets.windowsOf(workspace).slice(-captured.set.windows.length);
        pending = {workspace, set: captured.set, count: captured.set.windows.length, name: captured.set.name, windows};
        if (this._askTimer)
            GLib.source_remove(this._askTimer);
        // A question left unanswered goes by itself after two minutes
        this._askTimer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 120, () => {
            this._askTimer = 0;
            pending = null;
            this._changed();
            return GLib.SOURCE_REMOVE;
        });
        this._changed();
        this._module._ext.modules?.get('pill')?.putAway?.show('putaway');
        return {name: pending.name, count: pending.count};
    }

    cancel() {
        this._clearAsk();
        pending = null;
        this._changed();
    }

    _clearAsk() {
        if (this._askTimer)
            GLib.source_remove(this._askTimer);
        this._askTimer = 0;
    }

    // The person said yes: it is kept in memory at once, its windows are asked to close the way their close button asks, and the list is sealed
    confirm() {
        const ask = pending;
        this._clearAsk();
        pending = null;
        const sets = this._module.sets;
        if (!ask || !sets) {
            this._changed();
            return false;
        }
        items = add(items, ask.set);
        dirty = true;
        // Only the captured windows that are still there
        const windows = ask.windows.filter(w => w.get_workspace() === ask.workspace);
        stillOpen = windows.length;
        const time = global.get_current_time();
        for (const window of windows) {
            window.connectObject('unmanaged', () => {
                stillOpen = Math.max(0, stillOpen - 1);
                window.disconnectObject(this);
                this._watched = this._watched.filter(w => w !== window);
                this._changed();
            }, this);
            this._watched.push(window);
            window.delete(time);
        }
        // Three seconds on, the card says what is still open: the apps that asked first. A window that goes later is counted off
        // by its own signal, which is the only thing held until it does (or the part stops).
        this._check = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, CHECK_AFTER, () => {
            this._check = 0;
            this._changed();
            return GLib.SOURCE_REMOVE;
        });
        this._changed();
        this.keep();
        return true;
    }

    // The list sealed and written, or why not: a promise that always resolves, to whether it is kept
    keep() {
        return queued(() => this._keep());
    }

    async _keep() {
        if (!items.length) {
            if (onDisk && !unread)
                await deleteFile();
            else if (onDisk)
                return false;
            onDisk = false;
            unread = false;
            dirty = false;
            problem = null;
            this._changed();
            return true;
        }
        // A list on disk that nobody has read is never written over: it is read and merged first, or nothing is written
        if (onDisk && unread && !(await this._read()))
            return false;
        const sealed = await Vault.seal(PURPOSE, RECORD, encode(items));
        if (!sealed.ok) {
            problem = problemOf(sealed.why);
            dirty = true;
            this._changed();
            return false;
        }
        if (!await writeFile(sealed.data)) {
            problem = 'closed';
            dirty = true;
            this._changed();
            return false;
        }
        onDisk = true;
        unread = false;
        dirty = false;
        problem = null;
        this._changed();
        return true;
    }

    // The sealed list read into memory (merged with what this session put away); says whether it was. Only from a click.
    _read() {
        return (async () => {
            const size = await fileSize(FILE);
            if (size < 0 || size > MAX_BYTES) {
                // Nothing there, or no file this part wrote
                onDisk = false;
                unread = false;
                return true;
            }
            const bytes = await readFile();
            if (!bytes) {
                problem = 'closed';
                return false;
            }
            const opened = await Vault.open(PURPOSE, RECORD, bytes);
            if (!opened.ok) {
                problem = problemOf(opened.why);
                dirty = dirty || items.length > 0;
                return false;
            }
            const list = decode(opened.data);
            if (!list) {
                problem = 'refused';
                return false;
            }
            items = merge(list, items);
            unread = false;
            if (!dirty)
                problem = null;
            return true;
        })();
    }

    // Show: the sealed list read after a click; the pill's cards redraw
    load() {
        return queued(async () => {
            if (onDisk && unread)
                await this._read();
            else if (dirty)
                await this._keep();
            this._changed();
            return !unread;
        });
    }

    // One put-away brought back: its apps open as a saved set's do, and it leaves the list
    bringBack(name) {
        const item = items.find(i => i.name === name);
        if (!item)
            return false;
        items = without(items, name);
        this._module.sets?.openGiven(item, false);
        dirty = true;
        this._changed();
        this.keep();
        return true;
    }

    forget(name) {
        items = without(items, name);
        dirty = true;
        this._changed();
        this.keep();
    }

    // Open Vault, after a click: the app, never a prompt of its own
    openVault() {
        try {
            Gio.DesktopAppInfo.new(VAULT_APP)?.launch([], null);
        } catch (e) {
            console.warn(`Amethystora UI: the Vault could not be opened: ${e.message}`);
        }
    }

    // For the suites: the state without a window's title or a name that is not put away
    snapshot() {
        return {items: items.map(i => i.name), onDisk, unread, dirty, problem, pending: !!pending, stillOpen};
    }

    // For the suites: forget everything in memory, as a new GNOME Shell session would
    resetForTest() {
        items = [];
        onDisk = false;
        unread = false;
        dirty = false;
        problem = null;
        pending = null;
        stillOpen = 0;
    }
}
