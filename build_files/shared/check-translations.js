'use strict';

// The translations of the Amethystora apps (apps/, common/src/i18n.rs), for 20-tests.sh and for whoever
// changes a string.
//
//   check-translations.js [root]           check, and exit 1 on anything wrong
//   check-translations.js --write [root]   also rewrite each app's locale/en.json from the code
//   ... --po DIR                           also the shell programs' gettext catalogs in DIR (po/), and
//                                          with --write their template, DIR/amethystora.pot
//   ... --apps DIR                         the apps' Cargo workspace, if not apps/ two folders up from here
//
// root is where the catalogs are, /usr/lib/amethystora-<app>/resources/app/locale under it: / in the
// image (the default), or system_files/shared in the repository. In the image it runs on the Node that
// 20-tests.sh installs for it and check-manual.js, and takes out again.
//
// Every English message is found where the code says it: the first argument of t!, t_parts! and
// t_markup! in the app's crate (apps/<app>/src), which the macros only take as a string in quotes, and
// what an app shows from data of its own (t.dynamic), which is listed in DATA below. locale/en.json has
// to hold exactly those, each as its own translation: it is what Weblate translates from. Every other
// catalog has to parse, and use no value a message does not have. A translation of a message the code no
// longer has is only reported: Weblate takes it out at its next sync.

const fs = require('node:fs');
const path = require('node:path');

const args = process.argv.slice(2);
const write = args.includes('--write');
const poAt = args.indexOf('--po');
const poDir = poAt >= 0 ? args[poAt + 1] : null;
const appsAt = args.indexOf('--apps');
const appsDir = appsAt >= 0 ? args[appsAt + 1] : path.join(__dirname, '..', '..', 'apps');
const root = args.find((arg, at) => !arg.startsWith('--') && (poAt < 0 || at !== poAt + 1) && (appsAt < 0 || at !== appsAt + 1)) || '/';

const APPS = ['amethystora-manual', 'amethystora-security', 'amethystora-update', 'amethystora-logs', 'amethystora-notes', 'amethystora-control',
    'amethystora-backups', 'amethystora-vault', 'amethystora-phone', 'amethystora-shelf', 'amethystora-trim', 'amethystora-help'];

// --- Messages, as common/src/i18n.rs reads them ------------------------------------------------------
//
// ICU MessageFormat: { name }, { name, plural, one {...} other {...} } and { name, select, key {...}
// other {...} }, with # for the number inside a plural. A message as parts: text, HASH, and { name,
// type, options } for a value.

const HASH = { hash: true };

function parse(message) {
    let at = 0;
    const fail = (what) => {
        throw new Error(`${what} at ${at} in "${message}"`);
    };
    function parts(inPlural, nested) {
        const list = [];
        let text = '';
        const flush = () => {
            if (text) {
                list.push(text);
                text = '';
            }
        };
        while (at < message.length) {
            const c = message[at];
            if (c === '}') {
                if (!nested) {
                    fail('a } that closes nothing');
                }
                break;
            }
            if (c === '{') {
                flush();
                list.push(argument(inPlural));
            } else if (c === '#' && inPlural) {
                flush();
                list.push(HASH);
                at += 1;
            } else {
                text += c;
                at += 1;
            }
        }
        flush();
        return list;
    }
    function argument(inPlural) {
        at += 1;
        const head = /^\s*(\w+)\s*(?:,\s*(plural|select)\s*,\s*)?(})?/.exec(message.slice(at));
        if (!head) {
            fail('a value without a name');
        }
        at += head[0].length;
        const [, name, type, closed] = head;
        if (closed) {
            return { name };
        }
        if (!type) {
            fail(`a value ${name} that is not closed`);
        }
        const options = {};
        for (;;) {
            const key = /^\s*(=\d+|\w+)\s*\{/.exec(message.slice(at));
            if (!key) {
                break;
            }
            at += key[0].length;
            options[key[1]] = parts(type === 'plural' || inPlural, true);
            if (message[at] !== '}') {
                fail(`the option ${key[1]} of ${name} is not closed`);
            }
            at += 1;
        }
        const end = /^\s*}/.exec(message.slice(at));
        if (!end || !options.other) {
            fail(`${name} needs an "other" option and a closing }`);
        }
        at += end[0].length;
        return { name, type, options };
    }
    return parts(false, false);
}

// The names of the values a message takes, and of those its plurals and selects choose by
function names(message) {
    const found = new Set();
    const walk = (list) => {
        for (const part of list) {
            if (part && part.name) {
                found.add(part.name);
                Object.values(part.options || {}).forEach(walk);
            }
        }
    };
    walk(parse(message));
    return found;
}

// The words of an app's launcher, in its main entry and its actions, which translate-desktop.py writes
// into it in every language the app's catalogs have them in
function launcher(app) {
    const text = fs.readFileSync(path.join(root, 'usr/share/applications', `${app}.desktop`), 'utf8');
    return [...text.matchAll(/^(?:Name|GenericName|Comment|Keywords)=(.+)$/gm)].map((match) => match[1].trim());
}

// The messages an app reads from data rather than from its code (t.dynamic()), and its launcher's
const DATA = {
    // The names of the manual's sections, in its table of contents, and of the pages a language has not translated
    'amethystora-manual': () => [
        ...JSON.parse(fs.readFileSync(path.join(root, 'usr/share/amethystora/manual/pages.json'), 'utf8')).map((group) => group.section),
        ...untranslatedTitles(),
        ...launcher('amethystora-manual'),
    ],
    // ...and the private folder's, which has no app of its own (translate-desktop.py's OTHERS)
    'amethystora-security': () => [...launcher('amethystora-security'), ...launcher('amethystora-private-folder')],
    'amethystora-update': () => launcher('amethystora-update'),
    'amethystora-logs': () => launcher('amethystora-logs'),
    'amethystora-notes': () => launcher('amethystora-notes'),
    // ...and the launcher of a file nothing opens, which has no app of its own
    'amethystora-control': () => [...launcher('amethystora-control'), ...launcher('zz-amethystora-open-anything')],
    'amethystora-backups': () => launcher('amethystora-backups'),
    'amethystora-vault': () => launcher('amethystora-vault'),
    // ...and the launcher of sending files to it from Files, which has no app of its own
    'amethystora-phone': () => [...launcher('amethystora-phone'), ...launcher('amethystora-phone-send')],
    'amethystora-shelf': () => launcher('amethystora-shelf'),
    'amethystora-trim': () => launcher('amethystora-trim'),
    'amethystora-help': () => launcher('amethystora-help'),
};

// The title of every page of the manual that some language has no manual/<language>/<page>.md of
function untranslatedTitles() {
    const manual = path.join(root, 'usr/share/amethystora/manual');
    const languages = fs.readdirSync(manual).filter((name) => /^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$/.test(name) && fs.statSync(path.join(manual, name)).isDirectory());
    const files = JSON.parse(fs.readFileSync(path.join(manual, 'pages.json'), 'utf8')).flatMap((group) => group.pages);
    return files.filter((file) => languages.some((language) => !fs.existsSync(path.join(manual, language, path.basename(file)))))
        .map((file) => /^# (.+)$/m.exec(fs.readFileSync(path.join(manual, file), 'utf8'))[1].trim());
}

const problems = [];
const notes = [];

// A Rust string literal, as the text it stands for
function unquoteRust(literal) {
    return literal.slice(1, -1).replace(/\\(\n\s*|u\{[0-9a-fA-F]+\}|x[0-9a-fA-F]{2}|.)/g, (_match, escape) => {
        if (escape[0] === '\n') {
            return '';
        }
        if (escape[0] === 'u' || escape[0] === 'x') {
            return String.fromCodePoint(parseInt(escape.replace(/^[ux]\{?|\}$/g, ''), 16));
        }
        return { n: '\n', t: '\t', r: '\r', 0: '\0' }[escape] ?? escape;
    });
}

// Every .rs file under a folder
function rustFiles(dir) {
    return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
        const file = path.join(dir, entry.name);
        return entry.isDirectory() ? rustFiles(file) : entry.name.endsWith('.rs') ? [file] : [];
    });
}

// The messages of an app (apps/<name>/src): the first argument of t!, t_parts! and t_markup!, which
// the macros only take as a string literal
function messages(dir) {
    const found = new Map();
    for (const file of rustFiles(dir)) {
        const text = fs.readFileSync(file, 'utf8');
        for (const match of text.matchAll(/(?<![\w$.])t(?:_parts|_markup)?!\(\s*(?=\S)/g)) {
            const rest = text.slice(match.index + match[0].length);
            const literal = /^"(?:[^"\\]|\\[\s\S])*"/.exec(rest);
            const line = text.slice(0, match.index).split('\n').length;
            if (!literal) {
                problems.push(`${file}:${line}: t!() needs the English as a string in quotes`);
                continue;
            }
            const message = unquoteRust(literal[0]);
            if (!found.has(message)) {
                found.set(message, `${path.relative(dir, file)}:${line}`);
            }
        }
    }
    return found;
}

let checked = 0;
for (const app of APPS) {
    const crate = path.join(appsDir, app.replace(/^amethystora-/, ''), 'src');
    if (!fs.existsSync(crate)) {
        problems.push(`${crate}: not there`);
        continue;
    }
    const found = messages(crate);
    for (const message of DATA[app]?.() || []) {
        if (!found.has(message)) {
            found.set(message, 'data');
        }
    }
    for (const [message, where] of found) {
        try {
            parse(message);
        } catch (error) {
            problems.push(`${app} ${where}: ${error.message}`);
        }
    }
    const folder = path.join(root, 'usr/lib', app, 'resources/app/locale');
    const baseFile = path.join(folder, 'en.json');
    const sorted = [...found.keys()].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
    const base = Object.fromEntries(sorted.map((message) => [message, message]));
    if (write) {
        fs.mkdirSync(folder, { recursive: true });
        fs.writeFileSync(baseFile, `${JSON.stringify(base, null, 2)}\n`);
    }
    let current = null;
    try {
        current = JSON.parse(fs.readFileSync(baseFile, 'utf8'));
    } catch {
        problems.push(`${app}: locale/en.json is missing or is not JSON`);
    }
    if (current && JSON.stringify(current) !== JSON.stringify(base)) {
        const missing = sorted.filter((message) => !Object.hasOwn(current, message));
        const extra = Object.keys(current).filter((message) => !found.has(message));
        problems.push(`${app}: locale/en.json is not what the code says (${missing.length} missing, ${extra.length} no longer used, or out of order): run node build_files/shared/check-translations.js --write system_files/shared`);
    }
    for (const file of fs.existsSync(folder) ? fs.readdirSync(folder) : []) {
        if (!file.endsWith('.json') || file === 'en.json') {
            continue;
        }
        if (!/^[A-Za-z]{2,3}(-[A-Za-z0-9]{2,8})*\.json$/.test(file)) {
            problems.push(`${app}: locale/${file} is not named after a language`);
            continue;
        }
        let catalog;
        try {
            catalog = JSON.parse(fs.readFileSync(path.join(folder, file), 'utf8'));
        } catch (error) {
            problems.push(`${app}: locale/${file} is not JSON: ${error.message}`);
            continue;
        }
        let stale = 0;
        for (const [message, translation] of Object.entries(catalog)) {
            if (!found.has(message)) {
                stale += 1;
                continue;
            }
            if (typeof translation !== 'string') {
                problems.push(`${app}: locale/${file}: the translation of "${message}" is not text`);
                continue;
            }
            if (!translation) {
                continue;
            }
            try {
                const own = names(message);
                for (const name of names(translation)) {
                    if (!own.has(name)) {
                        problems.push(`${app}: locale/${file}: "${translation}" uses {${name}}, which "${message}" does not have`);
                    }
                }
            } catch (error) {
                problems.push(`${app}: locale/${file}: ${error.message}`);
            }
        }
        if (stale) {
            notes.push(`${app}: locale/${file} has ${stale} translation(s) of messages the code no longer has`);
        }
        const untranslated = sorted.filter((message) => !catalog[message]).length;
        if (untranslated) {
            notes.push(`${app}: locale/${file} leaves ${untranslated} message(s) in English`);
        }
        checked += 1;
    }
}

// --- The shell programs ------------------------------------------------------------------------------
//
// What the security report and the app permissions say is translated with gettext, in the text domain
// amethystora: gettext "...", eval_gettext "..." and eval_ngettext "..." "..." N, with their values as
// shell variables (${NAME}), which marks the message sh-format. N_ "..." and N_n SUMMARY PLURAL "..." "..." only
// mark a message for a program that is handed it unexpanded, amethystora-notify-users --msgid, which translates
// it for each account. A comment that starts "# Translators:" goes to the translators with every message up to
// the next empty line.

const PYTHON = ['usr/libexec/amethystora-text-shortcuts',
    // Files' What is using this? and Convert (plans/ui-ideas/spec-tools.md)
    'usr/libexec/amethystora-what-uses', 'usr/libexec/amethystora-convert', 'usr/libexec/amethystora-mirror',
    // A missing command, found (plans/ui-ideas/spec-time.md)
    'usr/libexec/amethystora-missing-command',
    // Downloads tidied (plans/ui-ideas/spec-apps.md)
    'usr/libexec/amethystora-tidy-downloads',
    // The Help window's helpers (plans/ui-ideas/r7/spec-help.md), and how it finds what Show me points at
    'usr/libexec/amethystora-wrong', 'usr/libexec/amethystora-changes',
    // Bring everything from your old computer (R7-48)
    'usr/libexec/amethystora-bring-over'];
const SHELL = ['usr/libexec/amethystora-security-status', 'usr/libexec/amethystora-app-permissions', 'usr/libexec/amethystora-layout-offer',
    'usr/libexec/amethystora-control-list', 'usr/libexec/amethystora-security-config', 'usr/libexec/amethystora-hardening',
    'usr/libexec/amethystora-phone-link-notice', 'usr/libexec/amethystora-deja-dup-notice', 'usr/libexec/amethystora-widgets-moved-notice',
    // The notice that the computer went back by itself from an update that did not start (R7-52)
    'usr/libexec/amethystora-update-alert',
    'usr/libexec/amethystora-agentic-off-notice', 'usr/libexec/amethystora-private-folder', 'usr/libexec/amethystora-windows-program',
    // Files' scripts menu: its entries' names, and what the three scripts say
    'usr/libexec/amethystora-folder-colour', 'usr/libexec/amethystora-back-in-time', 'usr/share/amethystora/user-setup.hooks.d/24-files-menu.sh',
    'usr/libexec/amethystora-clean-copy', 'usr/libexec/amethystora-start-times', 'usr/libexec/amethystora-pdf-sign', 'usr/libexec/amethystora-share-screen',
    // Send to anyone, which says where a file passes before it fetches Warp (R7-66)
    'usr/libexec/amethystora-send-anyone',
    // Restart while you are away: its notice of a night that did not restart
    'usr/libexec/amethystora-night-restart',
    // Files' New Document menu: the templates' names
    'usr/share/amethystora/user-setup.hooks.d/28-templates.sh',
    // The top bar's items, which Control and ame desktop top-bar name
    'usr/libexec/amethystora-bar-layout',
    // The callers of amethystora-notify-users --msgid, which mark their messages with N_
    'usr/libexec/amethystora-scan-alert', 'usr/libexec/amethystora-clamav-onaccess', 'usr/libexec/amethystora-lynis-audit',
    'usr/libexec/amethystora-driver-status', 'usr/libexec/amethystora-gpu-check', 'usr/libexec/amethystora-raid',
    'usr/libexec/amethystora-disk-health', 'usr/libexec/amethystora-update-check',
    'usr/libexec/amethystora-security-watch', 'usr/libexec/amethystora-sends',
    // The plugin system's command (plans/PLAN-plugins.md); its group's recipes are in the list of groups below
    'usr/libexec/amethystora-plugins',
    // What `ame pill` runs: a script live in the pill (R7-03)
    'usr/libexec/amethystora-pill',
    // The text shortcuts' helper, in Python, whose _("...") is read as an extension's is
    ...PYTHON,
    'usr/share/gnome-shell/extensions/amethystora-logo-menu@arsslensoft.github.io/extension.js',
    // Amethystora Clipboard's text actions, Amethystora's own file in it, in domain amethystora
    'usr/share/gnome-shell/extensions/amethystora-clipboard@arsslensoft.github.io/textActions.js',
    ...['agent', 'apps', 'backup', 'changelog', 'desktop', 'pill', 'plugin', 'report', 'security', 'system', 'update', 'vault'].map((group) => `usr/share/amethystora/just/${group}.just`),
    // Amethystora UI: every module, however deep it is split
    ...jsFiles('usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io')];

// The .js files under a folder of the root, sorted, so that the template keeps one order
function jsFiles(folder) {
    const out = [];
    const walk = (relative) => {
        const absolute = path.join(root, relative);
        if (!fs.existsSync(absolute)) {
            return;
        }
        for (const entry of fs.readdirSync(absolute, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
            const child = `${relative}/${entry.name}`;
            if (entry.isDirectory()) {
                walk(child);
            } else if (entry.name.endsWith('.js')) {
                out.push(child);
            }
        }
    };
    walk(folder);
    return out;
}

// A shell word in quotes, as the program it is passed to receives it
function unshell(word) {
    return word[0] === "'" ? word.slice(1, -1) : word.slice(1, -1).replace(/\\(["\\$`])/g, '$1');
}

const QUOTED = String.raw`("(?:[^"\\]|\\.)*"|'[^']*')`;
const CALL = new RegExp(String.raw`(?<![\w-])(eval_ngettext|eval_gettext|gettext|N_n|N_)(?:(?<=N_n)\s+\w+\s+\w+)?\s+${QUOTED}(?:\s+${QUOTED})?`, 'g');

// A GNOME Shell extension's own: _('...') and ngettext('...', '...', n), the gettext of
// resource:///org/gnome/shell/extensions/extension.js. A comment that starts "// Translators:" goes with them as "#" does.
// JavaScript's quotes take backslash escapes in either kind.
const JS_QUOTED = String.raw`("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')`;
const JS_CALL = new RegExp(String.raw`(?<![\w$.])(_|ngettext)\(\s*${JS_QUOTED}(?:\s*,\s*${JS_QUOTED})?`, 'g');
// and C_('context', '...'), pgettext, for a word that means one thing in one place (a key's name on a key cap)
const JS_CONTEXT_CALL = new RegExp(String.raw`(?<![\w$.])C_\(\s*${JS_QUOTED}\s*,\s*${JS_QUOTED}`, 'g');
const JS_ESCAPES = { n: '\n', t: '\t', '\\': '\\', "'": "'", '"': '"' };
const unquoteJs = (word) => word.slice(1, -1).replace(/\\(.)/g, (all, c) => JS_ESCAPES[c] ?? c);

function shellMessages() {
    const found = new Map();
    for (const script of SHELL) {
        const lines = fs.readFileSync(path.join(root, script), 'utf8').split('\n');
        let comment = '';
        lines.forEach((line) => {
            const note = (script.endsWith('.js') ? /^\s*\/\/\s*Translators:\s*(.*)$/ : /^\s*#\s*Translators:\s*(.*)$/).exec(line);
            if (note) {
                comment = note[1];
                return;
            }
            if (!line.trim()) {
                comment = '';
            }
            const quoted = script.endsWith('.js') || PYTHON.includes(script);
            for (const match of line.matchAll(quoted ? JS_CALL : CALL)) {
                const [, call, first, second] = match;
                const unquote = quoted ? unquoteJs : unshell;
                const id = unquote(first);
                const plural = ['eval_ngettext', 'N_n', 'ngettext'].includes(call) && second ? unquote(second) : null;
                const key = `${id}\u0000${plural || ''}`;
                const entry = found.get(key) || { id, plural, files: new Set(), comment: '' };
                entry.files.add(script);
                entry.comment ||= comment;
                found.set(key, entry);
            }
            for (const match of script.endsWith('.js') ? line.matchAll(JS_CONTEXT_CALL) : []) {
                const context = unquoteJs(match[1]);
                const id = unquoteJs(match[2]);
                const key = `${context}\u0004${id}\u0000`;
                const entry = found.get(key) || { context, id, plural: null, files: new Set(), comment: '' };
                entry.files.add(script);
                entry.comment ||= comment;
                found.set(key, entry);
            }
        });
    }
    // The phone link's program (apps/connect) and Move to this computer (apps/move) are not shell programs but speak through the
    // same catalogs: tr("...") and trf("...", &[("name", value)]) of src/text.rs, whose {name} placeholders msgfmt checks as
    // python-brace-format
    for (const crate of ['connect', 'move']) {
        const connect = path.join(appsDir, crate, 'src');
        for (const file of fs.existsSync(connect) ? rustFiles(connect).sort() : []) {
            const script = `apps/${crate}/src/${path.relative(connect, file).split(path.sep).join('/')}`;
            const text = fs.readFileSync(file, 'utf8');
            for (const match of text.matchAll(/(?<![\w$.])(?:text::)?trf?\(\s*("(?:[^"\\]|\\[\s\S])*")/g)) {
                const id = unquoteRust(match[1]);
                const key = `${id}\u0000`;
                const entry = found.get(key) || { id, plural: null, files: new Set(), comment: '' };
                entry.files.add(script);
                found.set(key, entry);
            }
        }
    }
    return [...found.values()];
}

const VARIABLE = /\$\{?([A-Za-z_][A-Za-z0-9_]*)\}?/g;
const variables = (text) => new Set([...text.matchAll(VARIABLE)].map((match) => match[1]));
const poString = (text) => `"${text.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, '\\n')}"`;

function template(entries) {
    const out = [
        '# What Amethystora\'s own shell programs say, the security report and the app permissions, for',
        '# translation. Written by build_files/shared/check-translations.js --write; translated on Weblate.',
        'msgid ""',
        'msgstr ""',
        '"Project-Id-Version: amethystora\\n"',
        '"Report-Msgid-Bugs-To: https://github.com/arsslensoft/amethystora/issues\\n"',
        '"MIME-Version: 1.0\\n"',
        '"Content-Type: text/plain; charset=UTF-8\\n"',
        '"Content-Transfer-Encoding: 8bit\\n"',
        '"Plural-Forms: nplurals=INTEGER; plural=EXPRESSION;\\n"',
        '',
    ];
    for (const entry of entries) {
        if (entry.comment) {
            out.push(`#. Translators: ${entry.comment}`);
        }
        out.push(`#: ${[...entry.files].join(' ')}`);
        if (variables(entry.id).size || (entry.plural && variables(entry.plural).size)) {
            out.push('#, sh-format');
        } else if (/\{[A-Za-z_][A-Za-z0-9_]*\}/.test(entry.id)) {
            out.push('#, python-brace-format');
        }
        if (entry.context) {
            out.push(`msgctxt ${poString(entry.context)}`);
        }
        out.push(`msgid ${poString(entry.id)}`);
        if (entry.plural) {
            out.push(`msgid_plural ${poString(entry.plural)}`, 'msgstr[0] ""', 'msgstr[1] ""');
        } else {
            out.push('msgstr ""');
        }
        out.push('');
    }
    return out.join('\n');
}

// The entries of a .po file: msgid, msgid_plural and every msgstr, with its flags
function readPo(text) {
    const entries = [];
    let entry = null;
    let field = null;
    const finish = () => {
        if (entry?.msgid !== undefined) {
            entries.push(entry);
        }
        entry = { flags: '', msgstr: [] };
    };
    finish();
    for (const raw of text.split('\n')) {
        const line = raw.trim();
        if (!line) {
            finish();
            field = null;
            continue;
        }
        if (line.startsWith('#,')) {
            entry.flags += line;
            continue;
        }
        if (line.startsWith('#')) {
            continue;
        }
        const keyword = /^(msgid_plural|msgid|msgctxt|msgstr(?:\[(\d+)\])?)\s+(".*")$/.exec(line);
        const value = (quoted) => JSON.parse(quoted.replace(/\\(?!["\\nt])/g, '\\\\'));
        if (keyword) {
            field = keyword[2] !== undefined ? ['msgstr', Number(keyword[2])] : keyword[1] === 'msgstr' ? ['msgstr', 0] : [keyword[1]];
            if (field[0] === 'msgstr') {
                entry.msgstr[field[1]] = value(keyword[3]);
            } else {
                entry[field[0]] = value(keyword[3]);
            }
        } else if (/^".*"$/.test(line) && field) {
            if (field[0] === 'msgstr') {
                entry.msgstr[field[1]] += value(line);
            } else {
                entry[field[0]] += value(line);
            }
        }
    }
    finish();
    return entries;
}

// Which plural forms of a catalog stand for many numbers, from its Plural-Forms header. msgfmt --check,
// which compiles the catalogs in the build, holds such a form, and every message without plurals, to
// exactly the variables of its English (of msgid_plural, where there is one); a form that fewer than five
// of the numbers 0 to 1000 take, such as the one for n == 1, may leave the number out. The expression is
// C, which JavaScript reads the same once nothing but n, digits and operators is in it.
function pluralForms(entries) {
    const header = entries.find((entry) => entry.msgid === '')?.msgstr[0] || '';
    const match = /Plural-Forms:\s*nplurals\s*=\s*(\d+)\s*;\s*plural\s*=\s*([^;\n]+);?/.exec(header);
    if (!match) {
        return null;
    }
    const nplurals = Number(match[1]);
    const expression = match[2].trim();
    if (!/^[\sn0-9?:()<>=!&|%+\-*/]+$/.test(expression)) {
        return { nplurals, often: null, problem: `a Plural-Forms expression with more than n, digits and operators: ${expression}` };
    }
    // eslint-disable-next-line no-new-func
    const plural = new Function('n', `return +(${expression});`);
    const counts = new Array(nplurals).fill(0);
    for (let n = 0; n <= 1000; n += 1) {
        const index = plural(n);
        if (index >= 0 && index < nplurals) {
            counts[index] += 1;
        }
    }
    return { nplurals, often: counts.map((count) => count >= 5) };
}

let shellCatalogs = 0;
if (poDir) {
    const entries = shellMessages();
    const potFile = path.join(poDir, 'amethystora.pot');
    const pot = template(entries);
    if (write) {
        fs.writeFileSync(potFile, pot);
    }
    if (!fs.existsSync(potFile) || fs.readFileSync(potFile, 'utf8') !== pot) {
        problems.push(`${potFile} is not what the shell programs say: run node build_files/shared/check-translations.js --write --po po system_files/shared`);
    }
    const known = new Map(entries.map((entry) => [`${entry.context || ''}\u0004${entry.id}`, entry]));
    for (const file of fs.existsSync(poDir) ? fs.readdirSync(poDir).filter((name) => name.endsWith('.po')) : []) {
        let stale = 0;
        const catalog = readPo(fs.readFileSync(path.join(poDir, file), 'utf8'));
        const forms = pluralForms(catalog);
        if (!forms) {
            problems.push(`po/${file}: no Plural-Forms header`);
        } else if (forms.problem) {
            problems.push(`po/${file}: ${forms.problem}`);
        }
        for (const entry of catalog) {
            if (entry.msgid === '' || entry.flags.includes('fuzzy')) {
                continue;
            }
            const source = known.get(`${entry.msgctxt || ''}\u0004${entry.msgid}`);
            if (!source) {
                stale += 1;
                continue;
            }
            const english = source.plural || source.id;
            const allowed = variables(english);
            entry.msgstr.forEach((translation, index) => {
                if (!translation) {
                    return;
                }
                const used = variables(translation);
                for (const name of used) {
                    if (!allowed.has(name)) {
                        problems.push(`po/${file}: "${translation}" uses \${${name}}, which "${english}" does not have`);
                    }
                }
                // What msgfmt --check holds it to: every variable, unless it is a plural form for few numbers
                const strict = !source.plural || forms?.often?.[index] !== false;
                if (strict) {
                    for (const name of allowed) {
                        if (!used.has(name)) {
                            problems.push(`po/${file}: "${translation}" leaves out \${${name}}, which msgfmt --check requires`);
                        }
                    }
                }
                if (source.plural && forms && index >= forms.nplurals) {
                    problems.push(`po/${file}: "${source.id}" has msgstr[${index}], but the language has ${forms.nplurals} forms`);
                }
            });
            if (source.plural && forms && entry.msgstr.length !== forms.nplurals) {
                problems.push(`po/${file}: "${source.id}" has ${entry.msgstr.length} forms where the language has ${forms.nplurals}`);
            }
        }
        if (stale) {
            notes.push(`po/${file} has ${stale} translation(s) of messages the shell programs no longer say`);
        }
        shellCatalogs += 1;
    }
}

for (const note of notes) {
    console.log(`translations: ${note}`);
}
for (const problem of problems) {
    console.error(`::error::translations: ${problem}`);
}
console.log(`translations: ${APPS.length} apps, ${checked} catalogs checked${poDir ? `, and ${shellCatalogs} of the shell programs` : ''}`);
process.exit(problems.length ? 1 : 0);
