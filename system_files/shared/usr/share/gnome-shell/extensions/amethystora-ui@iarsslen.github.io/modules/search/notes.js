// Notes in search (plans/ui-ideas/spec-later.md, R6-34): offered and off, the key `notes`. While Notes is running and
// its notes are unlocked, it answers on the session bus (org.amethystora.Notes, /org/amethystora/Notes/Search) with the
// titles of the notes and open tasks that hold the words typed, and only to this shell; locked or not running there is
// no such object, and this section says nothing. Nothing ever starts Notes (NO_AUTO_START), and a question waits a
// second at most. One question for each pause in typing of three letters or more; nothing is kept between searches
// but the last answer, which Enter may still open. Enter shows the note or the task in Notes.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {pause} from './run.js';

const BUS_NAME = 'org.amethystora.Notes';
const PATH = '/org/amethystora/Notes/Search';
const INTERFACE = 'org.amethystora.Notes.Search1';
const TIMEOUT_MS = 1000;
// As the manual's: a question for a pause, not for every letter
const QUIET = 250;
const LETTERS = /\p{L}|\p{N}/gu;
const ID = /^[0-9a-f]{32}$/;
const SHOWN = 5;

export class NotesProvider {
    constructor() {
        this.id = 'amethystora-notes';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._found = new Map();
        this._previous = new Map();
    }

    destroy() {
        this._gone = true;
        this._found = new Map();
        this._previous = new Map();
    }

    _ask(words, cancellable) {
        return new Promise(resolve => {
            Gio.DBus.session.call(BUS_NAME, PATH, INTERFACE, 'Find', new GLib.Variant('(s)', [words]),
                new GLib.VariantType('(a(ssss))'), Gio.DBusCallFlags.NO_AUTO_START, TIMEOUT_MS, cancellable,
                (connection, result) => {
                    try {
                        resolve(connection.call_finish(result).deepUnpack()[0]);
                    } catch {
                        // Not running, locked, refused or cancelled: nothing
                        resolve([]);
                    }
                });
        });
    }

    async getInitialResultSet(terms, cancellable) {
        this._previous = this._found;
        this._found = new Map();
        const text = terms.join(' ');
        if ((text.match(LETTERS) ?? []).length < 3 || /^clip:/iu.test(text))
            return [];
        if (!await pause(QUIET, cancellable) || this._gone)
            return [];
        const found = await this._ask(text.slice(0, 200), cancellable);
        if (cancellable?.is_cancelled() || this._gone)
            return [];
        const ids = [];
        for (const [id, title, kind, place] of found.slice(0, SHOWN * 2)) {
            if (!ID.test(id) || !['note', 'task'].includes(kind) || !title)
                continue;
            const key = `notes:${kind}:${id}`;
            this._found.set(key, {id, title, kind, place});
            ids.push(key);
        }
        return ids;
    }

    getSubsearchResultSet(previous, terms, cancellable) {
        return this.getInitialResultSet(terms, cancellable);
    }

    getResultMetas(ids) {
        return Promise.resolve(ids.map(key => {
            const hit = this._found.get(key) ?? this._previous.get(key);
            if (!hit)
                return {id: key, name: key};
            let description;
            if (hit.kind === 'task')
                description = hit.place ? _('Notes · task · %s').format(hit.place) : _('Notes · task');
            else
                description = hit.place ? _('Notes · %s').format(hit.place) : _('Notes · note');
            return {
                id: key, name: hit.title, description,
                symbol: hit.kind === 'task' ? 'object-select-symbolic' : 'text-x-generic-symbolic',
                hint: _('Enter opens'),
            };
        }));
    }

    filterResults(ids, max) {
        return ids.slice(0, Math.min(max, SHOWN));
    }

    activateResult(key) {
        const hit = this._found.get(key) ?? this._previous.get(key);
        if (!hit)
            return;
        Gio.DBus.session.call(BUS_NAME, PATH, INTERFACE, 'Open', new GLib.Variant('(ss)', [hit.kind, hit.id]), null,
            Gio.DBusCallFlags.NO_AUTO_START, TIMEOUT_MS, null, (connection, result) => {
                try {
                    connection.call_finish(result);
                    // Notes is told to show it; the shell, which was asked, brings its window forward
                    const window = Shell.AppSystem.get_default().lookup_app('amethystora-notes.desktop')?.get_windows()[0];
                    if (window)
                        Main.activateWindow(window);
                } catch (e) {
                    console.warn(`Amethystora UI: Notes did not open the item: ${e.message}`);
                }
            });
    }
}
