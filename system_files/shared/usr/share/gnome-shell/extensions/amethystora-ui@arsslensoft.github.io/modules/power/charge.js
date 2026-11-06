// Charged by the time you leave (plans/ui-ideas/r7/spec-keep.md, R7-70): plugged in for the night, the battery waits at the firmware's
// charge limit, and the hold comes off 75 minutes before the person usually leaves so that it is full then. "Full now, just this once" is
// always one press away. Offered and off (the key `charge-by-leaving`); a computer whose battery takes no limit never offers it and never
// connects to anything.
//
// The limit is UPower's own (EnableChargeThreshold, which asks polkit), read back after each change: a laptop that did not take it is said
// so, and nothing is held. /usr/libexec/amethystora-charge-limit says which batteries have `charge_control_end_threshold` at all.
//
// What it learns is the minute of the day of each unplug that followed a night on mains (chargedata.js), sealed in Vault under the purpose
// `charge-pattern`. It is read only after a person's own click ("Use my pattern"), and the leaving time it works out is held in memory
// only: after a restart of the shell, or with the vault closed, nothing is known and the computer charges fully, said once. An earlier
// event in the calendar (its start time, never its title) moves the time earlier.
//
// What the hold does outlives a screen lock, which turns the extension off: the keeper below is module state that a lock leaves running, with
// the one timer that takes the hold off. Everything is given back when the part is turned off or the session ends: a hold this part made is
// taken off, and a limit the person set themselves is never touched. A marker file (a flag, nothing more) lets the next start take off a hold
// the last one could not.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import * as Vault from '../vault.js';
import {FILE_NAME, PURPOSE, RECORD, decode, encode, learn, merge, nextLeave, plan, withCalendar} from './chargedata.js';

const BUS_NAME = 'org.freedesktop.UPower';
const TYPE_BATTERY = 2;
const ROOT_XML = `<node>
  <interface name="org.freedesktop.UPower">
    <method name="EnumerateDevices"><arg type="ao" direction="out"/></method>
    <property name="OnBattery" type="b" access="read"/>
  </interface>
</node>`;
const DEVICE_XML = `<node>
  <interface name="org.freedesktop.UPower.Device">
    <property name="Type" type="u" access="read"/>
    <property name="IsPresent" type="b" access="read"/>
    <property name="ChargeThresholdSupported" type="b" access="read"/>
    <property name="ChargeThresholdEnabled" type="b" access="read"/>
    <property name="ChargeEndThreshold" type="u" access="read"/>
    <method name="EnableChargeThreshold"><arg type="b" direction="in"/></method>
  </interface>
</node>`;
const RootProxy = Gio.DBusProxy.makeProxyWrapper(ROOT_XML);
const DeviceProxy = Gio.DBusProxy.makeProxyWrapper(DEVICE_XML);

const DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const PATTERN_FILE = GLib.build_filenamev([DIR, FILE_NAME]);
const MARKER = GLib.build_filenamev([DIR, 'charge-by-hold']);
// How long a change of the limit has to show in UPower's own property before it is called refused, in seconds
const CONFIRM = 3;
// How long what a restart did not know shows in the pill, in seconds
const SAID_FOR = 90;
const DAY_MS = 24 * 3600 * 1000;

// UPower, as the keeper uses it. The tests give their own with the same members.
export class UPowerBackend {
    constructor() {
        this.supported = false;
        this._waiters = [];
    }

    // `onChange` is told whenever the mains, the battery or its limit changes; `done(supported)` once it is known
    start(onChange, done) {
        this._cancellable = new Gio.Cancellable();
        const cancelled = () => !this._cancellable || this._cancellable.is_cancelled();
        new RootProxy(Gio.DBus.system, BUS_NAME, '/org/freedesktop/UPower', (root, error) => {
            if (error || cancelled()) {
                done(false);
                return;
            }
            this._root = root;
            root.EnumerateDevicesRemote((result, failure) => {
                if (failure || cancelled()) {
                    this._release();
                    done(false);
                    return;
                }
                this._probe(result[0], 0, onChange, done);
            });
        }, this._cancellable, Gio.DBusProxyFlags.DO_NOT_AUTO_START);
    }

    _probe(paths, index, onChange, done) {
        if (index >= paths.length) {
            this._release();
            done(false);
            return;
        }
        new DeviceProxy(Gio.DBus.system, BUS_NAME, paths[index], (proxy, error) => {
            if (!this._cancellable || this._cancellable.is_cancelled())
                return;
            if (!error && proxy.Type === TYPE_BATTERY && proxy.IsPresent === true && proxy.ChargeThresholdSupported === true) {
                this._battery = proxy;
                this.supported = true;
                this._ids = [
                    [this._root, this._root.connect('g-properties-changed', () => this._changed(onChange))],
                    [proxy, proxy.connect('g-properties-changed', () => this._changed(onChange))],
                ];
                done(true);
                return;
            }
            this._probe(paths, index + 1, onChange, done);
        }, this._cancellable, Gio.DBusProxyFlags.DO_NOT_AUTO_START);
    }

    _changed(onChange) {
        this._waiters = this._waiters.filter(waiter => !waiter());
        onChange();
    }

    get onBattery() {
        return this._root?.OnBattery === true;
    }

    get thresholdOn() {
        return this._battery?.ChargeThresholdEnabled === true;
    }

    get limit() {
        return this._battery?.ChargeEndThreshold ?? 0;
    }

    // The limit turned on or off, and said to have taken only when UPower's own property shows it: a promise of whether it did
    setThreshold(on) {
        return new Promise(resolve => {
            if (!this._battery) {
                resolve(false);
                return;
            }
            if (this.thresholdOn === on) {
                resolve(true);
                return;
            }
            let timer = 0;
            const settle = ok => {
                if (timer)
                    GLib.source_remove(timer);
                timer = 0;
                resolve(ok);
            };
            const waiter = () => {
                if (this.thresholdOn !== on)
                    return false;
                settle(true);
                return true;
            };
            this._waiters.push(waiter);
            timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, CONFIRM, () => {
                timer = 0;
                this._waiters = this._waiters.filter(w => w !== waiter);
                resolve(this.thresholdOn === on);
                return GLib.SOURCE_REMOVE;
            });
            this._battery.EnableChargeThresholdRemote(on, (result, error) => {
                if (error)
                    settle(false);
            });
        });
    }

    _release() {
        for (const [proxy, id] of this._ids ?? [])
            proxy.disconnect(id);
        this._ids = null;
        this._battery = null;
        this._root = null;
    }

    stop() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._waiters = [];
        this._release();
        this.supported = false;
    }
}

// What outlives the extension's disable and enable at every lock: whether it has started, the backend, what is learnt and what is held
const keeper = {
    started: false, backend: null, owner: null, ready: false, supported: false,
    onBattery: null, pluggedSince: null,
    // The pattern: `list` what is known (the file's, once opened, and this session's), `fresh` what was seen before it was opened
    list: [], fresh: [], loaded: false, onDisk: false, problem: null, saidSealed: false, saidUntil: 0,
    // The hold: whether this part turned the limit on, when it comes off, the leaving time it works from (memory only), and "Full now"
    hold: false, timer: 0, leaveAt: null, releaseAt: null, skip: false, refused: false, stuck: false, personal: null,
    inhibitor: null, inhibiting: false, subs: null, saidKeep: false,
};
let tail = Promise.resolve();

function queued(task) {
    const next = tail.then(task, task);
    tail = next.then(() => {}, () => {});
    return next;
}

function tell() {
    keeper.owner?.changed();
}

const fileExists = path => new Promise(resolve => {
    Gio.File.new_for_path(path).query_info_async('standard::size', Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_DEFAULT, null,
        (file, result) => {
            try {
                file.query_info_finish(result);
                resolve(true);
            } catch {
                resolve(false);
            }
        });
});

function removeFile(path) {
    return new Promise(resolve => Gio.File.new_for_path(path).delete_async(GLib.PRIORITY_DEFAULT, null, (file, result) => {
        try {
            file.delete_finish(result);
        } catch {}
        resolve();
    }));
}

function writeFile(path, bytes) {
    return new Promise(resolve => {
        const flags = Gio.FileCreateFlags.REPLACE_DESTINATION | Gio.FileCreateFlags.PRIVATE;
        const attempt = again => Gio.File.new_for_path(path).replace_contents_bytes_async(
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
                    resolve(false);
                }
            });
        attempt(true);
    });
}

function readFile(path) {
    return new Promise(resolve => Gio.File.new_for_path(path).load_contents_async(null, (file, result) => {
        try {
            resolve(file.load_contents_finish(result)[1]);
        } catch {
            resolve(null);
        }
    }));
}

// The start times of the calendar's events in the next day, from the event source GNOME's calendar menu has (times only; no title is read)
function eventStarts(now) {
    const source = Main.panel.statusArea.dateMenu?._eventSource;
    if (!source || typeof source.getEvents !== 'function' || source.isLoading)
        return [];
    try {
        return source.getEvents(new Date(now), new Date(now + DAY_MS))
            .map(event => ({start: event.date?.getTime?.(), stop: event.end?.getTime?.()}))
            .filter(({start, stop}) => Number.isFinite(start) && Number.isFinite(stop) && !(stop - start >= DAY_MS && new Date(start).getHours() === 0))
            .map(({start}) => start);
    } catch {
        return [];
    }
}

// ---- the hold ----

const LOGIND = 'org.freedesktop.login1';
const LOGIND_PATH = '/org/freedesktop/login1';
const LOGIND_MANAGER = 'org.freedesktop.login1.Manager';

function disarm() {
    if (keeper.timer)
        GLib.source_remove(keeper.timer);
    keeper.timer = 0;
    keeper.releaseAt = null;
}

// A delay inhibitor on sleep and shutdown while the limit is on, so that it can be taken off before the computer goes: the firmware keeps it through
// the night otherwise, and the battery is at the limit when the person leaves. Its file descriptor is closed to let the computer go.
function takeInhibitor() {
    if (keeper.inhibitor !== null || keeper.inhibiting)
        return;
    keeper.inhibiting = true;
    Gio.DBus.system.call_with_unix_fd_list(LOGIND, LOGIND_PATH, LOGIND_MANAGER, 'Inhibit',
        new GLib.Variant('(ssss)', ['sleep:shutdown', 'Amethystora UI', 'Taking the battery limit off', 'delay']),
        new GLib.VariantType('(h)'), Gio.DBusCallFlags.NONE, -1, null, null, (bus, result) => {
            keeper.inhibiting = false;
            try {
                const [reply, fds] = bus.call_with_unix_fd_list_finish(result);
                const fd = fds.get(reply.deepUnpack()[0]);
                if (keeper.hold)
                    keeper.inhibitor = fd;
                else
                    GLib.close(fd);
            } catch (e) {
                console.debug(`Amethystora UI: no inhibitor for the battery limit: ${e.message}`);
            }
        });
}

function dropInhibitor() {
    if (keeper.inhibitor === null)
        return;
    const fd = keeper.inhibitor;
    keeper.inhibitor = null;
    try {
        GLib.close(fd);
    } catch {}
}

// The limit taken off, if this part turned it on. The hold and its marker go only once UPower says it is off: a refusal (UPower asks polkit, which
// says no to a session that is not the active one) leaves both, so that the next change or the next start puts it right, and it is said once.
async function release() {
    disarm();
    if (!keeper.hold)
        return true;
    const ok = await keeper.backend?.setThreshold(false);
    if (!ok) {
        keeper.stuck = true;
        tell();
        return false;
    }
    keeper.hold = false;
    keeper.stuck = false;
    dropInhibitor();
    await removeFile(MARKER);
    tell();
    return true;
}

// A limit that was there before this part ever held the battery is the person's own (a vendor tool, TLP, Settings): UPower's own switch on, or an end
// threshold below full while it is off, the first time this looks in a session and no hold of ours has been made
function personalLimit(backend) {
    if (keeper.personal === null)
        keeper.personal = backend.thresholdOn || (backend.limit > 0 && backend.limit < 100);
    return keeper.personal;
}

async function decide() {
    const backend = keeper.backend;
    if (!keeper.ready || !backend?.supported || backend.onBattery)
        return;
    // A limit the person set themselves is theirs: nothing here touches it
    if (!keeper.hold && personalLimit(backend)) {
        keeper.leaveAt = null;
        tell();
        return;
    }
    const now = Date.now();
    // The pattern as far as it is known in memory: opened, or never sealed at all. A sealed one nobody has opened is not known.
    const usual = nextLeave(keeper.loaded ? keeper.list : keeper.onDisk ? [] : keeper.fresh, now);
    const leaveAt = usual === null || keeper.skip ? null : withCalendar(usual, eventStarts(now), now);
    keeper.leaveAt = leaveAt;
    const {hold, releaseAt} = keeper.skip ? {hold: false, releaseAt: null} : plan(now, leaveAt);
    if (!hold) {
        await release();
        // Nothing known because the pattern is sealed and nobody opened it: said once, with the way to open it
        if (!keeper.skip && !keeper.loaded && keeper.onDisk && !keeper.saidSealed)
            saySomething('saidSealed');
        tell();
        return;
    }
    if (!keeper.hold) {
        if (!await backend.setThreshold(true)) {
            // This laptop did not take it: said, and nothing is held
            keeper.refused = true;
            keeper.leaveAt = null;
            tell();
            return;
        }
        keeper.refused = false;
        keeper.hold = true;
        keeper.personal = false;
        await writeFile(MARKER, new TextEncoder().encode('1\n'));
    }
    takeInhibitor();
    arm(releaseAt);
    tell();
}

// The release at a time on the wall clock. The timer counts the time the computer is awake only, so when it fires the clock is asked again, and the
// sleep and shutdown signals below take the limit off before the computer goes.
function arm(releaseAt) {
    disarm();
    keeper.releaseAt = releaseAt;
    keeper.timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, Math.max(1, Math.ceil((releaseAt - Date.now()) / 1000)), () => {
        keeper.timer = 0;
        if (Date.now() + 1000 < releaseAt)
            arm(releaseAt);
        else
            release();
        return GLib.SOURCE_REMOVE;
    });
}

// Said once, for about a minute and a half, in the pill (which hides it when it is over)
function saySomething(flag) {
    keeper[flag] = true;
    keeper.saidUntil = Date.now() + SAID_FOR * 1000;
    GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, SAID_FOR, () => {
        tell();
        return GLib.SOURCE_REMOVE;
    });
}

async function sealList() {
    return queued(async () => {
        const sealed = await Vault.seal(PURPOSE, RECORD, encode(keeper.list));
        if (!sealed.ok || !await writeFile(PATTERN_FILE, sealed.data)) {
            keeper.problem = sealed.ok ? 'failed' : sealed.why;
            // What was learnt is kept in memory, and said once, with Keep it
            if (!keeper.saidKeep)
                saySomething('saidKeep');
            tell();
            return false;
        }
        keeper.onDisk = true;
        keeper.problem = null;
        tell();
        return true;
    });
}

// An unplug: what the night taught goes into the pattern, sealed at once when the Vault is open (the first night too), else kept in memory with Keep it;
// and the hold, if there is one, comes off
async function unplugged() {
    const now = Date.now();
    keeper.skip = false;
    await release();
    keeper.leaveAt = null;
    if (keeper.pluggedSince !== null) {
        // With no sealed pattern yet there is nothing to open first: what is learnt is the pattern, and is sealed now if it can be
        const adopt = !keeper.loaded && !keeper.onDisk;
        const before = keeper.loaded || adopt ? [...keeper.list, ...keeper.fresh] : keeper.fresh;
        const after = learn(before, now, keeper.pluggedSince);
        if (after !== before) {
            if (keeper.loaded || adopt) {
                keeper.list = after;
                keeper.fresh = [];
                keeper.loaded = true;
                sealList();
            } else {
                keeper.fresh = after;
            }
        }
    }
    keeper.pluggedSince = null;
    tell();
}

function plugged() {
    keeper.pluggedSince = Date.now();
    keeper.skip = false;
    decide();
}

function changed() {
    const backend = keeper.backend;
    if (!backend?.supported)
        return;
    const onBattery = backend.onBattery;
    const before = keeper.onBattery;
    keeper.onBattery = onBattery;
    if (before === null) {
        if (!onBattery)
            decide();
    } else if (before !== onBattery) {
        if (onBattery)
            unplugged();
        else
            plugged();
    } else {
        // A release UPower refused is tried again at any change; the view is told
        if (keeper.stuck)
            release();
        tell();
    }
}

// Going to sleep or shutting down: the limit comes off first, then the computer is let go. Waking: decided again against the clock.
function goingDown(signal) {
    return async (connection, sender, path, iface, name, parameters) => {
        const [on] = parameters.deepUnpack();
        if (name === 'PrepareForSleep' && !on) {
            keeper.onBattery = keeper.backend?.onBattery ?? null;
            if (keeper.onBattery === false)
                decide();
            return;
        }
        if (!on)
            return;
        await release();
        dropInhibitor();
    };
}

async function begin(backend) {
    keeper.backend = backend;
    keeper.started = true;
    const onDisk = await fileExists(PATTERN_FILE);
    // Stopped while that was looked up: nothing is started for a part that is gone
    if (keeper.backend !== backend)
        return;
    keeper.onDisk = onDisk;
    backend.start(changed, async supported => {
        if (keeper.backend !== backend)
            return;
        keeper.ready = true;
        keeper.supported = supported;
        if (!supported) {
            tell();
            return;
        }
        // A hold the last session could not take off: a restart means charge fully. The marker goes only once the limit is off.
        if (await fileExists(MARKER)) {
            if (!backend.thresholdOn || await backend.setThreshold(false))
                await removeFile(MARKER);
            else
                keeper.stuck = true;
            // Ours, not the person's, whatever UPower says of it now
            keeper.personal = false;
        }
        if (keeper.backend !== backend)
            return;
        const bus = Gio.DBus.system;
        keeper.subs = [
            bus.signal_subscribe(LOGIND, LOGIND_MANAGER, 'PrepareForSleep', LOGIND_PATH, null, Gio.DBusSignalFlags.NONE, goingDown()),
            bus.signal_subscribe(LOGIND, LOGIND_MANAGER, 'PrepareForShutdown', LOGIND_PATH, null, Gio.DBusSignalFlags.NONE, goingDown()),
        ];
        keeper.onBattery = backend.onBattery;
        if (!keeper.onBattery)
            decide();
        tell();
    });
}

// Everything given back: the hold taken off, the timer, the proxies. The keeper is reset at once, before anything is waited for, so that a part
// turned on again straight away starts clean; a limit UPower will not take off keeps its marker for the next start.
async function end() {
    const {backend, hold} = keeper;
    disarm();
    dropInhibitor();
    for (const id of keeper.subs ?? [])
        Gio.DBus.system.signal_unsubscribe(id);
    Object.assign(keeper, {
        started: false, backend: null, ready: false, supported: false, onBattery: null, pluggedSince: null, subs: null,
        list: [], fresh: [], loaded: false, onDisk: false, problem: null, saidSealed: false, saidKeep: false, saidUntil: 0,
        leaveAt: null, skip: false, refused: false, hold: false, stuck: false, personal: null,
    });
    if (hold) {
        if (await backend?.setThreshold(false))
            await removeFile(MARKER);
    }
    backend?.stop();
}

export class ChargeBy {
    constructor(ext, backend = null) {
        this._ext = ext;
        this._backend = backend;
    }

    enable() {
        keeper.owner = this;
        if (!keeper.started)
            begin(this._backend ?? new UPowerBackend());
        else
            this.changed();
    }

    disable() {
        if (keeper.owner === this)
            keeper.owner = null;
        // A lock only takes the part away: the hold, its one timer and what is learnt carry on through it
        if (!Main.sessionMode.isLocked)
            end();
    }

    changed() {
        this._ext.modules?.get('pill')?.chargeBy?.refresh();
    }

    // What the pill shows. `holding` and the times are only ever in memory; `sealed` says the pattern is on disk and not opened.
    get view() {
        const backend = keeper.backend;
        const onMains = !!backend?.supported && !backend.onBattery;
        return {
            supported: keeper.supported, onMains, holding: keeper.hold && onMains, limit: backend?.limit ?? 0,
            leaveAt: keeper.leaveAt, releaseAt: keeper.releaseAt, sealed: keeper.onDisk && !keeper.loaded,
            refused: keeper.refused, stuck: keeper.stuck, said: (keeper.saidSealed || keeper.saidKeep) && Date.now() < keeper.saidUntil,
            fresh: keeper.fresh.length,
            known: keeper.loaded ? keeper.list.length : keeper.fresh.length, problem: keeper.problem, skip: keeper.skip,
        };
    }

    // Full now, just this once: the limit off until the next unplug
    fullNow() {
        keeper.skip = true;
        keeper.leaveAt = null;
        return release().then(() => tell());
    }

    // Use my pattern: opened after a click, merged with what was seen since, and the hold decided again
    load() {
        return queued(async () => {
            if (keeper.onDisk && !keeper.loaded) {
                const bytes = await readFile(PATTERN_FILE);
                const opened = bytes ? await Vault.open(PURPOSE, RECORD, bytes) : {ok: false, why: 'failed'};
                if (!opened.ok) {
                    keeper.problem = opened.why;
                    tell();
                    return false;
                }
                const list = decode(opened.data);
                if (!list) {
                    keeper.problem = 'refused';
                    tell();
                    return false;
                }
                keeper.problem = null;
                keeper.list = merge(list, keeper.fresh);
            } else if (!keeper.loaded) {
                keeper.list = keeper.fresh;
            }
            const had = keeper.fresh.length;
            keeper.fresh = [];
            keeper.loaded = true;
            // What was learnt and not sealed yet (the Vault was closed, or not there) is sealed now: this is the person's click
            if (had || keeper.problem !== null)
                await sealList();
            await decide();
            tell();
            return true;
        });
    }

    // Keep what was learnt before it was ever sealed, after a click: opens nothing, only seals
    keepPattern() {
        return this.load();
    }

    openVault() {
        try {
            Gio.DesktopAppInfo.new('amethystora-vault.desktop')?.launch([], null);
        } catch (e) {
            console.warn(`Amethystora UI: the Vault could not be opened: ${e.message}`);
        }
    }

    // For the suites
    snapshot() {
        const {started, ready, supported, onBattery, hold, loaded, onDisk, leaveAt, releaseAt, skip, refused, problem, stuck, personal} = keeper;
        return {started, ready, supported, onBattery, hold, loaded, onDisk, leaveAt, releaseAt, skip, refused, problem, stuck, personal,
            list: keeper.list.length, fresh: keeper.fresh.length, timer: keeper.timer !== 0};
    }

    // For the suites: the keeper forgotten, as a new GNOME Shell session has it
    async resetForTest() {
        await end();
    }
}
