// Find it by what you remember (plans/ui-ideas/r7/spec-find.md, R7-60): a small fixed set of rules that reads kinds of
// file, dates and sizes out of what was typed: "pdf from last week", "big videos", "lease from march". What it cannot
// read stays as words of the file's name, and with no rule at all it answers nothing, so the search goes on as usual. No
// model and no agent, and nothing is kept: this file has no GNOME in it, so that a test can run it under node.
//
// A word list is one message, its alternatives separated by |, in lower case; the English ones work in every language,
// and the names of days and months come from the language's own data.

// File name endings people say as a kind of file, which are the same words everywhere
const ENDINGS = {
    pdf: 'pdf', pdfs: 'pdf', doc: 'document', docs: 'document', docx: 'document', odt: 'document', rtf: 'document', txt: 'text', xls: 'spreadsheet',
    xlsx: 'spreadsheet', ods: 'spreadsheet', csv: 'spreadsheet', ppt: 'presentation', pptx: 'presentation',
    odp: 'presentation', zip: 'archive', tar: 'archive', '7z': 'archive', rar: 'archive', jpg: 'image', jpeg: 'image',
    png: 'image', gif: 'image', webp: 'image', heic: 'image', mp3: 'audio', flac: 'audio', ogg: 'audio', wav: 'audio',
    m4a: 'audio', mp4: 'video', mkv: 'video', webm: 'video', mov: 'video', avi: 'video',
};
const BIG = 10 * 1024 * 1024;
const SMALL = 1024 * 1024;
const MOST_WORDS = 4;
const LONGEST_PHRASE = 4;
// A leftover word shorter than this is not asked for: it may be the start of a word still being typed
const SHORTEST_WORD = 3;
// More words than this are a paste, not a question
const MOST_TYPED = 16;

// Every word list, through gettext or as it is written
export function rememberWords(_) {
    // Translators: words separated by | are alternatives, in lower case, the words people say in your language. {n} is a number; keep it.
    return {
        document: _('document|documents|letter|letters'),
        spreadsheet: _('spreadsheet|spreadsheets'),
        presentation: _('presentation|presentations|slides'),
        image: _('photo|photos|picture|pictures|image|images|screenshot|screenshots'),
        video: _('video|videos|movie|movies|film|films'),
        audio: _('music|song|songs|audio|recording|recordings'),
        archive: _('archive|archives'),
        text: _('text file|text files'),
        today: _('today'),
        yesterday: _('yesterday'),
        thisWeek: _('this week'),
        lastWeek: _('last week|past week|a week ago'),
        thisMonth: _('this month'),
        lastMonth: _('last month|past month|a month ago'),
        thisYear: _('this year'),
        lastYear: _('last year|past year|a year ago'),
        daysAgo: _('{n} days ago|{n} day ago'),
        weeksAgo: _('{n} weeks ago|{n} week ago'),
        monthsAgo: _('{n} months ago|{n} month ago'),
        last: _('last'),
        big: _('big|large|huge'),
        small: _('small|tiny'),
        over: _('over|bigger than|larger than|more than'),
        under: _('under|smaller than|less than'),
        // Translators: exactly four words, for kilobytes, megabytes, gigabytes and terabytes, in this order: a size is read by the word's place in the list.
        units: _('kb|mb|gb|tb'),

        // Translators: "since" in your language, a word before a date that leaves the period open to now. Alternatives are separated by |, in lower case.
        since: _('since'),
        // Translators: words separated by | are alternatives, in lower case: small words left out of a search, such as "of" and "the".
        filler: _('from|of|in|on|at|the|my|a|an|some|any|file|files|that|which|around|about|with|and'),
    };
}

// A digit of any script as the ASCII digit of the same value (the digits of a script come in a run of ten)
function asciiDigit(digit) {
    const code = digit.codePointAt(0);
    if (code < 128)
        return digit;
    let start = code;
    while (/\p{Nd}/u.test(String.fromCodePoint(start - 1)))
        start--;
    return String((code - start) % 10);
}

// No case and no accents, digits as ASCII; an elision is a word of its own, so that "d'hier" is "d'" and "hier"
const fold = (text, locale) => text.toLocaleLowerCase(locale).normalize('NFD').replace(/\p{M}/gu, '')
    .replace(/\p{Nd}/gu, asciiDigit).replace(/٫/gu, '.').replace(/['’]/gu, "' ").trim();
const splitWords = (list, locale) => list.split('|').map(word => fold(word.trim(), locale)).filter(Boolean);

// The names of the days (from Sunday) and the months in a language
function dayNames(locale) {
    const format = new Intl.DateTimeFormat(locale, {weekday: 'long', timeZone: 'UTC'});
    // 1 January 2023 was a Sunday
    return [...Array(7).keys()].map(day => fold(format.format(new Date(Date.UTC(2023, 0, 1 + day))), locale));
}

function monthNames(locale) {
    const format = new Intl.DateTimeFormat(locale, {month: 'long', timeZone: 'UTC'});
    return [...Array(12).keys()].map(month => fold(format.format(new Date(Date.UTC(2023, month, 15))), locale));
}

// The first day of a week in that language, from Sunday as 0; Monday when its data does not say
function firstDayOfWeek(locale) {
    try {
        const about = new Intl.Locale(locale);
        const info = about.getWeekInfo?.() ?? about.weekInfo;
        return (info?.firstDay ?? 1) % 7;
    } catch {
        return 1;
    }
}

// Everything the rules know of one language, English included: the phrases, longest first when looked up
export function compileRemember(english, translated, locale = 'en') {
    const phrases = new Map();
    const add = (list, what) => {
        for (const phrase of splitWords(list, locale)) {
            if (!phrases.has(phrase))
                phrases.set(phrase, what);
        }
    };
    const templates = [];
    for (const source of [translated, english]) {
        for (const kind of ['document', 'spreadsheet', 'presentation', 'image', 'video', 'audio', 'archive', 'text'])
            add(source[kind], {type: 'kind', kind});
        for (const range of ['today', 'yesterday', 'thisWeek', 'lastWeek', 'thisMonth', 'lastMonth', 'thisYear', 'lastYear'])
            add(source[range], {type: 'range', range});
        add(source.last, {type: 'last'});
        add(source.since, {type: 'since'});
        add(source.big, {type: 'big'});
        add(source.small, {type: 'small'});
        add(source.over, {type: 'over'});
        add(source.under, {type: 'under'});
        add(source.filler, {type: 'filler'});
        for (const [key, unit] of [['daysAgo', 'day'], ['weeksAgo', 'week'], ['monthsAgo', 'month']]) {
            for (const template of splitWords(source[key], locale))
                templates.push({words: template.split(/\s+/u), unit});
        }
    }
    for (const [ending, kind] of Object.entries(ENDINGS))
        phrases.set(ending, {type: 'kind', kind});
    for (const names of [dayNames('en'), dayNames(locale)])
        names.forEach((name, day) => phrases.set(name, {type: 'day', day}));
    for (const names of [monthNames('en'), monthNames(locale)])
        names.forEach((name, month) => phrases.set(name, {type: 'month', month}));
    const units = new Map();
    for (const source of [translated, english]) {
        splitWords(source.units, locale).forEach((unit, power) => {
            if (!units.has(unit))
                units.set(unit, 1024 ** (power + 1));
        });
    }
    return {phrases, templates, units, locale, firstDay: firstDayOfWeek(locale)};
}

const startOfDay = (year, month, day) => new Date(year, month, day);

// The start and end of a period, the end not included, in the machine's own time, as the index compares them
function period(what, now, firstDay) {
    const y = now.getFullYear();
    const m = now.getMonth();
    const d = now.getDate();
    const week = d - ((now.getDay() - firstDay + 7) % 7);
    switch (what.range) {
    case 'today': return [startOfDay(y, m, d), startOfDay(y, m, d + 1)];
    case 'yesterday': return [startOfDay(y, m, d - 1), startOfDay(y, m, d)];
    case 'thisWeek': return [startOfDay(y, m, week), startOfDay(y, m, week + 7)];
    case 'lastWeek': return [startOfDay(y, m, week - 7), startOfDay(y, m, week)];
    case 'thisMonth': return [startOfDay(y, m, 1), startOfDay(y, m + 1, 1)];
    case 'lastMonth': return [startOfDay(y, m - 1, 1), startOfDay(y, m, 1)];
    case 'thisYear': return [startOfDay(y, 0, 1), startOfDay(y + 1, 0, 1)];
    case 'lastYear': return [startOfDay(y - 1, 0, 1), startOfDay(y, 0, 1)];
    }
    return null;
}

function ago(unit, count, now, firstDay) {
    const y = now.getFullYear();
    const m = now.getMonth();
    const d = now.getDate();
    if (unit === 'day')
        return [startOfDay(y, m, d - count), startOfDay(y, m, d - count + 1)];
    if (unit === 'month')
        return [startOfDay(y, m - count, 1), startOfDay(y, m - count + 1, 1)];
    const week = d - ((now.getDay() - firstDay + 7) % 7) - 7 * count;
    return [startOfDay(y, m, week), startOfDay(y, m, week + 7)];
}

// The last such weekday, today if it is today, unless "last" was said with it
function dayPeriod(day, strict, now) {
    const y = now.getFullYear();
    const m = now.getMonth();
    let back = (now.getDay() - day + 7) % 7;
    if (back === 0 && strict)
        back = 7;
    const d = now.getDate() - back;
    return [startOfDay(y, m, d), startOfDay(y, m, d + 1)];
}

// The last such month, this one if it is this one, unless a year was said with it
function monthPeriod(month, year, now) {
    let y = year ?? now.getFullYear();
    if (year === null && month > now.getMonth())
        y -= 1;
    return [startOfDay(y, month, 1), startOfDay(y, month + 1, 1)];
}

// That day of the month, in that month: this year's, or last year's when it has not come yet; null when there is no such date
function dateOf(month, day, year, now) {
    const passed = month > now.getMonth() || (month === now.getMonth() && day > now.getDate());
    const y = year ?? (passed ? now.getFullYear() - 1 : now.getFullYear());
    const date = new Date(y, month, day);
    return date.getMonth() === month ? [date, new Date(y, month, day + 1)] : null;
}

// The day of the week inside a period of up to a week ("monday last week"); null when the period is longer, where the
// weekday would be many days, or has none
function dayIn(range, day) {
    const [start, end] = range;
    if (new Date(start.getFullYear(), start.getMonth(), start.getDate() + 7) < end)
        return null;
    for (let k = 0; k < 7; k++) {
        const date = new Date(start.getFullYear(), start.getMonth(), start.getDate() + k);
        if (date < end && date.getDay() === day)
            return [date, new Date(date.getFullYear(), date.getMonth(), date.getDate() + 1)];
    }
    return null;
}

const NUMBER = /^\d+(?:[.,]\d+)?$/u;
const YEAR = /^(19|20)\d\d$/u;
const DAY_OF_MONTH = /^(?:[1-9]|[12]\d|3[01])$/u;
const number = text => Number(text.replace(',', '.'));

// A size such as "100 mb" or "100mb" at this place in the words: [bytes, how many words], or null
function sizeAt(tokens, at, units) {
    const here = tokens[at];
    if (here === undefined)
        return null;
    if (NUMBER.test(here) && units.has(tokens[at + 1]))
        return [number(here) * units.get(tokens[at + 1]), 2];
    const joined = /^(\d+(?:[.,]\d+)?)(\p{L}+)$/u.exec(here);
    if (joined && units.has(joined[2]))
        return [number(joined[1]) * units.get(joined[2]), 1];
    return null;
}

// What was typed, as the helper's question, or null when no rule read anything in it, or read it two ways ("monday
// march", "today yesterday"): then the search goes on as usual, rather than answering something else. One word is never
// a question: the row comes before GNOME's own and would take Enter from the app someone is typing the name of
// ("photos", "today"), and words are counted as typed, so that "aujourd'hui" is one
export function parseRemembered(text, compiled, now = new Date()) {
    const {phrases, templates, units, locale, firstDay} = compiled;
    const typed = text.trim().split(/\s+/u).length;
    if (typed < 2 || typed > MOST_TYPED)
        return null;
    const tokens = fold(text, locale).split(/\s+/u).filter(Boolean);

    // "3 days ago", "vor 3 tagen": a number in its place in a template
    const templateAt = at => templates.find(({words: shape}) => shape.every((part, place) =>
        part === '{n}' ? /^\d{1,3}$/u.test(tokens[at + place] ?? '') : part === tokens[at + place]));

    const kinds = new Set();
    const words = [];
    let range = null;
    let day = null;
    let month = null;
    let monthDay = null;
    let year = null;
    let last = false;
    let since = false;
    let min = null;
    let max = null;
    let order = 'modified';
    let clash = false;
    // What the word before this one was: a type of phrase, or 'word'
    let before = null;
    let i = 0;
    while (i < tokens.length) {
        const template = templateAt(i);
        if (template) {
            const slot = template.words.indexOf('{n}');
            clash ||= range !== null;
            range = ago(template.unit, Number(tokens[i + slot]), now, firstDay);
            i += template.words.length;
            before = 'range';
            continue;
        }
        // "5 march": the day of the month, before its month
        if (DAY_OF_MONTH.test(tokens[i]) && phrases.get(tokens[i + 1])?.type === 'month') {
            monthDay = Number(tokens[i]);
            i++;
            before = 'number';
            continue;
        }
        // The longest phrase that starts here
        let what = null;
        let length = 0;
        for (let size = Math.min(LONGEST_PHRASE, tokens.length - i); size > 0 && !what; size--) {
            what = phrases.get(tokens.slice(i, i + size).join(' ')) ?? null;
            length = what ? size : 0;
        }
        if (what?.type === 'over' || what?.type === 'under') {
            const size = sizeAt(tokens, i + length, units);
            if (size) {
                if (what.type === 'over') {
                    min = size[0];
                    order = 'size';
                } else {
                    max = size[0];
                }
                i += length + size[1];
                before = what.type;
                continue;
            }
            // A word of its own, not a size: left to the file's name
            what = null;
            length = 0;
        }
        if (!what) {
            // A year is one beside a month, "from" or "since"; elsewhere it is part of a name ("report 2024 pdf")
            if (YEAR.test(tokens[i]) && (['filler', 'since', 'month', 'year'].includes(before) || phrases.get(tokens[i + 1])?.type === 'month')) {
                clash ||= year !== null;
                year = Number(tokens[i]);
                before = 'year';
            } else {
                // A short word may be the start of one still being typed
                if (tokens[i].length >= SHORTEST_WORD)
                    words.push(tokens[i]);
                before = 'word';
            }
            i++;
            continue;
        }
        switch (what.type) {
        case 'kind': kinds.add(what.kind); break;
        case 'range':
            clash ||= range !== null;
            range = period(what, now, firstDay);
            break;
        case 'day':
            clash ||= day !== null;
            day = what.day;
            break;
        case 'month':
            clash ||= month !== null;
            month = what.month;
            // "march 5": the day of the month, unless the number starts a "5 days ago" or a size
            if (monthDay === null && DAY_OF_MONTH.test(tokens[i + length] ?? '') && !templateAt(i + length) && !units.has(tokens[i + length + 1])) {
                monthDay = Number(tokens[i + length]);
                length++;
            }
            break;
        case 'last': last = true; break;
        case 'since': since = true; break;
        case 'big': min = Math.max(min ?? 0, BIG); order = 'size'; break;
        case 'small': max = Math.min(max ?? Infinity, SMALL); order = 'small'; break;
        }
        before = what.type;
        i += length;
    }

    if (clash || (last && day === null) || (monthDay !== null && month === null))
        return null;
    if (day !== null) {
        // A weekday is the last such day, or the one inside a week that was named; with a month or a year it is no day
        if (month !== null || year !== null)
            return null;
        range = range ? dayIn(range, day) : dayPeriod(day, last, now);
    } else if (month !== null) {
        if (range)
            return null;
        range = monthDay === null ? monthPeriod(month, year, now) : dateOf(month, monthDay, year, now);
    } else if (year !== null) {
        if (range)
            return null;
        range = [startOfDay(year, 0, 1), startOfDay(year + 1, 0, 1)];
    }
    // "since" needs a date, and leaves the period open to now
    if (range === null && (since || day !== null || month !== null || year !== null))
        return null;
    if (!kinds.size && !range && min === null && max === null)
        return null;

    const question = {kinds: [...kinds], order, words: words.slice(0, MOST_WORDS)};
    if (range) {
        question.after = range[0].toISOString();
        if (!since)
            question.before = range[1].toISOString();
    }
    if (min !== null)
        question.min = Math.round(min);
    if (max !== null)
        question.max = Math.round(max);
    return question;
}
