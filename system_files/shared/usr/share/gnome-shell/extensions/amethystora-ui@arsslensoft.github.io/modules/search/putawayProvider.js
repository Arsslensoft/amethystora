// "Tax return" in search offers to bring the workspace put away under that name back (R7-34). Only what is in memory is searched: a
// sealed list nobody has opened this session is not read for a search (the Vault is asked only after a person's own click), and the
// names are never written anywhere. Nothing happens until Enter.

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

export class PutAwayProvider {
    constructor(ext) {
        this.id = 'amethystora-put-away';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._ext = ext;
    }

    enable() {}

    destroy() {}

    get _putAway() {
        return this._ext.modules?.get('workspaces')?.putAway ?? null;
    }

    _match(terms) {
        const wanted = terms.map(t => t.toLocaleLowerCase()).filter(Boolean);
        if (!wanted.length)
            return [];
        return (this._putAway?.items ?? []).filter(item => {
            const name = item.name.toLocaleLowerCase();
            return wanted.every(term => name.includes(term));
        }).map(item => item.name);
    }

    getInitialResultSet(terms) {
        return Promise.resolve(this._match(terms));
    }

    getSubsearchResultSet(previous, terms) {
        return this.getInitialResultSet(terms);
    }

    getResultMetas(ids) {
        return Promise.resolve(ids.map(id => ({
            id,
            name: _('Bring back “%s”').format(id),
            symbol: 'media-playback-pause-symbolic',
            description: _('Put away'),
            hint: _('Enter'),
        })));
    }

    filterResults(ids, max) {
        return ids.slice(0, max);
    }

    activateResult(id) {
        this._putAway?.bringBack(id);
    }

    // For the suites: the names a text would offer
    answerFor(text) {
        return this._match(text.split(/\s+/u));
    }
}
