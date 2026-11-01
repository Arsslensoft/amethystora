// The words of "remind me at 17:00 to call the garage", for the search's reminders (remindProvider.js): the day and the
// time they name, and the rest, which is the task's title. The words are English whatever the session's language, as
// the quick add of Notes reads them (apps/notes/src/quick.rs), which reads the line this hands it again. Kept free of
// GNOME Shell's imports, so that it runs under node and gjs alike.

import {readClock} from './times.js';

const WEEKDAYS = ['sunday', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday'];
const MINUTES = ['minute', 'minutes', 'min', 'mins'];
const HOURS = ['hour', 'hours', 'h', 'hr', 'hrs'];
const START = /^remind\s+me\s+/iu;

const pad = n => String(n).padStart(2, '0');

// A day ({year, month, day}) n days after another
function addDays(date, n) {
    const d = new Date(date.year, date.month - 1, date.day + n, 12, 0, 0);
    return {year: d.getFullYear(), month: d.getMonth() + 1, day: d.getDate()};
}

export function isoDay(date) {
    return `${date.year}-${pad(date.month)}-${pad(date.day)}`;
}

// The weekday a word names (0 Sunday), or -1: the whole name or its first three letters or more
function weekdayOf(word) {
    const lower = word.toLowerCase();
    if (lower.length < 3)
        return -1;
    return WEEKDAYS.findIndex(name => name.startsWith(lower));
}

// Whether the text starts as a reminder does, so that the search can stay out of the way of everything else
export function looksLikeReminder(text) {
    return START.test(`${text.trim()} `);
}

// The reminder, or null. now: {year, month, day, weekday, hour, minute}, the session's own clock.
// Returns {title, date: {year, month, day}, hour, minute, relative, offset}: offset the days after today (0, 1, ...).
export function readReminder(text, now) {
    const typed = text.trim().replace(/\s+/gu, ' ');
    if (!START.test(`${typed} `) || typed.length > 300)
        return null;
    const tokens = typed.replace(START, '').split(' ').filter(Boolean);
    const kept = [];
    let clock = null;
    let day = null;
    let relative = null;
    for (let i = 0; i < tokens.length; i++) {
        const word = tokens[i].toLowerCase();
        const next = tokens[i + 1]?.toLowerCase() ?? '';
        // "in 20 minutes", "in 2 hours"
        if (!clock && !relative && word === 'in' && /^\d{1,4}$/u.test(next) && i + 2 < tokens.length) {
            const unit = tokens[i + 2].toLowerCase();
            const minutes = MINUTES.includes(unit) ? Number(next) : HOURS.includes(unit) ? Number(next) * 60 : 0;
            if (minutes > 0 && minutes <= 7 * 24 * 60) {
                relative = minutes;
                i += 2;
                continue;
            }
        }
        // "at 17:00", "17:00", "at 5 pm", "5pm"
        if (!clock && !relative) {
            const from = word === 'at' ? i + 1 : i;
            const found = from < tokens.length ? readClock(tokens.slice(from).join(' ')) : null;
            if (found) {
                clock = found;
                const used = tokens.slice(from).join(' ').length - found.rest.length;
                i = from + tokens.slice(from).join(' ').slice(0, used).trim().split(' ').length - 1;
                continue;
            }
        }
        // "today", "tonight", "tomorrow", "friday", "on friday", "2026-11-03"
        if (!day && !relative) {
            const from = word === 'on' ? i + 1 : i;
            const candidate = tokens[from]?.toLowerCase() ?? '';
            let found = null;
            if (['today', 'tonight'].includes(candidate) && from === i) {
                found = {offset: 0};
            } else if (['tomorrow', 'tmr'].includes(candidate) && from === i) {
                found = {offset: 1};
            } else if (/^\d{4}-\d{2}-\d{2}$/u.test(candidate)) {
                const [y, m, d] = candidate.split('-').map(Number);
                const check = new Date(y, m - 1, d, 12);
                if (check.getMonth() === m - 1 && check.getDate() === d)
                    found = {date: {year: y, month: m, day: d}};
            } else if (weekdayOf(candidate) >= 0) {
                const ahead = (weekdayOf(candidate) - now.weekday + 7) % 7;
                found = {offset: ahead === 0 ? 7 : ahead};
            }
            if (found) {
                day = found;
                i = from;
                continue;
            }
        }
        kept.push(tokens[i]);
    }
    if (!clock && !relative)
        return null;
    // What is left is the title: "to call the garage" is "Call the garage"
    while (kept.length && ['to', 'at', 'on', 'that'].includes(kept[0].toLowerCase()))
        kept.shift();
    while (kept.length && ['at', 'on', 'to'].includes(kept[kept.length - 1].toLowerCase()))
        kept.pop();
    let title = kept.join(' ').trim();
    if (!title)
        return null;
    title = title[0].toLocaleUpperCase() + title.slice(1);
    const today = {year: now.year, month: now.month, day: now.day};

    if (relative) {
        const at = new Date(now.year, now.month - 1, now.day, now.hour, now.minute + relative, 0);
        const date = {year: at.getFullYear(), month: at.getMonth() + 1, day: at.getDate()};
        const offset = Math.round((new Date(date.year, date.month - 1, date.day, 12) -
            new Date(today.year, today.month - 1, today.day, 12)) / 86400000);
        return {title, date, hour: at.getHours(), minute: at.getMinutes(), relative: true, offset};
    }
    let date;
    let offset;
    if (day?.date) {
        date = day.date;
        offset = Math.round((new Date(date.year, date.month - 1, date.day, 12) -
            new Date(today.year, today.month - 1, today.day, 12)) / 86400000);
        if (offset < 0)
            return null;
    } else {
        // A time already past today is tomorrow's, as Notes reads it
        offset = day ? day.offset : clock.hour * 60 + clock.minute <= now.hour * 60 + now.minute ? 1 : 0;
        date = addDays(today, offset);
    }
    if (offset === 0 && clock.hour * 60 + clock.minute <= now.hour * 60 + now.minute)
        return null;
    return {title, date, hour: clock.hour, minute: clock.minute, relative: false, offset};
}

// The same reminder a day later, for the second row ("Tomorrow at 17:00 instead")
export function dayLater(reminder) {
    return {...reminder, date: addDays(reminder.date, 1), offset: reminder.offset + 1};
}

// The line Notes is handed on its standard input: the day and the time first, so that its quick add takes them before
// any word of the title that also names a day ("call about monday")
export function noteLine(reminder) {
    return `${isoDay(reminder.date)} ${pad(reminder.hour)}:${pad(reminder.minute)} ${reminder.title}`;
}
