// The emoji and symbols the picker offers, and their names, read from the files the image is built with
// (/usr/share/amethystora/emoji, made by build_files/shared/emoji-data.py from Unicode's emoji list and the Unicode
// CLDR's names): read when the picker opens and let go when it closes. Names and search words are the session
// language's and English's, compared folded (lower case, accents off), so that "euro" and "يورو" both find €.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {fold} from '../fuzzy/matcher.js';

const DIR = '/usr/share/amethystora/emoji';
// The languages names are made in, besides English
const LANGUAGES = ['ar', 'de', 'es', 'fr'];
// The most a search shows
const MOST = 120;

// The tabs in the order they are shown; Recent first, while it has something
export const TABS = ['recent', 'smileys', 'arrows', 'maths', 'money', 'punctuation', 'people', 'nature', 'food',
    'places', 'activities', 'objects', 'symbols', 'flags'];
// The skin tones, as the skin-tone key names them, in the order of each emoji's variants
export const TONES = ['none', 'light', 'medium-light', 'medium', 'medium-dark', 'dark'];

function readJson(path, cancellable) {
    return new Promise(resolve => {
        Gio.File.new_for_path(path).load_contents_async(cancellable, (file, result) => {
            try {
                const [, bytes] = file.load_contents_finish(result);
                const data = JSON.parse(new TextDecoder().decode(bytes));
                resolve(data && typeof data === 'object' ? data : null);
            } catch (e) {
                if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                    console.warn(`Amethystora UI: the emoji picker could not read ${path}: ${e.message}`);
                resolve(null);
            }
        });
    });
}

// The first of the session's languages that has names of its own
function sessionLanguage() {
    for (const name of GLib.get_language_names_with_category('LC_MESSAGES')) {
        const language = name.split(/[_.@]/)[0];
        if (LANGUAGES.includes(language))
            return language;
        if (language === 'en' || language === 'C')
            return null;
    }
    return null;
}

export class EmojiData {
    // What the files hold, or null when the core list cannot be read
    static async load(cancellable) {
        const language = sessionLanguage();
        const [core, english, local] = await Promise.all([
            readJson(`${DIR}/emoji.json`, cancellable),
            readJson(`${DIR}/names-en.json`, cancellable),
            language ? readJson(`${DIR}/names-${language}.json`, cancellable) : Promise.resolve(null),
        ]);
        if (!core?.tabs || typeof core.tabs !== 'object')
            return null;
        return new EmojiData(core, english ?? {}, local ?? {});
    }

    constructor(core, english, local) {
        const list = value => Array.isArray(value) ? value.filter(item => typeof item === 'string') : [];
        this.tabs = new Map(TABS.filter(tab => tab !== 'recent').map(tab => [tab, list(core.tabs[tab])]));
        this.tones = new Map(Object.entries(core.tones ?? {}).filter(([, variants]) =>
            list(variants).length === TONES.length - 1));
        this._english = english;
        this._local = local;
        this._index = null;
    }

    // The name a character is read out by: the session language's, else English's
    nameOf(character) {
        const names = this._local[character] ?? this._english[character];
        return Array.isArray(names) && typeof names[0] === 'string' ? names[0] : '';
    }

    // The character as the chosen skin tone draws it
    toned(character, tone) {
        const at = TONES.indexOf(tone);
        return at > 0 ? this.tones.get(character)?.[at - 1] ?? character : character;
    }

    // Every character once, in Unicode's order (the emoji tabs, then the symbols), with its folded names and words
    _ensureIndex() {
        if (this._index)
            return this._index;
        const seen = new Set();
        this._index = [];
        for (const items of this.tabs.values()) {
            for (const character of items) {
                if (seen.has(character))
                    continue;
                seen.add(character);
                const names = [];
                const words = [];
                for (const table of [this._local, this._english]) {
                    const entry = table[character];
                    if (!Array.isArray(entry))
                        continue;
                    if (typeof entry[0] === 'string')
                        names.push(fold(entry[0]));
                    for (const word of entry.slice(1)) {
                        if (typeof word === 'string')
                            words.push(fold(word));
                    }
                }
                this._index.push({character, names, words});
            }
        }
        return this._index;
    }

    // The characters that answer a search, best first: a name that is the text, a name that starts with it, a name
    // with a word starting with each word typed, then the search words; Unicode's order between equals
    search(text) {
        const query = fold(text).trim();
        if (!query)
            return [];
        const typed = query.split(/\s+/u);
        const wordsOf = list => list.flatMap(item => item.split(/[\s\-:,]+/u)).filter(Boolean);
        const tiers = [[], [], [], []];
        for (const {character, names, words} of this._ensureIndex()) {
            if (names.includes(query))
                tiers[0].push(character);
            else if (names.some(name => name.startsWith(query)))
                tiers[1].push(character);
            else if (typed.every(word => wordsOf(names).some(other => other.startsWith(word))))
                tiers[2].push(character);
            else if (typed.every(word => wordsOf(words).some(other => other.startsWith(word))))
                tiers[3].push(character);
        }
        return tiers.flat().slice(0, MOST);
    }
}
