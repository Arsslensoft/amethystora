// "Do it": Control's own settings, found by their words and changed with Enter. The list is Control's
// (amethystora-control-list --json), read in the background the first time a search starts and again when the copy
// is a minute old or a change was made from here; never at login. A row either sets one of a setting's other values
// (Phone pairing: On), runs an action, or opens the setting in Control. Matching is strict, every word typed the start
// of a word, because Enter may change something at once: only a setting's own words can change it, and its sentence's
// only open it. The only commands run are the list's own, and only when
// they are an `ame` command; one that asks something or needs a password opens in a terminal. Nothing of security is
// in Control's list, so nothing here switches a security feature. The whole name of a GNOME power action asks for that
// action, and Enter must not change a setting that only shares its words.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {namedAction} from '../fuzzy.js';
import {words} from '../fuzzy/matcher.js';
import {finished, output} from './run.js';

const LIST = '/usr/libexec/amethystora-control-list';
const IN_TERMINAL = '/usr/libexec/amethystora-in-terminal';
// Seconds a copy of the list is used before it is read again
const FRESH = 60;
// The rows of each kind, at most
const CHOICES = 3;
const OPENS = 2;
// A word of the entry's sentence counts from this many letters typed
const TEXT_WORD = 4;
// Control's pages, which `amethystora-control PAGE` opens
const PAGES = ['appearance', 'desktop', 'apps', 'agent', 'system', 'setup'];

const isArgv = value => Array.isArray(value) && value.length > 0 && value.every(word => typeof word === 'string');
// An `ame` command that runs as it is written: Control fills a {value} or a {key} in from what its window asks, which the
// search never has, so such an entry is offered only as Control › …
const PLACEHOLDER = /\{[a-z]+\}/;
const runnable = argv => isArgv(argv) && argv[0] === 'ame' && !argv.some(word => PLACEHOLDER.test(word));

export class SettingsProvider {
    constructor() {
        this.id = 'amethystora-settings';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._items = null;
        this._readAt = 0;
        this._reading = null;
        // What the last search found, and the one before, by ids that stay the same from one search to the next: GNOME may
        // open a row of the last search after the next has begun (Enter while its search was still waiting)
        this._found = new Map();
        this._previous = new Map();
    }

    destroy() {
        this._cancellable?.cancel();
        this._items = null;
        this._found = new Map();
        this._previous = new Map();
    }

    // Read the list again in the background when it is missing or a minute old; a search meanwhile uses the copy
    // it has. The search box asks this as it opens.
    prepare() {
        if (this._reading || (this._readAt && GLib.get_monotonic_time() - this._readAt < FRESH * 1e6))
            return;
        this._read();
    }

    _read() {
        this._cancellable?.cancel();
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        this._reading = output([LIST, '--json'], cancellable).then(text => {
            if (cancellable.is_cancelled())
                return;
            this._reading = null;
            // A list that could not be read is asked for again only after a minute, like an old one
            this._readAt = GLib.get_monotonic_time();
            try {
                const items = JSON.parse(text ?? '').items;
                if (Array.isArray(items)) {
                    this._items = items.filter(item => item && typeof item.id === 'string' &&
                        typeof item.title === 'string' && PAGES.includes(item.page));
                    this._readAt = GLib.get_monotonic_time();
                    // A search already under way shows what the list has now
                    this.changed?.();
                }
            } catch {
                console.warn('Amethystora UI: the search could not read Control\'s list');
            }
        });
    }

    // Control's page headings, in the words Control's list gives them
    _headings() {
        return {
            appearance: _('Appearance'), desktop: _('Desktop'), apps: _('Apps'), agent: _('The AI agent'),
            system: _('System'), setup: _('Your setup'),
        };
    }

    getInitialResultSet(terms) {
        this.prepare();
        const found = new Map();
        this._previous = this._found;
        this._found = found;
        const typed = terms.flatMap(words);
        if (!this._items || typed.length === 0 || /^clip:/iu.test(terms.join(' ')) || namedAction(terms))
            return Promise.resolve([]);

        const headings = this._headings();
        // Words of a setting's sentence find it only when no app answers to them either: Enter is the app's
        const appNamed = Shell.AppSystem.search(typed.join(' ')).length > 0;
        const startsAll = (list, also = []) => typed.every(word =>
            list.some(other => other.startsWith(word)) ||
            (word.length >= TEXT_WORD && also.some(other => other.startsWith(word))));
        const choices = [];
        const opens = [];
        this._items.forEach((item, order) => {
            const entry = [...words(item.title), ...words(item.group), ...words(headings[item.page])];
            const sentence = words(item.text);
            const current = Array.isArray(item.choices)
                ? item.choices.find(choice => choice?.value === item.state) ?? null : null;

            if (item.available !== false) {
                for (const choice of Array.isArray(item.choices) ? item.choices : []) {
                    if (!choice || choice.value === item.state || typeof choice.title !== 'string' ||
                        typeof choice.value !== 'string' || !runnable(choice.command))
                        continue;
                    const own = words(choice.title);
                    if (!startsAll([...own, ...entry]))
                        continue;
                    const rank = typed.some(word => own.some(other => other.startsWith(word))) ? 0
                        : typed.some(word => words(item.title).some(other => other.startsWith(word))) ? 1 : 2;
                    choices.push({
                        rank, order, id: `set:${item.id}:${choice.value}`, name: item.title,
                        description: choice.title, command: choice.command, terminal: choice.terminal === true,
                    });
                }
                if (item.kind === 'action' && runnable(item.command) && startsAll(entry)) {
                    const rank = typed.some(word => words(item.title).some(other => other.startsWith(word))) ? 1 : 2;
                    choices.push({
                        rank, order, id: `act:${item.id}`, name: item.title,
                        description: (item.button ?? '').replace(/[….]+$/u, ''), command: item.command,
                        terminal: item.terminal !== false,
                    });
                }
            }
            if (startsAll(entry, appNamed ? [] : sentence)) {
                opens.push({
                    rank: typed.some(word => words(item.title).some(other => other.startsWith(word))) ? 1 : 2,
                    order, id: `open:${item.id}`, name: _('Control › %s').format(item.title),
                    description: typeof current?.title === 'string' ? current.title : '', page: item.page,
                });
            }
        });
        const byRank = (a, b) => a.rank - b.rank || a.order - b.order;
        const shown = [...choices.sort(byRank).slice(0, CHOICES), ...opens.sort(byRank).slice(0, OPENS)];
        for (const result of shown)
            found.set(result.id, result);
        return Promise.resolve(shown.map(result => result.id));
    }

    getSubsearchResultSet(previous, terms) {
        return this.getInitialResultSet(terms);
    }

    getResultMetas(ids) {
        const gicon = Gio.ThemedIcon.new('amethystora-control');
        return Promise.resolve(ids.map(id => {
            const result = this._found.get(id) ?? this._previous.get(id);
            if (!result)
                return {id, name: id};
            return {
                id, name: result.name, description: result.description,
                gicon, giconSize: 20,
                hint: result.terminal ? _('Enter opens a terminal') : _('Enter'),
            };
        }));
    }

    filterResults(ids, max) {
        return ids.slice(0, max);
    }

    activateResult(id) {
        const result = this._found.get(id) ?? this._previous.get(id);
        if (!result)
            return;
        if (result.page) {
            if (PAGES.includes(result.page))
                finished(['amethystora-control', result.page]);
            return;
        }
        // Only what Control's list gave, and only an `ame` command as it is written
        if (!runnable(result.command)) {
            console.warn(`Amethystora UI: the search did not run ${id}: not an ame command`);
            return;
        }
        const argv = result.terminal ? [IN_TERMINAL, '--', ...result.command] : result.command;
        finished(argv).then(ok => {
            if (!ok && !result.terminal)
                console.warn(`Amethystora UI: the search's change ${id} failed`);
            // What it changed shows the next time
            this._items && this._read();
        });
    }
}
