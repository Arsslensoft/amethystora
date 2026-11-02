// A USB stick that tells the truth. A copy can say it is done while most of it is still in memory, on its way to the
// stick. While a removable disk is being written to, the pill says so with about how far it has got; once it is all there,
// it says that, and its card puts the disk away. The card names what holds the disk (What is using this?, with Quit), at
// once when putting it away is refused. "All on" is said only by stickmodel.js's rules: a sync that wrote nothing, dirty
// data that did not rise, nothing of the account holding the disk, and a file system a sync reaches the device of.
//
// Nothing tells a program that a disk is being written to (fanotify's mount marks are root's), so this looks, and keeps
// the looking small: with no writable removable disk nothing runs; with one mounted and someone at the computer, one
// read of /proc/meminfo every 15 seconds (30 after a minute of calm), the disk's own numbers only when the machine's dirty
// data has risen or the disk's top folder has changed (a file monitor on it wakes the look at once), each second while the
// disk is busy, and nothing at all after a minute with no input (Mutter's idle watch, which starts it again). Only
// sentences are made from what it reads: nothing is written, and the disk's name is shown as plain text.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {run} from '../tools/run.js';
import {UsersList} from '../tools/users.js';

import {buttonRow, cardButton, retire} from './clock.js';
import {BUSY, blockName, parseDirty, parseStat, provable, WriteWatch} from './stickmodel.js';
import {plain} from './text.js';

Gio._promisify(Gio.File.prototype, 'load_contents_async');
Gio._promisify(Gio.File.prototype, 'query_info_async');
Gio._promisify(Gio.File.prototype, 'query_filesystem_info_async');
Gio._promisify(Gio.Mount.prototype, 'unmount_with_operation');
Gio._promisify(Gio.Drive.prototype, 'eject_with_operation');
Gio._promisify(Gio.Drive.prototype, 'stop');
Gio._promisify(Gio.Subprocess.prototype, 'wait_check_async');

// Seconds between looks: while a disk is busy, while someone is here, and after a minute of calm
const BUSY_TICK = 1;
const QUIET_TICK = 15;
const SLOW_TICK = 30;
const CALM_READS = 4;
// Someone away this long has nothing to be told until they are back (ms); the words stay this many ticks once it is all there
const AWAY = 60000;
const DONE_TICKS = 8;
// A sync that has not come back in this many seconds proves nothing: it is given up on, and only its own process waits
const SYNC_LIMIT = 60;
// A nudge (a sync to start a stick writing now) at most this often per disk (µs), and a held disk is asked about this often
const NUDGE_EVERY = 60 * 1000000;
const ASK_EVERY = 10 * 1000000;
const NAME_LENGTH = 28;
const SYNC = '/usr/bin/sync';
const WHAT_USES = '/usr/libexec/amethystora-what-uses';
const MEGABYTE = 1024 * 1024;
const decoder = new TextDecoder();

async function readText(path) {
    const [bytes] = await Gio.File.new_for_path(path).load_contents_async(null);
    return decoder.decode(bytes);
}

// How many of the account's programs hold the folder, or null when that could not be read
async function holdersOf(root) {
    try {
        const {ok, stdout} = await run([WHAT_USES, 'list', '--json', '--', root], {timeout: 15000});
        if (!ok)
            return null;
        const data = JSON.parse(stdout);
        if (!Array.isArray(data?.holders))
            return null;
        // A scan cut short that found nothing has not looked everywhere
        return data.holders.length === 0 && data.truncated ? null : data.holders.length;
    } catch {
        return null;
    }
}

function isRemovable(mount) {
    const drive = mount.get_drive?.();
    return !!drive && (drive.is_removable?.() || drive.is_media_removable?.() || drive.can_eject?.());
}

function sameDrive(a, b) {
    if (!a || !b)
        return false;
    if (a === b)
        return true;
    const id = a.get_identifier?.('unix-device');
    return !!id && id === b.get_identifier?.('unix-device');
}

export class Stick {
    constructor(activities, notice) {
        this.id = 'stick';
        this.plate = '';
        this._activities = activities;
        this._notice = notice;
        this._disks = new Map();
        this._adopting = new Set();
    }

    enable() {
        this._monitor = Gio.VolumeMonitor.get();
        this._monitor.connectObject(
            'mount-added', () => this._look(),
            'mount-removed', () => this._look(),
            this);
        this._away = false;
        this._look();
    }

    disable() {
        this._monitor?.disconnectObject(this);
        this._monitor = null;
        this._stopTick();
        this._stopIdle();
        for (const disk of this._disks.values())
            this._drop(disk);
        this._disks.clear();
        this._adopting.clear();
        this._cardTitle = this._cardText = this._cardList = null;
        this._end();
    }

    // --- Which disks ----------------------------------------------------------------------------------------------

    _look() {
        const seen = new Set();
        for (const mount of this._monitor?.get_mounts() ?? []) {
            const root = mount.get_root?.()?.get_path?.();
            const device = mount.get_volume?.()?.get_identifier?.('unix-device');
            if (!root || !device || !isRemovable(mount))
                continue;
            seen.add(root);
            if (!this._disks.has(root) && !this._adopting.has(root)) {
                this._adopting.add(root);
                this._adopt(root, device, mount).catch(e => console.warn(`Amethystora UI: a removable disk: ${e.message}`))
                    .finally(() => this._adopting.delete(root));
            }
        }
        for (const [root, disk] of [...this._disks]) {
            if (!seen.has(root)) {
                this._drop(disk);
                this._disks.delete(root);
            }
        }
        this._show();
        this._syncIdle();
    }

    _drop(disk) {
        disk.monitor?.cancel();
        disk.monitor = null;
        if (disk.syncTimer)
            GLib.source_remove(disk.syncTimer);
        disk.syncTimer = 0;
    }

    // /dev/mapper/NAME is a link to /dev/dm-N, which is what sysfs knows it by
    async _adopt(root, device, mount) {
        let path = device;
        const link = await Gio.File.new_for_path(device).query_info_async('standard::symlink-target',
            Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, GLib.PRIORITY_DEFAULT, null);
        const target = link.get_symlink_target();
        if (target)
            path = GLib.path_get_basename(target);
        const name = blockName(GLib.path_get_basename(path));
        const file = Gio.File.new_for_path(root);
        const info = await file.query_filesystem_info_async('filesystem::type,filesystem::readonly', GLib.PRIORITY_DEFAULT, null);
        // Gone while this looked, or the module stopped
        if (!name || !this._monitor || !this._monitor.get_mounts().some(m => m.get_root?.()?.get_path?.() === root))
            return;
        // Nothing can be written to a disk mounted read-only: an installer stick costs nothing
        if (info.get_attribute_boolean('filesystem::readonly'))
            return;
        const disk = {root, mount, stat: `/sys/class/block/${name}/stat`, name: plain(mount.get_name(), NAME_LENGTH), done: 0,
            watch: new WriteWatch({proof: provable(info.get_attribute_string('filesystem::type'))}),
            syncing: false, wantSync: false, syncTimer: 0, asking: false, askedAt: 0, nudgedAt: 0,
            // A stick takes the nudge; a disk may be asleep, and its data goes when the kernel sends it
            nudges: !!mount.get_drive?.()?.is_media_removable?.()};
        // A change in the top folder says a look is due now rather than at the next tick
        try {
            disk.monitor = file.monitor_directory(Gio.FileMonitorFlags.NONE, null);
            disk.monitor.connect('changed', () => this._poke());
        } catch {}
        this._disks.set(root, disk);
        this._syncIdle();
    }

    // --- The looking ----------------------------------------------------------------------------------------------

    _syncIdle() {
        const any = this._disks.size > 0;
        if (any && !this._idleWatch)
            this._watchIdle();
        else if (!any)
            this._stopIdle();
        this._arm();
    }

    _watchIdle() {
        const monitor = global.backend.get_core_idle_monitor();
        this._idleWatch = monitor.add_idle_watch(AWAY, () => {
            this._away = true;
            this._activeWatch = monitor.add_user_active_watch(() => {
                this._activeWatch = 0;
                this._away = false;
                this._arm();
            });
        });
    }

    _stopIdle() {
        const monitor = global.backend.get_core_idle_monitor();
        if (this._idleWatch)
            monitor.remove_watch(this._idleWatch);
        if (this._activeWatch)
            monitor.remove_watch(this._activeWatch);
        this._idleWatch = 0;
        this._activeWatch = 0;
        this._away = false;
    }

    get _busy() {
        return [...this._disks.values()].some(disk => BUSY.includes(disk.watch.state));
    }

    // The next look: each second while a disk is busy, every 15 seconds (30 after a minute of calm) while someone is here,
    // otherwise none
    _arm(soon = false) {
        if (this._tick || this._reading)
            return;
        if (this._disks.size === 0 || (this._away && !this._busy))
            return;
        const seconds = soon || this._busy ? BUSY_TICK : this._calm >= CALM_READS ? SLOW_TICK : QUIET_TICK;
        this._tick = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT_IDLE, seconds, () => {
            this._tick = 0;
            this._read().catch(e => console.warn(`Amethystora UI: a removable disk's numbers: ${e.message}`)).finally(() => {
                this._reading = false;
                if (this._monitor)
                    this._arm();
            });
            return GLib.SOURCE_REMOVE;
        });
    }

    // Something changed in a disk's top folder: look within a second, and read the disk's numbers whatever the machine says
    _poke() {
        if (this._poked || !this._monitor)
            return;
        this._poked = true;
        this._calm = 0;
        this._stopTick();
        this._arm(true);
    }

    _stopTick() {
        if (this._tick)
            GLib.source_remove(this._tick);
        this._tick = 0;
    }

    async _read() {
        this._reading = true;
        const forced = this._poked;
        this._poked = false;
        const dirty = parseDirty(await readText('/proc/meminfo'));
        let calm = true;
        for (const disk of [...this._disks.values()]) {
            // A quiet disk's own numbers only once the machine has dirty data that may be for it
            if (!forced && !disk.watch.peek(dirty))
                continue;
            let stat = null;
            try {
                stat = parseStat(await readText(disk.stat));
            } catch {}
            // Gone while this looked, or the module stopped
            if (!stat || !this._monitor || this._disks.get(disk.root) !== disk)
                continue;
            const told = disk.watch.step({...stat, dirty});
            const now = GLib.get_monotonic_time();
            if (told.flush) {
                this._settle(disk);
            } else if (told.verify) {
                this._verify(disk);
            } else if (told.nudge && disk.nudges && now - disk.nudgedAt >= NUDGE_EVERY) {
                disk.nudgedAt = now;
                this._runSync(disk, null);
            }
            if (disk.watch.state === 'held')
                this._recheck(disk);
            if (disk.watch.state === 'done' && ++disk.done > DONE_TICKS) {
                disk.done = 0;
                disk.watch.rest();
            } else if (disk.watch.state !== 'done') {
                disk.done = 0;
            }
            if (disk.watch.state !== 'quiet')
                calm = false;
        }
        this._calm = calm ? (this._calm ?? 0) + 1 : 0;
        this._show();
    }

    // sync -f: the file system's data on its device when this returns. It runs as a program, which can wait out a slow
    // stick, one at a time per disk: a stick whose I/O hung leaves that one process waiting and starts no other. After
    // SYNC_LIMIT seconds nothing is waited for any more and the disk is not claimed to be anything. `after` gets whether it worked.
    _runSync(disk, after) {
        if (disk.syncing)
            return false;
        let process;
        try {
            process = Gio.Subprocess.new([SYNC, '-f', '--', disk.root], Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch {
            return false;
        }
        disk.syncing = true;
        disk.syncTimer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, SYNC_LIMIT, () => {
            disk.syncTimer = 0;
            disk.watch.timedOut();
            process.force_exit();
            this._show();
            return GLib.SOURCE_REMOVE;
        });
        process.wait_check_async(null).then(() => true, () => false).then(ok => {
            disk.syncing = false;
            if (disk.syncTimer)
                GLib.source_remove(disk.syncTimer);
            disk.syncTimer = 0;
            after?.(ok);
            // A proof was asked for while this one ran (a nudge): its turn is now
            if (disk.wantSync && this._disks.get(disk.root) === disk) {
                disk.wantSync = false;
                if (disk.watch.state === 'flushing')
                    this._settle(disk);
            }
        });
        return true;
    }

    // Asks for a sync to prove the disk; one already running (a nudge, or a hung one) is waited for, never doubled
    _settle(disk) {
        if (this._runSync(disk, ok => disk.watch.flushed(ok)))
            return;
        if (disk.syncing)
            disk.wantSync = true;
        else
            disk.watch.timedOut();
    }

    // Everything but "does anything of the account hold the disk?" says it is all there: ask, and say so only if not
    _verify(disk) {
        holdersOf(disk.root).then(count => {
            if (this._disks.get(disk.root) !== disk)
                return;
            disk.watch.verified(count === null ? null : count === 0);
            this._show();
            this._arm();
        });
    }

    // A held disk: when the holders have gone, all of it is proved again
    _recheck(disk) {
        const now = GLib.get_monotonic_time();
        if (disk.asking || now - disk.askedAt < ASK_EVERY)
            return;
        disk.asking = true;
        disk.askedAt = now;
        holdersOf(disk.root).then(count => {
            disk.asking = false;
            if (this._disks.get(disk.root) !== disk || count !== 0)
                return;
            if (disk.watch.release())
                this._settle(disk);
            this._show();
            this._arm();
        });
    }

    // --- What it says ---------------------------------------------------------------------------------------------

    // The disk the pill is about: one still being written to or held before one that is done
    _current() {
        const disks = [...this._disks.values()].filter(disk => disk.watch.state !== 'quiet');
        return disks.find(disk => disk.watch.state !== 'done') ?? disks[0] ?? null;
    }

    _show() {
        const disk = this._current();
        if (!disk) {
            this._end();
            return;
        }
        this._disk = disk;
        const state = disk.watch.state;
        const done = state === 'done';
        const {percent} = disk.watch.progress;
        const key = `${disk.root}/${state}/${percent}`;
        if (key === this._shownKey && this._activities.get('stick') === this)
            return;
        this._shownKey = key;
        this.pill();
        this._label.text = this.text;
        this._fill.scale_x = done || state === 'held' ? 1 : percent / 100;
        this._fill.visible = !done && state !== 'held';
        this._icon.icon_name = done ? 'emblem-ok-symbolic' : 'media-removable-symbolic';
        this._syncCard();
        if (this._activities.get('stick') === this)
            this._activities.update('stick');
        else
            this._activities.set('stick', this);
    }

    _end() {
        this._disk = null;
        this._shownKey = null;
        if (this._activities.get('stick') === this)
            this._activities.set('stick', null);
        retire(this._content);
        this._content = null;
    }

    get text() {
        const disk = this._disk;
        if (!disk)
            return '';
        const state = disk.watch.state;
        if (state === 'done')
            return _('All on “%s”').format(disk.name);
        if (state === 'held')
            return _('Something is still using “%s”').format(disk.name);
        const {percent} = disk.watch.progress;
        // Translators: the disk's name and roughly how much of what it was given has reached it; the number is an estimate
        return percent > 0 ? _('Still writing to “%s” · about %d%%').format(disk.name, percent) : _('Still writing to “%s”').format(disk.name);
    }

    get accessible() {
        return this.text;
    }

    // The icon follows the state, for the pill's bubble behind a more important activity; `short` is its few words
    get row() {
        const state = this._disk?.watch.state;
        const percent = this._disk?.watch.progress.percent ?? 0;
        let short;
        if (state === 'done')
            short = _('All on');
        else if (state === 'held')
            short = _('In use');
        else
            // Translators: a rough percent of what has reached a disk, for a small space
            short = percent > 0 ? _('About %d%%').format(percent) : _('Writing');
        return {icon: state === 'done' ? 'emblem-ok-symbolic' : 'media-removable-symbolic', text: this.text, short};
    }

    pill() {
        if (!this._content) {
            this._content = new St.Widget({style_class: 'amethystora-pill-content amethystora-pill-stick',
                layout_manager: new Clutter.BinLayout()});
            this._fill = new St.Widget({style_class: 'amethystora-pill-stick-fill', x_expand: true, y_expand: true});
            const rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
            this._fill.set_pivot_point(rtl ? 1 : 0, 0.5);
            this._content.add_child(this._fill);
            const row = new St.BoxLayout({style_class: 'amethystora-pill-stick-row', y_align: Clutter.ActorAlign.CENTER});
            this._icon = new St.Icon({icon_name: 'media-removable-symbolic', icon_size: 12, y_align: Clutter.ActorAlign.CENTER});
            this._label = new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER});
            row.add_child(this._icon);
            row.add_child(this._label);
            this._content.add_child(row);
        }
        return this._content;
    }

    // The two sentences of the card, from the disk's state now: an open card follows the disk
    _cardWords() {
        const disk = this._disk;
        const state = disk?.watch.state;
        const done = state === 'done';
        const {left} = disk?.watch.progress ?? {left: 0};
        const title = done ? _('Everything is on “%s”').format(disk.name) : _('Not safe to unplug yet');
        let text;
        if (done)
            text = _('Put it away, and it is safe to unplug.');
        else if (state === 'held')
            text = _('A program is still using it. A copy can look finished with data still on its way.');
        else if (left >= MEGABYTE)
            // Translators: a size, as "1.2 GB"; it counts everything waiting to be written on the computer
            text = _('A copy can look finished with data still on its way. About %s is still waiting to be written.').format(GLib.format_size(left));
        else
            text = _('A copy can look finished with data still on its way. It is still writing.');
        return {title, text};
    }

    _syncCard() {
        if (!this._cardTitle || !this._disk)
            return;
        const {title, text} = this._cardWords();
        this._cardTitle.text = title;
        this._cardText.text = text;
        if (this._disk.watch.state === 'held')
            this._cardList?.refresh();
    }

    card(api) {
        const disk = this._disk;
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body amethystora-pill-stick-card',
            orientation: Clutter.Orientation.VERTICAL});
        const {title, text} = this._cardWords();
        this._cardTitle = new St.Label({style_class: 'amethystora-pill-card-label', text: title});
        body.add_child(this._cardTitle);
        this._cardText = new St.Label({style_class: 'amethystora-pill-card-text', text});
        this._cardText.clutter_text.line_wrap = true;
        body.add_child(this._cardText);
        const list = new UsersList([disk.root]);
        this._cardList = list;
        body.add_child(list);
        body.connect('destroy', () => {
            if (this._cardList === list)
                this._cardTitle = this._cardText = this._cardList = null;
        });
        body.add_child(buttonRow([cardButton(_('Put it away'), () => {
            this._putAway(disk, list, api).catch(e => console.warn(`Amethystora UI: putting a disk away: ${e.message}`));
        }, {primary: true})]));
        return body;
    }

    // Every mount of the drive, as the volume monitor lists them now
    _mountsOf(drive) {
        return (this._monitor?.get_mounts() ?? []).filter(m => sameDrive(m.get_drive?.(), drive));
    }

    // Ejected when it can be, stopped when it can, otherwise every mount of its drive unmounted. "Safe to unplug" only when
    // none of the drive's mounts is left: a second partition may be in the middle of a write. Refused, the list says who holds it
    async _putAway(disk, list, api) {
        const {mount, name} = disk;
        const drive = mount.get_drive?.();
        try {
            if (drive?.can_eject()) {
                await drive.eject_with_operation(Gio.MountUnmountFlags.NONE, null, null);
            } else if (drive?.can_stop()) {
                await drive.stop(Gio.MountUnmountFlags.NONE, null, null);
            } else {
                for (const m of drive ? this._mountsOf(drive) : [mount])
                    await m.unmount_with_operation(Gio.MountUnmountFlags.NONE, null, null);
            }
        } catch {
            // No log line: a message of Gio's can name the disk
            if (list.get_stage())
                list.refresh();
            this._notice?.(_('“%s” could not be put away').format(name), 'dialog-warning-symbolic', 6);
            return;
        }
        api.close();
        const left = drive ? this._mountsOf(drive) : [];
        if (left.length === 0)
            this._notice?.(_('Safe to unplug “%s”').format(name), 'emblem-ok-symbolic', 6);
        else
            this._notice?.(_('“%s” is put away, but the rest of the drive is still in use').format(name), 'dialog-warning-symbolic', 6);
    }

    // For the suites and the harness
    get state() {
        return {
            disks: [...this._disks.values()].map(disk => ({root: disk.root, state: disk.watch.state})),
            looking: !!this._tick || !!this._reading,
            away: !!this._away,
            shown: this._activities.get('stick') === this,
            text: this.text,
        };
    }

    // A stand-in disk, with no real device behind it, for the harness: `reading` is what its numbers say
    fakeForTest(reading) {
        const root = '/tmp/stick';
        let disk = this._disks.get(root);
        if (!disk) {
            disk = {root, mount: null, stat: '/dev/null', watch: new WriteWatch(), name: 'Photos', done: 0, nudges: false,
                syncing: false, wantSync: false, syncTimer: 0, asking: false, askedAt: 0, nudgedAt: 0, monitor: null};
            this._disks.set(root, disk);
        }
        const told = disk.watch.step(reading);
        // The stand-in has no sync and nothing holding it: what a real disk's answers would be
        if (told.flush)
            disk.watch.flushed(true);
        else if (told.verify)
            disk.watch.verified(true);
        this._show();
        return disk.watch.state;
    }
}
