// Sums and conversions for the search: "1284 * 0.18", "3.5 km in miles", "21 °C in °F". The text is read by a
// tokenizer and a recursive-descent parser written here, never handed to anything that runs code, and anything that
// is not plainly a sum or a conversion (a date, a version, a phone number, a word, a lone number) gives null rather
// than an error. Numbers are read and written the way the session's language writes them (its decimal and group
// separators, and Arabic-Indic digits), through Intl. Kept free of GNOME Shell's imports, so that it runs under node
// and gjs alike (build_files/shared/test-search-calc.mjs).
//
//   evaluate(text, {locale, connectors})
//     locale       a BCP 47 tag ('fr-FR'); localeTag() makes one from an LC_NUMERIC name, which numericLocale() picks
//     connectors   the words that join two units in the session's language ("en", "vers"), lower case
//   returns null, or {kind: 'sum'|'conversion', value, display, copy, unit?, expression}

import {convert, findUnit} from './units.js';

// What the parser accepts at most: longer or deeper text is not a sum anyone types
const MAX_LENGTH = 200;
const MAX_TOKENS = 64;
const MAX_DEPTH = 16;
const MAX_DIGITS = 30;
const MAX_EXPONENT = 1000;
const MAX_MAGNITUDE = 1e300;

// The words and arrows that join two units in every language, besides the session's own
const CONNECTORS = ['in', 'to', 'as', 'into', '->', '→', '='];

const OPERATORS = new Map([
    ['+', '+'], ['-', '-'], ['−', '-'],
    ['*', '*'], ['×', '*'], ['·', '*'],
    ['/', '/'], ['÷', '/'],
    ['^', '^'], ['%', '%'], ['(', '('], [')', ')'],
]);

// Arabic's decimal and group separators, read in every language, and the spaces a group may be written with
const ARABIC_DECIMAL = '٫';
const GROUPS = ['٬', ' ', ' '];

const LETTER = /\p{L}/u;
const SUPERSCRIPT = /[²³]/u;

// The value of a digit in any of the three sets of digits read, or -1
function digitValue(ch) {
    const code = ch.codePointAt(0);
    if (code >= 0x30 && code <= 0x39)
        return code - 0x30;
    if (code >= 0x660 && code <= 0x669)
        return code - 0x660;
    if (code >= 0x6f0 && code <= 0x6f9)
        return code - 0x6f0;
    return -1;
}
const isDigit = ch => ch !== undefined && digitValue(ch) >= 0;

// The locale numbers are written in, from the environment as POSIX resolves LC_NUMERIC: LC_ALL, then LC_NUMERIC, then
// LANG, each only when set and not empty, else C. LANGUAGE is a list for messages only and never counts here (GLib's
// get_language_names_with_category puts it first for every category). env maps a variable's name to its value
export function numericLocale(env) {
    for (const name of ['LC_ALL', 'LC_NUMERIC', 'LANG']) {
        if (env?.[name])
            return env[name];
    }
    return 'C';
}

// An LC_NUMERIC locale name as a BCP 47 tag: fr_FR.UTF-8 → fr-FR; C and POSIX → en-US
export function localeTag(name) {
    const base = (name ?? '').split(/[.@]/)[0];
    if (!base || base === 'C' || base === 'POSIX')
        return 'en-US';
    const tag = base.replace('_', '-');
    try {
        return Intl.NumberFormat.supportedLocalesOf([tag]).length ? tag : 'en-US';
    } catch {
        return 'en-US';
    }
}

// The locale's separators, as Intl writes them
function separators(locale) {
    const parts = new Intl.NumberFormat(locale).formatToParts(1234.5);
    const decimal = parts.find(part => part.type === 'decimal')?.value ?? '.';
    const group = parts.find(part => part.type === 'group')?.value ?? ',';
    const decimals = new Set([decimal, ARABIC_DECIMAL]);
    if (group !== '.')
        decimals.add('.');
    const groups = new Set([group, ...GROUPS]);
    groups.delete(' ');
    for (const mark of decimals)
        groups.delete(mark);
    return {decimals, groups};
}

class NotASum extends Error {}
const fail = () => {
    throw new NotASum();
};

// A number literal at chars[i]: digits, groups of three after a group separator, a fraction, an exponent
function readNumber(chars, i, {decimals, groups}) {
    let digits = '';
    let count = 0;
    const take = () => {
        digits += String(digitValue(chars[i]));
        count++;
        i++;
    };

    // A literal that starts with 0 and another digit is a date, a code or a phone number, not a sum
    if (digitValue(chars[i]) === 0 && isDigit(chars[i + 1]))
        fail();
    while (isDigit(chars[i]))
        take();
    // A group separator only ever stands before exactly three digits
    while (groups.has(chars[i]) && isDigit(chars[i + 1]) && isDigit(chars[i + 2]) && isDigit(chars[i + 3]) &&
        !isDigit(chars[i + 4])) {
        i++;
        take();
        take();
        take();
    }
    if (decimals.has(chars[i]) && isDigit(chars[i + 1])) {
        digits += '.';
        i++;
        while (isDigit(chars[i]))
            take();
    }
    if ((chars[i] === 'e' || chars[i] === 'E') &&
        (isDigit(chars[i + 1]) || ((chars[i + 1] === '+' || chars[i + 1] === '-') && isDigit(chars[i + 2])))) {
        digits += 'e';
        i++;
        if (chars[i] === '+' || chars[i] === '-')
            digits += chars[i++];
        while (isDigit(chars[i]))
            take();
    }
    if (count > MAX_DIGITS)
        fail();
    // A separator left hanging after a number ("1.2.3", "1,5" in English) is nothing this reads
    if (decimals.has(chars[i]) || groups.has(chars[i]))
        fail();
    return [Number(digits), i];
}

function tokenize(text, separatorSet) {
    const chars = [...text];
    const tokens = [];
    let i = 0;
    while (i < chars.length) {
        const ch = chars[i];
        if (/\s/u.test(ch)) {
            i++;
        } else if (isDigit(ch)) {
            const [value, next] = readNumber(chars, i, separatorSet);
            tokens.push({type: 'number', value});
            i = next;
        } else if (ch === '*' && chars[i + 1] === '*') {
            tokens.push({type: 'op', value: '^'});
            i += 2;
        } else if (ch === '-' && chars[i + 1] === '>') {
            tokens.push({type: 'word', value: '->'});
            i += 2;
        } else if (OPERATORS.has(ch)) {
            tokens.push({type: 'op', value: OPERATORS.get(ch)});
            i++;
        } else if (ch === '→' || ch === '=' || ch === '"' || ch === '\'') {
            tokens.push({type: 'word', value: ch});
            i++;
        } else if (ch === '°' || LETTER.test(ch)) {
            let word = ch;
            i++;
            while (i < chars.length && LETTER.test(chars[i]))
                word += chars[i++];
            if (SUPERSCRIPT.test(chars[i] ?? ''))
                word += chars[i++];
            if (word === '°')
                fail();
            tokens.push({type: 'word', value: word});
        } else {
            fail();
        }
        if (tokens.length > MAX_TOKENS)
            fail();
    }
    return tokens;
}

// Every value worked out on the way has to be a finite number of a size a sum can have
function checked(value) {
    if (!Number.isFinite(value) || Math.abs(value) > MAX_MAGNITUDE)
        fail();
    return value;
}

class Parser {
    constructor(tokens) {
        this.tokens = tokens;
        this.at = 0;
        this.depth = 0;
        this.operators = 0;
    }

    peek() {
        const token = this.tokens[this.at];
        return token?.type === 'op' ? token.value : null;
    }

    // expr = term {("+" | "-") term}; a right operand that is only a percentage is a share of the left one
    expr() {
        let left = this.term().value;
        for (let op = this.peek(); op === '+' || op === '-'; op = this.peek()) {
            this.at++;
            this.operators++;
            const right = this.term();
            const amount = right.percent ? checked(left * right.share / 100) : right.value;
            left = checked(op === '+' ? left + amount : left - amount);
        }
        return left;
    }

    // term = signed {("*" | "/") signed}
    term() {
        const first = this.signed();
        let value = first.value;
        let alone = true;
        for (let op = this.peek(); op === '*' || op === '/'; op = this.peek()) {
            this.at++;
            this.operators++;
            alone = false;
            const right = this.signed().value;
            if (op === '/' && right === 0)
                fail();
            value = checked(op === '*' ? value * right : value / right);
        }
        return alone && first.percent ? first : {value, percent: false};
    }

    // signed = ("-" | "+") signed | power
    signed() {
        const op = this.peek();
        if (op === '-' || op === '+') {
            this.at++;
            if (++this.depth > MAX_DEPTH * 4)
                fail();
            const inner = this.signed();
            this.depth--;
            return op === '-'
                ? {value: checked(-inner.value), percent: inner.percent, share: -inner.share}
                : inner;
        }
        return this.power();
    }

    // power = postfix ["^" signed], so that 2^3^2 is 2^9 and -2^2 is -4
    power() {
        const base = this.postfix();
        if (this.peek() !== '^')
            return base;
        this.at++;
        this.operators++;
        const exponent = this.signed().value;
        if (Math.abs(exponent) > MAX_EXPONENT || (base.value === 0 && exponent < 0) ||
            (base.value < 0 && !Number.isInteger(exponent)))
            fail();
        return {value: checked(base.value ** exponent), percent: false};
    }

    // postfix = primary ["%"]
    postfix() {
        const value = this.primary();
        if (this.peek() !== '%')
            return {value, percent: false};
        this.at++;
        this.operators++;
        return {value: checked(value / 100), percent: true, share: value};
    }

    // primary = number | "(" expr ")"
    primary() {
        const token = this.tokens[this.at];
        if (token?.type === 'number') {
            this.at++;
            return checked(token.value);
        }
        if (this.peek() === '(') {
            this.at++;
            if (++this.depth > MAX_DEPTH)
                fail();
            const value = this.expr();
            if (this.peek() !== ')')
                fail();
            this.at++;
            this.depth--;
            return value;
        }
        return fail();
    }
}

// The units of a conversion from the words after the sum: one unit, a connector, another unit (each unit may be
// more than one word, "nautical miles"), or null
function unitsOf(words, connectors) {
    for (let i = 1; i < words.length - 1; i++) {
        if (!connectors.has(words[i].toLowerCase()))
            continue;
        const from = findUnit(words.slice(0, i).join(' '));
        const to = findUnit(words.slice(i + 1).join(' '));
        if (from && to && from.dimension === to.dimension)
            return [from, to];
    }
    return null;
}

function format(value, locale, options) {
    // -0 reads as "-0"
    return new Intl.NumberFormat(locale, options).format(value === 0 ? 0 : value);
}

function sumOptions(value) {
    const size = Math.abs(value);
    if (Number.isInteger(value) && size < 2 ** 53)
        return {maximumFractionDigits: 0};
    if (size >= 1e15 || (size > 0 && size < 1e-6))
        return {notation: 'scientific', maximumSignificantDigits: 10};
    return {maximumSignificantDigits: 10};
}

export function evaluate(text, {locale = 'en-US', connectors = []} = {}) {
    if (typeof text !== 'string')
        return null;
    const expression = text.trim();
    if (!expression || [...expression].length > MAX_LENGTH)
        return null;
    // A date (2024-10-28, 28/10/2024) is not a sum, whatever it would come to
    if (/^\d{4}[-/.]\d{1,2}[-/.]\d{1,2}$/.test(expression) || /^\d{1,2}[-/.]\d{1,2}[-/.]\d{4}$/.test(expression))
        return null;

    try {
        try {
            new Intl.NumberFormat(locale);
        } catch {
            locale = 'en-US';
        }
        const tokens = tokenize(expression, separators(locale));
        const firstWord = tokens.findIndex(token => token.type === 'word');
        const sumTokens = firstWord < 0 ? tokens : tokens.slice(0, firstWord);
        const parser = new Parser(sumTokens);
        const value = parser.expr();
        if (parser.at !== sumTokens.length)
            return null;

        if (firstWord < 0) {
            // A lone number is not a sum: a sum needs an operator
            if (parser.operators === 0)
                return null;
            const options = sumOptions(value);
            return {
                kind: 'sum',
                value,
                display: format(value, locale, options),
                copy: format(value, locale, {...options, useGrouping: false}),
                expression,
            };
        }

        const words = tokens.slice(firstWord);
        if (words.some(token => token.type !== 'word'))
            return null;
        const joining = new Set([...CONNECTORS, ...connectors.map(word => word.toLowerCase())]);
        const units = unitsOf(words.map(token => token.value), joining);
        if (!units)
            return null;
        const [from, to] = units;
        const result = checked(convert(value, from, to));
        const options = {maximumSignificantDigits: 6};
        return {
            kind: 'conversion',
            value: result,
            display: `${format(result, locale, options)} ${to.symbol}`,
            copy: format(result, locale, {...options, useGrouping: false}),
            unit: to.symbol,
            expression,
        };
    } catch (e) {
        if (e instanceof NotASum || e instanceof RangeError)
            return null;
        throw e;
    }
}
