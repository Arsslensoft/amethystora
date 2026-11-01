// Later and mute: a notification that cannot be acted on now comes back when it can. Later on a banner takes it away
// for an hour, until this evening, until tomorrow morning or until the screen is next unlocked; the banner then comes
// back as it was, with its own buttons, and rings once. Mute holds one app's banners for an hour, as quiet holds them
// (calls, alarms and urgent ones still pass). What waits is never hidden: the calendar's list has a Later group with
// when each comes back, and a Muted group with what each mute holds.
//
// A snoozed notification stays GNOME's: it stays in GNOME's own list, marked seen, so that its buttons keep working
// from there, and coming back is GNOME's own (a notification marked unseen again asks for its banner). Nothing is
// written anywhere: what waits is in memory, outside the class, so that the lock, which turns the extension off and on
// again, keeps it; a session that ends forgets it, as GNOME forgets the notifications themselves. One timeout exists
// while something waits, for the next one due, and logind's word that the machine woke up re-checks the times.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {onKey} from './clock.js';
import {plain, timeOfDay} from './text.js';

const HOUR = 3600 * 1000;
const TIME = /^([01][0-9]|2[0-3]):[0-5][0-9]$/;
const EVENING = '18:00';
const MORNING = '09:00';
// How long the pill says what was done, with its Undo (s), and the longest the one timeout waits (s)
const SAID = 5;
const QUARTER = 900;

// What waits, kept outside the class: the lock turns the extension off and on again, and must not lose it.
// later: notification -> {at (ms since the epoch, 0 for the next unlock), unlock}
// muted: an app's key -> {name, gicon, until (ms), held: Set of notifications}
const store = {later: new Map(), muted: new Map(), locked: false};

// An app's key, name and icon for a notification's source; null for a source that names no app
export function appOf(source) {
    if (!source)
        return null;
    // GNOME's policy for a notification that names no app is 'generic', shared by all of them: not an app's id
    const policyId = source.policy?.id && source.policy.id !== 'generic' ? source.policy.id : null;
    const id = source._appId ?? source.app?.get_id?.() ?? policyId;
    const app = id ? Shell.AppSystem.get_default().lookup_app(id.endsWith('.desktop') ? id : `${id}.desktop`) : null;
    const name = plain(app?.get_name?.() ?? source.title, 40);
    if (!name)
        return null;
    return {key: id ? `app:${id.replace(/\.desktop$/, '')}` : `name:${name}`, name, gicon: app?.get_icon?.() ?? source.icon ?? null};
}

// "HH:MM" today (or the day `days` on) as ms since the epoch
function at(time, days = 0) {
    const [h, m] = time.split(':').map(Number);
    const date = new Date();
    date.setDate(date.getDate() + days);
    date.setHours(h, m, 0, 0);
    return date.getTime();
}

// Whether GNOME still keeps this notification (a reference to a destroyed one is never touched)
function alive(notification) {
    return Main.messageTray.getSources().some(source => source.notifications?.includes(notification));
}

export class Later {
    // changed(): a mute began or ended, so the hold looks again at whether it holds; ended(entry, byHand): a mute
    // ended, with what it held
    constructor(settings, notice, {changed, ended}) {
        this._settings = settings;
        this._notice = notice;
        this._changed = changed;
        this._ended = ended;
    }

    enable() {
        this._decorated = null;
        // Forget what GNOME no longer keeps, and follow what it still does
        for (const notification of [...store.later.keys()]) {
            if (alive(notification))
                this._follow(notification);
            else
                store.later.delete(notification);
        }
        for (const entry of store.muted.values()) {
            for (const notification of [...entry.held]) {
                if (!alive(notification))
                    entry.held.delete(notification);
            }
        }
        // Back from the lock: what waited for the unlock comes back once the extension has started, since GNOME shows
        // its banner at once and the tray is not ready for one while the unlock is still enabling the extensions
        if (store.locked) {
            this._unlockedId = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                this._unlockedId = 0;
                store.locked = false;
                for (const [notification, item] of [...store.later]) {
                    if (item.unlock)
                        this._return(notification);
                }
                this._tick();
                return GLib.SOURCE_REMOVE;
            });
        }
        this._list = new LaterList(this);
        this._list.enable();
        this._tick();
    }

    // forget: the key was turned off, or the extension off without a lock: nothing waits any more
    disable(forget = false) {
        if (this._unlockedId)
            GLib.source_remove(this._unlockedId);
        this._unlockedId = 0;
        this._undecorate();
        this._disarm();
        this._list?.disable();
        this._list = null;
        for (const notification of store.later.keys())
            notification.disconnectObject(this);
        if (forget || !Main.sessionMode.isLocked) {
            // Snoozed ones stay in the list, seen; held ones stay in the list too: nothing is shown in a burst
            store.later.clear();
            store.muted.clear();
            store.locked = false;
        } else {
            store.locked = true;
        }
    }

    // --- The times ------------------------------------------------------------------------------------------------

    _times() {
        const list = this._settings.get_strv('later-times');
        const evening = TIME.test(list[0] ?? '') ? list[0] : EVENING;
        const morning = TIME.test(list[1] ?? '') ? list[1] : MORNING;
        return {evening, morning};
    }

    // The choices Later offers now, each {id, label}
    choices() {
        const {evening} = this._times();
        const list = [{id: 'hour', label: _('In an hour')}];
        if (Date.now() < at(evening))
            list.push({id: 'evening', label: _('This evening')});
        list.push({id: 'morning', label: _('Tomorrow morning')});
        list.push({id: 'unlock', label: _('When I next unlock')});
        return list;
    }

    _due(kind) {
        const {evening, morning} = this._times();
        switch (kind) {
        case 'hour':
            return Date.now() + HOUR;
        case 'evening':
            return at(evening);
        case 'morning':
            // Before the morning's time (Later at one at night), that same morning
            return at(morning, Date.now() < at(morning) ? 0 : 1);
        default:
            return 0;
        }
    }

    // "Back at 18:00", "Back tomorrow at 09:00", "Back when you next unlock"
    static backText(item) {
        if (item.unlock)
            return _('Back when you next unlock');
        const date = new Date(item.at);
        const today = new Date();
        if (date.toDateString() === today.toDateString())
            return _('Back at %s').format(timeOfDay(date));
        return _('Back tomorrow at %s').format(timeOfDay(date));
    }

    // --- Snoozing -------------------------------------------------------------------------------------------------

    snooze(notification, kind) {
        if (!notification || !alive(notification))
            return;
        const item = {at: this._due(kind), unlock: kind === 'unlock'};
        store.later.set(notification, item);
        this._follow(notification);
        // Seen: no banner, no count on the dot; and off the queue, or out of sight if it is the banner showing
        notification.acknowledged = true;
        const tray = Main.messageTray;
        if (Array.isArray(tray._notificationQueue) && tray._notificationQueue.includes(notification)) {
            tray._notificationQueue = tray._notificationQueue.filter(n => n !== notification);
            tray.emit('queue-changed');
        }
        if (tray._notification === notification)
            tray._expireNotification?.();
        this._tick();
        this._notice(Later.backText(item), 'alarm-symbolic', SAID, '', null,
            {label: _('Undo'), run: () => this.unsnooze(notification)});
    }

    // Undo: it waits no more, and stays in the list as it is
    unsnooze(notification) {
        if (!store.later.delete(notification))
            return;
        notification.disconnectObject(this);
        this._tick();
    }

    _follow(notification) {
        notification.disconnectObject(this);
        notification.connectObject(
            'destroy', () => {
                store.later.delete(notification);
                this._tick();
            },
            // Its app updated it while banners are held (GNOME marks it unseen): it waits no more
            'notify::acknowledged', () => {
                if (!notification.acknowledged && store.later.delete(notification))
                    this._tick();
            }, this);
    }

    // It comes back: marked unseen, which makes GNOME ask for its banner again (and quiet or a focus profile hold it
    // as they hold any)
    _return(notification) {
        store.later.delete(notification);
        notification.disconnectObject(this);
        if (alive(notification))
            notification.acknowledged = false;
    }

    // Now, from the list
    returnNow(notification) {
        if (!store.later.has(notification))
            return;
        this._return(notification);
        this._tick();
    }

    get waiting() {
        return [...store.later].sort((a, b) => (a[1].unlock - b[1].unlock) || (a[1].at - b[1].at));
    }

    // --- Muting ---------------------------------------------------------------------------------------------------

    mute(notification) {
        const app = appOf(notification?.source);
        if (!app)
            return;
        const until = Date.now() + HOUR;
        const entry = store.muted.get(app.key) ?? {name: app.name, gicon: app.gicon, held: new Set()};
        entry.until = until;
        store.muted.set(app.key, entry);
        this._tick();
        this._changed?.();
        this._notice(_('%s muted until %s').format(app.name, timeOfDay(new Date(until))), 'notifications-disabled-symbolic',
            SAID, '', null, {label: _('Undo'), run: () => this.unmute(app.key, false)});
    }

    // The mute that holds this notification's app, or null
    muteOf(notification) {
        const app = appOf(notification?.source);
        const entry = app ? store.muted.get(app.key) : null;
        return entry && entry.until > Date.now() ? entry : null;
    }

    get muting() {
        return store.muted.size > 0;
    }

    get muted() {
        return [...store.muted].sort((a, b) => a[1].until - b[1].until);
    }

    // A banner the mute held
    held(notification) {
        const entry = this.muteOf(notification);
        if (!entry || entry.held.has(notification))
            return;
        entry.held.add(notification);
        this._list?.refresh();
    }

    // The held notification went away (dismissed, replaced)
    forget(notification) {
        for (const entry of store.muted.values())
            entry.held.delete(notification);
    }

    // byHand: Unmute (the held ones shown as one card); else the hour is over or Undo (the pill says how many wait)
    unmute(key, byHand) {
        if (this._end(key, byHand))
            this._tick();
    }

    _end(key, byHand) {
        const entry = store.muted.get(key);
        if (!entry)
            return false;
        store.muted.delete(key);
        entry.held = new Set([...entry.held].filter(alive));
        this._ended?.(entry, byHand);
        this._changed?.();
        return true;
    }

    // --- The clock ------------------------------------------------------------------------------------------------

    // Everything due comes back, every mute over ends, and the one timeout is armed for the next
    _tick() {
        const now = Date.now();
        for (const [notification, item] of [...store.later]) {
            if (!item.unlock && item.at <= now)
                this._return(notification);
        }
        for (const [key, entry] of [...store.muted]) {
            if (entry.until <= now)
                this._end(key, false);
        }
        this._list?.refresh();
        const times = [...[...store.later.values()].filter(i => !i.unlock).map(i => i.at),
            ...[...store.muted.values()].map(e => e.until)];
        this._disarm();
        if (times.length === 0 && store.later.size === 0)
            return;
        if (times.length > 0) {
            // At most a quarter of an hour, so that a change of the clock (summer time, another time zone) is seen
            const seconds = Math.min(QUARTER, Math.max(1, Math.ceil((Math.min(...times) - now) / 1000)));
            this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, seconds, () => {
                this._timer = 0;
                this._tick();
                return GLib.SOURCE_REMOVE;
            });
        }
        // A sleep stops GLib's clock: the times are looked at again on waking
        this._sleep = Gio.DBus.system.signal_subscribe('org.freedesktop.login1', 'org.freedesktop.login1.Manager',
            'PrepareForSleep', '/org/freedesktop/login1', null, Gio.DBusSignalFlags.NONE,
            (connection, sender, path, iface, signal, params) => {
                if (!params.deepUnpack()[0])
                    this._tick();
            });
    }

    _disarm() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        if (this._sleep)
            Gio.DBus.system.signal_unsubscribe(this._sleep);
        this._sleep = 0;
    }

    // --- The banner's buttons -------------------------------------------------------------------------------------

    // Later (and, with room, Mute App for an hour) after the app's own buttons in the banner GNOME just made. Ours are
    // in a box of their own beside GNOME's row, so that GNOME's count of an app's buttons (three at most) never counts
    // them; a row never shows more than three buttons in all: with two of the app's, Mute is among the choices, and
    // with three, the choices show at once in place of Later. The app may add or take away buttons while its banner
    // shows: they are arranged again.
    decorate(banner) {
        const notification = banner?.notification;
        // A snoozed one shown again: its app updated it (GNOME shows the banner at once, and marks it seen again before
        // the notify handler of _follow can tell): it waits no more
        if (notification && store.later.delete(notification)) {
            notification.disconnectObject(this);
            this._tick();
        }
        if (!notification || notification.isTransient || notification.forFeedback || this._decorated?.banner === banner)
            return;
        this._undecorate();
        let row = banner._buttonBox;
        const made = !row;
        if (made) {
            // As GNOME makes it for an app's first button
            row = new St.BoxLayout({x_expand: true, style_class: 'notification-buttons-bin'});
            banner._buttonBox = row;
            global.focus_manager.add_group(row);
        } else {
            banner.setActionArea(null);
        }
        const column = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
            style_class: 'amethystora-pill-later-actions',
        });
        const line = new St.BoxLayout({x_expand: true, style_class: 'notification-buttons-bin amethystora-pill-later-line'});
        line.add_child(row);
        const ours = new St.BoxLayout({x_expand: true, style_class: 'notification-buttons-bin'});
        global.focus_manager.add_group(ours);
        line.add_child(ours);
        column.add_child(line);
        const choices = new St.Widget({
            style_class: 'amethystora-pill-later-choices',
            layout_manager: new Clutter.FlowLayout({column_spacing: 6, row_spacing: 6, homogeneous: false,
                snap_to_grid: false}),
            x_expand: true,
            visible: false,
        });
        column.add_child(choices);
        banner.setActionArea(column);

        const app = appOf(notification.source);
        const decorated = {banner, notification, row, line, ours, column, choices, made, open: false};
        this._decorated = decorated;
        decorated.later = this._button(_('Later'), () => this._toggleChoices());
        decorated.later.add_style_class_name('amethystora-pill-later-open');
        ours.add_child(decorated.later);
        const muteLabel = app ? _('Mute %s for an hour').format(app.name) : null;
        if (muteLabel) {
            decorated.muteRow = this._button(muteLabel, () => this.mute(notification));
            ours.add_child(decorated.muteRow);
        }
        for (const choice of this.choices())
            choices.add_child(this._button(choice.label, () => this.snooze(notification, choice.id), false));
        if (muteLabel) {
            decorated.muteChoice = this._button(muteLabel, () => this.mute(notification), false);
            choices.add_child(decorated.muteChoice);
        }
        this._arrange(decorated);
        notification.connectObject(
            'action-added', () => this._arrange(decorated),
            'action-removed', () => this._arrange(decorated), decorated);
        // L on a banner that has the keyboard: later, in an hour
        decorated.unkey = onKey(banner, symbol => {
            if (symbol !== Clutter.KEY_l && symbol !== Clutter.KEY_L)
                return false;
            this.snooze(notification, 'hour');
            return true;
        });
        banner.connectObject('destroy', () => {
            if (this._decorated === decorated)
                this._decorated = null;
        }, this);
    }

    // Where each of ours goes for the app's own buttons now
    _arrange(decorated) {
        const own = decorated.row.get_n_children();
        decorated.row.visible = own > 0;
        decorated.later.visible = own < 3;
        if (decorated.muteRow)
            decorated.muteRow.visible = own <= 1;
        if (decorated.muteChoice)
            decorated.muteChoice.visible = own > 1;
        decorated.ours.visible = own < 3;
        decorated.choices.visible = decorated.open || own >= 3;
    }

    _button(label, action, expand = true) {
        const button = new St.Button({
            style_class: 'notification-button amethystora-pill-later-button',
            label,
            can_focus: true,
            x_expand: expand,
            accessible_name: label,
        });
        button.clutter_text?.set_use_markup?.(false);
        button.connect('clicked', () => action());
        return button;
    }

    _toggleChoices() {
        const decorated = this._decorated;
        if (!decorated)
            return;
        decorated.open = !decorated.open;
        this._arrange(decorated);
        if (decorated.open)
            decorated.later.add_style_pseudo_class('checked');
        else
            decorated.later.remove_style_pseudo_class('checked');
        // The banner keeps its place while the person chooses
        if (decorated.open)
            decorated.choices.get_first_child()?.grab_key_focus();
    }

    // The banner showing loses our buttons: the module stops, or another banner comes. GNOME's own row goes back as
    // its action area, as it was.
    _undecorate() {
        const decorated = this._decorated;
        this._decorated = null;
        if (!decorated)
            return;
        const {banner, notification, row, line, column, made} = decorated;
        decorated.unkey?.();
        banner.disconnectObject(this);
        notification.disconnectObject(decorated);
        if (row.get_parent() === line)
            line.remove_child(row);
        row.visible = true;
        const keep = row.get_n_children() > 0;
        banner.setActionArea(keep ? row : null);
        if (made && !keep) {
            if (banner._buttonBox === row)
                banner._buttonBox = null;
            row.destroy();
        }
        column.destroy();
    }

    // What the banner showing has of ours, as a person sees it: the buttons in the row and the choices shown
    _shownLabels(box) {
        return box.visible ? box.get_children().filter(b => b.visible).map(b => b.label) : [];
    }

    // --- For the suites -------------------------------------------------------------------------------------------

    find(title) {
        for (const source of Main.messageTray.getSources()) {
            const found = source.notifications?.find(n => n.title === title);
            if (found)
                return found;
        }
        return null;
    }

    snoozeForTest(title, kind = 'hour') {
        const notification = this.find(title);
        this.snooze(notification, kind);
        return store.later.has(notification);
    }

    muteForTest(title) {
        this.mute(this.find(title));
        return this.muting;
    }

    // Its time is now: it comes back as at its time
    dueForTest(title) {
        const notification = this.find(title);
        const item = store.later.get(notification);
        if (!item)
            return false;
        item.at = Date.now() - 1;
        item.unlock = false;
        this._tick();
        return !store.later.has(notification);
    }

    endMutesForTest() {
        for (const entry of store.muted.values())
            entry.until = Date.now() - 1;
        this._tick();
    }

    state() {
        return {
            waiting: this.waiting.map(([n, item]) => ({title: n.title, back: Later.backText(item), unlock: item.unlock})),
            muted: this.muted.map(([key, e]) => ({key, name: e.name, held: e.held.size, until: e.until})),
            // The app's own buttons, ours in the row, and the choices (with whether they show)
            own: this._decorated ? this._decorated.row.get_children().map(b => b.label) : [],
            banner: this._decorated ? this._shownLabels(this._decorated.ours) : [],
            choices: this._decorated ? this._decorated.choices.get_children().filter(b => b.visible).map(b => b.label) : [],
            choicesShown: this._decorated?.choices.visible ?? false,
            list: this._list?.shown ?? false,
            listTexts: this._list?.texts() ?? [],
            timer: !!this._timer,
        };
    }

    // Opens the banner's Later choices, for the suites
    openChoicesForTest() {
        if (!this._decorated?.open)
            this._toggleChoices();
        return this._decorated?.open ?? false;
    }
}

// The two groups above GNOME's own list in the calendar menu: Later, with when each comes back, and Muted, with what
// each mute holds. Made again each time the menu opens, and while it is open whenever what waits changes.
class LaterList {
    constructor(later) {
        this._later = later;
    }

    enable() {
        const dateMenu = Main.panel.statusArea.dateMenu;
        const scroll = dateMenu?._messageList?._scrollView;
        const parent = scroll?.get_parent();
        if (!parent)
            return;
        this._dateMenu = dateMenu;
        this._box = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'amethystora-pill-later-list',
            x_expand: true,
            visible: false,
        });
        parent.insert_child_below(this._box, scroll);
        dateMenu.menu.connectObject('open-state-changed', (menu, open) => {
            if (open)
                this.refresh();
            else
                this._box?.destroy_all_children();
        }, this);
        if (dateMenu.menu.isOpen)
            this.refresh();
    }

    disable() {
        this._dateMenu?.menu.disconnectObject(this);
        this._dateMenu = null;
        this._box?.destroy();
        this._box = null;
    }

    get shown() {
        return !!this._box?.visible;
    }

    // Every word the two groups show, for the suites
    texts() {
        const texts = [];
        const visit = actor => {
            if (actor instanceof St.Label && actor.text)
                texts.push(actor.text);
            else if (actor instanceof St.Button && actor.label)
                texts.push(actor.label);
            for (const child of actor.get_children())
                visit(child);
        };
        if (this._box?.visible)
            visit(this._box);
        return texts;
    }

    refresh() {
        const box = this._box;
        if (!box)
            return;
        box.destroy_all_children();
        const waiting = this._later.waiting;
        const muted = this._later.muted;
        box.visible = waiting.length + muted.length > 0;
        if (!box.visible || !this._dateMenu?.menu.isOpen)
            return;
        if (waiting.length > 0) {
            // Translators: the heading of the notifications that wait, with how many
            box.add_child(this._heading(_('Later · %d').format(waiting.length)));
            for (const [notification, item] of waiting) {
                const gicon = notification.gicon ?? notification.source?.icon ?? null;
                box.add_child(this._row(gicon, plain(notification.title, 80), Later.backText(item), _('Now'),
                    () => this._later.returnNow(notification)));
            }
        }
        if (muted.length > 0) {
            box.add_child(this._heading(_('Muted')));
            for (const [key, entry] of muted) {
                const until = timeOfDay(new Date(entry.until));
                const held = entry.held.size;
                const text = held > 0
                    ? ngettext('Muted until %s · %d held', 'Muted until %s · %d held', held).format(until, held)
                    : _('Muted until %s').format(until);
                box.add_child(this._row(entry.gicon, entry.name, text, _('Unmute'), () => this._later.unmute(key, true)));
            }
        }
    }

    _heading(text) {
        // In capitals, as the board's small headings
        return new St.Label({style_class: 'amethystora-pill-later-heading', text: text.toLocaleUpperCase()});
    }

    _row(gicon, title, text, action, run) {
        const row = new St.BoxLayout({style_class: 'amethystora-pill-later-row', x_expand: true});
        row.add_child(new St.Icon({
            style_class: 'amethystora-pill-later-icon',
            icon_size: 26,
            ...(gicon ? {gicon} : {icon_name: 'application-x-executable-symbolic'}),
            y_align: Clutter.ActorAlign.CENTER,
        }));
        const words = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true,
            y_align: Clutter.ActorAlign.CENTER});
        const first = new St.Label({style_class: 'amethystora-pill-later-title', text: title});
        first.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        words.add_child(first);
        words.add_child(new St.Label({style_class: 'amethystora-pill-later-text', text}));
        row.add_child(words);
        const link = new St.Button({
            style_class: 'amethystora-pill-later-link',
            label: action,
            can_focus: true,
            y_align: Clutter.ActorAlign.CENTER,
            accessible_name: `${action}: ${title}`,
        });
        link.connect('clicked', () => run());
        row.add_child(link);
        return row;
    }
}
