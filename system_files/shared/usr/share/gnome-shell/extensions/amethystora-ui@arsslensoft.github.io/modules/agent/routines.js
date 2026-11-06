// Little routines (R7-15): "when I join the office Wi-Fi, open Slack and mute". A routine is one readable file the agent writes and
// the person keeps, sealed in the Vault, checked and run by `amethystora-agent-tools routines` (apps/agent-tools/src/routines.rs).
// This is the listening half: it asks the helper which routines there are, connects only the sources they name (a timer for the next
// time, NetworkManager, UPower, the app system, the phone link), and when one of them wakes a routine it asks the helper to fire it.
// Each run shows in the pill once, with Undo and Pause. A routine is never code in the shell: what runs is `ame` as an argument vector
// and a launcher's id, in the helper, as the account.
//
// Off, and with no routine made, it costs a folder's file monitor: there is no sealed file, so neither the Vault nor the helper is
// asked. The Vault is never prompted: a closed vault with no background key makes the routines wait, which a notice says once.
//
// A screen lock stops this extension, so while the screen is locked nothing here runs: a trigger that is due then is missed, and a
// routine for "when the screen locks" (which has to say `while_locked`) is fired as the lock stops it. An unlock is the next start.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {buttonRow, cardButton, retire} from '../pill/clock.js';
import {plain} from '../pill/text.js';
import {SHOWN, actionsRan, nextDue, parseRan, parseTriggers, routinesFor, sourcesNeeded, stateOf} from './routinesmodel.js';

const TOOLS = '/usr/libexec/amethystora-agent-tools';
const VAULT_APP = 'amethystora-vault';
const FOLDER = GLib.build_filenamev([GLib.get_user_data_dir(), 'amethystora', 'routines']);
const SEALED = GLib.build_filenamev([FOLDER, 'routines.avs']);

Gio._promisify(Gio.File.prototype, 'query_info_async');
Gio._promisify(Gio.Subprocess.prototype, 'communicate_utf8_async');

// What must outlive a lock (it stops the extension): that the stop was one, so that the next start is an unlock, and that the notice
// about a closed vault has been said
let stoppedByLock = false;
let saidClosed = false;
// The routines said once to have been missed because the Vault was closed
const saidMissed = new Set();

// What the helper printed for `args`, or '' when it could not run
async function helper(args, cancellable = null) {
    try {
        const process = Gio.Subprocess.new([TOOLS, 'routines', ...args], Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
        const [stdout] = await process.communicate_utf8_async(null, cancellable);
        return stdout ?? '';
    } catch {
        return '';
    }
}

// A run, in the pill and its card
class RoutineRan {
    constructor(owner, ran, started) {
        this._owner = owner;
        this.id = 'routine';
        this.ran = ran;
        // The apps it opened that were not running before: what Undo may close
        this._started = started;
        this.plate = ran.failed ? 'agent-ask' : 'agent-done';
        this._state = '';
    }

    get text() {
        return this.ran.failed ? _('%s stopped').format(this.ran.name) : _('%s ran').format(this.ran.name);
    }

    get accessible() {
        return this.text;
    }

    get row() {
        return {icon: this._owner.gicon, text: this.text};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-agent-pill'});
            this._content.add_child(new St.Icon({gicon: this._owner.gicon, icon_size: 13, y_align: Clutter.ActorAlign.CENTER}));
            this._label = new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER});
            this._label.clutter_text.use_markup = false;
            this._content.add_child(this._label);
        }
        return this._content;
    }

    destroy() {
        this._owner.pill?.clock?.forget(this._content);
        retire(this._content);
        this._content = null;
        this._label = null;
        this._body = null;
    }

    _words(result) {
        if (result.kind === 'open') {
            const app = Shell.AppSystem.get_default().lookup_app(result.what);
            return _('Opened %s').format(plain(app?.get_name() ?? result.what, 60));
        }
        return _('Ran %s').format(plain(result.what, 80));
    }

    card(api) {
        this._api = api;
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body amethystora-agent-card', orientation: Clutter.Orientation.VERTICAL});
        this._body = body;
        body.connect('destroy', () => {
            if (this._body === body)
                this._body = null;
        });
        this._fill();
        return body;
    }

    _fill() {
        const body = this._body;
        if (!body)
            return;
        body.destroy_all_children();
        const head = new St.BoxLayout({style_class: 'amethystora-agent-card-head'});
        head.add_child(new St.Label({style_class: 'amethystora-agent-card-title', text: this.text, x_expand: true, y_align: Clutter.ActorAlign.END}));
        body.add_child(head);
        for (const result of this.ran.results) {
            const row = new St.BoxLayout({style_class: 'amethystora-agent-act', x_expand: true});
            const what = new St.Label({style_class: 'amethystora-agent-act-what', text: this._words(result), x_expand: true});
            what.clutter_text.use_markup = false;
            row.add_child(what);
            row.add_child(new St.Label({style_class: 'amethystora-agent-act-state', text: result.ok ? _('Done') : _('Failed'), y_align: Clutter.ActorAlign.CENTER}));
            body.add_child(row);
        }
        if (this.ran.failed) {
            const failed = new St.Label({style_class: 'amethystora-agent-act-how amethystora-agent-note', text: _('It is paused until you start it again: %s').format(plain(this.ran.failed, 100))});
            failed.clutter_text.use_markup = false;
            failed.clutter_text.line_wrap = true;
            failed.clutter_text.line_wrap_mode = Pango.WrapMode.WORD_CHAR;
            failed.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
            body.add_child(failed);
        }
        const buttons = [];
        if (this._state === '') {
            if (!this.ran.failed)
                buttons.push(cardButton(_('Pause this routine'), () => this._pause(), {small: true}));
            if (this.ran.undoable || this._started.length > 0)
                buttons.push(cardButton(_('Undo'), () => this._undo(), {small: true, primary: true}));
        } else {
            body.add_child(new St.Label({style_class: 'amethystora-agent-act-how amethystora-agent-note', text: this._state}));
        }
        if (buttons.length) {
            const row = buttonRow(buttons);
            row.add_style_class_name('amethystora-agent-card-buttons');
            body.add_child(row);
        }
    }

    async _pause() {
        this._state = _('Paused. ame agent routines resume %s starts it again.').format(this.ran.id);
        this._fill();
        await helper(['pause', this.ran.id]);
        this._owner.reload();
    }

    // The way back each ame action says, newest first (the helper), and the apps this run opened and nothing else had
    async _undo() {
        this._state = _('Undoing…');
        this._fill();
        if (this.ran.undoable)
            await helper(['undo', this.ran.id, '--ran', String(actionsRan(this.ran)), '--json']);
        const running = Shell.AppSystem.get_default();
        for (const id of this._started)
            running.lookup_app(id)?.request_quit();
        this._state = _('Undone');
        this._fill();
    }
}

export class Routines {
    // owner: the agent module (`pill`, `gicon`, `_ext`)
    constructor(owner) {
        this._owner = owner;
    }

    get pill() {
        return this._owner.pill;
    }

    get gicon() {
        return this._owner.gicon;
    }

    start() {
        this._cancellable = new Gio.Cancellable();
        this._routines = [];
        this._last = new Map();
        this._sources = new Map();
        this._chain = Promise.resolve();
        // Nothing is made and nothing is watched until a sealed file exists: reload() looks for it, and `ame agent routines add` says
        // when it was made. Back from a lock, an unlock is an event of its own, once the routines are known
        const unlocked = stoppedByLock;
        stoppedByLock = false;
        this.reload().then(() => {
            if (unlocked && this._cancellable)
                this._event({kind: 'screen', state: 'unlocks'});
        });
    }

    stop() {
        // A lock stops the extension: a routine for the screen locking runs now, as the lock begins
        if (Main.sessionMode?.isLocked && this._routines?.length) {
            stoppedByLock = true;
            this._event({kind: 'screen', state: 'locks'});
        }
        this._cancellable?.cancel();
        this._cancellable = null;
        this._unwatch();
        this._closed = false;
        if (this._wait)
            GLib.source_remove(this._wait);
        this._wait = 0;
        this._closeSources(new Set());
        this._routines = [];
        this._dismiss();
    }

    // The sealed file changed (a routine kept, paused or removed): once, a moment after the last change
    _later() {
        if (this._wait)
            return;
        this._wait = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 400, () => {
            this._wait = 0;
            this.reload();
            return GLib.SOURCE_REMOVE;
        });
    }

    // The folder is watched only while the sealed file is there
    _watch() {
        if (this._monitor || !this._cancellable)
            return;
        try {
            this._monitor = Gio.File.new_for_path(FOLDER).monitor_directory(Gio.FileMonitorFlags.NONE, this._cancellable);
            this._monitor.connect('changed', () => this._later());
        } catch (e) {
            console.warn(`Amethystora UI: the routines cannot be watched: ${e.message}`);
        }
    }

    _unwatch() {
        this._monitor?.cancel();
        this._monitor = null;
        this._unwait();
    }

    // Which routines there are: nothing to ask, make or watch when there is no sealed file, else the helper, which opens the Vault with
    // its background key and never prompts. A Vault that is closed (and has no such key) leaves nothing to connect: the routines are
    // looked for again while the Vault's own window is open, which is the one way it gets opened
    async reload() {
        const cancellable = this._cancellable;
        if (!cancellable)
            return;
        const serial = this._serial = (this._serial ?? 0) + 1;
        let exists = false;
        try {
            await Gio.File.new_for_path(SEALED).query_info_async('standard::name', Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_DEFAULT, cancellable);
            exists = true;
        } catch {}
        if (cancellable !== this._cancellable || serial !== this._serial)
            return;
        let read = {state: 'none', routines: []};
        if (exists) {
            this._watch();
            read = parseTriggers(await helper(['triggers'], cancellable));
            if (cancellable !== this._cancellable || serial !== this._serial)
                return;
        } else {
            this._unwatch();
        }
        this._routines = read.routines;
        this._closed = read.state === 'closed';
        if (this._closed) {
            this._sayClosed(_('Your routines wait for the Vault'));
            this._await();
        } else {
            this._unwait();
        }
        this._closeSources(sourcesNeeded(this._routines));
        this._openSources(sourcesNeeded(this._routines));
        this._arm();
    }

    // Said once for a session, with the way to open the Vault, never asked for
    _sayClosed(words) {
        if (saidClosed)
            return;
        saidClosed = true;
        this.pill?.notice?.(words, 'dialog-password-symbolic', 8, '', null, {label: _('Open Vault'), run: () => Util.spawn([VAULT_APP])});
    }

    // While the Vault is closed and its own window is open, the helper is asked again every so often: the person is opening it, and
    // nothing else tells the desktop that they have
    _await() {
        if (this._awaiting) {
            this._look?.();
            return;
        }
        const system = Shell.AppSystem.get_default();
        const tag = this._awaiting = {};
        const look = () => {
            const open = system.lookup_app(`${VAULT_APP}.desktop`)?.state === Shell.AppState.RUNNING;
            if (open && !this._retry) {
                this._retry = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 15, () => {
                    this._retry = 0;
                    this.reload();
                    return GLib.SOURCE_REMOVE;
                });
            }
        };
        this._look = look;
        system.connectObject('app-state-changed', look, tag);
        look();
    }

    _unwait() {
        if (this._awaiting)
            Shell.AppSystem.get_default().disconnectObject(this._awaiting);
        this._awaiting = null;
        this._look = null;
        if (this._retry)
            GLib.source_remove(this._retry);
        this._retry = 0;
    }

    // --- The sources: each connected only while a routine names it ---------------------------------------------------------

    _closeSources(keep) {
        for (const [kind, close] of [...this._sources]) {
            if (!keep.has(kind)) {
                close();
                this._sources.delete(kind);
            }
        }
        if (!keep.has('time') && this._timer) {
            GLib.source_remove(this._timer);
            this._timer = 0;
        }
    }

    _openSources(need) {
        const open = {network: () => this._watchNetwork(), battery: () => this._watchBattery(), app: () => this._watchApps(), phone: () => this._watchPhones()};
        for (const kind of need) {
            if (open[kind] && !this._sources.has(kind))
                this._sources.set(kind, open[kind]());
        }
    }

    // Time: one timer, to the next due moment of any routine, set again after each
    _arm() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        const due = nextDue(this._routines, new Date());
        if (!due)
            return;
        const seconds = Math.max(1, Math.ceil((due.at.getTime() - Date.now()) / 1000));
        this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, seconds, () => {
            this._timer = 0;
            // The clock may be a little early after a coarse wakeup: not yet due is waited for again
            if (due.at.getTime() - Date.now() > 2000) {
                this._arm();
                return GLib.SOURCE_REMOVE;
            }
            for (const routine of this._routines.filter(r => due.ids.includes(r.id)))
                this._fire(routine);
            this._arm();
            return GLib.SOURCE_REMOVE;
        });
    }

    // Let go of a proxy's signal now, rather than leaving it to the collector (the extension starts again at every unlock)
    _closeBus(holder) {
        holder.closed = true;
        if (holder.proxy && holder.handler)
            holder.proxy.disconnect(holder.handler);
        holder.proxy = null;
    }

    _bus(name, path, iface, changed) {
        const cancellable = this._cancellable;
        const holder = {proxy: null};
        Gio.DBusProxy.new_for_bus(Gio.BusType.SYSTEM, Gio.DBusProxyFlags.NONE, null, name, path, iface, cancellable, (source, result) => {
            try {
                holder.proxy = Gio.DBusProxy.new_for_bus_finish(result);
            } catch {
                return;
            }
            if (cancellable !== this._cancellable || holder.closed) {
                holder.proxy = null;
                return;
            }
            holder.handler = holder.proxy.connect('g-properties-changed', (proxy, props) => changed(proxy, props));
            changed(holder.proxy, null);
        });
        return holder;
    }

    // The connection NetworkManager says the machine is on: its name is the one Network settings shows
    _watchNetwork() {
        let current = null;
        const holder = this._bus('org.freedesktop.NetworkManager', '/org/freedesktop/NetworkManager', 'org.freedesktop.NetworkManager', async (proxy, props) => {
            // Any other property of NetworkManager changing is not a different connection
            if (props && !props.lookup_value('PrimaryConnection', null))
                return;
            const path = proxy.get_cached_property('PrimaryConnection')?.deepUnpack() ?? '/';
            let name = '';
            if (path !== '/') {
                try {
                    const active = await new Promise((resolve, reject) => Gio.DBusProxy.new_for_bus(Gio.BusType.SYSTEM, Gio.DBusProxyFlags.NONE, null,
                        'org.freedesktop.NetworkManager', path, 'org.freedesktop.NetworkManager.Connection.Active', this._cancellable,
                        (s, r) => {
                            try {
                                resolve(Gio.DBusProxy.new_for_bus_finish(r));
                            } catch (e) {
                                reject(e);
                            }
                        }));
                    name = active.get_cached_property('Id')?.deepUnpack() ?? '';
                } catch {}
            }
            // The first reading is where it stands, not a join
            const first = current === null;
            if (name === current)
                return;
            current = name;
            if (!first && name)
                this._event({kind: 'network', name});
        });
        return () => this._closeBus(holder);
    }

    // Whether the machine runs on its battery and how full it is, from UPower
    _watchBattery() {
        let was = null;
        const read = proxies => {
            const onBattery = proxies.power.get_cached_property('OnBattery')?.deepUnpack() === true;
            const percent = proxies.display?.get_cached_property('Percentage')?.deepUnpack() ?? 100;
            const now = {onBattery, percent};
            if (was)
                this._event({kind: 'battery', ...now, was});
            was = now;
        };
        const proxies = {};
        const power = this._bus('org.freedesktop.UPower', '/org/freedesktop/UPower', 'org.freedesktop.UPower', (proxy, props) => {
            proxies.power = proxy;
            if (props && proxies.display)
                read(proxies);
            else if (!props && proxies.display)
                was = {onBattery: proxy.get_cached_property('OnBattery')?.deepUnpack() === true, percent: proxies.display.get_cached_property('Percentage')?.deepUnpack() ?? 100};
        });
        const display = this._bus('org.freedesktop.UPower', '/org/freedesktop/UPower/devices/DisplayDevice', 'org.freedesktop.UPower.Device', (proxy, props) => {
            proxies.display = proxy;
            if (props && proxies.power)
                read(proxies);
            else if (!props && proxies.power)
                was = {onBattery: proxies.power.get_cached_property('OnBattery')?.deepUnpack() === true, percent: proxy.get_cached_property('Percentage')?.deepUnpack() ?? 100};
        });
        return () => {
            this._closeBus(power);
            this._closeBus(display);
        };
    }

    // An app starting and stopping, and the focused one going fullscreen
    _watchApps() {
        const system = Shell.AppSystem.get_default();
        const tag = {};
        system.connectObject('app-state-changed', (s, app) => {
            if (app.state === Shell.AppState.RUNNING)
                this._event({kind: 'app', id: app.get_id(), state: 'opens'});
            else if (app.state === Shell.AppState.STOPPED)
                this._event({kind: 'app', id: app.get_id(), state: 'closes'});
        }, tag);
        global.display.connectObject('in-fullscreen-changed', () => {
            if (!Main.layoutManager.monitors.some(monitor => monitor.inFullscreen))
                return;
            const window = global.display.focus_window;
            const app = window ? Shell.WindowTracker.get_default().get_window_app(window) : null;
            if (app)
                this._event({kind: 'app', id: app.get_id(), state: 'fullscreen'});
        }, tag);
        return () => {
            system.disconnectObject(tag);
            global.display.disconnectObject(tag);
        };
    }

    // Paired phones that are connected, from the phone link's module
    _watchPhones() {
        const connect = this._owner._ext?.modules?.get('connect');
        if (!connect?.onPhonesChanged)
            return () => {};
        let known = new Set(connect.phones().map(p => p.name));
        const unlisten = connect.onPhonesChanged(() => {
            const now = new Set(connect.phones().map(p => p.name));
            for (const name of now) {
                if (!known.has(name))
                    this._event({kind: 'phone', state: 'near', name});
            }
            for (const name of known) {
                if (!now.has(name))
                    this._event({kind: 'phone', state: 'away', name});
            }
            known = now;
        });
        return () => unlisten?.();
    }

    // --- Firing -----------------------------------------------------------------------------------------------------------

    _event(event) {
        const at = Math.floor(Date.now() / 1000);
        for (const routine of routinesFor(this._routines ?? [], event, this._last, at))
            this._fire(routine, at);
    }


    // One routine, by the helper: it checks again, runs each action as the account and stops at the first that fails. One after the
    // other, since each keeps the whole store's state
    _fire(routine, at = Math.floor(Date.now() / 1000)) {
        this._last.set(routine.id, at);
        this._chain = this._chain.then(() => this._fireNow(routine)).catch(() => {});
    }

    async _fireNow(routine) {
        const cancellable = this._cancellable;
        if (!cancellable)
            return;
        const before = new Set(Shell.AppSystem.get_default().get_running().map(app => app.get_id()));
        const text = await helper(['fire', routine.id, '--json'], cancellable);
        if (cancellable !== this._cancellable)
            return;
        // The Vault closed since the routines were read: this run is missed, said once, and the routines are looked for again
        if (stateOf(text) === 'closed') {
            if (!saidMissed.has(routine.id)) {
                saidMissed.add(routine.id);
                this.pill?.notice?.(_('%s could not run: the Vault was closed').format(plain(routine.name, 40)), 'dialog-password-symbolic', 8, '', null,
                    {label: _('Open Vault'), run: () => Util.spawn([VAULT_APP])});
            }
            this.reload();
            return;
        }
        const ran = parseRan(text);
        if (!ran)
            return;
        // What it opened that was not open before: the only apps Undo closes
        const started = ran.results.filter(r => r.kind === 'open' && r.ok && !before.has(r.what) && !before.has(`${r.what}.desktop`)).map(r => r.what);
        this._show(new RoutineRan(this, ran, started));
        if (ran.failed)
            this.reload();
    }

    _show(activity) {
        this._dismiss();
        this._shown = activity;
        this.pill?.setRoutine?.(activity);
        this._gone = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, SHOWN, () => {
            this._gone = 0;
            this._dismiss();
            return GLib.SOURCE_REMOVE;
        });
    }

    _dismiss() {
        if (this._gone)
            GLib.source_remove(this._gone);
        this._gone = 0;
        const shown = this._shown;
        this._shown = null;
        if (shown) {
            this.pill?.setRoutine?.(null);
            shown.destroy();
        }
    }

    // For the suites
    state() {
        return {routines: (this._routines ?? []).map(r => ({id: r.id, paused: r.paused})), sources: [...(this._sources?.keys() ?? [])],
            timer: !!this._timer, shown: this._shown?.text ?? null};
    }
}
