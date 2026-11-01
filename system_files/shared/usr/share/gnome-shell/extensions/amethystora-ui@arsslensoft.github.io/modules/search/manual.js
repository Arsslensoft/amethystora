// The manual's pages and sections that answer what was typed, by meaning as well as by words: the agent's own index,
// read through `amethystora-agent-tools manual-search`, which reads the manual's passages only and asks nothing of
// the agent's switches (the manual is everyone's). One short process for each search of three letters or more,
// stopped when the search moves on; nothing kept between searches. Enter opens the Manual there.

import Gio from 'gi://Gio';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {output, pause} from './run.js';

const TOOLS = '/usr/libexec/amethystora-agent-tools';
const HITS = 3;
// Milliseconds of no typing before the manual is asked, on top of GNOME's own 150: one process for a pause, not for
// every letter
const QUIET = 250;
const LETTERS = /\p{L}/gu;
// What `amethystora-manual PAGE#ANCHOR` takes
const NAME = /^[a-z0-9][a-z0-9-]*$/;

export class ManualProvider {
    constructor() {
        this.id = 'amethystora-manual';
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

    async getInitialResultSet(terms, cancellable) {
        // The last search's results stay known until the next one is in: GNOME may open one of them meanwhile
        this._previous = this._found;
        this._found = new Map();
        const text = terms.join(' ');
        if ((text.match(LETTERS) ?? []).length < 3 || /^clip:/iu.test(text))
            return [];
        if (!await pause(QUIET, cancellable) || this._gone)
            return [];
        const printed = await output([TOOLS, 'manual-search', '-k', String(HITS), '--', ...terms], cancellable);
        if (cancellable?.is_cancelled() || printed === null)
            return [];
        let hits;
        try {
            hits = JSON.parse(printed);
        } catch {
            return [];
        }
        if (!Array.isArray(hits))
            return [];
        const ids = [];
        for (const hit of hits.slice(0, HITS)) {
            if (typeof hit?.id !== 'string' || typeof hit.title !== 'string' || !NAME.test(hit.page ?? '') ||
                (hit.anchor !== '' && !NAME.test(hit.anchor ?? '')))
                continue;
            this._found.set(hit.id, hit);
            ids.push(hit.id);
        }
        return ids;
    }

    getSubsearchResultSet(previous, terms, cancellable) {
        return this.getInitialResultSet(terms, cancellable);
    }

    getResultMetas(ids) {
        const gicon = Gio.ThemedIcon.new('amethystora-docs');
        return Promise.resolve(ids.map(id => {
            const hit = (this._found.get(id) ?? this._previous.get(id));
            return hit
                ? {id, name: hit.title, createIcon: size => new St.Icon({gicon, icon_size: size}), hint: _('Enter')}
                : {id, name: id};
        }));
    }

    filterResults(ids, max) {
        return ids.slice(0, max);
    }

    activateResult(id) {
        const hit = (this._found.get(id) ?? this._previous.get(id));
        if (!hit || !NAME.test(hit.page) || (hit.anchor && !NAME.test(hit.anchor)))
            return;
        const where = hit.anchor ? `${hit.page}#${hit.anchor}` : hit.page;
        try {
            Gio.Subprocess.new(['amethystora-manual', where], Gio.SubprocessFlags.NONE);
        } catch (e) {
            console.warn(`Amethystora UI: the search could not open the manual: ${e.message}`);
        }
    }
}
