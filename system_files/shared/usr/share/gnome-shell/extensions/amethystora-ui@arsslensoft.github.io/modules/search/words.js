// It speaks your old words (plans/ui-ideas/r7/spec-find.md, R7-49): the everyday words people bring with them, each
// leading to the thing that does that here. Nothing else is known here: this file has no GNOME in it, so that a test can
// run it under node.
//
// A word list is one message, its alternatives separated by |, in lower case: a translator gives the words people say
// in that language. The English ones work in every language. Everyday words only: never the name of another maker's
// product or feature, bar the four old names of things every system has (task manager, control panel, recycle bin,
// print screen); test-search-words.mjs, which reads the catalogs too, and 20-tests.sh hold that.

// wordsTable(_) names what each entry opens: the first of `apps` this machine has, or a place (`uri`), a part of the shell
// (`shell`), the manual (`manual`), or a command (`argv`, with `terminal` when it needs one). Each entry's words come last,
// under their own note to translators: that note goes with the message below it up to the next empty line.
export function wordsTable(_) {
    return [
        {
            id: 'running', apps: ['io.missioncenter.MissionCenter.desktop', 'org.gnome.SystemMonitor.desktop'],
            note: _('What is running, and what it uses'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('task manager|process list|running programs|what is running|stop a program|end a program'),
        },

        {
            id: 'settings', apps: ['org.gnome.Settings.desktop'],
            note: _('And Control for the rest'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('control panel|computer settings'),
        },

        {
            id: 'trash', uri: 'trash:///', icon: 'user-trash-symbolic', title: _('Trash'),
            note: _('In Files'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('recycle bin|rubbish bin|waste basket|wastebasket|garbage can|deleted files'),
        },

        {
            id: 'files', apps: ['org.gnome.Nautilus.desktop'],
            note: _('Your folders and files'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('file manager|file browser|my files'),
        },

        {
            id: 'software', apps: ['io.github.kolunmi.Bazaar.desktop'],
            note: _('Get apps, and remove them'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('software store|install a program|install programs|uninstall a program|uninstall programs|remove a program|remove programs'),
        },

        {
            id: 'updates', apps: ['amethystora-update.desktop'],
            note: _('Keeps the system up to date'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('check for updates|system update|install updates'),
        },

        {
            id: 'space', apps: ['org.gnome.baobab.desktop'],
            note: _('What takes up the space'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('disk space|free up space|what takes up space'),
        },

        {
            id: 'disks', apps: ['org.gnome.DiskUtility.desktop'],
            note: _('Format and look after your disks'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('format a disk|format a drive|partition a disk|partitions'),
        },

        {
            id: 'terminal', apps: ['kitty.desktop', 'org.gnome.Ptyxis.desktop'],
            note: _('Type commands'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('command line|command window|console'),
        },

        {
            id: 'notes', apps: ['amethystora-notes.desktop'],
            note: _('Notes and tasks, on this computer'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('to do list|to-do list|todo list'),
        },

        {
            id: 'screenshot', shell: 'screenshot', icon: 'camera-photo-symbolic', title: _('Take a screenshot'),
            note: _('The Print key does this too'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('print screen|screen capture|screen grab'),
        },

        {
            id: 'startup', argv: ['amethystora-control', 'apps', 'startup'], icon: 'amethystora-control',
            title: _('What starts with you'),
            note: _('In Control'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('startup programs|startup apps|programs at startup|apps at startup'),
        },

        {
            id: 'drivers', argv: ['ame', 'system', 'drivers'], terminal: true, icon: 'utilities-terminal-symbolic',
            title: _('Drivers'),
            note: _('What each device runs on, in a terminal'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('drivers|hardware drivers'),
        },

        {
            id: 'backups', apps: ['amethystora-backups.desktop'],
            note: _('Your home folder at any hour'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('restore files|restore my files|recover files|back up my files'),
        },

        {
            id: 'rollback', manual: 'rolling-back', icon: 'amethystora-docs', title: _('Rolling back'),
            note: _('In the manual'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('undo an update'),
        },

        {
            id: 'protection', apps: ['amethystora-security.desktop'],
            note: _('What protects this computer'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('antivirus|anti-virus|virus scan|virus scanner|malware|firewall'),
        },

        {
            id: 'pdf', apps: ['org.gnome.Papers.desktop'],
            note: _('Reads PDFs'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('pdf reader|pdf viewer|read a pdf'),
        },

        {
            id: 'photos', apps: ['org.gnome.Loupe.desktop'],
            note: _('Shows pictures'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('photo viewer|picture viewer'),
        },

        {
            id: 'videos', apps: ['org.gnome.Showtime.desktop'],
            note: _('Plays videos'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('video player|movie player|media player'),
        },

        {
            id: 'sound', apps: ['org.gnome.Decibels.desktop'],
            note: _('Plays sound files'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('music player|audio player'),
        },

        {
            id: 'draw', apps: ['com.github.PintaProject.Pinta.desktop'],
            note: _('Draw and edit pictures'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('drawing program|draw a picture|image editor'),
        },

        {
            id: 'archives', apps: ['org.gnome.FileRoller.desktop'],
            note: _('Opens and makes archives'),
            // Translators: words separated by | are alternatives: in lower case, the everyday words people say in your language, never a product's name.
            words: _('unzip|zip files|compress files|extract files'),
        },
    ];
}

// A language's name as the session has it (de_DE.UTF-8, sr@latin) as a tag the language functions take; English when
// it cannot be one, since they throw on anything else
export function localeTag(name) {
    const tag = (name ?? 'C').split(/[.@]/)[0].replaceAll('_', '-');
    if (tag === 'C' || tag === 'POSIX')
        return 'en';
    try {
        return Intl.getCanonicalLocales(tag)[0] ?? 'en';
    } catch {
        return 'en';
    }
}

// The form two words are compared in: no case, no accents
export function fold(text, locale = undefined) {
    return text.toLocaleLowerCase(locale).normalize('NFD').replace(/\p{M}/gu, '');
}

// Each entry with its phrases, English and translated, as lists of words
export function compileWords(english, translated, locale = undefined) {
    return english.map((entry, index) => {
        const lists = [entry.words, translated[index]?.words ?? ''];
        const phrases = new Set(lists.flatMap(list => list.split('|')).map(phrase => fold(phrase.trim(), locale)).filter(Boolean));
        return {...translated[index], phrases: [...phrases].map(phrase => phrase.split(/\s+/u))};
    });
}

// Whether what was typed is this phrase. A one-word search has to be the whole phrase, so that "task" on its way to
// something else takes nothing from the apps; from the second word on, the last one may still be being typed
function matches(typed, phrase) {
    if (typed.length > phrase.length || (typed.length === 1 && phrase.length > 1))
        return false;
    const last = typed.length - 1;
    if (!typed.slice(0, last).every((word, index) => word === phrase[index]))
        return false;
    return typed.length === 1 ? typed[0] === phrase[0] : phrase[last].startsWith(typed[last]);
}

// The entries that what was typed names, in the table's order
export function matchWords(compiled, terms, locale = undefined) {
    const typed = terms.map(term => fold(term, locale)).filter(Boolean);
    if (typed.length === 0 || typed.join(' ').startsWith('clip:'))
        return [];
    return compiled.filter(entry => entry.phrases.some(phrase => matches(typed, phrase)));
}

// Whether an installed app's name starts with what was typed: that app is what the person is typing, and keeps Enter.
// `apps` are [name, id] pairs; an id in `except` is the entry's own, which does not count
export function appNamedLike(apps, terms, locale = undefined, except = []) {
    const typed = terms.map(term => fold(term, locale)).filter(Boolean).join(' ');
    return typed !== '' && apps.some(([name, id]) => !except.includes(id) && fold(name, locale).startsWith(typed));
}
