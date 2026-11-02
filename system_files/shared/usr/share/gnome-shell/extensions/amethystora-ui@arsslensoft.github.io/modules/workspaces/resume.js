// Pick up where you left off (plans/ui-ideas/spec-restart.md 1): when an update restarts the computer, the apps that
// were open and their places are kept as the saved sets keep a workspace, and the first session after it offers to
// open them again, once, through the pill (pill/resume.js). Only app IDs, numbers and screen connectors are kept: never
// a window's title, a file or a page.
//
// What asks for it: org.amethystora.UI.Resume.Save() on the session bus, which amethystora-soft-reboot calls right
// before it restarts and System Updates before GNOME's restart dialog; and a screen lock while an update waits for its
// restart (the extension's beforeDisable, while every module still runs), so that the night restart of System Updates,
// which runs with the screen locked and this extension off, finds the apps. The unlock after such a lock deletes it.
//
// ~/.local/state/amethystora/resume.json (0600) is read once per GNOME Shell, at the first start of this part, and
// deleted at once: it is offered only when the system that wrote it is not the one running (an update, or a rollback,
// happened) and it is under three days old. A file that does not fit (setsdata.js cleanResume) is dropped whole.
//
// A flat battery (power/flat.js) keeps the apps here too, without blocking: `battery` when its card shows, and `asleep`
// at a lock on battery, since nothing runs here once the screen is locked. Those are offered on the same system as well,
// for two weeks, since a computer can lie flat for a while. Plugging in, Not yet and the unlock take them away.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {MAX_BYTES, OFF_THE_BATTERY, cleanResume} from './setsdata.js';

const DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
export const RESUME_FILE = GLib.build_filenamev([DIR, 'resume.json']);
const PATH = '/org/amethystora/UI/Resume';
const XML = `<node>
  <interface name="org.amethystora.UI.Resume">
    <method name="Save">
      <arg type="b" direction="out" name="saved"/>
    </method>
  </interface>
</node>`;
// A file older than this is not offered, in seconds
const MAX_AGE = 72 * 3600;
const MAX_AGE_BATTERY = 14 * 24 * 3600;
// How long the pill offers it, in milliseconds
const OFFER_FOR = 10 * 60 * 1000;
const STAGED = '/run/ostree/staged-deployment';
const STAGED_LOCKED = '/run/ostree/staged-deployment-locked';
const SEALED = '/usr/share/amethystora/sealed';

// Kept outside the class, since a lock turns the extension off: whether this GNOME Shell has read the file, what it
// offers, and whether a lock wrote the file the unlock must take away again
let checked = false;
let offered = null;
let lockWrote = false;
// What keep() and forgetKept() have under way, one after the other: a write is never cancelled (GIO leaves its temporary
// file behind when it is) and a delete never overtakes a write
let tail = Promise.resolve();

const now = () => Math.floor(Date.now() / 1000);

// The running system, as the file names it: the image's version and the commit it was built from. It cannot change while
// this GNOME Shell runs, and GLib reads os-release each time it is asked.
let systemIdOnce = null;
export function systemId() {
    if (systemIdOnce === null) {
        const id = [GLib.get_os_info('IMAGE_VERSION') ?? '', GLib.get_os_info('BUILD_ID') ?? ''].join(':')
            .replace(/[^A-Za-z0-9._:+-]/g, '').slice(0, 96);
        systemIdOnce = id === ':' ? 'unknown' : id;
    }
    return systemIdOnce;
}

// TASK (a function returning a promise) after what is under way; a promise of its own result that never rejects
function queued(task) {
    const next = tail.then(task, task);
    tail = next.then(() => {}, () => {});
    return next;
}

function writeFile(data) {
    return new Promise(resolve => {
        const bytes = new GLib.Bytes(new TextEncoder().encode(JSON.stringify(data)));
        const flags = Gio.FileCreateFlags.REPLACE_DESTINATION | Gio.FileCreateFlags.PRIVATE;
        const attempt = again => Gio.File.new_for_path(RESUME_FILE).replace_contents_bytes_async(
            bytes, null, false, flags, null, (file, result) => {
                try {
                    file.replace_contents_finish(result);
                    resolve(true);
                } catch (e) {
                    // The folder is made only when it is not there, which is once for an account
                    if (again && e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.NOT_FOUND)) {
                        GLib.mkdir_with_parents(DIR, 0o700);
                        attempt(false);
                        return;
                    }
                    console.warn(`Amethystora UI: the open apps could not be kept: ${e.message}`);
                    resolve(false);
                }
            });
        attempt(true);
    });
}

function deleteFile() {
    return new Promise(resolve => {
        Gio.File.new_for_path(RESUME_FILE).delete_async(GLib.PRIORITY_DEFAULT, null, (file, result) => {
            try {
                file.delete_finish(result);
            } catch {}
            resolve(true);
        });
    });
}

function remove() {
    try {
        Gio.File.new_for_path(RESUME_FILE).delete(null);
    } catch {}
}

// The file's JSON, or null; it is deleted whatever it held
function takeFile() {
    let data = null;
    try {
        const file = Gio.File.new_for_path(RESUME_FILE);
        const info = file.query_info('standard::size', Gio.FileQueryInfoFlags.NONE, null);
        if (info.get_size() <= MAX_BYTES) {
            const [, bytes] = GLib.file_get_contents(RESUME_FILE);
            data = JSON.parse(new TextDecoder().decode(bytes));
        }
    } catch {}
    remove();
    return data;
}

// The resume key off when the shell started: what a restart left is deleted unread
export function forgetUnread() {
    if (checked)
        return;
    checked = true;
    remove();
}

function staged() {
    return GLib.file_test(STAGED, GLib.FileTest.EXISTS) && !GLib.file_test(STAGED_LOCKED, GLib.FileTest.EXISTS) &&
        !GLib.file_test(SEALED, GLib.FileTest.EXISTS);
}

export class Resume {
    constructor(module) {
        this._module = module;
    }

    enable() {
        this._dbus = Gio.DBusExportedObject.wrapJSObject(XML, {Save: () => this.save('restart')});
        this._dbus.export(Gio.DBus.session, PATH);
        if (!checked) {
            checked = true;
            this._read(takeFile());
        } else if (lockWrote) {
            // The unlock after a lock that kept the apps: no restart came
            lockWrote = false;
            this.forgetKept();
        }
    }

    disable() {
        this._dbus?.unexport();
        this._dbus = null;
    }

    // A screen lock, before any module stops: kept while an update waits for the restart, or, for a flat battery, while
    // it runs on the battery
    beforeDisable() {
        if (!Main.sessionMode.isLocked)
            return;
        if (this._module._ext.modules?.get('power')?.flat?.discharging) {
            lockWrote = true;
            this.keep('asleep');
        } else if (staged() && this.save('locked')) {
            lockWrote = true;
        }
    }

    _snapshot(kind) {
        const sets = this._module.sets;
        if (!sets)
            return null;
        const manager = global.workspace_manager;
        const workspaces = [];
        for (let index = 0; index < manager.n_workspaces; index++) {
            const captured = sets.capture(manager.get_workspace_by_index(index));
            if (captured)
                workspaces.push({index, layout: captured.set.layout, windows: captured.set.windows});
        }
        return cleanResume({
            version: 1, saved: now(), from: systemId(), kind, active: manager.get_active_workspace_index(), workspaces,
        });
    }

    // The same as save(), without blocking; a promise that always resolves, to whether it wrote
    keep(kind) {
        const data = this._snapshot(kind);
        if (!data) {
            this.forgetKept();
            return Promise.resolve(false);
        }
        return queued(() => writeFile(data));
    }

    // Plugged in, or Not yet: what keep() wrote goes, after any write under way
    forgetKept() {
        return queued(deleteFile);
    }

    // Every workspace's windows, written whole into the file; nothing open removes an older file. Says whether it wrote.
    save(kind) {
        const data = this._snapshot(kind);
        if (!this._module.sets)
            return false;
        if (!data) {
            remove();
            return false;
        }
        try {
            GLib.mkdir_with_parents(DIR, 0o700);
            GLib.file_set_contents_full(RESUME_FILE, new TextEncoder().encode(JSON.stringify(data)),
                GLib.FileSetContentsFlags.CONSISTENT, 0o600);
        } catch (e) {
            console.warn(`Amethystora UI: the open apps could not be kept for after the restart: ${e.message}`);
            return false;
        }
        return true;
    }

    // What the file holds, offered when another system wrote it, lately, and some of its apps are installed
    _read(raw) {
        const data = cleanResume(raw);
        if (!data || data.saved > now() + 60)
            return;
        // A rollback or an update explains itself; on the same system, only what a flat battery left is offered
        const updated = data.from !== systemId();
        const flat = OFF_THE_BATTERY.includes(data.kind);
        if ((!updated && !flat) || now() - data.saved > (flat ? MAX_AGE_BATTERY : MAX_AGE))
            return;
        const appSystem = Shell.AppSystem.get_default();
        const apps = [];
        const seen = new Set();
        let windows = 0;
        const entries = [];
        for (const entry of data.workspaces) {
            const kept = entry.windows.filter(w => appSystem.lookup_app(w.app));
            if (!kept.length)
                continue;
            entries.push({...entry, windows: kept});
            windows += kept.length;
            for (const w of kept) {
                if (!seen.has(w.app)) {
                    seen.add(w.app);
                    apps.push({app: w.app, workspace: entry.index});
                }
            }
        }
        if (!entries.length)
            return;
        offered = {entries, active: data.active, apps, windows, workspaces: entries.length, at: Date.now(),
            until: Date.now() + OFFER_FOR, why: updated ? 'update' : data.kind};
    }

    // What the pill offers ({apps: [{app, workspace}], windows, workspaces, at, until, why: 'update', 'battery' or
    // 'asleep'}), or null once it is over
    get offer() {
        if (offered && Date.now() >= offered.until)
            offered = null;
        return offered;
    }

    reopen() {
        const offer = this.offer;
        offered = null;
        if (offer)
            this._module.sets?.openQueue(offer.entries, offer.active);
    }

    dismiss() {
        offered = null;
    }

    // For the suites: the file read again as at a shell's start; says whether it is offered
    readForTest() {
        offered = null;
        this._read(takeFile());
        return !!offered;
    }
}
