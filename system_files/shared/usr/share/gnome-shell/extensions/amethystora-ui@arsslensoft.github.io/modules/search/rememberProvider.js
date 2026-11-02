// Find it by what you remember (plans/ui-ideas/r7/spec-find.md, R7-60): "pdf from last week" finds the file from the index
// GNOME already keeps. The rules are in remember.js and run as the words are typed; only when they read something, after
// a pause in the typing, one short process (amethystora-file-find) asks the index and is stopped when the search moves on.
// The words go to it on standard input. Nothing is kept between searches, and the file names shown are plain text.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {compileRemember, parseRemembered, rememberWords} from './remember.js';
import {appNamedLike, localeTag} from './words.js';
import {output, pause} from './run.js';

const HELPER = '/usr/libexec/amethystora-file-find';
// Milliseconds of no typing before the index is asked, on top of GNOME's own 150, as the manual's row waits
const QUIET = 250;
// A file from up to this many days ago is said by its weekday
const WEEK = 6;

export class RememberedProvider {
    constructor() {
        this.id = 'amethystora-remembered';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._found = new Map();
        this._previous = new Map();
        this._sizes = false;
    }

    destroy() {
        this._gone = true;
        this._found = new Map();
        this._previous = new Map();
        this._compiled = null;
    }

    _rules() {
        if (!this._compiled) {
            this._locale = localeTag(GLib.get_language_names_with_category('LC_MESSAGES')[0]);
            this._compiled = compileRemember(rememberWords(text => text), rememberWords(_), this._locale);
        }
        return this._compiled;
    }

    async getInitialResultSet(terms, cancellable) {
        this._previous = this._found;
        this._found = new Map();
        const text = terms.join(' ');
        if (/^clip:/iu.test(text))
            return [];
        const question = parseRemembered(text, this._rules());
        if (!question)
            return [];
        // An installed app named like what was typed keeps Enter ("document sc" on its way to Document Scanner)
        const installed = Shell.AppSystem.get_default().get_installed().map(info => [info.get_name() ?? '', info.get_id()]);
        if (appNamedLike(installed, terms, this._locale))
            return [];
        if (!await pause(QUIET, cancellable) || this._gone)
            return [];
        const printed = await output([HELPER], cancellable, JSON.stringify(question));
        if (cancellable?.is_cancelled() || printed === null || this._gone)
            return [];
        let hits;
        try {
            hits = JSON.parse(printed);
        } catch {
            return [];
        }
        const home = GLib.get_home_dir();
        const ids = [];
        for (const hit of Array.isArray(hits) ? hits : []) {
            if (typeof hit?.path !== 'string' || !hit.path.startsWith(`${home}/`) || hit.path.includes('/../'))
                continue;
            const id = `remembered:${hit.path}`;
            this._found.set(id, {...hit, bySize: question.order !== 'modified'});
            ids.push(id);
        }
        return ids;
    }

    getSubsearchResultSet(previous, terms, cancellable) {
        return this.getInitialResultSet(terms, cancellable);
    }

    // "Thursday", or a date when it is older than a week
    _when(seconds) {
        const then = new Date(seconds * 1000);
        const now = new Date();
        const midnight = date => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
        const days = Math.round((midnight(now) - midnight(then)) / 86400000);
        const format = days >= 0 && days <= WEEK ? {weekday: 'long'} : {dateStyle: 'medium'};
        return new Intl.DateTimeFormat(this._locale, format).format(then);
    }

    // Where it is, from the home: "Documents › Taxes"; the home's own files say so
    _where(path) {
        const home = GLib.get_home_dir();
        const folders = GLib.path_get_dirname(path).slice(home.length).split('/').filter(Boolean);
        return folders.length ? folders.join(' › ') : _('Home');
    }

    getResultMetas(ids) {
        return Promise.resolve(ids.map(id => {
            const hit = this._found.get(id) ?? this._previous.get(id);
            if (!hit)
                return {id, name: id};
            const name = GLib.path_get_basename(hit.path);
            const about = hit.bySize ? GLib.format_size(hit.size) : this._when(hit.modified);
            const [type] = Gio.content_type_guess(name, null);
            const gicon = Gio.content_type_get_icon(type);
            return {
                id, name, description: `${this._where(hit.path)} · ${about}`,
                createIcon: size => new St.Icon({gicon, icon_size: size}), hint: _('Enter opens'),
            };
        }));
    }

    filterResults(ids, max) {
        return ids.slice(0, max);
    }

    activateResult(id) {
        const hit = this._found.get(id) ?? this._previous.get(id);
        if (!hit || !hit.path.startsWith(`${GLib.get_home_dir()}/`))
            return;
        // Not on the compositor's thread (a slow mount), and the error is not logged: its text names the file
        Gio.AppInfo.launch_default_for_uri_async(Gio.File.new_for_path(hit.path).get_uri(), global.create_app_launch_context(0, -1), null,
            (_source, result) => {
                try {
                    Gio.AppInfo.launch_default_for_uri_finish(result);
                } catch (e) {
                    console.warn(`Amethystora UI: the file did not open (error ${e.code ?? 'unknown'})`);
                }
            });
    }
}
