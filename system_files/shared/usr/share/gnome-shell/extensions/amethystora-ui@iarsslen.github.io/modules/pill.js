// The live pill and what goes with it. The clock in the middle of the top bar becomes whatever is happening now (a
// call, a recording, a link from the phone, the battery's question, a timer, what an update brought, music, an update
// ready, quiet), and a click opens its card; with nothing happening it is GNOME's clock, exactly. Beside it: the
// privacy dots, the glow round the screen while it is recorded or shared, quiet (banners held while fullscreen, sharing
// or at night), the digest, Later and mute on banners, the count on a stack of notifications, and volume and
// brightness as a pill under the top bar.
//
// Each part is a key of org.gnome.shell.extensions.amethystora-ui.pill (ame desktop pill|quiet|digest|notifications|
// whats-new, Control), read
// live; a part that is off has nothing connected and nothing made. Nothing here polls: every source is a signal. A
// focus profile (modules/focus.js) tells the pill its banners and sounds (setFocus) and gives it its mark
// (setFocusMark), the last of the activities.

import GLib from 'gi://GLib';
import St from 'gi://St';
import Clutter from 'gi://Clutter';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {Activities} from './pill/activities.js';
import {Battery, batteryView} from './pill/battery.js';
import {Capture, Recording} from './pill/capture.js';
import {Clock, chip, retire} from './pill/clock.js';
import {Glow} from './pill/glow.js';
import {Hold} from './pill/hold.js';
import {Media} from './pill/media.js';
import {Osd} from './pill/osd.js';
import {Phone} from './pill/phone.js';
import {Privacy} from './pill/privacy.js';
import {Stacks} from './pill/stacks.js';
import {disposeSettings} from './pill/text.js';
import {Timer, armOffRing, disarmOffRing} from './pill/timer.js';
import {Update} from './pill/update.js';
import {News, writeSeen} from './pill/whatsnew.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.pill';
// GNOME's own recording and sharing buttons in the bar: the pill and the dots carry their Stop
const GNOME_CAPTURE_ROLES = ['screenRecording', 'screenSharing'];

// A word in the pill for a few seconds: "Timer done", "2 waiting in the list", "Opened in the browser"; with an
// action ({label, run}), a chip for it beside the words ("Back at 18:00" and Undo)
class Notice {
    constructor(text, icon, plate, activate, action = null) {
        this.id = 'waiting';
        this.text = text;
        this.accessible = action ? `${text}. ${action.label}` : text;
        this.plate = plate;
        this.row = {icon, text};
        this._icon = icon;
        this._action = action;
        if (activate)
            this.activate = activate;
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-notice'});
            this._content.add_child(new St.Icon({icon_name: this._icon, icon_size: 12, y_align: Clutter.ActorAlign.CENTER}));
            this._content.add_child(new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER}));
            if (this._action)
                this._content.add_child(chip(this._action.label, this._action.run));
        }
        return this._content;
    }

    card() {
        return new St.Label({style_class: 'amethystora-pill-card-label', text: this.text});
    }

    destroy() {
        retire(this._content);
        this._content = null;
    }
}

export class Pill {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._capture = new Capture();
        this._activities = new Activities(() => this._clock?.sync());
        this._unlistenComfort = this._comfort?.listen(() => this._comfortChanged());
        this._osd = new Osd();
        this._osd.enable();
        this._stacks = new Stacks();
        this._stacks.enable();
        this._settings.connectObject(
            'changed::live', () => this._sync(),
            'changed::privacy-dots', () => this._sync(),
            'changed::glow', () => this._sync(),
            'changed::quiet', () => this._sync(),
            'changed::quiet-night', () => this._sync(),
            'changed::digest', () => this._sync(),
            'changed::later', () => this._sync(),
            this);
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._stopLive();
        // Turned off whole (as while the screen is locked): nothing waits; the timer rings at the next enable if it ended
        disarmOffRing();
        this._stopPart('_privacy');
        this._stopPart('_glow');
        this._stopPart('_hold');
        this._showGnomeCapture();
        this._stacks?.disable();
        this._stacks = null;
        this._osd?.disable();
        this._osd = null;
        this._unlistenComfort?.();
        this._unlistenComfort = null;
        this._endNotice();
        this._activities?.clear();
        this._activities = null;
        this._capture?.destroy();
        this._capture = null;
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        disposeSettings();
        // The focus module tells the rule again when it starts, and the present module its mark
        this._focusRule = null;
        this._presentMark = null;
    }

    get _comfort() {
        return this._ext.modules?.get('comfort') ?? null;
    }

    // Reduce motion, as the comfort module says (GNOME's animations off counting too)
    _reduce() {
        const comfort = this._comfort;
        return comfort ? comfort.reduceMotion : !St.Settings.get().enable_animations;
    }

    _comfortChanged() {
        // Under reduce motion the loops stand still at once
        if (this._reduce())
            this._media?._restBars?.(this._media.playing ? null : 0.35);
    }

    _glass() {
        return this._ext.modules?.get('glass') ?? null;
    }

    // Each part as its key says
    _sync() {
        const on = key => this._settings.get_boolean(key);
        if (on('live')) {
            this._startLive();
        } else {
            this._stopLive();
            // The live pill off: a timer that runs still rings at its end
            armOffRing();
        }
        this._toggle('_privacy', on('privacy-dots'), () => new Privacy(this._capture));
        this._toggle('_glow', on('glow'), () => new Glow(this._capture, () => this._reduce()));
        this._toggle('_hold', on('quiet') || on('quiet-night') || on('digest') || on('later') || this._focusWants() ||
            !!this._presentMark,
            () => new Hold(this._settings, this._capture, this._activities, (...a) => this.notice(...a), () => this._glass()));
        this._hold?.setFocus(this._focusRule ?? null);
        this._hold?.setPresenting(!!this._presentMark);
        if (on('live') || on('privacy-dots'))
            this._hideGnomeCapture();
        else
            this._showGnomeCapture();
    }

    // ---- Present mode (modules/present.js) ----

    // Its mark in the pill (an activity, or null), and banners held while it is there
    setPresent(mark) {
        if ((this._presentMark ?? null) === (mark ?? null))
            return;
        this._presentMark = mark ?? null;
        this._activities?.set('present', this._presentMark);
        // Told before the hold may stop, so that what it held is said once ("N waiting in the list")
        this._hold?.setPresenting(Boolean(this._presentMark));
        if (this._settings)
            this._sync();
    }

    presentChanged() {
        this._activities?.update('present');
    }

    // ---- A focus profile (modules/focus.js) ----

    // Whether the profile on needs the hold: banners held, or their sounds off
    _focusWants() {
        const rule = this._focusRule;
        return !!rule && (rule.banners !== 'shown' || rule.sounds === false);
    }

    // The profile's banners and sounds ({banners, apps, sounds}), or null: in memory, as quiet's are. The hold is told
    // first, so that what it held is said once before it stops.
    setFocus(rule) {
        if (JSON.stringify(rule ?? null) === JSON.stringify(this._focusRule ?? null))
            return;
        this._focusRule = rule ?? null;
        this._hold?.setFocus(this._focusRule);
        if (this._settings)
            this._sync();
    }

    // The profile's mark, the pill's last activity, or null (the mark fades out of the pill as any activity does)
    setFocusMark(mark) {
        if (this._activities && this._activities.get('focus') !== mark)
            this._activities.set('focus', mark);
    }

    focusMarkChanged() {
        this._activities?.update('focus');
    }

    // Reading the selection aloud (modules/access/speech.js), or null when it ends
    setReading(activity) {
        if (this._activities && this._activities.get('reading') !== activity)
            this._activities.set('reading', activity);
    }

    readingChanged() {
        this._activities?.update('reading');
    }

    // ---- The agent's changes (modules/agent.js) ----

    // The agent's activity (what its tools change while it runs), or null
    setAgent(activity) {
        if (this._activities && this._activities.get('agent') !== activity)
            this._activities.set('agent', activity);
    }

    agentChanged() {
        this._activities?.update('agent');
    }

    _toggle(name, wanted, make) {
        if (wanted && !this[name]) {
            const part = make();
            this[name] = part;
            try {
                part.enable();
            } catch (e) {
                console.error(`Amethystora UI: the pill module's ${name.slice(1)} did not start: ${e.message}\n${e.stack}`);
                this._stopPart(name);
            }
        } else if (!wanted && this[name]) {
            this._stopPart(name);
        }
    }

    _stopPart(name) {
        const part = this[name];
        this[name] = null;
        try {
            part?.disable();
        } catch (e) {
            console.error(`Amethystora UI: the pill module's ${name.slice(1)} did not stop cleanly: ${e.message}`);
        }
    }

    _startLive() {
        if (this._clock)
            return;
        const notice = (...a) => this.notice(...a);
        const reduce = () => this._reduce();
        this._clock = new Clock(this._activities, reduce);
        try {
            this._clock.enable();
        } catch (e) {
            console.error(`Amethystora UI: the pill module's clock did not start: ${e.message}\n${e.stack}`);
            this._stopLive();
            return;
        }
        this._live = [];
        const parts = [
            ['_media', () => new Media(this._activities, reduce)],
            ['_timer', () => new Timer(this._activities, notice)],
            ['_recording', () => new Recording(this._activities, notice, this._capture, reduce)],
            ['_update', () => new Update(this._activities)],
            ['_news', () => new News(this._activities, this._settings)],
            ['_battery', () => new Battery(this._activities, notice, this._comfort, () => this._glass())],
        ];
        const connect = this._ext.modules?.get('connect');
        if (connect?.addListener)
            parts.push(['_phone', () => new Phone(this._activities, notice, connect)]);
        for (const [name, make] of parts) {
            this._toggle(name, true, make);
            if (this[name])
                this._live.push(name);
        }
        // The level meter moves while the pointer is on the pill
        Main.panel.statusArea.dateMenu?.connectObject('notify::hover', actor => this._activities.top?.hovered?.(actor.hover), this);
        this._clock.sync();
    }

    _stopLive() {
        Main.panel.statusArea.dateMenu?.disconnectObject(this);
        // The clock lets the activities' contents go first, so that each part takes its own away
        if (this._clock) {
            try {
                this._clock.disable();
            } catch (e) {
                console.error(`Amethystora UI: the pill's clock did not stop cleanly: ${e.message}`);
            }
            this._clock = null;
        }
        for (const name of [...this._live ?? []].reverse())
            this._stopPart(name);
        this._live = null;
        this._endNotice();
    }

    _hideGnomeCapture() {
        if (this._gnomeCaptureHidden)
            return;
        this._gnomeCaptureHidden = true;
        for (const role of GNOME_CAPTURE_ROLES)
            Main.panel.statusArea[role]?.container?.hide();
    }

    _showGnomeCapture() {
        if (!this._gnomeCaptureHidden)
            return;
        this._gnomeCaptureHidden = false;
        for (const role of GNOME_CAPTURE_ROLES)
            Main.panel.statusArea[role]?.container?.show();
    }

    // A word in the pill for `seconds`, in the "waiting" place; plate is '' or one of the plate's looks; action
    // ({label, run}) adds a chip that runs it and ends the word
    notice(text, icon, seconds, plate = '', activate = null, action = null) {
        if (!this._activities)
            return;
        this._endNotice();
        const notice = new Notice(text, icon, plate, activate ? () => {
            this._endNotice();
            activate();
        } : null, action ? {
            label: action.label,
            run: () => {
                this._endNotice();
                action.run();
            },
        } : null);
        this._notice = notice;
        this._activities.set('waiting', notice);
        this._noticeTimer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, seconds, () => {
            this._noticeTimer = 0;
            this._endNotice();
            return GLib.SOURCE_REMOVE;
        });
    }

    _endNotice() {
        if (this._noticeTimer)
            GLib.source_remove(this._noticeTimer);
        this._noticeTimer = 0;
        const notice = this._notice;
        this._notice = null;
        if (notice && this._activities?.get('waiting') === notice)
            this._activities.set('waiting', null);
        notice?.destroy();
    }

    // --- For the glass, the suites and the harness ----------------------------------------------------------------

    glassSurfaces() {
        return [...this._battery?.glassSurfaces() ?? [], ...this._hold?.glassSurfaces() ?? []];
    }

    get timer() {
        return this._timer;
    }

    get privacy() {
        return this._privacy;
    }

    get capture() {
        return this._capture;
    }

    // Whether the screen is recorded or shared now, for the tips, which never show then
    sharing() {
        return !!this._capture?.active;
    }

    get hold() {
        return this._hold;
    }

    get clock() {
        return this._clock;
    }

    get news() {
        return this._news;
    }

    // For the suites and the harness: what's new as after an update from `seen` to `info` ({version, releases: [{version,
    // items}]}, the image's file), as the live pill's start would find it; seen undefined: a new account
    newsForTest(info, seen) {
        if (!this._live)
            return false;
        writeSeen(seen);
        this._stopPart('_news');
        this._toggle('_news', true, () => new News(this._activities, this._settings, info));
        return this._news?.shown ?? false;
    }

    get phone() {
        return this._phone;
    }

    // What the battery would show for a reading (a pure function, for the suites): {offer, hover}
    batteryView(reading) {
        const {offer, hover} = batteryView(reading);
        return {offer, hover};
    }

    state() {
        const activities = this._activities;
        const top = activities?.top ?? null;
        const dateMenu = Main.panel.statusArea.dateMenu;
        return {
            activity: top?.id ?? 'idle',
            text: top ? top.text : dateMenu?._clockDisplay?.text ?? '',
            others: activities?.others.map(a => a.id) ?? [],
            card: this._clock?.cardId ?? null,
            dots: this._privacy?.state ?? {microphone: false, camera: false, screen: false, location: false, visible: false},
            glow: this._glow?.monitors ?? 0,
            quiet: {on: !!this._hold?.reason, reason: this._hold?.reason ?? null, held: this._hold?.quietHeld ?? 0},
            digest: {held: this._hold?.digestHeld ?? 0, shown: this._hold?.digestShown ?? false},
            later: this._hold?.later?.state() ?? null,
            news: this._news?.shown ? this._news.news : null,
            osd: this._osd?.state ?? null,
            timer: {running: this._timer?.running ?? false, paused: this._timer?.paused ?? false, left: Math.round(this._timer?.left ?? 0)},
            paused: this._media?.paused === true,
            relayouts: this._clock?.relayouts ?? 0,
            motion: {
                morphing: this._clock?.morphing ?? false,
                bars: this._media?.barsMoving ?? false,
                pulsing: (this._recording?.pulsing ?? false) || (this._glow?.breathing ?? false),
                reduce: this._reduce(),
            },
        };
    }
}
