// The search's old words and its rules for what is remembered (modules/search/words.js and remember.js of Amethystora
// UI), and the translations of both lists, under node (gjs reads the rules alone):
//   node build_files/shared/test-search-words.mjs
// A failure throws, which makes either exit with an error.

import {appNamedLike, compileWords, fold, localeTag, matchWords, wordsTable} from
    '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/search/words.js';
import {compileRemember, parseRemembered, rememberWords} from
    '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/search/remember.js';

const say = globalThis.print ?? console.log;
const failures = [];
let count = 0;
const same = (what, got, want) => {
    count++;
    const a = JSON.stringify(got);
    const b = JSON.stringify(want);
    if (a !== b)
        failures.push(`${what}: expected ${b}, got ${a}`);
};

// --- The old words ---------------------------------------------------------------------------------------------------

const id = text => text;
const table = wordsTable(id);
const compiled = compileWords(table, table, 'en');
const ids = text => matchWords(compiled, text.split(' '), 'en').map(entry => entry.id);

same('a whole phrase', ids('task manager'), ['running']);
same('any case', ids('Recycle BIN'), ['trash']);
same('the last word on its way', ids('control pan'), ['settings']);
same('a phrase that is one word', ids('drivers'), ['drivers']);
same('bin alone is no phrase: it may be an app being typed', ids('bin'), []);
same('one word on its way to a phrase says nothing', ids('task'), []);
same('one word on its way to a word says nothing', ids('antivir'), []);
same('words out of order say nothing', ids('manager task'), []);
same('more words than the phrase say nothing', ids('recycle bin now'), []);
same('the clipboard is left to clip:', ids('clip: recycle bin'), []);
same('nothing typed', matchWords(compiled, [], 'en'), []);
same('print screen', ids('print screen'), ['screenshot']);
same('undo an update', ids('undo an update'), ['rollback']);
// Names of other systems' tools that are not everyday words lead nowhere
for (const dropped of ['disk cleanup', 'disk management', 'device manager', 'system restore', 'restore point', 'previous versions',
    'sticky notes', 'screen clip', 'my documents', 'system settings', 'software update', 'software center'])
    same(`${dropped} is not a word here`, ids(dropped), []);

// An installed app named like what was typed keeps Enter: the app, unless it is the entry's own
const apps = [['Document Scanner', 'simple-scan.desktop'], ['Sticky Notes', 'sticky.desktop'], ['Éditeur', 'editor.desktop']];
same('an app being typed', appNamedLike(apps, ['document', 'sc'], 'en'), true);
same('an app being typed, any case', appNamedLike(apps, ['DOCUMENT'], 'en'), true);
same('an app with that very name', appNamedLike(apps, ['sticky', 'notes'], 'en'), true);
same('the entry\'s own app does not count', appNamedLike(apps, ['sticky', 'notes'], 'en', ['sticky.desktop']), false);
same('an app being typed, without its accents', appNamedLike(apps, ['edit'], 'en'), true);
same('another name', appNamedLike(apps, ['image', 'v'], 'en'), false);
same('a name that only contains it', appNamedLike(apps, ['scanner'], 'en'), false);
same('nothing typed', appNamedLike(apps, [], 'en'), false);

// A translated list works beside the English one, without its accents
const german = wordsTable(text => (text.startsWith('recycle bin') ? 'papierkörbe|mülleimer' : text));
const inGerman = compileWords(table, german, 'de');
same('a translated word', matchWords(inGerman, ['PAPIERKORBE'], 'de').map(entry => entry.id), ['trash']);
same('the English word still', matchWords(inGerman, ['recycle', 'bin'], 'de').map(entry => entry.id), ['trash']);

// Everyday words only: no other maker's product or feature, in any of the table's words or notes, in English or in a
// catalog. Four old names of what every system has are kept: task manager, control panel, recycle bin and print screen
const NAMES = [
    'windows', 'microsoft', 'apple', 'macos', 'mac', 'google', 'finder', 'spotlight', 'launchpad', 'siri', 'cortana',
    'explorer', 'notepad', 'wordpad', 'paint', 'snipping', 'excel', 'word', 'powerpoint', 'outlook', 'onedrive', 'icloud',
    'itunes', 'quicktime', 'defender', 'edge', 'safari', 'chrome', 'time machine', 'mission control', 'app store',
    'system preferences', 'system settings', 'activity monitor', 'command prompt', 'cmd', 'powershell', 'this pc',
    'my computer', 'acrobat', 'photoshop', 'dropbox', 'post-it', 'konsole', 'ubuntu', 'kde', 'gnome',
    // English, the names of tools and folders of other systems
    'disk cleanup', 'disk management', 'device manager', 'system restore', 'restore point', 'previous versions', 'sticky notes',
    'screen clip', 'my documents', 'software update', 'software center', 'software centre', 'file explorer',
    // German
    'systemeinstellungen', 'datenträgerbereinigung', 'datenträgerverwaltung', 'geräte-manager', 'systemwiederherstellung',
    'wiederherstellungspunkt', 'haftnotizen', 'softwareaktualisierung', 'frühere versionen', 'meine dokumente', 'software-center',
    // Spanish
    'ajustes del sistema', 'administrador de dispositivos', 'limpieza de disco', 'administración de discos', 'restaurar sistema',
    'punto de restauración', 'versiones anteriores', 'notas adhesivas', 'mis documentos',
    // French
    'paramètres système', 'réglages système', 'gestionnaire de périphériques', 'nettoyage de disque', 'gestion des disques',
    'restauration du système', 'point de restauration', 'versions précédentes', 'notes adhésives', 'mes documents', 'logithèque',
    // Arabic
    'إعدادات النظام', 'مدير الأجهزة', 'تنظيف القرص', 'إدارة الأقراص', 'استعادة النظام', 'نقطة استعادة', 'الإصدارات السابقة',
    'ملاحظات لاصقة', 'مستنداتي',
];
const MARKS = new RegExp(`(?<![\\p{L}\\p{N}-])(${NAMES.join('|')})(?![\\p{L}\\p{N}-])`, 'iu');
const names = text => {
    count++;
    if (MARKS.test(text))
        failures.push(`"${text}" names another maker's product or feature`);
};
for (const entry of table) {
    for (const text of [entry.words, entry.note, entry.title ?? ''])
        names(text);
    same(`${entry.id} leads somewhere`, ['apps', 'uri', 'shell', 'manual', 'argv'].filter(key => entry[key]).length, 1);
    same(`${entry.id}'s words are folded`, entry.words, fold(entry.words, 'en'));
}
// Every id once, every phrase once
same('each id once', new Set(table.map(entry => entry.id)).size, table.length);
const phrases = table.flatMap(entry => entry.words.split('|'));
same('each phrase once', new Set(phrases).size, phrases.length);

// The session's language, whatever its name looks like
for (const [name, tag] of [['de_DE.UTF-8', 'de-DE'], ['sr@latin', 'sr'], ['zh_Hant_TW.UTF-8', 'zh-Hant-TW'], ['C', 'en'],
    ['POSIX', 'en'], [undefined, 'en'], ['not a language!', 'en']])
    same(`the tag of ${name}`, localeTag(name), tag);

// --- What is remembered ----------------------------------------------------------------------------------------------

const rules = compileRemember(rememberWords(id), rememberWords(id), 'en-GB');
// A Wednesday
const NOW = new Date(2026, 9, 28, 15, 30);
const iso = (y, m, d) => new Date(y, m - 1, d).toISOString();
const ask = text => parseRemembered(text, rules, NOW);
const range = (y1, m1, d1, y2, m2, d2) => ({after: iso(y1, m1, d1), before: iso(y2, m2, d2)});

same('pdf from last week', ask('pdf from last week'),
    {kinds: ['pdf'], order: 'modified', words: [], ...range(2026, 10, 19, 2026, 10, 26)});
same('big videos', ask('big videos'), {kinds: ['video'], order: 'size', words: [], min: 10485760});
same('photos yesterday', ask('photos yesterday'),
    {kinds: ['image'], order: 'modified', words: [], ...range(2026, 10, 27, 2026, 10, 28)});
for (const word of ['today', 'photos', 'pdf', 'big', 'march', 'tuesday', '2024'])
    same(`one word is never a question: ${word}`, ask(word), null);
same('today', ask('from today'), {kinds: [], order: 'modified', words: [], ...range(2026, 10, 28, 2026, 10, 29)});
same('a name with a date', ask('Lease from march'),
    {kinds: [], order: 'modified', words: ['lease'], ...range(2026, 3, 1, 2026, 4, 1)});
same('a month not yet come is last year\'s', ask('photos from december'),
    {kinds: ['image'], order: 'modified', words: [], ...range(2025, 12, 1, 2026, 1, 1)});
same('a month and a year', ask('invoice march 2024'),
    {kinds: [], order: 'modified', words: ['invoice'], ...range(2024, 3, 1, 2024, 4, 1)});
same('a year after from', ask('from 2024'), {kinds: [], order: 'modified', words: [], ...range(2024, 1, 1, 2025, 1, 1)});
same('a year among words is not a rule', ask('report 2024'), null);
same('a sum is not a rule', ask('1284 * 0.18'), null);
same('a weekday', ask('docx from tuesday'),
    {kinds: ['document'], order: 'modified', words: [], ...range(2026, 10, 27, 2026, 10, 28)});
same('today\'s weekday is today', ask('from wednesday'), {kinds: [], order: 'modified', words: [], ...range(2026, 10, 28, 2026, 10, 29)});
same('last and today\'s weekday is a week back', ask('last wednesday'),
    {kinds: [], order: 'modified', words: [], ...range(2026, 10, 21, 2026, 10, 22)});
same('days ago', ask('3 days ago'), {kinds: [], order: 'modified', words: [], ...range(2026, 10, 25, 2026, 10, 26)});
same('a month ago, across the year', parseRemembered('2 months ago', rules, new Date(2026, 0, 15)),
    {kinds: [], order: 'modified', words: [], ...range(2025, 11, 1, 2025, 12, 1)});
same('over a size', ask('invoice over 2 mb'), {kinds: [], order: 'size', words: ['invoice'], min: 2097152});
same('over a size, written together', ask('videos larger than 1.5gb'), {kinds: ['video'], order: 'size', words: [], min: 1610612736});
same('under a size', ask('under 100 kb'), {kinds: [], order: 'modified', words: [], max: 102400});
same('over with no size is a word', ask('game over'), null);
same('small', ask('small images'), {kinds: ['image'], order: 'small', words: [], max: 1048576});
same('words it cannot read', ask('holiday plans'), null);
same('only filler', ask('the file from'), null);
same('accents and case are one', ask('RÉSUMÉ pdf').words, ['resume']);
same('words beyond the limit are left out', ask('pdf aa1 bb2 cc3 dd4 ee5').words, ['aa1', 'bb2', 'cc3', 'dd4']);

// A word shorter than three letters may be the start of one still being typed: it is not asked for
same('a short leftover word is ignored', ask('document sc'), {kinds: ['document'], order: 'modified', words: []});
same('a longer one is asked for', ask('document scan').words, ['scan']);
same('a short word among others', ask('lease q3 pdf').words, ['lease']);
// More than sixteen words are a paste
same('sixteen words are a question', ask(`pdf ${'abc '.repeat(13)}last week`) !== null, true);
same('seventeen are not', ask(`pdf ${'abc '.repeat(14)}last week`), null);

// A date that is read two ways, or not fully, is no question: the search goes on as usual
same('a weekday of last week', ask('monday last week'),
    {kinds: [], order: 'modified', words: [], ...range(2026, 10, 19, 2026, 10, 20)});
same('a weekday of this week', ask('monday this week'),
    {kinds: [], order: 'modified', words: [], ...range(2026, 10, 26, 2026, 10, 27)});
same('a weekday of a month is many days', ask('monday last month'), null);
same('a weekday that is not today', ask('monday today'), null);
same('a weekday and a month', ask('monday march'), null);
same('two periods', ask('today yesterday'), null);
same('two months', ask('march april'), null);
same('last without a weekday', ask('last report'), null);
same('since without a date', ask('pdf since'), null);
same('since is an open period', ask('since last week'), {kinds: [], order: 'modified', words: [], after: iso(2026, 10, 19)});
same('since a month', ask('pdf since march'), {kinds: ['pdf'], order: 'modified', words: [], after: iso(2026, 3, 1)});
same('since a year', ask('since 2024'), {kinds: [], order: 'modified', words: [], after: iso(2024, 1, 1)});
// The number beside a month is the day
same('a month and its day', ask('march 5'), {kinds: [], order: 'modified', words: [], ...range(2026, 3, 5, 2026, 3, 6)});
same('a day and its month', ask('5 march'), {kinds: [], order: 'modified', words: [], ...range(2026, 3, 5, 2026, 3, 6)});
same('a name beside it', ask('invoice march 5').words, ['invoice']);
same('a day, a month and a year', ask('5 march 2024'), {kinds: [], order: 'modified', words: [], ...range(2024, 3, 5, 2024, 3, 6)});
same('a day not come yet is last year\'s', ask('photos november 3'),
    {kinds: ['image'], order: 'modified', words: [], ...range(2025, 11, 3, 2025, 11, 4)});
same('a day today is this year\'s', ask('october 28 pdf'), {kinds: ['pdf'], order: 'modified', words: [], ...range(2026, 10, 28, 2026, 10, 29)});
same('a date that is not one', ask('february 30'), null);
same('a number that is not a day is a name', ask('march 400').words, ['400']);
same('and a short one is dropped like any short word', ask('march 40').words, []);
same('a number that starts "3 days ago" is not a day', ask('march 3 days ago'), null);
// A year is one beside a month, "from" or "since"; elsewhere it is a part of a name
same('a year in a name beside a kind', ask('report 2024 pdf'), {kinds: ['pdf'], order: 'modified', words: ['report', '2024']});
same('a year after a kind', ask('pdf 2024').words, ['2024']);
same('from a year beside a kind', ask('pdf from 2024'), {kinds: ['pdf'], order: 'modified', words: [], ...range(2024, 1, 1, 2025, 1, 1)});
same('a year after its month', ask('march 2024 pdf').kinds, ['pdf']);
same('a year before its month', ask('2024 march'), {kinds: [], order: 'modified', words: [], ...range(2024, 3, 1, 2024, 4, 1)});
same('two years', ask('from 2023 2024'), null);

// A translated language: its words, its days and months from the language's own data, English beside them
const french = {
    ...rememberWords(id), image: 'photo|photos|image|images', lastWeek: 'la semaine dernière|semaine dernière',
    big: 'gros|grosses|grande|grandes', yesterday: 'hier', today: 'aujourd’hui', filler: "de|du|d'|des|la|le|les|mes|fichier|fichiers",
    units: 'ko|mo|go|to', over: 'plus de', daysAgo: 'il y a {n} jours', since: 'depuis',
};
const inFrench = compileRemember(rememberWords(id), french, 'fr-FR');
const askFr = text => parseRemembered(text, inFrench, NOW);
same('French words', askFr('photos de la semaine dernière'),
    {kinds: ['image'], order: 'modified', words: [], ...range(2026, 10, 19, 2026, 10, 26)});
same('French day, without its accents', askFr('pdf de mardi'),
    {kinds: ['pdf'], order: 'modified', words: [], ...range(2026, 10, 27, 2026, 10, 28)});
same('French month', askFr('facture de février').words, ['facture']);
same('French size', askFr('vidéos plus de 2 go'), {kinds: ['video'], order: 'size', words: [], min: 2147483648});
same('French template', askFr('il y a 2 jours'), {kinds: [], order: 'modified', words: [], ...range(2026, 10, 26, 2026, 10, 27)});
same('an elision', askFr("photos d'hier").kinds, ['image']);
same('English in a French session', askFr('pdf from last week').kinds, ['pdf']);
// One word with an apostrophe is still one word
for (const word of ["aujourd'hui", 'aujourd’hui', "l'image", "d'hier"])
    same(`one word with an elision is never a question: ${word}`, askFr(word), null);
same('two words, one an elision', askFr("aujourd'hui pdf"), {kinds: ['pdf'], order: 'modified', words: [], ...range(2026, 10, 28, 2026, 10, 29)});
same('depuis', askFr('depuis mardi'), {kinds: [], order: 'modified', words: [], after: iso(2026, 10, 27)});

// Digits of another script are numbers
const arabic = {...rememberWords(id), daysAgo: 'قبل {n} أيام', image: 'صور', over: 'أكثر من', units: 'كيلوبايت|ميغابايت|غيغابايت|تيرابايت'};
const inArabic = compileRemember(rememberWords(id), arabic, 'ar');
const askAr = text => parseRemembered(text, inArabic, NOW);
same('Arabic-Indic digits in a template', askAr('pdf قبل ٣ أيام'), {kinds: ['pdf'], order: 'modified', words: [], ...range(2026, 10, 25, 2026, 10, 26)});
same('Arabic-Indic digits in a size', askAr('صور أكثر من ٢ ميغابايت'), {kinds: ['image'], order: 'size', words: [], min: 2097152});
same('extended digits', askAr('pdf قبل ۲ أيام'), {kinds: ['pdf'], order: 'modified', words: [], ...range(2026, 10, 26, 2026, 10, 27)});

// --- The catalogs ----------------------------------------------------------------------------------------------------
// The four languages' lists are read by the rules as shipped: every message is there, every list keeps what the rules count
// on, and they read what a speaker means. Under node only (gjs has no file reader here)

const LANGUAGES = {de: 'de-DE', es: 'es-ES', fr: 'fr-FR', ar: 'ar'};
if (globalThis.process) {
    const fs = await import('node:fs');
    // msgid -> msgstr of the single-line messages (the lists, and the table's notes and titles, are)
    const unquote = text => text.replace(/\\(["\\])/g, '$1');
    const catalogOf = lang => {
        const text = fs.readFileSync(new URL(`../../po/${lang}.po`, import.meta.url), 'utf8').replaceAll('\r\n', '\n');
        const found = new Map();
        for (const match of text.matchAll(/^msgid "((?:[^"\\\n]|\\.)*)"\nmsgstr "((?:[^"\\\n]|\\.)*)"$/gm))
            found.set(unquote(match[1]), unquote(match[2]));
        return found;
    };
    const english = rememberWords(id);
    for (const [lang, locale] of Object.entries(LANGUAGES)) {
        const catalog = catalogOf(lang);
        const through = text => {
            count++;
            if (!catalog.get(text))
                failures.push(`${lang}.po has no translation of "${text}"`);
            return catalog.get(text) ?? text;
        };
        // The old words
        const translatedTable = wordsTable(through);
        const inLanguage = compileWords(table, translatedTable, locale);
        for (const entry of translatedTable) {
            for (const text of [entry.words, entry.note, entry.title ?? ''])
                names(text);
            same(`${lang}: ${entry.id} has words`, entry.words.split('|').filter(Boolean).length > 0, true);
        }
        // No phrase belongs to two entries, in either language, or one search would open two things
        const owner = new Map();
        for (const entry of inLanguage) {
            for (const phrase of entry.phrases.map(words => words.join(' '))) {
                if (owner.has(phrase) && owner.get(phrase) !== entry.id)
                    failures.push(`${lang}: "${phrase}" is in ${owner.get(phrase)} and ${entry.id}`);
                owner.set(phrase, entry.id);
            }
        }
        // What the rules count on
        const lists = rememberWords(through);
        for (const key of Object.keys(english)) {
            if (english[key].includes('{n}'))
                same(`${lang}: ${key} keeps {n}`, lists[key].split('|').every(phrase => phrase.includes('{n}')), true);
            names(lists[key]);
        }
        same(`${lang}: four units, kilobytes to terabytes in order`, lists.units.split('|').length, 4);
        same(`${lang}: since is one word or phrase`, lists.since.split('|').length >= 1, true);
        const ruled = compileRemember(english, lists, locale);
        const asked = text => parseRemembered(text, ruled, NOW);
        // A size in the language's own units
        const unit = lists.units.split('|');
        same(`${lang}: a size in its own unit`, asked(`${lists.over.split('|')[0]} 2 ${unit[1]}`)?.min, 2097152);
        // Every way the language says "last week", "this week", "last year", "this month" and so on reads as the rule (a
        // week starts on the language's own first day; NOW is a Wednesday, day 28)
        const start = 28 - ((3 - ruled.firstDay + 7) % 7);
        for (const [key, what] of [['lastWeek', [2026, 10, start - 7, 2026, 10, start]], ['thisWeek', [2026, 10, start, 2026, 10, start + 7]],
            ['lastMonth', [2026, 9, 1, 2026, 10, 1]], ['thisMonth', [2026, 10, 1, 2026, 11, 1]],
            ['lastYear', [2025, 1, 1, 2026, 1, 1]], ['thisYear', [2026, 1, 1, 2027, 1, 1]]]) {
            for (const phrase of lists[key].split('|')) {
                const got = asked(`pdf ${phrase}`);
                same(`${lang}: "${phrase}"`, got && [got.after, got.before], [iso(...what.slice(0, 3)), iso(...what.slice(3))]);
            }
        }
    }
    // German, as people say it: with "von", "vom", "aus", in the dative
    const inDe = compileRemember(english, rememberWords(text => catalogOf('de').get(text) ?? text), 'de-DE');
    const askDe = text => parseRemembered(text, inDe, NOW);
    same('German: von letzter woche', askDe('pdf von letzter woche'),
        {kinds: ['pdf'], order: 'modified', words: [], ...range(2026, 10, 19, 2026, 10, 26)});
    same('German: von der letzten woche', askDe('pdf von der letzten woche').kinds, ['pdf']);
    same('German: filme vom letzten jahr', askDe('filme vom letzten jahr'),
        {kinds: ['video'], order: 'modified', words: [], ...range(2025, 1, 1, 2026, 1, 1)});
    same('German: aus diesem jahr', askDe('pdf aus diesem jahr'), {kinds: ['pdf'], order: 'modified', words: [], ...range(2026, 1, 1, 2027, 1, 1)});
    same('German: dateien von dieser woche', askDe('dateien von dieser woche'),
        {kinds: [], order: 'modified', words: [], ...range(2026, 10, 26, 2026, 11, 2)});
    same('German: seit letzter woche', askDe('pdf seit letzter woche'), {kinds: ['pdf'], order: 'modified', words: [], after: iso(2026, 10, 19)});
    same('German: a weekday with last', askDe('bericht letzten montag').words, ['bericht']);
}

if (failures.length) {
    for (const failure of failures)
        say(failure);
    throw new Error(`${failures.length} of ${count} failed`);
}
say(`test-search-words: ${count} passed`);
