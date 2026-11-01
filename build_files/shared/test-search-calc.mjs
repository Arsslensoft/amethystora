// The search's sums and conversions (modules/search/calc.js of Amethystora UI), checked under node and gjs alike:
//   node build_files/shared/test-search-calc.mjs
//   gjs -m build_files/shared/test-search-calc.mjs
// Nothing here is node's or gjs's own: a failure throws, which makes either exit with an error.

import {evaluate, localeTag, numericLocale} from
    '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/search/calc.js';

const say = globalThis.print ?? console.log;
const failures = [];
let count = 0;

function expect(text, locale, display, copy) {
    count++;
    const result = evaluate(text, {locale, connectors: []});
    const got = result?.display ?? null;
    if (got !== display)
        failures.push(`${locale} ${JSON.stringify(text)}: expected ${JSON.stringify(display)}, got ${JSON.stringify(got)}`);
    if (copy !== undefined && result?.copy !== copy)
        failures.push(`${locale} ${JSON.stringify(text)}: copied ${JSON.stringify(result?.copy)}, expected ${JSON.stringify(copy)}`);
}

const en = 'en-US';
const nbsp = ' ';

// Sums
for (const [text, display] of [
    ['2+3*4', '14'], ['1284 * 0.18', '231.12'], ['200 + 10%', '220'], ['-2^2', '-4'], ['2^-1', '0.5'],
    ['10/3', '3.333333333'], ['2^3^2', '512'], ['(1+2)*3', '9'], ['1e3+1', '1,001'], ['5−2', '3'], ['6÷4', '1.5'],
    ['3·4', '12'], ['2**3', '8'], ['50%', '0.5'], ['20% * 50', '10'], ['200 - 10%', '180'], ['0.1+0.2', '0.3'],
    ['123456789 * 2', '246,913,578'], ['3 × (2 + 1)', '9'],
])
    expect(text, en, display);
expect('1284 * 0.18', en, '231.12', '231.12');
expect('123456789 * 2', en, '246,913,578', '246913578');

// Conversions: the target's symbol after a no-break space
for (const [text, display] of [
    ['5 km in mi', '3.10686 mi'], ['100 F in C', '37.7778 °C'], ['32 F to C', '0 °C'], ['0 K in C', '-273.15 °C'],
    ['5 kg in lb', '11.0231 lb'], ['12 in in cm', '30.48 cm'], ['1.5 GB in MiB', '1,430.51 MiB'],
    ['90 min in h', '1.5 h'], ['3.5 km in miles', '2.1748 mi'], ['21 °C in °F', '69.8 °F'],
    ['(2+3) km in mi', '3.10686 mi'], ['2 nautical miles in km', '3.704 km'], ['1 GiB -> MB', '1,073.74 MB'],
    ['8 bits as B', '1 B'], ['1 wk → h', '168 h'], ['5\' to cm', '152.4 cm'],
])
    expect(text, en, display.replace(' ', nbsp));
expect('100 F in C', en, `37.7778${nbsp}°C`, '37.7778');
count++;
if (evaluate('5 km en mi', {locale: 'fr-FR', connectors: ['en', 'vers']})?.display !== `3,10686${nbsp}mi`)
    failures.push('fr-FR "5 km en mi" with the translated connectors');

// Nothing at all for what is not a sum
const notSums = [
    '1/0', '(1+2', '42', '-3', '(5)', '2024-10-28', '07-12', '1.2.3', '5 km in kg', 'hello', '100 EUR in USD',
    `1${' '.repeat(198)}+1`, `${'1+'.repeat(32)}1`, `${'('.repeat(17)}1${')'.repeat(17)}+1`, `${'1'.repeat(31)}+1`, '2^1001', '1e400',
    '1e400+1', '1 000', 'in in cm', '2(3)', '5 km', '5 km in', '0^-1', '(-8)^0.5', '1,5 + 1', '10:30', '',
    '   ', '5 km in mi in m', 'Ubuntu 24.04', '1+', '+', '1e300*1e10',
];
for (const text of notSums)
    expect(text, en, null);
// Within the limits: 200 characters, 16 parentheses, 30 digits, an exponent of 1000
expect(`1${' '.repeat(197)}+1`, en, '2');
expect(`${'1+'.repeat(31)}1`, en, '32');
expect(`${'('.repeat(16)}1${')'.repeat(16)}+1`, en, '2');
expect(`${'1'.repeat(30)}*0+1`, en, '1');
expect('1^1000', en, '1');

// The session's own numbers
const fr = new Intl.NumberFormat('fr-FR');
expect('1,5 + 1', 'fr-FR', '2,5', '2,5');
expect('1 234,5 * 2', 'fr-FR', fr.format(2469), '2469');
expect('1284 * 0,18', 'fr-FR', '231,12', '231,12');
expect('1.5 + 1', 'fr-FR', '2,5');
expect('1.000,5 * 2', 'de-DE', '2.001', '2001');
expect('1,5 * 2', 'de-DE', '3');
const ar = new Intl.NumberFormat('ar-EG');
expect('٢+٣', 'ar-EG', ar.format(5), ar.format(5));
expect('2+3', 'ar-EG', ar.format(5));
expect('٢٫٥*٢', 'ar-EG', ar.format(5));

// Locale names
for (const [name, tag] of [['fr_FR.UTF-8', 'fr-FR'], ['C', 'en-US'], ['POSIX', 'en-US'], ['', 'en-US'],
    ['de_DE@euro', 'de-DE'], ['ar_EG.UTF-8', 'ar-EG'], ['C.UTF-8', 'en-US']]) {
    count++;
    if (localeTag(name) !== tag)
        failures.push(`localeTag(${JSON.stringify(name)}) is ${JSON.stringify(localeTag(name))}, not ${tag}`);
}

// Which variable names the numbers' locale: LC_ALL, then LC_NUMERIC, then LANG (POSIX), an empty one as unset, LANGUAGE never
for (const [env, name] of [
    [{LANGUAGE: 'en_US:en', LC_NUMERIC: 'fr_FR.UTF-8', LANG: 'en_US.UTF-8'}, 'fr_FR.UTF-8'],
    [{LC_ALL: 'de_DE.UTF-8', LC_NUMERIC: 'fr_FR.UTF-8', LANG: 'en_US.UTF-8'}, 'de_DE.UTF-8'],
    [{LC_ALL: '', LC_NUMERIC: '', LANG: 'ar_EG.UTF-8', LANGUAGE: 'fr'}, 'ar_EG.UTF-8'],
    [{LC_ALL: null, LC_NUMERIC: 'fr_FR.UTF-8'}, 'fr_FR.UTF-8'],
    [{LANGUAGE: 'fr_FR'}, 'C'],
    [{}, 'C'],
    [undefined, 'C'],
]) {
    count++;
    if (numericLocale(env) !== name)
        failures.push(`numericLocale(${JSON.stringify(env)}) is ${JSON.stringify(numericLocale(env))}, not ${name}`);
}

// What the answer holds
count++;
const answer = evaluate('  3.5 km in miles ', {locale: en});
if (answer?.kind !== 'conversion' || answer.unit !== 'mi' || answer.expression !== '3.5 km in miles' ||
    Math.abs(answer.value - 3500 / 1609.344) > 1e-12)
    failures.push(`the conversion's fields: ${JSON.stringify(answer)}`);

if (failures.length) {
    for (const failure of failures)
        say(`FAIL ${failure}`);
    throw new Error(`${failures.length} of ${count} checks of the search's sums failed`);
}
say(`ok: ${count} checks of the search's sums and conversions`);
