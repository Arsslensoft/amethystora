// Files by meaning (plans/ui-ideas/spec-later.md, R5-37): offered and off, the key `meaning`. Files in Documents found
// by what they are about rather than by their name or a word in them: `amethystora-agent-tools files-search` compares
// what was typed with the account's own index, which amethystora-file-meaning works out once a day on mains power from
// the text GNOME's indexer already read, with the agent's small embedding model. One short process for each pause in a
// search of two words or more, stopped when the search moves on; nothing kept between searches. Enter opens the file.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {output, pause} from './run.js';

const TOOLS = '/usr/libexec/amethystora-agent-tools';
const HITS = 3;
// A little longer than the manual's: the model is loaded for each question
const QUIET = 350;
const LETTERS = /\p{L}/gu;

export class MeaningProvider {
    constructor() {
        this.id = 'amethystora-meaning';
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

    // Documents, as GLib names it; never the home itself
    _documents() {
        const folder = GLib.get_user_special_dir(GLib.UserDirectory.DIRECTORY_DOCUMENTS);
        return folder && folder !== GLib.get_home_dir() ? folder : null;
    }

    async getInitialResultSet(terms, cancellable) {
        this._previous = this._found;
        this._found = new Map();
        const text = terms.join(' ');
        const words = text.split(/\s+/u).filter(Boolean);
        if (words.length < 2 || (text.match(LETTERS) ?? []).length < 6 || /^clip:/iu.test(text))
            return [];
        const documents = this._documents();
        if (!documents || !await pause(QUIET, cancellable) || this._gone)
            return [];
        const printed = await output([TOOLS, 'files-search', '-k', String(HITS), '--stdin', '--'], cancellable, words.join(' '));
        if (cancellable?.is_cancelled() || printed === null || this._gone)
            return [];
        let hits;
        try {
            hits = JSON.parse(printed);
        } catch {
            return [];
        }
        const ids = [];
        for (const hit of Array.isArray(hits) ? hits.slice(0, HITS) : []) {
            if (typeof hit?.path !== 'string' || !hit.path.startsWith(`${documents}/`) || hit.path.includes('/../'))
                continue;
            const id = `meaning:${hit.path}`;
            this._found.set(id, hit.path);
            ids.push(id);
        }
        return ids;
    }

    getSubsearchResultSet(previous, terms, cancellable) {
        return this.getInitialResultSet(terms, cancellable);
    }

    getResultMetas(ids) {
        const documents = this._documents() ?? '';
        return Promise.resolve(ids.map(id => {
            const path = this._found.get(id) ?? this._previous.get(id);
            if (!path)
                return {id, name: id};
            const name = GLib.path_get_basename(path);
            // Where it is, as a breadcrumb from Documents: "Documents › Taxes"
            const folders = GLib.path_get_dirname(path).slice(documents.length).split('/').filter(Boolean);
            const where = [GLib.path_get_basename(documents), ...folders].join(' › ');
            const [type] = Gio.content_type_guess(name, null);
            const gicon = Gio.content_type_get_icon(type);
            return {
                id, name, description: _('%s · by meaning').format(where), descriptionLtr: true,
                createIcon: size => new St.Icon({gicon, icon_size: size}), hint: _('Enter opens'),
            };
        }));
    }

    filterResults(ids, max) {
        return ids.slice(0, Math.min(max, HITS));
    }

    activateResult(id) {
        const path = this._found.get(id) ?? this._previous.get(id);
        const documents = this._documents();
        if (!path || !documents || !path.startsWith(`${documents}/`))
            return;
        // Not on the compositor's thread (a slow mount), and the error is not logged: its text names the file
        Gio.AppInfo.launch_default_for_uri_async(Gio.File.new_for_path(path).get_uri(), global.create_app_launch_context(0, -1), null,
            (_source, result) => {
                try {
                    Gio.AppInfo.launch_default_for_uri_finish(result);
                } catch (e) {
                    console.warn(`Amethystora UI: the file did not open (error ${e.code ?? 'unknown'})`);
                }
            });
    }
}
