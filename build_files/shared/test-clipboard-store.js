// Amethystora Clipboard's store.js (the extension's sealed history) outside the shell, for test-clipboard.sh,
// with a stand-in for the keyring and HOME a folder of the test's own: what is written is sealed and nowhere
// in the clear, opens again, is kept as keep-history says, a history another key sealed is moved aside, and
// what Clipboard Indicator left is taken over from its own folder only.
//
//   gjs -m test-clipboard-store.js MARKER

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import System from 'system';
import { Store, stamp, survivors, readOldCache, forgetOldCache } from
    'file:///usr/share/gnome-shell/extensions/amethystora-clipboard@arsslensoft.github.io/store.js';

const [marker] = System.programArgs;
const encoder = new TextEncoder();
const HOME = GLib.get_home_dir();
const DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora', 'clipboard']);
const OLD = GLib.build_filenamev([GLib.get_user_cache_dir(), 'clipboard-indicator@tudmotu.com']);

function check (what, ok) {
    if (!ok)
        throw new Error(what);
    print(`ok - ${what}`);
}

function contains (bytes, needle) {
    outer: for (let i = 0; i + needle.length <= bytes.length; i++) {
        for (let j = 0; j < needle.length; j++) {
            if (bytes[i + j] !== needle[j])
                continue outer;
        }
        return true;
    }
    return false;
}

// Every file under the home, and whether any holds the marker as it was written
function plainAnywhere (folder, needle) {
    const children = Gio.File.new_for_path(folder).enumerate_children('standard::name,standard::type',
        Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, null);
    let info;
    while ((info = children.next_file(null))) {
        const path = GLib.build_filenamev([folder, info.get_name()]);
        if (info.get_file_type() === Gio.FileType.DIRECTORY) {
            if (plainAnywhere(path, needle))
                return true;
        } else if (contains(GLib.file_get_contents(path)[1], needle)) {
            return true;
        }
    }
    return false;
}

const mode = path => Gio.File.new_for_path(path).query_info('unix::mode', 0, null).get_attribute_uint32('unix::mode') & 0o777;
const exists = path => GLib.file_test(path, GLib.FileTest.EXISTS);
const find = async newKey => ({ state: 'open', key: await newKey() });

async function main () {
    const store = new Store();
    await store.open(find);
    check('a key is made and the store opens', store.state === 'open' && store.key.length === 44);

    const text = 'text/plain;charset=utf-8';
    const id = Store.newId();
    await store.sealImage(id, encoder.encode(`${marker}-picture`));
    await store.writeHistory([
        { favorite: false, mimetype: text, contents: marker },
        { favorite: true, mimetype: text, contents: `${marker}-pinned` },
        { favorite: false, mimetype: 'image/png', contents: id },
    ]);
    check('the history and the picture are written, sealed', exists(`${DIR}/history`) && exists(`${DIR}/image-${id}`));
    check('nothing in the home holds what was copied', !plainAnywhere(HOME, encoder.encode(marker)));
    check('the folder is 0700 and the files 0600',
        mode(DIR) === 0o700 && mode(`${DIR}/history`) === 0o600 && mode(`${DIR}/image-${id}`) === 0o600);

    const history = await store.readHistory();
    check('the history opens again', history?.entries?.length === 3 && history.entries[0].contents === marker);
    check('so does the picture', new TextDecoder().decode(await store.openImage(id)) === `${marker}-picture`);
    check('a name that is not a picture\'s is refused', await store.openImage('../history') === null);

    const now = stamp();
    const count = (h, keep, at = now) => survivors(h, keep, at).length;
    check('keep-history keeps everything in this session', ['always', 'restart', 'session'].every(k => count(history, k) === 3));
    const later = { ...now, shell: now.shell + 1 };
    check('after a login: session keeps the pinned item only', count(history, 'session', later) === 1 &&
        count(history, 'restart', later) === 3 && count(history, 'always', later) === 3);
    const restarted = { ...now, boot: 'another start' };
    check('after a restart: restart and session keep the pinned item only', count(history, 'restart', restarted) === 1 &&
        count(history, 'session', restarted) === 1 && count(history, 'always', restarted) === 3);

    // A burst: every write asked for at once, only the newest waiting one written, the last of them on disk
    const burst = [];
    for (let i = 0; i < 50; i++)
        burst.push(store.writeHistory([{ favorite: false, mimetype: text, contents: `${marker}-burst-${i}` }]));
    await Promise.all(burst);
    const last = await store.readHistory();
    check('a burst of writes leaves the newest history', last?.entries?.[0]?.contents === `${marker}-burst-49`);
    await store.writeHistory(history.entries);

    GLib.file_set_contents(`${DIR}/.goutputstream-VMLAB1`, 'left by a write cut short');
    await store.prune([]);
    check('a picture the history does not name is taken away', !exists(`${DIR}/image-${id}`));
    check('and what a write cut short left', !exists(`${DIR}/.goutputstream-VMLAB1`));

    const other = new Store();
    await other.open(find);
    check('another key does not open it, and it is kept aside', await other.readHistory() === null &&
        exists(`${DIR}/history.unreadable`) && !exists(`${DIR}/history`));

    await other.writeHistory([{ favorite: false, mimetype: text, contents: 'x'.repeat(4096) }]);
    check('a history larger than the limit is moved aside', await other.readHistory(1024) === null && exists(`${DIR}/history~`));

    // What Clipboard Indicator left: its plain registry, a picture in its folder, and one named outside it
    GLib.mkdir_with_parents(OLD, 0o755);
    GLib.file_set_contents(`${OLD}/0123abcd`, `${marker}-old-picture`);
    GLib.file_set_contents(`${OLD}/registry.txt`, JSON.stringify([
        { favorite: true, mimetype: text, contents: `${marker}-old` },
        { favorite: false, mimetype: 'image/png', contents: `${OLD}/0123abcd` },
        { favorite: false, mimetype: 'image/png', contents: '/etc/os-release' },
    ]));
    const old = await readOldCache();
    check('Clipboard Indicator\'s history is read, its pictures from its own folder only', old?.length === 2 &&
        old[0].contents === `${marker}-old` && new TextDecoder().decode(old[1].bytes) === `${marker}-old-picture`);
    forgetOldCache();
    check('and its folder goes once it is sealed', !exists(OLD));

    GLib.mkdir_with_parents(OLD, 0o755);
    GLib.file_set_contents(`${OLD}/registry.txt`, '[]');
    GLib.mkdir_with_parents(`${GLib.get_user_data_dir()}/gnome-shell/extensions/clipboard-indicator@tudmotu.com`, 0o755);
    check('nothing is taken while the user\'s own Clipboard Indicator is installed', await readOldCache() === null);
}

const loop = new GLib.MainLoop(null, false);
let status = 0;
main().catch(e => {
    printerr(`not ok - ${e.message}`);
    status = 1;
}).finally(() => loop.quit());
loop.run();
System.exit(status);
