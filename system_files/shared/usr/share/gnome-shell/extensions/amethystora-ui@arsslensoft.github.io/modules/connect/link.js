// The module's side of the phone link: whether the program is there and the phones it knows. Everything is read from
// the program, over D-Bus; nothing is worked out here. While the program is not on the bus there is nothing: no
// proxy, no timer, no work.
//
// The name org.amethystora.Connect is free while the person's phone switch is off, so any program of the account can
// take it. The shell therefore believes an owner only when the process behind its connection runs the image's own
// program, and then talks to that connection, by its unique name, and not to whoever holds the name later. Any other
// owner is treated as if there were no phone link.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {BUS_NAME, deviceProxy, rootProxy} from './interfaces.js';

Gio._promisify(Gio.DBusConnection.prototype, 'call', 'call_finish');
Gio._promisify(Gio.File.prototype, 'query_info_async', 'query_info_finish');

// The only program that may be the phone link: what /proc/PID/exe says its process runs
export const TRUSTED_EXE = '/usr/libexec/amethystora-connect';
// Milliseconds the bus is given to say which process holds a connection
const CREDENTIALS_TIMEOUT = 5000;

const TYPES = new Set(['phone', 'tablet', 'laptop', 'desktop', 'tv']);
// Music from either side (plans/ui-ideas/spec-phone.md 3.2): what a player may be told, and how many of a phone's are read
const ACTIONS = ['Play', 'Pause', 'PlayPause', 'Next', 'Previous', 'Stop'];
const PLAYERS = 8;
// The phone as a slide clicker: the only three words the program says
const CLICKS = new Set(['next', 'previous', 'blank']);
// A name is somebody else's text: this long is all that is shown of one
const NAME_LENGTH = 48;
// Control characters, and the characters that turn text round or hide it (bidirectional marks and overrides)
const UNSEEN = /[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]/gu;
// The program has the name a moment before its object is ready, now and then: reading it is tried again this many
// times, this many seconds apart, before it is given up on until the program comes back
const ATTEMPTS = 4;
const RETRY_SECONDS = 2;

// Text from a phone as one plain line the menu can show
export function plain(text) {
    if (typeof text !== 'string')
        return '';
    const line = text.replace(UNSEEN, ' ').replace(/\s+/g, ' ').trim();
    const letters = [...line];
    return letters.length > NAME_LENGTH ? `${letters.slice(0, NAME_LENGTH - 1).join('')}…` : line;
}

function isCancelled(error) {
    return error?.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED) ?? false;
}

// The file that the process behind a bus connection runs. The bus names the process (the one that opened the
// connection) and /proc says what it runs; either failing is an error, which counts as "not the program".
async function programOf(connection, owner, cancellable) {
    const reply = await connection.call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
        'GetConnectionCredentials', new GLib.Variant('(s)', [owner]), new GLib.VariantType('(a{sv})'),
        Gio.DBusCallFlags.NONE, CREDENTIALS_TIMEOUT, cancellable);
    const [credentials] = reply.recursiveUnpack();
    const pid = credentials.ProcessID;
    if (!Number.isInteger(pid) || pid <= 0)
        throw new Error('the bus names no process for the connection');
    // The link itself, not what it points to
    const info = await Gio.File.new_for_path(`/proc/${pid}/exe`).query_info_async('standard::symlink-target',
        Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, GLib.PRIORITY_DEFAULT, cancellable);
    const target = info.get_symlink_target();
    if (!target)
        throw new Error(`process ${pid} has no program to read`);
    return target;
}

// One phone, as the program describes it now. Every reading is checked, since the program is a separate process.
export class Device {
    constructor(proxy, path) {
        this.proxy = proxy;
        this.path = path;
    }

    get id() {
        return typeof this.proxy.Id === 'string' ? this.proxy.Id : '';
    }

    get name() {
        return plain(this.proxy.Name) || this.id;
    }

    get type() {
        return TYPES.has(this.proxy.Type) ? this.proxy.Type : 'phone';
    }

    // Paired and connected: the only phones the top bar shows
    get ready() {
        return this.proxy.Paired === true && this.proxy.Connected === true && this.id !== '';
    }

    // Whether the phone's id can stand as a value on the Phone app's command line. The protocol lets an id start with a
    // hyphen, which the app would take for an option of its own: such a phone is reached through the menu's other rows
    // and in the app's window, never by an id on a command line.
    get launchable() {
        return /^[A-Za-z0-9_]/.test(this.id);
    }

    // What can be done with it now, as the program lists it
    get features() {
        return new Set(Array.isArray(this.proxy.Features) ? this.proxy.Features : []);
    }

    // The charge in percent, or -1 when the phone has not said
    get level() {
        const level = this.proxy.BatteryLevel;
        return Number.isInteger(level) && level >= 0 && level <= 100 ? level : -1;
    }

    get charging() {
        return this.proxy.BatteryCharging === true;
    }

    // The mobile signal in bars, 0 to 4, or -1 when the phone has not said
    get signal() {
        const signal = this.proxy.SignalStrength;
        return Number.isInteger(signal) && signal >= 0 && signal <= 4 ? signal : -1;
    }

    // The network that signal is on ("LTE", "5G"...), only as the short plain word the program passes on, else empty
    get network() {
        const network = this.proxy.NetworkType;
        return this.signal >= 0 && typeof network === 'string' && /^[A-Za-z0-9 .-]{1,16}$/.test(network) ? network : '';
    }

    ping() {
        return this.proxy.PingAsync();
    }

    sendUrl(url) {
        return this.proxy.SendUrlAsync(url);
    }

    // Silences the phone's ringer while a call rings (the live pill's Mute ringer)
    muteRinger() {
        return this.proxy.MuteRingerAsync();
    }

    // Rings the phone, even when it is silent, until Found it is pressed on it (the menu's Ring the phone)
    ring() {
        return this.proxy.RingAsync();
    }

    // The phone's players while music from either side is on, each checked: {player, title, artist, album, playing,
    // canPlay, canPause, canNext, canPrevious, length, position}, the times in milliseconds
    get players() {
        let list = [];
        try {
            list = this.proxy.get_cached_property('Players')?.recursiveUnpack() ?? [];
        } catch {}
        if (!Array.isArray(list))
            return [];
        const text = value => plain(typeof value === 'string' ? value : '');
        const time = value => Number.isSafeInteger(value) && value > 0 ? value : 0;
        return list.slice(0, PLAYERS).filter(p => p && typeof p.player === 'string' && text(p.player) !== '').map(p => ({
            player: p.player,
            title: text(p.title),
            artist: text(p.artist),
            album: text(p.album),
            playing: p.playing === true,
            canPlay: p.can_play === true,
            canPause: p.can_pause === true,
            canNext: p.can_next === true,
            canPrevious: p.can_previous === true,
            length: time(p.length),
            position: time(p.position),
        }));
    }

    // Tells one of the phone's players to play, pause or skip: one of the program's six words
    mediaAction(player, action) {
        if (!ACTIONS.includes(action))
            return Promise.reject(new Error(`not an action: ${action}`));
        return this.proxy.MediaActionAsync(player, action);
    }
}

export class Link {
    /**
     * @param {object} handlers
     * @param {function(): void} handlers.devices - the phones, or something about one, changed
     * @param {function(boolean): void} handlers.trust - the program is there and is the image's own, or no longer
     * @param {function(string): void} handlers.received - a phone's clipboard text, to set once
     * @param {function(Device, string, string, string): void} [handlers.call] - a phone's call: its state
     *   ("ringing", "talking", "missed", "ended"), number and name
     * @param {function(Device, string): void} [handlers.url] - a web link a ready phone shared
     * @param {function(Device, string): void} [handlers.clicker] - "next", "previous" or "blank" from a ready phone
     * @param {function(string): void} [handlers.code] - the digits of a one-time code that has just come in by text (only
     *   while the key codes is on); the shell never reads a text
     * @param {string} [trustedExe] - the program that may be the phone link, for a test to change
     */
    constructor(handlers, trustedExe = TRUSTED_EXE) {
        this._handlers = handlers;
        this._trustedExe = trustedExe;
        this._devices = new Map();
        this._pending = new Set();
        this._generation = 0;
        this._root = null;
        this._signals = [];
        this._retry = 0;
    }

    // The phones the program knows, paired and connected or not
    get devices() {
        return [...this._devices.values()];
    }

    // Gives the program this computer's new clipboard text, which it keeps for the phones that take it
    pushClipboard(text) {
        this._root?.PushClipboardAsync(text).catch(e => {
            console.debug(`Amethystora UI: the phone link did not take a copy: ${Gio.DBusError.strip_remote_error(e)}`);
        });
    }

    // Says to the program whether the shell shows the links phones share (the live pill): while it does, the program
    // sends them here rather than notifying them itself. A program from before it knew of this refuses: it notifies.
    takeLinks(take) {
        this._root?.TakeLinksAsync(take).catch(e => {
            console.debug(`Amethystora UI: the phone link did not take TakeLinks: ${Gio.DBusError.strip_remote_error(e)}`);
        });
    }

    // Says to the program whether the shell presents with the phone as a slide clicker: only while it does are the
    // phone's three keys passed on (Clicker)
    presenting(on) {
        this._root?.PresentingAsync(on).catch(e => {
            console.debug(`Amethystora UI: the phone link did not take Presenting: ${Gio.DBusError.strip_remote_error(e)}`);
        });
    }

    start() {
        this._cancellable = new Gio.Cancellable();
        // Not started if absent: the person's phone switch starts the program, and ends it
        this._watch = Gio.bus_watch_name(Gio.BusType.SESSION, BUS_NAME, Gio.BusNameWatcherFlags.NONE,
            (connection, name, owner) => this._appeared(connection, owner),
            () => this._forget());
    }

    stop() {
        if (this._watch)
            Gio.bus_unwatch_name(this._watch);
        this._watch = 0;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._forget();
    }

    // A new owner is a new program: what the last one said no longer holds, and it is looked at before it is believed
    async _appeared(connection, owner) {
        this._forget();
        const generation = this._generation;
        let program;
        try {
            program = await programOf(connection, owner, this._cancellable);
        } catch (e) {
            if (isCancelled(e) || generation !== this._generation)
                return;
            console.warn(`Amethystora UI: the owner of ${BUS_NAME} could not be identified, so it is not taken ` +
                `for the phone link: ${e.message}`);
            return;
        }
        if (generation !== this._generation)
            return;
        if (program !== this._trustedExe) {
            console.warn(`Amethystora UI: ${BUS_NAME} is held by ${JSON.stringify(program)}, which is not the phone ` +
                'link of this system, so it is ignored');
            return;
        }
        this._attach(connection, owner, ATTEMPTS);
    }

    // Drops everything read from the program and tells the module that there is nothing now
    _forget() {
        this._generation++;
        if (this._retry)
            GLib.source_remove(this._retry);
        this._retry = 0;
        const root = this._root;
        const hadDevices = this._devices.size > 0;
        this._root = null;
        this._connection = null;
        this._owner = null;
        this._pending.clear();
        for (const device of this._devices.values())
            this._drop(device);
        this._devices.clear();
        if (root) {
            for (const id of this._signals.splice(0))
                root.disconnectSignal(id);
            this._handlers.trust(false);
        }
        if (hadDevices)
            this._handlers.devices();
    }

    async _attach(connection, owner, attempts) {
        const generation = this._generation;
        try {
            const root = await rootProxy(connection, owner, this._cancellable);
            // The program left, or was replaced, while its object was being read
            if (generation !== this._generation)
                return;
            const [paths] = await root.GetDevicesAsync();
            if (generation !== this._generation)
                return;

            this._root = root;
            this._connection = connection;
            this._owner = owner;
            this._signals = [
                root.connectSignal('DeviceAdded', (proxy, sender, [path]) => this._add(path)),
                root.connectSignal('DeviceRemoved', (proxy, sender, [path]) => this._remove(path)),
                root.connectSignal('ClipboardReceived', (proxy, sender, [, text]) => this._handlers.received(text)),
                root.connectSignal('UrlReceived', (proxy, sender, [deviceId, url]) => this._url(deviceId, url)),
                root.connectSignal('Clicker', (proxy, sender, [deviceId, action]) => this._click(deviceId, action)),
                // A one-time code, the digits alone: the program looks at texts only while the key codes is on, and never sends one
                root.connectSignal('Code', (proxy, sender, [code]) => this._handlers.code?.(code)),
            ];
            this._handlers.trust(true);
            for (const path of paths)
                this._add(path);
        } catch (e) {
            if (isCancelled(e) || generation !== this._generation)
                return;
            if (attempts > 1) {
                this._retry = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, RETRY_SECONDS, () => {
                    this._retry = 0;
                    this._attach(connection, owner, attempts - 1);
                    return GLib.SOURCE_REMOVE;
                });
            } else {
                console.warn(`Amethystora UI: the phone link could not be read: ${e.message}`);
            }
        }
    }

    async _add(path) {
        if (!this._root || !GLib.Variant.is_object_path(path) || this._devices.has(path) || this._pending.has(path))
            return;
        this._pending.add(path);
        const generation = this._generation;
        try {
            const proxy = await deviceProxy(this._connection, this._owner, path, this._cancellable);
            // Gone again before it was read, or the whole program with it
            if (generation !== this._generation || !this._pending.has(path))
                return;
            proxy.connectObject('g-properties-changed', () => this._handlers.devices(), this);
            const device = new Device(proxy, path);
            device.callSignal = proxy.connectSignal('Call', (p, sender, [state, number, name]) =>
                this._handlers.call?.(device, state, number, name));
            this._syncTransfers(device);
            this._devices.set(path, device);
            this._handlers.devices();
        } catch (e) {
            if (!isCancelled(e))
                console.warn(`Amethystora UI: a phone could not be read: ${e.message}`);
        } finally {
            if (generation === this._generation)
                this._pending.delete(path);
        }
    }

    _remove(path) {
        this._pending.delete(path);
        const device = this._devices.get(path);
        if (!device)
            return;
        this._drop(device);
        this._devices.delete(path);
        this._handlers.devices();
    }

    _drop(device) {
        device.proxy.disconnectObject(this);
        if (device.callSignal)
            device.proxy.disconnectSignal(device.callSignal);
        device.callSignal = 0;
        if (device.transferSignal)
            device.proxy.disconnectSignal(device.transferSignal);
        device.transferSignal = 0;
    }

    // Hears files on their way to a phone, only while somebody asks: callback(device, id, name, incoming, done, total, state)
    // (done and total in bytes, total 0 when not said)
    watchTransfers(callback) {
        this._onTransfer = callback;
        for (const device of this._devices.values())
            this._syncTransfers(device);
    }

    _syncTransfers(device) {
        if (this._onTransfer && !device.transferSignal) {
            device.transferSignal = device.proxy.connectSignal('Transfer', (p, sender, [id, name, incoming, done, total, state]) =>
                this._onTransfer?.(device, id, name, incoming, done, total, state));
        } else if (!this._onTransfer && device.transferSignal) {
            device.proxy.disconnectSignal(device.transferSignal);
            device.transferSignal = 0;
        }
    }

    // A click of the slide clicker, from a ready phone, one of the three words
    _click(deviceId, action) {
        const device = this.devices.find(d => d.id === deviceId && d.ready);
        if (device && CLICKS.has(action))
            this._handlers.clicker?.(device, action);
    }

    // A link a phone shared. A phone whose object is still being read (one paired a moment ago) is not known yet:
    // the link is handed on all the same, with null for the phone, so that it is never lost on the way
    _url(deviceId, url) {
        if (typeof url !== 'string')
            return;
        const device = this.devices.find(d => d.id === deviceId && d.ready) ?? null;
        this.lastLink = url;
        this._handlers.url?.(device, url);
    }
}
