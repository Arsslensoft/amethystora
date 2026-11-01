// The quick timer: four buttons at the end of the calendar's column start one, and the pill counts it down with a
// ring. When it rings: a sound, an urgent notification (which quiet and the digest let through) and "Timer done" in
// the pill. The end is kept here, outside the module, so that a timer runs on while the screen is locked (GNOME turns
// the extension off then): one that ended meanwhile rings as soon as the screen is unlocked, and says when it ended.

import Clutter from 'gi://Clutter';
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

// The timer that runs, across the module being turned off and on: when it ends (ms since the epoch), how long it was
// set for (s), when it was started (ms), and how much was left when it was paused (s, or null)
let running = null;

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
let offRing = 0;

export function armOffRing() {
    disarmOffRing();
    if (!running || running.pausedLeft !== null)
        return;
    const seconds = Math.max(1, Math.ceil((running.end - Date.now()) / 1000));
    offRing = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, seconds, () => {
        offRing = 0;
        const timer = running;
        running = null;
        ring(timer, false);
        return GLib.SOURCE_REMOVE;
    });
}

export function disarmOffRing() {
    if (offRing)
        GLib.source_remove(offRing);
    offRing = 0;
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
    global.display.get_sound_player().play_from_theme('alarm-clock-elapsed', _('The timer is done'), null);
    const source = new MessageTray.Source({title: _('Timer'), iconName: 'alarm-symbolic'});
    Main.messageTray.add(source);
    source.addNotification(new MessageTray.Notification({
        source,
        title: _('The timer is done'),
        body,
        iconName: 'alarm-symbolic',
        urgency: MessageTray.Urgency.CRITICAL,
    }));
}

export class Timer {
    constructor(activities, notice) {
        this._activities = activities;
        this._notice = notice;
        this.id = 'timer';
        this.plate = '';
    }

    enable() {
        disarmOffRing();
        this._addRow();
        if (!running)
            return;
        if (running.pausedLeft !== null || running.end > Date.now())
            this._run();
        else
            this._ring(true);
    }

    // The timer itself goes on: only what shows it goes
    disable() {
        this._stopTick();
        this._row?.destroy();
        this._row = null;
        this._end();
    }

    // Seconds left, from the end or from where it was paused
    get left() {
        if (!running)
            return 0;
        if (running.pausedLeft !== null)
            return running.pausedLeft;
        return Math.max(0, (running.end - Date.now()) / 1000);
    }

    get running() {
        return running !== null;
    }

    get paused() {
        return running?.pausedLeft !== null && running !== null;
    }

    start(seconds) {
        seconds = Math.min(Math.max(1, Math.round(seconds)), LONGEST);
        running = {end: Date.now() + seconds * 1000, total: seconds, started: Date.now(), pausedLeft: null};
        this._run();
    }

    add(seconds) {
        if (!running) {
            this.start(seconds);
            return;
        }
        const left = Math.min(this.left + seconds, LONGEST);
        running.total += left - this.left;
        if (running.pausedLeft !== null)
            running.pausedLeft = left;
        else
            running.end = Date.now() + left * 1000;
        this._show();
    }

    pause() {
        if (!running || running.pausedLeft !== null)
            return;
        running.pausedLeft = this.left;
        this._stopTick();
        this._show();
    }

    resume() {
        if (!running || running.pausedLeft === null)
            return;
        running.end = Date.now() + running.pausedLeft * 1000;
        running.pausedLeft = null;
        this._run();
    }

    cancel() {
        running = null;
        this._stopTick();
        this._end();
        this._updateRow();
    }

    _run() {
        this._stopTick();
        if (running?.pausedLeft === null) {
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
        this._show();
    }

    _stopTick() {
        if (this._tick)
            GLib.source_remove(this._tick);
        this._tick = 0;
    }

    // It rings: a sound, a notification that passes quiet, and a word in the pill
    _ring(late) {
        const timer = running;
        running = null;
        this._stopTick();
        this._end();
        this._updateRow();
        if (!timer)
            return;
        ring(timer, late);
        this._notice?.(_('Timer done'), 'alarm-symbolic', 4);
    }

    // --- In the pill and the card ---------------------------------------------------------------------------------

    get text() {
        return clock(this.left);
    }

    get accessible() {
        return _('Timer: %s left').format(spoken(this.left));
    }

    get row() {
        return {icon: 'alarm-symbolic', text: _('Timer · %s').format(this.text)};
    }

    _fraction() {
        return running ? this.left / Math.max(1, running.total) : 0;
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-timer'});
            this._ring18 = new RingArea('amethystora-pill-ring', 3);
            this._label = new St.Label({style_class: 'amethystora-pill-time', y_align: Clutter.ActorAlign.CENTER});
            this._content.add_child(this._ring18);
            this._content.add_child(this._label);
        }
        return this._content;
    }

    _show() {
        if (!running) {
            this._end();
            return;
        }
        this.pill();
        const paused = running.pausedLeft !== null;
        this._label.text = this.text;
        this._ring18.fraction = this._fraction();
        this._ring18.queue_repaint();
        this._ring18.opacity = this._label.opacity = paused ? 153 : 255;
        this._activities.set('timer', this);
        this._activities.update('timer');
        this._card?.update();
        this._updateRow();
    }

    _end() {
        this._card = null;
        if (this._activities.get('timer') === this)
            this._activities.set('timer', null);
        retire(this._content);
        this._content = null;
    }

    card(api) {
        const card = new TimerCard(this, api);
        this._card = card;
        card.connect('destroy', () => {
            if (this._card === card)
                this._card = null;
        });
        return card;
    }

    get total() {
        return running?.total ?? 0;
    }

    get end() {
        return running ? new Date(running.pausedLeft !== null ? Date.now() + running.pausedLeft * 1000 : running.end) : null;
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
                if (running)
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
    }

    _updateRow() {
        if (!this._row)
            return;
        this._title.text = running ? _('Timer running: %s').format(this.text) : _('Start a timer');
        for (const button of this._buttons) {
            const minutes = button._minutes;
            button.accessible_name = running
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
        this._time.text = timer.text;
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
