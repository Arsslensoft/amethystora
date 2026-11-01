// Amethystora Clipboard: where the history is kept, sealed. Written for Amethystora; not part of Clipboard
// Indicator, whose registry.js wrote the history as plain JSON, and every copied picture as a plain file, to
// ~/.cache, where the hourly snapshots of the home folder took them too.
//
// The history and each picture are a file of their own in ~/.local/state/amethystora/clipboard (0700), each
// sealed with AES-256-GCM by /usr/libexec/amethystora-clipboard-seal under the key keyring.js keeps. GJS has
// no cipher, and the helper takes the key and the data on its stdin only. Every write goes to a file beside
// and is moved over the old one, in the order it was asked for, also across a disable and an enable (the
// screen locking), whose read waits for the writes before it.
//
// Only GLib and Gio here, so that 20-tests.sh can try it with gjs outside the shell.

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';

export const HELPER = '/usr/libexec/amethystora-clipboard-seal';
const HISTORY = 'history';
const IMAGE_ID = /^[0-9a-f]{32}$/;
// What Clipboard Indicator left, and where the user installs extensions of their own
const OLD_UUID = 'clipboard-indicator@tudmotu.com';

// The writes of every instance, one after another
let queue = Promise.resolve();
function enqueue (task) {
    const run = queue.then(task);
    queue = run.catch(e => console.warn(`Amethystora Clipboard: ${e.message}`));
    return run;
}

// A history waiting in the queue to be written, which a newer one replaces rather than queues behind: every copy
// asks for two or three, and a burst of copies would otherwise keep the queue writing for minutes
let waiting = null;

function run (argv, input) {
    return new Promise((resolve, reject) => {
        let proc;
        try {
            // A pipe for stdin only with something to write to it: GLib aborts the process, which here is
            // the whole session, on a piped stdin communicate() is given nothing for
            proc = Gio.Subprocess.new(argv, Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE |
                (input ? Gio.SubprocessFlags.STDIN_PIPE : Gio.SubprocessFlags.STDIN_INHERIT));
        } catch (e) {
            reject(e);
            return;
        }
        proc.communicate_async(input ? new GLib.Bytes(input) : null, null, (source, result) => {
            try {
                const [, stdout] = source.communicate_finish(result);
                if (source.get_successful())
                    // A copy (get_data): toArray() is a view of the GLib.Bytes' own memory, and what is
                    // returned here is wrapped in a GLib.Bytes again to be written
                    resolve(stdout?.get_data() ?? new Uint8Array());
                else
                    reject(new Error(`${argv[1]} ${argv[2] ?? ''} failed`));
            } catch (e) {
                reject(e);
            }
        });
    });
}

function withKey (key, data) {
    const head = new TextEncoder().encode(`${key}\n`);
    const input = new Uint8Array(head.length + data.length);
    input.set(head);
    input.set(data, head.length);
    return input;
}

function load (file) {
    return new Promise((resolve, reject) => file.load_contents_async(null, (source, result) => {
        try {
            resolve(source.load_contents_finish(result)[1]);
        } catch (e) {
            reject(e);
        }
    }));
}

function replace (file, bytes) {
    return new Promise((resolve, reject) => file.replace_contents_bytes_async(new GLib.Bytes(bytes), null, false,
        Gio.FileCreateFlags.PRIVATE | Gio.FileCreateFlags.REPLACE_DESTINATION, null, (source, result) => {
            try {
                resolve(source.replace_contents_finish(result));
            } catch (e) {
                reject(e);
            }
        }));
}

function remove (file) {
    try {
        file.delete(null);
    } catch (e) {
        if (!e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.NOT_FOUND))
            throw e;
    }
}

function names (dir) {
    const found = [];
    try {
        const children = dir.enumerate_children('standard::name', Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, null);
        let info;
        while ((info = children.next_file(null)))
            found.push(info.get_name());
    } catch (e) {
        if (!e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.NOT_FOUND))
            throw e;
    }
    return found;
}

// Which start of the computer and which session wrote the history: keep-history drops what is not pinned
// when either has ended since
export function stamp () {
    let boot = '';
    try {
        boot = new TextDecoder().decode(GLib.file_get_contents('/proc/sys/kernel/random/boot_id')[1]).trim();
    } catch (e) {}
    return { boot, shell: new Gio.Credentials().get_unix_pid() };
}

// The history's entries that keep-history (always, restart or session) keeps, now
export function survivors (history, keep, now = stamp()) {
    const entries = Array.isArray(history?.entries) ? history.entries : [];
    const restarted = history?.boot !== now.boot;
    const ended = keep === 'restart' ? restarted : keep === 'session' ? restarted || history?.shell !== now.shell : false;
    return ended ? entries.filter(entry => entry.favorite) : entries;
}

export class Store {
    constructor ({ dir = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora', 'clipboard']), helper = HELPER } = {}) {
        this.dir = Gio.File.new_for_path(dir);
        this.helper = helper;
        // closed until open(); then open, locked (the keyring is) or none (there is no keyring)
        this.state = 'closed';
        this.key = null;
    }

    static newId () {
        return GLib.uuid_string_random().replaceAll('-', '');
    }

    // find is keyring.js's, or a stand-in's: (newKey) => { state, key }
    async open (find) {
        const found = await find(async () => new TextDecoder().decode(await run([this.helper, 'new-key'])).trim());
        this.state = found.state;
        this.key = found.state === 'open' ? found.key : null;
    }

    #file (name) {
        return this.dir.get_child(name);
    }

    async #seal (label, data) {
        return run([this.helper, 'seal', label], withKey(this.key, data));
    }

    async #open (label, sealed) {
        return run([this.helper, 'open', label], withKey(this.key, sealed));
    }

    async #write (name, label, data) {
        if (this.state !== 'open')
            throw new Error('the history is not open');
        GLib.mkdir_with_parents(this.dir.get_path(), 0o700);
        await replace(this.#file(name), await this.#seal(label, data));
    }

    // { boot, shell, entries } as written last; null when there is none, it is larger than maxBytes (moved
    // to history~), or it does not open with this key (moved to history.unreadable, never deleted)
    async readHistory (maxBytes = Infinity) {
        await queue;
        if (this.state !== 'open')
            return null;
        const file = this.#file(HISTORY);
        let sealed;
        try {
            sealed = await load(file);
        } catch (e) {
            if (e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.NOT_FOUND))
                return null;
            throw e;
        }
        if (sealed.length > maxBytes) {
            file.move(this.#file(`${HISTORY}~`), Gio.FileCopyFlags.OVERWRITE, null, null);
            return null;
        }
        try {
            const history = JSON.parse(new TextDecoder().decode(await this.#open('history', sealed)));
            return history && typeof history === 'object' ? history : null;
        } catch (e) {
            console.warn(`Amethystora Clipboard: the history does not open with this keyring's key, kept as ${HISTORY}.unreadable`);
            file.move(this.#file(`${HISTORY}.unreadable`), Gio.FileCopyFlags.OVERWRITE, null, null);
            return null;
        }
    }

    writeHistory (entries) {
        if (waiting) {
            waiting.store = this;
            waiting.entries = entries;
            return waiting.done;
        }
        waiting = { store: this, entries };
        waiting.done = enqueue(() => {
            const { store, entries: latest } = waiting;
            waiting = null;
            return store.#write(HISTORY, 'history',
                new TextEncoder().encode(JSON.stringify({ version: 1, ...stamp(), entries: latest })));
        });
        return waiting.done;
    }

    sealImage (id, bytes) {
        return enqueue(() => this.#write(`image-${id}`, 'image', bytes));
    }

    async openImage (id) {
        if (this.state !== 'open' || !IMAGE_ID.test(id))
            return null;
        try {
            return await this.#open('image', await load(this.#file(`image-${id}`)));
        } catch (e) {
            return null;
        }
    }

    deleteImage (id) {
        return enqueue(() => IMAGE_ID.test(id) && remove(this.#file(`image-${id}`)));
    }

    // Every picture but those the history names, and what a write cut short left (Gio writes beside the file
    // first): nothing is being written while the queue runs this
    prune (ids) {
        return enqueue(() => {
            for (const name of names(this.dir)) {
                if ((name.startsWith('image-') && !ids.includes(name.slice(6))) || name.startsWith('.goutputstream-'))
                    remove(this.#file(name));
            }
        });
    }

    // Everything, for clear-on-boot
    clear () {
        return enqueue(() => names(this.dir).forEach(name => remove(this.#file(name))));
    }
}

// --- What Clipboard Indicator left -------------------------------------------------------------------------

function oldDir () {
    return Gio.File.new_for_path(GLib.build_filenamev([GLib.get_user_cache_dir(), OLD_UUID]));
}

// Clipboard Indicator's history in plain text, its pictures read into bytes, for the sealed store to take
// over: null when there is none, or while the user's own copy of Clipboard Indicator, whose folder it still
// is, is installed
export async function readOldCache () {
    const own = GLib.build_filenamev([GLib.get_user_data_dir(), 'gnome-shell', 'extensions', OLD_UUID]);
    const registry = oldDir().get_child('registry.txt');
    if (GLib.file_test(own, GLib.FileTest.EXISTS) || !registry.query_exists(null))
        return null;
    let items;
    try {
        items = JSON.parse(new TextDecoder().decode(await load(registry)));
    } catch (e) {
        return [];
    }
    const found = [];
    for (const item of Array.isArray(items) ? items : []) {
        if (typeof item?.contents !== 'string' || typeof item?.mimetype !== 'string')
            continue;
        if (item.mimetype.startsWith('image/')) {
            // Its own folder only
            const picture = Gio.File.new_for_path(item.contents);
            if (!picture.get_parent()?.equal(oldDir()))
                continue;
            try {
                found.push({ ...item, bytes: await load(picture) });
            } catch (e) {}
        } else {
            found.push(item);
        }
    }
    return found;
}

// Once what it held is sealed: the plain text, its pictures and the folder
export function forgetOldCache () {
    for (const name of [...names(oldDir()), null]) {
        try {
            (name ? oldDir().get_child(name) : oldDir()).delete(null);
        } catch (e) {}
    }
}
