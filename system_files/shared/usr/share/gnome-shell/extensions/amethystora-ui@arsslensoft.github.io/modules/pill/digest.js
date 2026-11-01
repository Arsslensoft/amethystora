// The digest card: what the digest held since the last one, together, where banners appear. One row for each app
// (its notification's title, or how many it sent), then Show all (the calendar's list, where they all are) and Clear.
// It stays twelve seconds, longer while the pointer is on it; nothing is lost when it goes.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';

import {onKey} from './clock.js';
import {plain, timeOfDay} from './text.js';

// Rows shown before "and N more apps", the time it stays (s) and the gap under the top bar (logical px)
const ROWS = 6;
const STAYS = 12;
const GAP = 8;

export class DigestCard {
    // title: the card's own heading (a mute's held ones), else the digest's
    constructor(notifications, time, closed, title = null) {
        this._notifications = notifications;
        this._closed = closed;

        const [h, m] = time.split(':').map(Number);
        const at = new Date();
        at.setHours(h, m, 0, 0);

        const card = new St.BoxLayout({
            style_class: 'amethystora-pill-digest',
            orientation: Clutter.Orientation.VERTICAL,
            reactive: true,
            track_hover: true,
            can_focus: true,
            name: 'amethystora-pill-digest',
        });
        const head = new St.BoxLayout({style_class: 'amethystora-pill-digest-head'});
        head.add_child(new St.Label({
            style_class: 'amethystora-pill-digest-title',
            text: title ?? _('Your %s digest').format(timeOfDay(at)),
            x_expand: true,
        }));
        const count = notifications.length;
        head.add_child(new St.Label({
            style_class: 'amethystora-pill-digest-count',
            text: ngettext('%d notification', '%d notifications', count).format(count),
            y_align: Clutter.ActorAlign.END,
        }));
        card.add_child(head);

        // One row for each app, by its name (a program that sends with notify-send gets a source of its own each time),
        // in the order their first notification came
        const apps = new Map();
        for (const notification of notifications) {
            const source = notification.source;
            const key = source?.title || source;
            if (!apps.has(key))
                apps.set(key, {source, list: []});
            apps.get(key).list.push(notification);
        }
        let shown = 0;
        for (const {source, list} of apps.values()) {
            if (shown === ROWS)
                break;
            shown++;
            const row = new St.BoxLayout({style_class: 'amethystora-pill-digest-row'});
            const gicon = source?.icon ?? null;
            row.add_child(new St.Icon({
                style_class: 'amethystora-pill-digest-icon',
                icon_size: 26,
                ...(gicon ? {gicon} : {icon_name: 'application-x-executable-symbolic'}),
                y_align: Clutter.ActorAlign.CENTER,
            }));
            const name = plain(source?.title, 40) || _('Unknown app');
            row.add_child(new St.Label({style_class: 'amethystora-pill-digest-app', text: name, y_align: Clutter.ActorAlign.CENTER}));
            const what = list.length === 1
                ? plain(list[0].title, 80)
                : ngettext('%d notification', '%d notifications', list.length).format(list.length);
            const words = new St.Label({
                style_class: 'amethystora-pill-digest-what',
                text: what ? ` · ${what}` : '',
                y_align: Clutter.ActorAlign.CENTER,
                x_expand: true,
            });
            words.clutter_text.ellipsize = Pango.EllipsizeMode.END;
            row.add_child(words);
            card.add_child(row);
        }
        if (apps.size > ROWS) {
            const more = apps.size - ROWS;
            card.add_child(new St.Label({
                style_class: 'amethystora-pill-digest-more',
                text: ngettext('and %d more app', 'and %d more apps', more).format(more),
            }));
        }

        const buttons = new St.BoxLayout({style_class: 'amethystora-pill-digest-buttons'});
        const showAll = new St.Button({
            style_class: 'amethystora-pill-digest-button amethystora-pill-digest-button-primary',
            label: _('Show all'),
            can_focus: true,
        });
        showAll.connect('clicked', () => {
            this.close();
            Main.panel.statusArea.dateMenu?.menu.open();
        });
        const clear = new St.Button({style_class: 'amethystora-pill-digest-button', label: _('Clear'), can_focus: true});
        clear.connect('clicked', () => {
            for (const notification of this._notifications)
                notification.destroy?.(MessageTray.NotificationDestroyedReason.DISMISSED);
            this.close();
        });
        buttons.add_child(showAll);
        buttons.add_child(clear);
        card.add_child(buttons);

        onKey(card, symbol => {
            if (symbol !== Clutter.KEY_Escape)
                return false;
            this.close();
            return true;
        });
        // It stays while the pointer is on it. It takes the keyboard (for Esc) only when it is clicked: a pointer that
        // merely passes over it never takes the keys from the window being typed in
        card.connect('notify::hover', () => {
            if (card.hover)
                this._stopTimer();
            else
                this._startTimer();
        });
        // A press on the card gives it the keys. Not a gesture of its own: that would compete with its buttons' for
        // the same press, and a click on Show all or Clear would only focus the card. GNOME 51 delivers no such signal
        // to actors; there, Esc reaches the card once Tab or a click on a button has put the focus in it.
        if (GObject.signal_lookup('button-press-event', Clutter.Actor.$gtype)) {
            card.connect('button-press-event', () => {
                card.grab_key_focus();
                return Clutter.EVENT_PROPAGATE;
            });
        }

        this.actor = card;
        Main.layoutManager.addTopChrome(card);
        card.connect('notify::width', () => this._place());
        this._place();
        card.opacity = 0;
        card.ease({opacity: 255, duration: 200, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        this._startTimer();
    }

    // Top centre of the main screen, under the top bar
    _place() {
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor || !this.actor)
            return;
        const area = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        this.actor.set_position(
            Math.round(monitor.x + (monitor.width - this.actor.width) / 2),
            Math.round(area.y + GAP * scale));
    }

    _startTimer() {
        this._stopTimer();
        this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, STAYS, () => {
            this._timer = 0;
            this.close();
            return GLib.SOURCE_REMOVE;
        });
    }

    _stopTimer() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
    }

    close() {
        this._stopTimer();
        const card = this.actor;
        if (!card)
            return;
        this.actor = null;
        Main.layoutManager.removeChrome(card);
        card.destroy();
        const closed = this._closed;
        this._closed = null;
        closed?.();
    }
}
