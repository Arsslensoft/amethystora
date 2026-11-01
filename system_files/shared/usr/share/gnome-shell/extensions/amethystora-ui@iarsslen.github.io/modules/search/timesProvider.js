// The time in other places, answered as it is typed: "time in Tokyo" shows the time there now, "15:00 Paris in Tokyo"
// what 15:00 in Paris is in Tokyo, with the time there now under it. Enter copies the answer, as GNOME's result copies a
// result's clipboardText. Offline: the places are those of GNOME's own location database (GWeather, which the world
// clocks and Weather read too), by their names in the session's language or in English, and time-zone ids, UTC and
// UTC+2 besides; the arithmetic is GLib's, with the system's own time-zone data. The names are gathered the first time
// a question about the time is asked after the search starts (one walk of the database), and kept until it stops.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GWeather from 'gi://GWeather';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';

import {normalize, offsetZone, readTimeQuestion} from './times.js';

const fill = (text, values) => text.replace(/\{(\w+)\}/g, (whole, name) => values[name] ?? whole);

export class TimesProvider {
    constructor() {
        this.id = 'amethystora-times';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._rows = [];
        this._places = null;
    }

    enable() {
        this._interface = new Gio.Settings({schema_id: 'org.gnome.desktop.interface'});
    }

    destroy() {
        // Disposed now rather than by the collector (dock.js says why)
        this._interface?.run_dispose();
        this._interface = null;
        this._places = null;
        this._rows = [];
    }

    // The words of a question in the session's language: before a place, after it, and between two
    _words() {
        return {
            // Translators: the words before a place that ask for the time there now, apart by "|", longest first
            before: _('time in|now in|what time is it in|the time in|current time in|time at'),
            // Translators: a word after a place that asks for the time there now ("Tokyo time"), apart by "|"
            after: _('time|now'),
            joins: _('in|to'),
        };
    }

    // GNOME splits the text at every space: the field's own text is read instead, when it is what the words came from
    _text(terms) {
        const typed = Main.overview.searchEntry?.get_text()?.trim() ?? '';
        return typed.split(/\s+/u).join(' ') === terms.join(' ') ? typed : terms.join(' ');
    }

    // Every place's name, normalized, and its zone and name to show: a city whose zone's own name is the city's wins
    // over another of the same name (Paris over Paris, Texas), then a country with one zone, then the rest
    _index() {
        if (this._places)
            return this._places;
        const places = new Map();
        const add = (name, tz, shown, score) => {
            const key = name ? normalize(name) : '';
            if (!key)
                return;
            const had = places.get(key);
            if (!had || had.score < score)
                places.set(key, {tz, name: shown, score});
        };
        const Level = GWeather.LocationLevel;
        const walk = location => {
            for (let child = location.next_child(null); child; child = location.next_child(child)) {
                const level = child.get_level();
                if (level === Level.CITY) {
                    const tz = child.get_timezone_str();
                    if (!tz)
                        continue;
                    const name = child.get_name();
                    const english = child.get_english_name();
                    const city = normalize(tz.split('/').pop());
                    const score = normalize(english ?? '') === city || normalize(name ?? '') === city ? 4 : 1;
                    add(name, tz, name, score);
                    add(english, tz, name, score);
                    // A zone's id shows as its own city ("Europe/Paris" as Paris), whichever city of it came first
                    add(tz, tz, tz.split('/').pop().replace(/_/gu, ' '), 5);
                } else if (level === Level.REGION || level === Level.COUNTRY || level === Level.ADM1) {
                    if (level === Level.COUNTRY) {
                        const tz = child.get_timezone_str();
                        if (tz) {
                            add(child.get_name(), tz, child.get_name(), 3);
                            add(child.get_english_name(), tz, child.get_name(), 3);
                        }
                    }
                    walk(child);
                }
            }
        };
        try {
            walk(GWeather.Location.get_world());
        } catch (e) {
            console.warn(`Amethystora UI: the search could not read GNOME's places: ${e.message}`);
        }
        // A zone's own city ("Kolkata", "Europe/Paris") when no city of the database is called so
        for (const [, place] of [...places]) {
            const city = place.tz.split('/').pop().replace(/_/gu, ' ');
            add(city, place.tz, city, 2);
        }
        this._places = places;
        return places;
    }

    // A place as typed: {zone, name}, or null. "UTC" and "UTC+2" need no database.
    _place(typed) {
        const offset = offsetZone(typed);
        if (offset) {
            const zone = GLib.TimeZone.new_identifier(offset);
            return zone ? {zone, name: offset === 'UTC' ? 'UTC' : `UTC${offset}`} : null;
        }
        const found = this._index().get(normalize(typed));
        if (!found)
            return null;
        const zone = GLib.TimeZone.new_identifier(found.tz);
        return zone ? {zone, name: found.name} : null;
    }

    _here() {
        return {zone: GLib.TimeZone.new_local(), name: _('here'), local: true};
    }

    // A time of day as the person's clock shows it (12- or 24-hour)
    _clock(dt) {
        const twelve = this._interface?.get_string('clock-format') === '12h';
        // Translators: the time of day on a 12-hour and on a 24-hour clock, as GLib.DateTime.format takes it
        return dt.format(twelve ? _('%l:%M %p') : _('%H:%M')).trim();
    }

    // "8 hours ahead of you", "5 hours 30 minutes behind you", "the same time as you"
    _against(dt) {
        const here = dt.to_local();
        const minutes = Math.round((dt.get_utc_offset() - here.get_utc_offset()) / 60e6);
        if (minutes === 0)
            return _('the same time as you');
        const whole = Math.abs(minutes);
        const hours = Math.floor(whole / 60);
        const rest = whole % 60;
        const parts = [];
        if (hours)
            parts.push(ngettext('%d hour', '%d hours', hours).format(hours));
        if (rest)
            parts.push(ngettext('%d minute', '%d minutes', rest).format(rest));
        const span = parts.join(' ');
        return minutes > 0 ? _('%s ahead of you').format(span) : _('%s behind you').format(span);
    }

    // "the same day", "the next day", "the day before": the day there against the day the time was given in
    _day(from, to) {
        const days = Math.round((Date.UTC(to.get_year(), to.get_month() - 1, to.get_day_of_month()) -
            Date.UTC(from.get_year(), from.get_month() - 1, from.get_day_of_month())) / 86400000);
        if (days === 0)
            return _('the same day');
        return days > 0 ? _('the next day') : _('the day before');
    }

    // The time now in a place, as a row
    _nowRow(place, answer) {
        const now = GLib.DateTime.new_now(place.zone);
        const time = this._clock(now);
        const weekday = now.format('%A');
        const against = this._against(now);
        if (answer) {
            return {
                id: 'now', name: time, answer: true, symbol: 'preferences-system-time-symbolic',
                expression: fill(_('Now in {place}'), {place: place.name}),
                description: fill(_('{weekday} in {place} · {against}'), {weekday, place: place.name, against}),
                hint: _('Enter copies'),
                clipboardText: fill(_('{time} in {place}'), {time, place: place.name}),
            };
        }
        return {
            id: 'now', name: fill(_('Now in {place}: {time}'), {place: place.name, time}),
            description: `${weekday} · ${against}`, symbol: 'preferences-system-time-symbolic', hint: _('Enter copies'),
            clipboardText: fill(_('{time} in {place}'), {time, place: place.name}),
        };
    }

    // A time in one place, in another
    _atRows(hour, minute, from, to) {
        const there = GLib.DateTime.new_now(from.zone);
        const start = GLib.DateTime.new(from.zone, there.get_year(), there.get_month(), there.get_day_of_month(), hour, minute, 0);
        if (!start)
            return [];
        const end = start.to_timezone(to.zone);
        const time = this._clock(end);
        const rows = [{
            id: 'at', name: time, answer: true,
            expression: fill(_('{time} {from} in {to}'), {time: this._clock(start), from: from.name, to: to.name}),
            description: to.local
                ? fill(_('here, {day}'), {day: this._day(start, end)})
                : fill(_('in {place}, {day}'), {place: to.name, day: this._day(start, end)}),
            hint: _('Enter copies'),
            clipboardText: to.local ? time : fill(_('{time} in {place}'), {time, place: to.name}),
        }];
        // The time there now, but for the session's own zone, whose time is on the bar
        const shown = to.local ? from : to;
        if (!shown.local)
            rows.push(this._nowRow(shown, false));
        return rows;
    }

    _answer(text) {
        const question = readTimeQuestion(text, this._words());
        if (!question)
            return [];
        if (question.kind === 'now') {
            const place = this._place(question.place);
            return place ? [this._nowRow(place, true)] : [];
        }
        const pairs = question.cuts ?? [[question.from?.[0] ?? null, question.to?.[0] ?? null]];
        for (const [fromName, toName] of pairs) {
            const from = fromName === null ? this._here() : this._place(fromName);
            const to = toName === null ? this._here() : this._place(toName);
            if (from && to)
                return this._atRows(question.hour, question.minute, from, to);
        }
        return [];
    }

    getInitialResultSet(terms) {
        try {
            this._rows = this._answer(this._text(terms));
        } catch (e) {
            console.warn(`Amethystora UI: the search's time answer: ${e.message}`);
            this._rows = [];
        }
        return Promise.resolve(this._rows.map(row => row.id));
    }

    getSubsearchResultSet(previous, terms) {
        return this.getInitialResultSet(terms);
    }

    getResultMetas(ids) {
        return Promise.resolve(ids.map(id => this._rows.find(row => row.id === id) ?? {id, name: id}));
    }

    filterResults(ids, max) {
        return ids.slice(0, max);
    }

    // GNOME's result copies the answer itself, from its clipboardText
    activateResult() {}

    // For the suites: the rows a text would show, as {name, description}
    answerFor(text) {
        return this._answer(text).map(({name, description}) => ({name, description}));
    }
}
