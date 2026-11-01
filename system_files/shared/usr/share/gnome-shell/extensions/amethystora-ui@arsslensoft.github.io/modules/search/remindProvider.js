// "remind me at 17:00 to call the garage": a task in Notes' inbox with a reminder at that time, made with Enter. The
// search reads the words (remind.js) to show what it will make, and nothing happens until Enter: then Notes is handed
// the task on its standard input (never its command line, which every account can read), as one line its own quick add
// reads, the day and the time first. Notes adds it when the notes are open; when they are locked, its quick note asks
// for the passphrase first, with the text in it. The second row is the same a day later ("Tab to change the day").

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {dayLater, noteLine, readReminder} from './remind.js';

const NOTES = '/usr/bin/amethystora-notes';
// Notes' vault, which says whether the notes are encrypted (and so locked until a passphrase is given)
const VAULT = 'amethystora-notes/vault.json';
// Notes' Today object, on the bus only while its notes are open (apps/notes/src/today.rs, as modules/today/tasks.js asks)
const BUS_NAME = 'org.amethystora.Notes';
const TODAY_PATH = '/org/amethystora/Notes/Today';
const TODAY_INTERFACE = 'org.amethystora.Notes.Today1';
const TIMEOUT_MS = 500;

const fill = (text, values) => text.replace(/\{(\w+)\}/g, (whole, name) => values[name] ?? whole);

export class RemindProvider {
    constructor() {
        this.id = 'amethystora-remind';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._rows = [];
    }

    enable() {
        this._interface = new Gio.Settings({schema_id: 'org.gnome.desktop.interface'});
    }

    destroy() {
        // Disposed now rather than by the collector (dock.js says why)
        this._interface?.run_dispose();
        this._interface = null;
        this._rows = [];
    }

    _text(terms) {
        const typed = Main.overview.searchEntry?.get_text()?.trim() ?? '';
        return typed.split(/\s+/u).join(' ') === terms.join(' ') ? typed : terms.join(' ');
    }

    _now() {
        const now = GLib.DateTime.new_now_local();
        return {
            year: now.get_year(), month: now.get_month(), day: now.get_day_of_month(),
            weekday: now.get_day_of_week() % 7, hour: now.get_hour(), minute: now.get_minute(),
        };
    }

    // Whether Notes keeps its notes encrypted (its vault says so), read only when a reminder is typed; a vault that
    // cannot be read counts as not encrypted, which only leaves out the words "asks first"
    _encrypted() {
        try {
            const [ok, bytes] = GLib.file_get_contents(GLib.build_filenamev([GLib.get_user_data_dir(), VAULT]));
            return ok && JSON.parse(new TextDecoder().decode(bytes))?.encrypted === true;
        } catch {
            return false;
        }
    }

    // Whether Notes has its notes open now: its Today object answers only then. Never starts Notes.
    _open(cancellable) {
        return new Promise(resolve => {
            Gio.DBus.session.call(BUS_NAME, TODAY_PATH, TODAY_INTERFACE, 'GetToday', null, null,
                Gio.DBusCallFlags.NO_AUTO_START, TIMEOUT_MS, cancellable, (connection, result) => {
                    try {
                        connection.call_finish(result);
                        resolve(true);
                    } catch {
                        resolve(false);
                    }
                });
        });
    }

    // "today at 17:00", "tomorrow at 9:00 AM", "Friday at 14:00", "24 December at 18:00"
    _when(reminder) {
        const at = GLib.DateTime.new_local(reminder.date.year, reminder.date.month, reminder.date.day,
            reminder.hour, reminder.minute, 0);
        const twelve = this._interface?.get_string('clock-format') === '12h';
        // Translators: the time of day on a 12-hour and on a 24-hour clock, as GLib.DateTime.format takes it
        const time = at.format(twelve ? _('%l:%M %p') : _('%H:%M')).trim();
        if (reminder.offset === 0)
            return fill(_('today at {time}'), {time});
        if (reminder.offset === 1)
            return fill(_('tomorrow at {time}'), {time});
        // Translators: a day in the coming week, as GLib.DateTime.format takes it
        const day = reminder.offset < 7 ? at.format(_('%A')) : at.format(_('%e %B')).trim();
        return fill(_('{day} at {time}'), {day, time});
    }

    async _answer(text, cancellable = null) {
        const reminder = readReminder(text, this._now());
        if (!reminder)
            return [];
        const locked = this._encrypted() && !await this._open(cancellable);
        const when = this._when(reminder);
        const rows = [{
            id: 'remind', name: reminder.title, symbol: 'object-select-symbolic', pictureClass: 'amethystora-search-pic-agent',
            description: locked
                ? fill(_('A task in Notes · {when} · Notes asks for its passphrase first'), {when})
                : fill(_('A task in Notes · {when}'), {when}),
            hint: _('Enter'), line: noteLine(reminder),
        }];
        if (!reminder.relative) {
            const later = dayLater(reminder);
            const sentence = this._when(later);
            rows.push({
                id: 'remind-later', name: fill(_('{when} instead'), {when: sentence[0].toLocaleUpperCase() + sentence.slice(1)}),
                symbol: 'alarm-symbolic', description: _('Tab to change the day'), hint: _('Enter'), line: noteLine(later),
            });
        }
        return rows;
    }

    async getInitialResultSet(terms, cancellable) {
        let rows = [];
        try {
            rows = await this._answer(this._text(terms), cancellable);
        } catch (e) {
            console.warn(`Amethystora UI: the search's reminder: ${e.message}`);
        }
        // A newer search began meanwhile: its own answer is the one kept
        if (cancellable?.is_cancelled())
            return [];
        this._rows = rows;
        return rows.map(row => row.id);
    }

    getSubsearchResultSet(previous, terms) {
        return this.getInitialResultSet(terms);
    }

    getResultMetas(ids) {
        return Promise.resolve(ids.map(id => {
            const row = this._rows.find(r => r.id === id);
            return row ? {...row, line: undefined} : {id, name: id};
        }));
    }

    filterResults(ids, max) {
        return ids.slice(0, max);
    }

    // Enter: Notes is handed the line on its standard input, and reads it whole before it adds anything
    activateResult(id) {
        const line = this._rows.find(row => row.id === id)?.line;
        if (line)
            this.hand(line);
    }

    // The hand-off itself, for the suites too: whether Notes could be started
    hand(line) {
        try {
            const process = Gio.Subprocess.new([NOTES, 'remind'],
                Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
            process.communicate_utf8_async(`${line}\n`, null, (source, result) => {
                try {
                    source.communicate_utf8_finish(result);
                } catch (e) {
                    console.warn(`Amethystora UI: Notes did not take the reminder: ${e.message}`);
                }
            });
            return true;
        } catch (e) {
            console.warn(`Amethystora UI: the search could not start Notes: ${e.message}`);
            return false;
        }
    }

    // For the suites: the rows a text would show, and the line Enter would hand Notes
    async answerFor(text) {
        return (await this._answer(text)).map(({name, description, line}) => ({name, description, line}));
    }
}
