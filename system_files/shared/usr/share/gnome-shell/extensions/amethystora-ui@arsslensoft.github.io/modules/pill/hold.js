// Holding banners: quiet (while a window is fullscreen on the main screen, while the screen is recorded or shared,
// and, when chosen, between Night Light's times) and the digest. A held notification still arrives: it is in the
// list, it counts on the clock's dot and on the app's dock badge, but it does not pop up and makes no sound. Urgent
// ones, GNOME's own feedback and GNOME Clocks' alarms and timers always pass. When quiet ends, the held ones stay in the
// list and the pill says how many are waiting, rather than showing them one after another; with the digest on, they
// wait for the next digest instead.
//
// Present mode (modules/present.js) is a reason of its own: while it is on, banners are held as quiet holds them,
// whatever the quiet key says.
//
// A focus profile (modules/focus.js) is one more reason, with its own rule: every banner held, or only the apps it
// names let through, and their sounds off. Its held banners are said once when it ends, as quiet's are; it has no
// activity of its own here (the profile's mark is).
//
// Later and mute (later.js) sit on the same wrap: a banner gets Later after its app's buttons, and a muted app's banners
// are held as quiet holds them, for the hour of the mute.
//
// Everything is held in memory: GNOME's Do Not Disturb setting is never written, so a crash, a lock or turning this
// off leaves notifications exactly as the person set them.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';

import {buttonRow, cardButton, retire} from './clock.js';
import {DigestCard} from './digest.js';
import {Later} from './later.js';
import {timeOfDay} from './text.js';

const COLOR_SCHEMA = 'org.gnome.settings-daemon.plugins.color';
const CLOCKS = ['org.gnome.clocks', 'org.gnome.clocks.desktop'];
const TIME = /^([01][0-9]|2[0-3]):[0-5][0-9]$/;
const MOST_TIMES = 6;
// How long "N waiting in the list" stays in the pill (s)
const WAITING = 4;

// gnome-settings-daemon's colour plugin knows when the sun rises and sets where the person is; GNOME's own interface
// file for it leaves the two properties out
const ColorIface = `
<node>
  <interface name="org.gnome.SettingsDaemon.Color">
    <property name="Sunrise" type="d" access="read"/>
    <property name="Sunset" type="d" access="read"/>
  </interface>
</node>`;
let ColorProxy = null;

// Whether a notification comes through whatever holds banners
export function passes(notification) {
    if (notification.urgency === MessageTray.Urgency.CRITICAL || notification.forFeedback)
        return true;
    const source = notification.source;
    return CLOCKS.includes(source?._appId) || CLOCKS.includes(source?.app?.get_id?.());
}

// The launcher IDs a notification's app goes by, each as APP.desktop
function idsOf(notification) {
    const source = notification.source;
    return [source?._appId, source?.app?.get_id?.()].filter(Boolean)
        .map(id => (id.endsWith('.desktop') ? id : `${id}.desktop`));
}

// Whether GNOME would show it as a banner at all (its own tests)
function wouldPopUp(notification) {
    if (notification.acknowledged || notification.urgency === MessageTray.Urgency.LOW)
        return false;
    return notification.source?.policy?.showBanners !== false || notification.urgency === MessageTray.Urgency.CRITICAL;
}

// The digest's times as the key gives them: HH:MM, at most six, sorted, each once
export function digestTimes(list) {
    return [...new Set((list ?? []).filter(t => TIME.test(t)))].sort().slice(0, MOST_TIMES);
}

// Whether `hour` (decimal) falls between `from` and `to`, a span that may pass midnight
function between(hour, from, to) {
    if (from === to)
        return false;
    return from < to ? hour >= from && hour < to : hour >= from || hour < to;
}

export class Hold {
    constructor(settings, capture, activities, notice, glass) {
        this._settings = settings;
        this._capture = capture;
        this._activities = activities;
        this._notice = notice;
        this._glass = glass;
        this.id = 'quiet';
        this.plate = 'quiet';
        this._focus = null;
        this._presenting = false;
    }

    enable() {
        this._quietHeld = new Set();
        this._digestHeld = new Set();
        this._focusHeld = new Set();
        this._reason = null;
        this._digestDue = false;
        this._held = new Map();

        // GNOME shows a banner by this one method, by its name: holding is a wrap of it, which lets through only
        // what passes
        const proto = MessageTray.MessageTray.prototype;
        this._original = proto._showNotification;
        const hold = this;
        const original = this._original;
        proto._showNotification = function (...args) {
            if (hold._holding()) {
                const queue = this._notificationQueue;
                const index = Array.isArray(queue) ? queue.findIndex(n => hold._lets(n)) : 0;
                if (index < 0)
                    return undefined;
                if (index > 0)
                    queue.unshift(...queue.splice(index, 1));
            }
            const result = original.apply(this, args);
            // Later and mute, after the app's own buttons
            try {
                hold._later?.decorate(this._banner);
            } catch (e) {
                console.error(`Amethystora UI: Later could not be added to a banner: ${e.message}`);
            }
            return result;
        };
        this._syncSound();

        this._settings.connectObject(
            'changed::quiet', () => this._configure(),
            'changed::quiet-night', () => this._configure(),
            'changed::digest', () => this._configure(),
            'changed::digest-times', () => this._configure(),
            'changed::digest-through', () => this._configure(),
            'changed::digest-wait', () => this._configure(),
            'changed::later', () => this._configure(),
            this);
        this._configure();
    }

    disable() {
        this._settings.disconnectObject(this);
        // The lock keeps what waits (later.js); anything else forgets it
        this._stopLater(false);
        if (this._original)
            MessageTray.MessageTray.prototype._showNotification = this._original;
        this._original = null;
        this._unwrapSound();
        this._unwatchQuiet();
        this._unwatchClock();
        this._unwatchNight();
        this._unwatchBanners();
        // What was held stays in the list, unseen; nothing is shown in a burst
        this._takeOffQueue([...this._quietHeld ?? [], ...this._digestHeld ?? []]);
        for (const notification of this._held?.keys() ?? [])
            notification.disconnectObject(this);
        this._held?.clear();
        this._quietHeld?.clear();
        this._digestHeld?.clear();
        this._focusHeld?.clear();
        this._closeDigest();
        this._end();
    }

    // --- What is on -----------------------------------------------------------------------------------------------

    get _quiet() {
        return this._settings.get_boolean('quiet');
    }

    get _night() {
        return this._settings.get_boolean('quiet-night');
    }

    get _digest() {
        return this._settings.get_boolean('digest');
    }

    get times() {
        return digestTimes(this._settings.get_strv('digest-times'));
    }

    // The digest's two lists (R5-33): apps that always come through while it is on, and apps that always wait for
    // its next time, also while it is off. Read at each banner, a lookup in a list of a few names.
    _listed(key, notification) {
        const list = this._settings.get_strv(key);
        return list.length > 0 && idsOf(notification).some(id => list.includes(id));
    }

    get _waitList() {
        return this._settings.get_strv('digest-wait').length > 0;
    }

    // Whether a banner held now waits for the digest: everything the digest holds but the apps let through, and the
    // apps that always wait
    _forDigest(notification) {
        return (this._digest && !this._listed('digest-through', notification)) ||
            this._listed('digest-wait', notification);
    }

    _configure() {
        if (this._settings.get_boolean('later'))
            this._startLater();
        else
            this._stopLater(true);
        if (this._quiet) {
            if (!this._quietWatched) {
                this._quietWatched = true;
                global.display.connectObject('in-fullscreen-changed', () => this._update(), this);
                this._unlistenCapture = this._capture.listen(() => this._update());
            }
        } else {
            this._unwatchQuiet();
        }
        if (this._night)
            this._watchNight();
        else
            this._unwatchNight();
        if (this._night || this._digest || this._waitList)
            this._watchClock();
        else
            this._unwatchClock();
        if (this._digestHeld.size > 0) {
            // The digest turned off, or an app moved off a list: what it no longer holds waits in the list, and is not
            // followed any more
            for (const notification of [...this._digestHeld]) {
                if (this._forDigest(notification))
                    continue;
                this._digestHeld.delete(notification);
                if (!this._quietHeld.has(notification) && !this._focusHeld.has(notification))
                    this._forget(notification);
            }
        }
        this._update();
    }

    _unwatchQuiet() {
        if (!this._quietWatched)
            return;
        this._quietWatched = false;
        global.display.disconnectObject(this);
        this._unlistenCapture?.();
        this._unlistenCapture = null;
    }

    // One clock of the module's own, once a minute, only while quiet at night or the digest is on
    // Once a minute, at the turn of the minute, only while quiet at night or the digest is on. (GNOME's own wall clock
    // would tick every second while the person shows seconds in the top bar.)
    _watchClock() {
        if (this._clock)
            return;
        const next = () => {
            const now = GLib.DateTime.new_now_local();
            const ms = Math.max(1, (60 - now.get_second()) * 1000 - Math.floor(now.get_microsecond() / 1000));
            this._clock = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms + 50, () => {
                this._clock = 0;
                next();
                this._minute();
                return GLib.SOURCE_REMOVE;
            });
        };
        next();
    }

    _unwatchClock() {
        if (this._clock)
            GLib.source_remove(this._clock);
        this._clock = 0;
    }

    _watchNight() {
        if (this._color)
            return;
        this._color = new Gio.Settings({schema_id: COLOR_SCHEMA});
        this._color.connectObject(
            'changed::night-light-schedule-automatic', () => this._update(),
            'changed::night-light-schedule-from', () => this._update(),
            'changed::night-light-schedule-to', () => this._update(),
            this);
        ColorProxy ??= Gio.DBusProxy.makeProxyWrapper(ColorIface);
        this._colorCancellable = new Gio.Cancellable();
        new ColorProxy(Gio.DBus.session, 'org.gnome.SettingsDaemon.Color', '/org/gnome/SettingsDaemon/Color',
            (proxy, error) => {
                if (error || !this._color)
                    return;
                this._sun = proxy;
                proxy.connectObject('g-properties-changed', () => this._update(), this);
                this._update();
            }, this._colorCancellable, Gio.DBusProxyFlags.DO_NOT_AUTO_START);
    }

    _unwatchNight() {
        this._colorCancellable?.cancel();
        this._colorCancellable = null;
        this._sun?.disconnectObject(this);
        this._sun = null;
        this._color?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._color?.run_dispose();
        this._color = null;
    }

    // Night Light's span, in decimal hours: the sun's times when it follows them and knows them, else the person's
    nightSpan() {
        const color = this._color;
        if (!color)
            return null;
        if (color.get_boolean('night-light-schedule-automatic')) {
            const sunset = this._sun?.Sunset;
            const sunrise = this._sun?.Sunrise;
            if (sunset > 0 && sunrise > 0)
                return {from: sunset, to: sunrise};
        }
        return {from: color.get_double('night-light-schedule-from'), to: color.get_double('night-light-schedule-to')};
    }

    _minute() {
        this._update();
        if (!this._digest && !this._waitList)
            return;
        const now = GLib.DateTime.new_now_local().format('%H:%M');
        if (this.times.includes(now) && this._digestHeld.size > 0) {
            if (this._reason)
                this._digestDue = true;
            else
                this._showDigest(now);
        }
    }

    // --- Quiet ----------------------------------------------------------------------------------------------------

    _currentReason() {
        if (this._presenting)
            return 'presenting';
        if (this._quiet) {
            if (this._capture.recording)
                return 'recording';
            if (this._capture.sharing)
                return 'sharing';
            if (Main.layoutManager.primaryMonitor?.inFullscreen)
                return 'fullscreen';
        }
        if (this._night) {
            const span = this.nightSpan();
            const now = GLib.DateTime.new_now_local();
            if (span && between(now.get_hour() + now.get_minute() / 60, span.from, span.to))
                return 'night';
        }
        return null;
    }

    get reason() {
        return this._reason;
    }

    _holding() {
        return this._reason !== null || this._digest || this._waitList || this._focusHolding() || !!this._later?.muting;
    }

    // --- Later and mute (later.js) --------------------------------------------------------------------------------

    _startLater() {
        if (this._later)
            return;
        const later = new Later(this._settings, this._notice, {
            changed: () => this._update(),
            ended: (entry, byHand) => this._muteEnded(entry, byHand),
        });
        this._later = later;
        try {
            later.enable();
        } catch (e) {
            console.error(`Amethystora UI: Later did not start: ${e.message}\n${e.stack}`);
            this._stopLater(true);
        }
    }

    // forget: the key was turned off (nothing waits any more); else the lock may keep what waits
    _stopLater(forget) {
        const later = this._later;
        this._later = null;
        try {
            later?.disable(forget);
        } catch (e) {
            console.error(`Amethystora UI: Later did not stop cleanly: ${e.message}`);
        }
        if (!later || !forget || !this._quietHeld)
            return;
        // What only a mute held stays in the list, followed no more
        for (const notification of [...this._held.keys()]) {
            if (!this._quietHeld.has(notification) && !this._digestHeld.has(notification) &&
                !this._focusHeld.has(notification))
                this._forget(notification);
        }
        this._update();
    }

    // A mute ended. By hand (Unmute), what it held is shown as one card; at the end of its hour the pill says how many
    // wait, as quiet does. What quiet or a profile still holds stays theirs.
    _muteEnded(entry, byHand) {
        if (!this._quietHeld)
            return;
        // A mute kept across the lock holds notifications this hold never followed: they count all the same
        const held = [...entry.held].filter(n => !this._quietHeld.has(n) && !this._focusHeld.has(n) &&
            !this._digestHeld.has(n));
        for (const notification of held) {
            if (this._held.has(notification))
                this._forget(notification);
        }
        if (held.length === 0)
            return;
        if (byHand) {
            this._closeDigest();
            this._digestCard = new DigestCard(held, GLib.DateTime.new_now_local().format('%H:%M'),
                () => this._closeDigest(), _('Held while %s was muted').format(entry.name));
            this._glass?.()?.surfacesChanged?.();
        } else {
            this._notice(ngettext('%d waiting in the list', '%d waiting in the list', held.length).format(held.length),
                'weather-clear-night-symbolic', WAITING, '', () => Main.panel.statusArea.dateMenu?.menu.open());
        }
    }

    get later() {
        return this._later;
    }

    // --- Present mode ----------------------------------------------------------------------------------------------

    setPresenting(on) {
        if (this._presenting === on)
            return;
        this._presenting = on;
        this._update();
    }

    // --- A focus profile --------------------------------------------------------------------------------------------

    // The profile's rule ({banners: shown|apps|held, apps, sounds}), or null when none is on
    setFocus(rule) {
        if (JSON.stringify(rule) === JSON.stringify(this._focus))
            return;
        const was = this._focusHolding();
        this._focus = rule;
        // Not started yet: the rule waits for enable()
        if (!this._quietHeld)
            return;
        if (was && !this._focusHolding())
            this._focusReleased();
        this._syncSound();
        this._update();
    }

    // A profile with its sounds off: what it lets through shows without a sound, but calls, alarms and urgent ones.
    // GNOME's method is wrapped only while such a profile is on.
    _syncSound() {
        const wanted = this._focus?.sounds === false;
        const proto = MessageTray.Notification.prototype;
        if (wanted && !this._originalSound) {
            const hold = this;
            const original = proto.playSound;
            this._originalSound = original;
            proto.playSound = function (...args) {
                if (hold._silenced(this))
                    return undefined;
                return original.apply(this, args);
            };
        } else if (!wanted) {
            this._unwrapSound();
        }
    }

    _unwrapSound() {
        if (this._originalSound)
            MessageTray.Notification.prototype.playSound = this._originalSound;
        this._originalSound = null;
    }

    _focusHolding() {
        return !!this._focus && this._focus.banners !== 'shown';
    }

    // Whether a banner may show now: what always passes, and what the profile lets through while nothing else holds
    _lets(notification) {
        if (passes(notification))
            return true;
        if (this._reason !== null || this._later?.muteOf(notification) || this._forDigest(notification))
            return false;
        if (!this._focusHolding())
            return true;
        if (this._focus.banners !== 'apps')
            return false;
        return idsOf(notification).some(id => this._focus.apps.includes(id));
    }

    _silenced(notification) {
        return !!this._focus && this._focus.sounds === false && !passes(notification);
    }

    // The profile ended or lets everything through: what it held stays in the list, said once; with the digest on it
    // waits for the digest, and what quiet still holds stays quiet's
    _focusReleased() {
        const held = [...this._focusHeld].filter(n => !this._quietHeld.has(n));
        this._focusHeld.clear();
        // What the digest holds waits for it
        const waiting = held.filter(n => this._held.has(n) && !this._digestHeld.has(n));
        for (const notification of waiting)
            this._forget(notification);
        if (waiting.length > 0) {
            this._notice(ngettext('%d waiting in the list', '%d waiting in the list', waiting.length).format(waiting.length),
                'weather-clear-night-symbolic', WAITING, '', () => Main.panel.statusArea.dateMenu?.menu.open());
        }
    }

    get focusHeld() {
        return this._focusHeld?.size ?? 0;
    }

    _update() {
        if (!this._quietHeld)
            return;
        const before = this._reason;
        this._reason = this._currentReason();
        if (this._holding()) {
            this._watchBanners();
            this._holdQueued();
        } else {
            this._unwatchBanners();
        }
        if (before !== null && this._reason === null)
            this._released();
        this._show();
    }

    // Quiet ended: the held ones stay in the list, and the pill says so once; with the digest on they wait for it
    _released() {
        const held = [...this._quietHeld];
        this._quietHeld.clear();
        this._takeOffQueue(held);
        if (this._digestDue) {
            this._digestDue = false;
            this._showDigest(GLib.DateTime.new_now_local().format('%H:%M'));
        }
        // What a focus profile still holds stays held until it ends, and what the digest holds waits for it
        const waiting = held.filter(n => this._held.has(n) && !this._focusHeld.has(n) && !this._digestHeld.has(n));
        for (const notification of waiting)
            this._forget(notification);
        if (waiting.length > 0) {
            this._notice(ngettext('%d waiting in the list', '%d waiting in the list', waiting.length).format(waiting.length),
                'weather-clear-night-symbolic', WAITING, '', () => Main.panel.statusArea.dateMenu?.menu.open());
        }
    }

    // --- Banners --------------------------------------------------------------------------------------------------

    _watchBanners() {
        if (this._bannersWatched)
            return;
        this._bannersWatched = true;
        Main.messageTray.connectObject('source-added', (tray, source) => this._watchSource(source), this);
        for (const source of Main.messageTray.getSources())
            this._watchSource(source);
    }

    _watchSource(source) {
        source.connectObject('notification-request-banner', (s, notification) => this._requested(notification), this);
    }

    _unwatchBanners() {
        if (!this._bannersWatched)
            return;
        this._bannersWatched = false;
        Main.messageTray.disconnectObject(this);
        for (const source of Main.messageTray.getSources())
            source.disconnectObject(this);
    }

    // GNOME has just queued (or, its queue being full, dropped) a banner: one that does not pass is taken off the
    // queue and counted
    // What GNOME queued before a reason to hold came (a mute pressed, the extension back after the lock, quiet
    // beginning), and a muted app's banner already on screen, are held as a banner asked for now would be
    _holdQueued() {
        const tray = Main.messageTray;
        for (const notification of [...tray._notificationQueue ?? []])
            this._requested(notification);
        const shown = tray._notification;
        if (shown && this._later?.muteOf(shown) && !passes(shown)) {
            this._requested(shown, true);
            tray._expireNotification?.();
        }
    }

    // onScreen: the banner showing now, which a mute takes away
    _requested(notification, onScreen = false) {
        // The banner on screen being updated is not held: it is shown already
        if (!this._holding() || this._lets(notification) ||
            (!onScreen && (!wouldPopUp(notification) || Main.messageTray._notification === notification)))
            return;
        this._takeOffQueue([notification]);
        if (!this._held.has(notification)) {
            this._held.set(notification, true);
            notification.connectObject('destroy', () => this._forget(notification), this);
        }
        if (this._reason)
            this._quietHeld.add(notification);
        if (this._forDigest(notification))
            this._digestHeld.add(notification);
        if (this._focusHolding())
            this._focusHeld.add(notification);
        this._later?.held(notification);
        this._show();
    }

    _forget(notification) {
        notification.disconnectObject(this);
        this._later?.forget(notification);
        this._held.delete(notification);
        this._quietHeld.delete(notification);
        this._digestHeld.delete(notification);
        this._focusHeld.delete(notification);
        this._show();
    }

    _takeOffQueue(notifications) {
        const tray = Main.messageTray;
        const queue = tray?._notificationQueue;
        if (!Array.isArray(queue) || notifications.length === 0)
            return;
        const kept = queue.filter(n => !notifications.includes(n));
        if (kept.length === queue.length)
            return;
        tray._notificationQueue = kept;
        tray.emit('queue-changed');
    }

    get quietHeld() {
        return this._quietHeld?.size ?? 0;
    }

    get digestHeld() {
        return this._digestHeld?.size ?? 0;
    }

    // --- Quiet in the pill ----------------------------------------------------------------------------------------

    _whyText() {
        switch (this._reason) {
        case 'fullscreen':
            return _('Quiet while fullscreen');
        case 'sharing':
        case 'recording':
            return _('Quiet while sharing');
        case 'presenting':
            return _('Quiet while presenting');
        case 'night': {
            const span = this.nightSpan();
            const hours = Math.floor(span.to);
            const minutes = Math.round((span.to - hours) * 60) % 60;
            const date = new Date();
            date.setHours(hours, minutes, 0, 0);
            return _('Quiet until %s').format(timeOfDay(date));
        }
        default:
            return '';
        }
    }

    get text() {
        const held = this.quietHeld;
        return held > 0 ? ngettext('%s · %d held', '%s · %d held', held).format(this._whyText(), held) : this._whyText();
    }

    get accessible() {
        const held = this.quietHeld;
        return held > 0
            ? ngettext('%s, %d notification held', '%s, %d notifications held', held).format(this._whyText(), held)
            : this._whyText();
    }

    get row() {
        return {icon: 'weather-clear-night-symbolic', text: this.text};
    }

    _show() {
        if (!this._reason) {
            this._end();
            return;
        }
        this.pill();
        this._label.text = this.text;
        this._activities.set('quiet', this);
        this._activities.update('quiet');
    }

    _end() {
        if (this._activities.get('quiet') === this)
            this._activities.set('quiet', null);
        retire(this._content);
        this._content = null;
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-quiet'});
            this._content.add_child(new St.Icon({icon_name: 'weather-clear-night-symbolic', icon_size: 12, y_align: Clutter.ActorAlign.CENTER}));
            this._label = new St.Label({y_align: Clutter.ActorAlign.CENTER});
            this._content.add_child(this._label);
        }
        return this._content;
    }

    card(api) {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        body.add_child(new St.Label({style_class: 'amethystora-pill-card-label', text: _('Notifications are waiting')}));
        const why = {
            fullscreen: _('a window is fullscreen'),
            sharing: _('the screen is shared'),
            recording: _('the screen is recorded'),
            presenting: _('you present'),
            night: _('it is night'),
        }[this._reason] ?? '';
        const held = this.quietHeld;
        const text = new St.Label({
            style_class: 'amethystora-pill-card-text',
            text: `${held > 0
                ? ngettext('%d held while %s.', '%d held while %s.', held).format(held, why)
                : _('Banners wait while %s.').format(why)} ${_('Calls, alarms and urgent ones still come through.')}`,
        });
        text.clutter_text.line_wrap = true;
        body.add_child(text);
        body.add_child(buttonRow([cardButton(_('Show them'), () => {
            api.close();
            Main.panel.statusArea.dateMenu?.menu.open();
        }, {primary: true})]));
        return body;
    }

    // --- The digest -----------------------------------------------------------------------------------------------

    _showDigest(time) {
        const held = [...this._digestHeld].filter(n => this._held.has(n));
        this._digestHeld.clear();
        for (const notification of held)
            this._forget(notification);
        if (held.length === 0)
            return;
        this._closeDigest();
        this._digestCard = new DigestCard(held, time, () => this._closeDigest());
        this._glass?.()?.surfacesChanged?.();
    }

    // For the suites and the harness: the digest now, of what is held
    showDigestNow() {
        this._showDigest(GLib.DateTime.new_now_local().format('%H:%M'));
    }

    _closeDigest() {
        const card = this._digestCard;
        this._digestCard = null;
        card?.close();
        if (card)
            this._glass?.()?.surfacesChanged?.();
    }

    get digestShown() {
        return !!this._digestCard;
    }

    get digestCard() {
        return this._digestCard ?? null;
    }

    glassSurfaces() {
        return this._digestCard?.actor ? [{actor: this._digestCard.actor, material: 'glass', radius: 22}] : [];
    }
}
