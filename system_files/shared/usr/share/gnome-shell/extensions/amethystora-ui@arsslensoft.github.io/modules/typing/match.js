// Text shortcuts, the part that needs nothing of GNOME's: which shortcuts a file holds, whether the characters just typed
// end with one of them, and a text with today's date put in. Plain JavaScript, so that node and gjs both run it
// (build_files/shared/test-text-shortcuts.mjs), and the same rules as the helper that writes the file
// (/usr/libexec/amethystora-text-shortcuts): a shortcut the helper would refuse is never taken from the file here.

export const LIMITS = Object.freeze({
    count: 200,
    abbrMin: 2,
    abbrMax: 32,
    textMax: 2000,
    fileMax: 256 * 1024,
});

// The characters that may come just before a shortcut, besides a white space and the start of the text: an opening
// bracket or quotation mark, so that "(;addr" expands and "x;addr" does not
const OPENERS = new Set(['(', '[', '{', '"', '\'', '«', '“', '‘', '‹']);

// White space, the same in the helper: the space separators, the line and paragraph separators, and the five ASCII
// spaces that are not separators
const SPACE = /^[\p{Zs}\p{Zl}\p{Zp}\t\n\v\f\r]$/u;
// What cannot be typed as text: control and format characters and lone surrogates. A shortcut may carry the zero
// width joiner (in an emoji); a text may carry it, the non-joiner and the marks that set the direction of Arabic and
// Hebrew, and line breaks and tabs
const HIDDEN = /^[\p{Cc}\p{Cf}\p{Cs}]$/u;
const ABBR_ALLOWED = new Set(['\u200d']);
const TEXT_ALLOWED = new Set(['\n', '\t', '\u200c', '\u200d', '\u200e', '\u200f', '\u061c']);

export function isSpace(ch) {
    return ch !== undefined && SPACE.test(ch);
}

// Whether a shortcut may start after this character; undefined is the start of what is known
export function isBoundary(ch) {
    return ch === undefined || isSpace(ch) || OPENERS.has(ch);
}

// Whether a character typed is one the buffer keeps: printable, not a space
export function isPrintable(ch) {
    return typeof ch === 'string' && ch !== '' && [...ch].length === 1 && !isSpace(ch) && !HIDDEN.test(ch);
}

// A shortcut's abbreviation: 2 to 32 characters, none of them a space or hidden
export function validAbbr(abbr) {
    if (typeof abbr !== 'string')
        return false;
    const chars = [...abbr];
    if (chars.length < LIMITS.abbrMin || chars.length > LIMITS.abbrMax)
        return false;
    return chars.every(ch => !isSpace(ch) && (!HIDDEN.test(ch) || ABBR_ALLOWED.has(ch)));
}

// Why a text cannot be a shortcut's: 'length', 'character', or null when it can
export function textProblem(text) {
    if (typeof text !== 'string')
        return 'length';
    const chars = [...text];
    if (chars.length < 1 || chars.length > LIMITS.textMax)
        return 'length';
    return chars.every(ch => !HIDDEN.test(ch) || TEXT_ALLOWED.has(ch)) ? null : 'character';
}

// The shortcuts a file holds, {version: 1, shortcuts: [{abbr, text}]}: each that fits the rules, the first of any
// abbreviation twice, the first 200. A file that is not that shape holds none. Returns {map: Map(abbr → text),
// lengths: the abbreviations' lengths in characters, longest first, maxLength, dropped: how many were left out, ok:
// whether it was a shortcuts file at all (when not, its copy is read, as the helper reads it)}
export function parseShortcuts(source) {
    const map = new Map();
    let dropped = 0;
    let data = null;
    if (typeof source === 'string' && source.length <= LIMITS.fileMax) {
        try {
            data = JSON.parse(source);
        } catch {
            data = null;
        }
    }
    const ok = !!data && typeof data === 'object' && Array.isArray(data.shortcuts);
    const list = ok ? data.shortcuts : [];
    for (const entry of list) {
        const abbr = entry?.abbr;
        const text = entry?.text;
        if (map.size >= LIMITS.count || !validAbbr(abbr) || textProblem(text) !== null || map.has(abbr)) {
            dropped++;
            continue;
        }
        map.set(abbr, text);
    }
    const lengths = [...new Set([...map.keys()].map(abbr => [...abbr].length))].sort((a, b) => b - a);
    return {map, lengths, maxLength: lengths[0] ?? 0, dropped, ok};
}

// The longest abbreviation the characters typed end with, where a word starts: the character before it a boundary, or
// none known (the buffer began there). buffer is an array of characters (or a string); null when none
export function find(buffer, lengths, map) {
    const chars = typeof buffer === 'string' ? [...buffer] : buffer;
    for (const length of lengths) {
        if (length > chars.length)
            continue;
        const abbr = chars.slice(chars.length - length).join('');
        if (map.has(abbr) && isBoundary(chars[chars.length - length - 1]))
            return abbr;
    }
    return null;
}

// Whether text before the cursor, as the app says it is, ends with the abbreviation and the space typed after it where
// a word starts: 'yes', 'not-yet' when it ends with the abbreviation alone (the app has not told of the space yet), or
// 'no'
export function endsWithTyped(before, abbr) {
    const chars = typeof before === 'string' ? [...before] : before;
    const n = [...abbr].length;
    // The abbreviation ending `end` characters from the end, a word's start before it
    const at = end => {
        const start = chars.length - end - n;
        return start >= 0 && chars.slice(start, start + n).join('') === abbr && isBoundary(chars[start - 1]);
    };
    if (chars[chars.length - 1] === ' ' && at(1))
        return 'yes';
    return at(0) ? 'not-yet' : 'no';
}

// A locale name as GLib gives it (en_GB.UTF-8, sr_RS@latin, C) as a tag Intl takes; en-US where Intl has none
export function localeTag(name) {
    const base = (name ?? '').split(/[.@]/)[0];
    if (!base || base === 'C' || base === 'POSIX')
        return 'en-US';
    const tag = base.replace('_', '-');
    try {
        return Intl.DateTimeFormat.supportedLocalesOf([tag]).length ? tag : 'en-US';
    } catch {
        return 'en-US';
    }
}

// The long date style of a locale, made once: making one is most of what an expansion with a date costs
const dateFormats = new Map();
function dateFormat(locale) {
    if (!dateFormats.has(locale)) {
        let format;
        try {
            format = new Intl.DateTimeFormat(locale, {dateStyle: 'long'});
        } catch {
            format = new Intl.DateTimeFormat('en-US', {dateStyle: 'long'});
        }
        dateFormats.set(locale, format);
    }
    return dateFormats.get(locale);
}

// A shortcut's text with {date} as the date in the locale's long style; nothing else is a placeholder
export function fillText(text, locale, date = new Date()) {
    if (!text.includes('{date}'))
        return text;
    return text.split('{date}').join(dateFormat(locale).format(date));
}
