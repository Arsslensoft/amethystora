// The words of a question about the time, for the search's time answers (timesProvider.js) and its reminders
// (remindProvider.js): "time in Tokyo", "15:00 Paris in Tokyo", "3pm in UTC+2", "remind me at 17:00 to call the
// garage". Only the words are read here: which zone a place is in, and the arithmetic, are the providers'. Kept free of
// GNOME Shell's imports, so that it runs under node and gjs alike.

// A name as it is compared: lower case, without accents, its spaces and dashes one space ("São Paulo" and "sao-paulo")
export function normalize(name) {
    return name.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase()
        .replace(/(?<=\p{L})[-.'’](?=\p{L})/gu, ' ').replace(/[\s_,]+/gu, ' ').trim();
}

// A time of day at the start of the text: "15:00", "9:30", "3pm", "3 pm", "11:45am". A bare number is not one (it is a
// sum's). Returns {hour, minute, rest} or null.
export function readClock(text) {
    const found = /^(\d{1,2})(?::(\d{2}))?(?:\s*([ap])\.?\s*m\.?)?(?=\s|$)/iu.exec(text);
    if (!found || (found[2] === undefined && found[3] === undefined))
        return null;
    let hour = Number(found[1]);
    const minute = found[2] === undefined ? 0 : Number(found[2]);
    if (minute > 59)
        return null;
    if (found[3]) {
        if (hour < 1 || hour > 12)
            return null;
        hour = hour % 12 + (found[3].toLowerCase() === 'p' ? 12 : 0);
    } else if (hour > 23) {
        return null;
    }
    return {hour, minute, rest: text.slice(found[0].length).trim()};
}

// The words that ask for the time now in a place, before it ("time in", "what time is it in") or after it ("time"),
// and the ones that join two places ("in", "to"): each list a translation of its own, words apart by "|"
function wordsOf(list) {
    return list.split('|').map(word => normalize(word)).filter(Boolean)
        .sort((a, b) => b.length - a.length);
}

// The question, or null: {kind: 'now', place} or {kind: 'at', hour, minute, from, to} (from or to null for the
// session's own zone). words: {before, after, joins}, each a "|" list.
export function readTimeQuestion(text, words) {
    const typed = text.trim().replace(/[?!.]+$/u, '').replace(/\s+/gu, ' ');
    if (!typed || typed.length > 120)
        return null;
    const plain = normalize(typed);
    const before = wordsOf(words.before);
    const after = wordsOf(words.after);
    const joins = wordsOf(words.joins);

    const clock = readClock(typed);
    if (clock) {
        const rest = normalize(clock.rest);
        if (!rest)
            return null;
        // "15:00 in Tokyo": from here
        for (const join of joins) {
            if (rest.startsWith(`${join} `))
                return {kind: 'at', hour: clock.hour, minute: clock.minute, from: null, to: [rest.slice(join.length + 1)]};
        }
        // "15:00 Paris in Tokyo": every way of cutting the words at a joining word, for the provider to try in turn
        const cuts = [];
        const parts = rest.split(' ');
        for (let i = 1; i < parts.length - 1; i++) {
            if (joins.includes(parts[i]))
                cuts.push([parts.slice(0, i).join(' '), parts.slice(i + 1).join(' ')]);
        }
        if (cuts.length)
            return {kind: 'at', hour: clock.hour, minute: clock.minute, cuts};
        // "3pm Paris": there, in the session's own zone
        return {kind: 'at', hour: clock.hour, minute: clock.minute, from: [rest], to: null};
    }
    for (const word of before) {
        if (plain.startsWith(`${word} `))
            return {kind: 'now', place: plain.slice(word.length + 1)};
    }
    for (const word of after) {
        if (plain.endsWith(` ${word}`))
            return {kind: 'now', place: plain.slice(0, -word.length - 1)};
    }
    return null;
}

// "UTC", "GMT", "UTC+2", "utc-03:30", "GMT+5:45" as an offset GLib takes for a zone ("+02:00"), else null
export function offsetZone(name) {
    const found = /^(?:utc|gmt)(?:([+-])(\d{1,2})(?::?(\d{2}))?)?$/u.exec(name.toLowerCase().replace(/\s+/gu, ''));
    if (!found)
        return null;
    if (!found[1])
        return 'UTC';
    const hours = Number(found[2]);
    const minutes = Number(found[3] ?? 0);
    if (hours > 14 || minutes > 59)
        return null;
    return `${found[1]}${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}`;
}
