// Present mode, with Scribble (board 38, R4-02; offered and off). One switch, in quick settings or by itself when a
// screen is plugged in (it asks first): while presenting, the screen stays awake, banners wait (the pill's hold, with a
// reason of its own), the pill says "Presenting" and for how long, and the pointer is easy to follow: a halo or a
// torch (modules/present/mark.js). It ends by hand, or with the screen it was started
// for, or with the share or recording that was running when it started. Super+Alt+D draws on the screen while Present
// mode is offered, presenting or not (modules/present/scribble.js). While presenting, a paired phone may be a slide clicker:
// the phone link passes on its Next, Previous and Blank only while the person's switch `clicker` of org.amethystora.Connect
// is on, and they are pressed here as Page Down, Page Up and B (plans/ui-ideas/spec-phone.md 4).
//
// The keys are org.gnome.shell.extensions.amethystora-ui.present: enabled (offered), pointer-mark, ask-on-screen and
// scribble (the keybinding), written by `ame desktop present`, Control and the setup file. Presenting itself is the
// moment's state and is never written. Off, nothing is connected but the settings.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';
import {QuickToggle, SystemIndicator} from 'resource:///org/gnome/shell/ui/quickSettings.js';

import {Capture} from './pill/capture.js';
import {buttonRow, cardButton, retire} from './pill/clock.js';
import {PointerMark} from './present/mark.js';
import {Scribble} from './present/scribble.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.present';
const MARKS = ['halo', 'torch', 'none'];
// gnome-session's flag for "the session is not idle": no blanking, no lock, no suspend for being idle
const INHIBIT_IDLE = 8;
const ICON = 'video-display-symbolic';
// The phone as a slide clicker (plans/ui-ideas/spec-phone.md 4): the three words the phone link says, as keys
const CLICKER_KEYS = {next: Clutter.KEY_Page_Down, previous: Clutter.KEY_Page_Up, blank: Clutter.KEY_b};

// Across a screen lock, when GNOME stops the extension, presenting waits here to start again
let waiting = null;
// The screens already seen this session, asked about at most once each, a lock and unlock included
let seen = null;

function markName(mark) {
    return {halo: _('Halo'), torch: _('Torch'), none: _('No mark')}[mark] ?? _('Halo');
}

// m:ss, or h:mm:ss past an hour
function elapsed(seconds) {
    const s = Math.max(0, Math.floor(seconds));
    const pad = n => String(n).padStart(2, '0');
    const h = Math.floor(s / 3600);
    const m = Math.floor(s / 60) % 60;
    return h > 0 ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`;
}

// The pill's activity while presenting: "Presenting · 12:40" on the accent's plate, and a card with Stop
class PresentMark {
    constructor(present) {
        this._present = present;
        this.id = 'present';
        this.plate = 'focus';
    }

    get text() {
        return _('Presenting · %s').format(elapsed(this._present.seconds));
    }

    get accessible() {
        return _('Presenting for %s').format(elapsed(this._present.seconds));
    }

    get row() {
        return {icon: ICON, text: this.text};
    }

    pill() {
        if (!this._content) {
            const content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-present-mark'});
            content.add_child(new St.Icon({icon_name: ICON, icon_size: 13, y_align: Clutter.ActorAlign.CENTER}));
            this._label = new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER});
            this._label.clutter_text.use_markup = false;
            content.add_child(this._label);
            this._content = content;
            content.connect('destroy', () => {
                if (this._content === content) {
                    this._content = null;
                    this._label = null;
                }
            });
        }
        return this._content;
    }

    tick() {
        if (this._label)
            this._label.text = this.text;
    }

    card(api) {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        body.add_child(new St.Label({style_class: 'amethystora-pill-card-label', text: _('Presenting')}));
        const text = new St.Label({
            style_class: 'amethystora-pill-card-text',
            text: _('The screen stays awake and banners wait until you stop. Super+Alt+D draws on the screen.'),
        });
        text.clutter_text.line_wrap = true;
        body.add_child(text);
        if (this._present.clicker) {
            const phone = new St.Label({
                style_class: 'amethystora-pill-card-text',
                text: _('Your phone can go to the next or previous slide, or blank the screen.'),
            });
            phone.clutter_text.line_wrap = true;
            body.add_child(phone);
        }
        body.add_child(buttonRow([cardButton(_('Stop'), () => {
            api.close();
            this._present.stop();
        }, {primary: true})]));
        return body;
    }

    destroy() {
        retire(this._content);
        this._content = null;
        this._label = null;
    }
}

const PresentToggle = GObject.registerClass(
class AmethystoraPresentToggle extends QuickToggle {
    _init(present) {
        super._init({title: _('Present'), iconName: ICON, toggleMode: true});
        this._amethystoraQuickId = 'present';
        this._present = present;
        this.connect('clicked', () => {
            if (this._present.presenting)
                this._present.stop();
            else
                this._present.start();
            this.sync();
        });
    }

    sync() {
        const present = this._present;
        this.checked = present.presenting;
        this.subtitle = present.presenting ? elapsed(present.seconds) : markName(present.mark);
    }
});

export class Present {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject(
            'changed::enabled', () => this._sync(),
            'changed::pointer-mark', () => this._syncMark(),
            'changed::ask-on-screen', () => {
                // Turned on: a screen plugged in while it was off is no new screen
                for (const connector of this._connectors())
                    seen?.add(connector);
                this._syncScreens();
            }, this);
        this._sync();
        // Presenting through a screen lock
        const resume = waiting;
        waiting = null;
        if (resume && this.offered)
            this.start(resume);
    }

    disable() {
        if (this._presenting && Main.sessionMode.isLocked)
            waiting = {since: this._since, connector: this._forConnector, share: this._forShare};
        this.stop(true);
        this._withdraw();
        this._scribble?.close();
        this._scribble = null;
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        this._notifications?.destroy();
        this._notifications = null;
    }

    get offered() {
        return this._settings?.get_boolean('enabled') ?? false;
    }

    get presenting() {
        return Boolean(this._presenting);
    }

    get mark() {
        const mark = this._settings?.get_string('pointer-mark');
        return MARKS.includes(mark) ? mark : 'halo';
    }

    get seconds() {
        return this._since ? (GLib.get_monotonic_time() - this._since) / 1e6 : 0;
    }

    // ---- Offered or not ----

    _sync() {
        if (this.offered) {
            this._offer();
        } else {
            this.stop();
            this._scribble?.close();
            this._withdraw();
        }
    }

    _offer() {
        this._scribble ??= new Scribble(this, this._ext.dir.get_child('icons'));
        if (!this._bound) {
            Main.wm.addKeybinding('scribble', this._settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
                Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW | Shell.ActionMode.POPUP, () => this.toggleScribble());
            this._bound = true;
        }
        if (!this._indicator) {
            this._indicator = new SystemIndicator();
            this._toggle = new PresentToggle(this);
            this._indicator.quickSettingsItems.push(this._toggle);
            Main.panel.statusArea.quickSettings.addExternalIndicator(this._indicator);
            this._toggle.sync();
        }
        this._syncScreens();
    }

    _withdraw() {
        if (this._bound)
            Main.wm.removeKeybinding('scribble');
        this._bound = false;
        this._unwatchScreens();
        this._toggle?.destroy();
        this._toggle = null;
        this._indicator?.destroy();
        this._indicator = null;
        this._scribble?.close();
        this._scribble = null;
    }

    // ---- Presenting ----

    // resume: {since, connector, share} after a lock; otherwise it starts now, for the screen asked about (connector)
    start({since = 0, connector = null, share = null} = {}) {
        // Never for a screen that has gone (unplugged during a lock, or before the question was answered)
        if (this._presenting || !this.offered || (connector && !this._connectors().includes(connector)))
            return;
        // A share or recording running now ends it when it ends; one that ended during a lock ends it now
        const capture = new Capture();
        const unlisten = capture.listen(() => this._captureChanged());
        if (share && !capture.active) {
            unlisten();
            return;
        }
        this._capture = capture;
        this._unlistenCapture = unlisten;
        this._forShare = share ?? capture.active;
        this._presenting = true;
        this._since = since || GLib.get_monotonic_time();
        this._forConnector = connector;
        this._inhibit();
        this._listenToPhone();
        this._activity = new PresentMark(this);
        this._pill?.setPresent(this._activity);
        this._syncMark();
        this._syncScreens();
        this._tick = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 1, () => {
            this._activity?.tick();
            this._pill?.presentChanged();
            // The tile's time only while quick settings is open: closed, nothing of it is drawn
            if (Main.panel.statusArea.quickSettings?.menu.isOpen)
                this._toggle?.sync();
            return GLib.SOURCE_CONTINUE;
        });
        this._toggle?.sync();
    }

    // quietly: the module stops, nothing is said
    stop(quietly = false) {
        if (!this._presenting)
            return;
        this._presenting = false;
        if (this._tick)
            GLib.source_remove(this._tick);
        this._tick = 0;
        this._pointer?.destroy();
        this._pointer = null;
        this._uninhibit();
        this._stopListeningToPhone();
        this._unlistenCapture?.();
        this._unlistenCapture = null;
        this._capture = null;
        this._pill?.setPresent(null);
        this._activity?.destroy();
        this._activity = null;
        this._since = 0;
        this._forConnector = null;
        this._forShare = false;
        if (!quietly) {
            this._toggle?.sync();
            this._syncScreens();
        }
    }

    get _pill() {
        return this._ext.modules?.get('pill') ?? null;
    }

    // ---- The phone as a slide clicker ----

    // While presenting, the phone link is told so (it passes the phone's keys on only while the person has the clicker
    // on, which it checks itself), and each click is pressed as its key into the focused window
    _listenToPhone() {
        const connect = this._ext.modules?.get('connect');
        if (!connect?.onClicker)
            return;
        this._connect = connect;
        this._unlistenClicker = connect.onClicker((device, action) => this._click(action));
        connect.setPresenting(true);
    }

    _stopListeningToPhone() {
        this._unlistenClicker?.();
        this._unlistenClicker = null;
        this._connect?.setPresenting(false);
        this._connect = null;
        // Made at the first click, gone with the presenting
        this._keyboard?.run_dispose();
        this._keyboard = null;
    }

    // Whether the phone can click now: the person's switch, and the phone link there
    get clicker() {
        return this._presenting === true && this._connect?.clicker === true;
    }

    _click(action) {
        const key = CLICKER_KEYS[action];
        if (!this._presenting || key === undefined)
            return;
        this._keyboard ??= global.stage.context.get_backend().get_default_seat()
            .create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
        const time = GLib.get_monotonic_time();
        this._keyboard.notify_keyval(time, key, Clutter.KeyState.PRESSED);
        this._keyboard.notify_keyval(time + 1, key, Clutter.KeyState.RELEASED);
        this.clicks = (this.clicks ?? 0) + 1;
    }

    _syncMark() {
        this._toggle?.sync();
        if (!this._presenting) {
            this._pointer?.destroy();
            this._pointer = null;
            return;
        }
        if (this._pointer?.kind === this.mark)
            return;
        this._pointer?.destroy();
        this._pointer = this.mark === 'none' ? null : new PointerMark(this.mark);
    }

    _captureChanged() {
        if (this._forShare && !this._capture?.active)
            this.stop();
    }

    // The idle inhibitor, asked of gnome-session on the shell's own connection: gone with it if the shell ends
    _inhibit() {
        const serial = this._inhibitSerial = (this._inhibitSerial ?? 0) + 1;
        Gio.DBus.session.call('org.gnome.SessionManager', '/org/gnome/SessionManager', 'org.gnome.SessionManager',
            'Inhibit', new GLib.Variant('(susu)', ['org.gnome.Shell', 0, _('Presenting'), INHIBIT_IDLE]),
            new GLib.VariantType('(u)'), Gio.DBusCallFlags.NONE, -1, null, (connection, result) => {
                let cookie = 0;
                try {
                    [cookie] = connection.call_finish(result).deepUnpack();
                } catch (e) {
                    console.warn(`Amethystora UI: the screen could not be kept awake: ${e.message}`);
                    return;
                }
                // Stopped (or started again) before the answer came
                if (serial !== this._inhibitSerial || !this._presenting)
                    this._release(cookie);
                else
                    this._cookie = cookie;
            });
    }

    _uninhibit() {
        this._inhibitSerial = (this._inhibitSerial ?? 0) + 1;
        if (this._cookie)
            this._release(this._cookie);
        this._cookie = 0;
    }

    _release(cookie) {
        Gio.DBus.session.call('org.gnome.SessionManager', '/org/gnome/SessionManager', 'org.gnome.SessionManager',
            'Uninhibit', new GLib.Variant('(u)', [cookie]), null, Gio.DBusCallFlags.NONE, -1, null, (c, r) => {
                try {
                    c.call_finish(r);
                } catch {}
            });
    }

    // ---- A screen plugged in ----

    _connectors() {
        const list = [];
        for (const logical of global.backend.get_monitor_manager().get_logical_monitors()) {
            for (const monitor of logical.get_monitors())
                list.push(monitor.get_connector());
        }
        return list;
    }

    // Watched while the question is on, and while presenting for a screen, whatever the question says
    _syncScreens() {
        const wanted = this.offered && (this._settings.get_boolean('ask-on-screen') || Boolean(this._forConnector));
        if (wanted && !this._screensWatched) {
            global.backend.get_monitor_manager().connectObject('monitors-changed', () => this._screensChanged(), this);
            this._screensWatched = true;
            // The first time in a session, what is there already; later (after a lock), a screen plugged in meanwhile
            // is asked about now
            if (seen)
                this._screensChanged();
            else
                seen = new Set(this._connectors());
        } else if (!wanted) {
            this._unwatchScreens();
        }
    }

    _unwatchScreens() {
        if (this._screensWatched)
            global.backend.get_monitor_manager().disconnectObject(this);
        this._screensWatched = false;
    }

    _screensChanged() {
        const now = this._connectors();
        // The screen presenting was started for has gone
        if (this._presenting && this._forConnector && !now.includes(this._forConnector))
            this.stop();
        for (const connector of now) {
            if (seen.has(connector))
                continue;
            seen.add(connector);
            if (!this._presenting && this._settings.get_boolean('ask-on-screen'))
                this._ask(connector);
        }
    }

    // Once per screen and session; nothing happens without the click
    _ask(connector) {
        if (!this._notifications) {
            this._notifications = new MessageTray.Source({title: _('Present'), iconName: ICON});
            this._notifications.connect('destroy', source => {
                if (this._notifications === source)
                    this._notifications = null;
            });
            Main.messageTray.add(this._notifications);
        }
        const notification = new MessageTray.Notification({
            source: this._notifications,
            title: _('Present on the new screen?'),
            body: _('The screen stays awake, banners wait, and the pointer is easy to follow.'),
            isTransient: true,
        });
        notification.addAction(_('Present'), () => this.start({connector}));
        this._notifications.addNotification(notification);
        this.asked = (this.asked ?? 0) + 1;
    }

    // ---- Scribble ----

    // Closes in any mode; opens only over the desktop itself, never over a menu, a dialog or the screenshot bar
    toggleScribble() {
        if (this._scribble?.isOpen)
            this._scribble.close();
        else if (Main.modalCount === 0 && !Main.screenshotUI?.visible)
            this._scribble?.open();
    }

    scribbleClosed() {
        this.surfacesChanged();
    }

    get scribble() {
        return this._scribble;
    }

    glassSurfaces() {
        const bar = this._scribble?.toolbar;
        return bar ? [{actor: bar, material: 'glass', radius: null}] : [];
    }

    surfacesChanged() {
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    // For the VM suite: what runs, as a person's machine would show it
    state() {
        return {
            offered: this.offered,
            presenting: this.presenting,
            keybinding: Boolean(this._bound),
            tile: Boolean(this._toggle),
            screens: Boolean(this._screensWatched),
            inhibitor: this._cookie ?? 0,
            mark: this._pointer?.kind ?? null,
            markActor: Boolean(this._pointer?.actor),
            capture: Boolean(this._capture),
            timer: Boolean(this._tick),
            scribble: this._scribble?.isOpen ?? false,
            strokes: this._scribble?.strokes ?? 0,
            phoneListened: Boolean(this._unlistenClicker),
            keyboard: Boolean(this._keyboard),
            clicks: this.clicks ?? 0,
        };
    }
}
