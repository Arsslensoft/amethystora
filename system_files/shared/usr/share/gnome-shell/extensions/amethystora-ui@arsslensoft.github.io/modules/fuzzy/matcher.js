// The forgiving matcher behind the app search, kept free of GNOME Shell's imports so that it can be tried on its
// own (gjs -m). It compares what was typed with the words of each launcher: a whole word or the start of one
// counts most, then the same letters inside a word, then a word with a letter wrong, swapped, missing or extra,
// then the letters of a word in order with some left out (vowels dropped), then the capital letters of a name
// (initials), and last pairs and single letters in common. A launcher's name counts more than its keywords and
// its command, and those more than its description. Text is compared folded: lower case, and with accents and
// other combining marks taken off after the canonical decomposition, so that any script matches itself and an
// accented letter never breaks a word in two.

// How much each part of a launcher counts
const WEIGHTS = {names: 1, keywords: 0.6, command: 0.55, description: 0.3};
// A name's later words count a little less than its first
const LATER_WORD = 0.85;
// What each kind of evidence is worth for one typed word against one word of a launcher
const EXACT = 1;
const PREFIX = 0.75;
const INSIDE = 0.5;
const TYPO = 0.45;
const IN_ORDER = 0.5;
const INITIALS = 0.8;
const INITIALS_START = 0.65;
const PAIRS = 0.2;
const LETTERS = 0.05;
// A launcher with less than this share of the best one's evidence is not shown, unless GNOME found it itself; nor,
// once a typed word has four letters or more, one with less than this much evidence at all (a few letter pairs in
// common are a coincidence, not a match)
const CUT_OFF = 0.15;
const FLOOR = 0.1;

const MARKS = /\p{M}/gu;
const WORD = /[\p{L}\p{N}]+/gu;
const CAPITAL = /\p{Lu}/gu;

export function fold(text) {
    return (text ?? '').normalize('NFD').replace(MARKS, '').toLowerCase();
}

export function words(text) {
    return fold(text).match(WORD) ?? [];
}

// The capital letters of a name, as one word ("VSCodium" and "Mission Center" give "vsc" and "mc"); a name with
// fewer than two has none
function initialsOf(name) {
    const capitals = (name ?? '').normalize('NFD').match(CAPITAL) ?? [];
    return capitals.length >= 2 ? fold(capitals.join('')) : '';
}

// A launcher or an action, made once and kept: names, keywords and descriptions are any text; command is the
// program's name. Other is text GNOME's own search also reads (the generic name, the ID), which decides only
// whether GNOME found it by the start of a word.
export function makeEntry(id, {names = [], keywords = [], command = '', description = '', other = []}) {
    const unique = list => [...new Set(list)];
    const entry = {
        id,
        label: fold(names[0] ?? id),
        fields: {
            names: unique(names.flatMap(words)),
            keywords: unique(keywords.flatMap(words)),
            command: unique(words(command)),
            description: unique(words(description)),
        },
        initials: unique(names.map(initialsOf).filter(Boolean)),
    };
    entry.all = new Set([...Object.values(entry.fields).flat(), ...other.flatMap(words)]);
    return entry;
}

// The words typed, each one on its own; punctuation between them is ignored
export function queryWords(terms) {
    return terms.flatMap(words);
}

// Optimal string alignment distance: substitutions, insertions, deletions and two neighbours swapped each cost one
function distance(a, b) {
    const rows = [];
    for (let i = 0; i <= a.length; i++) {
        rows.push([i]);
        for (let j = 1; j <= b.length; j++) {
            if (i === 0) {
                rows[0].push(j);
                continue;
            }
            const cost = a[i - 1] === b[j - 1] ? 0 : 1;
            let d = Math.min(rows[i - 1][j] + 1, rows[i][j - 1] + 1, rows[i - 1][j - 1] + cost);
            if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1])
                d = Math.min(d, rows[i - 2][j - 2] + 1);
            rows[i].push(d);
        }
    }
    return rows[a.length][b.length];
}

// The fewest mistakes between what was typed and the start of a word, so that a typo in the first letters of a
// long name is found before the rest has been typed
function typoDistance(query, word) {
    let best = Infinity;
    for (let length = query.length - 1; length <= query.length + 1; length++) {
        if (length < 1 || length > word.length)
            continue;
        best = Math.min(best, distance(query, word.slice(0, length)));
    }
    return best;
}

// The two are the same but for one pair of neighbouring letters the other way round
function swappedOnce(a, b) {
    if (a.length !== b.length)
        return false;
    const differ = [...a].flatMap((letter, i) => letter === b[i] ? [] : [i]);
    return differ.length === 2 && differ[1] === differ[0] + 1 && a[differ[0]] === b[differ[1]] && a[differ[1]] === b[differ[0]];
}

// Two slips in a word of six or seven letters, one of them two neighbours swapped ("setigns", "fierfx"): with the
// pair put back, one mistake is left
function swapAndSlip(query, word) {
    for (let i = 0; i < query.length - 1; i++) {
        if (query[i] === query[i + 1])
            continue;
        const fixed = query.slice(0, i) + query[i + 1] + query[i] + query.slice(i + 2);
        if (typoDistance(fixed, word) <= 1)
            return true;
    }
    return false;
}

// The letters typed appear in the word in the same order, from its first letter on
function inOrder(query, word) {
    let at = 0;
    for (const letter of word) {
        if (letter === query[at])
            at++;
        if (at === query.length)
            return true;
    }
    return false;
}

function pairs(text) {
    const list = [];
    for (let i = 0; i + 1 < text.length; i++)
        list.push(text.slice(i, i + 2));
    return list;
}

// The share of letter pairs the two have in common (each pair counted once on each side)
function pairsInCommon(query, word) {
    const left = pairs(query);
    const right = pairs(word);
    if (left.length === 0 || right.length === 0)
        return 0;
    const pool = new Map();
    for (const pair of right)
        pool.set(pair, (pool.get(pair) ?? 0) + 1);
    let shared = 0;
    for (const pair of left) {
        const count = pool.get(pair) ?? 0;
        if (count > 0) {
            shared++;
            pool.set(pair, count - 1);
        }
    }
    return 2 * shared / (left.length + right.length);
}

function lettersInCommon(query, word) {
    let shared = 0;
    for (const letter of new Set(query)) {
        if (word.includes(letter))
            shared++;
    }
    return shared / new Set(query).size;
}

// What one typed word is worth against one word of a launcher, from 0 to 1
function evidence(query, word) {
    if (word === query)
        return EXACT;
    if (word.startsWith(query))
        return PREFIX + 0.2 * query.length / word.length;
    if (query.length >= 2 && word.includes(query))
        return INSIDE + 0.15 * query.length / word.length;

    // Single letters in common count only for a word of up to three letters ("fx", "tb"): for anything longer they
    // would make a mistyped string find whatever shares a letter with it
    let best = query.length <= 3 ? LETTERS * lettersInCommon(query, word) : 0;
    if (query.length >= 2)
        best = Math.max(best, PAIRS * pairsInCommon(query, word));
    // A typo is looked for only from five letters on: below that, too many short words are one letter apart
    if (query.length >= 5 && (query[0] === word[0] || query.length >= 6)) {
        const mistakes = typoDistance(query, word);
        if (mistakes <= (query.length >= 8 ? 2 : 1) ||
            (mistakes === 2 && query.length >= 6 && query[0] === word[0] && swapAndSlip(query, word)))
            best = Math.max(best, TYPO * (1 - mistakes / query.length));
    }
    // At four letters, only two neighbours swapped ("clac", "lgos"), as fingers do: few short words are that far apart
    if (query.length === 4 && query[0] === word[0] && word.length >= 4 && swappedOnce(query, word.slice(0, 4)))
        best = Math.max(best, TYPO * 0.75);
    if (query.length >= 4 && query[0] === word[0] && query.length <= word.length && inOrder(query, word))
        best = Math.max(best, IN_ORDER * Math.sqrt(query.length / word.length));
    return best;
}

// How much evidence a launcher has for everything typed: each typed word adds its best match on its own, so a
// launcher does not need every word
export function score(entry, query) {
    let total = 0;
    for (const typed of query) {
        let best = 0;
        for (const [field, weight] of Object.entries(WEIGHTS)) {
            const list = entry.fields[field];
            for (let index = 0; index < list.length; index++) {
                const later = field === 'names' && index > 0 ? LATER_WORD : 1;
                best = Math.max(best, weight * later * evidence(typed, list[index]));
            }
        }
        if (typed.length >= 2) {
            for (const initials of entry.initials) {
                if (initials === typed)
                    best = Math.max(best, INITIALS);
                else if (initials.startsWith(typed))
                    best = Math.max(best, INITIALS_START);
            }
        }
        total += best;
    }
    return total;
}

// Whether every typed word starts a word GNOME's own search reads: such a result keeps GNOME's place for it
export function startsWords(entry, query) {
    return query.every(typed => [...entry.all].some(word => word.startsWith(typed)));
}

// The order to show: first what GNOME's own search found by the start of a word, in GNOME's own order (how well the
// words matched, then usage; where GNOME has nothing to tell two apart, the closer match first); then the launchers
// with enough evidence, strongest first, GNOME's usage order between equals, then by name; and last whatever else
// GNOME found, so that nothing it finds is lost.
//   entries: the launchers and actions that may be shown, by ID
//   found: GNOME's own results, in its order
//   compare: GNOME's usage order of two IDs
//   group: how well GNOME's search matched an app (lower is better); the actions come after the apps
export function rank(entries, terms, found, compare = () => 0, group = () => 0) {
    const query = queryWords(terms);
    if (query.length === 0)
        return found;

    const values = new Map();
    let best = 0;
    for (const entry of entries.values()) {
        const value = score(entry, query);
        values.set(entry.id, value);
        best = Math.max(best, value);
    }

    const first = [];
    const later = [];
    for (const id of found) {
        const entry = entries.get(id);
        if (!entry || startsWords(entry, query))
            first.push(id);
        else
            later.push(id);
    }
    first.sort((a, b) => group(a) - group(b) || compare(a, b) || (values.get(b) ?? 0) - (values.get(a) ?? 0));

    const shown = new Set(first);
    const scored = [];
    for (const entry of entries.values()) {
        const value = values.get(entry.id);
        if (value > 0 && !shown.has(entry.id))
            scored.push({entry, value});
    }
    const least = Math.max(CUT_OFF * best, query.some(typed => typed.length >= 4) ? FLOOR : 0);
    const strong = scored.filter(({value}) => value >= least);
    strong.sort((a, b) => b.value - a.value || compare(a.entry.id, b.entry.id) ||
        a.entry.label.localeCompare(b.entry.label));

    const order = [...first, ...strong.map(({entry}) => entry.id)];
    for (const id of order)
        shown.add(id);
    return [...order, ...later.filter(id => !shown.has(id))];
}
