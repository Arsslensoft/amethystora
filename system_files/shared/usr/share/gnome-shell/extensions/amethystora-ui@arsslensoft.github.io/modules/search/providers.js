// Amethystora's own search providers, in GNOME's own search: the clipboard (clip:), sums and units, the time in other
// places, reminders into Notes, the words people already know, Control's settings, open windows, files by what is
// remembered of them, the manual and the agent row. They are added to the overview's search controller, so the
// overview and the search box show the same sections in the same order, each drawn by section.js. Each is there while
// its key is on and taken away when it is turned off, at once.
//
// The order: the clipboard, the answer, the old words, Do it, the open windows and what is remembered first, then
// GNOME's apps and its other providers
// (Files, Settings, the calculator...) in GNOME's own order, then the manual and last the agent. GNOME's view gives the
// Enter key to the first section with a result in its list of providers and draws the sections in the order of its
// content box, so both are kept in that order: after each provider added here, and whenever GNOME adds its own again
// (it does when an app is installed or its search settings change). Those two, and the list of providers itself, are
// the only parts of the view used that GNOME does not offer; if one is not as expected, the providers are added the
// plain way, in GNOME's look and order, and the module says it is degraded.

import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {Section} from './section.js';
import {AnswersProvider} from './answers.js';
import {SettingsProvider} from './settings.js';
import {WindowsProvider} from './windows.js';
import {ManualProvider} from './manual.js';
import {ClipboardProvider} from './clipboard.js';
import {AgentProvider} from './agent.js';
import {TimesProvider} from './timesProvider.js';
import {RemindProvider} from './remindProvider.js';
import {BookmarksProvider} from './bookmarks.js';
import {NotesProvider} from './notes.js';
import {MeaningProvider} from './meaning.js';
import {WordsProvider} from './wordsProvider.js';
import {RememberedProvider} from './rememberProvider.js';

// The key that turns each on, its place, and how many rows it shows at most; GNOME's own come between 4 and 6
const PROVIDERS = [
    {key: 'clipboard', rank: 0, max: 8},
    {key: 'answers', rank: 1, max: 1},
    {key: 'times', rank: 1, max: 2},
    {key: 'reminders', rank: 1, max: 2},
    {key: 'old-words', rank: 1, max: 3},
    {key: 'settings', rank: 2, max: 5},
    {key: 'windows', rank: 3, max: 5},
    // Before GNOME's own, whose forgiving app search finds something in almost any few words and would take Enter. It
    // answers only two words or more that a rule read
    {key: 'remembered', rank: 3, max: 5},
    {key: 'manual', rank: 6, max: 3},
    // Offered and off (plans/ui-ideas/spec-later.md): after GNOME's own, before the manual's place in the list
    {key: 'notes', rank: 5, max: 5},
    {key: 'bookmarks', rank: 5, max: 5},
    {key: 'meaning', rank: 5, max: 3},
    {key: 'agent', rank: 7, max: 1},
];
const GNOME_RANK = 4;

// The session's language as a BCP 47 tag, for upper-casing the sections' titles in its own way
function sessionLocale() {
    const name = GLib.get_language_names_with_category('LC_MESSAGES')[0] ?? 'C';
    const tag = name.split(/[.@]/)[0].replace('_', '-');
    return tag === 'C' || tag === 'POSIX' ? 'en' : tag;
}

export class Providers {
    constructor(ext, host) {
        this._ext = ext;
        this._host = host;
        this.degraded = false;
        this._live = new Map();
    }

    enable(settings) {
        this._settings = settings;
        const view = Main.overview.searchController?._searchResults;
        this._view = view;
        // The view's list of providers and its content box, as GNOME 50 and 51 have them
        const ok = view && Array.isArray(view._providers) && view._content instanceof St.BoxLayout &&
            view._content.name === 'searchResultsContent';
        if (!ok) {
            this.degraded = true;
            console.warn('Amethystora UI: search sections fall back to GNOME\'s display');
        } else {
            view._content.connectObject('child-added', () => this._queueOrder(), this);
        }
        for (const {key} of PROVIDERS)
            settings.connectObject(`changed::${key}`, () => this._sync(key), this);
        for (const {key} of PROVIDERS)
            this._sync(key);
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._settings = null;
        this._view?._content?.disconnectObject?.(this);
        if (this._orderLater)
            global.compositor.get_laters().remove(this._orderLater);
        this._orderLater = 0;
        for (const key of [...this._live.keys()])
            this._remove(key);
        this._view = null;
    }

    get(key) {
        return this._live.get(key) ?? null;
    }

    // The search box opens: Control's list is read again if it is old
    prepare() {
        this._live.get('settings')?.prepare();
    }

    _make(key) {
        switch (key) {
        case 'answers': return new AnswersProvider();
        case 'settings': return new SettingsProvider();
        case 'windows': return new WindowsProvider();
        case 'manual': return new ManualProvider();
        case 'clipboard': return new ClipboardProvider(this._host);
        case 'agent': return new AgentProvider(this._ext);
        case 'times': return new TimesProvider();
        case 'reminders': return new RemindProvider();
        case 'bookmarks': return new BookmarksProvider();
        case 'notes': return new NotesProvider();
        case 'meaning': return new MeaningProvider();
        case 'old-words': return new WordsProvider();
        case 'remembered': return new RememberedProvider();
        }
        return null;
    }

    _titles(key) {
        return {
            clipboard: {
                title: _('Clipboard history'),
                note: _('The history Amethystora Clipboard already keeps, in memory. Search stores nothing new.'),
            },
            answers: {},
            times: {},
            reminders: {title: _('Remind me')},
            settings: {title: _('Do it')},
            'old-words': {title: _('Here it is')},
            remembered: {title: _('By what you remember'), note: _('Also understood: kinds of file, dates and sizes.')},
            windows: {title: _('Open windows')},
            manual: {title: _('Manual')},
            bookmarks: {title: _('Bookmarks'), note: _('From a copy of Firefox’s bookmarks, never its history.')},
            notes: {title: _('Notes'), tag: _('While Notes is unlocked')},
            meaning: {title: _('Files by meaning'), tag: _('On this computer')},
            agent: {title: _('The agent'), tag: _('Only while it is on')},
        }[key];
    }

    _sync(key) {
        const on = this._settings.get_boolean(key);
        if (on && !this._live.has(key))
            this._add(key);
        else if (!on && this._live.has(key))
            this._remove(key);
    }

    _add(key) {
        const {max} = PROVIDERS.find(entry => entry.key === key);
        const provider = this._make(key);
        provider.enable?.();
        provider.amethystoraRank = PROVIDERS.find(entry => entry.key === key).rank;
        provider.changed = () => this._searchAgain(provider);
        const controller = Main.overview.searchController;
        if (this.degraded) {
            // GNOME's grid draws it: every result needs a picture there
            const metas = provider.getResultMetas.bind(provider);
            provider.getResultMetas = async (...args) => (await metas(...args)).map(meta => ({
                ...meta, createIcon: meta.createIcon ?? (size => new St.Icon({icon_name: 'edit-find-symbolic', icon_size: size})),
            }));
            controller.addProvider(provider);
        } else {
            provider.display = new Section(provider, this._view, {...this._titles(key), max, locale: sessionLocale()});
            this._view._content.add_child(provider.display);
            controller.addProvider(provider);
            this._order();
        }
        this._live.set(key, provider);
    }

    _remove(key) {
        const provider = this._live.get(key);
        this._live.delete(key);
        if (!provider)
            return;
        // GNOME destroys the display of a provider it has; a section of ours it never took is ours to destroy (degraded,
        // the display is GNOME's own)
        const known = !this.degraded && this._view._providers.includes(provider);
        Main.overview.searchController.removeProvider(provider);
        if (!this.degraded && !known)
            provider.display?.destroy();
        provider.display = null;
        provider.destroy?.();
    }

    // A provider's own answer changed (Control's list read, the agent turned on or off): its section is searched again
    // for the text already typed, and the view picks the result Enter opens again
    async _searchAgain(provider) {
        const view = this._view;
        const terms = view?.terms ?? [];
        if (this.degraded || !terms.length || !(provider.display instanceof Section) || !this._live.has(this._keyOf(provider)))
            return;
        const ids = await provider.getInitialResultSet(terms, null);
        if (view.terms !== terms || !(provider.display instanceof Section))
            return;
        await provider.display.updateSearch(ids, terms);
        view._maybeSetInitialSelection?.();
        view._updateSearchProgress?.();
    }

    _keyOf(provider) {
        return [...this._live].find(([, live]) => live === provider)?.[0] ?? null;
    }

    _queueOrder() {
        if (this._ordering || this._orderLater)
            return;
        this._orderLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._orderLater = 0;
            this._order();
            return GLib.SOURCE_REMOVE;
        });
    }

    // The view's providers and their sections in the order above; GNOME's own keep their order among themselves
    _order() {
        const view = this._view;
        if (!view || this.degraded)
            return;
        const providers = view._providers;
        const rankOf = (provider, index) => (provider.amethystoraRank ?? GNOME_RANK) * 1000 + index;
        const sorted = providers.map((provider, index) => [provider, rankOf(provider, index)])
            .sort((a, b) => a[1] - b[1]).map(([provider]) => provider);
        this._ordering = true;
        try {
            providers.splice(0, providers.length, ...sorted);
            const content = view._content;
            let at = 0;
            for (const provider of sorted) {
                const display = provider.display;
                // A section already destroyed (its provider not yet out of the list) is passed over
                if (display && !display._gone && display.get_parent() === content)
                    content.set_child_at_index(display, at++);
            }
        } finally {
            this._ordering = false;
        }
    }

    // Each section the view shows now, in its order: ours and GNOME's, for the box's Ctrl keys and the tests
    sections(all = false) {
        const content = this._view?._content;
        if (!content)
            return [];
        return content.get_children().filter(child => all || child.visible)
            .map(display => ({display, provider: display.provider}))
            .filter(({provider}) => provider);
    }
}

// The results a section shows, in order, whoever draws it: our rows, GNOME's list rows, GNOME's app tiles
export function resultsOf(display) {
    if (display instanceof Section)
        return display.rows.filter(row => row.visible);
    const found = [];
    const walk = actor => {
        for (const child of actor.get_children()) {
            if (!child.visible)
                continue;
            if (child instanceof St.Button && child.provider !== undefined && 'metaInfo' in child)
                found.push(child);
            else if (child instanceof St.Button && child.app && child.can_focus)
                found.push(child);
            else if (!(child instanceof St.Button))
                walk(child);
        }
    };
    walk(display);
    return found.filter(result => result.can_focus !== false && result.has_allocation?.() !== false);
}

