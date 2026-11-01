// The phone link in the shell, in place of the extension that used to carry it. The link itself, the network, the
// pairing and the files, is a program of its own that runs only while the person has turned it on (amethystora-connect,
// which owns org.amethystora.Connect on the session bus), so that a fault in it can never end the session. The shell
// has two small parts of it, which only the shell can do:
//
//  - the top bar: an icon, with the phone's battery beside it, while a paired phone is connected, and a menu to send
//    it files or a link, ping it and open its messages. The program shows a pairing request itself, as a
//    notification, so the bar adds nothing for one;
//  - the clipboard: under Wayland only the shell is told when the clipboard changes and may read it, so it passes the
//    text on to the program when the person has turned sending on, and sets it when a phone's text comes and the
//    person has turned receiving on (the settings org.amethystora.Connect, written by `ame desktop phone`).
//
// The menu also rings a phone (even a silent one), the phone's music reaches the live pill's now playing while the person
// has music from either side on (connect/players.js), and Present hears the phone's slide clicker through it, only while
// presenting with the clicker on (plans/ui-ideas/spec-phone.md).
//
// It also passes a call ringing and a link a phone shared to whoever listens (the live pill, modules/pill.js). A link
// nobody takes is shown here, as a notification with Open and Copy, so that a link is never lost and never opened by
// itself: the program raises none of its own for it while the shell is its client.
//
// While the program is not running this module only waits for its name on the bus, and it believes only the program
// of this system (link.js). Windows, the lists of phones and everything else belong to the Phone app, which the menu
// starts.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {Bridge, KEY_RECEIVE, KEY_SEND, openSettings} from './connect/clipboard.js';
import {shownAddress} from './pill/text.js';
import {Indicator} from './connect/indicator.js';
import {Link} from './connect/link.js';
import {PhonePlayers} from './connect/players.js';

const ROLE = 'amethystora-connect';
// By its full path: the session's PATH may hold a folder of the account's own before the system's
const APP = '/usr/bin/amethystora-phone';
// The shell's own indicators at the right of the top bar. The phone stands to the left of all of them.
const SYSTEM_ROLES = ['screenRecording', 'screenSharing', 'dwellClick', 'a11y', 'keyboard', 'quickSettings'];
// The longest address sent from the clipboard, in characters
const MAX_ADDRESS = 8192;
// The phone as a slide clicker, off as the image ships (plans/ui-ideas/spec-phone.md 4)
const KEY_CLICKER = 'clicker';

// The clipboard's text (or a link a phone shared) if it is one web address and nothing else, otherwise null
export function address(text) {
    const candidate = typeof text === 'string' ? text.trim() : '';
    // GLib is lenient about a space in a host name, so a line with one is refused here
    if (candidate === '' || candidate.length > MAX_ADDRESS || /[\s\p{Cc}]/u.test(candidate))
        return null;
    try {
        const uri = GLib.Uri.parse(candidate, GLib.UriFlags.NONE);
        return ['http', 'https'].includes(uri.get_scheme()) && uri.get_host() ? candidate : null;
    } catch {
        return null;
    }
}

export class Connect {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._bolt = Gio.FileIcon.new(this._ext.dir.get_child('icons').get_child('connect')
            .get_child('charging-symbolic.svg'));
        this._actions = {
            sendFiles: device => this._appFor(device, ['send', '--device', device.id]),
            sendLink: device => this._sendLink(device),
            ping: device => this._ping(device),
            ring: device => this._ring(device),
            messages: device => this._appFor(device, ['messages', device.id]),
            phones: () => this._app([]),
        };
        this._bridge = new Bridge(text => this._link?.pushClipboard(text));
        this._trusted = false;
        this._listeners = new Set();
        this._phoneListeners = new Set();
        this._clickListeners = new Set();
        this._presenting = false;
        this._toldPresenting = false;
        this._settings = openSettings();
        this._settings?.connectObject(`changed::${KEY_SEND}`, () => this._switchesChanged(),
            `changed::${KEY_RECEIVE}`, () => this._switchesChanged(), this);
        if (this._settings?.settings_schema.has_key(KEY_CLICKER))
            this._settings.connectObject(`changed::${KEY_CLICKER}`, () => this._syncPresenting(), this);
        this._link = new Link({
            devices: () => this._devicesChanged(),
            trust: trusted => {
                this._trusted = trusted;
                this._switchesChanged();
                // The shell shows the links phones share from now on (in the live pill, or here when nobody listens)
                if (trusted)
                    this._link?.takeLinks(true);
                // A new program has heard nothing of presenting yet
                this._toldPresenting = false;
                this._syncPresenting();
                // The program went: a phone that was ringing is no longer heard
                if (!trusted)
                    this._endCalls();
            },
            received: text => this._bridge?.receive(text),
            call: (device, state, number, name) => this._tell('call', device, state, number, name),
            url: (device, url) => this._url(device, url),
            clicker: (device, action) => this._clicked(device, action),
        });
        this._link.start();
    }

    disable() {
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        // The program says links itself again while the shell does not run this (the screen locked, the extension off)
        this._link?.takeLinks(false);
        // And hears no clicks: the shell is not presenting while this is off
        if (this._toldPresenting)
            this._link?.presenting(false);
        this._toldPresenting = false;
        this._presenting = false;
        this._clickListeners?.clear();
        this._clickListeners = null;
        this._phonePlayers?.sync([]);
        this._phonePlayers = null;
        // The link says, as it stops, that there are no phones and nothing to trust, which takes the rest down
        this._link?.stop();
        this._link = null;
        this._bridge?.stop();
        this._bridge = null;
        this._indicator?.destroy();
        this._indicator = null;
        this._source?.destroy();
        this._source = null;
        // Links kept in the list stay there when the shell turns the module off (as it does while the screen is
        // locked): their source is GNOME's to end once it is empty
        this._linkSource = null;
        this._listeners?.clear();
        this._listeners = null;
        this._phoneListeners?.clear();
        this._phoneListeners = null;
        this._ringing = null;
        this._actions = null;
        this._bolt = null;
    }

    // The clipboard is shared only while the program is the image's own and the person has said so
    _switchesChanged() {
        const on = key => this._trusted && this._settings?.get_boolean(key) === true;
        this._bridge?.setSwitches(on(KEY_SEND), on(KEY_RECEIVE));
    }

    // The phones that are paired and connected, by name
    _ready() {
        return this._link.devices.filter(device => device.ready)
            .sort((a, b) => a.name.localeCompare(b.name) || a.path.localeCompare(b.path));
    }

    _devicesChanged() {
        if (!this._link)
            return;
        const ready = this._ready();
        this._phonePlayers?.sync(ready);
        for (const listener of [...this._phoneListeners ?? []]) {
            try {
                listener();
            } catch (e) {
                console.error(`Amethystora UI: a listener of the phones failed: ${e.message}`);
            }
        }
        if (ready.length === 0) {
            this._indicator?.destroy();
            return;
        }
        if (!this._indicator) {
            // One left by a module that did not stop cleanly
            Main.panel.statusArea[ROLE]?.destroy();
            this._indicator = new Indicator(this._actions, this._bolt);
            this._indicator.connect('destroy', actor => {
                if (this._indicator === actor)
                    this._indicator = null;
            });
            Main.panel.addToStatusArea(ROLE, this._indicator, this._position(), 'right');
            // The tip (the hints module), as a paired phone connects: the rules make it the first time only
            const text = _('Tip: drop a file on Phone in the dock to send it to your phone.');
            this._ext.modules?.get('hints')?.offer('phone-drop', {text, anchor: this._indicator});
        }
        this._indicator.update(ready);
    }

    // Left of the shell's own indicators, which are the last of the right side of the bar
    _position() {
        const containers = SYSTEM_ROLES.map(role => Main.panel.statusArea[role]?.container)
            .filter(container => container?.get_parent());
        if (containers.length === 0)
            return 0;
        const siblings = containers[0].get_parent().get_children();
        return Math.min(...containers.map(container => siblings.indexOf(container)));
    }

    // The app, started as the person's own program with the arguments it takes
    _app(args) {
        Util.spawn([APP, ...args]);
    }

    // The app for one phone, only when the phone's id is safe to pass on its command line
    _appFor(device, args) {
        if (device.launchable)
            this._app(args);
    }

    _ping(device) {
        device.ping().catch(e => this._failed(device, e));
    }

    // One request; a second one, by the protocol's own rule, stops the ringing
    _ring(device) {
        device.ring().then(() => this._say(_('Ringing %s').format(device.name), _('Until Found it is pressed on the phone.')))
            .catch(e => this._failed(device, e));
    }

    // The address in the clipboard, if it is one. The clipboard is read here, for this request only. Without an
    // address the app's own dialog opens, where one can be typed.
    _sendLink(device) {
        St.Clipboard.get_default().get_text(St.ClipboardType.CLIPBOARD, (clipboard, text) => {
            if (!this._link)
                return;
            const url = address(text);
            if (url)
                device.sendUrl(url).catch(e => this._failed(device, e));
            else
                this._appFor(device, ['send', '--device', device.id]);
        });
    }

    // --- For the phone's tile in quick settings ---------------------------------------------------------------------

    // The phones that are paired and connected, by name: what the tile says of each
    phones() {
        if (!this._link)
            return [];
        return this._ready().map(device => ({name: device.name, level: device.level, charging: device.charging}));
    }

    // Whether the phone link program is there and is the image's own. Walk-away lock (modules/privacy/away.js) tells a
    // phone that left from the program that stopped: the link says the program went before it lets the phones go.
    get linked() {
        return this._trusted === true;
    }

    // Calls back whenever the phones, or something about one, changed; returns what stops it
    onPhonesChanged(callback) {
        this._phoneListeners?.add(callback);
        return () => this._phoneListeners?.delete(callback);
    }

    // --- Music from either side and the clicker (plans/ui-ideas/spec-phone.md 3.2, 4) -------------------------------

    // The phone's players, as a list the pill's now playing follows beside GNOME's own: made when first asked for
    get mediaSource() {
        if (!this._link)
            return null;
        if (!this._phonePlayers) {
            this._phonePlayers = new PhonePlayers();
            this._phonePlayers.sync(this._ready());
        }
        return this._phonePlayers;
    }

    // Present says whether it is presenting; the program is told only while the clicker is on too
    setPresenting(on) {
        this._presenting = on === true;
        this._syncPresenting();
    }

    // Whether the clicker can click now: the switch on and the program the image's own
    get clicker() {
        return this._trusted === true && this._settings?.settings_schema.has_key(KEY_CLICKER) === true &&
            this._settings.get_boolean(KEY_CLICKER);
    }

    _syncPresenting() {
        const want = this._presenting && this.clicker;
        if (want === this._toldPresenting || !this._link)
            return;
        this._toldPresenting = want;
        this._link.presenting(want);
    }

    // callback(device, action) for each click while presenting; returns what stops it
    onClicker(callback) {
        this._clickListeners?.add(callback);
        return () => this._clickListeners?.delete(callback);
    }

    _clicked(device, action) {
        if (!this._toldPresenting)
            return;
        for (const listener of [...this._clickListeners ?? []]) {
            try {
                listener(device, action);
            } catch (e) {
                console.error(`Amethystora UI: a listener of the clicker failed: ${e.message}`);
            }
        }
    }

    // --- For the live pill ----------------------------------------------------------------------------------------

    // listener: {call(device, state, number, name), url(device, url)}; url returns true when it took the link
    addListener(listener) {
        this._listeners?.add(listener);
    }

    removeListener(listener) {
        this._listeners?.delete(listener);
    }

    // The Phone app's texts with this phone
    messages(device) {
        this._appFor(device, ['messages', device.id]);
    }

    // The address again, as the program sent it and as Open needs it: null when it is not one web address
    checkAddress(url) {
        return address(url);
    }

    _tell(kind, ...args) {
        if (kind === 'call') {
            const [device, state] = args;
            this._ringing ??= new Set();
            if (state === 'ringing')
                this._ringing.add(device);
            else
                this._ringing.delete(device);
        }
        let taken = false;
        for (const listener of [...this._listeners ?? []]) {
            try {
                taken = listener[kind]?.(...args) === true || taken;
            } catch (e) {
                console.error(`Amethystora UI: a listener of the phone link failed: ${e.message}`);
            }
        }
        return taken;
    }

    _endCalls() {
        for (const device of [...this._ringing ?? []])
            this._tell('call', device, 'ended', '', '');
        this._ringing?.clear();
    }

    // A link from a ready phone, checked again: to whoever listens, or else into the list
    // A link a phone shared, which the program sent here rather than notify it: shown in the pill, or kept here; one
    // that is not a web address the module shows (or from a phone not read yet) is still said, as text, never dropped
    _url(device, url) {
        if (!this._link)
            return;
        const known = device?.ready && this._ready().includes(device) ? device : null;
        if (!address(url)) {
            this._linkAsText(known, url);
            return;
        }
        if (!known || !this._tell('url', known, url))
            this.keepLink(known, url, false);
    }

    _linkSourceFor() {
        if (!this._linkSource) {
            this._linkSource = new MessageTray.Source({title: _('Phones'), iconName: 'smartphone-symbolic'});
            this._linkSource.connect('destroy', () => (this._linkSource = null));
            Main.messageTray.add(this._linkSource);
        }
        return this._linkSource;
    }

    // What a phone shared that is no web address to open: its words only, with no button that opens anything
    _linkAsText(device, url) {
        if (typeof url !== 'string' || !this._listeners)
            return;
        const source = this._linkSourceFor();
        source.addNotification(new MessageTray.Notification({
            source,
            title: _('%s sent a link').format(device?.name || _('Your phone')),
            body: [...url.replace(/[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]/gu, ' ')].slice(0, 300).join(''),
            isTransient: false,
        }));
    }

    // The link as a notification of its own, from Phones, with Open and Copy; quietly, it goes to the list without a
    // banner. Open checks the address again; nothing opens it by itself.
    keepLink(device, url, quietly) {
        const checked = address(url);
        if (!checked || !this._listeners)
            return;
        // A host the ASCII form cannot be made of is said as text, with nothing to open
        const shown = shownAddress(checked, true);
        if (!shown) {
            this._linkAsText(device, url);
            return;
        }
        const source = this._linkSourceFor();
        // Shown as the pill shows it: the host in its ASCII form, nothing invisible, so a look-alike cannot pass for
        // another site; what a click on it would open is that form, never the address as the phone wrote it
        const notification = new MessageTray.Notification({
            source,
            title: _('%s sent a link').format(device?.name || _('Your phone')),
            body: shown,
            isTransient: false,
            urgency: quietly ? MessageTray.Urgency.LOW : MessageTray.Urgency.NORMAL,
        });
        notification.addAction(_('Open'), () => {
            const again = address(checked);
            if (again)
                Gio.AppInfo.launch_default_for_uri(again, global.create_app_launch_context(0, -1));
        });
        notification.addAction(_('Copy'), () => St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, checked));
        source.addNotification(notification);
    }

    _failed(device, error) {
        console.debug(`Amethystora UI: the phone link refused a request: ${Gio.DBusError.strip_remote_error(error)}`);
        this._say(_('Could not reach the phone'), device.name);
    }

    // Said by the phone link, in passing: it does not stay in the list
    _say(title, body) {
        if (!this._source) {
            this._source = new MessageTray.Source({title: _('Phones'), iconName: 'smartphone-symbolic'});
            this._source.connect('destroy', () => (this._source = null));
            Main.messageTray.add(this._source);
        }
        this._source.addNotification(new MessageTray.Notification({
            source: this._source,
            title,
            body,
            isTransient: true,
        }));
    }
}
