// The words of "tea 4 min", for the search's timers (timerProvider.js): how long, and what it is called. The words are
// English whatever the session's language, as "remind me" reads them. Kept free of GNOME Shell's imports, so that it runs
// under node and gjs alike.

const UNITS = {
    sec: 1, secs: 1, second: 1, seconds: 1,
    min: 60, mins: 60, minute: 60, minutes: 60,
    hr: 3600, hrs: 3600, hour: 3600, hours: 3600,
};
// The most a timer runs (pill/timer.js LONGEST), and the longest name kept
const LONGEST = 99 * 60;
const NAME = 24;
const WORDS = /^(?:timer\s+)?(.*?)\s*(\d{1,4})\s*([a-z]+)(?:\s+timer)?$/iu;
// A name is plain words: nothing that looks like a sum, a path, an address or a reminder
const PLAIN = /^[\p{L}][\p{L}\p{N}' -]*$/u;

// {name, seconds}, or null. name is '' for "5 min" and "5 min timer".
export function readTimer(text) {
    const typed = String(text ?? '').trim().replace(/\s+/gu, ' ');
    if (typed.length > 60 || /^remind\s/iu.test(typed))
        return null;
    const found = WORDS.exec(typed);
    if (!found)
        return null;
    // An own word of the table only: "constructor" and "toString" are not units
    const word = found[3].toLowerCase();
    const unit = Object.hasOwn(UNITS, word) ? UNITS[word] : undefined;
    const seconds = Number(found[2]) * (unit ?? 0);
    if (!unit || seconds < 1 || seconds > LONGEST)
        return null;
    const raw = found[1].trim();
    if (raw !== '' && !PLAIN.test(raw))
        return null;
    const letters = [...raw];
    const name = letters.slice(0, NAME).join('');
    return {name: name ? name[0].toLocaleUpperCase() + name.slice(1) : '', seconds};
}
