// "Due today, from Notes": the titles of the open tasks due today and of the overdue ones, as Notes' own Today view
// lists them, under GNOME's events in the calendar menu. Notes says them on the session bus (org.amethystora.Notes,
// /org/amethystora/Notes/Today) only while its notes are open, and only to this shell: it is asked when the menu
// opens and when the day picked in the calendar is today again, never started for it, and listened to only while the
// menu is open. With Notes not running, locked, or nothing due, there is no section at all. A click opens Notes' tasks.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'org.amethystora.Notes';
const PATH = '/org/amethystora/Notes/Today';
const INTERFACE = 'org.amethystora.Notes.Today1';
// Rows shown; the rest are counted
const SHOWN = 5;
// Notes answers in well under a millisecond; one that does not answer in a second is not there
const TIMEOUT_MS = 1000;

function isToday(datetime) {
    const now = GLib.DateTime.new_now_local();
    return datetime.get_year() === now.get_year() && datetime.get_day_of_year() === now.get_day_of_year();
}

export class Tasks {
    constructor(dateMenu, settings) {
        this._dateMenu = dateMenu;
        this._settings = settings;
    }

    enable() {
        this._titles = [];
        this._section = new St.Button({
            style_class: 'amethystora-today-section amethystora-today-tasks',
            can_focus: true,
            x_expand: true,
            visible: false,
        });
        const box = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        this._section.child = box;
        this._heading = new St.Label({style_class: 'amethystora-today-heading'});
        box.add_child(this._heading);
        this._list = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, style_class: 'amethystora-today-list'});
        box.add_child(this._list);
        this._more = new St.Label({style_class: 'amethystora-today-more'});
        box.add_child(this._more);
        this._section.connect('clicked', () => this._open());

        // Under GNOME's events
        const events = this._dateMenu._eventsItem;
        const displays = events?.get_parent();
        if (!displays)
            throw new Error('the calendar menu has no events section');
        displays.insert_child_above(this._section, events);

        this._dateMenu.menu.connectObject('open-state-changed', (menu, open) => this._opened(open), this);
        this._dateMenu._calendar?.connectObject('selected-date-changed', (calendar, datetime) => {
            this._today = isToday(datetime);
            if (this._isOpen && this._today)
                this.ask();
            else
                this._show();
        }, this);
        this._settings.connectObject('changed::tasks', () => {
            if (this._isOpen)
                this.ask();
            this._show();
        }, this);
        this._today = true;
        if (this._dateMenu.menu.isOpen)
            this._opened(true);
    }

    disable() {
        this._opened(false);
        this._dateMenu.menu.disconnectObject(this);
        this._dateMenu._calendar?.disconnectObject(this);
        this._settings.disconnectObject(this);
        this._section?.destroy();
        this._section = null;
        this._titles = null;
    }

    titles() {
        return this._section?.visible ? [...this._titles] : [];
    }

    // Asked when the menu opens; Notes' word that its tasks changed is heard only while it is open
    _opened(open) {
        this._isOpen = open;
        if (open) {
            this._today = true;
            if (!this._signal) {
                this._signal = Gio.DBus.session.signal_subscribe(BUS_NAME, INTERFACE, 'Changed', PATH, null,
                    Gio.DBusSignalFlags.NONE, () => this.ask());
            }
            this.ask();
        } else {
            if (this._signal)
                Gio.DBus.session.signal_unsubscribe(this._signal);
            this._signal = 0;
            this._cancellable?.cancel();
            this._cancellable = null;
            // Nothing of the notes is kept while the menu is closed: the next opening asks again
            this._titles = [];
            this._total = 0;
            this._list?.destroy_all_children();
            if (this._section)
                this._section.visible = false;
        }
    }

    // One question at a time: a newer one cancels the one still waiting
    ask() {
        if (!this._section || !this._settings.get_boolean('tasks')) {
            this._titles = [];
            this._show();
            return;
        }
        this._cancellable?.cancel();
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        Gio.DBus.session.call(BUS_NAME, PATH, INTERFACE, 'GetToday', null, new GLib.VariantType('(suas)'),
            Gio.DBusCallFlags.NO_AUTO_START, TIMEOUT_MS, cancellable, (connection, result) => {
                let day, total, titles;
                try {
                    [day, total, titles] = connection.call_finish(result).deepUnpack();
                } catch {
                    // Not running, locked, or cancelled: no section
                    if (cancellable.is_cancelled() || !this._section)
                        return;
                    day = null;
                }
                if (this._cancellable === cancellable)
                    this._cancellable = null;
                if (!this._section)
                    return;
                // An answer from before midnight is for another day
                const today = GLib.DateTime.new_now_local().format('%F');
                if (day !== today) {
                    this._titles = [];
                    this._total = 0;
                } else {
                    this._titles = titles.filter(title => typeof title === 'string' && title !== '');
                    this._total = Math.max(total, this._titles.length);
                }
                this._show();
            });
    }

    _show() {
        if (!this._section)
            return;
        const shown = this._settings.get_boolean('tasks') && this._today && this._titles.length > 0;
        this._section.visible = shown;
        if (!shown)
            return;
        // The heading is the board's: small capitals, made here since St has no text-transform
        this._heading.text = _('Due today, from Notes').toLocaleUpperCase();
        this._list.destroy_all_children();
        for (const title of this._titles.slice(0, SHOWN)) {
            const row = new St.BoxLayout({style_class: 'amethystora-today-task'});
            row.add_child(new St.Widget({style_class: 'amethystora-today-mark', y_align: Clutter.ActorAlign.CENTER}));
            // A title is the person's text, never markup
            const label = new St.Label({text: title, style_class: 'amethystora-today-title', x_expand: true,
                y_align: Clutter.ActorAlign.CENTER});
            label.clutter_text.use_markup = false;
            label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
            row.add_child(label);
            this._list.add_child(row);
        }
        const more = this._total - Math.min(this._titles.length, SHOWN);
        this._more.visible = more > 0;
        if (more > 0)
            this._more.text = ngettext('%d more', '%d more', more).format(more);
        this._section.accessible_name = _('Due today, from Notes');
    }

    _open() {
        this._dateMenu.menu.close();
        try {
            Gio.Subprocess.new(['amethystora-notes', 'tasks'], Gio.SubprocessFlags.NONE);
        } catch (e) {
            console.warn(`Amethystora UI: Notes did not open: ${e.message}`);
        }
    }
}
