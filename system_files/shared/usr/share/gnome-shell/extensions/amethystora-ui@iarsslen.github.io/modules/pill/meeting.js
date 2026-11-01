// A meeting about to start, in the pill: five minutes before an event of today that carries a video-call link, the pill
// says "Design review in 4 min", and its card has the times, the calendar, the link and Join. The link opens in the
// browser only when Join is clicked; Later puts it away until the event starts, and for good once it has.
//
// The events are GNOME's own: the calendar menu's event source, which GNOME's calendar server fills (the same events
// the calendar menu lists), read again when it says it changed; nothing here asks the server for another range, which
// would move the calendar menu's. The server sends titles and times only, so the link is asked of the calendar itself
// by the image's helper (/usr/libexec/amethystora-meeting-link), once for each event as it comes within five minutes,
// the event's id on its standard input. One timeout waits for the next moment anything changes (an event's five
// minutes before, the next minute of the countdown, the end of its showing, midnight); nothing else runs.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';

import {buttonRow, cardButton, retire} from './clock.js';
import {plain, shownAddress, timeOfDay} from './text.js';

const HELPER = '/usr/libexec/amethystora-meeting-link';
// Seconds: shown this long before an event starts, and at most this long after
const LEAD = 5 * 60;
const AFTER = 10 * 60;
// The helper's answer is read at most this long (it stops itself before)
const ASK_MS = 15000;
// Answers kept, and how many at most (a day's events)
const MOST = 200;

Gio._promisify(Gio.Subprocess.prototype, 'communicate_utf8_async');

// What Later and Join put away (an event's key, until when), kept outside the part so that a screen lock, which turns
// the extension off, does not bring a meeting back; an entry goes once its event is a day old
const dismissed = new Map();

// The link as the helper found it, checked again: one https address, shown without its scheme
function checked(link) {
    if (typeof link !== 'string' || link.length > 2048 || !/^https:\/\/[^\s]+$/u.test(link))
        return null;
    const shown = shownAddress(link);
    return shown ? {url: link, shown} : null;
}

export class Meetings {
    constructor(activities, notice) {
        this._activities = activities;
        this._notice = notice;
    }

    enable() {
        this._links = new Map();
        this._dismissed = dismissed;
        this._cancellable = new Gio.Cancellable();
        this._timer = 0;
        this._shown = null;
        // GNOME makes a new event source when the session's mode changes (the screen locked and unlocked): picked again
        Main.sessionMode.connectObject('updated', () => this._follow(), this);
        this._follow();
    }

    disable() {
        Main.sessionMode.disconnectObject(this);
        this._source?.disconnectObject(this);
        this._source = null;
        this._cancellable?.cancel();
        this._cancellable = null;
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        this._end();
        this._links = null;
        this._dismissed = null;
    }

    _follow() {
        const source = Main.panel.statusArea.dateMenu?._eventSource ?? null;
        if (source !== this._source) {
            this._source?.disconnectObject(this);
            this._source = source;
            source?.connectObject('changed', () => this._scan(), this);
        }
        this._scan();
    }

    // Today's events still to come or just begun, from the event source as it is now
    _events(now) {
        const source = this._source;
        if (!source || typeof source.getEvents !== 'function' || source.isLoading)
            return [];
        const end = new Date(now);
        end.setHours(23, 59, 59, 999);
        let events = [];
        try {
            events = source.getEvents(new Date(now - AFTER * 1000), end);
        } catch (e) {
            console.debug(`Amethystora UI: the calendar's events could not be read: ${e.message}`);
            return [];
        }
        return events.filter(event => {
            const start = event.date?.getTime?.();
            const stop = event.end?.getTime?.();
            if (!Number.isFinite(start) || !Number.isFinite(stop) || typeof event.id !== 'string')
                return false;
            // A day-long event is no meeting to join
            const allDay = stop - start >= 86400000 && event.date.getHours() === 0 && event.date.getMinutes() === 0;
            return !allDay && start >= now - AFTER * 1000 && start <= end.getTime();
        }).sort((a, b) => a.date - b.date);
    }

    _scan() {
        if (!this._links)
            return;
        const now = Date.now();
        const midnight = new Date(now);
        midnight.setHours(24, 0, 1, 0);
        let wake = midnight.getTime();
        let chosen = null;
        for (const [key, {start}] of dismissed) {
            if (start < now - 86400000)
                dismissed.delete(key);
        }
        for (const event of this._events(now)) {
            const start = event.date.getTime();
            const until = Math.min(start + AFTER * 1000, event.end.getTime());
            const key = `${event.id}@${start}`;
            if (now < start - LEAD * 1000) {
                wake = Math.min(wake, start - LEAD * 1000);
                continue;
            }
            if (now >= until)
                continue;
            const away = this._dismissed.get(key)?.until ?? 0;
            if (away > now) {
                wake = Math.min(wake, away);
                continue;
            }
            if (!this._links.has(key)) {
                this._ask(event, key);
                continue;
            }
            const link = this._links.get(key);
            if (!link || link === 'asking')
                continue;
            chosen = {event, key, link, start, until};
            wake = Math.min(wake, until);
            break;
        }
        if (chosen) {
            // The countdown's next minute
            const left = chosen.start - now;
            if (left > 0)
                wake = Math.min(wake, chosen.start - (Math.ceil(left / 60000) - 1) * 60000);
            this._show(chosen);
        } else {
            this._end();
        }
        this._arm(wake - now);
    }

    _arm(ms) {
        if (this._timer)
            GLib.source_remove(this._timer);
        const seconds = Math.min(Math.max(1, Math.ceil(ms / 1000)), 24 * 3600);
        this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, seconds, () => {
            this._timer = 0;
            this._scan();
            return GLib.SOURCE_REMOVE;
        });
    }

    // The link of an event, asked of the helper once; nothing is shown until it answers
    async _ask(event, key) {
        if (this._links.size >= MOST)
            this._links.clear();
        this._links.set(key, 'asking');
        const cancellable = this._cancellable;
        let answer = null;
        try {
            const process = Gio.Subprocess.new([HELPER],
                Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
            const stop = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ASK_MS, () => {
                process.force_exit();
                return GLib.SOURCE_REMOVE;
            });
            let out = '';
            try {
                [out] = await process.communicate_utf8_async(`${event.id}\n`, cancellable);
            } finally {
                GLib.source_remove(stop);
            }
            const line = out?.split('\n').find(text => text.trim()) ?? '';
            if (line) {
                const found = JSON.parse(line);
                const link = checked(found?.link);
                if (link)
                    answer = {...link, calendar: plain(found?.calendar ?? '', 40)};
            }
        } catch (e) {
            if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                return;
            console.debug(`Amethystora UI: the meeting's link could not be read: ${e.message}`);
        }
        if (!this._links || cancellable.is_cancelled())
            return;
        this._links.set(key, answer);
        this._scan();
    }

    _show(chosen) {
        if (this._shown?.key === chosen.key) {
            this._shown.update(chosen);
            this._activities.update('meeting');
            return;
        }
        this._end();
        this._shown = new MeetingActivity(chosen, this);
        this._activities.set('meeting', this._shown);
    }

    _end() {
        const shown = this._shown;
        this._shown = null;
        if (shown && this._activities.get('meeting') === shown)
            this._activities.set('meeting', null);
        shown?.destroy();
    }

    join(activity) {
        const link = checked(activity.link.url);
        this._dismissed.set(activity.key, {until: Infinity, start: activity.start});
        this._scan();
        if (!link)
            return;
        try {
            Gio.AppInfo.launch_default_for_uri(link.url, global.create_app_launch_context(0, -1));
            this._notice(_('Opened in the browser'), 'web-browser-symbolic', 3);
        } catch (e) {
            console.warn(`Amethystora UI: the meeting's link could not be opened: ${e.message}`);
        }
    }

    // Until it starts, then for good
    later(activity) {
        this._dismissed.set(activity.key, {until: Date.now() < activity.start ? activity.start : Infinity, start: activity.start});
        this._scan();
    }

    // For the suites and the harness
    state() {
        return this._shown ? {key: this._shown.key, text: this._shown.text, link: this._shown.link.shown} : null;
    }

    // For the suites and the harness: an event as GNOME's source would give it, with the link the helper would find
    showForTest(event, link) {
        const start = event.date.getTime();
        const key = `${event.id}@${start}`;
        const found = checked(link);
        this._links.set(key, found ? {...found, calendar: plain(event.calendar ?? '', 40)} : null);
        if (!found)
            return false;
        this._show({event, key, link: this._links.get(key), start, until: Math.min(start + AFTER * 1000, event.end.getTime())});
        return true;
    }
}

class MeetingActivity {
    constructor(chosen, meetings) {
        this.id = 'meeting';
        this.plate = 'link';
        this._meetings = meetings;
        this.update(chosen);
    }

    update(chosen) {
        this.key = chosen.key;
        this.start = chosen.start;
        this.link = chosen.link;
        this._title = plain(chosen.event.summary ?? '', 60) || _('A meeting');
        this._end = chosen.event.end;
        this._date = chosen.event.date;
        if (this._label)
            this._label.text = this.text;
    }

    get text() {
        const left = Math.ceil((this.start - Date.now()) / 60000);
        if (left <= 0)
            return _('%s now').format(this._title);
        return ngettext('%s in %d min', '%s in %d min', left).format(this._title, left);
    }

    get accessible() {
        return `${this.text}. ${_('Join')}`;
    }

    get row() {
        return {icon: 'camera-web-symbolic', text: this.text};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-meeting'});
            this._content.add_child(new St.Icon({
                icon_name: 'camera-web-symbolic', icon_size: 12, y_align: Clutter.ActorAlign.CENTER,
            }));
            this._label = new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER});
            this._label.clutter_text.use_markup = false;
            this._label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
            this._content.add_child(this._label);
        }
        return this._content;
    }

    card(api) {
        const body = new St.BoxLayout({
            style_class: 'amethystora-pill-card-body amethystora-pill-meeting-card',
            orientation: Clutter.Orientation.VERTICAL,
        });
        const title = new St.Label({style_class: 'amethystora-pill-card-label amethystora-pill-meeting-title', text: this._title});
        title.clutter_text.use_markup = false;
        title.clutter_text.line_wrap = true;
        body.add_child(title);
        const times = _('%s to %s').format(timeOfDay(this._date), timeOfDay(this._end));
        const when = new St.Label({
            style_class: 'amethystora-pill-card-text amethystora-pill-meeting-when',
            text: this.link.calendar ? `${times} · ${this.link.calendar}` : times,
        });
        when.clutter_text.use_markup = false;
        body.add_child(when);
        const address = new St.Label({
            style_class: 'amethystora-pill-card-text amethystora-pill-meeting-link',
            text: this.link.shown,
        });
        address.clutter_text.use_markup = false;
        address.clutter_text.ellipsize = Pango.EllipsizeMode.MIDDLE;
        address.set_text_direction(Clutter.TextDirection.LTR);
        body.add_child(address);
        const row = buttonRow([
            cardButton(_('Later'), () => {
                api.close();
                this._meetings.later(this);
            }),
            cardButton(_('Join'), () => {
                api.close();
                this._meetings.join(this);
            }, {primary: true}),
        ]);
        row.x_align = Clutter.ActorAlign.END;
        body.add_child(row);
        return body;
    }

    destroy() {
        retire(this._content);
        this._content = null;
        this._label = null;
    }
}
