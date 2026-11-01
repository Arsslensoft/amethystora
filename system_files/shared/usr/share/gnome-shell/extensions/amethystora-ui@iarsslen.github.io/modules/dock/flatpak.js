// Which Flatpak apps are being updated right now, for the dock and the app grid to grey their icons. Neither Bazaar,
// `flatpak update` nor uupd says so, and the system helper sends no signal, but what a pull leaves on disk names the
// app before its download starts:
// - through the system helper: /var/tmp/flatpak-cache-*/<app id>-XXXXXX, the download's mountpoint, gone just
//   before the deploy;
// - straight into a repository (--user, and uupd as root): repo/tmp/staging-*, whose commit says the ref, held
//   by staging-*-lock while the pull runs.
// Folder monitors see them come and go, and one that comes counts at once; one that goes, or a staging folder whose
// lock goes, ends at once. A pull that dies says nothing, so only while something is being updated a check every 5 s
// asks whether each one is still alive: the helper's mountpoint still mounted, the staging lock still held
// (/proc/locks); one found dead a whole check after it came stops counting. A folder found already there at start
// counts only if it is alive. Monitors watch one folder's own entries, never the objects written inside a staging
// folder: while a staging folder's commit has not come, the folder is looked into twice a second.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const CHECK_SECONDS = 5;
const COMMIT_MS = 500;
const CACHE_PARENT = '/var/tmp';
const CACHE_PREFIX = 'flatpak-cache-';
const STAGING_PREFIX = 'staging-';
const LOCK_SUFFIX = '-lock';
// <app id>-XXXXXX, the id as Flatpak allows it
const DOWNLOAD = /^([A-Za-z_][\w-]*(?:\.[A-Za-z_][\w-]*)+)-[A-Za-z0-9]{6}$/;
// A commit object's type: (metadata, parent, related, subject, body, time, root tree, root meta)
const COMMIT_TYPE = '(a{sv}aya(say)sstayay)';

function children(path) {
    const names = [];
    try {
        const list = Gio.File.new_for_path(path).enumerate_children('standard::name',
            Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, null);
        let info;
        while ((info = list.next_file(null)))
            names.push(info.get_name());
        list.close(null);
    } catch {
        // Gone, or not ours to read
    }
    return names;
}

function exists(path) {
    return GLib.file_test(path, GLib.FileTest.EXISTS);
}

function readText(path) {
    try {
        const [ok, bytes] = GLib.file_get_contents(path);
        return ok ? new TextDecoder().decode(bytes) : '';
    } catch {
        return '';
    }
}

// The app a staging folder's commit object is for, or null while the commit has not arrived yet
function stagedApp(staging) {
    for (const dir of children(staging)) {
        if (!/^[0-9a-f]{2}$/.test(dir))
            continue;
        for (const name of children(`${staging}/${dir}`)) {
            if (!name.endsWith('.commit'))
                continue;
            try {
                const [ok, bytes] = GLib.file_get_contents(`${staging}/${dir}/${name}`);
                if (!ok)
                    continue;
                const commit = GLib.Variant.new_from_bytes(new GLib.VariantType(COMMIT_TYPE),
                    new GLib.Bytes(bytes), false);
                const metadata = commit.get_child_value(0).recursiveUnpack();
                for (const ref of metadata['ostree.ref-binding'] ?? []) {
                    const parts = String(ref).split('/');
                    if (parts[0] === 'app' && parts[1])
                        return parts[1];
                }
            } catch {
                // Half written: looked at again
            }
        }
    }
    return null;
}

export class FlatpakUpdates {
    // onChange(launcherId) is called whenever an app starts or stops being updated
    constructor(onChange) {
        this._onChange = onChange;
        this._started = false;
    }

    start() {
        if (this._started)
            return;
        this._started = true;
        // Every pull seen, by the folder that marks it: {kind: 'download' | 'staging', app (null until known),
        // since (when it was seen), locked (its lock was seen)}
        this._pulls = new Map();
        this._monitors = new Map();
        this._watch(CACHE_PARENT, () => this._scanCaches(false));
        for (const repo of ['/var/lib/flatpak/repo', `${GLib.get_user_data_dir()}/flatpak/repo`])
            this._watch(`${repo}/tmp`, () => this._scanStaging(`${repo}/tmp`, false));
        // What is already there counts only if it is alive
        for (const repo of ['/var/lib/flatpak/repo', `${GLib.get_user_data_dir()}/flatpak/repo`])
            this._scanStaging(`${repo}/tmp`, true);
        this._scanCaches(true);
    }

    stop() {
        if (!this._started)
            return;
        this._started = false;
        for (const id of [this._timer, this._commitTimer]) {
            if (id)
                GLib.source_remove(id);
        }
        this._timer = this._commitTimer = 0;
        for (const monitor of this._monitors.values())
            monitor.cancel();
        this._monitors = null;
        const apps = this._apps();
        this._pulls = null;
        for (const app of apps)
            this._onChange(`${app}.desktop`);
    }

    // Whether the app with this launcher ID is being updated
    has(id) {
        if (!this._pulls || !id?.endsWith('.desktop'))
            return false;
        return this._apps().has(id.slice(0, -'.desktop'.length));
    }

    _apps() {
        return new Set([...this._pulls?.values() ?? []].map(pull => pull.app).filter(Boolean));
    }

    _watch(path, changed) {
        if (this._monitors.has(path))
            return;
        let monitor;
        try {
            monitor = Gio.File.new_for_path(path).monitor_directory(Gio.FileMonitorFlags.WATCH_MOVES, null);
        } catch {
            return;
        }
        monitor.connect('changed', () => {
            if (this._started)
                changed();
        });
        this._monitors.set(path, monitor);
    }

    _unwatch(path) {
        this._monitors?.get(path)?.cancel();
        this._monitors?.delete(path);
    }

    // The system helper's cache folders, each watched for the downloads made into it
    _scanCaches(atStart) {
        const caches = children(CACHE_PARENT).filter(name => name.startsWith(CACHE_PREFIX))
            .map(name => `${CACHE_PARENT}/${name}`);
        for (const path of [...this._monitors.keys()]) {
            if (path.startsWith(`${CACHE_PARENT}/${CACHE_PREFIX}`) && !caches.includes(path))
                this._unwatch(path);
        }
        for (const cache of caches)
            this._watch(cache, () => this._scanDownloads(cache, false));
        for (const cache of caches)
            this._scanDownloads(cache, atStart);
        this._changed(atStart);
    }

    _scanDownloads(cache, atStart) {
        const before = this._apps();
        for (const name of children(cache)) {
            const match = DOWNLOAD.exec(name);
            const path = `${cache}/${name}`;
            if (match && !this._pulls.has(path))
                this._pulls.set(path, {kind: 'download', app: match[1], since: atStart ? 0 : Date.now()});
        }
        this._changed(atStart, before);
    }

    _scanStaging(tmp, atStart) {
        const before = this._apps();
        for (const name of children(tmp)) {
            const path = `${tmp}/${name}`;
            if (name.startsWith(STAGING_PREFIX) && !name.endsWith(LOCK_SUFFIX) && !this._pulls.has(path))
                this._pulls.set(path, {kind: 'staging', app: null, since: atStart ? 0 : Date.now()});
        }
        this._changed(atStart, before);
    }

    // Whether a pull is still running: the download still mounted, the staging folder's lock still held
    _alive(path, pull, mounts, locks) {
        if (pull.kind === 'download')
            return mounts.has(path);
        try {
            const info = Gio.File.new_for_path(`${path}${LOCK_SUFFIX}`).query_info('unix::inode',
                Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, null);
            return locks.has(String(info.get_attribute_uint64('unix::inode')));
        } catch {
            return false;
        }
    }

    // After a monitor's news: what went away ends, a staging folder's app is read, and the dock told. At start, each
    // pull is checked for life at once. BEFORE: the apps being updated before the news, when the caller already changed
    // what is known
    _changed(atStart, before = this._apps()) {
        if (!this._started)
            return;
        for (const [path, pull] of [...this._pulls]) {
            const lock = `${path}${LOCK_SUFFIX}`;
            if (pull.kind === 'staging' && exists(lock))
                pull.locked = true;
            if (!exists(path) || (pull.kind === 'staging' && pull.locked && !exists(lock)))
                this._pulls.delete(path);
            else if (!pull.app && pull.kind === 'staging')
                pull.app = stagedApp(path);
        }
        if (atStart)
            this._check(true);
        this._tell(before);
        this._schedule();
    }

    // The slow check: a pull found dead a whole check after it came stops counting
    _check(all) {
        const mounts = new Set(readText('/proc/self/mountinfo').split('\n')
            .map(line => line.split(' ')[4]).filter(Boolean)
            .map(point => point.replace(/\\040/g, ' ')));
        const locks = new Set(readText('/proc/locks').split('\n')
            .map(line => line.trim().split(/\s+/)[5]?.split(':')[2]).filter(Boolean));
        // A second's slack, so that the check that comes a period after a pull judges it
        const old = Date.now() - (CHECK_SECONDS - 1) * 1000;
        for (const [path, pull] of [...this._pulls]) {
            if ((all || pull.since <= old) && !this._alive(path, pull, mounts, locks))
                this._pulls.delete(path);
        }
    }

    _tell(before) {
        const after = this._apps();
        for (const app of new Set([...before, ...after])) {
            if (before.has(app) !== after.has(app))
                this._onChange(`${app}.desktop`);
        }
    }

    // The checks run only while something is being updated
    _schedule() {
        if (this._pulls.size && !this._timer) {
            this._timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, CHECK_SECONDS * 1000, () => {
                this._timer = 0;
                const before = this._apps();
                this._check(false);
                this._tell(before);
                this._schedule();
                return GLib.SOURCE_REMOVE;
            });
        }
        const waiting = [...this._pulls.values()].some(pull => pull.kind === 'staging' && !pull.app);
        if (waiting && !this._commitTimer) {
            this._commitTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, COMMIT_MS, () => {
                this._commitTimer = 0;
                this._changed(false);
                return GLib.SOURCE_REMOVE;
            });
        }
    }
}
