// Firefox's bookmarks, never its history (plans/ui-ideas/spec-later.md, R6-35): offered and off, the key `bookmarks`.
// The list comes from /usr/libexec/amethystora-bookmarks, which reads a copy of Firefox's bookmarks file and deletes
// the copy before it answers (Firefox is left as it is): once when a search needs it, then kept in memory while
// searches follow one another, and dropped two minutes after the last one, so that a new bookmark shows the next
// time and nothing of it lingers. Every word typed has to be in the title, the address or the folder. Enter opens the
// address in the default browser, and nothing opens by itself.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {fold} from '../fuzzy/matcher.js';
import {output} from './run.js';

const HELPER = '/usr/libexec/amethystora-bookmarks';
// The list is let go this long after the last search that used it
const KEEP_S = 120;
const SHOWN = 5;
const LETTERS = /\p{L}|\p{N}/gu;
const SCHEMES = /^(https?|file):\/\//i;

// The host of an address, for the line under a bookmark
function hostOf(url) {
    try {
        return GLib.Uri.parse(url, GLib.UriFlags.NONE).get_host() ?? '';
    } catch {
        return '';
    }
}

export class BookmarksProvider {
    constructor() {
        this.id = 'amethystora-bookmarks';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._found = new Map();
        this._previous = new Map();
        this._list = null;
        this._loading = null;
    }

    destroy() {
        this._gone = true;
        if (this._dropId)
            GLib.source_remove(this._dropId);
        this._dropId = 0;
        this._list = null;
        this._found = new Map();
        this._previous = new Map();
    }

    // The list, read once for a run of searches
    _bookmarks() {
        if (this._dropId)
            GLib.source_remove(this._dropId);
        this._dropId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, KEEP_S, () => {
            this._dropId = 0;
            this._list = null;
            return GLib.SOURCE_REMOVE;
        });
        if (this._list)
            return Promise.resolve(this._list);
        this._loading ??= output([HELPER, 'list']).then(printed => {
            this._loading = null;
            let list = [];
            try {
                list = JSON.parse(printed ?? '[]');
            } catch {}
            if (this._gone || !Array.isArray(list))
                return [];
            this._list = list.filter(item => typeof item?.title === 'string' && typeof item.url === 'string' &&
                SCHEMES.test(item.url)).map(item => ({
                ...item, folder: typeof item.folder === 'string' ? item.folder : '',
                words: fold(`${item.title} ${item.url} ${item.folder ?? ''}`),
            }));
            return this._list;
        });
        return this._loading;
    }

    async getInitialResultSet(terms) {
        this._previous = this._found;
        this._found = new Map();
        const text = terms.join(' ');
        if ((text.match(LETTERS) ?? []).length < 2 || /^clip:/iu.test(text))
            return [];
        const list = await this._bookmarks();
        const wanted = fold(text).split(/\s+/u).filter(Boolean);
        const ids = [];
        list.forEach((item, index) => {
            if (ids.length >= SHOWN * 4 || !wanted.every(word => item.words.includes(word)))
                return;
            const id = `bookmark:${index}`;
            this._found.set(id, item);
            ids.push(id);
        });
        return ids;
    }

    getSubsearchResultSet(previous, terms) {
        return this.getInitialResultSet(terms);
    }

    getResultMetas(ids) {
        return Promise.resolve(ids.map(id => {
            const item = this._found.get(id) ?? this._previous.get(id);
            if (!item)
                return {id, name: id};
            const description = item.folder
                ? _('bookmark · in “%s”').format(item.folder)
                : _('bookmark · %s').format(hostOf(item.url) || item.url);
            return {id, name: item.title, description, symbol: 'starred-symbolic', hint: _('Enter opens')};
        }));
    }

    filterResults(ids, max) {
        return ids.slice(0, Math.min(max, SHOWN));
    }

    activateResult(id) {
        const item = this._found.get(id) ?? this._previous.get(id);
        if (!item || !SCHEMES.test(item.url))
            return;
        try {
            Gio.AppInfo.launch_default_for_uri(item.url, global.create_app_launch_context(0, -1));
        } catch (e) {
            console.warn(`Amethystora UI: the bookmark did not open: ${e.message}`);
        }
    }
}
