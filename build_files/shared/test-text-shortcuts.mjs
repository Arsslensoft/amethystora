// Text shortcuts (Amethystora UI's modules/typing/match.js and /usr/libexec/amethystora-text-shortcuts), checked under
// node and gjs alike:
//   node build_files/shared/test-text-shortcuts.mjs [HELPER]
//   gjs -m build_files/shared/test-text-shortcuts.mjs [HELPER]
// The shell's rules and the helper's are held to one list of cases: every abbreviation and text below is taken or refused
// by both, and the two fixture files are read by both. HELPER is the helper to run (python3 -I), by default the
// repository's. A failure throws, which makes either exit with an error.

import {LIMITS, endsWithTyped, fillText, find, isPrintable, localeTag, parseShortcuts, textProblem, validAbbr} from
    '../../system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/typing/match.js';

const say = globalThis.print ?? console.log;
const failures = [];
let count = 0;
const expect = (what, got, want) => {
    count++;
    if (JSON.stringify(got) !== JSON.stringify(want))
        failures.push(`${what}: expected ${JSON.stringify(want)}, got ${JSON.stringify(got)}`);
};

// --- Running the helper, and files, in node or gjs -------------------------------------------------------------------
// This file's folder (gjs has no URL)
const here = decodeURIComponent(import.meta.url.replace(/^file:\/\//, '').replace(/[^/]*$/, ''));
let system;
if (globalThis.process?.versions?.node) {
    const fs = await import('node:fs');
    const os = await import('node:os');
    const path = await import('node:path');
    const {spawnSync} = await import('node:child_process');
    system = {
        args: process.argv.slice(2),
        tmp: () => fs.mkdtempSync(path.join(os.tmpdir(), 'text-shortcuts-')),
        write: (file, text) => fs.writeFileSync(file, text),
        read: file => fs.readFileSync(file, 'utf8'),
        exists: file => fs.existsSync(file),
        mode: file => fs.statSync(file).mode & 0o777,
        remove: dir => fs.rmSync(dir, {recursive: true, force: true}),
        run: (argv, env, input = '') => {
            const r = spawnSync(argv[0], argv.slice(1), {env: {...process.env, LANGUAGE: 'C', LC_ALL: 'C.UTF-8', ...env},
                encoding: 'utf8', input});
            return {code: r.status, out: r.stdout, err: r.stderr};
        },
    };
} else {
    const {default: GLib} = await import('gi://GLib');
    const {default: Gio} = await import('gi://Gio');
    const {programArgs} = await import('system');
    system = {
        args: programArgs,
        tmp: () => GLib.dir_make_tmp('text-shortcuts-XXXXXX'),
        write: (file, text) => GLib.file_set_contents(file, new TextEncoder().encode(text)),
        read: file => new TextDecoder().decode(GLib.file_get_contents(file)[1]),
        exists: file => GLib.file_test(file, GLib.FileTest.EXISTS),
        mode: file => Gio.File.new_for_path(file).query_info('unix::mode', Gio.FileQueryInfoFlags.NONE, null)
            .get_attribute_uint32('unix::mode') & 0o777,
        remove: dir => GLib.spawn_command_line_sync(`rm -rf ${GLib.shell_quote(dir)}`),
        run: (argv, env, input = '') => {
            const launcher = new Gio.SubprocessLauncher({
                flags: Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE,
            });
            for (const [k, v] of Object.entries({LANGUAGE: 'C', LC_ALL: 'C.UTF-8', ...env}))
                launcher.setenv(k, v, true);
            const proc = launcher.spawnv(argv);
            const [, out, err] = proc.communicate_utf8(input, null);
            return {code: proc.get_exit_status(), out, err};
        },
    };
}
const HELPER = system.args[0] ?? `${here}../../system_files/shared/usr/libexec/amethystora-text-shortcuts`;
const FIXTURES = `${here}fixtures`;
const helper = (args, env = {}, input = '') => system.run(['python3', '-I', HELPER, ...args], env, input);

// --- The rules, the same on both sides -------------------------------------------------------------------------------
const ABBRS = [
    [';sig', true], [';a', true], ['ab', true], [';', false], ['a', false], ['', false], [';'.repeat(32), true],
    [';'.repeat(33), false], ['é'.repeat(32), true], ['😀😀', true], ['; x', false], [';x\u00a0', false],
    [';x\u3000', false], [';x\t', false], [';x\n', false], [';x\u2028', false], [';x\u0007', false],
    [';x\u001b', false], [';x\u200b', false], [';x\u202e', false], [';👨\u200d👩', true], [';x\u200c', false],
    [';x\ud800', false], [':shrug', true], ['«x', true],
];
const TEXTS = [
    ['a', true], ['', false], ['x'.repeat(2000), true], ['x'.repeat(2001), false], ['é'.repeat(2000), true],
    ['😀'.repeat(2000), true], ['Best wishes,\nSam', true], ['a\tb', true], ['a\rb', false], ['\u001b[31m', false],
    ['a\u0000', false], ['a\u007f', false], ['a\u0085', false], ['\u200fمرحبا', true], ['a\u200cb', true],
    ['a\u202eb', false], ['a\u2066b', false], ['a\ufeffb', false], ['a\u00adb', false], ['a\udc00', false],
    ['  spaced  ', true], ['Date: {date}', true],
];
for (const [abbr, ok] of ABBRS)
    expect(`validAbbr(${JSON.stringify(abbr)})`, validAbbr(abbr), ok);
for (const [text, ok] of TEXTS)
    expect(`textProblem(${JSON.stringify(text).slice(0, 40)})`, textProblem(text) === null, ok);

// The helper's check takes or refuses a file of one shortcut exactly where the shell does
const tmp = system.tmp();
try {
    const one = (abbr, text) => {
        // A lone surrogate is not UTF-8: JSON's escape for it, as a file could hold
        const file = `${tmp}/one.json`;
        system.write(file, JSON.stringify({version: 1, shortcuts: [{abbr, text}]}).replace(/[\ud800-\udfff]/gu,
            c => `\\u${c.charCodeAt(0).toString(16)}`));
        return helper(['check', file]).code === 0;
    };
    for (const [abbr, ok] of ABBRS)
        expect(`the helper takes the shortcut ${JSON.stringify(abbr)}`, one(abbr, 'text'), ok);
    for (const [text, ok] of TEXTS)
        expect(`the helper takes the text ${JSON.stringify(text).slice(0, 40)}`, one(';x', text), ok);

    // The fixtures: the good one whole on both sides, the bad one refused by the helper and its bad entry dropped here
    const good = system.read(`${FIXTURES}/text-shortcuts-good.json`);
    const bad = system.read(`${FIXTURES}/text-shortcuts-bad.json`);
    const goodParsed = parseShortcuts(good);
    expect('the good fixture, here', [goodParsed.map.size, goodParsed.dropped], [6, 0]);
    expect('the good fixture, by the helper', helper(['check', `${FIXTURES}/text-shortcuts-good.json`]).code, 0);
    const badParsed = parseShortcuts(bad);
    expect('the bad fixture, here: one kept, one dropped', [[...badParsed.map.keys()], badParsed.dropped], [[';ok'], 1]);
    const badCheck = helper(['check', `${FIXTURES}/text-shortcuts-bad.json`]);
    expect('the bad fixture, by the helper', [badCheck.code, badCheck.err.trim()],
        [2, 'The text has a character that cannot be typed.']);

    // Files that are not shortcuts files hold none here
    for (const source of [null, '', 'garbage', '[]', '{"shortcuts": 3}', '{"shortcuts": [null, 1, "x"]}',
        `{"shortcuts": [{"abbr": ";x", "text": "${'a'.repeat(LIMITS.fileMax)}"}]}`])
        expect(`parseShortcuts(${String(source).slice(0, 30)})`, parseShortcuts(source).map.size, 0);
    // The first of an abbreviation twice; 200 at most; lengths longest first
    const many = {version: 1, shortcuts: [{abbr: ';a', text: 'one'}, {abbr: ';a', text: 'two'}]};
    for (let i = 0; i < 205; i++)
        many.shortcuts.push({abbr: `;n${i}`, text: 'x'});
    const manyParsed = parseShortcuts(JSON.stringify(many));
    expect('the first of two', manyParsed.map.get(';a'), 'one');
    expect('200 at most', manyParsed.map.size, 200);
    expect('lengths, longest first', parseShortcuts(good).lengths, [6, 5, 4, 3, 2]);
    // Whether it was a shortcuts file at all, after which the shell reads the copy instead
    expect('ok: a shortcuts file', [parseShortcuts(good).ok, parseShortcuts(bad).ok, parseShortcuts('garbage').ok,
        parseShortcuts('{"shortcuts": 3}').ok, parseShortcuts(null).ok], [true, true, false, false, false]);

    // --- The helper's commands, in a home of their own -------------------------------------------------------------
    const env = {XDG_CONFIG_HOME: `${tmp}/config`, HOME: tmp};
    const FILE = `${tmp}/config/amethystora/text-shortcuts.json`;
    // Reading an account without the folder (Control's list, anywhere) makes nothing
    expect('list, with no folder', [helper(['list', '--json'], env).out.trim(), system.exists(`${tmp}/config`)], ['[]', false]);
    expect('add', helper(['add', ';sig', 'Best wishes,\r\nSam'], env).code, 0);
    expect('add another', helper(['add', ';addr', '4 Amethyst Lane, Riverton'], env).code, 0);
    expect('the file is 0600', system.mode(FILE), 0o600);
    expect('the folder is 0700', system.mode(`${tmp}/config/amethystora`), 0o700);
    expect('its copy beside it', system.exists(`${FILE}.bak`), true);
    expect('\\r\\n is kept as \\n', helper(['show', ';sig'], env).out, 'Best wishes,\nSam');
    expect('list: abbreviations only, sorted', helper(['list'], env).out, ';addr\n;sig\n');
    expect('list --json: no text', JSON.parse(helper(['list', '--json'], env).out),
        [{abbr: ';addr', lines: 1}, {abbr: ';sig', lines: 2}]);
    expect('what the shell reads of it', [...parseShortcuts(system.read(FILE)).map.keys()], [';sig', ';addr']);
    expect('add replaces', helper(['add', ';sig', 'Cheers'], env).code, 0);
    expect('...the text', helper(['show', ';sig'], env).out, 'Cheers');
    // The text on stdin, as Control and a pipe give it, kept exactly (its last line break too)
    expect('add, the text on stdin', helper(['add', ';in'], env, 'From stdin,\r\nkept\n').code, 0);
    expect('...as it came, \\r\\n as \\n', helper(['show', ';in'], env).out, 'From stdin,\nkept\n');
    expect('...empty stdin is no text', helper(['add', ';in2'], env, '').err.trim(), 'The text is 1 to 2000 characters.');
    expect('...nor is too much of it', helper(['add', ';in2'], env, 'x'.repeat(16003)).err.trim(), 'The text is 1 to 2000 characters.');
    expect('remove the stdin one', helper(['remove', ';in'], env).code, 0);
    const refusals = [
        [['add', 'a b', 'x'], 'A shortcut is 2 to 32 characters, with no spaces.'],
        [['add', ';x', '\u001b[31m'], 'The text has a character that cannot be typed.'],
        [['add', ';x', ''], 'The text is 1 to 2000 characters.'],
        [['add', ';x', 'x'.repeat(2001)], 'The text is 1 to 2000 characters.'],
        [['remove', ';nope'], 'There is no shortcut ;nope.'],
        [['show', ';nope'], 'There is no shortcut ;nope.'],
        [['remove', '\u001b[2J'], 'A shortcut is 2 to 32 characters, with no spaces.'],
    ];
    for (const [args, message] of refusals) {
        const r = helper(args, env);
        expect(`refused: ${JSON.stringify(args).slice(0, 50)}`, [r.code, r.err.trim()], [2, message]);
    }
    expect('remove', helper(['remove', ';addr'], env).code, 0);
    expect('...gone', helper(['list'], env).out, ';sig\n');
    // A file that cannot be read is read from its copy by list and show, which change nothing (the folder's mode
    // neither), and put back from it by the next change; neither readable: none, the file kept aside once
    system.write(FILE, 'garbage');
    expect('read from the copy', helper(['list'], env).out, ';sig\n');
    expect('...and show too', helper(['show', ';sig'], env).out, 'Cheers');
    expect('...the file left as it is', system.read(FILE), 'garbage');
    expect('the next change puts it back', helper(['add', ';two', '2'], env).code, 0);
    expect('...from the copy', [...parseShortcuts(system.read(FILE)).map.keys()], [';sig', ';two']);
    system.write(FILE, 'garbage');
    system.write(`${FILE}.bak`, 'garbage');
    expect('neither readable: none', helper(['list'], env).out, '');
    expect('...nothing moved by a list', system.exists(`${FILE}.unreadable`), false);
    expect('a change keeps the file aside', helper(['add', ';sig', 'x'], env).code, 0);
    expect('...the file kept aside', system.read(`${FILE}.unreadable`), 'garbage');
    expect('...and the new one made', helper(['list'], env).out, ';sig\n');
    const folder = `${tmp}/config/amethystora`;
    system.run(['chmod', '0755', folder], {});
    helper(['list'], env);
    expect('list leaves the folder\'s mode as it was', system.mode(folder), 0o755);
    helper(['add', ';sig', 'x'], env);
    expect('...a change takes it back to 0700', system.mode(folder), 0o700);
    // 256 KiB at most, said as such: 128 texts of 2000 characters fit, one more does not
    const big = {version: 1, shortcuts: []};
    for (let i = 0; i < 128; i++)
        big.shortcuts.push({abbr: `;b${i}`, text: 'x'.repeat(2000)});
    system.write(FILE, `${JSON.stringify(big, null, 1)}\n`);
    expect('the big file fits', helper(['list'], env).out.split('\n').length, 129);
    const tooBig = helper(['add', ';b-more', 'x'.repeat(2000)], env);
    expect('one more is past 256 KiB', [tooBig.code, tooBig.err.trim()],
        [2, 'Your shortcuts are 256 KiB already: shorten or remove one first.']);
    // 200 at most
    const full = {version: 1, shortcuts: []};
    for (let i = 0; i < 200; i++)
        full.shortcuts.push({abbr: `;n${i}`, text: 'x'});
    system.write(FILE, JSON.stringify(full));
    const r201 = helper(['add', ';one-more', 'x'], env);
    expect('the 201st', [r201.code, r201.err.trim()], [2, 'There are 200 shortcuts already.']);
    expect('...but one there can change', helper(['add', ';n7', 'y'], env).code, 0);
} finally {
    system.remove(tmp);
}

// --- Finding a shortcut in what was typed ----------------------------------------------------------------------------
const {map, lengths} = parseShortcuts(JSON.stringify({version: 1, shortcuts: [
    {abbr: ';sig', text: 'a'}, {abbr: ';sig2', text: 'b'}, {abbr: ';addr', text: 'c'}, {abbr: ';d', text: 'd'},
]}));
for (const [typed, want] of [
    [';addr', ';addr'], ['x;addr', null], [' ;addr', ';addr'], ['(;addr', ';addr'], ['«;addr', ';addr'],
    ['“;addr', ';addr'], ['\';addr', ';addr'], ['\t;addr', ';addr'], [';sig2', ';sig2'], [';sig', ';sig'],
    [';Sig', null], ['a;sig2', null], [';ad', null], [';addrx', null], ['', null], [';d', ';d'], ['a;d', null],
    ['ab;d', null], [`${'z'.repeat(28)} ;addr`, ';addr'],
])
    expect(`find(${JSON.stringify(typed)})`, find(typed, lengths, map), want);
// The window the shell keeps: the longest shortcut and one more character
const window = [...`${'q'.repeat(40)} ;sig2`].slice(-(5 + 1));
expect('find in the kept window', find(window, lengths, map), ';sig2');
expect('find, as an array', find([...';addr'], lengths, map), ';addr');

// What the app says is before its cursor, after the space
for (const [before, want] of [
    ['hello ;addr ', 'yes'], [';addr ', 'yes'], ['hello ;addr', 'not-yet'], ['x;addr ', 'no'], ['hello ;adr ', 'no'],
    ['(;addr ', 'yes'], ['hello ;addr  ', 'no'], ['', 'no'],
])
    expect(`endsWithTyped(${JSON.stringify(before)})`, endsWithTyped(before, ';addr'), want);

// What the buffer keeps
for (const [ch, want] of [['a', true], ['é', true], ['😀', true], [' ', false], ['\t', false], ['\r', false],
    ['\u001b', false], ['', false], ['\u0000', false], [undefined, false], ['ab', false], ['\u200b', false]])
    expect(`isPrintable(${JSON.stringify(ch)})`, isPrintable(ch), want);

// --- {date}: Intl's long style, in the session's locale ---------------------------------------------------------------
const day = new Date(2026, 9, 28, 12);
for (const locale of ['en-US', 'fr-FR', 'ar-EG', 'en-GB']) {
    const today = new Intl.DateTimeFormat(locale, {dateStyle: 'long'}).format(day);
    expect(`{date} in ${locale}`, fillText('Date: {date}, {date}', locale, day), `Date: ${today}, ${today}`);
}
expect('en-US', fillText('{date}', 'en-US', day), 'October 28, 2026');
expect('no placeholder', fillText('{Date} {time}', 'en-US', day), '{Date} {time}');
for (const [name, tag] of [['en_GB.UTF-8', 'en-GB'], ['fr_FR.utf8', 'fr-FR'], ['C', 'en-US'], ['POSIX', 'en-US'],
    ['', 'en-US'], ['sr_RS@latin', 'sr-RS'], [undefined, 'en-US']])
    expect(`localeTag(${name})`, localeTag(name), tag);

if (failures.length) {
    for (const failure of failures)
        say(`FAIL ${failure}`);
    throw new Error(`${failures.length} of ${count} checks of the text shortcuts failed`);
}
say(`ok: ${count} checks of the text shortcuts, the shell's and the helper's`);
