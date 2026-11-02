// The quick timer: four buttons at the end of the calendar's column start one, and the pill counts it down with a
// ring. When it rings: a sound, an urgent notification (which quiet and the digest let through) and "Timer done" in
// the pill. The end is kept here, outside the module, so that a timer runs on while the screen is locked (GNOME turns
// the extension off then): one that ended meanwhile rings as soon as the screen is unlocked, and says when it ended.
//
// A timer you can feel (R7-22, the key timer-feel): the ring drains round the pill itself, a second timer sits beside the
// first in the bubble, a timer can have a name ("tea 4 min" in search), and its end gives a gentle glow as well as the
// sound. While it cannot be seen it is one wait to its end, not a tick a second. A name is in memory only.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';

import {buttonRow, cardButton, retire} from './clock.js';
import {clock, spoken, timeOfDay} from './text.js';

// Minutes of the four buttons, and the most a timer runs
const MINUTES = [1, 5, 10, 25];
const LONGEST = 99 * 60;

// The timers that run, across the module being turned off and on: when each ends (ms since the epoch), how long it was
// set for (s), when it was started (ms), how much was left when it was paused (s, or null), and its name ('' for none).
// The second place is only used while timer-feel is on.
const slots = [null, null];

// The ring: a track and the part left, from the top, clockwise. The board draws it in a 20 px box with a radius of 8.
const RingArea = GObject.registerClass(
class AmethystoraPillRing extends St.DrawingArea {
    _init(styleClass, stroke) {
        super._init({style_class: styleClass, y_align: Clutter.ActorAlign.CENTER});
        this._stroke = stroke;
        this.fraction = 1;
    }

    vfunc_repaint() {
        const cr = this.get_context();
        const [width, height] = this.get_surface_size();
        const node = this.get_theme_node();
        const size = Math.min(width, height);
        const unit = size / 20;
        const radius = 8 * unit;
        const [cx, cy] = [width / 2, height / 2];
        cr.setLineWidth(this._stroke * unit);
        const [found, track] = node.lookup_color('-amethystora-pill-track', false);
        if (found) {
            cr.setSourceColor(track);
            cr.arc(cx, cy, radius, 0, 2 * Math.PI);
            cr.stroke();
        }
        const fraction = Math.min(Math.max(this.fraction, 0), 1);
        if (fraction > 0) {
            cr.setLineCap(1); // round
            cr.setSourceColor(node.get_foreground_color());
            const start = -Math.PI / 2;
            cr.arc(cx, cy, radius, start, start + 2 * Math.PI * fraction);
            cr.stroke();
        }
        cr.$dispose();
    }
});

// While the live pill is off, a timer that runs still rings: one timeout to its end, which only rings. The pill module
// arms it when its key goes off and takes it away when it stops (a lock) or when the pill is on again.
const offRings = [0, 0];
let offWake = null;

// A wait is on the monotonic clock, which a suspend does not count: on waking the end is looked at again by the wall clock.
// Only while a wait is armed does anything listen; returns what stops it
function onWake(callback) {
    const bus = Gio.DBus.system;
    const id = bus.signal_subscribe('org.freedesktop.login1', 'org.freedesktop.login1.Manager', 'PrepareForSleep',
        '/org/freedesktop/login1', null, Gio.DBusSignalFlags.NONE, (c, s, p, i, n, params) => {
            if (params.deepUnpack()[0] === false)
                callback();
        });
    return () => bus.signal_unsubscribe(id);
}

export function armOffRing() {
    disarmOffRing();
    slots.forEach((timer, slot) => {
        if (!timer || timer.pausedLeft !== null)
            return;
        offWake ??= onWake(() => armOffRing());
        const seconds = Math.max(1, Math.ceil((timer.end - Date.now()) / 1000));
        offRings[slot] = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, seconds, () => {
            offRings[slot] = 0;
            const ended = slots[slot];
            slots[slot] = null;
            ring(ended, false);
            if (!offRings.some(Boolean)) {
                offWake?.();
                offWake = null;
            }
            return GLib.SOURCE_REMOVE;
        });
    });
}

export function disarmOffRing() {
    offWake?.();
    offWake = null;
    offRings.forEach((id, slot) => {
        if (id)
            GLib.source_remove(id);
        offRings[slot] = 0;
    });
}

// Whether the second place holds a timer: its part stays while it does, whatever the key says
export function secondRuns() {
    return slots[1] !== null;
}

// The sound and the urgent notification (which quiet and the digest let through) of a timer that ended
function ring(timer, late) {
    if (!timer)
        return;
    const minutes = Math.max(1, Math.round(timer.total / 60));
    const length = ngettext('%d minute', '%d minutes', minutes).format(minutes);
    const body = late
        ? _('%s, ended at %s').format(length, timeOfDay(new Date(timer.end)))
        : _('%s, started at %s').format(length, timeOfDay(new Date(timer.started)));
    const title = timer.name ? _('%s is done').format(timer.name) : _('The timer is done');
    global.display.get_sound_player().play_from_theme('alarm-clock-elapsed', title, null);
    const source = new MessageTray.Source({title: _('Timer'), iconName: 'alarm-symbolic'});
    Main.messageTray.add(source);
    source.addNotification(new MessageTray.Notification({
        source,
        title,
        body,
        iconName: 'alarm-symbolic',
        urgency: MessageTray.Urgency.CRITICAL,
    }));
}

export class Timer {
    // slot: the first timer, with the quick buttons in the calendar, or the second; feel: whether timer-feel is on now;
    // flash: the pill's gentle glow
    constructor(activities, notice, {slot = 0, feel = () => false, flash = () => {}} = {}) {
        this._activities = activities;
        this._notice = notice;
        this._slot = slot;
        this._feel = feel;
        this._flash = flash;
        this.id = slot ? 'timer2' : 'timer';
        this.plate = '';
        this._seen = false;
    }

    enable() {
        this._gone = false;
        disarmOffRing();
        if (this._slot === 0)
            this._addRow();
        if (!slots[this._slot])
            return;
        if (slots[this._slot].pausedLeft !== null || slots[this._slot].end > Date.now())
            this._run();
        else
            this._ring(true);
    }

    // The timer itself goes on: only what shows it goes
    disable() {
        // Ending what shows it tells the pill that it is not seen any more, which must not start a wait on a part that is
        // gone: that one would ring beside the next, or end the timer that is started in its place
        this._gone = true;
        this._stopTick();
        Main.panel.statusArea.dateMenu?.menu.disconnectObject(this);
        this._row?.destroy();
        this._row = null;
        this._end();
        this._stopTick();
    }

    // Seconds left, from the end or from where it was paused
    get left() {
        if (!slots[this._slot])
            return 0;
        if (slots[this._slot].pausedLeft !== null)
            return slots[this._slot].pausedLeft;
        return Math.max(0, (slots[this._slot].end - Date.now()) / 1000);
    }

    get running() {
        return slots[this._slot] !== null;
    }

    get paused() {
        return slots[this._slot]?.pausedLeft !== null && slots[this._slot] !== null;
    }

    start(seconds, name = '') {
        seconds = Math.min(Math.max(1, Math.round(seconds)), LONGEST);
        slots[this._slot] = {end: Date.now() + seconds * 1000, total: seconds, started: Date.now(), pausedLeft: null, name};
        this._run();
    }

    add(seconds) {
        if (!slots[this._slot]) {
            this.start(seconds);
            return;
        }
        const left = Math.min(this.left + seconds, LONGEST);
        slots[this._slot].total += left - this.left;
        if (slots[this._slot].pausedLeft !== null)
            slots[this._slot].pausedLeft = left;
        else
            slots[this._slot].end = Date.now() + left * 1000;
        // The wait to the end, where there is no tick, moves with it
        this._run();
    }

    pause() {
        if (!slots[this._slot] || slots[this._slot].pausedLeft !== null)
            return;
        slots[this._slot].pausedLeft = this.left;
        this._stopTick();
        this._show();
    }

    resume() {
        if (!slots[this._slot] || slots[this._slot].pausedLeft === null)
            return;
        slots[this._slot].end = Date.now() + slots[this._slot].pausedLeft * 1000;
        slots[this._slot].pausedLeft = null;
        this._run();
    }

    cancel() {
        slots[this._slot] = null;
        this._stopTick();
        this._end();
        this._updateRow();
    }

    _run() {
        this._schedule();
        this._show();
    }

    // A tick a second while it can be seen; with timer-feel on and nothing to see, one wait to its end
    _schedule() {
        this._stopTick();
        if (this._gone || slots[this._slot]?.pausedLeft !== null)
            return;
        if (this._feel() && !this._seen && !this._card) {
            this._unwake = onWake(() => this._schedule());
            this._tick = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, Math.max(1, Math.ceil(this.left)), () => {
                this._tick = 0;
                // Time was added, or the wait was short of it: wait again for what is left
                if (this.left > 1)
                    this._schedule();
                else
                    this._ring(false);
                return GLib.SOURCE_REMOVE;
            });
            return;
        }
        this._tick = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 1, () => {
            if (this.left <= 0) {
                this._tick = 0;
                this._ring(false);
                return GLib.SOURCE_REMOVE;
            }
            this._show();
            return GLib.SOURCE_CONTINUE;
        });
    }

    // Told by the pill whether it can be seen now
    showing(seen) {
        if (this._seen === seen)
            return;
        this._seen = seen;
        if (slots[this._slot] && this._feel())
            this._schedule();
    }

    // timer-feel was turned on or off: the pill's content is the other one now
    feelChanged() {
        retire(this._content);
        this._content = null;
        if (slots[this._slot])
            this._run();
    }

    _stopTick() {
        if (this._tick)
            GLib.source_remove(this._tick);
        this._tick = 0;
        this._unwake?.();
        this._unwake = null;
    }

    // It rings: a sound, a notification that passes quiet, and a word in the pill
    _ring(late) {
        const timer = slots[this._slot];
        slots[this._slot] = null;
        this._stopTick();
        this._end();
        this._updateRow();
        if (!timer)
            return;
        ring(timer, late);
        this._notice?.(timer.name ? _('%s done').format(timer.name) : _('Timer done'), 'alarm-symbolic', 4);
        if (this._feel())
            this._flash();
    }

    // --- In the pill and the card ---------------------------------------------------------------------------------

    get name() {
        return slots[this._slot]?.name ?? '';
    }

    // The time left alone
    get short() {
        return clock(this.left);
    }

    get text() {
        return this.name ? `${this.name} · ${this.short}` : this.short;
    }

    get accessible() {
        return this.name
            ? _('Timer %s: %s left').format(this.name, spoken(this.left))
            : _('Timer: %s left').format(spoken(this.left));
    }

    get row() {
        return {icon: 'alarm-symbolic', text: this.name ? this.text : _('Timer · %s').format(this.text)};
    }

    // The ring round the pill, with timer-feel on
    get progress() {
        return this._feel() && slots[this._slot] ? this._fraction() : null;
    }

    _fraction() {
        return slots[this._slot] ? this.left / Math.max(1, slots[this._slot].total) : 0;
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-timer'});
            this._ring18 = this._feel()
                ? new St.Icon({icon_name: 'alarm-symbolic', icon_size: 14, y_align: Clutter.ActorAlign.CENTER})
                : new RingArea('amethystora-pill-ring', 3);
            this._label = new St.Label({style_class: 'amethystora-pill-time', y_align: Clutter.ActorAlign.CENTER});
            this._content.add_child(this._ring18);
            this._content.add_child(this._label);
        }
        return this._content;
    }

    _show() {
        if (!slots[this._slot]) {
            this._end();
            return;
        }
        this.pill();
        const paused = slots[this._slot].pausedLeft !== null;
        this._label.text = this.text;
        if (this._ring18 instanceof RingArea) {
            this._ring18.fraction = this._fraction();
            this._ring18.queue_repaint();
        }
        this._ring18.opacity = this._label.opacity = paused ? 153 : 255;
        this._activities.set(this.id, this);
        this._activities.update(this.id);
        this._card?.update();
        this._updateRow();
    }

    _end() {
        this._card = null;
        if (this._activities.get(this.id) === this)
            this._activities.set(this.id, null);
        retire(this._content);
        this._content = null;
    }

    card(api) {
        const card = new TimerCard(this, api);
        this._card = card;
        if (this._feel() && slots[this._slot] && !this._seen)
            this._schedule();
        card.connect('destroy', () => {
            if (this._card === card) {
                this._card = null;
                if (this._feel() && slots[this._slot] && !this._seen)
                    this._schedule();
            }
        });
        return card;
    }

    get total() {
        return slots[this._slot]?.total ?? 0;
    }

    get end() {
        return slots[this._slot] ? new Date(slots[this._slot].pausedLeft !== null ? Date.now() + slots[this._slot].pausedLeft * 1000 : slots[this._slot].end) : null;
    }

    // --- In the calendar ------------------------------------------------------------------------------------------

    // The last section of the calendar's column, after GNOME's weather
    _addRow() {
        const weather = Main.panel.statusArea.dateMenu?._weatherItem;
        const box = weather?.get_parent();
        if (!box) {
            console.warn('Amethystora UI: the pill did not find the calendar\'s column for the quick timer');
            return;
        }
        this._row = new St.BoxLayout({
            style_class: 'amethystora-pill-timer-section',
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
        });
        this._title = new St.Label({style_class: 'amethystora-pill-timer-title'});
        this._row.add_child(this._title);
        this._buttons = MINUTES.map(minutes => {
            const button = new St.Button({
                style_class: `amethystora-pill-timer-button${minutes === 25 ? ' amethystora-pill-timer-button-primary' : ''}`,
                label: ngettext('%d min', '%d min', minutes).format(minutes),
                can_focus: true,
            });
            button.connect('clicked', () => {
                if (slots[this._slot])
                    this.add(minutes * 60);
                else
                    this.start(minutes * 60);
                Main.panel.statusArea.dateMenu?.menu.close();
            });
            button._minutes = minutes;
            return button;
        });
        const buttons = new St.BoxLayout({style_class: 'amethystora-pill-timer-buttons'});
        for (const button of this._buttons)
            buttons.add_child(button);
        this._row.add_child(buttons);
        box.add_child(this._row);
        this._updateRow();
        // Its words are brought up to date as the calendar opens, since no tick may be running
        Main.panel.statusArea.dateMenu.menu.connectObject('open-state-changed', () => this._updateRow(), this);
    }

    _updateRow() {
        if (!this._row)
            return;
        this._title.text = slots[this._slot] ? _('Timer running: %s').format(this.short) : _('Start a timer');
        for (const button of this._buttons) {
            const minutes = button._minutes;
            button.accessible_name = slots[this._slot]
                ? ngettext('Add %d minute', 'Add %d minutes', minutes).format(minutes)
                : ngettext('Start a %d minute timer', 'Start a %d minute timer', minutes).format(minutes);
        }
    }
}

// The timer's card: the ring, the time left and when it rings, and +1 min, Pause and Cancel
const TimerCard = GObject.registerClass(
class AmethystoraPillTimerCard extends St.BoxLayout {
    _init(timer, api) {
        super._init({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        this._timer = timer;
        const head = new St.BoxLayout({style_class: 'amethystora-pill-timer-head'});
        this._ring = new RingArea('amethystora-pill-ring-large', 1.8);
        const words = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, y_align: Clutter.ActorAlign.CENTER, style_class: 'amethystora-pill-timer-words'});
        this._time = new St.Label({style_class: 'amethystora-pill-timer-big'});
        this._sub = new St.Label({style_class: 'amethystora-pill-card-text'});
        if (timer.name)
            words.add_child(new St.Label({style_class: 'amethystora-pill-card-title', text: timer.name}));
        words.add_child(this._time);
        words.add_child(this._sub);
        head.add_child(this._ring);
        head.add_child(words);
        this.add_child(head);
        this._pause = cardButton(_('Pause'), () => (timer.paused ? timer.resume() : timer.pause()), {primary: true});
        this.add_child(buttonRow([
            cardButton(_('+1 min'), () => timer.add(60)),
            this._pause,
            cardButton(_('Cancel'), () => {
                timer.cancel();
                api.close();
            }),
        ]));
        this.update();
    }

    update() {
        const timer = this._timer;
        this._time.text = timer.short;
        const minutes = Math.max(1, Math.round(timer.total / 60));
        const end = timer.end;
        this._sub.text = end
            ? _('of %s, rings at %s').format(ngettext('%d minute', '%d minutes', minutes).format(minutes), timeOfDay(end))
            : '';
        this._ring.fraction = timer.total ? timer.left / timer.total : 0;
        this._ring.queue_repaint();
        this._ring.opacity = this._time.opacity = timer.paused ? 153 : 255;
        this._pause.label = timer.paused ? _('Resume') : _('Pause');
        this._pause.accessible_name = this._pause.label;
    }
});
