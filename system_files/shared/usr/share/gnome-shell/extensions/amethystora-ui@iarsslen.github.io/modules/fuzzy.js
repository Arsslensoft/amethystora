// App search that forgives: in the overview and in the search box (Super+Space), an app is found from a typo, two
// letters swapped, letters left out, a piece from the middle of its name or its initials, and GNOME's power actions
// (Power Off, Restart, Log Out...) are found the same way. What GNOME's own search finds by the start of a word keeps
// the place GNOME gives it, so that "fire" still opens Firefox; the forgiving matches follow, strongest first, and
// GNOME's usage order settles equals. An app Parental Controls keep from this account is never shown, as in GNOME's
// own search. The other search providers (Files, Settings, Calculator...) are left as they are.
//
// The apps are read into memory when the module starts and again whenever an app is installed, removed or changed;
// nothing is written to disk. The matching itself is in fuzzy/matcher.js.

import GLib from 'gi://GLib';
import Shell from 'gi://Shell';

import * as AppDisplay from 'resource:///org/gnome/shell/ui/appDisplay.js';
import * as ParentalControlsManager from 'resource:///org/gnome/shell/misc/parentalControlsManager.js';
import * as SystemActions from 'resource:///org/gnome/shell/misc/systemActions.js';

import {makeEntry, rank, words} from './fuzzy/matcher.js';
import {renamed} from './appgrid/looks.js';

// GNOME shows six app results; a longer list leaves room for it, and no more is ever needed
const LONGEST = 30;

// The power action whose name or keyword is what was typed, whole ("power off", "restart"), if it can be used now.
// That is somebody asking for that action, so it goes first, and no setting that only shares the words comes before it
export function namedAction(terms) {
    const typed = terms.flatMap(words).join(' ');
    const actions = SystemActions.getDefault()._actions;
    if (!typed || !(actions instanceof Map))
        return null;
    for (const [id, {name, keywords, available}] of actions) {
        if (available && [name, ...keywords ?? []].some(said => words(said).join(' ') === typed))
            return id;
    }
    return null;
}

export class Fuzzy {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._read();
        Shell.AppSystem.get_default().connectObject('installed-changed', () => this._read(), this);

        // GNOME's own answer is asked first and kept as the base, so that whatever goes wrong here, GNOME's
        // results are what the search shows
        const provider = AppDisplay.AppSearchProvider.prototype;
        const original = provider.getInitialResultSet;
        this._original = original;
        const fuzzy = this;
        provider.getInitialResultSet = async function (terms, cancellable) {
            const found = await original.call(this, terms, cancellable);
            if (!fuzzy._apps)
                return found;
            try {
                return fuzzy._rank(terms, found);
            } catch (e) {
                console.error(`Amethystora UI: the forgiving app search failed, GNOME's own results are shown: ${e.message}`);
                return found;
            }
        };
    }

    disable() {
        if (this._original)
            AppDisplay.AppSearchProvider.prototype.getInitialResultSet = this._original;
        this._original = null;
        Shell.AppSystem.get_default().disconnectObject(this);
        this._apps = null;
        this._parts = null;
    }

    // Every launcher GNOME's search could show, in the session's language
    _read() {
        const apps = new Map();
        const parts = new Map();
        for (const info of Shell.AppSystem.get_default().get_installed()) {
            const id = info.get_id();
            if (!id || !info.should_show())
                continue;
            const executable = info.get_executable();
            const part = {
                names: [info.get_name(), info.get_display_name()],
                keywords: info.get_keywords?.() ?? [],
                command: executable ? GLib.path_get_basename(executable) : '',
                description: info.get_description() ?? '',
                other: [info.get_generic_name?.() ?? '', id.replace(/\.desktop$/, '')],
            };
            parts.set(id, part);
            apps.set(id, makeEntry(id, part));
        }
        this._apps = apps;
        this._parts = parts;
    }

    // An app the account gave a name of its own (the app grid's Rename) is found by that name first, and by its own
    _entry(id, entry) {
        const name = renamed.get(id);
        const part = name ? this._parts?.get(id) : null;
        return part ? makeEntry(id, {...part, names: [name, ...part.names]}) : entry;
    }

    // GNOME keeps each power action's name, keywords (already folded) and whether it can be used now together;
    // they are read as they are at each search, and an action that cannot be used is never offered
    _actions() {
        const entries = [];
        const actions = SystemActions.getDefault()._actions;
        if (!(actions instanceof Map))
            return entries;
        for (const [id, {name, keywords, available}] of actions) {
            if (available && name)
                entries.push(makeEntry(id, {names: [name], keywords: keywords ?? []}));
        }
        return entries;
    }

    _rank(terms, found) {
        const parental = ParentalControlsManager.getDefault();
        const appSystem = Shell.AppSystem.get_default();
        const entries = new Map();
        for (const [id, entry] of this._apps) {
            const app = appSystem.lookup_app(id);
            if (app && parental.shouldShowApp(app.app_info))
                entries.set(id, this._entry(id, entry));
        }
        for (const entry of this._actions())
            entries.set(entry.id, entry);

        // GNOME's own search again, as GNOME asks it, for how well it matched each app: its answer above is already
        // in that order, and this only tells where one of its groups ends
        const groups = new Map();
        Shell.AppSystem.search(terms.join(' ')).forEach((ids, index) => {
            for (const id of ids)
                groups.set(id, index);
        });
        const usage = Shell.AppUsage.get_default();
        const ranked = rank(entries, terms, found, (a, b) => usage.compare(a, b),
            id => groups.get(id) ?? Infinity);
        const named = namedAction(terms);
        return (named ? [named, ...ranked.filter(id => id !== named)] : ranked).slice(0, LONGEST);
    }
}
